//! Decoded SoftGPU gfx1201 SALU instructions (Phase 10 subset).

/// Scalar source / dest encoding byte (AMDGPU operand encoding).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarEnc(pub u8);

/// SoftGPU Phase 10 decoded instruction (named subset only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inst {
    /// `s_nop simm16`
    SNop { simm16: u16 },
    /// `s_endpgm`
    SEndPgm,
    /// `s_sleep simm16` — SoftGPU: no-op (sequential interpreter; not HW sleep).
    SSleep { simm16: u16 },
    /// `s_waitcnt simm16` — SoftGPU: no-op (no async memory pipeline).
    SWaitCnt { simm16: u16 },
    /// `s_mov_b32 sdst, ssrc0`
    SMovB32 { sdst: ScalarEnc, ssrc0: ScalarEnc },
    /// `s_add_co_u32 sdst, ssrc0, ssrc1` — carry out to SCC.
    SAddCoU32 {
        sdst: ScalarEnc,
        ssrc0: ScalarEnc,
        ssrc1: ScalarEnc,
    },
}

impl Inst {
    pub fn mnemonic(&self) -> &'static str {
        match self {
            Inst::SNop { .. } => "s_nop",
            Inst::SEndPgm => "s_endpgm",
            Inst::SSleep { .. } => "s_sleep",
            Inst::SWaitCnt { .. } => "s_waitcnt",
            Inst::SMovB32 { .. } => "s_mov_b32",
            Inst::SAddCoU32 { .. } => "s_add_co_u32",
        }
    }
}
