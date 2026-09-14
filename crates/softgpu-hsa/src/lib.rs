//! SoftGPU ROCr/HSA adapter (`libhsa-runtime64`).
//!
//! Phase 3 surface: init/shutdown/system info, agent discovery (`FEATURE` =
//! `KERNEL_DISPATCH` for queue ABI only), Path C memory (regions + AMD pools),
//! signals, and queue create/observe. Packet execution remains unsupported.
//!
//! # Panic / unwind policy
//!
//! Every exported function wraps its body in `catch_unwind`. Panics become
//! `HSA_STATUS_ERROR` and never unwind across the FFI boundary.

#![allow(non_camel_case_types)]

pub mod ffi;
pub mod status;

use softgpu_core::memory::{PoolInfoValue, RegionInfoValue};
use softgpu_core::runtime::{self, AgentInfoValue, PoolInfoAttr, RegionInfoAttr, RuntimeError};
use softgpu_core::{AgentInfoAttr, AgentKind, DeviceProfile, PackedHandle};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::time::Instant;

pub use ffi::*;

fn map_runtime_error(err: RuntimeError) -> hsa_status_t {
    match err {
        RuntimeError::NotInitialized => HSA_STATUS_ERROR_NOT_INITIALIZED,
        RuntimeError::RefcountOverflow => HSA_STATUS_ERROR_REFCOUNT_OVERFLOW,
        RuntimeError::InvalidArgument | RuntimeError::UnsupportedAttribute => {
            HSA_STATUS_ERROR_INVALID_ARGUMENT
        }
        RuntimeError::InvalidAgent => HSA_STATUS_ERROR_INVALID_AGENT,
        RuntimeError::InvalidRegion => HSA_STATUS_ERROR_INVALID_REGION,
        RuntimeError::InvalidPool => HSA_STATUS_ERROR_INVALID_MEMORY_POOL,
        RuntimeError::InvalidSignal => HSA_STATUS_ERROR_INVALID_SIGNAL,
        RuntimeError::InvalidQueue => HSA_STATUS_ERROR_INVALID_QUEUE,
        RuntimeError::InvalidQueueCreation => HSA_STATUS_ERROR_INVALID_QUEUE_CREATION,
        RuntimeError::InvalidAllocation => HSA_STATUS_ERROR_INVALID_ALLOCATION,
        RuntimeError::OutOfResources => HSA_STATUS_ERROR_OUT_OF_RESOURCES,
        RuntimeError::Internal(_) => HSA_STATUS_ERROR,
    }
}

fn catch_status(f: impl FnOnce() -> hsa_status_t) -> hsa_status_t {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(status) => status,
        Err(_) => HSA_STATUS_ERROR,
    }
}

/// Load a SoftGPU device profile into the process runtime (before `hsa_init`).
pub fn load_profile_path(path: impl AsRef<Path>) -> Result<(), String> {
    let profile = DeviceProfile::load_path(path).map_err(|e| e.to_string())?;
    runtime::install_runtime(profile).map_err(|e| e.to_string())
}

fn map_attr(attr: hsa_agent_info_t) -> Option<AgentInfoAttr> {
    match attr {
        HSA_AGENT_INFO_NAME => Some(AgentInfoAttr::Name),
        HSA_AGENT_INFO_VENDOR_NAME => Some(AgentInfoAttr::VendorName),
        HSA_AGENT_INFO_FEATURE => Some(AgentInfoAttr::Feature),
        HSA_AGENT_INFO_DEVICE => Some(AgentInfoAttr::Device),
        HSA_AGENT_INFO_VERSION_MAJOR => Some(AgentInfoAttr::VersionMajor),
        HSA_AGENT_INFO_VERSION_MINOR => Some(AgentInfoAttr::VersionMinor),
        HSA_AGENT_INFO_QUEUES_MAX => Some(AgentInfoAttr::QueuesMax),
        HSA_AGENT_INFO_QUEUE_MIN_SIZE => Some(AgentInfoAttr::QueueMinSize),
        HSA_AGENT_INFO_QUEUE_MAX_SIZE => Some(AgentInfoAttr::QueueMaxSize),
        HSA_AGENT_INFO_QUEUE_TYPE => Some(AgentInfoAttr::QueueType),
        _ => None,
    }
}

fn map_region_attr(attr: hsa_region_info_t) -> Option<RegionInfoAttr> {
    match attr {
        HSA_REGION_INFO_SEGMENT => Some(RegionInfoAttr::Segment),
        HSA_REGION_INFO_GLOBAL_FLAGS => Some(RegionInfoAttr::GlobalFlags),
        HSA_REGION_INFO_SIZE => Some(RegionInfoAttr::Size),
        HSA_REGION_INFO_ALLOC_MAX_SIZE => Some(RegionInfoAttr::AllocMaxSize),
        HSA_REGION_INFO_RUNTIME_ALLOC_ALLOWED => Some(RegionInfoAttr::RuntimeAllocAllowed),
        HSA_REGION_INFO_RUNTIME_ALLOC_GRANULE => Some(RegionInfoAttr::Granule),
        HSA_REGION_INFO_RUNTIME_ALLOC_ALIGNMENT => Some(RegionInfoAttr::Alignment),
        _ => None,
    }
}

fn map_pool_attr(attr: hsa_amd_memory_pool_info_t) -> Option<PoolInfoAttr> {
    match attr {
        HSA_AMD_MEMORY_POOL_INFO_SEGMENT => Some(PoolInfoAttr::Segment),
        HSA_AMD_MEMORY_POOL_INFO_GLOBAL_FLAGS => Some(PoolInfoAttr::GlobalFlags),
        HSA_AMD_MEMORY_POOL_INFO_SIZE => Some(PoolInfoAttr::Size),
        HSA_AMD_MEMORY_POOL_INFO_RUNTIME_ALLOC_ALLOWED => Some(PoolInfoAttr::RuntimeAllocAllowed),
        HSA_AMD_MEMORY_POOL_INFO_RUNTIME_ALLOC_GRANULE => Some(PoolInfoAttr::Granule),
        HSA_AMD_MEMORY_POOL_INFO_RUNTIME_ALLOC_ALIGNMENT => Some(PoolInfoAttr::Alignment),
        HSA_AMD_MEMORY_POOL_INFO_ACCESSIBLE_BY_ALL => Some(PoolInfoAttr::AccessibleByAll),
        HSA_AMD_MEMORY_POOL_INFO_ALLOC_MAX_SIZE => Some(PoolInfoAttr::AllocMaxSize),
        HSA_AMD_MEMORY_POOL_INFO_LOCATION => Some(PoolInfoAttr::Location),
        HSA_AMD_MEMORY_POOL_INFO_RUNTIME_ALLOC_REC_GRANULE => Some(PoolInfoAttr::RecGranule),
        _ => None,
    }
}

fn write_c_string64(dest: *mut core::ffi::c_void, text: &str) -> hsa_status_t {
    if dest.is_null() {
        return HSA_STATUS_ERROR_INVALID_ARGUMENT;
    }
    // SAFETY: caller-provided buffer must be at least 64 bytes per HSA agent name attrs.
    let buf = unsafe { core::slice::from_raw_parts_mut(dest as *mut u8, 64) };
    buf.fill(0);
    let bytes = text.as_bytes();
    let n = bytes.len().min(63);
    buf[..n].copy_from_slice(&bytes[..n]);
    HSA_STATUS_SUCCESS
}

fn write_u32(dest: *mut core::ffi::c_void, value: u32) -> hsa_status_t {
    if dest.is_null() {
        return HSA_STATUS_ERROR_INVALID_ARGUMENT;
    }
    unsafe {
        *(dest as *mut u32) = value;
    }
    HSA_STATUS_SUCCESS
}

fn write_u16(dest: *mut core::ffi::c_void, value: u16) -> hsa_status_t {
    if dest.is_null() {
        return HSA_STATUS_ERROR_INVALID_ARGUMENT;
    }
    unsafe {
        *(dest as *mut u16) = value;
    }
    HSA_STATUS_SUCCESS
}

fn write_u64(dest: *mut core::ffi::c_void, value: u64) -> hsa_status_t {
    if dest.is_null() {
        return HSA_STATUS_ERROR_INVALID_ARGUMENT;
    }
    unsafe {
        *(dest as *mut u64) = value;
    }
    HSA_STATUS_SUCCESS
}

fn write_usize(dest: *mut core::ffi::c_void, value: usize) -> hsa_status_t {
    if dest.is_null() {
        return HSA_STATUS_ERROR_INVALID_ARGUMENT;
    }
    unsafe {
        *(dest as *mut usize) = value;
    }
    HSA_STATUS_SUCCESS
}

fn write_bool(dest: *mut core::ffi::c_void, value: bool) -> hsa_status_t {
    if dest.is_null() {
        return HSA_STATUS_ERROR_INVALID_ARGUMENT;
    }
    unsafe {
        *(dest as *mut bool) = value;
    }
    HSA_STATUS_SUCCESS
}

fn soft_clock_ns() -> u64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let start = START.get_or_init(Instant::now);
    u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

fn require_initialized() -> Result<(), RuntimeError> {
    runtime::with_runtime(|rt| {
        if !rt.is_initialized() {
            Err(RuntimeError::NotInitialized)
        } else {
            Ok(())
        }
    })?
}

/// # Safety
///
/// Exported C ABI. Must not unwind. See crate-level panic policy.
#[no_mangle]
pub unsafe extern "C" fn hsa_init() -> hsa_status_t {
    catch_status(|| match runtime::with_runtime_autostart(|rt| rt.init()) {
        Ok(Ok(())) => HSA_STATUS_SUCCESS,
        Ok(Err(err)) => map_runtime_error(err),
        Err(err) => map_runtime_error(err),
    })
}

/// # Safety
///
/// Exported C ABI. Must not unwind.
#[no_mangle]
pub unsafe extern "C" fn hsa_shut_down() -> hsa_status_t {
    catch_status(|| match runtime::with_runtime(|rt| rt.shut_down()) {
        Ok(Ok(())) => HSA_STATUS_SUCCESS,
        Ok(Err(err)) => map_runtime_error(err),
        Err(err) => map_runtime_error(err),
    })
}

/// # Safety
///
/// `value` must be non-null and large enough for `attribute`.
#[no_mangle]
pub unsafe extern "C" fn hsa_system_get_info(
    attribute: hsa_system_info_t,
    value: *mut core::ffi::c_void,
) -> hsa_status_t {
    catch_status(|| {
        if value.is_null() {
            return HSA_STATUS_ERROR_INVALID_ARGUMENT;
        }
        if let Err(err) = require_initialized() {
            return map_runtime_error(err);
        }

        match attribute {
            HSA_SYSTEM_INFO_VERSION_MAJOR => write_u16(value, 1),
            HSA_SYSTEM_INFO_VERSION_MINOR => write_u16(value, 2),
            HSA_SYSTEM_INFO_TIMESTAMP => write_u64(value, soft_clock_ns()),
            HSA_SYSTEM_INFO_TIMESTAMP_FREQUENCY => write_u64(value, 1_000_000_000),
            HSA_SYSTEM_INFO_SIGNAL_MAX_WAIT => write_u64(value, u64::from(u32::MAX)),
            HSA_SYSTEM_INFO_ENDIANNESS => {
                #[cfg(target_endian = "little")]
                {
                    write_u32(value, HSA_ENDIANNESS_LITTLE)
                }
                #[cfg(not(target_endian = "little"))]
                {
                    HSA_STATUS_ERROR_INVALID_ARGUMENT
                }
            }
            HSA_SYSTEM_INFO_MACHINE_MODEL => {
                #[cfg(target_pointer_width = "64")]
                {
                    write_u32(value, HSA_MACHINE_MODEL_LARGE)
                }
                #[cfg(not(target_pointer_width = "64"))]
                {
                    HSA_STATUS_ERROR_INVALID_ARGUMENT
                }
            }
            HSA_SYSTEM_INFO_EXTENSIONS => {
                let buf = unsafe { core::slice::from_raw_parts_mut(value as *mut u8, 128) };
                buf.fill(0);
                HSA_STATUS_SUCCESS
            }
            other => {
                let _ = runtime::with_runtime(|rt| {
                    rt.record_unsupported(
                        "hsa_system_get_info",
                        &format!("unsupported attribute {other}"),
                    );
                });
                HSA_STATUS_ERROR_INVALID_ARGUMENT
            }
        }
    })
}

/// # Safety
///
/// SoftGPU releases the runtime lock before invoking `callback` so nested
/// SoftGPU HSA calls do not deadlock.
#[no_mangle]
pub unsafe extern "C" fn hsa_iterate_agents(
    callback: Option<unsafe extern "C" fn(hsa_agent_t, *mut core::ffi::c_void) -> hsa_status_t>,
    data: *mut core::ffi::c_void,
) -> hsa_status_t {
    catch_status(|| {
        let Some(callback) = callback else {
            return HSA_STATUS_ERROR_INVALID_ARGUMENT;
        };
        let agents = match runtime::with_runtime(|rt| {
            if !rt.is_initialized() {
                return Err(RuntimeError::NotInitialized);
            }
            let mut handles = Vec::new();
            rt.iterate_agents(|agent| {
                handles.push(hsa_agent_t {
                    handle: agent.handle.raw(),
                });
                Ok(())
            })?;
            Ok(handles)
        }) {
            Ok(Ok(handles)) => handles,
            Ok(Err(err)) => return map_runtime_error(err),
            Err(err) => return map_runtime_error(err),
        };

        for agent in agents {
            let status = catch_unwind(AssertUnwindSafe(|| callback(agent, data)))
                .unwrap_or(HSA_STATUS_ERROR);
            if status != HSA_STATUS_SUCCESS {
                return status;
            }
        }
        HSA_STATUS_SUCCESS
    })
}

/// # Safety
///
/// `value` must point to a buffer large enough for `attribute` per HSA.
#[no_mangle]
pub unsafe extern "C" fn hsa_agent_get_info(
    agent: hsa_agent_t,
    attribute: hsa_agent_info_t,
    value: *mut core::ffi::c_void,
) -> hsa_status_t {
    catch_status(|| {
        if value.is_null() {
            return HSA_STATUS_ERROR_INVALID_ARGUMENT;
        }
        let Some(attr) = map_attr(attribute) else {
            let _ = runtime::with_runtime(|rt| {
                rt.record_unsupported(
                    "hsa_agent_get_info",
                    &format!("unsupported attribute {attribute}"),
                );
            });
            return HSA_STATUS_ERROR_INVALID_ARGUMENT;
        };
        let handle = PackedHandle::from_raw(agent.handle);
        match runtime::with_runtime(|rt| rt.agent_get_info(handle, attr)) {
            Ok(Ok(info)) => match info {
                AgentInfoValue::Name(s) | AgentInfoValue::VendorName(s) => {
                    write_c_string64(value, &s)
                }
                AgentInfoValue::Feature(mask) => write_u32(value, mask),
                AgentInfoValue::Device(kind) => {
                    let device = match kind {
                        AgentKind::Cpu => HSA_DEVICE_TYPE_CPU,
                        AgentKind::Gpu => HSA_DEVICE_TYPE_GPU,
                    };
                    write_u32(value, device)
                }
                AgentInfoValue::U16(v) => write_u16(value, v),
                AgentInfoValue::U32(v) => write_u32(value, v),
            },
            Ok(Err(err)) => map_runtime_error(err),
            Err(err) => map_runtime_error(err),
        }
    })
}

/// # Safety
///
/// `status_string` must be non-null. SoftGPU returns pointers to static CStrs.
#[no_mangle]
pub unsafe extern "C" fn hsa_status_string(
    status: hsa_status_t,
    status_string: *mut *const core::ffi::c_char,
) -> hsa_status_t {
    catch_status(|| {
        if status_string.is_null() {
            return HSA_STATUS_ERROR_INVALID_ARGUMENT;
        }
        let s = status::status_string(status);
        unsafe {
            *status_string = s.as_ptr();
        }
        HSA_STATUS_SUCCESS
    })
}

/// # Safety
///
/// Callback may nest SoftGPU HSA calls; lock is released before callback.
#[no_mangle]
pub unsafe extern "C" fn hsa_agent_iterate_regions(
    agent: hsa_agent_t,
    callback: Option<unsafe extern "C" fn(hsa_region_t, *mut core::ffi::c_void) -> hsa_status_t>,
    data: *mut core::ffi::c_void,
) -> hsa_status_t {
    catch_status(|| {
        let Some(callback) = callback else {
            return HSA_STATUS_ERROR_INVALID_ARGUMENT;
        };
        let regions = match runtime::with_runtime(|rt| {
            let mut out = Vec::new();
            rt.iterate_regions(PackedHandle::from_raw(agent.handle), |space| {
                out.push(hsa_region_t {
                    handle: space.handle.raw(),
                });
                Ok(())
            })?;
            Ok(out)
        }) {
            Ok(Ok(v)) => v,
            Ok(Err(err)) => return map_runtime_error(err),
            Err(err) => return map_runtime_error(err),
        };
        for region in regions {
            let status = catch_unwind(AssertUnwindSafe(|| callback(region, data)))
                .unwrap_or(HSA_STATUS_ERROR);
            if status != HSA_STATUS_SUCCESS {
                return status;
            }
        }
        HSA_STATUS_SUCCESS
    })
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn hsa_region_get_info(
    region: hsa_region_t,
    attribute: hsa_region_info_t,
    value: *mut core::ffi::c_void,
) -> hsa_status_t {
    catch_status(|| {
        if value.is_null() {
            return HSA_STATUS_ERROR_INVALID_ARGUMENT;
        }
        let Some(attr) = map_region_attr(attribute) else {
            return HSA_STATUS_ERROR_INVALID_ARGUMENT;
        };
        match runtime::with_runtime(|rt| {
            rt.region_get_info(PackedHandle::from_raw(region.handle), attr)
        }) {
            Ok(Ok(info)) => match info {
                RegionInfoValue::Segment(v) | RegionInfoValue::GlobalFlags(v) => {
                    write_u32(value, v)
                }
                RegionInfoValue::Size(v)
                | RegionInfoValue::AllocMaxSize(v)
                | RegionInfoValue::Granule(v)
                | RegionInfoValue::Alignment(v) => write_usize(value, v),
                RegionInfoValue::RuntimeAllocAllowed(v) => write_bool(value, v),
            },
            Ok(Err(err)) => map_runtime_error(err),
            Err(err) => map_runtime_error(err),
        }
    })
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn hsa_memory_allocate(
    region: hsa_region_t,
    size: usize,
    ptr: *mut *mut core::ffi::c_void,
) -> hsa_status_t {
    catch_status(|| {
        if ptr.is_null() || size == 0 {
            return HSA_STATUS_ERROR_INVALID_ARGUMENT;
        }
        match runtime::with_runtime(|rt| {
            rt.memory_allocate(PackedHandle::from_raw(region.handle), size)
        }) {
            Ok(Ok(p)) => {
                unsafe {
                    *ptr = p as *mut core::ffi::c_void;
                }
                HSA_STATUS_SUCCESS
            }
            Ok(Err(err)) => map_runtime_error(err),
            Err(err) => map_runtime_error(err),
        }
    })
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn hsa_memory_free(ptr: *mut core::ffi::c_void) -> hsa_status_t {
    catch_status(
        || match runtime::with_runtime(|rt| rt.memory_free(ptr as *mut u8)) {
            Ok(Ok(())) => HSA_STATUS_SUCCESS,
            Ok(Err(err)) => map_runtime_error(err),
            Err(err) => map_runtime_error(err),
        },
    )
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn hsa_memory_copy(
    dst: *mut core::ffi::c_void,
    src: *const core::ffi::c_void,
    size: usize,
) -> hsa_status_t {
    catch_status(|| {
        match runtime::with_runtime(|rt| {
            // SAFETY: HSA caller provides valid dst/src of `size`.
            unsafe { rt.memory_copy(dst as *mut u8, src as *const u8, size) }
        }) {
            Ok(Ok(())) => HSA_STATUS_SUCCESS,
            Ok(Err(err)) => map_runtime_error(err),
            Err(err) => map_runtime_error(err),
        }
    })
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn hsa_amd_agent_iterate_memory_pools(
    agent: hsa_agent_t,
    callback: Option<
        unsafe extern "C" fn(hsa_amd_memory_pool_t, *mut core::ffi::c_void) -> hsa_status_t,
    >,
    data: *mut core::ffi::c_void,
) -> hsa_status_t {
    catch_status(|| {
        let Some(callback) = callback else {
            return HSA_STATUS_ERROR_INVALID_ARGUMENT;
        };
        let pools = match runtime::with_runtime(|rt| {
            let mut out = Vec::new();
            rt.iterate_pools(PackedHandle::from_raw(agent.handle), |space| {
                out.push(hsa_amd_memory_pool_t {
                    handle: space.handle.raw(),
                });
                Ok(())
            })?;
            Ok(out)
        }) {
            Ok(Ok(v)) => v,
            Ok(Err(err)) => return map_runtime_error(err),
            Err(err) => return map_runtime_error(err),
        };
        for pool in pools {
            let status =
                catch_unwind(AssertUnwindSafe(|| callback(pool, data))).unwrap_or(HSA_STATUS_ERROR);
            if status != HSA_STATUS_SUCCESS {
                return status;
            }
        }
        HSA_STATUS_SUCCESS
    })
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn hsa_amd_memory_pool_get_info(
    memory_pool: hsa_amd_memory_pool_t,
    attribute: hsa_amd_memory_pool_info_t,
    value: *mut core::ffi::c_void,
) -> hsa_status_t {
    catch_status(|| {
        if value.is_null() {
            return HSA_STATUS_ERROR_INVALID_ARGUMENT;
        }
        let Some(attr) = map_pool_attr(attribute) else {
            return HSA_STATUS_ERROR_INVALID_ARGUMENT;
        };
        match runtime::with_runtime(|rt| {
            rt.pool_get_info(PackedHandle::from_raw(memory_pool.handle), attr)
        }) {
            Ok(Ok(info)) => match info {
                PoolInfoValue::Segment(v)
                | PoolInfoValue::GlobalFlags(v)
                | PoolInfoValue::Location(v) => write_u32(value, v),
                PoolInfoValue::Size(v)
                | PoolInfoValue::Granule(v)
                | PoolInfoValue::Alignment(v)
                | PoolInfoValue::AllocMaxSize(v)
                | PoolInfoValue::RecGranule(v) => write_usize(value, v),
                PoolInfoValue::RuntimeAllocAllowed(v) | PoolInfoValue::AccessibleByAll(v) => {
                    write_bool(value, v)
                }
            },
            Ok(Err(err)) => map_runtime_error(err),
            Err(err) => map_runtime_error(err),
        }
    })
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn hsa_amd_memory_pool_allocate(
    memory_pool: hsa_amd_memory_pool_t,
    size: usize,
    _flags: u32,
    ptr: *mut *mut core::ffi::c_void,
) -> hsa_status_t {
    catch_status(|| {
        if ptr.is_null() || size == 0 {
            return HSA_STATUS_ERROR_INVALID_ARGUMENT;
        }
        match runtime::with_runtime(|rt| {
            rt.memory_allocate(PackedHandle::from_raw(memory_pool.handle), size)
        }) {
            Ok(Ok(p)) => {
                unsafe {
                    *ptr = p as *mut core::ffi::c_void;
                }
                HSA_STATUS_SUCCESS
            }
            Ok(Err(err)) => map_runtime_error(err),
            Err(err) => map_runtime_error(err),
        }
    })
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn hsa_amd_memory_pool_free(ptr: *mut core::ffi::c_void) -> hsa_status_t {
    catch_status(
        || match runtime::with_runtime(|rt| rt.memory_free(ptr as *mut u8)) {
            Ok(Ok(())) => HSA_STATUS_SUCCESS,
            Ok(Err(err)) => map_runtime_error(err),
            Err(err) => map_runtime_error(err),
        },
    )
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn hsa_amd_agents_allow_access(
    num_agents: u32,
    agents: *const hsa_agent_t,
    _flags: *const u32,
    ptr: *const core::ffi::c_void,
) -> hsa_status_t {
    catch_status(|| {
        if num_agents > 0 && agents.is_null() {
            return HSA_STATUS_ERROR_INVALID_ARGUMENT;
        }
        let list: Vec<PackedHandle> = if num_agents == 0 {
            Vec::new()
        } else {
            // SAFETY: caller provides num_agents valid agents.
            unsafe { core::slice::from_raw_parts(agents, num_agents as usize) }
                .iter()
                .map(|a| PackedHandle::from_raw(a.handle))
                .collect()
        };
        match runtime::with_runtime(|rt| rt.agents_allow_access(&list, ptr as *const u8)) {
            Ok(Ok(())) => HSA_STATUS_SUCCESS,
            Ok(Err(err)) => map_runtime_error(err),
            Err(err) => map_runtime_error(err),
        }
    })
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn hsa_amd_agent_memory_pool_get_info(
    agent: hsa_agent_t,
    memory_pool: hsa_amd_memory_pool_t,
    attribute: hsa_amd_agent_memory_pool_info_t,
    value: *mut core::ffi::c_void,
) -> hsa_status_t {
    catch_status(|| {
        if value.is_null() {
            return HSA_STATUS_ERROR_INVALID_ARGUMENT;
        }
        if attribute != HSA_AMD_AGENT_MEMORY_POOL_INFO_ACCESS {
            return HSA_STATUS_ERROR_INVALID_ARGUMENT;
        }
        match runtime::with_runtime(|rt| {
            rt.agent_memory_pool_access(
                PackedHandle::from_raw(agent.handle),
                PackedHandle::from_raw(memory_pool.handle),
            )
        }) {
            Ok(Ok(access)) => write_u32(value, access),
            Ok(Err(err)) => map_runtime_error(err),
            Err(err) => map_runtime_error(err),
        }
    })
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn hsa_signal_create(
    initial_value: hsa_signal_value_t,
    num_consumers: u32,
    consumers: *const hsa_agent_t,
    signal: *mut hsa_signal_t,
) -> hsa_status_t {
    catch_status(|| {
        if signal.is_null() || (num_consumers > 0 && consumers.is_null()) {
            return HSA_STATUS_ERROR_INVALID_ARGUMENT;
        }
        match runtime::with_runtime(|rt| rt.signal_create(initial_value)) {
            Ok(Ok(handle)) => {
                unsafe {
                    *signal = hsa_signal_t {
                        handle: handle.raw(),
                    };
                }
                HSA_STATUS_SUCCESS
            }
            Ok(Err(err)) => map_runtime_error(err),
            Err(err) => map_runtime_error(err),
        }
    })
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn hsa_signal_destroy(signal: hsa_signal_t) -> hsa_status_t {
    catch_status(|| {
        match runtime::with_runtime(|rt| rt.signal_destroy(PackedHandle::from_raw(signal.handle))) {
            Ok(Ok(())) => HSA_STATUS_SUCCESS,
            Ok(Err(err)) => map_runtime_error(err),
            Err(err) => map_runtime_error(err),
        }
    })
}

fn signal_load_impl(signal: hsa_signal_t) -> hsa_signal_value_t {
    match runtime::with_runtime(|rt| rt.signal_load(PackedHandle::from_raw(signal.handle))) {
        Ok(Ok(v)) => v,
        _ => 0,
    }
}

fn signal_store_impl(signal: hsa_signal_t, value: hsa_signal_value_t) {
    let _ =
        runtime::with_runtime(|rt| rt.signal_store(PackedHandle::from_raw(signal.handle), value));
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn hsa_signal_load_scacquire(signal: hsa_signal_t) -> hsa_signal_value_t {
    catch_unwind(AssertUnwindSafe(|| signal_load_impl(signal))).unwrap_or(0)
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn hsa_signal_load_relaxed(signal: hsa_signal_t) -> hsa_signal_value_t {
    catch_unwind(AssertUnwindSafe(|| signal_load_impl(signal))).unwrap_or(0)
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn hsa_signal_store_screlease(
    signal: hsa_signal_t,
    value: hsa_signal_value_t,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| signal_store_impl(signal, value)));
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn hsa_signal_store_relaxed(signal: hsa_signal_t, value: hsa_signal_value_t) {
    let _ = catch_unwind(AssertUnwindSafe(|| signal_store_impl(signal, value)));
}

fn signal_wait_impl(
    signal: hsa_signal_t,
    condition: hsa_signal_condition_t,
    compare_value: hsa_signal_value_t,
    timeout_hint: u64,
    wait_state_hint: hsa_wait_state_t,
) -> hsa_signal_value_t {
    // Clone Arc under the runtime lock, then wait without holding it so
    // destroy/cancel can proceed from another thread.
    let sig = match runtime::with_runtime(|rt| rt.signal_arc(PackedHandle::from_raw(signal.handle)))
    {
        Ok(Ok(sig)) => sig,
        _ => return 0,
    };
    let cond = match softgpu_core::SignalCondition::from_u32(condition) {
        Some(c) => c,
        None => return 0,
    };
    match sig.wait(cond, compare_value, timeout_hint, wait_state_hint) {
        softgpu_core::SignalWaitOutcome::Satisfied(v)
        | softgpu_core::SignalWaitOutcome::TimedOut(v)
        | softgpu_core::SignalWaitOutcome::Cancelled(v) => v,
    }
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn hsa_signal_wait_scacquire(
    signal: hsa_signal_t,
    condition: hsa_signal_condition_t,
    compare_value: hsa_signal_value_t,
    timeout_hint: u64,
    wait_state_hint: hsa_wait_state_t,
) -> hsa_signal_value_t {
    catch_unwind(AssertUnwindSafe(|| {
        signal_wait_impl(
            signal,
            condition,
            compare_value,
            timeout_hint,
            wait_state_hint,
        )
    }))
    .unwrap_or(0)
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn hsa_signal_wait_relaxed(
    signal: hsa_signal_t,
    condition: hsa_signal_condition_t,
    compare_value: hsa_signal_value_t,
    timeout_hint: u64,
    wait_state_hint: hsa_wait_state_t,
) -> hsa_signal_value_t {
    catch_unwind(AssertUnwindSafe(|| {
        signal_wait_impl(
            signal,
            condition,
            compare_value,
            timeout_hint,
            wait_state_hint,
        )
    }))
    .unwrap_or(0)
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn hsa_queue_create(
    agent: hsa_agent_t,
    size: u32,
    type_: hsa_queue_type32_t,
    _callback: Option<unsafe extern "C" fn(hsa_status_t, *mut hsa_queue_t, *mut core::ffi::c_void)>,
    _data: *mut core::ffi::c_void,
    _private_segment_size: u32,
    _group_segment_size: u32,
    queue: *mut *mut hsa_queue_t,
) -> hsa_status_t {
    catch_status(|| {
        if queue.is_null() {
            return HSA_STATUS_ERROR_INVALID_ARGUMENT;
        }
        match runtime::with_runtime(|rt| {
            rt.queue_create(PackedHandle::from_raw(agent.handle), size, type_)
        }) {
            Ok(Ok(abi)) => {
                unsafe {
                    *queue = abi;
                }
                HSA_STATUS_SUCCESS
            }
            Ok(Err(err)) => map_runtime_error(err),
            Err(err) => map_runtime_error(err),
        }
    })
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn hsa_queue_destroy(queue: *mut hsa_queue_t) -> hsa_status_t {
    catch_status(
        || match runtime::with_runtime(|rt| rt.queue_destroy(queue)) {
            Ok(Ok(())) => HSA_STATUS_SUCCESS,
            Ok(Err(err)) => map_runtime_error(err),
            Err(err) => map_runtime_error(err),
        },
    )
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn hsa_queue_load_read_index_scacquire(queue: *const hsa_queue_t) -> u64 {
    catch_unwind(AssertUnwindSafe(|| {
        runtime::with_runtime(|rt| rt.queue_load_read_index(queue))
            .ok()
            .and_then(Result::ok)
            .unwrap_or(0)
    }))
    .unwrap_or(0)
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn hsa_queue_load_read_index_relaxed(queue: *const hsa_queue_t) -> u64 {
    unsafe { hsa_queue_load_read_index_scacquire(queue) }
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn hsa_queue_load_write_index_scacquire(queue: *const hsa_queue_t) -> u64 {
    catch_unwind(AssertUnwindSafe(|| {
        runtime::with_runtime(|rt| rt.queue_load_write_index(queue))
            .ok()
            .and_then(Result::ok)
            .unwrap_or(0)
    }))
    .unwrap_or(0)
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn hsa_queue_load_write_index_relaxed(queue: *const hsa_queue_t) -> u64 {
    unsafe { hsa_queue_load_write_index_scacquire(queue) }
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn hsa_queue_store_write_index_relaxed(
    queue: *const hsa_queue_t,
    value: u64,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let _ = runtime::with_runtime(|rt| rt.queue_store_write_index(queue, value));
    }));
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn hsa_queue_store_write_index_screlease(
    queue: *const hsa_queue_t,
    value: u64,
) {
    unsafe { hsa_queue_store_write_index_relaxed(queue, value) }
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn hsa_queue_store_read_index_relaxed(queue: *const hsa_queue_t, value: u64) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let _ = runtime::with_runtime(|rt| rt.queue_store_read_index(queue, value));
    }));
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn hsa_queue_store_read_index_screlease(
    queue: *const hsa_queue_t,
    value: u64,
) {
    unsafe { hsa_queue_store_read_index_relaxed(queue, value) }
}
