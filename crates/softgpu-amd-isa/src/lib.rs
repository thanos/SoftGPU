//! SoftGPU `gfx1201` ISA (`softgpu-gfx1201-compute-v2`).
//!
//! Broader SALU/VALU compute subset for SoftGPU v0.7. Nested Phase 11
//! `tiny_add` path remains supported.
//!
//! Fidelity: **Architectural ISA** for the named subset only.
//! Unsupported encodings trap before silently corrupting state.

pub mod arch;
pub mod decode;
pub mod disasm;
pub mod error;
pub mod exec;
pub mod inst;
pub mod kernel;
pub mod mem;
pub mod provenance;
pub mod state;
pub mod word;

pub use arch::{Arch, WaveSize};
pub use decode::{decode_at, decode_word};
pub use disasm::{disasm_at, disasm_word};
pub use error::{IsaError, IsaErrorKind, Result, TrapKind};
pub use exec::{run, run_salu, step, step_salu, StepOutcome};
pub use inst::{
    Inst, SBranchCond, SCmpOp, ScalarEnc, Sop1Op, Sop2Op, SopkOp, Vop1Op, Vop2Op, VopcOp,
};
pub use kernel::{
    clamp64_host_ref, run_clamp64_1d, run_code_1d, run_select_gt50_1d, run_tiny_add_1d,
    select_gt50_host_ref, tiny_add_host_ref, TinyAddKernarg, CLAMP64_TEXT, SELECT_GT50_TEXT,
    TINY_ADD_TEXT,
};
pub use mem::{GlobalArena, IsaMemory};
pub use provenance::{
    AMDGPU_USAGE_URL, GOLDEN_ACCESS_DATE, GOLDEN_TOOL, SUBSET_E2E_TINY_V1, SUBSET_NAME,
    SUPPORTED_FAMILIES, TABLE_LICENSE_NOTE, TARGET_ARCH,
};
pub use state::{MachineState, SGPR_COUNT, VGPR_PER_LANE};
pub use word::{fetch_word, parse_hex_word, word_to_le_bytes, words_to_code};

/// SoftGPU amd-isa crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
