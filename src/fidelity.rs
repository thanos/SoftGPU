//! Fidelity vocabulary for SoftGPU runs, diagnostics, and reports.
//!
//! Every public claim must name its fidelity level. See the master prompt §5
//! and `docs/architecture.md`.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Declared fidelity level for a SoftGPU result or advertisement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FidelityLevel {
    /// Host process can call a declared HSA/ROCr subset with verified C ABI.
    Abi,
    /// Queues, signals, AQL, registration, and dispatch flow for a declared subset.
    Protocol,
    /// Kernels execute on a CPU-backed semantic engine; not gfx1201 ISA evidence.
    Functional,
    /// Supported gfx1201 instructions execute per verified architectural semantics.
    ArchitecturalIsa,
    /// Extra checking that may perturb scheduling/storage/timing.
    Sanitized,
    /// Parameterized estimates only; not cycle accuracy unless separately named.
    AnalyticalPerformance,
    /// Named test + toolchain + device profile + real hardware sample evidence.
    HardwareConformant,
}

impl FidelityLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Abi => "abi",
            Self::Protocol => "protocol",
            Self::Functional => "functional",
            Self::ArchitecturalIsa => "architectural-isa",
            Self::Sanitized => "sanitized",
            Self::AnalyticalPerformance => "analytical-performance",
            Self::HardwareConformant => "hardware-conformant",
        }
    }
}

impl fmt::Display for FidelityLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_round_trip_uses_kebab_case() {
        let json = serde_json::to_string(&FidelityLevel::ArchitecturalIsa).unwrap();
        assert_eq!(json, "\"architectural-isa\"");
        let parsed: FidelityLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, FidelityLevel::ArchitecturalIsa);
    }
}
