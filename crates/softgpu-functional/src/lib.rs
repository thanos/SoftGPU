//! SoftGPU Phase 6 functional execution (CPU) + Phase 7 GPU semantic machine
//! + Phase 8 sanitizers.
//!
//! **Research gate (locked):** SoftGPU Functional IR (`softgpu-sfir-v1`) — an
//! explicit, SoftGPU-owned IR. Phase 7 adds SoftGPU software waves/lanes, group
//! memory, barriers, structured divergence, and selected atomics. Phase 8 adds
//! a declared SoftGPU happens-before sanitizer subset. SoftGPU does **not**
//! claim gfx1201 ISA emulation.
//!
//! See `docs/functional-path.md` and Articles 7–9.

pub mod error;
pub mod exec;
pub mod ir;
pub mod kernels;
pub mod memory;
pub mod sanitize;

pub use error::{FunctionalError, Result};
pub use exec::{
    run, run_with_budget, run_with_config, run_with_config_sanitized, run_with_sanitizer,
    ExecConfig, LaunchConfig, RunReport, SchedulePolicy, DEFAULT_WAVE_SIZE, FUNCTIONAL_MODE_MARKER,
    MAX_GROUP_BYTES,
};
pub use ir::{
    load_program_path, load_program_str, AddrSpace, AtomicOrder, AtomicScope, Op, Program, TypeId,
    SFIR_SCHEMA,
};
pub use memory::GlobalArena;
pub use sanitize::{
    Finding, FindingKind, ReplayBundle, SanitizeMode, SanitizeReport, Sanitizer, WorkItemId,
    MAX_SHADOW_BYTES, SANITIZER_REPLAY_SCHEMA,
};

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
