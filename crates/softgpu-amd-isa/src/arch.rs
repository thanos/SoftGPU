//! Architecture / feature gate for SoftGPU ISA interpretation.

use crate::error::{IsaError, IsaErrorKind, Result};
use crate::provenance::TARGET_ARCH;

/// SoftGPU-supported AMDGPU ISA target for Phase 10.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    Gfx1201,
}

impl Arch {
    pub fn parse(s: &str) -> Result<Self> {
        match s.trim() {
            "gfx1201" => Ok(Self::Gfx1201),
            other => Err(IsaError::new(
                IsaErrorKind::Unsupported,
                format!("unsupported ISA target '{other}'"),
            )
            .with_remediation(format!("SoftGPU Phase 10 accepts only {TARGET_ARCH}"))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gfx1201 => TARGET_ARCH,
        }
    }
}

/// SoftGPU wave size is a **software parameter**, not R9700 wavefront evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaveSize {
    Wave32 = 32,
    Wave64 = 64,
}

impl WaveSize {
    pub fn lanes(self) -> usize {
        self as usize
    }

    pub fn parse(n: u32) -> Result<Self> {
        match n {
            32 => Ok(Self::Wave32),
            64 => Ok(Self::Wave64),
            other => Err(IsaError::new(
                IsaErrorKind::Config,
                format!("unsupported SoftGPU wave_size={other}"),
            )
            .with_remediation("use 32 or 64 (SoftGPU software parameter)")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_gfx1201() {
        assert_eq!(Arch::parse("gfx1201").unwrap(), Arch::Gfx1201);
        assert!(Arch::parse("gfx1100").is_err());
    }
}
