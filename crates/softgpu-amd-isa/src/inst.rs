//! Decoded SoftGPU gfx1201 instructions (`softgpu-gfx1201-compute-v2`).

/// Scalar source / dest encoding byte (AMDGPU operand encoding).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarEnc(pub u8);

/// SoftGPU SOPC compare ops (sets SCC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SCmpOp {
    EqI32,
    LtI32,
    EqU32,
    LgU32,
    GtU32,
    GeU32,
    LeU32,
}

/// SoftGPU SOPP conditional branches (dword offset from next PC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SBranchCond {
    Scc0,
    Scc1,
    ExecZ,
    ExecNz,
}

/// SoftGPU SOP2 arithmetic / logic (subset).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sop2Op {
    AddCoU32,
    SubCoU32,
    AndB32,
    OrB32,
    XorB32,
    MinU32,
    MaxU32,
    MulI32,
}

/// SoftGPU SOP1 ops beyond `s_mov_b32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sop1Op {
    MovB32,
    BrevB32,
    NotB32,
    AndSaveexecB32,
    OrSaveexecB32,
}

/// SoftGPU SOPK ops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SopkOp {
    MovkI32,
    AddkCoI32,
    MulkI32,
}

/// SoftGPU VOP1 ops (e32).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vop1Op {
    MovB32,
    NotB32,
    BfrevB32,
}

/// SoftGPU VOP2 ops (e32).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vop2Op {
    CndmaskB32,
    MinU32,
    MaxU32,
    LshlRevB32,
    LshrRevB32,
    AshrRevI32,
    AndB32,
    OrB32,
    XorB32,
    AddNcU32,
    SubNcU32,
}

/// SoftGPU VOPC compare ops (write VCC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VopcOp {
    EqU32,
    NeU32,
    LtU32,
    GtU32,
    LeU32,
    GeU32,
}

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
    /// SOPP conditional branch.
    SCbranch { cond: SBranchCond, simm16: u16 },
    /// SOPC compare → SCC.
    SCmp {
        op: SCmpOp,
        ssrc0: ScalarEnc,
        ssrc1: ScalarEnc,
    },
    /// SOPK with 16-bit signed immediate.
    Sopk {
        op: SopkOp,
        sdst: ScalarEnc,
        simm16: u16,
    },
    /// SOP1.
    Sop1 {
        op: Sop1Op,
        sdst: ScalarEnc,
        ssrc0: ScalarEnc,
    },
    /// SOP2.
    Sop2 {
        op: Sop2Op,
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
    /// VOP1 e32.
    Vop1 {
        op: Vop1Op,
        vdst: u8,
        src0_enc: u16,
        size: u32,
    },
    /// VOP2 e32.
    Vop2 {
        op: Vop2Op,
        vdst: u8,
        src0_enc: u16,
        src1: u8,
        size: u32,
    },
    /// VOPC e32 → VCC.
    Vopc {
        op: VopcOp,
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
            Inst::SCbranch { cond, .. } => match cond {
                SBranchCond::Scc0 => "s_cbranch_scc0",
                SBranchCond::Scc1 => "s_cbranch_scc1",
                SBranchCond::ExecZ => "s_cbranch_execz",
                SBranchCond::ExecNz => "s_cbranch_execnz",
            },
            Inst::SCmp { op, .. } => match op {
                SCmpOp::EqI32 => "s_cmp_eq_i32",
                SCmpOp::LtI32 => "s_cmp_lt_i32",
                SCmpOp::EqU32 => "s_cmp_eq_u32",
                SCmpOp::LgU32 => "s_cmp_lg_u32",
                SCmpOp::GtU32 => "s_cmp_gt_u32",
                SCmpOp::GeU32 => "s_cmp_ge_u32",
                SCmpOp::LeU32 => "s_cmp_le_u32",
            },
            Inst::Sopk { op, .. } => match op {
                SopkOp::MovkI32 => "s_movk_i32",
                SopkOp::AddkCoI32 => "s_addk_co_i32",
                SopkOp::MulkI32 => "s_mulk_i32",
            },
            Inst::Sop1 { op, .. } => match op {
                Sop1Op::MovB32 => "s_mov_b32",
                Sop1Op::BrevB32 => "s_brev_b32",
                Sop1Op::NotB32 => "s_not_b32",
                Sop1Op::AndSaveexecB32 => "s_and_saveexec_b32",
                Sop1Op::OrSaveexecB32 => "s_or_saveexec_b32",
            },
            Inst::Sop2 { op, .. } => match op {
                Sop2Op::AddCoU32 => "s_add_co_u32",
                Sop2Op::SubCoU32 => "s_sub_co_u32",
                Sop2Op::AndB32 => "s_and_b32",
                Sop2Op::OrB32 => "s_or_b32",
                Sop2Op::XorB32 => "s_xor_b32",
                Sop2Op::MinU32 => "s_min_u32",
                Sop2Op::MaxU32 => "s_max_u32",
                Sop2Op::MulI32 => "s_mul_i32",
            },
            Inst::SLoadB64 { .. } => "s_load_b64",
            Inst::Vop1 { op, .. } => match op {
                Vop1Op::MovB32 => "v_mov_b32_e32",
                Vop1Op::NotB32 => "v_not_b32_e32",
                Vop1Op::BfrevB32 => "v_bfrev_b32_e32",
            },
            Inst::Vop2 { op, .. } => match op {
                Vop2Op::CndmaskB32 => "v_cndmask_b32_e32",
                Vop2Op::MinU32 => "v_min_u32_e32",
                Vop2Op::MaxU32 => "v_max_u32_e32",
                Vop2Op::LshlRevB32 => "v_lshlrev_b32_e32",
                Vop2Op::LshrRevB32 => "v_lshrrev_b32_e32",
                Vop2Op::AshrRevI32 => "v_ashrrev_i32_e32",
                Vop2Op::AndB32 => "v_and_b32_e32",
                Vop2Op::OrB32 => "v_or_b32_e32",
                Vop2Op::XorB32 => "v_xor_b32_e32",
                Vop2Op::AddNcU32 => "v_add_nc_u32_e32",
                Vop2Op::SubNcU32 => "v_sub_nc_u32_e32",
            },
            Inst::Vopc { op, .. } => match op {
                VopcOp::EqU32 => "v_cmp_eq_u32_e32",
                VopcOp::NeU32 => "v_cmp_ne_u32_e32",
                VopcOp::LtU32 => "v_cmp_lt_u32_e32",
                VopcOp::GtU32 => "v_cmp_gt_u32_e32",
                VopcOp::LeU32 => "v_cmp_le_u32_e32",
                VopcOp::GeU32 => "v_cmp_ge_u32_e32",
            },
            Inst::GlobalLoadB32 { .. } => "global_load_b32",
            Inst::GlobalStoreB32 { .. } => "global_store_b32",
        }
    }

    /// Encoded size in bytes (4 or 8/12 for memory forms SoftGPU claims).
    pub fn size_bytes(&self) -> u32 {
        match self {
            Inst::SLoadB64 { size, .. }
            | Inst::Vop1 { size, .. }
            | Inst::Vop2 { size, .. }
            | Inst::Vopc { size, .. }
            | Inst::GlobalLoadB32 { size, .. }
            | Inst::GlobalStoreB32 { size, .. } => *size,
            _ => 4,
        }
    }

    /// True when `apply` sets PC (branch taken or not).
    pub fn redirects_pc(&self) -> bool {
        matches!(self, Inst::SCbranch { .. })
    }
}
