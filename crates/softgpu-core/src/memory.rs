//! SoftGPU software memory: one host allocator behind regions and AMD pools.
//!
//! Provenance: `softgpu-software`. Allocations are process CPU memory, not
//! R9700 VRAM. Sizes are SoftGPU defaults, not invented hardware limits.
//!
//! # Ownership invariants
//!
//! - Every live SoftGPU allocation is keyed by base pointer in `live`.
//! - `Allocation` is `Send` because SoftGPU never shares a mutable allocation
//!   across threads without going through the process-global runtime mutex;
//!   the raw pointer is exclusive to SoftGPU until `free`.
//! - Free of an unknown pointer fails closed (no silent host `free`).

use crate::handle::PackedHandle;
use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::collections::HashMap;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU64, Ordering};

/// SoftGPU software pool capacity (host bytes). Not hardware VRAM.
pub const SOFTGPU_POOL_BYTES: usize = 256 * 1024 * 1024;
/// SoftGPU max single allocation. Not an R9700 claim.
pub const SOFTGPU_MAX_ALLOC_BYTES: usize = 64 * 1024 * 1024;
pub const SOFTGPU_ALLOC_GRANULE: usize = 4096;
pub const SOFTGPU_ALLOC_ALIGNMENT: usize = 256;

/// HSA `HSA_REGION_SEGMENT_GLOBAL`.
pub const REGION_SEGMENT_GLOBAL: u32 = 0;
/// `HSA_REGION_GLOBAL_FLAG_KERNARG | FINE_GRAINED`.
pub const REGION_FLAGS_FINE_KERNARG: u32 = 1 | 2;
/// `HSA_REGION_GLOBAL_FLAG_COARSE_GRAINED`.
pub const REGION_FLAGS_COARSE: u32 = 4;

/// AMD `HSA_AMD_SEGMENT_GLOBAL`.
pub const AMD_SEGMENT_GLOBAL: u32 = 0;
/// `KERNARG_INIT | FINE_GRAINED`.
pub const AMD_POOL_FLAGS_FINE_KERNARG: u32 = 1 | 2;
/// `COARSE_GRAINED`.
pub const AMD_POOL_FLAGS_COARSE: u32 = 4;
pub const AMD_POOL_LOCATION_CPU: u32 = 0;
pub const AMD_POOL_LOCATION_GPU: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryViewKind {
    RegionFineKernarg,
    RegionCoarse,
    PoolFineHost,
    PoolCoarseDevice,
}

#[derive(Debug, Clone)]
pub struct MemorySpace {
    pub handle: PackedHandle,
    pub kind: MemoryViewKind,
    pub agent_handle: PackedHandle,
    pub segment: u32,
    pub global_flags: u32,
    pub size_bytes: usize,
    pub alloc_max_size: usize,
    pub runtime_alloc_allowed: bool,
    pub granule: usize,
    pub alignment: usize,
    pub location: Option<u32>,
}

impl MemorySpace {
    pub fn is_pool(&self) -> bool {
        matches!(
            self.kind,
            MemoryViewKind::PoolFineHost | MemoryViewKind::PoolCoarseDevice
        )
    }

    pub fn is_region(&self) -> bool {
        matches!(
            self.kind,
            MemoryViewKind::RegionFineKernarg | MemoryViewKind::RegionCoarse
        )
    }
}

/// SoftGPU permission / lifetime metadata for a tracked allocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocationMeta {
    pub ptr: usize,
    pub size: usize,
    pub space: PackedHandle,
    pub alignment: usize,
    pub alloc_id: u64,
    pub host_readable: bool,
    pub host_writable: bool,
}

#[derive(Debug)]
struct Allocation {
    ptr: NonNull<u8>,
    layout: Layout,
    size: usize,
    space: PackedHandle,
    alloc_id: u64,
    host_readable: bool,
    host_writable: bool,
}

// SAFETY: SoftGPU owns the allocation exclusively after allocate; the process
// runtime mutex serializes allocate/free. Pointers are never handed to foreign
// code with overlapping SoftGPU mutation.
unsafe impl Send for Allocation {}

/// Process-heap SoftGPU allocator with tracked pointers.
#[derive(Debug)]
pub struct SoftGpuAllocator {
    live: HashMap<usize, Allocation>,
    bytes_in_use: usize,
    capacity: usize,
    next_alloc_id: AtomicU64,
}

impl Default for SoftGpuAllocator {
    fn default() -> Self {
        Self::new(SOFTGPU_POOL_BYTES)
    }
}

impl SoftGpuAllocator {
    pub fn new(capacity: usize) -> Self {
        Self {
            live: HashMap::new(),
            bytes_in_use: 0,
            capacity,
            next_alloc_id: AtomicU64::new(1),
        }
    }

    pub fn bytes_in_use(&self) -> usize {
        self.bytes_in_use
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn live_count(&self) -> usize {
        self.live.len()
    }

    pub fn lookup(&self, ptr: *const u8) -> Option<AllocationMeta> {
        if ptr.is_null() {
            return None;
        }
        self.live.get(&(ptr as usize)).map(|a| AllocationMeta {
            ptr: a.ptr.as_ptr() as usize,
            size: a.size,
            space: a.space,
            alignment: a.layout.align(),
            alloc_id: a.alloc_id,
            host_readable: a.host_readable,
            host_writable: a.host_writable,
        })
    }

    /// Find SoftGPU allocation covering `addr` (for ISA global memory).
    pub fn find_covering(&self, addr: u64) -> Option<AllocationMeta> {
        let a = addr as usize;
        for alloc in self.live.values() {
            let base = alloc.ptr.as_ptr() as usize;
            if a >= base && a < base + alloc.size {
                return Some(AllocationMeta {
                    ptr: base,
                    size: alloc.size,
                    space: alloc.space,
                    alignment: alloc.layout.align(),
                    alloc_id: alloc.alloc_id,
                    host_readable: alloc.host_readable,
                    host_writable: alloc.host_writable,
                });
            }
        }
        None
    }

    /// Read bytes from a SoftGPU-tracked allocation (fail closed if OOB/unknown).
    pub fn read_bytes_at(&self, addr: u64, out: &mut [u8]) -> Result<(), AllocError> {
        let meta = self
            .find_covering(addr)
            .ok_or(AllocError::InvalidArgument)?;
        let off = (addr as usize) - meta.ptr;
        if off + out.len() > meta.size || !meta.host_readable {
            return Err(AllocError::InvalidArgument);
        }
        let base = meta.ptr as *const u8;
        // SAFETY: allocation is live SoftGPU memory; range checked above.
        unsafe {
            std::ptr::copy_nonoverlapping(base.add(off), out.as_mut_ptr(), out.len());
        }
        Ok(())
    }

    /// Write bytes into a SoftGPU-tracked allocation.
    pub fn write_bytes_at(&mut self, addr: u64, data: &[u8]) -> Result<(), AllocError> {
        let meta = self
            .find_covering(addr)
            .ok_or(AllocError::InvalidArgument)?;
        let off = (addr as usize) - meta.ptr;
        if off + data.len() > meta.size || !meta.host_writable {
            return Err(AllocError::InvalidArgument);
        }
        let base = meta.ptr as *mut u8;
        // SAFETY: allocation is live SoftGPU memory; range checked above.
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), base.add(off), data.len());
        }
        Ok(())
    }

    pub fn allocate(
        &mut self,
        space: PackedHandle,
        size: usize,
        granule: usize,
        alignment: usize,
        max_alloc: usize,
    ) -> Result<*mut u8, AllocError> {
        if size == 0 {
            return Err(AllocError::InvalidArgument);
        }
        if size > max_alloc {
            return Err(AllocError::InvalidAllocation);
        }
        let align = alignment.max(1).next_power_of_two();
        if !align.is_power_of_two() {
            return Err(AllocError::InvalidArgument);
        }
        let rounded = round_up(size, granule.max(1));
        if self.bytes_in_use.saturating_add(rounded) > self.capacity {
            return Err(AllocError::OutOfResources);
        }
        let layout =
            Layout::from_size_align(rounded, align).map_err(|_| AllocError::OutOfResources)?;
        // SAFETY: layout is non-zero size with power-of-two alignment.
        let ptr = unsafe { alloc_zeroed(layout) };
        let Some(nn) = NonNull::new(ptr) else {
            return Err(AllocError::OutOfResources);
        };
        if (nn.as_ptr() as usize) % align != 0 {
            // SAFETY: free the failed-alignment allocation.
            unsafe { dealloc(nn.as_ptr(), layout) };
            return Err(AllocError::OutOfResources);
        }
        let alloc_id = self.next_alloc_id.fetch_add(1, Ordering::SeqCst);
        self.live.insert(
            nn.as_ptr() as usize,
            Allocation {
                ptr: nn,
                layout,
                size: rounded,
                space,
                alloc_id,
                host_readable: true,
                host_writable: true,
            },
        );
        self.bytes_in_use = self.bytes_in_use.saturating_add(rounded);
        Ok(nn.as_ptr())
    }

    pub fn free(&mut self, ptr: *mut u8) -> Result<AllocationMeta, AllocError> {
        if ptr.is_null() {
            return Err(AllocError::InvalidArgument);
        }
        let Some(alloc) = self.live.remove(&(ptr as usize)) else {
            return Err(AllocError::InvalidArgument);
        };
        let meta = AllocationMeta {
            ptr: alloc.ptr.as_ptr() as usize,
            size: alloc.size,
            space: alloc.space,
            alignment: alloc.layout.align(),
            alloc_id: alloc.alloc_id,
            host_readable: alloc.host_readable,
            host_writable: alloc.host_writable,
        };
        self.bytes_in_use = self.bytes_in_use.saturating_sub(alloc.size);
        // SAFETY: pointer and layout came from this allocator.
        unsafe { dealloc(alloc.ptr.as_ptr(), alloc.layout) };
        Ok(meta)
    }

    pub fn contains(&self, ptr: *const u8) -> bool {
        !ptr.is_null() && self.live.contains_key(&(ptr as usize))
    }

    pub fn clear_all(&mut self) {
        for (_, alloc) in self.live.drain() {
            // SAFETY: owned by this allocator.
            unsafe { dealloc(alloc.ptr.as_ptr(), alloc.layout) };
        }
        self.bytes_in_use = 0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocError {
    InvalidArgument,
    InvalidAllocation,
    OutOfResources,
}

fn round_up(value: usize, granule: usize) -> usize {
    if granule <= 1 {
        return value;
    }
    value.div_ceil(granule).saturating_mul(granule)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegionInfoValue {
    Segment(u32),
    GlobalFlags(u32),
    Size(usize),
    AllocMaxSize(usize),
    RuntimeAllocAllowed(bool),
    Granule(usize),
    Alignment(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolInfoValue {
    Segment(u32),
    GlobalFlags(u32),
    Size(usize),
    RuntimeAllocAllowed(bool),
    Granule(usize),
    Alignment(usize),
    AccessibleByAll(bool),
    AllocMaxSize(usize),
    Location(u32),
    RecGranule(usize),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handle::HandleKind;

    #[test]
    fn allocate_and_free_round_trip() {
        let mut alloc = SoftGpuAllocator::new(SOFTGPU_POOL_BYTES);
        let space = PackedHandle::pack(HandleKind::Region, 1, 0);
        let ptr = alloc
            .allocate(
                space,
                100,
                SOFTGPU_ALLOC_GRANULE,
                SOFTGPU_ALLOC_ALIGNMENT,
                SOFTGPU_MAX_ALLOC_BYTES,
            )
            .unwrap();
        assert!(!ptr.is_null());
        let meta = alloc.lookup(ptr).unwrap();
        assert!(meta.host_readable && meta.host_writable);
        assert_eq!(meta.space, space);
        assert!(alloc.contains(ptr));
        assert!(alloc.bytes_in_use() >= 100);
        alloc.free(ptr).unwrap();
        assert_eq!(alloc.bytes_in_use(), 0);
        assert!(alloc.lookup(ptr).is_none());
    }

    #[test]
    fn rejects_oversize_alloc() {
        let mut alloc = SoftGpuAllocator::new(SOFTGPU_POOL_BYTES);
        let space = PackedHandle::pack(HandleKind::MemoryPool, 1, 0);
        let err = alloc
            .allocate(
                space,
                SOFTGPU_MAX_ALLOC_BYTES + 1,
                SOFTGPU_ALLOC_GRANULE,
                SOFTGPU_ALLOC_ALIGNMENT,
                SOFTGPU_MAX_ALLOC_BYTES,
            )
            .unwrap_err();
        assert_eq!(err, AllocError::InvalidAllocation);
    }

    #[test]
    fn exhaustion_fails_closed() {
        let mut alloc = SoftGpuAllocator::new(SOFTGPU_ALLOC_GRANULE);
        let space = PackedHandle::pack(HandleKind::Region, 1, 0);
        let ptr = alloc
            .allocate(
                space,
                SOFTGPU_ALLOC_GRANULE,
                SOFTGPU_ALLOC_GRANULE,
                SOFTGPU_ALLOC_ALIGNMENT,
                SOFTGPU_MAX_ALLOC_BYTES,
            )
            .unwrap();
        let err = alloc
            .allocate(
                space,
                SOFTGPU_ALLOC_GRANULE,
                SOFTGPU_ALLOC_GRANULE,
                SOFTGPU_ALLOC_ALIGNMENT,
                SOFTGPU_MAX_ALLOC_BYTES,
            )
            .unwrap_err();
        assert_eq!(err, AllocError::OutOfResources);
        alloc.free(ptr).unwrap();
    }

    #[test]
    fn free_unknown_pointer_fails() {
        let mut alloc = SoftGpuAllocator::new(SOFTGPU_POOL_BYTES);
        let err = alloc.free(std::ptr::dangling_mut::<u8>()).unwrap_err();
        assert_eq!(err, AllocError::InvalidArgument);
    }
}
