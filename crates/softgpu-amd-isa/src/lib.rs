//! SoftGPU `gfx1201` ISA foundation (Phase 10).
//!
//! Narrow, evidence-sourced decoder/interpreter for a named SALU subset.
//! Fidelity: **Architectural ISA** for `softgpu-gfx1201-salu-v1` only.
//! Unsupported encodings trap before silently corrupting state.
//!
//! This crate does **not** claim HIP/HSA AQL kernel success (Phase 11).

pub mod arch;
pub mod decode;
pub mod disasm;
pub mod error;
pub mod exec;
pub mod inst;
pub mod provenance;
pub mod state;
pub mod word;

pub use arch::{Arch, WaveSize};
pub use decode::decode_word;
pub use disasm::{disasm_at, disasm_word};
pub use error::{IsaError, IsaErrorKind, Result, TrapKind};
pub use exec::{run, step, StepOutcome};
pub use inst::{Inst, ScalarEnc};
pub use provenance::{
    AMDGPU_USAGE_URL, GOLDEN_ACCESS_DATE, GOLDEN_TOOL, SUBSET_NAME, SUPPORTED_FAMILIES,
    TABLE_LICENSE_NOTE, TARGET_ARCH,
};
pub use state::{MachineState, SGPR_COUNT, VGPR_PER_LANE};
pub use word::{fetch_word, parse_hex_word, word_to_le_bytes, words_to_code};

/// SoftGPU amd-isa crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
