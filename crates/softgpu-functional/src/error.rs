//! Fail-closed functional-execution errors.

use crate::sanitize::Finding;
use std::fmt;

pub type Result<T> = std::result::Result<T, FunctionalError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionalError {
    Io(String),
    Parse(String),
    Unsupported {
        detail: String,
    },
    Validation {
        detail: String,
    },
    Bounds {
        addr: u64,
        size: usize,
        arena_len: usize,
    },
    UndefinedReg {
        name: String,
    },
    StepBudgetExceeded {
        steps: u64,
    },
    /// SoftGPU Phase 8 sanitizer finding (fail-fast or hard fault).
    Sanitize(Finding),
    Internal(String),
}

impl fmt::Display for FunctionalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(s) => write!(f, "io: {s}"),
            Self::Parse(s) => write!(f, "parse: {s}"),
            Self::Unsupported { detail } => write!(f, "unsupported: {detail}"),
            Self::Validation { detail } => write!(f, "validation: {detail}"),
            Self::Bounds {
                addr,
                size,
                arena_len,
            } => write!(
                f,
                "bounds: addr={addr:#x} size={size} arena_len={arena_len}"
            ),
            Self::UndefinedReg { name } => write!(f, "undefined register '{name}'"),
            Self::StepBudgetExceeded { steps } => {
                write!(f, "step budget exceeded after {steps} steps")
            }
            Self::Sanitize(finding) => write!(
                f,
                "sanitize: {:?} space={:?} addr={:#x} detail={}",
                finding.kind, finding.space, finding.addr, finding.detail
            ),
            Self::Internal(s) => write!(f, "internal: {s}"),
        }
    }
}

impl std::error::Error for FunctionalError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_covers_variants() {
        let cases = [
            FunctionalError::Io("x".into()),
            FunctionalError::Parse("p".into()),
            FunctionalError::Unsupported { detail: "u".into() },
            FunctionalError::Validation { detail: "v".into() },
            FunctionalError::Bounds {
                addr: 0x10,
                size: 4,
                arena_len: 2,
            },
            FunctionalError::UndefinedReg { name: "r0".into() },
            FunctionalError::StepBudgetExceeded { steps: 9 },
            FunctionalError::Sanitize(crate::sanitize::Finding {
                kind: crate::sanitize::FindingKind::Race,
                space: crate::ir::AddrSpace::Global,
                addr: 0,
                size: 4,
                step: 1,
                barrier_gen: 0,
                actor: crate::sanitize::WorkItemId {
                    workgroup: [0, 0, 0],
                    wave: 0,
                    lane: 0,
                    flat_local: 0,
                },
                other: None,
                detail: "t".into(),
            }),
            FunctionalError::Internal("i".into()),
        ];
        for err in cases {
            let s = err.to_string();
            assert!(!s.is_empty(), "{err:?}");
        }
    }
}
