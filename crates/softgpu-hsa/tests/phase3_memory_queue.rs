//! Phase 3 Path C memory + signals + queue observe smoke tests.

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

#[test]
fn path_c_region_and_pool_allocate() {
    let _guard = test_lock();
    reset_runtime("softgpu-generic-v0.json");
    unsafe {
        assert_eq!(hsa_init(), HSA_STATUS_SUCCESS);
        let mut agent = hsa_agent_t { handle: 0 };
        assert_eq!(
            hsa_iterate_agents(Some(capture_agent), &mut agent as *mut _ as *mut _),
            HSA_STATUS_INFO_BREAK
        );

        let mut region = hsa_region_t { handle: 0 };
        unsafe extern "C" fn on_region(
            r: hsa_region_t,
            data: *mut core::ffi::c_void,
        ) -> hsa_status_t {
            let out = data as *mut hsa_region_t;
            if (*out).handle == 0 {
                *out = r;
            }
            HSA_STATUS_SUCCESS
        }
        assert_eq!(
            hsa_agent_iterate_regions(agent, Some(on_region), &mut region as *mut _ as *mut _),
            HSA_STATUS_SUCCESS
        );
        assert_ne!(region.handle, 0);

        let mut ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        assert_eq!(
            hsa_memory_allocate(region, 1024, &mut ptr),
            HSA_STATUS_SUCCESS
        );
        assert!(!ptr.is_null());
        assert_eq!(hsa_memory_free(ptr), HSA_STATUS_SUCCESS);

        let mut pool = hsa_amd_memory_pool_t { handle: 0 };
        unsafe extern "C" fn on_pool(
            p: hsa_amd_memory_pool_t,
            data: *mut core::ffi::c_void,
        ) -> hsa_status_t {
            let out = data as *mut hsa_amd_memory_pool_t;
            if (*out).handle == 0 {
                *out = p;
            }
            HSA_STATUS_SUCCESS
        }
        assert_eq!(
            hsa_amd_agent_iterate_memory_pools(agent, Some(on_pool), &mut pool as *mut _ as *mut _),
            HSA_STATUS_SUCCESS
        );
        assert_ne!(pool.handle, 0);
        let mut ptr2: *mut core::ffi::c_void = std::ptr::null_mut();
        assert_eq!(
            hsa_amd_memory_pool_allocate(pool, 2048, 0, &mut ptr2),
            HSA_STATUS_SUCCESS
        );
        assert_eq!(hsa_amd_memory_pool_free(ptr2), HSA_STATUS_SUCCESS);

        assert_eq!(hsa_shut_down(), HSA_STATUS_SUCCESS);
    }
}

#[test]
fn signal_wait_and_queue_doorbell_observe() {
    let _guard = test_lock();
    reset_runtime("softgpu-generic-v0.json");
    unsafe {
        assert_eq!(hsa_init(), HSA_STATUS_SUCCESS);
        let mut agent = hsa_agent_t { handle: 0 };
        assert_eq!(
            hsa_iterate_agents(Some(capture_agent), &mut agent as *mut _ as *mut _),
            HSA_STATUS_INFO_BREAK
        );

        let mut signal = hsa_signal_t { handle: 0 };
        assert_eq!(
            hsa_signal_create(0, 0, std::ptr::null(), &mut signal),
            HSA_STATUS_SUCCESS
        );
        hsa_signal_store_screlease(signal, 5);
        assert_eq!(hsa_signal_load_scacquire(signal), 5);
        let observed = hsa_signal_wait_scacquire(
            signal,
            HSA_SIGNAL_CONDITION_EQ,
            5,
            1_000_000_000,
            HSA_WAIT_STATE_ACTIVE,
        );
        assert_eq!(observed, 5);
        assert_eq!(hsa_signal_destroy(signal), HSA_STATUS_SUCCESS);

        let mut min_size = 0u32;
        assert_eq!(
            hsa_agent_get_info(
                agent,
                HSA_AGENT_INFO_QUEUE_MIN_SIZE,
                &mut min_size as *mut u32 as *mut _
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
        assert!(!queue.is_null());
        assert_eq!((*queue).size, min_size);
        assert_ne!((*queue).base_address as usize, 0);

        let doorbell = (*queue).doorbell_signal;
        hsa_signal_store_screlease(hsa_signal_t { handle: doorbell }, 1);
        assert_eq!(hsa_queue_load_write_index_scacquire(queue), 0);
        hsa_queue_store_write_index_screlease(queue, 1);
        assert_eq!(hsa_queue_load_write_index_scacquire(queue), 1);

        let events = runtime::with_runtime(|rt| rt.trace_snapshot()).unwrap();
        assert!(events
            .iter()
            .any(|e| matches!(e, softgpu_core::TraceEvent::QueueDoorbell { .. })));
        assert!(events
            .iter()
            .any(|e| matches!(e, softgpu_core::TraceEvent::QueueCreate { .. })));

        assert_eq!(hsa_queue_destroy(queue), HSA_STATUS_SUCCESS);
        assert_eq!(hsa_shut_down(), HSA_STATUS_SUCCESS);
    }
}
