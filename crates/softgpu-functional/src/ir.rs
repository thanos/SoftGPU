//! SoftGPU Functional IR (`softgpu-sfir-v1`).

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    Const {
        dst: String,
        ty: TypeId,
        value: i64,
    },
    /// SoftGPU global linear id for dimension 0/1/2.
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
    /// Load pointer/value from kernarg blob at byte offset.
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
        for op in &self.body {
            match op {
                Op::GlobalId { dim, .. }
                | Op::LocalId { dim, .. }
                | Op::WorkgroupId { dim, .. }
                    if *dim > 2 =>
                {
                    return Err(FunctionalError::Validation {
                        detail: format!("dim {dim} out of range 0..=2"),
                    });
                }
                _ => {}
            }
        }
        Ok(())
    }
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
