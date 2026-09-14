//! HSA C types and constants sourced from the pinned ROCR `hsa.h`.
//!
//! Provenance: `third_party/rocr-headers/` (see README + COMMIT.txt).
//! Layouts are cross-checked by `tools/hsa-layout-probe`.

use core::mem;
use softgpu_core::queue::HsaQueueAbi;

/// HSA status codes (subset) from pinned `hsa.h`.
pub type hsa_status_t = u32;

pub const HSA_STATUS_SUCCESS: hsa_status_t = 0x0;
pub const HSA_STATUS_INFO_BREAK: hsa_status_t = 0x1;
pub const HSA_STATUS_ERROR: hsa_status_t = 0x1000;
pub const HSA_STATUS_ERROR_INVALID_ARGUMENT: hsa_status_t = 0x1001;
pub const HSA_STATUS_ERROR_INVALID_QUEUE_CREATION: hsa_status_t = 0x1002;
pub const HSA_STATUS_ERROR_INVALID_ALLOCATION: hsa_status_t = 0x1003;
pub const HSA_STATUS_ERROR_INVALID_AGENT: hsa_status_t = 0x1004;
pub const HSA_STATUS_ERROR_INVALID_REGION: hsa_status_t = 0x1005;
pub const HSA_STATUS_ERROR_INVALID_SIGNAL: hsa_status_t = 0x1006;
pub const HSA_STATUS_ERROR_INVALID_QUEUE: hsa_status_t = 0x1007;
pub const HSA_STATUS_ERROR_OUT_OF_RESOURCES: hsa_status_t = 0x1008;
pub const HSA_STATUS_ERROR_NOT_INITIALIZED: hsa_status_t = 0x100B;
pub const HSA_STATUS_ERROR_REFCOUNT_OVERFLOW: hsa_status_t = 0x100C;
pub const HSA_STATUS_ERROR_INVALID_MEMORY_POOL: hsa_status_t = 40;

/// Opaque agent handle (`struct hsa_agent_s { uint64_t handle; }`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct hsa_agent_t {
    pub handle: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct hsa_region_t {
    pub handle: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct hsa_amd_memory_pool_t {
    pub handle: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct hsa_signal_t {
    pub handle: u64,
}

pub type hsa_signal_value_t = i64;
pub type hsa_queue_t = HsaQueueAbi;

pub type hsa_agent_info_t = i32;
pub type hsa_device_type_t = u32;
pub type hsa_system_info_t = i32;
pub type hsa_region_info_t = i32;
pub type hsa_amd_memory_pool_info_t = i32;
pub type hsa_amd_agent_memory_pool_info_t = i32;
pub type hsa_signal_condition_t = u32;
pub type hsa_wait_state_t = u32;
pub type hsa_queue_type32_t = u32;

pub const HSA_AGENT_INFO_NAME: hsa_agent_info_t = 0;
pub const HSA_AGENT_INFO_VENDOR_NAME: hsa_agent_info_t = 1;
pub const HSA_AGENT_INFO_FEATURE: hsa_agent_info_t = 2;
pub const HSA_AGENT_INFO_QUEUES_MAX: hsa_agent_info_t = 12;
pub const HSA_AGENT_INFO_QUEUE_MIN_SIZE: hsa_agent_info_t = 13;
pub const HSA_AGENT_INFO_QUEUE_MAX_SIZE: hsa_agent_info_t = 14;
pub const HSA_AGENT_INFO_QUEUE_TYPE: hsa_agent_info_t = 15;
pub const HSA_AGENT_INFO_DEVICE: hsa_agent_info_t = 17;
pub const HSA_AGENT_INFO_VERSION_MAJOR: hsa_agent_info_t = 21;
pub const HSA_AGENT_INFO_VERSION_MINOR: hsa_agent_info_t = 22;

pub const HSA_AGENT_FEATURE_KERNEL_DISPATCH: u32 = 1;

pub const HSA_DEVICE_TYPE_CPU: hsa_device_type_t = 0;
pub const HSA_DEVICE_TYPE_GPU: hsa_device_type_t = 1;

pub const HSA_SYSTEM_INFO_VERSION_MAJOR: hsa_system_info_t = 0;
pub const HSA_SYSTEM_INFO_VERSION_MINOR: hsa_system_info_t = 1;
pub const HSA_SYSTEM_INFO_TIMESTAMP: hsa_system_info_t = 2;
pub const HSA_SYSTEM_INFO_TIMESTAMP_FREQUENCY: hsa_system_info_t = 3;
pub const HSA_SYSTEM_INFO_SIGNAL_MAX_WAIT: hsa_system_info_t = 4;
pub const HSA_SYSTEM_INFO_ENDIANNESS: hsa_system_info_t = 5;
pub const HSA_SYSTEM_INFO_MACHINE_MODEL: hsa_system_info_t = 6;
pub const HSA_SYSTEM_INFO_EXTENSIONS: hsa_system_info_t = 7;

pub const HSA_ENDIANNESS_LITTLE: u32 = 0;
pub const HSA_MACHINE_MODEL_LARGE: u32 = 1;

pub const HSA_REGION_INFO_SEGMENT: hsa_region_info_t = 0;
pub const HSA_REGION_INFO_GLOBAL_FLAGS: hsa_region_info_t = 1;
pub const HSA_REGION_INFO_SIZE: hsa_region_info_t = 2;
pub const HSA_REGION_INFO_ALLOC_MAX_SIZE: hsa_region_info_t = 4;
pub const HSA_REGION_INFO_RUNTIME_ALLOC_ALLOWED: hsa_region_info_t = 5;
pub const HSA_REGION_INFO_RUNTIME_ALLOC_GRANULE: hsa_region_info_t = 6;
pub const HSA_REGION_INFO_RUNTIME_ALLOC_ALIGNMENT: hsa_region_info_t = 7;

pub const HSA_AMD_MEMORY_POOL_INFO_SEGMENT: hsa_amd_memory_pool_info_t = 0;
pub const HSA_AMD_MEMORY_POOL_INFO_GLOBAL_FLAGS: hsa_amd_memory_pool_info_t = 1;
pub const HSA_AMD_MEMORY_POOL_INFO_SIZE: hsa_amd_memory_pool_info_t = 2;
pub const HSA_AMD_MEMORY_POOL_INFO_RUNTIME_ALLOC_ALLOWED: hsa_amd_memory_pool_info_t = 5;
pub const HSA_AMD_MEMORY_POOL_INFO_RUNTIME_ALLOC_GRANULE: hsa_amd_memory_pool_info_t = 6;
pub const HSA_AMD_MEMORY_POOL_INFO_RUNTIME_ALLOC_ALIGNMENT: hsa_amd_memory_pool_info_t = 7;
pub const HSA_AMD_MEMORY_POOL_INFO_ACCESSIBLE_BY_ALL: hsa_amd_memory_pool_info_t = 15;
pub const HSA_AMD_MEMORY_POOL_INFO_ALLOC_MAX_SIZE: hsa_amd_memory_pool_info_t = 16;
pub const HSA_AMD_MEMORY_POOL_INFO_LOCATION: hsa_amd_memory_pool_info_t = 17;
pub const HSA_AMD_MEMORY_POOL_INFO_RUNTIME_ALLOC_REC_GRANULE: hsa_amd_memory_pool_info_t = 18;

pub const HSA_AMD_AGENT_MEMORY_POOL_INFO_ACCESS: hsa_amd_agent_memory_pool_info_t = 0;

pub const HSA_SIGNAL_CONDITION_EQ: hsa_signal_condition_t = 0;
pub const HSA_WAIT_STATE_BLOCKED: hsa_wait_state_t = 0;
pub const HSA_WAIT_STATE_ACTIVE: hsa_wait_state_t = 1;

const _: () = assert!(mem::size_of::<hsa_agent_t>() == 8);
const _: () = assert!(mem::align_of::<hsa_agent_t>() == 8);
const _: () = assert!(mem::size_of::<hsa_status_t>() == 4);
const _: () = assert!(mem::size_of::<hsa_queue_t>() == 40);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_handle_is_64_bit_struct() {
        assert_eq!(std::mem::size_of::<hsa_agent_t>(), 8);
        assert_eq!(std::mem::align_of::<hsa_agent_t>(), 8);
    }
}
