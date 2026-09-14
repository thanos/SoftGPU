//! Virtual SoftGPU agents backed by a device profile.

use crate::handle::PackedHandle;
use crate::profile::DeviceProfile;

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

/// Attributes SoftGPU may answer for an agent in Phase 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentInfoAttr {
    Name,
    VendorName,
    Feature,
    Device,
    VersionMajor,
    VersionMinor,
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
    /// Bitmask of agent features. Phase 2 keeps this at 0 (no dispatch claim).
    pub feature_mask: u32,
    pub profile_id: String,
    pub profile_revision: String,
}

impl VirtualAgent {
    /// Build the Phase 2 default GPU agent from a device profile.
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
            feature_mask: 0,
            profile_id: profile.profile_id.clone(),
            profile_revision: profile.profile_revision.clone(),
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
    fn gpu_agent_truncates_names_and_claims_no_features() {
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
        assert_eq!(agent.feature_mask, 0);
        assert_eq!(agent.name, "Radeon AI PRO R9700");
        assert_eq!(agent.vendor_name, "AMD");
    }
}
