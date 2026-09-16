//! SoftGPU gfx1201 machine state for Phase 10 SALU interpretation.
//!
//! Fidelity: **Architectural ISA** for the named SoftGPU subset only.
//! `wave_size` is a SoftGPU software parameter (not R9700 wavefront proof).

use crate::arch::{Arch, WaveSize};
use crate::error::{IsaError, IsaErrorKind, Result, TrapKind};
use crate::inst::ScalarEnc;

/// Number of architected SGPRs SoftGPU models for Phase 10.
pub const SGPR_COUNT: usize = 106;
/// SoftGPU VGPR file size per lane (present for future VALU; unused in Phase 10).
pub const VGPR_PER_LANE: usize = 256;

/// Explicit SoftGPU ISA machine state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineState {
    pub arch: Arch,
    pub wave_size: WaveSize,
    /// Byte PC into the code buffer (4-byte aligned for Phase 10 SALU).
    pub pc: u32,
    pub sgpr: [u32; SGPR_COUNT],
    /// Per-lane VGPRs. SoftGPU allocates for `wave_size` lanes.
    pub vgpr: Vec<[u32; VGPR_PER_LANE]>,
    /// Scalar condition code (carry / compare).
    pub scc: bool,
    /// EXEC mask (low `wave_size` bits meaningful).
    pub exec: u64,
    /// VCC mask (present; Phase 10 SALU subset does not update it).
    pub vcc: u64,
    pub halted: bool,
}

impl MachineState {
    pub fn new(arch: Arch, wave_size: WaveSize) -> Self {
        let lanes = wave_size.lanes();
        let exec = if wave_size == WaveSize::Wave32 {
            0xffff_ffff
        } else {
            u64::MAX
        };
        Self {
            arch,
            wave_size,
            pc: 0,
            sgpr: [0; SGPR_COUNT],
            vgpr: vec![[0; VGPR_PER_LANE]; lanes],
            scc: false,
            exec,
            vcc: 0,
            halted: false,
        }
    }

    pub fn lane_active(&self, lane: usize) -> bool {
        if lane >= self.wave_size.lanes() {
            return false;
        }
        (self.exec >> lane) & 1 == 1
    }

    pub fn read_scalar(&self, enc: ScalarEnc, pc: u32) -> Result<u32> {
        match resolve_src(enc) {
            Ok(ScalarValue::Sgpr(i)) => Ok(self.sgpr[i]),
            Ok(ScalarValue::Imm(v)) => Ok(v),
            Err(role) => Err(TrapKind::UnsupportedOperand {
                encoding: enc.0,
                role,
                pc,
            }
            .to_error()),
        }
    }

    pub fn write_sgpr(&mut self, enc: ScalarEnc, value: u32, pc: u32) -> Result<()> {
        match resolve_dst(enc) {
            Ok(i) => {
                self.sgpr[i] = value;
                Ok(())
            }
            Err(role) => Err(TrapKind::UnsupportedOperand {
                encoding: enc.0,
                role,
                pc,
            }
            .to_error()),
        }
    }

    pub fn read_sgpr_pair(&self, base: u8, pc: u32) -> Result<u64> {
        let b = base as usize;
        if b + 1 >= SGPR_COUNT || b % 2 != 0 {
            return Err(TrapKind::UnsupportedOperand {
                encoding: base,
                role: "sgpr_pair",
                pc,
            }
            .to_error());
        }
        let lo = self.sgpr[b] as u64;
        let hi = self.sgpr[b + 1] as u64;
        Ok(lo | (hi << 32))
    }

    pub fn write_sgpr_pair(&mut self, base: u8, value: u64, pc: u32) -> Result<()> {
        let b = base as usize;
        if b + 1 >= SGPR_COUNT || b % 2 != 0 {
            return Err(TrapKind::UnsupportedOperand {
                encoding: base,
                role: "sgpr_pair",
                pc,
            }
            .to_error());
        }
        self.sgpr[b] = value as u32;
        self.sgpr[b + 1] = (value >> 32) as u32;
        Ok(())
    }
}

enum ScalarValue {
    Sgpr(usize),
    Imm(u32),
}

fn resolve_src(enc: ScalarEnc) -> std::result::Result<ScalarValue, &'static str> {
    let e = enc.0 as usize;
    if e < SGPR_COUNT {
        return Ok(ScalarValue::Sgpr(e));
    }
    // Inline integer constants: 128..=192 → 0..=64; 193 → -1.
    // Source: LLVM AMDGPU operand encoding / llvm-mc goldens for s_mov_b32 immediates.
    if (128..=192).contains(&e) {
        return Ok(ScalarValue::Imm((e - 128) as u32));
    }
    if e == 193 {
        return Ok(ScalarValue::Imm(u32::MAX));
    }
    Err("ssrc")
}

fn resolve_dst(enc: ScalarEnc) -> std::result::Result<usize, &'static str> {
    let e = enc.0 as usize;
    if e < SGPR_COUNT {
        Ok(e)
    } else {
        Err("sdst")
    }
}

/// Validate that `arch` is SoftGPU Phase 10 target.
pub fn require_gfx1201(arch: &str) -> Result<Arch> {
    Arch::parse(arch).map_err(|e| {
        IsaError::new(IsaErrorKind::Unsupported, e.message())
            .with_remediation(e.format_diagnostic())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_constants() {
        let st = MachineState::new(Arch::Gfx1201, WaveSize::Wave32);
        assert_eq!(st.read_scalar(ScalarEnc(0x80), 0).unwrap(), 0);
        assert_eq!(st.read_scalar(ScalarEnc(0x81), 0).unwrap(), 1);
        assert_eq!(st.read_scalar(ScalarEnc(0xc0), 0).unwrap(), 64);
        assert_eq!(st.read_scalar(ScalarEnc(0xc1), 0).unwrap(), u32::MAX);
    }
}
