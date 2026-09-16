//! Provenance for SoftGPU Phase 10 gfx1201 encoding tables.
//!
//! SoftGPU does **not** copy restricted AMD ISA manuals. Opcode/encoding facts
//! used here were observed with official LLVM tools and cross-checked against
//! the public LLVM AMDGPUUsage documentation.

/// Architecture id SoftGPU accepts for this crate revision.
pub const TARGET_ARCH: &str = "gfx1201";

/// Human-readable subset name for diagnostics and Article 11.
pub const SUBSET_NAME: &str = "softgpu-gfx1201-salu-v1";

/// Access date for the llvm-mc observation that seeded checked-in goldens.
pub const GOLDEN_ACCESS_DATE: &str = "2026-09-16";

/// Tool that produced checked-in encoding goldens.
pub const GOLDEN_TOOL: &str = "llvm-mc";

/// Documented public encoding reference (not a substitute for tool goldens).
pub const AMDGPU_USAGE_URL: &str = "https://llvm.org/docs/AMDGPUUsage.html";

/// License posture for SoftGPU-maintained tables.
pub const TABLE_LICENSE_NOTE: &str = "\
Hand-maintained SoftGPU tables with provenance comments. Byte encodings in \
crates/softgpu-amd-isa/goldens/ were observed from LLVM llvm-mc \
(Apache-2.0 WITH LLVM-exception). SoftGPU does not redistribute restricted \
AMD ISA PDF content.";

/// Instruction families included in `SUBSET_NAME`.
pub const SUPPORTED_FAMILIES: &[&str] = &[
    "SOPP:s_nop",
    "SOPP:s_endpgm",
    "SOPP:s_sleep",
    "SOPP:s_waitcnt",
    "SOP1:s_mov_b32",
    "SOP2:s_add_co_u32",
];
