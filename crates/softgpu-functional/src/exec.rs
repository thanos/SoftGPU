//! Deterministic SoftGPU Functional IR interpreter.

use crate::error::{FunctionalError, Result};
use crate::ir::{Op, Program, TypeId};
use crate::memory::{kernarg_load, GlobalArena};
use serde::Serialize;
use std::collections::BTreeMap;

/// Marker required on all functional-mode outputs.
pub const FUNCTIONAL_MODE_MARKER: &str = "softgpu_functional_cpu_not_gfx1201_isa";

/// SoftGPU software launch limits (not hardware).
pub const MAX_FLAT_WORKITEMS: u64 = 1_048_576;
pub const MAX_WORKGROUP_FLAT: u32 = 1_024;
pub const DEFAULT_STEP_BUDGET: u64 = 50_000_000;

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
}

/// Execute `program` for every workitem under a deterministic schedule:
/// workgroups in lexicographic order, then local ids lexicographically.
pub fn run(
    program: &Program,
    launch: LaunchConfig,
    arena: &mut GlobalArena,
    kernarg: &[u8],
) -> Result<RunReport> {
    run_with_budget(program, launch, arena, kernarg, DEFAULT_STEP_BUDGET)
}

pub fn run_with_budget(
    program: &Program,
    launch: LaunchConfig,
    arena: &mut GlobalArena,
    kernarg: &[u8],
    step_budget: u64,
) -> Result<RunReport> {
    program.validate()?;
    launch.validate()?;

    let nwg = launch.num_workgroups();
    let mut steps = 0u64;
    let mut workitems = 0u64;

    for gz in 0..nwg[2] {
        for gy in 0..nwg[1] {
            for gx in 0..nwg[0] {
                for lz in 0..launch.workgroup[2] {
                    for ly in 0..launch.workgroup[1] {
                        for lx in 0..launch.workgroup[0] {
                            let global = [
                                gx * launch.workgroup[0] + lx,
                                gy * launch.workgroup[1] + ly,
                                gz * launch.workgroup[2] + lz,
                            ];
                            let local = [lx, ly, lz];
                            let wg = [gx, gy, gz];
                            let used = run_workitem(
                                program,
                                arena,
                                kernarg,
                                global,
                                local,
                                wg,
                                step_budget.saturating_sub(steps),
                            )?;
                            steps = steps.saturating_add(used);
                            if steps > step_budget {
                                return Err(FunctionalError::StepBudgetExceeded { steps });
                            }
                            workitems += 1;
                        }
                    }
                }
            }
        }
    }

    Ok(RunReport {
        fidelity: "functional",
        mode: FUNCTIONAL_MODE_MARKER,
        note: "not_gfx1201_isa_emulation",
        kernel: program.name.clone(),
        workitems_executed: workitems,
        steps,
        grid: launch.grid,
        workgroup: launch.workgroup,
    })
}

fn run_workitem(
    program: &Program,
    arena: &mut GlobalArena,
    kernarg: &[u8],
    global: [u32; 3],
    local: [u32; 3],
    workgroup: [u32; 3],
    budget: u64,
) -> Result<u64> {
    let mut regs: BTreeMap<String, i64> = BTreeMap::new();
    let mut steps = 0u64;
    for op in &program.body {
        steps += 1;
        if steps > budget {
            return Err(FunctionalError::StepBudgetExceeded { steps });
        }
        match op {
            Op::Const { dst, ty, value } => {
                regs.insert(dst.clone(), narrow(*value, *ty)?);
            }
            Op::GlobalId { dst, dim } => {
                regs.insert(dst.clone(), i64::from(global[*dim as usize]));
            }
            Op::LocalId { dst, dim } => {
                regs.insert(dst.clone(), i64::from(local[*dim as usize]));
            }
            Op::WorkgroupId { dst, dim } => {
                regs.insert(dst.clone(), i64::from(workgroup[*dim as usize]));
            }
            Op::Add { dst, lhs, rhs, ty } => {
                let a = get_reg(&regs, lhs)?;
                let b = get_reg(&regs, rhs)?;
                regs.insert(dst.clone(), narrow(a.wrapping_add(b), *ty)?);
            }
            Op::Sub { dst, lhs, rhs, ty } => {
                let a = get_reg(&regs, lhs)?;
                let b = get_reg(&regs, rhs)?;
                regs.insert(dst.clone(), narrow(a.wrapping_sub(b), *ty)?);
            }
            Op::Mul { dst, lhs, rhs, ty } => {
                let a = get_reg(&regs, lhs)?;
                let b = get_reg(&regs, rhs)?;
                regs.insert(dst.clone(), narrow(a.wrapping_mul(b), *ty)?);
            }
            Op::KernargLoad { dst, offset, ty } => {
                regs.insert(dst.clone(), kernarg_load(kernarg, *offset, *ty)?);
            }
            Op::LoadGlobal { dst, addr, ty } => {
                let a = get_reg(&regs, addr)? as u64;
                regs.insert(dst.clone(), arena.load(a, *ty)?);
            }
            Op::StoreGlobal { addr, src, ty } => {
                let a = get_reg(&regs, addr)? as u64;
                let v = get_reg(&regs, src)?;
                arena.store(a, *ty, v)?;
            }
            Op::Ret => return Ok(steps),
        }
    }
    Err(FunctionalError::Internal(
        "fell off end of body without ret".into(),
    ))
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
