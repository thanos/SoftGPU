//! Host-backed global memory arena (SoftGPU functional address space).

use crate::error::{FunctionalError, Result};
use crate::ir::TypeId;

/// Linear global memory. SoftGPU functional "pointers" are byte offsets into this arena.
#[derive(Debug, Clone)]
pub struct GlobalArena {
    bytes: Vec<u8>,
}

impl GlobalArena {
    pub fn new(len: usize) -> Self {
        Self {
            bytes: vec![0u8; len],
        }
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.bytes
    }

    pub fn fill_i32_ramp(&mut self, start: i32) {
        let mut v = start;
        for chunk in self.bytes.chunks_exact_mut(4) {
            chunk.copy_from_slice(&v.to_le_bytes());
            v = v.wrapping_add(1);
        }
    }

    pub fn read_i32_slice(&self) -> Result<Vec<i32>> {
        if self.bytes.len() % 4 != 0 {
            return Err(FunctionalError::Validation {
                detail: "arena length not multiple of 4".into(),
            });
        }
        Ok(self
            .bytes
            .chunks_exact(4)
            .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect())
    }

    pub fn load(&self, addr: u64, ty: TypeId) -> Result<i64> {
        let size = ty_size(ty);
        let start = usize::try_from(addr).map_err(|_| FunctionalError::Bounds {
            addr,
            size,
            arena_len: self.bytes.len(),
        })?;
        let end = start.checked_add(size).ok_or(FunctionalError::Bounds {
            addr,
            size,
            arena_len: self.bytes.len(),
        })?;
        if end > self.bytes.len() {
            return Err(FunctionalError::Bounds {
                addr,
                size,
                arena_len: self.bytes.len(),
            });
        }
        let s = &self.bytes[start..end];
        Ok(match ty {
            TypeId::I32 => i32::from_le_bytes([s[0], s[1], s[2], s[3]]) as i64,
            TypeId::U32 => u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as i64,
            TypeId::U64 => {
                u64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]) as i64
            }
        })
    }

    pub fn store(&mut self, addr: u64, ty: TypeId, value: i64) -> Result<()> {
        let size = ty_size(ty);
        let start = usize::try_from(addr).map_err(|_| FunctionalError::Bounds {
            addr,
            size,
            arena_len: self.bytes.len(),
        })?;
        let end = start.checked_add(size).ok_or(FunctionalError::Bounds {
            addr,
            size,
            arena_len: self.bytes.len(),
        })?;
        if end > self.bytes.len() {
            return Err(FunctionalError::Bounds {
                addr,
                size,
                arena_len: self.bytes.len(),
            });
        }
        match ty {
            TypeId::I32 => {
                self.bytes[start..end].copy_from_slice(&(value as i32).to_le_bytes());
            }
            TypeId::U32 => {
                self.bytes[start..end].copy_from_slice(&(value as u32).to_le_bytes());
            }
            TypeId::U64 => {
                self.bytes[start..end].copy_from_slice(&(value as u64).to_le_bytes());
            }
        }
        Ok(())
    }
}

pub fn ty_size(ty: TypeId) -> usize {
    match ty {
        TypeId::I32 | TypeId::U32 => 4,
        TypeId::U64 => 8,
    }
}

pub fn kernarg_load(kernarg: &[u8], offset: u32, ty: TypeId) -> Result<i64> {
    let size = ty_size(ty);
    let start = offset as usize;
    let end = start.checked_add(size).ok_or(FunctionalError::Bounds {
        addr: u64::from(offset),
        size,
        arena_len: kernarg.len(),
    })?;
    if end > kernarg.len() {
        return Err(FunctionalError::Bounds {
            addr: u64::from(offset),
            size,
            arena_len: kernarg.len(),
        });
    }
    let s = &kernarg[start..end];
    Ok(match ty {
        TypeId::I32 => i32::from_le_bytes([s[0], s[1], s[2], s[3]]) as i64,
        TypeId::U32 => u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as i64,
        TypeId::U64 => u64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]) as i64,
    })
}
