//! SoftGPU global memory for Phase 11 ISA loads/stores.
//!
//! Addresses are SoftGPU software virtual addresses (for the arena helper) or
//! host pointers tracked by SoftGPU's allocator (AQL path).

use crate::error::{IsaError, IsaErrorKind, Result};

/// Memory view used by SoftGPU ISA global/SMEM ops.
pub trait IsaMemory {
    fn load_u32(&self, addr: u64) -> Result<u32>;
    fn store_u32(&mut self, addr: u64, value: u32) -> Result<()>;
    fn load_u64(&self, addr: u64) -> Result<u64>;
    fn store_u64(&mut self, addr: u64, value: u64) -> Result<()>;
}

/// SoftGPU-owned byte arena used as gfx1201 global memory for offline/CLI runs.
#[derive(Debug, Clone)]
pub struct GlobalArena {
    pub base: u64,
    pub bytes: Vec<u8>,
}

impl GlobalArena {
    pub fn new(base: u64, size: usize) -> Self {
        Self {
            base,
            bytes: vec![0; size],
        }
    }

    pub fn contains(&self, addr: u64, len: usize) -> bool {
        let end = addr.checked_add(len as u64);
        match end {
            Some(e) => addr >= self.base && e <= self.base + self.bytes.len() as u64,
            None => false,
        }
    }

    fn offset(&self, addr: u64, len: usize) -> Result<usize> {
        if !self.contains(addr, len) {
            return Err(IsaError::new(
                IsaErrorKind::Trap,
                format!(
                    "SoftGPU global OOB addr=0x{addr:x} len={len} base=0x{:x} size={}",
                    self.base,
                    self.bytes.len()
                ),
            )
            .with_remediation("ensure kernarg/buffer pointers stay inside SoftGPU arena"));
        }
        Ok((addr - self.base) as usize)
    }

    pub fn write_bytes(&mut self, addr: u64, data: &[u8]) -> Result<()> {
        let off = self.offset(addr, data.len())?;
        self.bytes[off..off + data.len()].copy_from_slice(data);
        Ok(())
    }

    pub fn read_bytes(&self, addr: u64, out: &mut [u8]) -> Result<()> {
        let off = self.offset(addr, out.len())?;
        out.copy_from_slice(&self.bytes[off..off + out.len()]);
        Ok(())
    }
}

impl IsaMemory for GlobalArena {
    fn load_u32(&self, addr: u64) -> Result<u32> {
        let off = self.offset(addr, 4)?;
        Ok(u32::from_le_bytes(
            self.bytes[off..off + 4].try_into().unwrap(),
        ))
    }

    fn store_u32(&mut self, addr: u64, value: u32) -> Result<()> {
        let off = self.offset(addr, 4)?;
        self.bytes[off..off + 4].copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    fn load_u64(&self, addr: u64) -> Result<u64> {
        let off = self.offset(addr, 8)?;
        Ok(u64::from_le_bytes(
            self.bytes[off..off + 8].try_into().unwrap(),
        ))
    }

    fn store_u64(&mut self, addr: u64, value: u64) -> Result<()> {
        let off = self.offset(addr, 8)?;
        self.bytes[off..off + 8].copy_from_slice(&value.to_le_bytes());
        Ok(())
    }
}
