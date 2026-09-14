//! Vendor-neutral SoftGPU core.
//!
//! Owns profiles, generation-safe handles, runtime session state, virtual
//! agents, memory/signals/queues, and structured traces. AMD/HSA ABI types
//! stay in `softgpu-hsa`.

pub mod agent;
pub mod aql;
pub mod error;
pub mod fidelity;
pub mod handle;
pub mod memory;
pub mod profile;
pub mod queue;
pub mod runtime;
pub mod signal;
pub mod trace;

pub use agent::{AgentInfoAttr, AgentKind, VirtualAgent, AGENT_FEATURE_KERNEL_DISPATCH};
pub use aql::{
    golden_kernel_dispatch_1d, parse_kernel_dispatch, parse_supported_packet, replay_dispatch,
    AqlParseError, DispatchDescriptor, KernargClass, PacketType, DIAGNOSTIC_COMPLETE_NO_EXECUTION,
    DIAGNOSTIC_REJECTED, PACKET_TYPE_AGENT_DISPATCH, PACKET_TYPE_BARRIER_AND,
    PACKET_TYPE_BARRIER_OR,
};
pub use error::{Error, ErrorCategory, Result};
pub use fidelity::FidelityLevel;
pub use handle::{HandleKind, PackedHandle};
pub use memory::{AllocationMeta, MemorySpace, MemoryViewKind};
pub use profile::{
    CapabilityProvenance, DeviceProfile, ProfileField, ProfileIdentity, SupportState,
};
pub use queue::{HsaQueueAbi, PacketObservation, SoftGpuQueue};
pub use runtime::{PoolInfoAttr, RegionInfoAttr, Runtime, RuntimeError};
pub use signal::{SignalCondition, SignalWaitOutcome, SoftGpuSignal};
pub use trace::{TraceEvent, TraceLog, TraceSink};

/// SoftGPU core crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Active roadmap phase label for this crate revision.
pub const ACTIVE_PHASE: &str = "phase-4";
