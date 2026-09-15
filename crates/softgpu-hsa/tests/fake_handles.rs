//! Forged/null HSA handles and expanded Path C surface checks.
//!
//! SoftGPU's adapter is a C ABI (no trait seam for mockall). These tests drive
//! real exports with invalid inputs (fail-closed) and exercise Path C paths the
//! smoke tests only skim.

use hsa_runtime64::*;
use softgpu_core::runtime;
use softgpu_core::DeviceProfile;
use std::path::PathBuf;
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
    unsafe { while hsa_shut_down() == HSA_STATUS_SUCCESS {} }
    runtime::install_runtime(profile).unwrap();
}

unsafe extern "C" fn capture_agent(
    agent: hsa_agent_t,
    data: *mut core::ffi::c_void,
) -> hsa_status_t {
    *(data as *mut hsa_agent_t) = agent;
    HSA_STATUS_INFO_BREAK
}

fn first_agent() -> hsa_agent_t {
    let mut agent = hsa_agent_t { handle: 0 };
    unsafe {
        assert_eq!(
            hsa_iterate_agents(Some(capture_agent), &mut agent as *mut _ as *mut _),
            HSA_STATUS_INFO_BREAK
        );
    }
    agent
}

#[test]
fn null_and_forged_handles_fail_closed() {
    let _guard = test_lock();
    reset_runtime("softgpu-generic-v0.json");
    unsafe {
        assert_eq!(hsa_init(), HSA_STATUS_SUCCESS);
        let agent = first_agent();

        assert_eq!(
            hsa_system_get_info(HSA_SYSTEM_INFO_VERSION_MAJOR, std::ptr::null_mut()),
            HSA_STATUS_ERROR_INVALID_ARGUMENT
        );
        assert_eq!(
            hsa_agent_get_info(agent, HSA_AGENT_INFO_NAME, std::ptr::null_mut()),
            HSA_STATUS_ERROR_INVALID_ARGUMENT
        );
        assert_eq!(
            hsa_agent_iterate_regions(agent, None, std::ptr::null_mut()),
            HSA_STATUS_ERROR_INVALID_ARGUMENT
        );
        assert_eq!(
            hsa_amd_agent_iterate_memory_pools(agent, None, std::ptr::null_mut()),
            HSA_STATUS_ERROR_INVALID_ARGUMENT
        );
        assert_eq!(
            hsa_queue_create(
                agent,
                64,
                0,
                None,
                std::ptr::null_mut(),
                0,
                0,
                std::ptr::null_mut()
            ),
            HSA_STATUS_ERROR_INVALID_ARGUMENT
        );
        assert_eq!(
            hsa_memory_allocate(hsa_region_t { handle: 1 }, 16, std::ptr::null_mut()),
            HSA_STATUS_ERROR_INVALID_ARGUMENT
        );
        assert_eq!(
            hsa_memory_allocate(hsa_region_t { handle: 0 }, 0, &mut std::ptr::null_mut()),
            HSA_STATUS_ERROR_INVALID_ARGUMENT
        );

        let forged_agent = hsa_agent_t {
            handle: 0xDEAD_BEEF_CAFE,
        };
        let mut tmp = 0u32;
        assert_eq!(
            hsa_agent_get_info(
                forged_agent,
                HSA_AGENT_INFO_DEVICE,
                &mut tmp as *mut _ as *mut _
            ),
            HSA_STATUS_ERROR_INVALID_AGENT
        );

        let fake_sig = hsa_signal_t { handle: 0x1111 };
        assert_eq!(hsa_signal_load_relaxed(fake_sig), 0);
        hsa_signal_store_relaxed(fake_sig, 9);
        assert_eq!(
            hsa_signal_wait_relaxed(
                fake_sig,
                HSA_SIGNAL_CONDITION_EQ,
                1,
                0,
                HSA_WAIT_STATE_ACTIVE
            ),
            0
        );
        assert_eq!(
            hsa_signal_destroy(fake_sig),
            HSA_STATUS_ERROR_INVALID_SIGNAL
        );
        assert_eq!(hsa_queue_load_read_index_relaxed(std::ptr::null()), 0);
        assert_eq!(hsa_queue_load_write_index_relaxed(std::ptr::null()), 0);

        assert_eq!(hsa_shut_down(), HSA_STATUS_SUCCESS);
    }
}

#[test]
fn path_c_system_region_pool_copy_and_relaxed_aliases() {
    let _guard = test_lock();
    reset_runtime("softgpu-generic-v0.json");
    unsafe {
        assert_eq!(hsa_init(), HSA_STATUS_SUCCESS);
        let agent = first_agent();

        let mut major = 0u16;
        assert_eq!(
            hsa_system_get_info(
                HSA_SYSTEM_INFO_VERSION_MAJOR,
                &mut major as *mut _ as *mut _
            ),
            HSA_STATUS_SUCCESS
        );
        let mut ts = 0u64;
        assert_eq!(
            hsa_system_get_info(HSA_SYSTEM_INFO_TIMESTAMP, &mut ts as *mut _ as *mut _),
            HSA_STATUS_SUCCESS
        );
        let mut endian = 0u32;
        assert_eq!(
            hsa_system_get_info(HSA_SYSTEM_INFO_ENDIANNESS, &mut endian as *mut _ as *mut _),
            HSA_STATUS_SUCCESS
        );
        let mut model = 0u32;
        assert_eq!(
            hsa_system_get_info(
                HSA_SYSTEM_INFO_MACHINE_MODEL,
                &mut model as *mut _ as *mut _
            ),
            HSA_STATUS_SUCCESS
        );
        let mut ext = [0u8; 128];
        assert_eq!(
            hsa_system_get_info(HSA_SYSTEM_INFO_EXTENSIONS, ext.as_mut_ptr() as *mut _),
            HSA_STATUS_SUCCESS
        );
        assert_eq!(
            hsa_system_get_info(0xFFFF, &mut model as *mut _ as *mut _),
            HSA_STATUS_ERROR_INVALID_ARGUMENT
        );

        let mut region = hsa_region_t { handle: 0 };
        unsafe extern "C" fn on_region(
            r: hsa_region_t,
            data: *mut core::ffi::c_void,
        ) -> hsa_status_t {
            *(data as *mut hsa_region_t) = r;
            HSA_STATUS_INFO_BREAK
        }
        assert_eq!(
            hsa_agent_iterate_regions(agent, Some(on_region), &mut region as *mut _ as *mut _),
            HSA_STATUS_INFO_BREAK
        );

        let mut seg = 0u32;
        assert_eq!(
            hsa_region_get_info(
                region,
                HSA_REGION_INFO_SEGMENT,
                &mut seg as *mut _ as *mut _
            ),
            HSA_STATUS_SUCCESS
        );
        let mut allowed = false;
        assert_eq!(
            hsa_region_get_info(
                region,
                HSA_REGION_INFO_RUNTIME_ALLOC_ALLOWED,
                &mut allowed as *mut _ as *mut _
            ),
            HSA_STATUS_SUCCESS
        );
        assert_eq!(
            hsa_region_get_info(region, 0xFFFF, &mut seg as *mut _ as *mut _),
            HSA_STATUS_ERROR_INVALID_ARGUMENT
        );

        let mut a: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut b: *mut core::ffi::c_void = std::ptr::null_mut();
        assert_eq!(hsa_memory_allocate(region, 64, &mut a), HSA_STATUS_SUCCESS);
        assert_eq!(hsa_memory_allocate(region, 64, &mut b), HSA_STATUS_SUCCESS);
        (a as *mut u8).write_bytes(0xAB, 64);
        assert_eq!(hsa_memory_copy(b, a, 64), HSA_STATUS_SUCCESS);
        assert_eq!(*(b as *const u8), 0xAB);
        assert_eq!(hsa_memory_free(a), HSA_STATUS_SUCCESS);
        assert_eq!(hsa_memory_free(b), HSA_STATUS_SUCCESS);

        let mut pool = hsa_amd_memory_pool_t { handle: 0 };
        unsafe extern "C" fn on_pool(
            p: hsa_amd_memory_pool_t,
            data: *mut core::ffi::c_void,
        ) -> hsa_status_t {
            *(data as *mut hsa_amd_memory_pool_t) = p;
            HSA_STATUS_INFO_BREAK
        }
        assert_eq!(
            hsa_amd_agent_iterate_memory_pools(agent, Some(on_pool), &mut pool as *mut _ as *mut _),
            HSA_STATUS_INFO_BREAK
        );
        let mut psize = 0usize;
        assert_eq!(
            hsa_amd_memory_pool_get_info(
                pool,
                HSA_AMD_MEMORY_POOL_INFO_SIZE,
                &mut psize as *mut _ as *mut _
            ),
            HSA_STATUS_SUCCESS
        );
        let mut access = 0u32;
        assert_eq!(
            hsa_amd_agent_memory_pool_get_info(
                agent,
                pool,
                HSA_AMD_AGENT_MEMORY_POOL_INFO_ACCESS,
                &mut access as *mut _ as *mut _
            ),
            HSA_STATUS_SUCCESS
        );
        let mut agents = [agent];
        let mut pptr: *mut core::ffi::c_void = std::ptr::null_mut();
        assert_eq!(
            hsa_amd_memory_pool_allocate(pool, 32, 0, &mut pptr),
            HSA_STATUS_SUCCESS
        );
        assert_eq!(
            hsa_amd_agents_allow_access(1, agents.as_mut_ptr(), std::ptr::null(), pptr),
            HSA_STATUS_SUCCESS
        );
        assert_eq!(hsa_amd_memory_pool_free(pptr), HSA_STATUS_SUCCESS);

        let mut min_size = 0u32;
        assert_eq!(
            hsa_agent_get_info(
                agent,
                HSA_AGENT_INFO_QUEUE_MIN_SIZE,
                &mut min_size as *mut _ as *mut _
            ),
            HSA_STATUS_SUCCESS
        );
        let mut queue: *mut hsa_queue_t = std::ptr::null_mut();
        assert_eq!(
            hsa_queue_create(
                agent,
                min_size,
                0,
                None,
                std::ptr::null_mut(),
                0,
                0,
                &mut queue
            ),
            HSA_STATUS_SUCCESS
        );
        hsa_queue_store_write_index_relaxed(queue, 3);
        assert_eq!(hsa_queue_load_write_index_relaxed(queue), 3);
        hsa_queue_store_read_index_screlease(queue, 1);
        assert_eq!(hsa_queue_load_read_index_relaxed(queue), 1);

        let mut sig = hsa_signal_t { handle: 0 };
        assert_eq!(
            hsa_signal_create(1, 0, std::ptr::null(), &mut sig),
            HSA_STATUS_SUCCESS
        );
        hsa_signal_store_relaxed(sig, 2);
        assert_eq!(hsa_signal_load_relaxed(sig), 2);
        assert_eq!(hsa_signal_destroy(sig), HSA_STATUS_SUCCESS);
        assert_eq!(hsa_queue_destroy(queue), HSA_STATUS_SUCCESS);
        assert_eq!(hsa_shut_down(), HSA_STATUS_SUCCESS);
    }
}

#[test]
fn status_string_maps_known_and_unknown_codes() {
    // SoftGPU must never panic / return empty for status strings HIP may query.
    let codes = [
        HSA_STATUS_SUCCESS,
        HSA_STATUS_INFO_BREAK,
        HSA_STATUS_ERROR,
        HSA_STATUS_ERROR_INVALID_ARGUMENT,
        HSA_STATUS_ERROR_INVALID_QUEUE_CREATION,
        HSA_STATUS_ERROR_INVALID_ALLOCATION,
        HSA_STATUS_ERROR_INVALID_AGENT,
        HSA_STATUS_ERROR_INVALID_REGION,
        HSA_STATUS_ERROR_INVALID_SIGNAL,
        HSA_STATUS_ERROR_INVALID_QUEUE,
        HSA_STATUS_ERROR_OUT_OF_RESOURCES,
        HSA_STATUS_ERROR_NOT_INITIALIZED,
        HSA_STATUS_ERROR_REFCOUNT_OVERFLOW,
        HSA_STATUS_ERROR_INVALID_MEMORY_POOL,
        0xBEEF,
    ];
    unsafe {
        for code in codes {
            let mut ptr: *const core::ffi::c_char = std::ptr::null();
            assert_eq!(hsa_status_string(code, &mut ptr), HSA_STATUS_SUCCESS);
            assert!(!ptr.is_null());
            assert!(!std::ffi::CStr::from_ptr(ptr).to_bytes().is_empty());
        }
    }
}
