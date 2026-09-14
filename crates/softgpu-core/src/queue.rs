//! SoftGPU user-mode queues: create / destroy / indexes / AQL observe.
//!
//! Phase 4: SoftGPU validates kernel-dispatch packets, records normalized
//! descriptors, and may apply the experimental **diagnostic completion**
//! contract (completion signal store) **without** executing kernel semantics.
//! Advancing HSA `read_index` after diagnostic complete/reject is packet-
//! processor protocol progress, not kernel success.

use crate::handle::PackedHandle;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};

/// AQL packet size in bytes (HSA kernel dispatch packet).
pub const AQL_PACKET_BYTES: usize = 64;
/// `HSA_PACKET_TYPE_INVALID` in the packet header type field.
pub const PACKET_TYPE_INVALID: u16 = 1;
/// `HSA_PACKET_TYPE_KERNEL_DISPATCH`.
pub const PACKET_TYPE_KERNEL_DISPATCH: u16 = 2;
/// SoftGPU software queue size limits (not hardware).
pub const SOFTGPU_QUEUE_MIN_SIZE: u32 = 64;
pub const SOFTGPU_QUEUE_MAX_SIZE: u32 = 16_384;
pub const SOFTGPU_QUEUES_MAX: u32 = 64;
/// `HSA_QUEUE_TYPE_MULTI`.
pub const QUEUE_TYPE_MULTI: u32 = 0;
/// `HSA_QUEUE_TYPE_SINGLE`.
pub const QUEUE_TYPE_SINGLE: u32 = 1;
/// `HSA_QUEUE_FEATURE_KERNEL_DISPATCH`.
pub const QUEUE_FEATURE_KERNEL_DISPATCH: u32 = 1 << 0;

/// Host-visible `hsa_queue_t` layout under `HSA_LARGE_MODEL` (64-bit).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct HsaQueueAbi {
    pub type_: u32,
    pub features: u32,
    pub base_address: *mut u8,
    pub doorbell_signal: u64,
    pub size: u32,
    pub reserved1: u32,
    pub id: u64,
}

// SAFETY: queue ABI pointers are owned by SoftGPU and only freed on destroy;
// create/destroy are serialized by the process runtime mutex.
unsafe impl Send for HsaQueueAbi {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketObservation {
    pub packet_index: u64,
    pub packet_type: u16,
    pub bytes: [u8; AQL_PACKET_BYTES],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PacketValidateError {
    StillInvalid { packet_index: u64 },
    UnsupportedType { packet_index: u64, packet_type: u16 },
    AlreadyObserved { packet_index: u64 },
    BufferFault { packet_index: u64 },
}

#[derive(Debug)]
pub struct SoftGpuQueue {
    pub handle: PackedHandle,
    pub agent_handle: PackedHandle,
    pub id: u64,
    pub abi: Box<HsaQueueAbi>,
    pub packet_buffer: *mut u8,
    pub packet_bytes: usize,
    pub doorbell: PackedHandle,
    pub write_index: AtomicU64,
    pub read_index: AtomicU64,
    pub doorbell_stores: AtomicU64,
    /// SoftGPU observe cursor (mirrors processor progress with read_index in Phase 4).
    pub observed_through: AtomicU64,
    observed_ids: std::sync::Mutex<HashSet<u64>>,
}

// SAFETY: SoftGPU owns the packet buffer exclusively; mutation of metadata is
// under the runtime mutex except for atomic indexes / observed set lock.
unsafe impl Send for SoftGpuQueue {}

impl SoftGpuQueue {
    pub fn new(
        handle: PackedHandle,
        agent_handle: PackedHandle,
        id: u64,
        abi: Box<HsaQueueAbi>,
        packet_buffer: *mut u8,
        packet_bytes: usize,
        doorbell: PackedHandle,
    ) -> Self {
        Self {
            handle,
            agent_handle,
            id,
            abi,
            packet_buffer,
            packet_bytes,
            doorbell,
            write_index: AtomicU64::new(0),
            read_index: AtomicU64::new(0),
            doorbell_stores: AtomicU64::new(0),
            observed_through: AtomicU64::new(0),
            observed_ids: std::sync::Mutex::new(HashSet::new()),
        }
    }

    pub fn size_packets(&self) -> u64 {
        u64::from(self.abi.size)
    }

    pub fn write_index(&self) -> u64 {
        self.write_index.load(Ordering::SeqCst)
    }

    pub fn read_index(&self) -> u64 {
        self.read_index.load(Ordering::SeqCst)
    }

    pub fn observed_through(&self) -> u64 {
        self.observed_through.load(Ordering::SeqCst)
    }

    pub fn store_write_index(&self, value: u64) {
        self.write_index.store(value, Ordering::SeqCst);
    }

    pub fn store_read_index(&self, value: u64) {
        self.read_index.store(value, Ordering::SeqCst);
    }

    pub fn note_doorbell_store(&self) {
        self.doorbell_stores.fetch_add(1, Ordering::SeqCst);
    }

    pub fn doorbell_store_count(&self) -> u64 {
        self.doorbell_stores.load(Ordering::SeqCst)
    }

    pub fn abi_ptr(&self) -> *mut HsaQueueAbi {
        (&*self.abi) as *const HsaQueueAbi as *mut HsaQueueAbi
    }

    pub fn packet_bytes_at(
        &self,
        packet_index: u64,
    ) -> Result<[u8; AQL_PACKET_BYTES], PacketValidateError> {
        let size = self.size_packets().max(1);
        let slot = (packet_index % size) as usize;
        let offset = slot.saturating_mul(AQL_PACKET_BYTES);
        if self.packet_buffer.is_null() || offset + AQL_PACKET_BYTES > self.packet_bytes {
            return Err(PacketValidateError::BufferFault { packet_index });
        }
        let mut out = [0u8; AQL_PACKET_BYTES];
        // SAFETY: SoftGPU owns buffer; offset checked.
        unsafe {
            std::ptr::copy_nonoverlapping(
                self.packet_buffer.add(offset),
                out.as_mut_ptr(),
                AQL_PACKET_BYTES,
            );
        }
        Ok(out)
    }

    /// Write a packet header type into the ring (test / SoftGPU producer helper).
    pub fn write_packet_type(&self, packet_index: u64, packet_type: u16) {
        let size = self.size_packets().max(1);
        let slot = (packet_index % size) as usize;
        let offset = slot.saturating_mul(AQL_PACKET_BYTES);
        if self.packet_buffer.is_null() || offset + 2 > self.packet_bytes {
            return;
        }
        // SAFETY: SoftGPU owns the buffer.
        unsafe {
            let p = self.packet_buffer.add(offset);
            *p = (packet_type & 0xff) as u8;
            *p.add(1) = (packet_type >> 8) as u8;
        }
    }

    /// Copy full packet bytes into the ring slot (SoftGPU / test producer).
    pub fn write_packet_bytes(&self, packet_index: u64, bytes: &[u8; AQL_PACKET_BYTES]) {
        let size = self.size_packets().max(1);
        let slot = (packet_index % size) as usize;
        let offset = slot.saturating_mul(AQL_PACKET_BYTES);
        if self.packet_buffer.is_null() || offset + AQL_PACKET_BYTES > self.packet_bytes {
            return;
        }
        // SAFETY: SoftGPU owns the buffer.
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                self.packet_buffer.add(offset),
                AQL_PACKET_BYTES,
            );
        }
    }

    /// Mark slot INVALID after SoftGPU consumed it (protocol progress).
    pub fn invalidate_packet_slot(&self, packet_index: u64) {
        self.write_packet_type(packet_index, PACKET_TYPE_INVALID);
    }

    /// Observe newly submitted packets exactly once. Copies bytes for Phase 4 parse.
    pub fn observe_packets(&self) -> Result<Vec<PacketObservation>, PacketValidateError> {
        let write = self.write_index();
        let mut cursor = self.observed_through();
        let mut out = Vec::new();
        while cursor < write {
            let bytes = self.packet_bytes_at(cursor)?;
            let ty = u16::from(bytes[0]);
            if ty == PACKET_TYPE_INVALID {
                return Err(PacketValidateError::StillInvalid {
                    packet_index: cursor,
                });
            }
            {
                let mut set = self.observed_ids.lock().unwrap_or_else(|p| p.into_inner());
                if !set.insert(cursor) {
                    return Err(PacketValidateError::AlreadyObserved {
                        packet_index: cursor,
                    });
                }
            }
            out.push(PacketObservation {
                packet_index: cursor,
                packet_type: ty,
                bytes,
            });
            cursor += 1;
        }
        self.observed_through.store(cursor, Ordering::SeqCst);
        Ok(out)
    }

    pub fn observed_count(&self) -> usize {
        self.observed_ids
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .len()
    }
}

/// Initialize every packet header type field to INVALID.
pub fn init_packet_buffer(buf: &mut [u8]) {
    assert_eq!(buf.len() % AQL_PACKET_BYTES, 0);
    for chunk in buf.chunks_exact_mut(AQL_PACKET_BYTES) {
        chunk.fill(0);
        chunk[0] = PACKET_TYPE_INVALID as u8;
        chunk[1] = 0;
    }
}

pub fn is_power_of_two(n: u32) -> bool {
    n != 0 && n.is_power_of_two()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handle::HandleKind;

    fn test_queue(size: u32) -> SoftGpuQueue {
        let bytes = (size as usize) * AQL_PACKET_BYTES;
        let mut buf = vec![0u8; bytes];
        init_packet_buffer(&mut buf);
        let leaked = Box::leak(buf.into_boxed_slice());
        let abi = Box::new(HsaQueueAbi {
            type_: QUEUE_TYPE_MULTI,
            features: QUEUE_FEATURE_KERNEL_DISPATCH,
            base_address: leaked.as_mut_ptr(),
            doorbell_signal: 0,
            size,
            reserved1: 0,
            id: 1,
        });
        SoftGpuQueue::new(
            PackedHandle::pack(HandleKind::Queue, 1, 0),
            PackedHandle::pack(HandleKind::Agent, 1, 0),
            1,
            abi,
            leaked.as_mut_ptr(),
            bytes,
            PackedHandle::pack(HandleKind::Signal, 1, 0),
        )
    }

    #[test]
    fn packet_buffer_marks_invalid() {
        let mut buf = vec![0u8; AQL_PACKET_BYTES * 4];
        init_packet_buffer(&mut buf);
        assert_eq!(buf[0], PACKET_TYPE_INVALID as u8);
    }

    #[test]
    fn observe_packet_exactly_once() {
        let q = test_queue(SOFTGPU_QUEUE_MIN_SIZE);
        q.write_packet_type(0, PACKET_TYPE_KERNEL_DISPATCH);
        q.store_write_index(1);
        let first = q.observe_packets().unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].packet_index, 0);
        let second = q.observe_packets().unwrap();
        assert!(second.is_empty());
    }
}
