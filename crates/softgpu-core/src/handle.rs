//! Generation-safe packed handles.
//!
//! SoftGPU never exposes raw table indices alone. A handle packs
//! `kind | generation | index` so stale, forged, and cross-kind values fail
//! deterministically after recycle.

use std::fmt;

/// Discriminator stored in the high bits of a packed handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum HandleKind {
    Agent = 1,
}

impl HandleKind {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Agent),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
        }
    }
}

/// Opaque 64-bit handle: `[kind:8][generation:24][index:32]`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PackedHandle(u64);

impl PackedHandle {
    pub const INVALID: Self = Self(0);

    pub fn pack(kind: HandleKind, generation: u32, index: u32) -> Self {
        let generation = generation & 0x00FF_FFFF;
        let value =
            (u64::from(kind as u8) << 56) | (u64::from(generation) << 32) | u64::from(index);
        Self(value)
    }

    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    pub fn raw(self) -> u64 {
        self.0
    }

    pub fn is_invalid(self) -> bool {
        self.0 == 0
    }

    pub fn kind(self) -> Option<HandleKind> {
        HandleKind::from_u8((self.0 >> 56) as u8)
    }

    pub fn generation(self) -> u32 {
        ((self.0 >> 32) & 0x00FF_FFFF) as u32
    }

    pub fn index(self) -> u32 {
        self.0 as u32
    }
}

impl fmt::Debug for PackedHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind() {
            Some(kind) => write!(
                f,
                "PackedHandle {{ kind: {}, gen: {}, index: {}, raw: {:#x} }}",
                kind.as_str(),
                self.generation(),
                self.index(),
                self.0
            ),
            None => write!(f, "PackedHandle {{ invalid_or_unknown: {:#x} }}", self.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_round_trip() {
        let h = PackedHandle::pack(HandleKind::Agent, 7, 3);
        assert_eq!(h.kind(), Some(HandleKind::Agent));
        assert_eq!(h.generation(), 7);
        assert_eq!(h.index(), 3);
        assert_eq!(PackedHandle::from_raw(h.raw()), h);
    }

    #[test]
    fn generation_is_masked_to_24_bits() {
        let h = PackedHandle::pack(HandleKind::Agent, 0x01FF_FFFF, 1);
        assert_eq!(h.generation(), 0x00FF_FFFF);
    }

    #[test]
    fn zero_is_invalid() {
        assert!(PackedHandle::INVALID.is_invalid());
        assert!(PackedHandle::from_raw(0).kind().is_none());
    }
}
