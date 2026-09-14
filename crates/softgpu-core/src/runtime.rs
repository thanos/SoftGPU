//! Process-global SoftGPU runtime session (vendor-neutral).
//!
//! Thread-safe via a single mutex. Init is reference-counted. Handles are
//! generation-bumping so destroy/recycle cannot revive stale IDs.

use crate::agent::{AgentInfoAttr, AgentKind, VirtualAgent};
use crate::error::{Error, ErrorCategory};
use crate::fidelity::FidelityLevel;
use crate::handle::{HandleKind, PackedHandle};
use crate::profile::DeviceProfile;
use crate::trace::{SharedTrace, TraceEvent, TraceLog, TraceSink};
use std::sync::{Mutex, OnceLock};

/// Errors returned by the vendor-neutral runtime (mapped to HSA at the edge).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    NotInitialized,
    RefcountOverflow,
    InvalidArgument,
    InvalidAgent,
    UnsupportedAttribute,
    Internal(String),
}

impl RuntimeError {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotInitialized => "not_initialized",
            Self::RefcountOverflow => "refcount_overflow",
            Self::InvalidArgument => "invalid_argument",
            Self::InvalidAgent => "invalid_agent",
            Self::UnsupportedAttribute => "unsupported_attribute",
            Self::Internal(_) => "internal",
        }
    }
}

struct AgentSlot {
    generation: u32,
    live: bool,
    agent: Option<VirtualAgent>,
}

/// SoftGPU runtime state.
pub struct Runtime {
    refcount: u32,
    profile: DeviceProfile,
    agents: Vec<AgentSlot>,
    /// Monotonic generation seed so recycled slots never revive stale handles.
    next_generation: u32,
    trace: SharedTrace,
}

impl Runtime {
    pub fn new(profile: DeviceProfile) -> Result<Self, Error> {
        profile.validate()?;
        Ok(Self {
            refcount: 0,
            profile,
            agents: Vec::new(),
            next_generation: 1,
            trace: SharedTrace::new(256),
        })
    }

    pub fn profile(&self) -> &DeviceProfile {
        &self.profile
    }

    pub fn trace_snapshot(&self) -> Vec<TraceEvent> {
        self.trace.snapshot()
    }

    pub fn init(&mut self) -> Result<(), RuntimeError> {
        if self.refcount == u32::MAX {
            return Err(RuntimeError::RefcountOverflow);
        }
        if self.refcount == 0 {
            self.bootstrap_agents();
            let seq = self.trace.with_log(TraceLog::next_seq);
            self.trace.with_log(|log| {
                log.record(TraceEvent::RuntimeInit {
                    seq,
                    refcount: 1,
                    profile_id: self.profile.profile_id.clone(),
                    profile_revision: self.profile.profile_revision.clone(),
                    fidelity: TraceLog::fidelity_label(self.profile.max_fidelity),
                });
            });
        }
        self.refcount += 1;
        Ok(())
    }

    pub fn shut_down(&mut self) -> Result<(), RuntimeError> {
        if self.refcount == 0 {
            return Err(RuntimeError::NotInitialized);
        }
        self.refcount -= 1;
        let seq = self.trace.with_log(TraceLog::next_seq);
        self.trace.with_log(|log| {
            log.record(TraceEvent::RuntimeShutdown {
                seq,
                refcount: self.refcount,
            });
        });
        if self.refcount == 0 {
            self.teardown_agents();
        }
        Ok(())
    }

    pub fn is_initialized(&self) -> bool {
        self.refcount > 0
    }

    fn bootstrap_agents(&mut self) {
        self.agents.clear();
        // Phase 2: one controlled virtual GPU agent; no CPU agent yet.
        let generation = self.alloc_generation();
        let index = 0u32;
        let handle = PackedHandle::pack(HandleKind::Agent, generation, index);
        let agent = VirtualAgent::gpu_from_profile(handle, &self.profile);
        self.agents.push(AgentSlot {
            generation,
            live: true,
            agent: Some(agent),
        });
    }

    fn alloc_generation(&mut self) -> u32 {
        let generation = self.next_generation;
        // Keep generation in the 24-bit packed field; skip 0 (reserved/invalid).
        self.next_generation = if generation >= 0x00FF_FFFF {
            1
        } else {
            generation + 1
        };
        generation
    }

    fn teardown_agents(&mut self) {
        let live_count = self.agents.iter().filter(|s| s.live).count();
        for _ in 0..live_count {
            let _ = self.alloc_generation();
        }
        self.agents.clear();
    }

    fn resolve_agent(&self, handle: PackedHandle) -> Result<&VirtualAgent, RuntimeError> {
        if handle.is_invalid() || handle.kind() != Some(HandleKind::Agent) {
            return Err(RuntimeError::InvalidAgent);
        }
        let idx = handle.index() as usize;
        let slot = self.agents.get(idx).ok_or(RuntimeError::InvalidAgent)?;
        if !slot.live || slot.generation != handle.generation() {
            return Err(RuntimeError::InvalidAgent);
        }
        slot.agent.as_ref().ok_or(RuntimeError::InvalidAgent)
    }

    pub fn iterate_agents<F>(&self, mut callback: F) -> Result<(), RuntimeError>
    where
        F: FnMut(&VirtualAgent) -> Result<(), RuntimeError>,
    {
        if !self.is_initialized() {
            return Err(RuntimeError::NotInitialized);
        }
        let seq = self.trace.with_log(TraceLog::next_seq);
        self.trace.with_log(|log| {
            log.record(TraceEvent::AgentIterateBegin {
                seq,
                agent_count: self.agents.iter().filter(|s| s.live).count(),
            });
        });
        for slot in &self.agents {
            if !slot.live {
                continue;
            }
            let agent = slot.agent.as_ref().ok_or(RuntimeError::Internal(
                "live agent slot missing agent".into(),
            ))?;
            let seq = self.trace.with_log(TraceLog::next_seq);
            self.trace.with_log(|log| {
                log.record(TraceEvent::AgentIterateVisit {
                    seq,
                    agent_handle: agent.handle.raw(),
                    kind: agent.kind.as_str().to_string(),
                });
            });
            callback(agent)?;
        }
        Ok(())
    }

    pub fn agent_get_info(
        &self,
        handle: PackedHandle,
        attr: AgentInfoAttr,
    ) -> Result<AgentInfoValue, RuntimeError> {
        if !self.is_initialized() {
            return Err(RuntimeError::NotInitialized);
        }
        let agent = match self.resolve_agent(handle) {
            Ok(a) => a,
            Err(err) => {
                self.trace_get_info(handle, attr, err.as_str());
                return Err(err);
            }
        };
        let value = match attr {
            AgentInfoAttr::Name => AgentInfoValue::Name(agent.name.clone()),
            AgentInfoAttr::VendorName => AgentInfoValue::VendorName(agent.vendor_name.clone()),
            AgentInfoAttr::Feature => AgentInfoValue::Feature(agent.feature_mask),
            AgentInfoAttr::Device => AgentInfoValue::Device(agent.kind),
            // HSA Runtime Programmer's Reference 1.2 — SoftGPU advertises the
            // specification family it targets, not full conformance.
            AgentInfoAttr::VersionMajor => AgentInfoValue::U16(1),
            AgentInfoAttr::VersionMinor => AgentInfoValue::U16(2),
        };
        self.trace_get_info(handle, attr, "ok");
        Ok(value)
    }

    fn trace_get_info(&self, handle: PackedHandle, attr: AgentInfoAttr, outcome: &str) {
        let seq = self.trace.with_log(TraceLog::next_seq);
        self.trace.with_log(|log| {
            log.record(TraceEvent::AgentGetInfo {
                seq,
                agent_handle: handle.raw(),
                attribute: attr.as_str().to_string(),
                outcome: outcome.to_string(),
            });
        });
    }

    pub fn record_unsupported(&self, api: &str, detail: &str) {
        let seq = self.trace.with_log(TraceLog::next_seq);
        self.trace.with_log(|log| {
            log.record(TraceEvent::Unsupported {
                seq,
                api: api.to_string(),
                detail: detail.to_string(),
            });
        });
    }

    pub fn max_fidelity(&self) -> FidelityLevel {
        self.profile.max_fidelity
    }
}

/// Typed agent info payload before ABI packing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentInfoValue {
    Name(String),
    VendorName(String),
    Feature(u32),
    Device(AgentKind),
    U16(u16),
}

/// Process-global runtime holder.
static GLOBAL: OnceLock<Mutex<Option<Runtime>>> = OnceLock::new();

fn global() -> &'static Mutex<Option<Runtime>> {
    GLOBAL.get_or_init(|| Mutex::new(None))
}

/// Install the process runtime from a profile (idempotent replace only when down).
pub fn install_runtime(profile: DeviceProfile) -> Result<(), Error> {
    let mut guard = global()
        .lock()
        .map_err(|_| Error::new(ErrorCategory::Internal, "runtime mutex poisoned"))?;
    if let Some(rt) = guard.as_ref() {
        if rt.is_initialized() {
            return Err(Error::new(
                ErrorCategory::Internal,
                "cannot replace SoftGPU runtime while initialized",
            ));
        }
    }
    *guard = Some(Runtime::new(profile)?);
    Ok(())
}

/// Ensure a default runtime exists.
///
/// Honors `SOFTGPU_PROFILE` if set to a profile JSON path; otherwise installs
/// the generic bundled profile.
pub fn ensure_default_runtime() -> Result<(), Error> {
    let mut guard = global()
        .lock()
        .map_err(|_| Error::new(ErrorCategory::Internal, "runtime mutex poisoned"))?;
    if guard.is_none() {
        let profile = if let Ok(path) = std::env::var("SOFTGPU_PROFILE") {
            DeviceProfile::load_path(path)?
        } else {
            DeviceProfile::parse_bytes(DEFAULT_GENERIC_PROFILE.as_bytes())?
        };
        *guard = Some(Runtime::new(profile)?);
    }
    Ok(())
}

const DEFAULT_GENERIC_PROFILE: &str = include_str!("../../../profiles/softgpu-generic-v0.json");

/// Access the global runtime under the process mutex.
pub fn with_runtime<R>(f: impl FnOnce(&mut Runtime) -> R) -> Result<R, RuntimeError> {
    let mut guard = global()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let rt = guard.as_mut().ok_or(RuntimeError::NotInitialized)?;
    Ok(f(rt))
}

/// Like `with_runtime` but auto-installs the default profile if missing.
pub fn with_runtime_autostart<R>(f: impl FnOnce(&mut Runtime) -> R) -> Result<R, RuntimeError> {
    ensure_default_runtime().map_err(|e| RuntimeError::Internal(e.to_string()))?;
    with_runtime(f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{ProfileField, ProfileIdentity, ResourceLimits};
    use std::sync::Barrier;
    use std::thread;

    fn test_profile() -> DeviceProfile {
        DeviceProfile {
            schema_version: 1,
            profile_id: "test-gpu".into(),
            profile_revision: "0".into(),
            identity: ProfileIdentity {
                vendor: "SoftGPU".into(),
                product_name: "Test GPU".into(),
                architecture_family: "softgpu-abstract".into(),
                llvm_target: ProfileField::unknown(),
            },
            max_fidelity: FidelityLevel::Abi,
            conformance_allowed: false,
            resource_limits: ResourceLimits::default(),
            analytical_performance: Default::default(),
            quirks: vec![],
        }
    }

    #[test]
    fn init_enumerates_one_gpu_agent() {
        let mut rt = Runtime::new(test_profile()).unwrap();
        rt.init().unwrap();
        let mut count = 0;
        rt.iterate_agents(|agent| {
            count += 1;
            assert_eq!(agent.kind, AgentKind::Gpu);
            assert_eq!(agent.feature_mask, 0);
            Ok(())
        })
        .unwrap();
        assert_eq!(count, 1);
        let name = rt
            .agent_get_info(
                rt.agents[0].agent.as_ref().unwrap().handle,
                AgentInfoAttr::Name,
            )
            .unwrap();
        assert_eq!(name, AgentInfoValue::Name("Test GPU".into()));
        rt.shut_down().unwrap();
    }

    #[test]
    fn stale_handle_after_shutdown_fails() {
        let mut rt = Runtime::new(test_profile()).unwrap();
        rt.init().unwrap();
        let handle = rt.agents[0].agent.as_ref().unwrap().handle;
        rt.shut_down().unwrap();
        let err = rt.agent_get_info(handle, AgentInfoAttr::Name).unwrap_err();
        assert_eq!(err, RuntimeError::NotInitialized);
        rt.init().unwrap();
        // Old handle generation must not resolve against a new agent table.
        let err = rt.agent_get_info(handle, AgentInfoAttr::Name).unwrap_err();
        assert_eq!(err, RuntimeError::InvalidAgent);
        rt.shut_down().unwrap();
    }

    #[test]
    fn forged_handle_rejected() {
        let mut rt = Runtime::new(test_profile()).unwrap();
        rt.init().unwrap();
        let forged = PackedHandle::from_raw(0xDEAD_BEEF_DEAD_BEEF);
        assert_eq!(
            rt.agent_get_info(forged, AgentInfoAttr::Device)
                .unwrap_err(),
            RuntimeError::InvalidAgent
        );
        rt.shut_down().unwrap();
    }

    #[test]
    fn concurrent_init_and_iterate() {
        let mut rt = Runtime::new(test_profile()).unwrap();
        rt.init().unwrap();
        // Move into mutex for shared access in this unit test.
        let shared = Mutex::new(rt);
        let barrier = Barrier::new(4);
        thread::scope(|scope| {
            for _ in 0..4 {
                scope.spawn(|| {
                    barrier.wait();
                    let guard = shared.lock().unwrap();
                    guard
                        .iterate_agents(|agent| {
                            assert_eq!(agent.kind, AgentKind::Gpu);
                            Ok(())
                        })
                        .unwrap();
                });
            }
        });
        shared.lock().unwrap().shut_down().unwrap();
    }

    #[test]
    fn traces_include_profile_and_fidelity() {
        let mut rt = Runtime::new(test_profile()).unwrap();
        rt.init().unwrap();
        let events = rt.trace_snapshot();
        assert!(events.iter().any(|e| matches!(
            e,
            TraceEvent::RuntimeInit {
                fidelity,
                profile_id,
                ..
            } if fidelity == "abi" && profile_id == "test-gpu"
        )));
        rt.shut_down().unwrap();
    }
}
