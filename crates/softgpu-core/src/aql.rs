//! AQL packet decode/validate for SoftGPU Phase 4 (no kernel execution).
//!
//! Parses HSA kernel-dispatch (and minimal barrier) packets from owned bytes.
//! Layout provenance: pinned ROCR `hsa.h` (`hsa_kernel_dispatch_packet_t`
//! under `HSA_LARGE_MODEL`, 64 bytes).

use crate::queue::{AQL_PACKET_BYTES, PACKET_TYPE_INVALID, PACKET_TYPE_KERNEL_DISPATCH};

/// `HSA_PACKET_TYPE_BARRIER_AND`.
pub const PACKET_TYPE_BARRIER_AND: u16 = 3;
/// `HSA_PACKET_TYPE_AGENT_DISPATCH`.
pub const PACKET_TYPE_AGENT_DISPATCH: u16 = 4;
/// `HSA_PACKET_TYPE_BARRIER_OR`.
pub const PACKET_TYPE_BARRIER_OR: u16 = 5;

/// SoftGPU experimental no-execution completion contract label.
pub const DIAGNOSTIC_COMPLETE_NO_EXECUTION: &str = "diagnostic_complete_no_execution";
pub const DIAGNOSTIC_REJECTED: &str = "diagnostic_rejected";
/// SoftGPU Phase 11: registered ISA kernel ran successfully (named subset only).
pub const KERNEL_SUCCESS: &str = "softgpu_kernel_success";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketType {
    Invalid,
    KernelDispatch,
    BarrierAnd,
    AgentDispatch,
    BarrierOr,
    VendorOrUnknown(u16),
}

impl PacketType {
    pub fn from_header_type(ty: u16) -> Self {
        match ty & 0xff {
            1 => Self::Invalid,
            2 => Self::KernelDispatch,
            3 => Self::BarrierAnd,
            4 => Self::AgentDispatch,
            5 => Self::BarrierOr,
            other => Self::VendorOrUnknown(other),
        }
    }

    pub fn as_u16(self) -> u16 {
        match self {
            Self::Invalid => PACKET_TYPE_INVALID,
            Self::KernelDispatch => PACKET_TYPE_KERNEL_DISPATCH,
            Self::BarrierAnd => PACKET_TYPE_BARRIER_AND,
            Self::AgentDispatch => PACKET_TYPE_AGENT_DISPATCH,
            Self::BarrierOr => PACKET_TYPE_BARRIER_OR,
            Self::VendorOrUnknown(v) => v,
        }
    }
}

/// How SoftGPU classifies a kernarg pointer without claiming contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernargClass {
    Null,
    /// SoftGPU-tracked allocation (alloc_id when known).
    SoftGpu {
        alloc_id: Option<u64>,
        addr: u64,
    },
    /// Non-null pointer SoftGPU does not own — opaque for replay.
    ForeignOpaque {
        addr: u64,
    },
}

/// Normalized dispatch descriptor (vendor-neutral SoftGPU view).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchDescriptor {
    pub packet_index: u64,
    pub packet_type: PacketType,
    pub header: u16,
    pub setup: u16,
    pub dimensions: u16,
    pub workgroup_size: [u16; 3],
    pub grid_size: [u32; 3],
    pub private_segment_size: u32,
    pub group_segment_size: u32,
    pub kernel_object: u64,
    pub kernarg: KernargClass,
    pub completion_signal: u64,
    /// Owned AQL bytes for replay without living host pointers.
    pub captured_bytes: [u8; AQL_PACKET_BYTES],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AqlParseError {
    WrongLength { got: usize },
    StillInvalid,
    UnsupportedType { packet_type: u16 },
    ZeroWorkgroup { dim: usize },
    GridSmallerThanWorkgroup { dim: usize },
    BadDimensions { dimensions: u16 },
    DimConstraint { detail: String },
}

impl std::fmt::Display for AqlParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

fn read_u16(bytes: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([bytes[off], bytes[off + 1]])
}

fn read_u32(bytes: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]])
}

fn read_u64(bytes: &[u8], off: usize) -> u64 {
    u64::from_le_bytes([
        bytes[off],
        bytes[off + 1],
        bytes[off + 2],
        bytes[off + 3],
        bytes[off + 4],
        bytes[off + 5],
        bytes[off + 6],
        bytes[off + 7],
    ])
}

/// Parse a Phase 4-supported packet (kernel-dispatch, or minimal barrier).
pub fn parse_supported_packet(
    bytes: &[u8],
    packet_index: u64,
    classify_kernarg: impl FnOnce(u64) -> KernargClass,
) -> Result<DispatchDescriptor, AqlParseError> {
    if bytes.len() != AQL_PACKET_BYTES {
        return Err(AqlParseError::WrongLength { got: bytes.len() });
    }
    let header = read_u16(bytes, 0);
    let ty = header & 0xff;
    match PacketType::from_header_type(ty) {
        PacketType::Invalid => Err(AqlParseError::StillInvalid),
        PacketType::KernelDispatch => parse_kernel_dispatch(bytes, packet_index, classify_kernarg),
        PacketType::BarrierAnd | PacketType::BarrierOr => {
            parse_barrier_minimal(bytes, packet_index)
        }
        other => Err(AqlParseError::UnsupportedType {
            packet_type: other.as_u16(),
        }),
    }
}

/// Minimal barrier accept: header type + completion signal only (deps unchecked).
pub fn parse_barrier_minimal(
    bytes: &[u8],
    packet_index: u64,
) -> Result<DispatchDescriptor, AqlParseError> {
    if bytes.len() != AQL_PACKET_BYTES {
        return Err(AqlParseError::WrongLength { got: bytes.len() });
    }
    let header = read_u16(bytes, 0);
    let ty = header & 0xff;
    let packet_type = PacketType::from_header_type(ty);
    match packet_type {
        PacketType::BarrierAnd | PacketType::BarrierOr => {}
        PacketType::Invalid => return Err(AqlParseError::StillInvalid),
        other => {
            return Err(AqlParseError::UnsupportedType {
                packet_type: other.as_u16(),
            });
        }
    }
    let mut captured = [0u8; AQL_PACKET_BYTES];
    captured.copy_from_slice(bytes);
    Ok(DispatchDescriptor {
        packet_index,
        packet_type,
        header,
        setup: 0,
        dimensions: 0,
        workgroup_size: [0, 0, 0],
        grid_size: [0, 0, 0],
        private_segment_size: 0,
        group_segment_size: 0,
        kernel_object: 0,
        kernarg: KernargClass::Null,
        completion_signal: read_u64(bytes, 56),
        captured_bytes: captured,
    })
}

/// Parse and validate a kernel-dispatch packet from exactly 64 bytes.
pub fn parse_kernel_dispatch(
    bytes: &[u8],
    packet_index: u64,
    classify_kernarg: impl FnOnce(u64) -> KernargClass,
) -> Result<DispatchDescriptor, AqlParseError> {
    if bytes.len() != AQL_PACKET_BYTES {
        return Err(AqlParseError::WrongLength { got: bytes.len() });
    }
    let header = read_u16(bytes, 0);
    let ty = header & 0xff;
    let packet_type = PacketType::from_header_type(ty);
    match packet_type {
        PacketType::Invalid => return Err(AqlParseError::StillInvalid),
        PacketType::KernelDispatch => {}
        other => {
            return Err(AqlParseError::UnsupportedType {
                packet_type: other.as_u16(),
            });
        }
    }

    let setup = read_u16(bytes, 2);
    // HSA_KERNEL_DISPATCH_PACKET_SETUP_DIMENSIONS width 2 at bit 0.
    let dimensions = setup & 0b11;
    if !(1..=3).contains(&dimensions) {
        return Err(AqlParseError::BadDimensions { dimensions });
    }

    let workgroup_size = [read_u16(bytes, 4), read_u16(bytes, 6), read_u16(bytes, 8)];
    let grid_size = [
        read_u32(bytes, 12),
        read_u32(bytes, 16),
        read_u32(bytes, 20),
    ];
    let private_segment_size = read_u32(bytes, 24);
    let group_segment_size = read_u32(bytes, 28);
    let kernel_object = read_u64(bytes, 32);
    let kernarg_addr = read_u64(bytes, 40);
    let completion_signal = read_u64(bytes, 56);

    for d in 0..3 {
        if workgroup_size[d] == 0 {
            return Err(AqlParseError::ZeroWorkgroup { dim: d });
        }
        if grid_size[d] < u32::from(workgroup_size[d]) {
            return Err(AqlParseError::GridSmallerThanWorkgroup { dim: d });
        }
    }
    if dimensions == 1 && (workgroup_size[1] != 1 || workgroup_size[2] != 1) {
        return Err(AqlParseError::DimConstraint {
            detail: "dims=1 requires workgroup y=z=1".into(),
        });
    }
    if dimensions == 1 && (grid_size[1] != 1 || grid_size[2] != 1) {
        return Err(AqlParseError::DimConstraint {
            detail: "dims=1 requires grid y=z=1".into(),
        });
    }
    if dimensions == 2 && (workgroup_size[2] != 1 || grid_size[2] != 1) {
        return Err(AqlParseError::DimConstraint {
            detail: "dims=2 requires workgroup/grid z=1".into(),
        });
    }

    let mut captured = [0u8; AQL_PACKET_BYTES];
    captured.copy_from_slice(bytes);

    Ok(DispatchDescriptor {
        packet_index,
        packet_type,
        header,
        setup,
        dimensions,
        workgroup_size,
        grid_size,
        private_segment_size,
        group_segment_size,
        kernel_object,
        kernarg: classify_kernarg(kernarg_addr),
        completion_signal,
        captured_bytes: captured,
    })
}

/// Replay parser on captured bytes (no live host pointers required).
pub fn replay_dispatch(
    bytes: &[u8],
    packet_index: u64,
) -> Result<DispatchDescriptor, AqlParseError> {
    parse_supported_packet(bytes, packet_index, |addr| {
        if addr == 0 {
            KernargClass::Null
        } else {
            // Replay cannot re-resolve SoftGPU ownership; keep opaque.
            KernargClass::ForeignOpaque { addr }
        }
    })
}

/// Build a lawful minimal 1D kernel-dispatch packet (SoftGPU test fixture).
pub fn golden_kernel_dispatch_1d(
    workgroup_x: u16,
    grid_x: u32,
    kernel_object: u64,
    kernarg: u64,
    completion_signal: u64,
) -> [u8; AQL_PACKET_BYTES] {
    let mut b = [0u8; AQL_PACKET_BYTES];
    // header: type = KERNEL_DISPATCH
    b[0] = PACKET_TYPE_KERNEL_DISPATCH as u8;
    // setup: dimensions = 1
    b[2] = 1;
    b[4..6].copy_from_slice(&workgroup_x.to_le_bytes());
    b[6..8].copy_from_slice(&1u16.to_le_bytes());
    b[8..10].copy_from_slice(&1u16.to_le_bytes());
    b[12..16].copy_from_slice(&grid_x.to_le_bytes());
    b[16..20].copy_from_slice(&1u32.to_le_bytes());
    b[20..24].copy_from_slice(&1u32.to_le_bytes());
    b[32..40].copy_from_slice(&kernel_object.to_le_bytes());
    b[40..48].copy_from_slice(&kernarg.to_le_bytes());
    b[56..64].copy_from_slice(&completion_signal.to_le_bytes());
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn golden_parses_and_replays() {
        let bytes = golden_kernel_dispatch_1d(64, 256, 0xABCDu64, 0, 0x1111);
        let d = parse_kernel_dispatch(&bytes, 0, |_| KernargClass::Null).unwrap();
        assert_eq!(d.dimensions, 1);
        assert_eq!(d.workgroup_size, [64, 1, 1]);
        assert_eq!(d.grid_size, [256, 1, 1]);
        assert_eq!(d.kernel_object, 0xABCD);
        assert_eq!(d.completion_signal, 0x1111);
        let replayed = replay_dispatch(&d.captured_bytes, 0).unwrap();
        assert_eq!(replayed.grid_size, d.grid_size);
        assert_eq!(replayed.kernel_object, d.kernel_object);
    }

    #[test]
    fn rejects_zero_workgroup() {
        let mut bytes = golden_kernel_dispatch_1d(0, 256, 1, 0, 0);
        // force wg_x = 0 already
        let err = parse_kernel_dispatch(&bytes, 0, |_| KernargClass::Null).unwrap_err();
        assert!(matches!(err, AqlParseError::ZeroWorkgroup { dim: 0 }));
        bytes = golden_kernel_dispatch_1d(64, 32, 1, 0, 0); // grid < wg
        let err = parse_kernel_dispatch(&bytes, 0, |_| KernargClass::Null).unwrap_err();
        assert!(matches!(
            err,
            AqlParseError::GridSmallerThanWorkgroup { dim: 0 }
        ));
    }

    #[test]
    fn rejects_invalid_and_unsupported() {
        let mut bytes = [0u8; AQL_PACKET_BYTES];
        bytes[0] = PACKET_TYPE_INVALID as u8;
        assert!(matches!(
            parse_kernel_dispatch(&bytes, 0, |_| KernargClass::Null),
            Err(AqlParseError::StillInvalid)
        ));
        bytes[0] = PACKET_TYPE_AGENT_DISPATCH as u8;
        assert!(matches!(
            parse_kernel_dispatch(&bytes, 0, |_| KernargClass::Null),
            Err(AqlParseError::UnsupportedType { .. })
        ));
    }
}
