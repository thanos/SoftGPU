//! Every Phase 2 advertised agent attribute must have locked provenance.

use hsa_runtime64::*;
use softgpu_core::runtime;
use softgpu_core::{CapabilityProvenance, DeviceProfile};
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

#[test]
fn advertised_agent_fields_match_profile_provenance() {
    let _guard = test_lock();
    reset_runtime("amd-radeon-ai-pro-r9700-gfx1201-v0.json");
    let profile =
        DeviceProfile::load_path(repo_profile("amd-radeon-ai-pro-r9700-gfx1201-v0.json")).unwrap();
    assert_eq!(
        profile.identity.llvm_target.provenance,
        CapabilityProvenance::Verified
    );
    assert!(!profile.conformance_allowed);

    unsafe {
        assert_eq!(hsa_init(), HSA_STATUS_SUCCESS);
        let mut agent = hsa_agent_t { handle: 0 };
        unsafe extern "C" fn capture(a: hsa_agent_t, data: *mut core::ffi::c_void) -> hsa_status_t {
            *(data as *mut hsa_agent_t) = a;
            HSA_STATUS_INFO_BREAK
        }
        assert_eq!(
            hsa_iterate_agents(Some(capture), &mut agent as *mut _ as *mut _),
            HSA_STATUS_INFO_BREAK
        );

        let mut name = [0u8; 64];
        assert_eq!(
            hsa_agent_get_info(agent, HSA_AGENT_INFO_NAME, name.as_mut_ptr() as *mut _),
            HSA_STATUS_SUCCESS
        );
        let name = std::ffi::CStr::from_bytes_until_nul(&name)
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(name, profile.identity.product_name);

        let mut vendor = [0u8; 64];
        assert_eq!(
            hsa_agent_get_info(
                agent,
                HSA_AGENT_INFO_VENDOR_NAME,
                vendor.as_mut_ptr() as *mut _
            ),
            HSA_STATUS_SUCCESS
        );
        let vendor = std::ffi::CStr::from_bytes_until_nul(&vendor)
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(vendor, profile.identity.vendor);

        let mut feature = 99u32;
        assert_eq!(
            hsa_agent_get_info(
                agent,
                HSA_AGENT_INFO_FEATURE,
                &mut feature as *mut u32 as *mut _
            ),
            HSA_STATUS_SUCCESS
        );
        assert_eq!(
            feature, HSA_AGENT_FEATURE_KERNEL_DISPATCH,
            "SoftGPU advertises KERNEL_DISPATCH for queue+AQL intercept only"
        );

        let mut device = 99u32;
        assert_eq!(
            hsa_agent_get_info(
                agent,
                HSA_AGENT_INFO_DEVICE,
                &mut device as *mut u32 as *mut _
            ),
            HSA_STATUS_SUCCESS
        );
        assert_eq!(device, HSA_DEVICE_TYPE_GPU);

        let mut major = 0u16;
        let mut minor = 0u16;
        assert_eq!(
            hsa_agent_get_info(
                agent,
                HSA_AGENT_INFO_VERSION_MAJOR,
                &mut major as *mut u16 as *mut _
            ),
            HSA_STATUS_SUCCESS
        );
        assert_eq!(
            hsa_agent_get_info(
                agent,
                HSA_AGENT_INFO_VERSION_MINOR,
                &mut minor as *mut u16 as *mut _
            ),
            HSA_STATUS_SUCCESS
        );
        assert_eq!((major, minor), (1, 2));

        // Unsupported attribute still fails closed.
        let mut junk = 0u32;
        assert_eq!(
            hsa_agent_get_info(agent, 6, &mut junk as *mut u32 as *mut _),
            HSA_STATUS_ERROR_INVALID_ARGUMENT
        );

        let events = runtime::with_runtime(|rt| rt.trace_snapshot()).unwrap();
        assert!(events.iter().any(|e| matches!(
            e,
            softgpu_core::TraceEvent::RuntimeInit {
                fidelity,
                profile_id,
                ..
            } if fidelity == "abi" && profile_id == "amd-radeon-ai-pro-r9700-gfx1201"
        )));

        assert_eq!(hsa_shut_down(), HSA_STATUS_SUCCESS);
    }
}

#[test]
fn system_get_info_version_and_rejects_unknown() {
    let _guard = test_lock();
    reset_runtime("softgpu-generic-v0.json");
    unsafe {
        assert_eq!(hsa_init(), HSA_STATUS_SUCCESS);
        let mut major = 0u16;
        assert_eq!(
            hsa_system_get_info(
                HSA_SYSTEM_INFO_VERSION_MAJOR,
                &mut major as *mut u16 as *mut _
            ),
            HSA_STATUS_SUCCESS
        );
        assert_eq!(major, 1);
        let mut amd_attr = 0u32;
        // AMD-only attribute range should fail closed in Phase 2 core surface.
        assert_eq!(
            hsa_system_get_info(0x200, &mut amd_attr as *mut u32 as *mut _),
            HSA_STATUS_ERROR_INVALID_ARGUMENT
        );
        assert_eq!(hsa_shut_down(), HSA_STATUS_SUCCESS);
    }
}
