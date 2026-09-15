//! SoftGPU Phase 8 sanitizers (functional fidelity).
//!
//! Declared subset (honest, not AMDGPU memory-model evidence):
//! - Shadow tracks allocated / uninitialized / initialized / freed per byte
//! - SoftGPU race: different workitems, overlapping bytes, ≥1 non-atomic write,
//!   same workgroup, same barrier generation (no SoftGPU barrier between them)
//! - Cross-workgroup non-atomic conflicting global accesses are findings
//! - SoftGPU atomics on the same address do not race with each other under the
//!   SoftGPU sequential interpreter; they still mark bytes initialized
//! - Divergent barriers remain validate-time errors (Phase 7)
//!
//! Blind spots (documented): full GPU memory orders, silent aliasing through
//! host pointers, and true hardware concurrency are out of scope.

use crate::error::{FunctionalError, Result};
use crate::exec::{ExecConfig, SchedulePolicy};
use crate::ir::{AddrSpace, Program, TypeId};
use crate::memory::ty_size;
use serde::{Deserialize, Serialize};

pub const SANITIZER_REPLAY_SCHEMA: &str = "softgpu-sanitizer-replay-v1";
pub const MAX_SHADOW_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SanitizeMode {
    /// No shadow / race instrumentation.
    Off,
    /// Collect findings; execution continues unless a hard bounds fault occurs.
    Collect,
    /// Stop at the first sanitizer finding.
    FailFast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    OutOfBounds,
    UseAfterFree,
    UninitializedRead,
    Race,
    MissingBarrier,
    CrossWorkgroupRace,
    ShadowLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct WorkItemId {
    pub workgroup: [u32; 3],
    pub wave: u32,
    pub lane: u32,
    pub flat_local: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub kind: FindingKind,
    pub space: AddrSpace,
    pub addr: u64,
    pub size: usize,
    pub step: u64,
    pub barrier_gen: u64,
    pub actor: WorkItemId,
    pub other: Option<WorkItemId>,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SanitizeReport {
    pub fidelity: &'static str,
    pub mode: &'static str,
    pub note: &'static str,
    pub findings: Vec<Finding>,
}

impl SanitizeReport {
    pub fn clean() -> Self {
        Self {
            fidelity: "sanitized",
            mode: "softgpu_functional_sanitizer_v1",
            note: "not_gfx1201_isa_emulation",
            findings: Vec::new(),
        }
    }

    pub fn ok(&self) -> bool {
        self.findings.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayBundle {
    pub schema: String,
    pub program_name: String,
    pub source_provenance: String,
    pub exec: ReplayExec,
    pub finding: Finding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayExec {
    pub grid: [u32; 3],
    pub workgroup: [u32; 3],
    pub wave_size: u32,
    pub group_bytes: u32,
    pub schedule: SchedulePolicy,
    pub step_budget: u64,
}

impl ReplayBundle {
    pub fn from_finding(program: &Program, cfg: &ExecConfig, finding: Finding) -> Self {
        Self {
            schema: SANITIZER_REPLAY_SCHEMA.into(),
            program_name: program.name.clone(),
            source_provenance: program.source_provenance.clone(),
            exec: ReplayExec {
                grid: cfg.launch.grid,
                workgroup: cfg.launch.workgroup,
                wave_size: cfg.wave_size,
                group_bytes: cfg.group_bytes.max(program.group_bytes),
                schedule: cfg.schedule,
                step_budget: cfg.step_budget,
            },
            finding,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ByteState {
    Unallocated,
    Uninitialized,
    Initialized,
    Freed,
}

#[derive(Debug, Clone, Copy)]
struct AccessRecord {
    actor: WorkItemId,
    barrier_gen: u64,
    #[allow(dead_code)]
    step: u64,
    is_write: bool,
    is_atomic: bool,
}

#[derive(Debug, Clone)]
struct ByteShadow {
    state: ByteState,
    last: Option<AccessRecord>,
}

impl Default for ByteShadow {
    fn default() -> Self {
        Self {
            state: ByteState::Unallocated,
            last: None,
        }
    }
}

#[derive(Debug)]
pub struct Sanitizer {
    mode: SanitizeMode,
    global: Vec<ByteShadow>,
    group: Vec<ByteShadow>,
    findings: Vec<Finding>,
    /// SoftGPU WG-local barrier generation (bumped between barrier segments).
    pub barrier_gen: u64,
    pub step: u64,
}

impl Sanitizer {
    pub fn new(mode: SanitizeMode, global_len: usize, group_len: usize) -> Result<Self> {
        if mode == SanitizeMode::Off {
            return Ok(Self {
                mode,
                global: Vec::new(),
                group: Vec::new(),
                findings: Vec::new(),
                barrier_gen: 0,
                step: 0,
            });
        }
        let total = global_len.saturating_add(group_len);
        if total > MAX_SHADOW_BYTES {
            return Err(FunctionalError::Sanitize(Finding {
                kind: FindingKind::ShadowLimit,
                space: AddrSpace::Global,
                addr: 0,
                size: total,
                step: 0,
                barrier_gen: 0,
                actor: WorkItemId {
                    workgroup: [0, 0, 0],
                    wave: 0,
                    lane: 0,
                    flat_local: 0,
                },
                other: None,
                detail: format!("shadow bytes {total} > SoftGPU max {MAX_SHADOW_BYTES}"),
            }));
        }
        let mut s = Self {
            mode,
            global: vec![ByteShadow::default(); global_len],
            group: vec![ByteShadow::default(); group_len],
            findings: Vec::new(),
            barrier_gen: 0,
            step: 0,
        };
        // Host-visible global arena bytes are SoftGPU-initialized at launch; group
        // memory starts uninitialized each workgroup (see reset_group).
        s.mark_initialized(AddrSpace::Global, 0, global_len);
        s.mark_allocated(AddrSpace::Group, 0, group_len);
        Ok(s)
    }

    pub fn mode(&self) -> SanitizeMode {
        self.mode
    }

    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    pub fn into_report(self) -> SanitizeReport {
        let mut r = SanitizeReport::clean();
        r.findings = self.findings;
        r
    }

    pub fn reset_group(&mut self) {
        for b in &mut self.group {
            *b = ByteShadow {
                state: ByteState::Uninitialized,
                last: None,
            };
        }
    }

    pub fn mark_allocated(&mut self, space: AddrSpace, addr: u64, len: usize) {
        if self.mode == SanitizeMode::Off || len == 0 {
            return;
        }
        let map = self.map_mut(space);
        let start = addr as usize;
        let end = (start + len).min(map.len());
        for b in map.iter_mut().take(end).skip(start) {
            b.state = ByteState::Uninitialized;
            b.last = None;
        }
    }

    pub fn mark_initialized(&mut self, space: AddrSpace, addr: u64, len: usize) {
        if self.mode == SanitizeMode::Off || len == 0 {
            return;
        }
        let map = self.map_mut(space);
        let start = addr as usize;
        let end = (start + len).min(map.len());
        for b in map.iter_mut().take(end).skip(start) {
            b.state = ByteState::Initialized;
            b.last = None;
        }
    }

    pub fn mark_uninitialized(&mut self, space: AddrSpace, addr: u64, len: usize) {
        self.mark_allocated(space, addr, len);
    }

    pub fn mark_freed(&mut self, space: AddrSpace, addr: u64, len: usize) {
        if self.mode == SanitizeMode::Off || len == 0 {
            return;
        }
        let map = self.map_mut(space);
        let start = addr as usize;
        let end = (start + len).min(map.len());
        for b in map.iter_mut().take(end).skip(start) {
            b.state = ByteState::Freed;
            b.last = None;
        }
    }

    pub fn note_barrier(&mut self) {
        self.barrier_gen = self.barrier_gen.saturating_add(1);
    }

    pub fn on_access(
        &mut self,
        space: AddrSpace,
        addr: u64,
        ty: TypeId,
        is_write: bool,
        is_atomic: bool,
        actor: WorkItemId,
    ) -> Result<()> {
        if self.mode == SanitizeMode::Off {
            return Ok(());
        }
        let size = ty_size(ty);
        let map_len = self.map(space).len();
        let start = match usize::try_from(addr) {
            Ok(s) => s,
            Err(_) => {
                return self.push_finding(Finding {
                    kind: FindingKind::OutOfBounds,
                    space,
                    addr,
                    size,
                    step: self.step,
                    barrier_gen: self.barrier_gen,
                    actor,
                    other: None,
                    detail: "address does not fit SoftGPU arena".into(),
                });
            }
        };
        let end = match start.checked_add(size) {
            Some(e) => e,
            None => {
                return self.push_finding(Finding {
                    kind: FindingKind::OutOfBounds,
                    space,
                    addr,
                    size,
                    step: self.step,
                    barrier_gen: self.barrier_gen,
                    actor,
                    other: None,
                    detail: "access size overflow".into(),
                });
            }
        };
        if end > map_len {
            return self.push_finding(Finding {
                kind: FindingKind::OutOfBounds,
                space,
                addr,
                size,
                step: self.step,
                barrier_gen: self.barrier_gen,
                actor,
                other: None,
                detail: format!("addr+size exceeds arena_len={map_len}"),
            });
        }

        for off in 0..size {
            let idx = start + off;
            let state = self.map(space)[idx].state;
            match state {
                ByteState::Unallocated => {
                    return self.push_finding(Finding {
                        kind: FindingKind::OutOfBounds,
                        space,
                        addr: addr + off as u64,
                        size: 1,
                        step: self.step,
                        barrier_gen: self.barrier_gen,
                        actor,
                        other: None,
                        detail: "access to unallocated SoftGPU shadow byte".into(),
                    });
                }
                ByteState::Freed => {
                    return self.push_finding(Finding {
                        kind: FindingKind::UseAfterFree,
                        space,
                        addr: addr + off as u64,
                        size: 1,
                        step: self.step,
                        barrier_gen: self.barrier_gen,
                        actor,
                        other: None,
                        detail: "access to SoftGPU-freed shadow byte".into(),
                    });
                }
                ByteState::Uninitialized if !is_write => {
                    return self.push_finding(Finding {
                        kind: FindingKind::UninitializedRead,
                        space,
                        addr: addr + off as u64,
                        size: 1,
                        step: self.step,
                        barrier_gen: self.barrier_gen,
                        actor,
                        other: None,
                        detail: "read of uninitialized SoftGPU shadow byte".into(),
                    });
                }
                _ => {}
            }

            if let Some(prev) = self.map(space)[idx].last {
                if let Some(kind) =
                    race_kind(prev, actor, self.barrier_gen, is_write, is_atomic, space)
                {
                    let detail = match kind {
                        FindingKind::MissingBarrier => {
                            "group conflict in same SoftGPU barrier generation without barrier"
                                .into()
                        }
                        FindingKind::Race => {
                            "global conflict in same SoftGPU barrier generation (declared SoftGPU HB subset)".into()
                        }
                        FindingKind::CrossWorkgroupRace => {
                            "conflicting SoftGPU global accesses from different workgroups".into()
                        }
                        _ => "SoftGPU happens-before race in declared subset".into(),
                    };
                    return self.push_finding(Finding {
                        kind,
                        space,
                        addr: addr + off as u64,
                        size: 1,
                        step: self.step,
                        barrier_gen: self.barrier_gen,
                        actor,
                        other: Some(prev.actor),
                        detail,
                    });
                }
            }
        }

        let record = AccessRecord {
            actor,
            barrier_gen: self.barrier_gen,
            step: self.step,
            is_write,
            is_atomic,
        };
        for off in 0..size {
            let b = &mut self.map_mut(space)[start + off];
            if is_write {
                b.state = ByteState::Initialized;
            }
            b.last = Some(record);
        }
        Ok(())
    }

    fn push_finding(&mut self, finding: Finding) -> Result<()> {
        self.findings.push(finding.clone());
        let hard = matches!(
            finding.kind,
            FindingKind::OutOfBounds | FindingKind::UseAfterFree | FindingKind::ShadowLimit
        );
        if self.mode == SanitizeMode::FailFast || hard {
            return Err(FunctionalError::Sanitize(finding));
        }
        Ok(())
    }

    fn map(&self, space: AddrSpace) -> &[ByteShadow] {
        match space {
            AddrSpace::Global => &self.global,
            AddrSpace::Group => &self.group,
        }
    }

    fn map_mut(&mut self, space: AddrSpace) -> &mut [ByteShadow] {
        match space {
            AddrSpace::Global => &mut self.global,
            AddrSpace::Group => &mut self.group,
        }
    }
}

fn race_kind(
    prev: AccessRecord,
    cur: WorkItemId,
    cur_barrier_gen: u64,
    cur_write: bool,
    cur_atomic: bool,
    space: AddrSpace,
) -> Option<FindingKind> {
    // Same workitem: SoftGPU program order — not a race.
    if prev.actor.workgroup == cur.workgroup && prev.actor.flat_local == cur.flat_local {
        return None;
    }
    // Need a write involved.
    if !prev.is_write && !cur_write {
        return None;
    }
    // SoftGPU atomics on both sides: ordered by SoftGPU sequential interpreter.
    if prev.is_atomic && cur_atomic {
        return None;
    }
    if prev.actor.workgroup != cur.workgroup {
        return Some(FindingKind::CrossWorkgroupRace);
    }
    // Same WG: race if same SoftGPU barrier generation (no barrier between).
    if prev.barrier_gen == cur_barrier_gen {
        return Some(match space {
            AddrSpace::Group => FindingKind::MissingBarrier,
            AddrSpace::Global => FindingKind::Race,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_schema_is_stable() {
        assert_eq!(SANITIZER_REPLAY_SCHEMA, "softgpu-sanitizer-replay-v1");
    }
}
