//! SoftGPU v0.8 — HSA executable load → AQL softgpu_kernel_success.

use softgpu_amd_code_object::fixture::{fixture_tiny_add_gfx1201, fixture_tiny_add_with_text};
use softgpu_core::aql::{golden_kernel_dispatch_1d, KERNEL_SUCCESS};
use softgpu_core::handle::PackedHandle;
use softgpu_core::memory::MemoryViewKind;
use softgpu_core::profile::{ProfileField, ProfileIdentity, ResourceLimits};
use softgpu_core::queue::{QUEUE_TYPE_MULTI, SOFTGPU_QUEUE_MIN_SIZE};
use softgpu_core::runtime::Runtime;
use softgpu_core::{DeviceProfile, FidelityLevel, TraceEvent};

fn test_profile() -> DeviceProfile {
    DeviceProfile {
        schema_version: 1,
        profile_id: "phase-hip-load-test".into(),
        profile_revision: "0".into(),
        identity: ProfileIdentity {
            vendor: "SoftGPU".into(),
            product_name: "HIP load GPU".into(),
            architecture_family: "softgpu-abstract".into(),
            llvm_target: ProfileField::unknown(),
        },
        max_fidelity: FidelityLevel::ArchitecturalIsa,
        conformance_allowed: false,
        resource_limits: ResourceLimits::default(),
        analytical_performance: Default::default(),
        quirks: vec![],
    }
}

fn gpu_agent(rt: &Runtime) -> PackedHandle {
    let mut h = PackedHandle::INVALID;
    rt.iterate_agents(|a| {
        h = a.handle;
        Ok(())
    })
    .unwrap();
    h
}

fn region(rt: &Runtime, agent: PackedHandle, kind: MemoryViewKind) -> PackedHandle {
    let mut out = PackedHandle::INVALID;
    rt.iterate_regions(agent, |space| {
        if space.kind == kind {
            out = space.handle;
        }
        Ok(())
    })
    .unwrap();
    out
}

#[test]
fn executable_load_then_aql_kernel_success() {
    let mut rt = Runtime::new(test_profile()).unwrap();
    rt.init().unwrap();
    let agent = gpu_agent(&rt);
    let fine = region(&rt, agent, MemoryViewKind::RegionFineKernarg);
    let coarse = region(&rt, agent, MemoryViewKind::RegionCoarse);

    let bytes = fixture_tiny_add_with_text();
    let reader = rt.code_object_reader_create_from_memory(&bytes).unwrap();
    let exec = rt.executable_create().unwrap();
    rt.executable_load_agent_code_object(exec, reader).unwrap();
    rt.executable_freeze(exec).unwrap();
    let sym = rt.executable_get_symbol_by_name(exec, "tiny_add").unwrap();
    let kid = rt.executable_symbol_kernel_object(sym).unwrap();

    let n = 32usize;
    let a_ptr = rt.memory_allocate(coarse, n * 4).unwrap();
    let b_ptr = rt.memory_allocate(coarse, n * 4).unwrap();
    let kernarg = rt.memory_allocate(fine, 16).unwrap();
    unsafe {
        let a32 = std::slice::from_raw_parts_mut(a_ptr as *mut i32, n);
        for (i, v) in a32.iter_mut().enumerate() {
            *v = i as i32 + 10;
        }
        let k = kernarg as *mut u64;
        *k = a_ptr as u64;
        *k.add(1) = b_ptr as u64;
    }

    let q = rt
        .queue_create(agent, SOFTGPU_QUEUE_MIN_SIZE, QUEUE_TYPE_MULTI)
        .unwrap();
    let completion = rt.signal_create(1).unwrap();
    let doorbell = PackedHandle::from_raw(unsafe { (*q).doorbell_signal });
    let pkt = golden_kernel_dispatch_1d(32, n as u32, kid, kernarg as u64, completion.raw());
    unsafe {
        std::ptr::copy_nonoverlapping(pkt.as_ptr(), (*q).base_address, 64);
    }
    rt.queue_store_write_index(q, 1).unwrap();
    rt.signal_store(doorbell, 1).unwrap();

    assert_eq!(rt.signal_load(completion).unwrap(), 0);
    let events = rt.trace_snapshot();
    assert!(
        events.iter().any(|e| matches!(
            e,
            TraceEvent::DiagnosticComplete { contract, note, .. }
                if contract == KERNEL_SUCCESS && note.contains("tiny_add")
        )),
        "events={events:?}"
    );
    unsafe {
        let b32 = std::slice::from_raw_parts(b_ptr as *const i32, n);
        for (i, got) in b32.iter().enumerate() {
            assert_eq!(*got, (i as i32 + 10) + 1, "i={i}");
        }
    }
    rt.queue_destroy(q).unwrap();
    rt.shut_down().unwrap();
}

#[test]
fn load_rejects_metadata_only() {
    let mut rt = Runtime::new(test_profile()).unwrap();
    rt.init().unwrap();
    let bytes = fixture_tiny_add_gfx1201();
    let reader = rt.code_object_reader_create_from_memory(&bytes).unwrap();
    let exec = rt.executable_create().unwrap();
    assert!(rt.executable_load_agent_code_object(exec, reader).is_err());
    rt.shut_down().unwrap();
}
