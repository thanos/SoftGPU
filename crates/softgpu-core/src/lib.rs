//! Vendor-neutral SoftGPU core.
//!
//! Owns profiles, generation-safe handles, runtime session state, virtual
//! agents, and structured traces. AMD/HSA ABI types stay in `softgpu-hsa`.

pub mod agent;
pub mod error;
pub mod fidelity;
pub mod handle;
pub mod profile;
pub mod runtime;
pub mod trace;

pub use agent::{AgentInfoAttr, AgentKind, VirtualAgent};
pub use error::{Error, ErrorCategory, Result};
pub use fidelity::FidelityLevel;
pub use handle::{HandleKind, PackedHandle};
pub use profile::{
    CapabilityProvenance, DeviceProfile, ProfileField, ProfileIdentity, SupportState,
};
pub use runtime::{Runtime, RuntimeError};
pub use trace::{TraceEvent, TraceLog, TraceSink};

/// SoftGPU core crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Active roadmap phase label for this crate revision.
pub const ACTIVE_PHASE: &str = "phase-2";
