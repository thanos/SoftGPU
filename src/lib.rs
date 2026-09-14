//! SoftGPU library surface (CLI + re-exports of `softgpu-core`).

pub use softgpu_core::{
    agent, error, fidelity, handle, memory, profile, queue, runtime, signal, trace, ACTIVE_PHASE,
    VERSION,
};
pub use softgpu_core::{
    AgentInfoAttr, AgentKind, CapabilityProvenance, DeviceProfile, Error, ErrorCategory,
    FidelityLevel, HandleKind, PackedHandle, ProfileField, ProfileIdentity, Result, Runtime,
    RuntimeError, SupportState, TraceEvent, TraceLog, VirtualAgent, AGENT_FEATURE_KERNEL_DISPATCH,
};
