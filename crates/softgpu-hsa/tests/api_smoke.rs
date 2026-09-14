//! Phase 1/2 HSA adapter smoke tests (linked via rlib, not HIP).

use hsa_runtime64::*;
use softgpu_core::runtime;
use softgpu_core::DeviceProfile;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

fn repo_profile(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../profiles")
        .join(name)
}

fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|p| p.into_inner())
}

fn reset_runtime(profile_file: &str) {
    let profile = DeviceProfile::load_path(repo_profile(profile_file)).unwrap();
    // Drain any leftover refcount from other tests.
    unsafe { while hsa_shut_down() == HSA_STATUS_SUCCESS {} }
    runtime::install_runtime(profile).unwrap();
}

#[test]
fn init_iterate_gpu_agent_and_shutdown() {
    let _guard = test_lock();
    reset_runtime("amd-radeon-ai-pro-r9700-gfx1201-v0.json");

    unsafe {
        assert_eq!(hsa_init(), HSA_STATUS_SUCCESS);

        static COUNT: AtomicUsize = AtomicUsize::new(0);
        COUNT.store(0, Ordering::SeqCst);
        unsafe extern "C" fn cb(agent: hsa_agent_t, _data: *mut core::ffi::c_void) -> hsa_status_t {
            COUNT.fetch_add(1, Ordering::SeqCst);
            let mut device: u32 = 0xFFFF;
            let st = hsa_agent_get_info(
                agent,
                HSA_AGENT_INFO_DEVICE,
                &mut device as *mut u32 as *mut _,
            );
            assert_eq!(st, HSA_STATUS_SUCCESS);
            assert_eq!(device, HSA_DEVICE_TYPE_GPU);

            let mut name = [0u8; 64];
            let st = hsa_agent_get_info(agent, HSA_AGENT_INFO_NAME, name.as_mut_ptr() as *mut _);
            assert_eq!(st, HSA_STATUS_SUCCESS);
            let cstr = std::ffi::CStr::from_bytes_until_nul(&name).unwrap();
            assert!(cstr.to_bytes().starts_with(b"Radeon"));

            let mut feature: u32 = 1;
            let st = hsa_agent_get_info(
                agent,
                HSA_AGENT_INFO_FEATURE,
                &mut feature as *mut u32 as *mut _,
            );
            assert_eq!(st, HSA_STATUS_SUCCESS);
            assert_eq!(feature, 0, "Phase 2 must not claim dispatch features");
            HSA_STATUS_SUCCESS
        }

        assert_eq!(
            hsa_iterate_agents(Some(cb), std::ptr::null_mut()),
            HSA_STATUS_SUCCESS
        );
        assert_eq!(COUNT.load(Ordering::SeqCst), 1);
        assert_eq!(hsa_shut_down(), HSA_STATUS_SUCCESS);
    }
}

#[test]
fn unsupported_attribute_fails_closed() {
    let _guard = test_lock();
    reset_runtime("softgpu-generic-v0.json");
    unsafe {
        assert_eq!(hsa_init(), HSA_STATUS_SUCCESS);
        let mut agent_slot = hsa_agent_t { handle: 0 };
        unsafe extern "C" fn capture(
            agent: hsa_agent_t,
            data: *mut core::ffi::c_void,
        ) -> hsa_status_t {
            let out = data as *mut hsa_agent_t;
            *out = agent;
            HSA_STATUS_INFO_BREAK
        }
        let st = hsa_iterate_agents(Some(capture), &mut agent_slot as *mut _ as *mut _);
        assert_eq!(st, HSA_STATUS_INFO_BREAK);

        let mut tmp: u32 = 0;
        // HSA_AGENT_INFO_WAVEFRONT_SIZE = 6 — intentionally unimplemented in Phase 2.
        let st = hsa_agent_get_info(agent_slot, 6, &mut tmp as *mut u32 as *mut _);
        assert_eq!(st, HSA_STATUS_ERROR_INVALID_ARGUMENT);

        let forged = hsa_agent_t {
            handle: 0x1111_2222_3333_4444,
        };
        let st = hsa_agent_get_info(forged, HSA_AGENT_INFO_NAME, &mut tmp as *mut u32 as *mut _);
        assert_eq!(st, HSA_STATUS_ERROR_INVALID_AGENT);

        assert_eq!(hsa_shut_down(), HSA_STATUS_SUCCESS);
    }
}

#[test]
fn not_initialized_errors() {
    let _guard = test_lock();
    reset_runtime("softgpu-generic-v0.json");
    unsafe {
        while hsa_shut_down() == HSA_STATUS_SUCCESS {}
        assert_eq!(
            hsa_iterate_agents(None, std::ptr::null_mut()),
            HSA_STATUS_ERROR_INVALID_ARGUMENT
        );
        assert_eq!(hsa_init(), HSA_STATUS_SUCCESS);
        assert_eq!(hsa_shut_down(), HSA_STATUS_SUCCESS);
        assert_eq!(hsa_shut_down(), HSA_STATUS_ERROR_NOT_INITIALIZED);
    }
}
