//! Provenance for SoftGPU gfx1201 encoding tables (`softgpu-gfx1201-compute-v2`).
//!
//! SoftGPU does **not** copy restricted AMD ISA manuals. Opcode/encoding facts
//! used here were observed with official LLVM tools and cross-checked against
//! the public LLVM AMDGPUUsage documentation.

/// Architecture id SoftGPU accepts for this crate revision.
pub const TARGET_ARCH: &str = "gfx1201";

/// Nested Phase 11 subset (still accepted / tested).
pub const SUBSET_E2E_TINY_V1: &str = "softgpu-gfx1201-e2e-tiny-v1";

/// Human-readable primary subset name for diagnostics (v0.7+).
pub const SUBSET_NAME: &str = "softgpu-gfx1201-compute-v2";

/// Access date for the llvm-mc observation that seeded checked-in goldens.
pub const GOLDEN_ACCESS_DATE: &str = "2026-09-16";

/// Tool that produced checked-in encoding goldens / tiny kernel `.text`.
pub const GOLDEN_TOOL: &str = "llvm-mc";

/// Documented public encoding reference (not a substitute for tool goldens).
pub const AMDGPU_USAGE_URL: &str = "https://llvm.org/docs/AMDGPUUsage.html";

/// License posture for SoftGPU-maintained tables.
pub const TABLE_LICENSE_NOTE: &str = "\
Hand-maintained SoftGPU tables with provenance comments. Byte encodings in \
crates/softgpu-amd-isa/goldens/ and kernel text blobs were observed from LLVM \
llvm-mc (Apache-2.0 WITH LLVM-exception). SoftGPU does not redistribute \
restricted AMD ISA PDF content.";

/// Instruction families included in `SUBSET_NAME`.
pub const SUPPORTED_FAMILIES: &[&str] = &[
    "SOPP:s_nop",
    "SOPP:s_endpgm",
    "SOPP:s_sleep",
    "SOPP:s_waitcnt",
    "SOPP:s_cbranch_scc0",
    "SOPP:s_cbranch_scc1",
    "SOPP:s_cbranch_execz",
    "SOPP:s_cbranch_execnz",
    "SOPC:s_cmp_eq_i32",
    "SOPC:s_cmp_lt_i32",
    "SOPC:s_cmp_eq_u32",
    "SOPC:s_cmp_lg_u32",
    "SOPC:s_cmp_gt_u32",
    "SOPC:s_cmp_ge_u32",
    "SOPC:s_cmp_le_u32",
    "SOPK:s_movk_i32",
    "SOPK:s_addk_co_i32",
    "SOPK:s_mulk_i32",
    "SOP1:s_mov_b32",
    "SOP1:s_brev_b32",
    "SOP1:s_not_b32",
    "SOP1:s_and_saveexec_b32",
    "SOP1:s_or_saveexec_b32",
    "SOP2:s_add_co_u32",
    "SOP2:s_sub_co_u32",
    "SOP2:s_and_b32",
    "SOP2:s_or_b32",
    "SOP2:s_xor_b32",
    "SOP2:s_min_u32",
    "SOP2:s_max_u32",
    "SOP2:s_mul_i32",
    "SMEM:s_load_b64",
    "VOP1:v_mov_b32_e32",
    "VOP1:v_not_b32_e32",
    "VOP1:v_bfrev_b32_e32",
    "VOP2:v_cndmask_b32_e32",
    "VOP2:v_min_u32_e32",
    "VOP2:v_max_u32_e32",
    "VOP2:v_lshlrev_b32_e32",
    "VOP2:v_lshrrev_b32_e32",
    "VOP2:v_ashrrev_i32_e32",
    "VOP2:v_and_b32_e32",
    "VOP2:v_or_b32_e32",
    "VOP2:v_xor_b32_e32",
    "VOP2:v_add_nc_u32_e32",
    "VOP2:v_sub_nc_u32_e32",
    "VOPC:v_cmp_eq_u32_e32",
    "VOPC:v_cmp_ne_u32_e32",
    "VOPC:v_cmp_lt_u32_e32",
    "VOPC:v_cmp_gt_u32_e32",
    "VOPC:v_cmp_le_u32_e32",
    "VOPC:v_cmp_ge_u32_e32",
    "GLOBAL:global_load_b32",
    "GLOBAL:global_store_b32",
];
