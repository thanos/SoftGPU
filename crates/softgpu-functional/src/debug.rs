//! SoftGPU Phase 9 debugger and deterministic exploration.
//!
//! Declared subset (honest):
//! - Versioned JSONL traces (`softgpu-debug-trace-v1`) with a hard event budget
//! - Breakpoints on SoftGPU global step and/or memory-access ops
//! - Wave/lane/register snapshots at stop; global memory peek by address
//! - Seeded schedule exploration (`schedule_seed` + `wave_size` candidates)
//! - Trace minimization to the earliest failing SoftGPU step
//! - Source mapping: **only** `Program.source_provenance` — never invents lines/files
//!
//! Hostile trace input is rejected with validation/parse errors (no panic on junk).

use crate::error::{FunctionalError, Result};
use crate::exec::{run_with_config_sanitized, ExecConfig, LaunchConfig, RunReport, SchedulePolicy};
use crate::ir::{AddrSpace, Op, Program};
use crate::memory::GlobalArena;
use crate::sanitize::{
    Finding, FindingKind, ReplayBundle, SanitizeMode, SanitizeReport, WorkItemId,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DEBUG_TRACE_SCHEMA: &str = "softgpu-debug-trace-v1";
pub const DEFAULT_TRACE_EVENT_BUDGET: usize = 64 * 1024;
pub const MAX_TRACE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_TRACE_LINE_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugAction {
    Continue,
    Break,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Breakpoint {
    /// Stop after SoftGPU global step `step` completes (1-based sanitizer/debug step).
    AfterStep { step: u64 },
    /// Stop after a global or group load/store/atomic.
    OnMemoryAccess,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceEvent {
    Header {
        schema: String,
        program_name: String,
        source_provenance: String,
        grid: [u32; 3],
        workgroup: [u32; 3],
        wave_size: u32,
        schedule: SchedulePolicy,
        schedule_seed: u64,
        note: String,
    },
    Step {
        step: u64,
        barrier_gen: u64,
        actor: WorkItemId,
        op: String,
    },
    MemoryAccess {
        step: u64,
        space: AddrSpace,
        addr: u64,
        is_write: bool,
        is_atomic: bool,
        actor: WorkItemId,
    },
    Barrier {
        barrier_gen: u64,
    },
    Break {
        step: u64,
        reason: String,
    },
    SanitizeFinding {
        finding: Finding,
    },
    Done {
        steps: u64,
        workitems: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub step: u64,
    pub barrier_gen: u64,
    pub actor: WorkItemId,
    pub op: String,
    pub registers: BTreeMap<String, i64>,
    /// SoftGPU never invents source locations; this is the program provenance string only.
    pub source_provenance: String,
    pub source_mapping: Option<()>,
}

impl StateSnapshot {
    pub fn from_lane(
        step: u64,
        barrier_gen: u64,
        actor: WorkItemId,
        op: &str,
        regs: &BTreeMap<String, i64>,
        program: &Program,
    ) -> Self {
        Self {
            step,
            barrier_gen,
            actor,
            op: op.to_string(),
            registers: regs.clone(),
            source_provenance: program.source_provenance.clone(),
            // Explicitly absent: SoftGPU has no verified SFIR→source line map.
            source_mapping: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugStop {
    pub reason: String,
    pub snapshot: StateSnapshot,
}

#[derive(Debug, Default)]
pub struct TraceLog {
    events: Vec<TraceEvent>,
    budget: usize,
    exhausted: bool,
}

impl TraceLog {
    pub fn new(budget: usize) -> Self {
        Self {
            events: Vec::new(),
            budget: budget.max(1),
            exhausted: false,
        }
    }

    pub fn push(&mut self, ev: TraceEvent) -> Result<()> {
        if self.events.len() >= self.budget {
            self.exhausted = true;
            return Err(FunctionalError::Validation {
                detail: format!(
                    "debug trace event budget exhausted (budget={})",
                    self.budget
                ),
            });
        }
        self.events.push(ev);
        Ok(())
    }

    pub fn events(&self) -> &[TraceEvent] {
        &self.events
    }

    pub fn exhausted(&self) -> bool {
        self.exhausted
    }

    pub fn to_jsonl(&self) -> Result<String> {
        let mut out = String::new();
        for ev in &self.events {
            let line =
                serde_json::to_string(ev).map_err(|e| FunctionalError::Internal(e.to_string()))?;
            out.push_str(&line);
            out.push('\n');
        }
        Ok(out)
    }
}

/// Hostile-input JSONL reader for SoftGPU debug traces.
pub fn parse_trace_jsonl(input: &str) -> Result<Vec<TraceEvent>> {
    if input.len() > MAX_TRACE_BYTES {
        return Err(FunctionalError::Validation {
            detail: format!(
                "trace input {} bytes exceeds SoftGPU max {MAX_TRACE_BYTES}",
                input.len()
            ),
        });
    }
    let mut events = Vec::new();
    for (i, line) in input.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.len() > MAX_TRACE_LINE_BYTES {
            return Err(FunctionalError::Validation {
                detail: format!(
                    "trace line {} length {} exceeds SoftGPU max {MAX_TRACE_LINE_BYTES}",
                    i + 1,
                    line.len()
                ),
            });
        }
        let ev: TraceEvent = serde_json::from_str(line)
            .map_err(|e| FunctionalError::Parse(format!("trace line {}: {e}", i + 1)))?;
        if events.len() >= DEFAULT_TRACE_EVENT_BUDGET {
            return Err(FunctionalError::Validation {
                detail: format!(
                    "trace has more than {DEFAULT_TRACE_EVENT_BUDGET} events (SoftGPU limit)"
                ),
            });
        }
        events.push(ev);
    }
    if let Some(TraceEvent::Header { schema, .. }) = events.first() {
        if schema != DEBUG_TRACE_SCHEMA {
            return Err(FunctionalError::Validation {
                detail: format!("trace schema '{schema}' != '{DEBUG_TRACE_SCHEMA}'"),
            });
        }
    } else if !events.is_empty() {
        return Err(FunctionalError::Validation {
            detail: "debug trace must begin with a header event".into(),
        });
    }
    Ok(events)
}

fn op_label(op: &Op) -> String {
    match op {
        Op::Const { .. } => "const".into(),
        Op::GlobalId { .. } => "global_id".into(),
        Op::LocalId { .. } => "local_id".into(),
        Op::WorkgroupId { .. } => "workgroup_id".into(),
        Op::LaneId { .. } => "lane_id".into(),
        Op::WaveId { .. } => "wave_id".into(),
        Op::WaveSize { .. } => "wave_size".into(),
        Op::Add { .. } => "add".into(),
        Op::Sub { .. } => "sub".into(),
        Op::Mul { .. } => "mul".into(),
        Op::CmpEq { .. } => "cmp_eq".into(),
        Op::CmpNe { .. } => "cmp_ne".into(),
        Op::And { .. } => "and".into(),
        Op::KernargLoad { .. } => "kernarg_load".into(),
        Op::LoadGlobal { .. } => "load_global".into(),
        Op::StoreGlobal { .. } => "store_global".into(),
        Op::LoadGroup { .. } => "load_group".into(),
        Op::StoreGroup { .. } => "store_group".into(),
        Op::AtomicAdd { .. } => "atomic_add".into(),
        Op::Barrier => "barrier".into(),
        Op::If { .. } => "if".into(),
        Op::While { .. } => "while".into(),
        Op::Ret => "ret".into(),
    }
}

fn is_memory_op(op: &Op) -> bool {
    matches!(
        op,
        Op::LoadGlobal { .. }
            | Op::StoreGlobal { .. }
            | Op::LoadGroup { .. }
            | Op::StoreGroup { .. }
            | Op::AtomicAdd { .. }
    )
}

/// Observer invoked from the SoftGPU interpreter (Phase 9).
pub trait ExecObserver {
    fn on_lane_step(
        &mut self,
        step: u64,
        barrier_gen: u64,
        actor: WorkItemId,
        op: &Op,
        regs: &BTreeMap<String, i64>,
    ) -> Result<DebugAction>;

    fn on_barrier(&mut self, barrier_gen: u64) -> Result<()> {
        let _ = barrier_gen;
        Ok(())
    }
}

#[derive(Debug)]
pub struct NoopObserver;

impl ExecObserver for NoopObserver {
    fn on_lane_step(
        &mut self,
        _step: u64,
        _barrier_gen: u64,
        _actor: WorkItemId,
        _op: &Op,
        _regs: &BTreeMap<String, i64>,
    ) -> Result<DebugAction> {
        Ok(DebugAction::Continue)
    }
}

pub struct DebugSession<'a> {
    program: &'a Program,
    breakpoints: Vec<Breakpoint>,
    pub trace: TraceLog,
    stop: Option<DebugStop>,
    single_step: bool,
}

impl<'a> DebugSession<'a> {
    pub fn new(program: &'a Program, breakpoints: Vec<Breakpoint>, trace_budget: usize) -> Self {
        Self {
            program,
            breakpoints,
            trace: TraceLog::new(trace_budget),
            stop: None,
            single_step: false,
        }
    }

    pub fn with_single_step(mut self) -> Self {
        self.single_step = true;
        self
    }

    pub fn take_stop(&mut self) -> Option<DebugStop> {
        self.stop.take()
    }

    fn should_break(&self, step: u64, op: &Op) -> Option<&'static str> {
        if self.single_step {
            return Some("single_step");
        }
        for bp in &self.breakpoints {
            match bp {
                Breakpoint::AfterStep { step: s } if *s == step => return Some("after_step"),
                Breakpoint::OnMemoryAccess if is_memory_op(op) => return Some("memory_access"),
                _ => {}
            }
        }
        None
    }
}

impl ExecObserver for DebugSession<'_> {
    fn on_lane_step(
        &mut self,
        step: u64,
        barrier_gen: u64,
        actor: WorkItemId,
        op: &Op,
        regs: &BTreeMap<String, i64>,
    ) -> Result<DebugAction> {
        let label = op_label(op);
        self.trace.push(TraceEvent::Step {
            step,
            barrier_gen,
            actor,
            op: label.clone(),
        })?;
        if is_memory_op(op) {
            let (space, is_write, is_atomic) = match op {
                Op::LoadGlobal { .. } => (AddrSpace::Global, false, false),
                Op::StoreGlobal { .. } => (AddrSpace::Global, true, false),
                Op::LoadGroup { .. } => (AddrSpace::Group, false, false),
                Op::StoreGroup { .. } => (AddrSpace::Group, true, false),
                Op::AtomicAdd { space, .. } => (*space, true, true),
                _ => unreachable!(),
            };
            // Address is not re-derived here; record actor/step for replay correlation.
            self.trace.push(TraceEvent::MemoryAccess {
                step,
                space,
                addr: 0,
                is_write,
                is_atomic,
                actor,
            })?;
        }
        if let Some(reason) = self.should_break(step, op) {
            let snap =
                StateSnapshot::from_lane(step, barrier_gen, actor, &label, regs, self.program);
            self.trace.push(TraceEvent::Break {
                step,
                reason: reason.into(),
            })?;
            self.stop = Some(DebugStop {
                reason: reason.into(),
                snapshot: snap,
            });
            return Ok(DebugAction::Break);
        }
        Ok(DebugAction::Continue)
    }

    fn on_barrier(&mut self, barrier_gen: u64) -> Result<()> {
        self.trace.push(TraceEvent::Barrier { barrier_gen })
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DebugRunReport {
    pub fidelity: &'static str,
    pub mode: &'static str,
    pub note: &'static str,
    pub run: Option<RunReport>,
    pub sanitize: SanitizeReport,
    pub stop: Option<DebugStop>,
    pub trace_events: usize,
}

/// Execute under SoftGPU debugger controls.
pub fn run_debug(
    program: &Program,
    cfg: ExecConfig,
    arena: &mut GlobalArena,
    kernarg: &[u8],
    breakpoints: Vec<Breakpoint>,
    single_step: bool,
    trace_budget: usize,
) -> Result<(DebugRunReport, TraceLog)> {
    let mut session = DebugSession::new(program, breakpoints, trace_budget);
    if single_step {
        session = session.with_single_step();
    }
    session.trace.push(TraceEvent::Header {
        schema: DEBUG_TRACE_SCHEMA.into(),
        program_name: program.name.clone(),
        source_provenance: program.source_provenance.clone(),
        grid: cfg.launch.grid,
        workgroup: cfg.launch.workgroup,
        wave_size: cfg.wave_size,
        schedule: cfg.schedule,
        schedule_seed: cfg.schedule_seed,
        note: "not_gfx1201_isa_emulation".into(),
    })?;

    let outcome = crate::exec::run_with_observer(program, cfg, arena, kernarg, &mut session);
    let stop = session.take_stop();
    match outcome {
        Ok((run, sanitize)) => {
            for f in &sanitize.findings {
                let _ = session
                    .trace
                    .push(TraceEvent::SanitizeFinding { finding: f.clone() });
            }
            session.trace.push(TraceEvent::Done {
                steps: run.steps,
                workitems: run.workitems_executed,
            })?;
            let n = session.trace.events().len();
            Ok((
                DebugRunReport {
                    fidelity: "functional",
                    mode: "softgpu_functional_debug_v1",
                    note: "not_gfx1201_isa_emulation",
                    run: Some(run),
                    sanitize,
                    stop,
                    trace_events: n,
                },
                session.trace,
            ))
        }
        Err(FunctionalError::DebugBreak) => {
            let n = session.trace.events().len();
            Ok((
                DebugRunReport {
                    fidelity: "functional",
                    mode: "softgpu_functional_debug_v1",
                    note: "not_gfx1201_isa_emulation",
                    run: None,
                    sanitize: SanitizeReport::clean(),
                    stop,
                    trace_events: n,
                },
                session.trace,
            ))
        }
        Err(FunctionalError::Sanitize(finding)) => {
            let _ = session.trace.push(TraceEvent::SanitizeFinding {
                finding: finding.clone(),
            });
            let snap = StateSnapshot {
                step: finding.step,
                barrier_gen: finding.barrier_gen,
                actor: finding.actor,
                op: format!("{:?}", finding.kind),
                registers: BTreeMap::new(),
                source_provenance: program.source_provenance.clone(),
                source_mapping: None,
            };
            let _ = session.trace.push(TraceEvent::Break {
                step: finding.step,
                reason: "sanitize_finding".into(),
            });
            let n = session.trace.events().len();
            Ok((
                DebugRunReport {
                    fidelity: "sanitized",
                    mode: "softgpu_functional_debug_v1",
                    note: "not_gfx1201_isa_emulation",
                    run: None,
                    sanitize: SanitizeReport {
                        fidelity: "sanitized",
                        mode: "softgpu_functional_sanitizer_v1",
                        note: "not_gfx1201_isa_emulation",
                        findings: vec![finding],
                    },
                    stop: Some(DebugStop {
                        reason: "sanitize_finding".into(),
                        snapshot: snap,
                    }),
                    trace_events: n,
                },
                session.trace,
            ))
        }
        Err(e) => Err(e),
    }
}

/// Replay a sanitizer finding: run FailFast and capture a debug stop at the finding step.
pub fn inspect_sanitizer_finding(
    program: &Program,
    mut cfg: ExecConfig,
    arena: &mut GlobalArena,
    kernarg: &[u8],
    finding: &Finding,
) -> Result<(DebugRunReport, TraceLog, ReplayBundle)> {
    cfg.sanitize = SanitizeMode::FailFast;
    let bps = vec![Breakpoint::AfterStep {
        step: finding.step.max(1),
    }];
    let (report, trace) = run_debug(
        program,
        cfg,
        arena,
        kernarg,
        bps,
        false,
        DEFAULT_TRACE_EVENT_BUDGET,
    )?;
    let bundle = ReplayBundle::from_finding(program, &cfg, finding.clone());
    Ok((report, trace, bundle))
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ExploreTrial {
    pub wave_size: u32,
    pub schedule_seed: u64,
    pub workgroup_x: u32,
    pub found: bool,
    pub kind: Option<FindingKind>,
    pub step: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ExploreReport {
    pub fidelity: &'static str,
    pub mode: &'static str,
    pub note: &'static str,
    pub seed: u64,
    pub trials: Vec<ExploreTrial>,
    pub minimized: Option<ExploreTrial>,
}

/// Bounded seeded search for a SoftGPU ordering/race defect, then minimize WG size.
pub fn explore_and_minimize_race(
    program: &Program,
    base: ExecConfig,
    kernarg: &[u8],
    arena_len: usize,
    seed: u64,
    max_trials: u32,
) -> Result<ExploreReport> {
    let mut trials = Vec::new();
    let mut hit: Option<ExploreTrial> = None;
    let wave_candidates = [
        base.wave_size,
        ((seed % 31) as u32 + 1).min(base.launch.workgroup[0].max(1)),
        1,
        2,
        4,
        8,
        base.launch.workgroup[0].max(1),
    ];
    let mut attempt = 0u32;
    for &ws in &wave_candidates {
        if attempt >= max_trials {
            break;
        }
        for bit in 0..2u64 {
            if attempt >= max_trials {
                break;
            }
            attempt += 1;
            let mut cfg = base;
            cfg.wave_size = ws.max(1);
            cfg.schedule_seed = seed ^ (bit << 1) ^ u64::from(ws);
            cfg.sanitize = SanitizeMode::FailFast;
            let mut arena = GlobalArena::new(arena_len);
            let found = match run_with_config_sanitized(program, cfg, &mut arena, kernarg) {
                Err(FunctionalError::Sanitize(f))
                    if matches!(
                        f.kind,
                        FindingKind::Race
                            | FindingKind::MissingBarrier
                            | FindingKind::CrossWorkgroupRace
                    ) =>
                {
                    Some(ExploreTrial {
                        wave_size: cfg.wave_size,
                        schedule_seed: cfg.schedule_seed,
                        workgroup_x: cfg.launch.workgroup[0],
                        found: true,
                        kind: Some(f.kind),
                        step: Some(f.step),
                    })
                }
                Ok(_) | Err(FunctionalError::Sanitize(_)) => Some(ExploreTrial {
                    wave_size: cfg.wave_size,
                    schedule_seed: cfg.schedule_seed,
                    workgroup_x: cfg.launch.workgroup[0],
                    found: false,
                    kind: None,
                    step: None,
                }),
                Err(e) => return Err(e),
            };
            if let Some(t) = found {
                if t.found && hit.is_none() {
                    hit = Some(t.clone());
                }
                trials.push(t);
            }
        }
    }

    let minimized = if let Some(h) = hit.clone() {
        let mut best = h.clone();
        let mut wg = h.workgroup_x;
        while wg > 1 {
            let next = wg / 2;
            let mut cfg = base;
            cfg.wave_size = best.wave_size.min(next).max(1);
            cfg.schedule_seed = best.schedule_seed;
            cfg.sanitize = SanitizeMode::FailFast;
            cfg.launch = LaunchConfig {
                grid: [next, 1, 1],
                workgroup: [next, 1, 1],
            };
            let mut arena = GlobalArena::new(arena_len);
            match run_with_config_sanitized(program, cfg, &mut arena, kernarg) {
                Err(FunctionalError::Sanitize(f))
                    if matches!(
                        f.kind,
                        FindingKind::Race
                            | FindingKind::MissingBarrier
                            | FindingKind::CrossWorkgroupRace
                    ) =>
                {
                    best = ExploreTrial {
                        wave_size: cfg.wave_size,
                        schedule_seed: cfg.schedule_seed,
                        workgroup_x: next,
                        found: true,
                        kind: Some(f.kind),
                        step: Some(f.step),
                    };
                    wg = next;
                }
                _ => break,
            }
        }
        // Also minimize to earliest failing step via AfterStep binary search when possible.
        if let Some(step) = best.step {
            let mut lo = 1u64;
            let mut hi = step;
            let mut earliest = step;
            while lo < hi {
                let mid = (lo + hi) / 2;
                let mut cfg = base;
                cfg.wave_size = best.wave_size;
                cfg.schedule_seed = best.schedule_seed;
                cfg.sanitize = SanitizeMode::FailFast;
                cfg.launch = LaunchConfig {
                    grid: [best.workgroup_x, 1, 1],
                    workgroup: [best.workgroup_x, 1, 1],
                };
                cfg.step_budget = mid;
                let mut arena = GlobalArena::new(arena_len);
                match run_with_config_sanitized(program, cfg, &mut arena, kernarg) {
                    Err(FunctionalError::Sanitize(f)) => {
                        earliest = f.step.min(earliest);
                        hi = mid;
                    }
                    Err(FunctionalError::StepBudgetExceeded { .. }) => {
                        lo = mid + 1;
                    }
                    _ => {
                        lo = mid + 1;
                    }
                }
            }
            best.step = Some(earliest);
        }
        Some(best)
    } else {
        None
    };

    Ok(ExploreReport {
        fidelity: "functional",
        mode: "softgpu_schedule_explore_v1",
        note: "not_gfx1201_isa_emulation",
        seed,
        trials,
        minimized,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_constant_stable() {
        assert_eq!(DEBUG_TRACE_SCHEMA, "softgpu-debug-trace-v1");
    }

    #[test]
    fn snapshot_never_invents_source_mapping() {
        let p = Program {
            schema: crate::ir::SFIR_SCHEMA.into(),
            fidelity: "functional".into(),
            note: "not_gfx1201_isa_emulation".into(),
            name: "t".into(),
            source_provenance: "SoftGPU-authored".into(),
            kernarg_layout: vec![],
            group_bytes: 0,
            body: vec![Op::Ret],
        };
        let snap = StateSnapshot::from_lane(
            1,
            0,
            WorkItemId {
                workgroup: [0, 0, 0],
                wave: 0,
                lane: 0,
                flat_local: 0,
            },
            "const",
            &BTreeMap::new(),
            &p,
        );
        assert!(snap.source_mapping.is_none());
        assert_eq!(snap.source_provenance, "SoftGPU-authored");
    }
}
