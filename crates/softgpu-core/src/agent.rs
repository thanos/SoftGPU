//! Virtual SoftGPU agents backed by a device profile.

use crate::handle::PackedHandle;
use crate::profile::DeviceProfile;
use crate::queue::{
    QUEUE_TYPE_MULTI, SOFTGPU_QUEUES_MAX, SOFTGPU_QUEUE_MAX_SIZE, SOFTGPU_QUEUE_MIN_SIZE,
};

/// SoftGPU agent device class (vendor-neutral).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentKind {
    Cpu,
    Gpu,
}

impl AgentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Gpu => "gpu",
        }
    }
}

/// `HSA_AGENT_FEATURE_KERNEL_DISPATCH` — SoftGPU queue + AQL intercept claim only.
pub const AGENT_FEATURE_KERNEL_DISPATCH: u32 = 1;

/// Attributes SoftGPU may answer for an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentInfoAttr {
    Name,
    VendorName,
    Feature,
    Device,
    VersionMajor,
    VersionMinor,
    QueuesMax,
    QueueMinSize,
    QueueMaxSize,
    QueueType,
}

impl AgentInfoAttr {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::VendorName => "vendor_name",
            Self::Feature => "feature",
            Self::Device => "device",
            Self::VersionMajor => "version_major",
            Self::VersionMinor => "version_minor",
            Self::QueuesMax => "queues_max",
            Self::QueueMinSize => "queue_min_size",
            Self::QueueMaxSize => "queue_max_size",
            Self::QueueType => "queue_type",
        }
    }
}

/// One virtual agent instance.
#[derive(Debug, Clone)]
pub struct VirtualAgent {
    pub handle: PackedHandle,
    pub kind: AgentKind,
    pub name: String,
    pub vendor_name: String,
    /// Bitmask of agent features. SoftGPU: `KERNEL_DISPATCH` for queue + AQL intercept only.
    pub feature_mask: u32,
    pub profile_id: String,
    pub profile_revision: String,
    pub queues_max: u32,
    pub queue_min_size: u32,
    pub queue_max_size: u32,
    pub queue_type: u32,
}

impl VirtualAgent {
    /// Build the SoftGPU GPU agent from a device profile.
    pub fn gpu_from_profile(handle: PackedHandle, profile: &DeviceProfile) -> Self {
        let mut name = profile.identity.product_name.clone();
        name.truncate(63);
        let mut vendor_name = profile.identity.vendor.clone();
        vendor_name.truncate(63);
        Self {
            handle,
            kind: AgentKind::Gpu,
            name,
            vendor_name,
            // SoftGPU Phase 3: advertise kernel-agent for queue ABI observation.
            // Packet execution remains unsupported (Phase 4).
            feature_mask: AGENT_FEATURE_KERNEL_DISPATCH,
            profile_id: profile.profile_id.clone(),
            profile_revision: profile.profile_revision.clone(),
            queues_max: SOFTGPU_QUEUES_MAX,
            queue_min_size: SOFTGPU_QUEUE_MIN_SIZE,
            queue_max_size: SOFTGPU_QUEUE_MAX_SIZE,
            queue_type: QUEUE_TYPE_MULTI,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fidelity::FidelityLevel;
    use crate::handle::HandleKind;
    use crate::profile::{ProfileField, ProfileIdentity, ResourceLimits};

    #[test]
    fn gpu_agent_truncates_names_and_claims_kernel_dispatch() {
        let profile = DeviceProfile {
            schema_version: 1,
            profile_id: "amd-radeon-ai-pro-r9700-gfx1201".into(),
            profile_revision: "0".into(),
            identity: ProfileIdentity {
                vendor: "AMD".into(),
                product_name: "Radeon AI PRO R9700".into(),
                architecture_family: "RDNA 4".into(),
                llvm_target: ProfileField::verified(
                    "gfx1201".into(),
                    "docs/sources.md#rocm-compatibility-r9700-gfx1201",
                ),
            },
            max_fidelity: FidelityLevel::Abi,
            conformance_allowed: false,
            resource_limits: ResourceLimits::default(),
            analytical_performance: Default::default(),
            quirks: vec![],
        };
        let agent =
            VirtualAgent::gpu_from_profile(PackedHandle::pack(HandleKind::Agent, 1, 0), &profile);
        assert_eq!(agent.kind, AgentKind::Gpu);
        assert_eq!(agent.feature_mask, AGENT_FEATURE_KERNEL_DISPATCH);
        assert_eq!(agent.name, "Radeon AI PRO R9700");
        assert_eq!(agent.vendor_name, "AMD");
    }
}
