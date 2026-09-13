//! Device profile schema with per-field provenance.
//!
//! A profile is a versioned compatibility contract, not a bag of marketing
//! strings. Unknown is valid. SoftGPU must never invent numeric device facts
//! merely because an API requests them.

use crate::error::{Error, ErrorCategory, Result};
use crate::fidelity::FidelityLevel;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// How a profile field was established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapabilityProvenance {
    /// Confirmed against a primary source or reproducible measurement.
    Verified,
    /// Observed in a pinned toolchain/runtime without full normative citation.
    Observed,
    /// Derived from related evidence; not directly measured.
    Inferred,
    /// Explicitly unknown; callers must not treat as a concrete capability.
    Unknown,
    /// Temporary stand-in required for scaffolding; never used for conformance.
    Provisional,
}

impl CapabilityProvenance {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Observed => "observed",
            Self::Inferred => "inferred",
            Self::Unknown => "unknown",
            Self::Provisional => "provisional",
        }
    }

    /// Whether this provenance may participate in conformance claims.
    pub fn allows_conformance(self) -> bool {
        matches!(self, Self::Verified | Self::Observed)
    }
}

/// Machine-readable support verification state (support matrix cells).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SupportState {
    ImplementedUnverified,
    VerifiedUnit,
    VerifiedIntegration,
    HardwareDifferential,
    Experimental,
    Unsupported,
}

impl SupportState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ImplementedUnverified => "implemented-unverified",
            Self::VerifiedUnit => "verified-unit",
            Self::VerifiedIntegration => "verified-integration",
            Self::HardwareDifferential => "hardware-differential",
            Self::Experimental => "experimental",
            Self::Unsupported => "unsupported",
        }
    }
}

/// A typed profile field carrying value + provenance + optional notes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileField<T> {
    pub value: Option<T>,
    pub provenance: CapabilityProvenance,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

impl<T> ProfileField<T> {
    pub fn unknown() -> Self {
        Self {
            value: None,
            provenance: CapabilityProvenance::Unknown,
            source_ref: None,
            notes: Some("not established; do not invent".into()),
        }
    }

    pub fn verified(value: T, source_ref: impl Into<String>) -> Self {
        Self {
            value: Some(value),
            provenance: CapabilityProvenance::Verified,
            source_ref: Some(source_ref.into()),
            notes: None,
        }
    }

    pub fn provisional(value: T, reason: impl Into<String>) -> Self {
        Self {
            value: Some(value),
            provenance: CapabilityProvenance::Provisional,
            source_ref: None,
            notes: Some(reason.into()),
        }
    }
}

/// Identity advertised through a runtime adapter (Phase 2+).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileIdentity {
    pub vendor: String,
    pub product_name: String,
    pub architecture_family: String,
    pub llvm_target: ProfileField<String>,
}

/// Versioned device profile document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceProfile {
    pub schema_version: u32,
    pub profile_id: String,
    pub profile_revision: String,
    pub identity: ProfileIdentity,
    /// Maximum advertised fidelity SoftGPU may claim with this profile today.
    pub max_fidelity: FidelityLevel,
    /// Whether any conformance claim is permitted for this profile revision.
    pub conformance_allowed: bool,
    /// Resource limits used to reject impossible launches. Unknown is valid.
    pub resource_limits: ResourceLimits,
    /// Optional analytical performance parameters (never used as correctness).
    #[serde(default)]
    pub analytical_performance: AnalyticalPerformanceParams,
    #[serde(default)]
    pub quirks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ResourceLimits {
    pub max_workgroup_size_x: ProfileField<u32>,
    pub max_workgroup_size_y: ProfileField<u32>,
    pub max_workgroup_size_z: ProfileField<u32>,
    pub wavefront_size: ProfileField<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AnalyticalPerformanceParams {
    /// Explicitly empty in Phase 0; present so schema shape is stable.
    #[serde(default)]
    pub notes: Vec<String>,
}

impl Default for ProfileField<u32> {
    fn default() -> Self {
        Self::unknown()
    }
}

impl DeviceProfile {
    pub const CURRENT_SCHEMA_VERSION: u32 = 1;

    /// Load and validate a profile JSON document from disk.
    pub fn load_path(path: impl AsRef<Path>) -> Result<Self> {
        let bytes = fs::read(path.as_ref())?;
        Self::parse_bytes(&bytes)
    }

    /// Parse and validate profile bytes.
    pub fn parse_bytes(bytes: &[u8]) -> Result<Self> {
        let profile: DeviceProfile = serde_json::from_slice(bytes)?;
        profile.validate()?;
        Ok(profile)
    }

    /// Validate schema invariants without trusting serde alone.
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != Self::CURRENT_SCHEMA_VERSION {
            return Err(Error::new(
                ErrorCategory::Profile,
                format!(
                    "unsupported profile schema_version {}; expected {}",
                    self.schema_version,
                    Self::CURRENT_SCHEMA_VERSION
                ),
            )
            .with_remediation("migrate the profile or use a SoftGPU build that supports it"));
        }

        if self.profile_id.trim().is_empty() {
            return Err(Error::new(
                ErrorCategory::Profile,
                "profile_id must be non-empty",
            ));
        }

        if self.identity.vendor.trim().is_empty()
            || self.identity.product_name.trim().is_empty()
            || self.identity.architecture_family.trim().is_empty()
        {
            return Err(Error::new(
                ErrorCategory::Profile,
                "identity.vendor/product_name/architecture_family must be non-empty",
            ));
        }

        if self.conformance_allowed {
            if self.max_fidelity == FidelityLevel::HardwareConformant {
                return Err(Error::new(
                    ErrorCategory::Profile,
                    "conformance_allowed cannot be true for hardware-conformant max_fidelity until Phase 12 evidence exists",
                )
                .with_remediation("set conformance_allowed=false or lower max_fidelity"));
            }
            // Any provisional/unknown resource limit forbids conformance.
            for (name, field) in [
                (
                    "max_workgroup_size_x",
                    &self.resource_limits.max_workgroup_size_x,
                ),
                (
                    "max_workgroup_size_y",
                    &self.resource_limits.max_workgroup_size_y,
                ),
                (
                    "max_workgroup_size_z",
                    &self.resource_limits.max_workgroup_size_z,
                ),
                ("wavefront_size", &self.resource_limits.wavefront_size),
            ] {
                if !field.provenance.allows_conformance() {
                    return Err(Error::new(
                        ErrorCategory::Profile,
                        format!(
                            "conformance_allowed requires verified/observed provenance for {name}; found {}",
                            field.provenance.as_str()
                        ),
                    ));
                }
            }
            if !self.identity.llvm_target.provenance.allows_conformance() {
                return Err(Error::new(
                    ErrorCategory::Profile,
                    format!(
                        "conformance_allowed requires verified/observed llvm_target; found {}",
                        self.identity.llvm_target.provenance.as_str()
                    ),
                ));
            }
        }

        // llvm_target may be unknown, but if a value is present it must be non-empty.
        if let Some(target) = &self.identity.llvm_target.value {
            if target.trim().is_empty() {
                return Err(Error::new(
                    ErrorCategory::Profile,
                    "identity.llvm_target.value must not be an empty string when present",
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generic_profile() -> DeviceProfile {
        DeviceProfile {
            schema_version: 1,
            profile_id: "softgpu-generic".into(),
            profile_revision: "0".into(),
            identity: ProfileIdentity {
                vendor: "SoftGPU".into(),
                product_name: "Generic Test Device".into(),
                architecture_family: "softgpu-abstract".into(),
                llvm_target: ProfileField::unknown(),
            },
            max_fidelity: FidelityLevel::Abi,
            conformance_allowed: false,
            resource_limits: ResourceLimits::default(),
            analytical_performance: AnalyticalPerformanceParams::default(),
            quirks: vec![],
        }
    }

    #[test]
    fn valid_generic_profile_passes() {
        generic_profile().validate().unwrap();
    }

    #[test]
    fn wrong_schema_version_fails() {
        let mut p = generic_profile();
        p.schema_version = 99;
        let err = p.validate().unwrap_err();
        assert_eq!(err.category(), ErrorCategory::Profile);
        assert!(err.message().contains("schema_version"));
    }

    #[test]
    fn conformance_forbidden_with_unknown_limits() {
        let mut p = generic_profile();
        p.conformance_allowed = true;
        p.identity.llvm_target =
            ProfileField::verified("gfx1201".into(), "docs/sources.md#llvm-amdgpu");
        let err = p.validate().unwrap_err();
        assert!(err.message().contains("conformance_allowed"));
    }

    #[test]
    fn empty_llvm_target_value_rejected() {
        let mut p = generic_profile();
        p.identity.llvm_target = ProfileField {
            value: Some("".into()),
            provenance: CapabilityProvenance::Provisional,
            source_ref: None,
            notes: Some("bad".into()),
        };
        let err = p.validate().unwrap_err();
        assert!(err.message().contains("llvm_target"));
    }
}
