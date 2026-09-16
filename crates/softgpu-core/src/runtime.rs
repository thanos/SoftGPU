//! Process-global SoftGPU runtime session (vendor-neutral).
//!
//! Thread-safe via a single mutex. Init is reference-counted. Handles are
//! generation-bumping so destroy/recycle cannot revive stale IDs.

use crate::agent::{AgentInfoAttr, AgentKind, VirtualAgent};
use crate::aql::{
    parse_supported_packet, DispatchDescriptor, KernargClass, DIAGNOSTIC_COMPLETE_NO_EXECUTION,
    DIAGNOSTIC_REJECTED, KERNEL_SUCCESS,
};
use crate::error::{Error, ErrorCategory};
use crate::fidelity::FidelityLevel;
use crate::handle::{HandleKind, PackedHandle};
use crate::memory::{
    AllocError, MemorySpace, MemoryViewKind, PoolInfoValue, RegionInfoValue, SoftGpuAllocator,
    AMD_POOL_FLAGS_COARSE, AMD_POOL_FLAGS_FINE_KERNARG, AMD_POOL_LOCATION_CPU,
    AMD_POOL_LOCATION_GPU, AMD_SEGMENT_GLOBAL, REGION_FLAGS_COARSE, REGION_FLAGS_FINE_KERNARG,
    REGION_SEGMENT_GLOBAL, SOFTGPU_ALLOC_ALIGNMENT, SOFTGPU_ALLOC_GRANULE, SOFTGPU_MAX_ALLOC_BYTES,
    SOFTGPU_POOL_BYTES,
};
use crate::profile::DeviceProfile;
use crate::queue::{
    init_packet_buffer, is_power_of_two, HsaQueueAbi, PacketObservation, SoftGpuQueue,
    AQL_PACKET_BYTES, QUEUE_FEATURE_KERNEL_DISPATCH, QUEUE_TYPE_MULTI, SOFTGPU_QUEUES_MAX,
};
use crate::signal::{SignalCondition, SignalWaitOutcome, SoftGpuSignal};
use crate::trace::{SharedTrace, TraceEvent, TraceLog, TraceSink};
use softgpu_amd_isa::{run_code_1d, IsaMemory, WaveSize, TINY_ADD_TEXT};
use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Errors returned by the vendor-neutral runtime (mapped to HSA at the edge).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    NotInitialized,
    RefcountOverflow,
    InvalidArgument,
    InvalidAgent,
    InvalidRegion,
    InvalidPool,
    InvalidSignal,
    InvalidQueue,
    InvalidQueueCreation,
    InvalidAllocation,
    OutOfResources,
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
            Self::InvalidRegion => "invalid_region",
            Self::InvalidPool => "invalid_pool",
            Self::InvalidSignal => "invalid_signal",
            Self::InvalidQueue => "invalid_queue",
            Self::InvalidQueueCreation => "invalid_queue_creation",
            Self::InvalidAllocation => "invalid_allocation",
            Self::OutOfResources => "out_of_resources",
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

struct SpaceSlot {
    generation: u32,
    live: bool,
    space: Option<MemorySpace>,
}

struct SignalSlot {
    generation: u32,
    live: bool,
    signal: Option<std::sync::Arc<SoftGpuSignal>>,
}

struct QueueSlot {
    #[allow(dead_code)]
    generation: u32,
    live: bool,
    queue: Option<SoftGpuQueue>,
}

/// SoftGPU-registered ISA kernel image (Phase 11 / v0.8 executable load).
#[derive(Debug, Clone)]
pub struct RegisteredIsaKernel {
    pub name: String,
    pub code: Vec<u8>,
    pub kernarg_segment_size: u32,
    pub group_segment_size: u32,
    pub private_segment_size: u32,
}

/// SoftGPU runtime state.
pub struct Runtime {
    refcount: u32,
    profile: DeviceProfile,
    agents: Vec<AgentSlot>,
    regions: Vec<SpaceSlot>,
    pools: Vec<SpaceSlot>,
    signals: Vec<SignalSlot>,
    queues: Vec<QueueSlot>,
    allocator: SoftGpuAllocator,
    next_generation: u32,
    next_queue_id: u64,
    /// Captured validated dispatches for offline replay (owned bytes).
    dispatch_captures: Vec<DispatchDescriptor>,
    /// kernel_object handle → SoftGPU ISA image (Phase 11 / v0.8).
    isa_kernels: HashMap<u64, RegisteredIsaKernel>,
    next_kernel_object: u64,
    /// SoftGPU code-object readers (v0.8).
    code_object_readers: HashMap<u64, crate::executable::CodeObjectReader>,
    next_reader_id: u64,
    /// SoftGPU executables (v0.8).
    executables: HashMap<u64, crate::executable::SoftGpuExecutable>,
    next_executable_id: u64,
    /// executable_symbol handle → (executable_id, symbol_index).
    executable_symbols: HashMap<u64, (u64, usize)>,
    next_symbol_id: u64,
    trace: SharedTrace,
}

impl Runtime {
    pub fn new(profile: DeviceProfile) -> Result<Self, Error> {
        profile.validate()?;
        Ok(Self {
            refcount: 0,
            profile,
            agents: Vec::new(),
            regions: Vec::new(),
            pools: Vec::new(),
            signals: Vec::new(),
            queues: Vec::new(),
            allocator: SoftGpuAllocator::new(SOFTGPU_POOL_BYTES),
            next_generation: 1,
            next_queue_id: 1,
            dispatch_captures: Vec::new(),
            isa_kernels: HashMap::new(),
            next_kernel_object: 1,
            code_object_readers: HashMap::new(),
            next_reader_id: 1,
            executables: HashMap::new(),
            next_executable_id: 1,
            executable_symbols: HashMap::new(),
            next_symbol_id: 1,
            trace: SharedTrace::new(512),
        })
    }

    /// Register a SoftGPU ISA kernel; returns `kernel_object` id for AQL packets.
    pub fn register_isa_kernel(
        &mut self,
        name: impl Into<String>,
        code: Vec<u8>,
    ) -> Result<u64, RuntimeError> {
        self.register_isa_kernel_ex(name, code, 0, 0, 0)
    }

    /// Register ISA kernel with segment sizes (HSA symbol_get_info).
    pub fn register_isa_kernel_ex(
        &mut self,
        name: impl Into<String>,
        code: Vec<u8>,
        kernarg_segment_size: u32,
        group_segment_size: u32,
        private_segment_size: u32,
    ) -> Result<u64, RuntimeError> {
        if code.is_empty() {
            return Err(RuntimeError::InvalidArgument);
        }
        let id = self.next_kernel_object;
        self.next_kernel_object = self.next_kernel_object.saturating_add(1);
        self.isa_kernels.insert(
            id,
            RegisteredIsaKernel {
                name: name.into(),
                code,
                kernarg_segment_size,
                group_segment_size,
                private_segment_size,
            },
        );
        Ok(id)
    }

    /// Register the SoftGPU llvm-mc `tiny_add` kernel image.
    pub fn register_builtin_tiny_add(&mut self) -> Result<u64, RuntimeError> {
        self.register_isa_kernel_ex("tiny_add", TINY_ADD_TEXT.to_vec(), 16, 0, 0)
    }

    pub fn lookup_isa_kernel(&self, kernel_object: u64) -> Option<&RegisteredIsaKernel> {
        self.isa_kernels.get(&kernel_object)
    }

    pub fn code_object_reader_create_from_memory(
        &mut self,
        bytes: &[u8],
    ) -> Result<u64, RuntimeError> {
        if bytes.is_empty() {
            return Err(RuntimeError::InvalidArgument);
        }
        let id = self.next_reader_id;
        self.next_reader_id = self.next_reader_id.saturating_add(1);
        self.code_object_readers.insert(
            id,
            crate::executable::CodeObjectReader {
                bytes: bytes.to_vec(),
            },
        );
        Ok(id)
    }

    pub fn code_object_reader_destroy(&mut self, reader_id: u64) -> Result<(), RuntimeError> {
        if self.code_object_readers.remove(&reader_id).is_none() {
            return Err(RuntimeError::InvalidArgument);
        }
        Ok(())
    }

    pub fn executable_create(&mut self) -> Result<u64, RuntimeError> {
        let id = self.next_executable_id;
        self.next_executable_id = self.next_executable_id.saturating_add(1);
        self.executables.insert(
            id,
            crate::executable::SoftGpuExecutable {
                frozen: false,
                symbols: Vec::new(),
            },
        );
        Ok(id)
    }

    pub fn executable_destroy(&mut self, exec_id: u64) -> Result<(), RuntimeError> {
        let Some(exec) = self.executables.remove(&exec_id) else {
            return Err(RuntimeError::InvalidArgument);
        };
        self.executable_symbols
            .retain(|_, (eid, _)| *eid != exec_id);
        for sym in exec.symbols {
            self.isa_kernels.remove(&sym.kernel_object);
        }
        Ok(())
    }

    pub fn executable_load_agent_code_object(
        &mut self,
        exec_id: u64,
        reader_id: u64,
    ) -> Result<(), RuntimeError> {
        let frozen = self
            .executables
            .get(&exec_id)
            .map(|e| e.frozen)
            .ok_or(RuntimeError::InvalidArgument)?;
        if frozen {
            return Err(RuntimeError::InvalidArgument);
        }
        let bytes = self
            .code_object_readers
            .get(&reader_id)
            .map(|r| r.bytes.clone())
            .ok_or(RuntimeError::InvalidArgument)?;
        let kernels = crate::executable::parse_agent_kernels(&bytes)
            .map_err(|_| RuntimeError::InvalidArgument)?;
        let mut symbols = Vec::new();
        for k in kernels {
            let ko = self.register_isa_kernel_ex(
                k.name.clone(),
                k.text,
                k.kernarg_segment_size,
                k.group_segment_size,
                k.private_segment_size,
            )?;
            symbols.push(crate::executable::ExecutableSymbol {
                name: k.name,
                symbol: k.symbol,
                kernel_object: ko,
                kernarg_segment_size: k.kernarg_segment_size,
                kernarg_segment_align: k.kernarg_segment_align.max(1),
                group_segment_size: k.group_segment_size,
                private_segment_size: k.private_segment_size,
                launch_abi: k.launch_abi,
            });
        }
        let exec = self
            .executables
            .get_mut(&exec_id)
            .ok_or(RuntimeError::InvalidArgument)?;
        exec.symbols.extend(symbols);
        Ok(())
    }

    pub fn executable_freeze(&mut self, exec_id: u64) -> Result<(), RuntimeError> {
        let Some(exec) = self.executables.get_mut(&exec_id) else {
            return Err(RuntimeError::InvalidArgument);
        };
        if exec.symbols.is_empty() {
            return Err(RuntimeError::InvalidArgument);
        }
        exec.frozen = true;
        Ok(())
    }

    pub fn executable_get_symbol_by_name(
        &mut self,
        exec_id: u64,
        name: &str,
    ) -> Result<u64, RuntimeError> {
        let Some(exec) = self.executables.get(&exec_id) else {
            return Err(RuntimeError::InvalidArgument);
        };
        if !exec.frozen {
            return Err(RuntimeError::InvalidArgument);
        }
        let idx_map = crate::executable::index_symbols(&exec.symbols);
        let Some(&sym_idx) = idx_map.get(name) else {
            return Err(RuntimeError::InvalidArgument);
        };
        let id = self.next_symbol_id;
        self.next_symbol_id = self.next_symbol_id.saturating_add(1);
        self.executable_symbols.insert(id, (exec_id, sym_idx));
        Ok(id)
    }

    pub fn executable_symbol_kernel_object(&self, symbol_id: u64) -> Result<u64, RuntimeError> {
        let Some(&(exec_id, sym_idx)) = self.executable_symbols.get(&symbol_id) else {
            return Err(RuntimeError::InvalidArgument);
        };
        let Some(exec) = self.executables.get(&exec_id) else {
            return Err(RuntimeError::InvalidArgument);
        };
        let Some(sym) = exec.symbols.get(sym_idx) else {
            return Err(RuntimeError::InvalidArgument);
        };
        Ok(sym.kernel_object)
    }

    pub fn executable_symbol_info(
        &self,
        symbol_id: u64,
    ) -> Result<&crate::executable::ExecutableSymbol, RuntimeError> {
        let Some(&(exec_id, sym_idx)) = self.executable_symbols.get(&symbol_id) else {
            return Err(RuntimeError::InvalidArgument);
        };
        let Some(exec) = self.executables.get(&exec_id) else {
            return Err(RuntimeError::InvalidArgument);
        };
        exec.symbols
            .get(sym_idx)
            .ok_or(RuntimeError::InvalidArgument)
    }

    pub fn executable_is_frozen(&self, exec_id: u64) -> Result<bool, RuntimeError> {
        self.executables
            .get(&exec_id)
            .map(|e| e.frozen)
            .ok_or(RuntimeError::InvalidArgument)
    }

    /// Owned packet captures suitable for offline `aql::replay_dispatch`.
    pub fn dispatch_captures(&self) -> &[DispatchDescriptor] {
        &self.dispatch_captures
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
            self.bootstrap();
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
            self.teardown();
        }
        Ok(())
    }

    pub fn is_initialized(&self) -> bool {
        self.refcount > 0
    }

    fn bootstrap(&mut self) {
        self.teardown();
        let generation = self.alloc_generation();
        let index = 0u32;
        let handle = PackedHandle::pack(HandleKind::Agent, generation, index);
        let agent = VirtualAgent::gpu_from_profile(handle, &self.profile);
        self.agents.push(AgentSlot {
            generation,
            live: true,
            agent: Some(agent),
        });
        self.bootstrap_memory(handle);
    }

    fn bootstrap_memory(&mut self, agent_handle: PackedHandle) {
        let specs = [
            (
                HandleKind::Region,
                MemoryViewKind::RegionFineKernarg,
                REGION_SEGMENT_GLOBAL,
                REGION_FLAGS_FINE_KERNARG,
                None,
            ),
            (
                HandleKind::Region,
                MemoryViewKind::RegionCoarse,
                REGION_SEGMENT_GLOBAL,
                REGION_FLAGS_COARSE,
                None,
            ),
            (
                HandleKind::MemoryPool,
                MemoryViewKind::PoolFineHost,
                AMD_SEGMENT_GLOBAL,
                AMD_POOL_FLAGS_FINE_KERNARG,
                Some(AMD_POOL_LOCATION_CPU),
            ),
            (
                HandleKind::MemoryPool,
                MemoryViewKind::PoolCoarseDevice,
                AMD_SEGMENT_GLOBAL,
                AMD_POOL_FLAGS_COARSE,
                Some(AMD_POOL_LOCATION_GPU),
            ),
        ];
        for (kind, view, segment, flags, location) in specs {
            let generation = self.alloc_generation();
            let table = if kind == HandleKind::Region {
                &mut self.regions
            } else {
                &mut self.pools
            };
            let index = table.len() as u32;
            let handle = PackedHandle::pack(kind, generation, index);
            let space = MemorySpace {
                handle,
                kind: view,
                agent_handle,
                segment,
                global_flags: flags,
                size_bytes: SOFTGPU_POOL_BYTES,
                alloc_max_size: SOFTGPU_MAX_ALLOC_BYTES,
                runtime_alloc_allowed: true,
                granule: SOFTGPU_ALLOC_GRANULE,
                alignment: SOFTGPU_ALLOC_ALIGNMENT,
                location,
            };
            table.push(SpaceSlot {
                generation,
                live: true,
                space: Some(space),
            });
        }
    }

    fn alloc_generation(&mut self) -> u32 {
        let generation = self.next_generation;
        self.next_generation = if generation >= 0x00FF_FFFF {
            1
        } else {
            generation + 1
        };
        generation
    }

    fn teardown(&mut self) {
        // Cancel waiters before dropping signal/queue tables.
        for slot in &self.signals {
            if let Some(sig) = slot.signal.as_ref() {
                sig.cancel();
            }
        }
        let mut doorbells = Vec::new();
        while let Some(slot) = self.queues.pop() {
            if let Some(q) = slot.queue {
                doorbells.push(q.doorbell);
                self.destroy_queue_resources(q);
            }
        }
        for doorbell in doorbells {
            let _ = self.signal_destroy_doorbell(doorbell);
        }
        self.signals.clear();
        self.allocator.clear_all();
        self.regions.clear();
        self.pools.clear();
        let live_count = self.agents.iter().filter(|s| s.live).count();
        for _ in 0..live_count {
            let _ = self.alloc_generation();
        }
        self.agents.clear();
        self.dispatch_captures.clear();
    }

    fn destroy_queue_resources(&mut self, queue: SoftGpuQueue) {
        if !queue.packet_buffer.is_null() {
            let layout = Layout::from_size_align(queue.packet_bytes, AQL_PACKET_BYTES)
                .unwrap_or_else(|_| Layout::from_size_align(AQL_PACKET_BYTES, 8).unwrap());
            // SAFETY: buffer allocated by SoftGPU for this queue.
            unsafe { dealloc(queue.packet_buffer, layout) };
        }
        // Doorbell signal is destroyed with the queue ownership table entry separately.
        let _ = queue;
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

    fn resolve_space(
        &self,
        handle: PackedHandle,
        kind: HandleKind,
    ) -> Result<&MemorySpace, RuntimeError> {
        let make_err = || {
            if kind == HandleKind::Region {
                RuntimeError::InvalidRegion
            } else {
                RuntimeError::InvalidPool
            }
        };
        if handle.is_invalid() || handle.kind() != Some(kind) {
            return Err(make_err());
        }
        let table = if kind == HandleKind::Region {
            &self.regions
        } else {
            &self.pools
        };
        let slot = table.get(handle.index() as usize).ok_or_else(make_err)?;
        if !slot.live || slot.generation != handle.generation() {
            return Err(make_err());
        }
        slot.space.as_ref().ok_or_else(make_err)
    }

    fn resolve_signal(
        &self,
        handle: PackedHandle,
    ) -> Result<std::sync::Arc<SoftGpuSignal>, RuntimeError> {
        if handle.is_invalid() || handle.kind() != Some(HandleKind::Signal) {
            return Err(RuntimeError::InvalidSignal);
        }
        let slot = self
            .signals
            .get(handle.index() as usize)
            .ok_or(RuntimeError::InvalidSignal)?;
        if !slot.live || slot.generation != handle.generation() {
            return Err(RuntimeError::InvalidSignal);
        }
        slot.signal
            .as_ref()
            .cloned()
            .ok_or(RuntimeError::InvalidSignal)
    }

    fn resolve_queue_by_abi(&self, abi: *const HsaQueueAbi) -> Result<&SoftGpuQueue, RuntimeError> {
        if abi.is_null() {
            return Err(RuntimeError::InvalidQueue);
        }
        for slot in &self.queues {
            if !slot.live {
                continue;
            }
            if let Some(q) = slot.queue.as_ref() {
                if std::ptr::eq(q.abi_ptr(), abi) {
                    return Ok(q);
                }
            }
        }
        Err(RuntimeError::InvalidQueue)
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
            AgentInfoAttr::VersionMajor => AgentInfoValue::U16(1),
            AgentInfoAttr::VersionMinor => AgentInfoValue::U16(2),
            AgentInfoAttr::QueuesMax => AgentInfoValue::U32(agent.queues_max),
            AgentInfoAttr::QueueMinSize => AgentInfoValue::U32(agent.queue_min_size),
            AgentInfoAttr::QueueMaxSize => AgentInfoValue::U32(agent.queue_max_size),
            AgentInfoAttr::QueueType => AgentInfoValue::U32(agent.queue_type),
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

    pub fn iterate_regions<F>(
        &self,
        agent: PackedHandle,
        mut callback: F,
    ) -> Result<(), RuntimeError>
    where
        F: FnMut(&MemorySpace) -> Result<(), RuntimeError>,
    {
        if !self.is_initialized() {
            return Err(RuntimeError::NotInitialized);
        }
        let _ = self.resolve_agent(agent)?;
        for slot in &self.regions {
            if !slot.live {
                continue;
            }
            let space = slot
                .space
                .as_ref()
                .ok_or(RuntimeError::Internal("live region missing".into()))?;
            if space.agent_handle != agent {
                continue;
            }
            callback(space)?;
        }
        Ok(())
    }

    pub fn iterate_pools<F>(&self, agent: PackedHandle, mut callback: F) -> Result<(), RuntimeError>
    where
        F: FnMut(&MemorySpace) -> Result<(), RuntimeError>,
    {
        if !self.is_initialized() {
            return Err(RuntimeError::NotInitialized);
        }
        let _ = self.resolve_agent(agent)?;
        for slot in &self.pools {
            if !slot.live {
                continue;
            }
            let space = slot
                .space
                .as_ref()
                .ok_or(RuntimeError::Internal("live pool missing".into()))?;
            if space.agent_handle != agent {
                continue;
            }
            callback(space)?;
        }
        Ok(())
    }

    pub fn region_get_info(
        &self,
        handle: PackedHandle,
        attr: RegionInfoAttr,
    ) -> Result<RegionInfoValue, RuntimeError> {
        if !self.is_initialized() {
            return Err(RuntimeError::NotInitialized);
        }
        let space = self.resolve_space(handle, HandleKind::Region)?;
        Ok(match attr {
            RegionInfoAttr::Segment => RegionInfoValue::Segment(space.segment),
            RegionInfoAttr::GlobalFlags => RegionInfoValue::GlobalFlags(space.global_flags),
            RegionInfoAttr::Size => RegionInfoValue::Size(space.size_bytes),
            RegionInfoAttr::AllocMaxSize => RegionInfoValue::AllocMaxSize(space.alloc_max_size),
            RegionInfoAttr::RuntimeAllocAllowed => {
                RegionInfoValue::RuntimeAllocAllowed(space.runtime_alloc_allowed)
            }
            RegionInfoAttr::Granule => RegionInfoValue::Granule(space.granule),
            RegionInfoAttr::Alignment => RegionInfoValue::Alignment(space.alignment),
        })
    }

    pub fn pool_get_info(
        &self,
        handle: PackedHandle,
        attr: PoolInfoAttr,
    ) -> Result<PoolInfoValue, RuntimeError> {
        if !self.is_initialized() {
            return Err(RuntimeError::NotInitialized);
        }
        let space = self.resolve_space(handle, HandleKind::MemoryPool)?;
        Ok(match attr {
            PoolInfoAttr::Segment => PoolInfoValue::Segment(space.segment),
            PoolInfoAttr::GlobalFlags => PoolInfoValue::GlobalFlags(space.global_flags),
            PoolInfoAttr::Size => PoolInfoValue::Size(space.size_bytes),
            PoolInfoAttr::RuntimeAllocAllowed => {
                PoolInfoValue::RuntimeAllocAllowed(space.runtime_alloc_allowed)
            }
            PoolInfoAttr::Granule => PoolInfoValue::Granule(space.granule),
            PoolInfoAttr::Alignment => PoolInfoValue::Alignment(space.alignment),
            PoolInfoAttr::AccessibleByAll => PoolInfoValue::AccessibleByAll(true),
            PoolInfoAttr::AllocMaxSize => PoolInfoValue::AllocMaxSize(space.alloc_max_size),
            PoolInfoAttr::Location => {
                PoolInfoValue::Location(space.location.unwrap_or(AMD_POOL_LOCATION_CPU))
            }
            PoolInfoAttr::RecGranule => PoolInfoValue::RecGranule(space.granule),
        })
    }

    pub fn memory_allocate(
        &mut self,
        space_handle: PackedHandle,
        size: usize,
    ) -> Result<*mut u8, RuntimeError> {
        if !self.is_initialized() {
            return Err(RuntimeError::NotInitialized);
        }
        let kind = space_handle.kind().ok_or(RuntimeError::InvalidArgument)?;
        if kind != HandleKind::Region && kind != HandleKind::MemoryPool {
            return Err(RuntimeError::InvalidArgument);
        }
        let space = self.resolve_space(space_handle, kind)?.clone();
        if !space.runtime_alloc_allowed {
            return Err(RuntimeError::InvalidAllocation);
        }
        let result = self.allocator.allocate(
            space.handle,
            size,
            space.granule,
            space.alignment,
            space.alloc_max_size,
        );
        let (ptr, outcome) = match &result {
            Ok(p) => (*p as u64, "ok"),
            Err(AllocError::InvalidArgument) => (0, "invalid_argument"),
            Err(AllocError::InvalidAllocation) => (0, "invalid_allocation"),
            Err(AllocError::OutOfResources) => (0, "out_of_resources"),
        };
        let seq = self.trace.with_log(TraceLog::next_seq);
        self.trace.with_log(|log| {
            log.record(TraceEvent::MemoryAllocate {
                seq,
                space_handle: space_handle.raw(),
                size,
                ptr,
                outcome: outcome.into(),
            });
        });
        result.map_err(|e| match e {
            AllocError::InvalidArgument => RuntimeError::InvalidArgument,
            AllocError::InvalidAllocation => RuntimeError::InvalidAllocation,
            AllocError::OutOfResources => RuntimeError::OutOfResources,
        })
    }

    pub fn memory_free(&mut self, ptr: *mut u8) -> Result<(), RuntimeError> {
        if !self.is_initialized() {
            return Err(RuntimeError::NotInitialized);
        }
        let result = self.allocator.free(ptr);
        let outcome = match &result {
            Ok(_) => "ok",
            Err(_) => "invalid_argument",
        };
        let seq = self.trace.with_log(TraceLog::next_seq);
        self.trace.with_log(|log| {
            log.record(TraceEvent::MemoryFree {
                seq,
                ptr: ptr as u64,
                outcome: outcome.into(),
            });
        });
        result
            .map(|_| ())
            .map_err(|_| RuntimeError::InvalidArgument)
    }

    pub fn allocation_lookup(
        &self,
        ptr: *const u8,
    ) -> Result<crate::memory::AllocationMeta, RuntimeError> {
        if !self.is_initialized() {
            return Err(RuntimeError::NotInitialized);
        }
        self.allocator
            .lookup(ptr)
            .ok_or(RuntimeError::InvalidArgument)
    }

    /// # Safety
    ///
    /// `dst` and `src` must be valid for `size` bytes; SoftGPU does not
    /// validate OS-mapped pointers beyond SoftGPU allocations.
    pub unsafe fn memory_copy(
        &self,
        dst: *mut u8,
        src: *const u8,
        size: usize,
    ) -> Result<(), RuntimeError> {
        if !self.is_initialized() {
            return Err(RuntimeError::NotInitialized);
        }
        if dst.is_null() || src.is_null() {
            return Err(RuntimeError::InvalidArgument);
        }
        // SAFETY: caller guarantees valid non-overlapping regions of `size`.
        unsafe {
            std::ptr::copy_nonoverlapping(src, dst, size);
        }
        Ok(())
    }

    pub fn agents_allow_access(
        &self,
        agents: &[PackedHandle],
        _ptr: *const u8,
    ) -> Result<(), RuntimeError> {
        if !self.is_initialized() {
            return Err(RuntimeError::NotInitialized);
        }
        for agent in agents {
            let _ = self.resolve_agent(*agent)?;
        }
        // SoftGPU host memory is already CPU-visible; allow is a no-op success.
        Ok(())
    }

    pub fn agent_memory_pool_access(
        &self,
        agent: PackedHandle,
        pool: PackedHandle,
    ) -> Result<u32, RuntimeError> {
        if !self.is_initialized() {
            return Err(RuntimeError::NotInitialized);
        }
        let _ = self.resolve_agent(agent)?;
        let _ = self.resolve_space(pool, HandleKind::MemoryPool)?;
        // HSA_AMD_MEMORY_POOL_ACCESS_ALLOWED_BY_DEFAULT
        Ok(1)
    }

    pub fn signal_create(&mut self, initial: i64) -> Result<PackedHandle, RuntimeError> {
        if !self.is_initialized() {
            return Err(RuntimeError::NotInitialized);
        }
        let generation = self.alloc_generation();
        let index = self.signals.len() as u32;
        let handle = PackedHandle::pack(HandleKind::Signal, generation, index);
        self.signals.push(SignalSlot {
            generation,
            live: true,
            signal: Some(std::sync::Arc::new(SoftGpuSignal::new(handle, initial))),
        });
        let seq = self.trace.with_log(TraceLog::next_seq);
        self.trace.with_log(|log| {
            log.record(TraceEvent::SignalCreate {
                seq,
                signal_handle: handle.raw(),
                initial,
            });
        });
        Ok(handle)
    }

    pub fn signal_destroy(&mut self, handle: PackedHandle) -> Result<(), RuntimeError> {
        if !self.is_initialized() {
            return Err(RuntimeError::NotInitialized);
        }
        if handle.is_invalid() || handle.kind() != Some(HandleKind::Signal) {
            return Err(RuntimeError::InvalidSignal);
        }
        let idx = handle.index() as usize;
        let slot = self
            .signals
            .get_mut(idx)
            .ok_or(RuntimeError::InvalidSignal)?;
        if !slot.live || slot.generation != handle.generation() {
            return Err(RuntimeError::InvalidSignal);
        }
        if slot.signal.as_ref().map(|s| s.is_doorbell).unwrap_or(false) {
            // Doorbell owned by a live queue — reject independent destroy.
            return Err(RuntimeError::InvalidArgument);
        }
        if let Some(sig) = slot.signal.as_ref() {
            sig.cancel();
        }
        slot.live = false;
        slot.signal = None;
        let _ = self.alloc_generation();
        let seq = self.trace.with_log(TraceLog::next_seq);
        self.trace.with_log(|log| {
            log.record(TraceEvent::SignalDestroy {
                seq,
                signal_handle: handle.raw(),
            });
        });
        Ok(())
    }

    pub fn signal_load(&self, handle: PackedHandle) -> Result<i64, RuntimeError> {
        if !self.is_initialized() {
            return Err(RuntimeError::NotInitialized);
        }
        Ok(self.resolve_signal(handle)?.load())
    }

    pub fn signal_store(&mut self, handle: PackedHandle, value: i64) -> Result<(), RuntimeError> {
        if !self.is_initialized() {
            return Err(RuntimeError::NotInitialized);
        }
        let sig = self.resolve_signal(handle)?;
        if sig.is_cancelled() {
            return Err(RuntimeError::InvalidSignal);
        }
        let is_doorbell = sig.is_doorbell;
        let queue_id = sig.queue_id;
        sig.store(value);
        if is_doorbell {
            if let Some(qid) = queue_id {
                let mut pending: Vec<PacketObservation> = Vec::new();
                let mut observe_err: Option<String> = None;
                for slot in &self.queues {
                    if let Some(q) = slot.queue.as_ref() {
                        if q.id == qid {
                            q.note_doorbell_store();
                            let seq = self.trace.with_log(TraceLog::next_seq);
                            self.trace.with_log(|log| {
                                log.record(TraceEvent::QueueDoorbell {
                                    seq,
                                    queue_id: qid,
                                    value,
                                });
                            });
                            match q.observe_packets() {
                                Ok(obs) => pending = obs,
                                Err(err) => observe_err = Some(format!("{err:?}")),
                            }
                            break;
                        }
                    }
                }
                if let Some(detail) = observe_err {
                    let seq = self.trace.with_log(TraceLog::next_seq);
                    self.trace.with_log(|log| {
                        log.record(TraceEvent::PacketValidateFailed {
                            seq,
                            queue_id: qid,
                            detail,
                        });
                    });
                }
                for p in pending {
                    self.process_observed_packet(qid, &p);
                }
            }
        }
        Ok(())
    }

    fn classify_kernarg(&self, addr: u64) -> KernargClass {
        if addr == 0 {
            return KernargClass::Null;
        }
        match self.allocator.lookup(addr as *const u8) {
            Some(meta) => KernargClass::SoftGpu {
                alloc_id: Some(meta.alloc_id),
                addr,
            },
            None => KernargClass::ForeignOpaque { addr },
        }
    }

    fn kernarg_class_label(k: &KernargClass) -> String {
        match k {
            KernargClass::Null => "null".into(),
            KernargClass::SoftGpu { alloc_id, .. } => {
                format!("softgpu:{}", alloc_id.unwrap_or(0))
            }
            KernargClass::ForeignOpaque { .. } => "foreign_opaque".into(),
        }
    }

    /// Store a completion signal value without doorbell observe side effects.
    fn signal_store_plain(&self, handle: PackedHandle, value: i64) -> Result<(), RuntimeError> {
        let sig = self.resolve_signal(handle)?;
        if sig.is_cancelled() || sig.is_doorbell {
            return Err(RuntimeError::InvalidSignal);
        }
        sig.store(value);
        Ok(())
    }

    fn process_observed_packet(&mut self, queue_id: u64, obs: &PacketObservation) {
        let seq = self.trace.with_log(TraceLog::next_seq);
        self.trace.with_log(|log| {
            log.record(TraceEvent::PacketObserved {
                seq,
                queue_id,
                packet_index: obs.packet_index,
                packet_type: obs.packet_type,
            });
        });

        let parsed = parse_supported_packet(&obs.bytes, obs.packet_index, |addr| {
            self.classify_kernarg(addr)
        });

        match parsed {
            Ok(desc) => {
                let seq = self.trace.with_log(TraceLog::next_seq);
                self.trace.with_log(|log| {
                    log.record(TraceEvent::DispatchValidated {
                        seq,
                        queue_id,
                        packet_index: desc.packet_index,
                        packet_type: desc.packet_type.as_u16(),
                        dimensions: desc.dimensions,
                        workgroup_size: desc.workgroup_size,
                        grid_size: desc.grid_size,
                        private_segment_size: desc.private_segment_size,
                        group_segment_size: desc.group_segment_size,
                        kernel_object: desc.kernel_object,
                        kernarg_class: Self::kernarg_class_label(&desc.kernarg),
                        completion_signal: desc.completion_signal,
                    });
                });
                self.try_dispatch_or_diagnose(queue_id, &desc);
                self.dispatch_captures.push(desc);
            }
            Err(err) => {
                let seq = self.trace.with_log(TraceLog::next_seq);
                self.trace.with_log(|log| {
                    log.record(TraceEvent::DispatchRejected {
                        seq,
                        queue_id,
                        packet_index: obs.packet_index,
                        packet_type: obs.packet_type,
                        detail: format!("{err:?}"),
                        contract: DIAGNOSTIC_REJECTED.into(),
                    });
                });
                self.apply_diagnostic_reject(queue_id, obs.packet_index);
            }
        }
    }

    fn try_dispatch_or_diagnose(&mut self, queue_id: u64, desc: &DispatchDescriptor) {
        if desc.packet_type != crate::aql::PacketType::KernelDispatch {
            self.apply_diagnostic_complete(queue_id, desc);
            return;
        }
        let Some(kernel) = self.isa_kernels.get(&desc.kernel_object).cloned() else {
            self.apply_diagnostic_complete(queue_id, desc);
            return;
        };
        let KernargClass::SoftGpu { addr, .. } = desc.kernarg else {
            self.apply_diagnostic_complete(queue_id, desc);
            return;
        };
        let grid_x = desc.grid_size[0];
        if grid_x == 0 {
            self.apply_diagnostic_complete(queue_id, desc);
            return;
        }

        struct AllocMem<'a>(&'a mut SoftGpuAllocator);
        impl IsaMemory for AllocMem<'_> {
            fn load_u32(&self, addr: u64) -> softgpu_amd_isa::Result<u32> {
                let mut buf = [0u8; 4];
                self.0.read_bytes_at(addr, &mut buf).map_err(|_| {
                    softgpu_amd_isa::IsaError::new(
                        softgpu_amd_isa::IsaErrorKind::Trap,
                        format!("SoftGPU alloc load_u32 OOB/unknown addr=0x{addr:x}"),
                    )
                })?;
                Ok(u32::from_le_bytes(buf))
            }
            fn store_u32(&mut self, addr: u64, value: u32) -> softgpu_amd_isa::Result<()> {
                self.0
                    .write_bytes_at(addr, &value.to_le_bytes())
                    .map_err(|_| {
                        softgpu_amd_isa::IsaError::new(
                            softgpu_amd_isa::IsaErrorKind::Trap,
                            format!("SoftGPU alloc store_u32 OOB/unknown addr=0x{addr:x}"),
                        )
                    })
            }
            fn load_u64(&self, addr: u64) -> softgpu_amd_isa::Result<u64> {
                let mut buf = [0u8; 8];
                self.0.read_bytes_at(addr, &mut buf).map_err(|_| {
                    softgpu_amd_isa::IsaError::new(
                        softgpu_amd_isa::IsaErrorKind::Trap,
                        format!("SoftGPU alloc load_u64 OOB/unknown addr=0x{addr:x}"),
                    )
                })?;
                Ok(u64::from_le_bytes(buf))
            }
            fn store_u64(&mut self, addr: u64, value: u64) -> softgpu_amd_isa::Result<()> {
                self.0
                    .write_bytes_at(addr, &value.to_le_bytes())
                    .map_err(|_| {
                        softgpu_amd_isa::IsaError::new(
                            softgpu_amd_isa::IsaErrorKind::Trap,
                            format!("SoftGPU alloc store_u64 OOB/unknown addr=0x{addr:x}"),
                        )
                    })
            }
        }

        let mut mem = AllocMem(&mut self.allocator);
        match run_code_1d(&kernel.code, &mut mem, addr, grid_x, WaveSize::Wave32) {
            Ok(_) => self.apply_kernel_success(queue_id, desc, &kernel.name),
            Err(e) => {
                let seq = self.trace.with_log(TraceLog::next_seq);
                self.trace.with_log(|log| {
                    log.record(TraceEvent::DispatchRejected {
                        seq,
                        queue_id,
                        packet_index: desc.packet_index,
                        packet_type: desc.packet_type.as_u16(),
                        detail: e.to_string(),
                        contract: DIAGNOSTIC_REJECTED.into(),
                    });
                });
                self.apply_diagnostic_reject(queue_id, desc.packet_index);
            }
        }
    }

    fn apply_kernel_success(
        &mut self,
        queue_id: u64,
        desc: &DispatchDescriptor,
        kernel_name: &str,
    ) {
        if desc.completion_signal != 0 {
            let handle = PackedHandle::from_raw(desc.completion_signal);
            let _ = self.signal_store_plain(handle, 0);
        }
        self.advance_packet_processor(queue_id, desc.packet_index);
        let seq = self.trace.with_log(TraceLog::next_seq);
        self.trace.with_log(|log| {
            log.record(TraceEvent::DiagnosticComplete {
                seq,
                queue_id,
                packet_index: desc.packet_index,
                completion_signal: desc.completion_signal,
                contract: KERNEL_SUCCESS.into(),
                note: format!("softgpu_isa_kernel:{kernel_name}"),
            });
        });
    }
    fn apply_diagnostic_complete(&mut self, queue_id: u64, desc: &DispatchDescriptor) {
        // HSA completion convention: producer waits for signal == 0.
        // SoftGPU stores 0 only as the experimental no-execution contract.
        if desc.completion_signal != 0 {
            let handle = PackedHandle::from_raw(desc.completion_signal);
            let _ = self.signal_store_plain(handle, 0);
        }
        self.advance_packet_processor(queue_id, desc.packet_index);
        let seq = self.trace.with_log(TraceLog::next_seq);
        self.trace.with_log(|log| {
            log.record(TraceEvent::DiagnosticComplete {
                seq,
                queue_id,
                packet_index: desc.packet_index,
                completion_signal: desc.completion_signal,
                contract: DIAGNOSTIC_COMPLETE_NO_EXECUTION.into(),
                note: "not_kernel_success".into(),
            });
        });
    }

    fn apply_diagnostic_reject(&mut self, queue_id: u64, packet_index: u64) {
        // Reject does not store completion as success; still advance protocol.
        self.advance_packet_processor(queue_id, packet_index);
    }

    fn advance_packet_processor(&mut self, queue_id: u64, packet_index: u64) {
        for slot in &self.queues {
            if let Some(q) = slot.queue.as_ref() {
                if q.id == queue_id {
                    q.invalidate_packet_slot(packet_index);
                    let next = packet_index.saturating_add(1);
                    // Monotonic processor progress; never move read_index backwards.
                    if next > q.read_index() {
                        q.store_read_index(next);
                    }
                    break;
                }
            }
        }
    }

    /// Clone the signal Arc for waiting **without** holding the runtime lock.
    pub fn signal_arc(
        &self,
        handle: PackedHandle,
    ) -> Result<std::sync::Arc<SoftGpuSignal>, RuntimeError> {
        if !self.is_initialized() {
            return Err(RuntimeError::NotInitialized);
        }
        self.resolve_signal(handle)
    }

    pub fn signal_wait(
        &self,
        handle: PackedHandle,
        condition: u32,
        compare: i64,
        timeout_hint_ns: u64,
        wait_state_hint: u32,
    ) -> Result<SignalWaitOutcome, RuntimeError> {
        let sig = self.signal_arc(handle)?;
        let cond = SignalCondition::from_u32(condition).ok_or(RuntimeError::InvalidArgument)?;
        Ok(sig.wait(cond, compare, timeout_hint_ns, wait_state_hint))
    }

    /// SoftGPU packet observation without doorbell (test / tooling).
    pub fn queue_observe(
        &self,
        abi: *const HsaQueueAbi,
    ) -> Result<Vec<crate::queue::PacketObservation>, RuntimeError> {
        if !self.is_initialized() {
            return Err(RuntimeError::NotInitialized);
        }
        let q = self.resolve_queue_by_abi(abi)?;
        q.observe_packets()
            .map_err(|e| RuntimeError::Internal(format!("{e:?}")))
    }

    pub fn queue_create(
        &mut self,
        agent: PackedHandle,
        size: u32,
        type_: u32,
    ) -> Result<*mut HsaQueueAbi, RuntimeError> {
        if !self.is_initialized() {
            return Err(RuntimeError::NotInitialized);
        }
        let agent_ref = self.resolve_agent(agent)?;
        if size == 0 || !is_power_of_two(size) {
            return Err(RuntimeError::InvalidArgument);
        }
        if size < agent_ref.queue_min_size || size > agent_ref.queue_max_size {
            return Err(RuntimeError::InvalidArgument);
        }
        if type_ != QUEUE_TYPE_MULTI && type_ != 1 {
            // SoftGPU supports single and multi; cooperative unsupported.
            return Err(RuntimeError::InvalidQueueCreation);
        }
        let live_queues = self.queues.iter().filter(|s| s.live).count() as u32;
        if live_queues >= SOFTGPU_QUEUES_MAX {
            return Err(RuntimeError::OutOfResources);
        }

        let queue_id = self.next_queue_id;
        self.next_queue_id = self.next_queue_id.saturating_add(1);

        let generation = self.alloc_generation();
        let index = self.signals.len() as u32;
        let doorbell = PackedHandle::pack(HandleKind::Signal, generation, index);
        self.signals.push(SignalSlot {
            generation,
            live: true,
            signal: Some(std::sync::Arc::new(SoftGpuSignal::new_doorbell(
                doorbell, 0, queue_id,
            ))),
        });
        let seq = self.trace.with_log(TraceLog::next_seq);
        self.trace.with_log(|log| {
            log.record(TraceEvent::SignalCreate {
                seq,
                signal_handle: doorbell.raw(),
                initial: 0,
            });
        });

        let packet_bytes = (size as usize).saturating_mul(AQL_PACKET_BYTES);
        let layout = Layout::from_size_align(packet_bytes, AQL_PACKET_BYTES)
            .map_err(|_| RuntimeError::OutOfResources)?;
        // SAFETY: layout validated.
        let packet_ptr = unsafe { alloc_zeroed(layout) };
        if packet_ptr.is_null() {
            let _ = self.signal_destroy_doorbell(doorbell);
            return Err(RuntimeError::OutOfResources);
        }
        // SAFETY: freshly allocated buffer of packet_bytes.
        unsafe {
            init_packet_buffer(std::slice::from_raw_parts_mut(packet_ptr, packet_bytes));
        }

        let generation = self.alloc_generation();
        let index = self.queues.len() as u32;
        let handle = PackedHandle::pack(HandleKind::Queue, generation, index);

        let abi = Box::new(HsaQueueAbi {
            type_,
            features: QUEUE_FEATURE_KERNEL_DISPATCH,
            base_address: packet_ptr,
            doorbell_signal: doorbell.raw(),
            size,
            reserved1: 0,
            id: queue_id,
        });

        let queue = SoftGpuQueue::new(
            handle,
            agent,
            queue_id,
            abi,
            packet_ptr,
            packet_bytes,
            doorbell,
        );
        let abi_ptr = queue.abi_ptr();
        self.queues.push(QueueSlot {
            generation,
            live: true,
            queue: Some(queue),
        });

        let seq = self.trace.with_log(TraceLog::next_seq);
        self.trace.with_log(|log| {
            log.record(TraceEvent::QueueCreate {
                seq,
                queue_id,
                size,
                agent_handle: agent.raw(),
            });
        });
        Ok(abi_ptr)
    }

    fn signal_destroy_doorbell(&mut self, handle: PackedHandle) -> Result<(), RuntimeError> {
        let idx = handle.index() as usize;
        let slot = self
            .signals
            .get_mut(idx)
            .ok_or(RuntimeError::InvalidSignal)?;
        if !slot.live || slot.generation != handle.generation() {
            return Err(RuntimeError::InvalidSignal);
        }
        if let Some(sig) = slot.signal.as_ref() {
            sig.cancel();
        }
        slot.live = false;
        slot.signal = None;
        let _ = self.alloc_generation();
        Ok(())
    }

    pub fn queue_destroy(&mut self, abi: *mut HsaQueueAbi) -> Result<(), RuntimeError> {
        if !self.is_initialized() {
            return Err(RuntimeError::NotInitialized);
        }
        if abi.is_null() {
            return Err(RuntimeError::InvalidQueue);
        }
        let mut found = None;
        for (i, slot) in self.queues.iter().enumerate() {
            if !slot.live {
                continue;
            }
            if let Some(q) = slot.queue.as_ref() {
                if q.abi_ptr() == abi {
                    found = Some(i);
                    break;
                }
            }
        }
        let idx = found.ok_or(RuntimeError::InvalidQueue)?;
        let slot = &mut self.queues[idx];
        let Some(mut queue) = slot.queue.take() else {
            return Err(RuntimeError::InvalidQueue);
        };
        slot.live = false;
        let queue_id = queue.id;
        let doorbell = queue.doorbell;
        if !queue.packet_buffer.is_null() {
            let layout = Layout::from_size_align(queue.packet_bytes, AQL_PACKET_BYTES)
                .unwrap_or_else(|_| Layout::from_size_align(AQL_PACKET_BYTES, 8).unwrap());
            unsafe { dealloc(queue.packet_buffer, layout) };
            queue.packet_buffer = std::ptr::null_mut();
        }
        let _ = self.signal_destroy_doorbell(doorbell);
        let _ = self.alloc_generation();
        let seq = self.trace.with_log(TraceLog::next_seq);
        self.trace.with_log(|log| {
            log.record(TraceEvent::QueueDestroy { seq, queue_id });
        });
        Ok(())
    }

    pub fn queue_load_write_index(&self, abi: *const HsaQueueAbi) -> Result<u64, RuntimeError> {
        if !self.is_initialized() {
            return Err(RuntimeError::NotInitialized);
        }
        Ok(self.resolve_queue_by_abi(abi)?.write_index())
    }

    pub fn queue_load_read_index(&self, abi: *const HsaQueueAbi) -> Result<u64, RuntimeError> {
        if !self.is_initialized() {
            return Err(RuntimeError::NotInitialized);
        }
        Ok(self.resolve_queue_by_abi(abi)?.read_index())
    }

    pub fn queue_store_write_index(
        &self,
        abi: *const HsaQueueAbi,
        value: u64,
    ) -> Result<(), RuntimeError> {
        if !self.is_initialized() {
            return Err(RuntimeError::NotInitialized);
        }
        let q = self.resolve_queue_by_abi(abi)?;
        q.store_write_index(value);
        let seq = self.trace.with_log(TraceLog::next_seq);
        self.trace.with_log(|log| {
            log.record(TraceEvent::QueueIndexStore {
                seq,
                queue_id: q.id,
                which: "write".into(),
                value,
            });
        });
        Ok(())
    }

    pub fn queue_store_read_index(
        &self,
        abi: *const HsaQueueAbi,
        value: u64,
    ) -> Result<(), RuntimeError> {
        if !self.is_initialized() {
            return Err(RuntimeError::NotInitialized);
        }
        let q = self.resolve_queue_by_abi(abi)?;
        q.store_read_index(value);
        let seq = self.trace.with_log(TraceLog::next_seq);
        self.trace.with_log(|log| {
            log.record(TraceEvent::QueueIndexStore {
                seq,
                queue_id: q.id,
                which: "read".into(),
                value,
            });
        });
        Ok(())
    }

    pub fn queue_doorbell_count(&self, abi: *const HsaQueueAbi) -> Result<u64, RuntimeError> {
        if !self.is_initialized() {
            return Err(RuntimeError::NotInitialized);
        }
        Ok(self.resolve_queue_by_abi(abi)?.doorbell_store_count())
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
    U32(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionInfoAttr {
    Segment,
    GlobalFlags,
    Size,
    AllocMaxSize,
    RuntimeAllocAllowed,
    Granule,
    Alignment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolInfoAttr {
    Segment,
    GlobalFlags,
    Size,
    RuntimeAllocAllowed,
    Granule,
    Alignment,
    AccessibleByAll,
    AllocMaxSize,
    Location,
    RecGranule,
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

const DEFAULT_GENERIC_PROFILE: &str = include_str!("../embedded/softgpu-generic-v0.json");

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
    use crate::agent::AGENT_FEATURE_KERNEL_DISPATCH;
    use crate::profile::{ProfileField, ProfileIdentity, ResourceLimits};
    use crate::queue::{
        QUEUE_TYPE_MULTI, SOFTGPU_QUEUES_MAX, SOFTGPU_QUEUE_MAX_SIZE, SOFTGPU_QUEUE_MIN_SIZE,
    };
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
            assert_eq!(agent.feature_mask, AGENT_FEATURE_KERNEL_DISPATCH);
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
    fn path_c_regions_and_pools_allocate() {
        let mut rt = Runtime::new(test_profile()).unwrap();
        rt.init().unwrap();
        let agent = rt.agents[0].agent.as_ref().unwrap().handle;
        let mut region = None;
        rt.iterate_regions(agent, |space| {
            if space.kind == MemoryViewKind::RegionFineKernarg {
                region = Some(space.handle);
            }
            Ok(())
        })
        .unwrap();
        let region = region.expect("fine region");
        let ptr = rt.memory_allocate(region, 128).unwrap();
        assert!(!ptr.is_null());
        rt.memory_free(ptr).unwrap();

        let mut pool = None;
        rt.iterate_pools(agent, |space| {
            if space.kind == MemoryViewKind::PoolFineHost {
                pool = Some(space.handle);
            }
            Ok(())
        })
        .unwrap();
        let pool = pool.expect("fine pool");
        let ptr = rt.memory_allocate(pool, 256).unwrap();
        rt.memory_free(ptr).unwrap();
        rt.shut_down().unwrap();
    }

    #[test]
    fn signal_and_queue_observe_doorbell() {
        let mut rt = Runtime::new(test_profile()).unwrap();
        rt.init().unwrap();
        let agent = rt.agents[0].agent.as_ref().unwrap().handle;
        let q = rt
            .queue_create(agent, SOFTGPU_QUEUE_MIN_SIZE, QUEUE_TYPE_MULTI)
            .unwrap();
        assert!(!q.is_null());
        let doorbell = unsafe { (*q).doorbell_signal };
        rt.signal_store(PackedHandle::from_raw(doorbell), 1)
            .unwrap();
        assert_eq!(rt.queue_doorbell_count(q).unwrap(), 1);
        rt.queue_store_write_index(q, 1).unwrap();
        assert_eq!(rt.queue_load_write_index(q).unwrap(), 1);
        rt.queue_destroy(q).unwrap();
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

    #[test]
    fn softgpu_queue_limits_are_software_defaults() {
        assert!(SOFTGPU_QUEUE_MIN_SIZE.is_power_of_two());
        assert!(SOFTGPU_QUEUE_MAX_SIZE.is_power_of_two());
        const { assert!(SOFTGPU_QUEUES_MAX > 0) };
    }
}
