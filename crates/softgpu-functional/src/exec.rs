//! Deterministic SoftGPU Functional IR interpreter (Phase 6–7).
//!
//! Phase 7 adds SoftGPU software waves/lanes, group memory, barrier segments,
//! structured divergence, and selected atomics. This is **not** gfx1201 ISA.

use crate::error::{FunctionalError, Result};
use crate::ir::{barrier_segments, AddrSpace, Op, Program, TypeId};
use crate::memory::{kernarg_load, GlobalArena};
use crate::sanitize::{SanitizeMode, SanitizeReport, Sanitizer, WorkItemId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Marker required on all functional-mode outputs.
pub const FUNCTIONAL_MODE_MARKER: &str = "softgpu_functional_cpu_not_gfx1201_isa";

/// SoftGPU software launch limits (not hardware).
pub const MAX_FLAT_WORKITEMS: u64 = 1_048_576;
pub const MAX_WORKGROUP_FLAT: u32 = 1_024;
pub const DEFAULT_STEP_BUDGET: u64 = 50_000_000;
pub const DEFAULT_WAVE_SIZE: u32 = 32;
pub const MAX_WAVE_SIZE: u32 = 128;
pub const MAX_GROUP_BYTES: u32 = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchedulePolicy {
    /// Phase 6 compatible: each workitem runs to completion in lex order.
    LexWorkitem,
    /// Waves in order; barrier-separated segments sync the workgroup.
    WaveBarrier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchConfig {
    pub grid: [u32; 3],
    pub workgroup: [u32; 3],
}

impl LaunchConfig {
    pub fn validate(&self) -> Result<()> {
        for d in 0..3 {
            if self.workgroup[d] == 0 {
                return Err(FunctionalError::Validation {
                    detail: format!("workgroup[{d}] must be > 0"),
                });
            }
            if self.grid[d] == 0 {
                return Err(FunctionalError::Validation {
                    detail: format!("grid[{d}] must be > 0"),
                });
            }
            if self.grid[d] % self.workgroup[d] != 0 {
                return Err(FunctionalError::Validation {
                    detail: format!(
                        "grid[{d}]={} not divisible by workgroup[{d}]={}",
                        self.grid[d], self.workgroup[d]
                    ),
                });
            }
        }
        let wg_flat = u64::from(self.workgroup[0])
            * u64::from(self.workgroup[1])
            * u64::from(self.workgroup[2]);
        if wg_flat > u64::from(MAX_WORKGROUP_FLAT) {
            return Err(FunctionalError::Validation {
                detail: format!("workgroup flat {wg_flat} > {MAX_WORKGROUP_FLAT}"),
            });
        }
        let flat = u64::from(self.grid[0]) * u64::from(self.grid[1]) * u64::from(self.grid[2]);
        if flat > MAX_FLAT_WORKITEMS {
            return Err(FunctionalError::Validation {
                detail: format!("flat workitems {flat} > {MAX_FLAT_WORKITEMS}"),
            });
        }
        Ok(())
    }

    pub fn num_workgroups(&self) -> [u32; 3] {
        [
            self.grid[0] / self.workgroup[0],
            self.grid[1] / self.workgroup[1],
            self.grid[2] / self.workgroup[2],
        ]
    }

    pub fn workgroup_flat(&self) -> u32 {
        self.workgroup[0] * self.workgroup[1] * self.workgroup[2]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecConfig {
    pub launch: LaunchConfig,
    pub wave_size: u32,
    pub group_bytes: u32,
    pub schedule: SchedulePolicy,
    pub step_budget: u64,
    pub sanitize: SanitizeMode,
}

impl ExecConfig {
    pub fn from_launch(launch: LaunchConfig) -> Self {
        Self {
            launch,
            wave_size: DEFAULT_WAVE_SIZE,
            group_bytes: 0,
            schedule: SchedulePolicy::LexWorkitem,
            step_budget: DEFAULT_STEP_BUDGET,
            sanitize: SanitizeMode::Off,
        }
    }

    pub fn validate(&self, program: &Program) -> Result<()> {
        self.launch.validate()?;
        if self.wave_size == 0 || self.wave_size > MAX_WAVE_SIZE {
            return Err(FunctionalError::Validation {
                detail: format!(
                    "wave_size {} out of range 1..={MAX_WAVE_SIZE}",
                    self.wave_size
                ),
            });
        }
        let need = self.group_bytes.max(program.group_bytes);
        if need > MAX_GROUP_BYTES {
            return Err(FunctionalError::Validation {
                detail: format!("group_bytes {need} > SoftGPU max {MAX_GROUP_BYTES}"),
            });
        }
        if program.has_barrier() && self.schedule != SchedulePolicy::WaveBarrier {
            return Err(FunctionalError::Validation {
                detail: "programs with barrier require schedule=wave_barrier".into(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RunReport {
    pub fidelity: &'static str,
    pub mode: &'static str,
    pub note: &'static str,
    pub kernel: String,
    pub workitems_executed: u64,
    pub steps: u64,
    pub grid: [u32; 3],
    pub workgroup: [u32; 3],
    pub wave_size: u32,
    pub schedule: SchedulePolicy,
    pub barrier_generations: u64,
    pub atomics_executed: u64,
}

/// Execute `program` under lex workitem schedule (Phase 6 API).
pub fn run(
    program: &Program,
    launch: LaunchConfig,
    arena: &mut GlobalArena,
    kernarg: &[u8],
) -> Result<RunReport> {
    let mut cfg = ExecConfig::from_launch(launch);
    cfg.group_bytes = program.group_bytes;
    if program.has_barrier() {
        cfg.schedule = SchedulePolicy::WaveBarrier;
    }
    Ok(run_with_config_sanitized(program, cfg, arena, kernarg)?.0)
}

pub fn run_with_budget(
    program: &Program,
    launch: LaunchConfig,
    arena: &mut GlobalArena,
    kernarg: &[u8],
    step_budget: u64,
) -> Result<RunReport> {
    let mut cfg = ExecConfig::from_launch(launch);
    cfg.group_bytes = program.group_bytes;
    cfg.step_budget = step_budget;
    if program.has_barrier() {
        cfg.schedule = SchedulePolicy::WaveBarrier;
    }
    Ok(run_with_config_sanitized(program, cfg, arena, kernarg)?.0)
}

pub fn run_with_config(
    program: &Program,
    cfg: ExecConfig,
    arena: &mut GlobalArena,
    kernarg: &[u8],
) -> Result<RunReport> {
    Ok(run_with_config_sanitized(program, cfg, arena, kernarg)?.0)
}

/// Execute with optional SoftGPU Phase 8 sanitizer instrumentation.
pub fn run_with_config_sanitized(
    program: &Program,
    cfg: ExecConfig,
    arena: &mut GlobalArena,
    kernarg: &[u8],
) -> Result<(RunReport, SanitizeReport)> {
    let group_len = cfg.group_bytes.max(program.group_bytes) as usize;
    let sanitizer = Sanitizer::new(cfg.sanitize, arena.len(), group_len)?;
    run_with_sanitizer(program, cfg, arena, kernarg, sanitizer)
}

/// Execute with a caller-owned sanitizer (for shadow setup such as SoftGPU free/uninit).
pub fn run_with_sanitizer(
    program: &Program,
    cfg: ExecConfig,
    arena: &mut GlobalArena,
    kernarg: &[u8],
    mut sanitizer: Sanitizer,
) -> Result<(RunReport, SanitizeReport)> {
    program.validate()?;
    cfg.validate(program)?;

    let mut steps = 0u64;
    let mut workitems = 0u64;
    let mut barrier_generations = 0u64;
    let mut atomics_executed = 0u64;
    let nwg = cfg.launch.num_workgroups();

    for gz in 0..nwg[2] {
        for gy in 0..nwg[1] {
            for gx in 0..nwg[0] {
                let group_len = cfg.group_bytes.max(program.group_bytes) as usize;
                let mut group = GlobalArena::new(group_len);
                sanitizer.reset_group();
                sanitizer.barrier_gen = 0;
                let (s, wi, bg, at) = match cfg.schedule {
                    SchedulePolicy::LexWorkitem => run_workgroup_lex(
                        program,
                        &cfg,
                        arena,
                        &mut group,
                        kernarg,
                        [gx, gy, gz],
                        cfg.step_budget.saturating_sub(steps),
                        &mut sanitizer,
                    )?,
                    SchedulePolicy::WaveBarrier => run_workgroup_wave_barrier(
                        program,
                        &cfg,
                        arena,
                        &mut group,
                        kernarg,
                        [gx, gy, gz],
                        cfg.step_budget.saturating_sub(steps),
                        &mut sanitizer,
                    )?,
                };
                steps = steps.saturating_add(s);
                if steps > cfg.step_budget {
                    return Err(FunctionalError::StepBudgetExceeded { steps });
                }
                workitems += wi;
                barrier_generations = barrier_generations.saturating_add(bg);
                atomics_executed = atomics_executed.saturating_add(at);
            }
        }
    }

    let report = RunReport {
        fidelity: "functional",
        mode: FUNCTIONAL_MODE_MARKER,
        note: "not_gfx1201_isa_emulation",
        kernel: program.name.clone(),
        workitems_executed: workitems,
        steps,
        grid: cfg.launch.grid,
        workgroup: cfg.launch.workgroup,
        wave_size: cfg.wave_size,
        schedule: cfg.schedule,
        barrier_generations,
        atomics_executed,
    };
    Ok((report, sanitizer.into_report()))
}

#[allow(clippy::too_many_arguments)]
fn run_workgroup_lex(
    program: &Program,
    cfg: &ExecConfig,
    global: &mut GlobalArena,
    group: &mut GlobalArena,
    kernarg: &[u8],
    wg: [u32; 3],
    budget: u64,
    sanitizer: &mut Sanitizer,
) -> Result<(u64, u64, u64, u64)> {
    let mut steps = 0u64;
    let mut atomics = 0u64;
    let mut workitems = 0u64;
    let launch = cfg.launch;
    for lz in 0..launch.workgroup[2] {
        for ly in 0..launch.workgroup[1] {
            for lx in 0..launch.workgroup[0] {
                let local = [lx, ly, lz];
                let flat = flat_local(local, launch.workgroup);
                let global_id = [
                    wg[0] * launch.workgroup[0] + lx,
                    wg[1] * launch.workgroup[1] + ly,
                    wg[2] * launch.workgroup[2] + lz,
                ];
                let mut lane = Lane::new(
                    global_id,
                    local,
                    wg,
                    flat / cfg.wave_size,
                    flat % cfg.wave_size,
                    cfg.wave_size,
                );
                let (used, at) = exec_ops(
                    &program.body,
                    std::slice::from_mut(&mut lane),
                    &[true],
                    global,
                    group,
                    kernarg,
                    budget.saturating_sub(steps),
                    true,
                    launch.workgroup,
                    sanitizer,
                )?;
                steps = steps.saturating_add(used);
                atomics = atomics.saturating_add(at);
                if steps > budget {
                    return Err(FunctionalError::StepBudgetExceeded { steps });
                }
                workitems += 1;
            }
        }
    }
    Ok((steps, workitems, 0, atomics))
}

#[allow(clippy::too_many_arguments)]
fn run_workgroup_wave_barrier(
    program: &Program,
    cfg: &ExecConfig,
    global: &mut GlobalArena,
    group: &mut GlobalArena,
    kernarg: &[u8],
    wg: [u32; 3],
    budget: u64,
    sanitizer: &mut Sanitizer,
) -> Result<(u64, u64, u64, u64)> {
    let launch = cfg.launch;
    let flat_n = launch.workgroup_flat();
    let mut lanes: Vec<Lane> = Vec::with_capacity(flat_n as usize);
    for flat in 0..flat_n {
        let local = unflat_local(flat, launch.workgroup);
        let global_id = [
            wg[0] * launch.workgroup[0] + local[0],
            wg[1] * launch.workgroup[1] + local[1],
            wg[2] * launch.workgroup[2] + local[2],
        ];
        lanes.push(Lane::new(
            global_id,
            local,
            wg,
            flat / cfg.wave_size,
            flat % cfg.wave_size,
            cfg.wave_size,
        ));
    }

    let segments = barrier_segments(&program.body)?;
    let barrier_gens = segments.len().saturating_sub(1) as u64;
    let mut steps = 0u64;
    let mut atomics = 0u64;
    let n_waves = flat_n.div_ceil(cfg.wave_size);

    for (si, seg) in segments.iter().enumerate() {
        for wave in 0..n_waves {
            let lo = (wave * cfg.wave_size) as usize;
            let hi = ((wave * cfg.wave_size + cfg.wave_size).min(flat_n)) as usize;
            let mask: Vec<bool> = (lo..hi).map(|_| true).collect();
            let (used, at) = exec_ops(
                seg,
                &mut lanes[lo..hi],
                &mask,
                global,
                group,
                kernarg,
                budget.saturating_sub(steps),
                false,
                launch.workgroup,
                sanitizer,
            )?;
            steps = steps.saturating_add(used);
            atomics = atomics.saturating_add(at);
            if steps > budget {
                return Err(FunctionalError::StepBudgetExceeded { steps });
            }
        }
        if si + 1 < segments.len() {
            sanitizer.note_barrier();
        }
    }

    Ok((steps, u64::from(flat_n), barrier_gens, atomics))
}

fn flat_local(local: [u32; 3], wg: [u32; 3]) -> u32 {
    local[0] + local[1] * wg[0] + local[2] * wg[0] * wg[1]
}

fn unflat_local(flat: u32, wg: [u32; 3]) -> [u32; 3] {
    let x = flat % wg[0];
    let t = flat / wg[0];
    let y = t % wg[1];
    let z = t / wg[1];
    [x, y, z]
}

#[derive(Debug, Clone)]
struct Lane {
    global: [u32; 3],
    local: [u32; 3],
    wg: [u32; 3],
    wave_id: u32,
    lane_id: u32,
    wave_size: u32,
    regs: BTreeMap<String, i64>,
}

impl Lane {
    fn new(
        global: [u32; 3],
        local: [u32; 3],
        wg: [u32; 3],
        wave_id: u32,
        lane_id: u32,
        wave_size: u32,
    ) -> Self {
        Self {
            global,
            local,
            wg,
            wave_id,
            lane_id,
            wave_size,
            regs: BTreeMap::new(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn exec_ops(
    ops: &[Op],
    lanes: &mut [Lane],
    mask: &[bool],
    global: &mut GlobalArena,
    group: &mut GlobalArena,
    kernarg: &[u8],
    budget: u64,
    stop_at_ret: bool,
    workgroup: [u32; 3],
    sanitizer: &mut Sanitizer,
) -> Result<(u64, u64)> {
    let mut steps = 0u64;
    let mut atomics = 0u64;

    for op in ops {
        if matches!(op, Op::Ret) {
            if stop_at_ret {
                return Ok((steps, atomics));
            }
            continue;
        }
        if matches!(op, Op::Barrier) {
            return Err(FunctionalError::Internal(
                "barrier must be handled by segment splitter".into(),
            ));
        }

        match op {
            Op::If {
                cond,
                then_body,
                else_body,
            } => {
                let mut then_mask = vec![false; lanes.len()];
                let mut else_mask = vec![false; lanes.len()];
                for (i, lane) in lanes.iter().enumerate() {
                    if !mask[i] {
                        continue;
                    }
                    let c = get_reg(&lane.regs, cond)?;
                    if c != 0 {
                        then_mask[i] = true;
                    } else {
                        else_mask[i] = true;
                    }
                }
                let (s1, a1) = exec_ops(
                    then_body,
                    lanes,
                    &then_mask,
                    global,
                    group,
                    kernarg,
                    budget.saturating_sub(steps),
                    false,
                    workgroup,
                    sanitizer,
                )?;
                steps = steps.saturating_add(s1);
                atomics = atomics.saturating_add(a1);
                let (s2, a2) = exec_ops(
                    else_body,
                    lanes,
                    &else_mask,
                    global,
                    group,
                    kernarg,
                    budget.saturating_sub(steps),
                    false,
                    workgroup,
                    sanitizer,
                )?;
                steps = steps.saturating_add(s2);
                atomics = atomics.saturating_add(a2);
            }
            Op::While { cond, body } => loop {
                let mut iter_mask = vec![false; lanes.len()];
                let mut any = false;
                for (i, lane) in lanes.iter().enumerate() {
                    if !mask[i] {
                        continue;
                    }
                    let c = get_reg(&lane.regs, cond)?;
                    if c != 0 {
                        iter_mask[i] = true;
                        any = true;
                    }
                }
                if !any {
                    break;
                }
                let (s, a) = exec_ops(
                    body,
                    lanes,
                    &iter_mask,
                    global,
                    group,
                    kernarg,
                    budget.saturating_sub(steps),
                    false,
                    workgroup,
                    sanitizer,
                )?;
                steps = steps.saturating_add(s);
                atomics = atomics.saturating_add(a);
                if steps > budget {
                    return Err(FunctionalError::StepBudgetExceeded { steps });
                }
            },
            other => {
                for (i, lane) in lanes.iter_mut().enumerate() {
                    if !mask[i] {
                        continue;
                    }
                    steps += 1;
                    if steps > budget {
                        return Err(FunctionalError::StepBudgetExceeded { steps });
                    }
                    sanitizer.step = sanitizer.step.saturating_add(1);
                    let at =
                        exec_lane_op(other, lane, global, group, kernarg, workgroup, sanitizer)?;
                    atomics = atomics.saturating_add(at);
                }
            }
        }
        if steps > budget {
            return Err(FunctionalError::StepBudgetExceeded { steps });
        }
    }
    Ok((steps, atomics))
}

fn actor_of(lane: &Lane, workgroup: [u32; 3]) -> WorkItemId {
    WorkItemId {
        workgroup: lane.wg,
        wave: lane.wave_id,
        lane: lane.lane_id,
        flat_local: flat_local(lane.local, workgroup),
    }
}

#[allow(clippy::too_many_arguments)]
fn exec_lane_op(
    op: &Op,
    lane: &mut Lane,
    global: &mut GlobalArena,
    group: &mut GlobalArena,
    kernarg: &[u8],
    workgroup: [u32; 3],
    sanitizer: &mut Sanitizer,
) -> Result<u64> {
    let actor = actor_of(lane, workgroup);
    match op {
        Op::Const { dst, ty, value } => {
            lane.regs.insert(dst.clone(), narrow(*value, *ty)?);
            Ok(0)
        }
        Op::GlobalId { dst, dim } => {
            lane.regs
                .insert(dst.clone(), i64::from(lane.global[*dim as usize]));
            Ok(0)
        }
        Op::LocalId { dst, dim } => {
            lane.regs
                .insert(dst.clone(), i64::from(lane.local[*dim as usize]));
            Ok(0)
        }
        Op::WorkgroupId { dst, dim } => {
            lane.regs
                .insert(dst.clone(), i64::from(lane.wg[*dim as usize]));
            Ok(0)
        }
        Op::LaneId { dst } => {
            lane.regs.insert(dst.clone(), i64::from(lane.lane_id));
            Ok(0)
        }
        Op::WaveId { dst } => {
            lane.regs.insert(dst.clone(), i64::from(lane.wave_id));
            Ok(0)
        }
        Op::WaveSize { dst } => {
            lane.regs.insert(dst.clone(), i64::from(lane.wave_size));
            Ok(0)
        }
        Op::Add { dst, lhs, rhs, ty } => {
            let a = get_reg(&lane.regs, lhs)?;
            let b = get_reg(&lane.regs, rhs)?;
            lane.regs
                .insert(dst.clone(), narrow(a.wrapping_add(b), *ty)?);
            Ok(0)
        }
        Op::Sub { dst, lhs, rhs, ty } => {
            let a = get_reg(&lane.regs, lhs)?;
            let b = get_reg(&lane.regs, rhs)?;
            lane.regs
                .insert(dst.clone(), narrow(a.wrapping_sub(b), *ty)?);
            Ok(0)
        }
        Op::Mul { dst, lhs, rhs, ty } => {
            let a = get_reg(&lane.regs, lhs)?;
            let b = get_reg(&lane.regs, rhs)?;
            lane.regs
                .insert(dst.clone(), narrow(a.wrapping_mul(b), *ty)?);
            Ok(0)
        }
        Op::CmpEq { dst, lhs, rhs, ty } => {
            let a = narrow(get_reg(&lane.regs, lhs)?, *ty)?;
            let b = narrow(get_reg(&lane.regs, rhs)?, *ty)?;
            lane.regs.insert(dst.clone(), i64::from(a == b));
            Ok(0)
        }
        Op::CmpNe { dst, lhs, rhs, ty } => {
            let a = narrow(get_reg(&lane.regs, lhs)?, *ty)?;
            let b = narrow(get_reg(&lane.regs, rhs)?, *ty)?;
            lane.regs.insert(dst.clone(), i64::from(a != b));
            Ok(0)
        }
        Op::And { dst, lhs, rhs, ty } => {
            let a = get_reg(&lane.regs, lhs)?;
            let b = get_reg(&lane.regs, rhs)?;
            let v = match ty {
                TypeId::I32 => i64::from((a as i32) & (b as i32)),
                TypeId::U32 => i64::from((a as u32) & (b as u32)),
                TypeId::U64 => (a as u64 & b as u64) as i64,
            };
            lane.regs.insert(dst.clone(), v);
            Ok(0)
        }
        Op::KernargLoad { dst, offset, ty } => {
            lane.regs
                .insert(dst.clone(), kernarg_load(kernarg, *offset, *ty)?);
            Ok(0)
        }
        Op::LoadGlobal { dst, addr, ty } => {
            let a = get_reg(&lane.regs, addr)? as u64;
            sanitizer.on_access(AddrSpace::Global, a, *ty, false, false, actor)?;
            lane.regs.insert(dst.clone(), global.load(a, *ty)?);
            Ok(0)
        }
        Op::StoreGlobal { addr, src, ty } => {
            let a = get_reg(&lane.regs, addr)? as u64;
            let v = get_reg(&lane.regs, src)?;
            sanitizer.on_access(AddrSpace::Global, a, *ty, true, false, actor)?;
            global.store(a, *ty, v)?;
            Ok(0)
        }
        Op::LoadGroup { dst, addr, ty } => {
            let a = get_reg(&lane.regs, addr)? as u64;
            sanitizer.on_access(AddrSpace::Group, a, *ty, false, false, actor)?;
            lane.regs.insert(dst.clone(), group.load(a, *ty)?);
            Ok(0)
        }
        Op::StoreGroup { addr, src, ty } => {
            let a = get_reg(&lane.regs, addr)? as u64;
            let v = get_reg(&lane.regs, src)?;
            sanitizer.on_access(AddrSpace::Group, a, *ty, true, false, actor)?;
            group.store(a, *ty, v)?;
            Ok(0)
        }
        Op::AtomicAdd {
            dst,
            addr,
            src,
            space,
            scope: _,
            order: _,
        } => {
            let a = get_reg(&lane.regs, addr)? as u64;
            let v = get_reg(&lane.regs, src)?;
            sanitizer.on_access(*space, a, TypeId::I32, true, true, actor)?;
            let arena = match space {
                AddrSpace::Global => &mut *global,
                AddrSpace::Group => &mut *group,
            };
            let old = arena.load(a, TypeId::I32)?;
            let newv = i64::from((old as i32).wrapping_add(v as i32));
            arena.store(a, TypeId::I32, newv)?;
            lane.regs.insert(dst.clone(), old);
            Ok(1)
        }
        Op::Barrier | Op::If { .. } | Op::While { .. } | Op::Ret => Err(FunctionalError::Internal(
            format!("unexpected op in exec_lane_op: {op:?}"),
        )),
    }
}

fn get_reg(regs: &BTreeMap<String, i64>, name: &str) -> Result<i64> {
    regs.get(name)
        .copied()
        .ok_or_else(|| FunctionalError::UndefinedReg {
            name: name.to_string(),
        })
}

fn narrow(v: i64, ty: TypeId) -> Result<i64> {
    Ok(match ty {
        TypeId::I32 => v as i32 as i64,
        TypeId::U32 => (v as u32) as i64,
        TypeId::U64 => v,
    })
}
