//! SoftGPU Phase 0 library surface.
//!
//! This crate intentionally contains **no** ROCr/HSA ABI implementation.
//! Phase 0 establishes error taxonomy, device-profile schema with provenance,
//! fidelity vocabulary, and a small CLI for validation smoke tests.

pub mod error;
pub mod fidelity;
pub mod profile;

pub use error::{Error, ErrorCategory, Result};
pub use fidelity::FidelityLevel;
pub use profile::{
    CapabilityProvenance, DeviceProfile, ProfileField, ProfileIdentity, SupportState,
};

/// SoftGPU crate version string.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Active roadmap phase label.
pub const ACTIVE_PHASE: &str = "phase-0";
