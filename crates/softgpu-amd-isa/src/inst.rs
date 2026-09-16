//! Decoded SoftGPU gfx1201 instructions (Phase 10 SALU + Phase 11 e2e tiny).

/// Scalar source / dest encoding byte (AMDGPU operand encoding).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarEnc(pub u8);

/// SoftGPU decoded instruction (named subsets only).
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
    /// `s_load_b64 s[sdst:sdst+1], s[sbase:sbase+1], offset`
    SLoadB64 {
        sdst: u8,
        sbase: u8,
        offset: u32,
        size: u32,
    },
    /// `v_lshlrev_b32_e32 vdst, src0_imm_or_enc, src1_vgpr`
    VLshlRevB32E32 {
        vdst: u8,
        src0_enc: u16,
        src1: u8,
        size: u32,
    },
    /// `v_add_nc_u32_e32 vdst, src0, src1`
    VAddNcU32E32 {
        vdst: u8,
        src0_enc: u16,
        src1: u8,
        size: u32,
    },
    /// `global_load_b32 vdst, vaddr, s[saddr:saddr+1]`
    GlobalLoadB32 {
        vdst: u8,
        vaddr: u8,
        saddr: u8,
        size: u32,
    },
    /// `global_store_b32 vaddr, vdata, s[saddr:saddr+1]`
    GlobalStoreB32 {
        vaddr: u8,
        vdata: u8,
        saddr: u8,
        size: u32,
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
            Inst::SLoadB64 { .. } => "s_load_b64",
            Inst::VLshlRevB32E32 { .. } => "v_lshlrev_b32_e32",
            Inst::VAddNcU32E32 { .. } => "v_add_nc_u32_e32",
            Inst::GlobalLoadB32 { .. } => "global_load_b32",
            Inst::GlobalStoreB32 { .. } => "global_store_b32",
        }
    }

    /// Encoded size in bytes (4, 8, or 12 for SoftGPU Phase 11 subset).
    pub fn size_bytes(&self) -> u32 {
        match self {
            Inst::SLoadB64 { size, .. }
            | Inst::VLshlRevB32E32 { size, .. }
            | Inst::VAddNcU32E32 { size, .. }
            | Inst::GlobalLoadB32 { size, .. }
            | Inst::GlobalStoreB32 { size, .. } => *size,
            _ => 4,
        }
    }
}
