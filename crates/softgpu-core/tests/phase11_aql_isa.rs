//! Phase 11: AQL dispatch → SoftGPU ISA tiny_add → kernel success.

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
        profile_id: "phase11-test".into(),
        profile_revision: "0".into(),
        identity: ProfileIdentity {
            vendor: "SoftGPU".into(),
            product_name: "Phase11 GPU".into(),
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
fn aql_dispatch_runs_registered_tiny_add() {
    let mut rt = Runtime::new(test_profile()).unwrap();
    rt.init().unwrap();
    let agent = gpu_agent(&rt);
    let fine = region(&rt, agent, MemoryViewKind::RegionFineKernarg);
    let coarse = region(&rt, agent, MemoryViewKind::RegionCoarse);

    let n = 32usize;
    let a_ptr = rt.memory_allocate(coarse, n * 4).unwrap();
    let b_ptr = rt.memory_allocate(coarse, n * 4).unwrap();
    let kernarg = rt.memory_allocate(fine, 16).unwrap();

    // Fill A and kernarg {a,b}
    unsafe {
        let a = std::slice::from_raw_parts_mut(a_ptr, n);
        // treat as i32 words
        let a32 = std::slice::from_raw_parts_mut(a_ptr as *mut i32, n);
        for (i, v) in a32.iter_mut().enumerate() {
            *v = i as i32 + 10;
        }
        let k = kernarg as *mut u64;
        *k = a_ptr as u64;
        *k.add(1) = b_ptr as u64;
        let _ = a;
    }

    let kid = rt.register_builtin_tiny_add().unwrap();
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
fn unregistered_kernel_stays_diagnostic_only() {
    let mut rt = Runtime::new(test_profile()).unwrap();
    rt.init().unwrap();
    let agent = gpu_agent(&rt);
    let q = rt
        .queue_create(agent, SOFTGPU_QUEUE_MIN_SIZE, QUEUE_TYPE_MULTI)
        .unwrap();
    let completion = rt.signal_create(1).unwrap();
    let doorbell = PackedHandle::from_raw(unsafe { (*q).doorbell_signal });
    let pkt = golden_kernel_dispatch_1d(64, 64, 0xDEAD, 0, completion.raw());
    unsafe {
        std::ptr::copy_nonoverlapping(pkt.as_ptr(), (*q).base_address, 64);
    }
    rt.queue_store_write_index(q, 1).unwrap();
    rt.signal_store(doorbell, 1).unwrap();
    let events = rt.trace_snapshot();
    assert!(events.iter().any(|e| matches!(
        e,
        TraceEvent::DiagnosticComplete { contract, .. }
            if contract == softgpu_core::DIAGNOSTIC_COMPLETE_NO_EXECUTION
    )));
    rt.queue_destroy(q).unwrap();
    rt.shut_down().unwrap();
}
