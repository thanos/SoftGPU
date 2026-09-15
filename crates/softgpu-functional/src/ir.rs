//! SoftGPU Functional IR (`softgpu-sfir-v1`).
//!
//! Phase 7 extends the disclosed op set with waves/lanes, group memory,
//! barriers, structured divergence, comparisons, and selected atomics.
//! Schema id remains `softgpu-sfir-v1`; unknown ops still fail at parse time.

use crate::error::{FunctionalError, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Schema id embedded in every program document.
pub const SFIR_SCHEMA: &str = "softgpu-sfir-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TypeId {
    I32,
    U32,
    U64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AddrSpace {
    Global,
    Group,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AtomicScope {
    /// SoftGPU workgroup scope (not a hardware memory-order claim).
    Workgroup,
    /// SoftGPU device/global arena scope (functional atomic only).
    Device,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AtomicOrder {
    /// SoftGPU sequential functional atomic; not a claim of GPU memory model.
    Relaxed,
    AcqRel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    Const {
        dst: String,
        ty: TypeId,
        value: i64,
    },
    GlobalId {
        dst: String,
        dim: u8,
    },
    LocalId {
        dst: String,
        dim: u8,
    },
    WorkgroupId {
        dst: String,
        dim: u8,
    },
    /// SoftGPU software lane id within the wave (`flat_local % wave_size`).
    LaneId {
        dst: String,
    },
    /// SoftGPU software wave id within the workgroup (`flat_local / wave_size`).
    WaveId {
        dst: String,
    },
    WaveSize {
        dst: String,
    },
    Add {
        dst: String,
        lhs: String,
        rhs: String,
        ty: TypeId,
    },
    Sub {
        dst: String,
        lhs: String,
        rhs: String,
        ty: TypeId,
    },
    Mul {
        dst: String,
        lhs: String,
        rhs: String,
        ty: TypeId,
    },
    CmpEq {
        dst: String,
        lhs: String,
        rhs: String,
        ty: TypeId,
    },
    CmpNe {
        dst: String,
        lhs: String,
        rhs: String,
        ty: TypeId,
    },
    /// Bitwise and (SoftGPU scalar; used for lane predication masks).
    And {
        dst: String,
        lhs: String,
        rhs: String,
        ty: TypeId,
    },
    KernargLoad {
        dst: String,
        offset: u32,
        ty: TypeId,
    },
    LoadGlobal {
        dst: String,
        addr: String,
        ty: TypeId,
    },
    StoreGlobal {
        addr: String,
        src: String,
        ty: TypeId,
    },
    LoadGroup {
        dst: String,
        addr: String,
        ty: TypeId,
    },
    StoreGroup {
        addr: String,
        src: String,
        ty: TypeId,
    },
    /// Workgroup barrier (SoftGPU generation sync). Illegal inside `If`/`While`.
    Barrier,
    /// Structured divergence: active lanes with `cond != 0` run `then_body`,
    /// others run `else_body`, then reconverge. SoftGPU SIMT, not gfx1201.
    If {
        cond: String,
        then_body: Vec<Op>,
        #[serde(default)]
        else_body: Vec<Op>,
    },
    /// SoftGPU loop: while any active lane has `cond != 0`, those lanes run `body`.
    While {
        cond: String,
        body: Vec<Op>,
    },
    /// `dst = atomic_add(addr, src)` returning the previous value (i32 only).
    AtomicAdd {
        dst: String,
        addr: String,
        src: String,
        space: AddrSpace,
        scope: AtomicScope,
        order: AtomicOrder,
    },
    Ret,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KernargField {
    pub name: String,
    pub offset: u32,
    pub size: u32,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub schema: String,
    pub fidelity: String,
    pub note: String,
    pub name: String,
    pub source_provenance: String,
    #[serde(default)]
    pub kernarg_layout: Vec<KernargField>,
    /// SoftGPU group/LDS bytes required for this program (software limit).
    #[serde(default)]
    pub group_bytes: u32,
    pub body: Vec<Op>,
}

impl Program {
    pub fn validate(&self) -> Result<()> {
        if self.schema != SFIR_SCHEMA {
            return Err(FunctionalError::Validation {
                detail: format!("schema '{}' != '{SFIR_SCHEMA}'", self.schema),
            });
        }
        if self.fidelity != "functional" {
            return Err(FunctionalError::Validation {
                detail: format!("fidelity must be 'functional', got '{}'", self.fidelity),
            });
        }
        if self.note != "not_gfx1201_isa_emulation" {
            return Err(FunctionalError::Validation {
                detail: format!(
                    "note must be 'not_gfx1201_isa_emulation', got '{}'",
                    self.note
                ),
            });
        }
        if self.body.is_empty() {
            return Err(FunctionalError::Validation {
                detail: "empty body".into(),
            });
        }
        if !matches!(self.body.last(), Some(Op::Ret)) {
            return Err(FunctionalError::Validation {
                detail: "body must end with ret".into(),
            });
        }
        validate_ops(&self.body, /*allow_barrier*/ true)?;
        Ok(())
    }

    pub fn has_barrier(&self) -> bool {
        ops_have_barrier(&self.body)
    }
}

fn ops_have_barrier(ops: &[Op]) -> bool {
    for op in ops {
        match op {
            Op::Barrier => return true,
            Op::If {
                then_body,
                else_body,
                ..
            } => {
                if ops_have_barrier(then_body) || ops_have_barrier(else_body) {
                    return true;
                }
            }
            Op::While { body, .. } if ops_have_barrier(body) => return true,
            _ => {}
        }
    }
    false
}

fn validate_ops(ops: &[Op], allow_barrier: bool) -> Result<()> {
    for op in ops {
        match op {
            Op::GlobalId { dim, .. } | Op::LocalId { dim, .. } | Op::WorkgroupId { dim, .. }
                if *dim > 2 =>
            {
                return Err(FunctionalError::Validation {
                    detail: format!("dim {dim} out of range 0..=2"),
                });
            }
            Op::Barrier if !allow_barrier => {
                return Err(FunctionalError::Validation {
                    detail: "barrier is illegal inside if/while (divergent barrier unsupported)"
                        .into(),
                });
            }
            Op::If {
                then_body,
                else_body,
                ..
            } => {
                validate_ops(then_body, false)?;
                validate_ops(else_body, false)?;
            }
            Op::While { body, .. } => {
                validate_ops(body, false)?;
            }
            Op::AtomicAdd { .. } => {}
            Op::Ret => {}
            _ => {}
        }
    }
    Ok(())
}

/// Split a top-level body into barrier-separated segments (Ret stripped from last).
pub fn barrier_segments(body: &[Op]) -> Result<Vec<Vec<Op>>> {
    let mut segs = Vec::new();
    let mut cur = Vec::new();
    for op in body {
        match op {
            Op::Barrier => {
                segs.push(std::mem::take(&mut cur));
            }
            Op::Ret => {
                segs.push(std::mem::take(&mut cur));
                break;
            }
            other => cur.push(other.clone()),
        }
    }
    if segs.is_empty() {
        return Err(FunctionalError::Validation {
            detail: "empty barrier segments".into(),
        });
    }
    Ok(segs)
}

pub fn load_program_str(s: &str) -> Result<Program> {
    let p: Program = serde_json::from_str(s).map_err(|e| FunctionalError::Parse(e.to_string()))?;
    p.validate()?;
    Ok(p)
}

pub fn load_program_path(path: impl AsRef<Path>) -> Result<Program> {
    let s =
        std::fs::read_to_string(path.as_ref()).map_err(|e| FunctionalError::Io(e.to_string()))?;
    load_program_str(&s)
}
