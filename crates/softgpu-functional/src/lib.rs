//! SoftGPU Phase 6 functional execution (CPU).
//!
//! **Research gate (locked):** SoftGPU Functional IR (`softgpu-sfir-v1`) — an
//! explicit, SoftGPU-owned IR with JSON fixtures and hand-translated provenance
//! from tiny reference C sources. SoftGPU does **not** claim that arbitrary AMD
//! code objects contain executable high-level IR, and does **not** claim gfx1201
//! ISA emulation.
//!
//! See `docs/functional-path.md` and Article 7.

pub mod error;
pub mod exec;
pub mod ir;
pub mod kernels;
pub mod memory;

pub use error::{FunctionalError, Result};
pub use exec::{run, run_with_budget, LaunchConfig, RunReport, FUNCTIONAL_MODE_MARKER};
pub use ir::{load_program_path, load_program_str, Op, Program, TypeId, SFIR_SCHEMA};
pub use memory::GlobalArena;

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
