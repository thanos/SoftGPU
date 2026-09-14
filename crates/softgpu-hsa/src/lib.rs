//! SoftGPU ROCr/HSA adapter (`libhsa-runtime64`).
//!
//! Phase 1/2 surface: `hsa_init`, `hsa_shut_down`, `hsa_system_get_info` (subset),
//! `hsa_iterate_agents`, `hsa_agent_get_info`, and `hsa_status_string`. Remaining
//! APIs are fail-closed generated C stubs.
//!
//! # Panic / unwind policy
//!
//! Every exported function wraps its body in `catch_unwind`. Panics become
//! `HSA_STATUS_ERROR` and never unwind across the FFI boundary.

#![allow(non_camel_case_types)]

pub mod ffi;
pub mod status;

use softgpu_core::runtime::{self, RuntimeError};
use softgpu_core::{AgentInfoAttr, AgentKind, DeviceProfile, PackedHandle};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::time::Instant;

pub use ffi::{
    hsa_agent_info_t, hsa_agent_t, hsa_device_type_t, hsa_status_t, hsa_system_info_t,
    HSA_AGENT_INFO_DEVICE, HSA_AGENT_INFO_FEATURE, HSA_AGENT_INFO_NAME, HSA_AGENT_INFO_VENDOR_NAME,
    HSA_AGENT_INFO_VERSION_MAJOR, HSA_AGENT_INFO_VERSION_MINOR, HSA_DEVICE_TYPE_CPU,
    HSA_DEVICE_TYPE_GPU, HSA_ENDIANNESS_LITTLE, HSA_MACHINE_MODEL_LARGE, HSA_STATUS_ERROR,
    HSA_STATUS_ERROR_INVALID_AGENT, HSA_STATUS_ERROR_INVALID_ARGUMENT,
    HSA_STATUS_ERROR_NOT_INITIALIZED, HSA_STATUS_ERROR_REFCOUNT_OVERFLOW, HSA_STATUS_INFO_BREAK,
    HSA_STATUS_SUCCESS, HSA_SYSTEM_INFO_ENDIANNESS, HSA_SYSTEM_INFO_EXTENSIONS,
    HSA_SYSTEM_INFO_MACHINE_MODEL, HSA_SYSTEM_INFO_SIGNAL_MAX_WAIT, HSA_SYSTEM_INFO_TIMESTAMP,
    HSA_SYSTEM_INFO_TIMESTAMP_FREQUENCY, HSA_SYSTEM_INFO_VERSION_MAJOR,
    HSA_SYSTEM_INFO_VERSION_MINOR,
};

fn map_runtime_error(err: RuntimeError) -> hsa_status_t {
    match err {
        RuntimeError::NotInitialized => HSA_STATUS_ERROR_NOT_INITIALIZED,
        RuntimeError::RefcountOverflow => HSA_STATUS_ERROR_REFCOUNT_OVERFLOW,
        RuntimeError::InvalidArgument | RuntimeError::UnsupportedAttribute => {
            HSA_STATUS_ERROR_INVALID_ARGUMENT
        }
        RuntimeError::InvalidAgent => HSA_STATUS_ERROR_INVALID_AGENT,
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
        ffi::HSA_AGENT_INFO_NAME => Some(AgentInfoAttr::Name),
        ffi::HSA_AGENT_INFO_VENDOR_NAME => Some(AgentInfoAttr::VendorName),
        ffi::HSA_AGENT_INFO_FEATURE => Some(AgentInfoAttr::Feature),
        ffi::HSA_AGENT_INFO_DEVICE => Some(AgentInfoAttr::Device),
        ffi::HSA_AGENT_INFO_VERSION_MAJOR => Some(AgentInfoAttr::VersionMajor),
        ffi::HSA_AGENT_INFO_VERSION_MINOR => Some(AgentInfoAttr::VersionMinor),
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
    // SAFETY: caller buffer must hold a u32 for this attribute.
    unsafe {
        *(dest as *mut u32) = value;
    }
    HSA_STATUS_SUCCESS
}

fn write_u16(dest: *mut core::ffi::c_void, value: u16) -> hsa_status_t {
    if dest.is_null() {
        return HSA_STATUS_ERROR_INVALID_ARGUMENT;
    }
    // SAFETY: caller buffer must hold a u16 for this attribute.
    unsafe {
        *(dest as *mut u16) = value;
    }
    HSA_STATUS_SUCCESS
}

fn write_u64(dest: *mut core::ffi::c_void, value: u64) -> hsa_status_t {
    if dest.is_null() {
        return HSA_STATUS_ERROR_INVALID_ARGUMENT;
    }
    // SAFETY: caller buffer must hold a u64 for this attribute.
    unsafe {
        *(dest as *mut u64) = value;
    }
    HSA_STATUS_SUCCESS
}

fn soft_clock_ns() -> u64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let start = START.get_or_init(Instant::now);
    u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX)
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
        match runtime::with_runtime(|rt| {
            if !rt.is_initialized() {
                Err(RuntimeError::NotInitialized)
            } else {
                Ok(())
            }
        }) {
            Ok(Ok(())) => {}
            Ok(Err(err)) | Err(err) => return map_runtime_error(err),
        }

        match attribute {
            HSA_SYSTEM_INFO_VERSION_MAJOR => write_u16(value, 1),
            HSA_SYSTEM_INFO_VERSION_MINOR => write_u16(value, 2),
            HSA_SYSTEM_INFO_TIMESTAMP => write_u64(value, soft_clock_ns()),
            // Provisional SoftGPU software clock (not hardware). Not for conformance.
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
                // SAFETY: HSA requires uint8_t[128]; SoftGPU advertises no extensions yet.
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
                softgpu_core::runtime::AgentInfoValue::Name(s)
                | softgpu_core::runtime::AgentInfoValue::VendorName(s) => {
                    write_c_string64(value, &s)
                }
                softgpu_core::runtime::AgentInfoValue::Feature(mask) => write_u32(value, mask),
                softgpu_core::runtime::AgentInfoValue::Device(kind) => {
                    let device = match kind {
                        AgentKind::Cpu => ffi::HSA_DEVICE_TYPE_CPU,
                        AgentKind::Gpu => ffi::HSA_DEVICE_TYPE_GPU,
                    };
                    write_u32(value, device)
                }
                softgpu_core::runtime::AgentInfoValue::U16(v) => write_u16(value, v),
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
