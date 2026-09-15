//! SoftGPU Phase 6 functional execution (CPU) + Phase 7 GPU semantic machine.
//!
//! **Research gate (locked):** SoftGPU Functional IR (`softgpu-sfir-v1`) — an
//! explicit, SoftGPU-owned IR. Phase 7 adds SoftGPU software waves/lanes, group
//! memory, barriers, structured divergence, and selected atomics. SoftGPU does
//! **not** claim gfx1201 ISA emulation.
//!
//! See `docs/functional-path.md` and Articles 7–8.

pub mod error;
pub mod exec;
pub mod ir;
pub mod kernels;
pub mod memory;

pub use error::{FunctionalError, Result};
pub use exec::{
    run, run_with_budget, run_with_config, ExecConfig, LaunchConfig, RunReport, SchedulePolicy,
    DEFAULT_WAVE_SIZE, FUNCTIONAL_MODE_MARKER, MAX_GROUP_BYTES,
};
pub use ir::{
    load_program_path, load_program_str, AddrSpace, AtomicOrder, AtomicScope, Op, Program, TypeId,
    SFIR_SCHEMA,
};
pub use memory::GlobalArena;

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
