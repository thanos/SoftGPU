//! Phase 4 AQL interception charter tests: validate, diagnostic complete/reject,
//! capture/replay, kernarg classification — without kernel execution.

use softgpu_core::aql::{
    golden_kernel_dispatch_1d, replay_dispatch, AqlParseError, PacketType,
    DIAGNOSTIC_COMPLETE_NO_EXECUTION, DIAGNOSTIC_REJECTED, PACKET_TYPE_AGENT_DISPATCH,
    PACKET_TYPE_BARRIER_AND,
};
use softgpu_core::handle::PackedHandle;
use softgpu_core::memory::MemoryViewKind;
use softgpu_core::profile::{ProfileField, ProfileIdentity, ResourceLimits};
use softgpu_core::queue::{QUEUE_TYPE_MULTI, SOFTGPU_QUEUE_MIN_SIZE};
use softgpu_core::runtime::Runtime;
use softgpu_core::{DeviceProfile, FidelityLevel, KernargClass, TraceEvent};

fn test_profile() -> DeviceProfile {
    DeviceProfile {
        schema_version: 1,
        profile_id: "phase4-test".into(),
        profile_revision: "0".into(),
        identity: ProfileIdentity {
            vendor: "SoftGPU".into(),
            product_name: "Phase4 GPU".into(),
            architecture_family: "softgpu-abstract".into(),
            llvm_target: ProfileField::unknown(),
        },
        max_fidelity: FidelityLevel::Abi,
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

fn fine_region(rt: &Runtime, agent: PackedHandle) -> PackedHandle {
    let mut region = PackedHandle::INVALID;
    rt.iterate_regions(agent, |space| {
        if space.kind == MemoryViewKind::RegionFineKernarg {
            region = space.handle;
        }
        Ok(())
    })
    .unwrap();
    region
}

#[test]
fn diagnostic_complete_advances_read_index_and_completion() {
    let mut rt = Runtime::new(test_profile()).unwrap();
    rt.init().unwrap();
    let agent = gpu_agent(&rt);
    let q = rt
        .queue_create(agent, SOFTGPU_QUEUE_MIN_SIZE, QUEUE_TYPE_MULTI)
        .unwrap();
    let completion = rt.signal_create(1).unwrap();
    let doorbell = PackedHandle::from_raw(unsafe { (*q).doorbell_signal });
    let pkt = golden_kernel_dispatch_1d(64, 256, 0xABCD, 0, completion.raw());
    unsafe {
        std::ptr::copy_nonoverlapping(pkt.as_ptr(), (*q).base_address, 64);
    }
    rt.queue_store_write_index(q, 1).unwrap();
    rt.signal_store(doorbell, 1).unwrap();

    assert_eq!(rt.signal_load(completion).unwrap(), 0);
    assert_eq!(rt.queue_load_read_index(q).unwrap(), 1);
    assert!(!rt.dispatch_captures().is_empty());
    let cap = &rt.dispatch_captures()[0];
    assert_eq!(cap.kernel_object, 0xABCD);
    assert_eq!(cap.grid_size, [256, 1, 1]);

    let events = rt.trace_snapshot();
    assert!(events.iter().any(|e| matches!(
        e,
        TraceEvent::DiagnosticComplete {
            contract,
            note,
            ..
        } if contract == DIAGNOSTIC_COMPLETE_NO_EXECUTION && note == "not_kernel_success"
    )));
    assert!(events
        .iter()
        .any(|e| matches!(e, TraceEvent::DispatchValidated { .. })));

    // Slot marked INVALID after processor progress.
    unsafe {
        assert_eq!(*(*q).base_address, 1); // PACKET_TYPE_INVALID
    }
    rt.queue_destroy(q).unwrap();
    rt.shut_down().unwrap();
}

#[test]
fn malformed_packet_diagnostic_reject_no_completion_store() {
    let mut rt = Runtime::new(test_profile()).unwrap();
    rt.init().unwrap();
    let agent = gpu_agent(&rt);
    let q = rt
        .queue_create(agent, SOFTGPU_QUEUE_MIN_SIZE, QUEUE_TYPE_MULTI)
        .unwrap();
    let completion = rt.signal_create(1).unwrap();
    let doorbell = PackedHandle::from_raw(unsafe { (*q).doorbell_signal });
    // Type KERNEL_DISPATCH but zero workgroup → reject.
    let mut bad = golden_kernel_dispatch_1d(0, 256, 1, 0, completion.raw());
    bad[0] = 2; // keep type
    unsafe {
        std::ptr::copy_nonoverlapping(bad.as_ptr(), (*q).base_address, 64);
    }
    rt.queue_store_write_index(q, 1).unwrap();
    rt.signal_store(doorbell, 1).unwrap();

    assert_eq!(rt.signal_load(completion).unwrap(), 1); // unchanged
    assert_eq!(rt.queue_load_read_index(q).unwrap(), 1); // protocol still advances
    assert!(rt.trace_snapshot().iter().any(|e| matches!(
        e,
        TraceEvent::DispatchRejected { contract, .. } if contract == DIAGNOSTIC_REJECTED
    )));
    rt.queue_destroy(q).unwrap();
    rt.shut_down().unwrap();
}

#[test]
fn unsupported_agent_dispatch_rejected() {
    let mut rt = Runtime::new(test_profile()).unwrap();
    rt.init().unwrap();
    let agent = gpu_agent(&rt);
    let q = rt
        .queue_create(agent, SOFTGPU_QUEUE_MIN_SIZE, QUEUE_TYPE_MULTI)
        .unwrap();
    let doorbell = PackedHandle::from_raw(unsafe { (*q).doorbell_signal });
    let mut bytes = [0u8; 64];
    bytes[0] = PACKET_TYPE_AGENT_DISPATCH as u8;
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), (*q).base_address, 64);
    }
    rt.queue_store_write_index(q, 1).unwrap();
    rt.signal_store(doorbell, 1).unwrap();
    assert!(rt.trace_snapshot().iter().any(|e| matches!(
        e,
        TraceEvent::DispatchRejected {
            packet_type,
            ..
        } if *packet_type == PACKET_TYPE_AGENT_DISPATCH
    )));
    rt.queue_destroy(q).unwrap();
    rt.shut_down().unwrap();
}

#[test]
fn barrier_minimal_diagnostic_complete() {
    let mut rt = Runtime::new(test_profile()).unwrap();
    rt.init().unwrap();
    let agent = gpu_agent(&rt);
    let q = rt
        .queue_create(agent, SOFTGPU_QUEUE_MIN_SIZE, QUEUE_TYPE_MULTI)
        .unwrap();
    let completion = rt.signal_create(1).unwrap();
    let doorbell = PackedHandle::from_raw(unsafe { (*q).doorbell_signal });
    let mut bytes = [0u8; 64];
    bytes[0] = PACKET_TYPE_BARRIER_AND as u8;
    bytes[56..64].copy_from_slice(&completion.raw().to_le_bytes());
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), (*q).base_address, 64);
    }
    rt.queue_store_write_index(q, 1).unwrap();
    rt.signal_store(doorbell, 1).unwrap();
    assert_eq!(rt.signal_load(completion).unwrap(), 0);
    let cap = rt.dispatch_captures().last().unwrap();
    assert_eq!(cap.packet_type, PacketType::BarrierAnd);
    rt.queue_destroy(q).unwrap();
    rt.shut_down().unwrap();
}

#[test]
fn kernarg_softgpu_vs_foreign_and_replay() {
    let mut rt = Runtime::new(test_profile()).unwrap();
    rt.init().unwrap();
    let agent = gpu_agent(&rt);
    let region = fine_region(&rt, agent);
    let kernarg = rt.memory_allocate(region, 64).unwrap();
    let q = rt
        .queue_create(agent, SOFTGPU_QUEUE_MIN_SIZE, QUEUE_TYPE_MULTI)
        .unwrap();
    let doorbell = PackedHandle::from_raw(unsafe { (*q).doorbell_signal });

    let soft = golden_kernel_dispatch_1d(32, 32, 0x42, kernarg as u64, 0);
    unsafe {
        std::ptr::copy_nonoverlapping(soft.as_ptr(), (*q).base_address, 64);
    }
    rt.queue_store_write_index(q, 1).unwrap();
    rt.signal_store(doorbell, 1).unwrap();
    let soft_cap = rt.dispatch_captures().last().unwrap().clone();
    assert!(matches!(soft_cap.kernarg, KernargClass::SoftGpu { .. }));

    let foreign = golden_kernel_dispatch_1d(32, 32, 0x43, 0xDEAD_BEEF, 0);
    unsafe {
        // Next logical packet index is 1 → physical slot 1.
        std::ptr::copy_nonoverlapping(foreign.as_ptr(), (*q).base_address.add(64), 64);
    }
    rt.queue_store_write_index(q, 2).unwrap();
    rt.signal_store(doorbell, 2).unwrap();
    let foreign_cap = rt.dispatch_captures().last().unwrap().clone();
    assert!(matches!(
        foreign_cap.kernarg,
        KernargClass::ForeignOpaque { addr: 0xDEAD_BEEF }
    ));

    // Offline replay does not need the live runtime or SoftGPU ownership table.
    let replayed = replay_dispatch(&soft_cap.captured_bytes, soft_cap.packet_index).unwrap();
    assert_eq!(replayed.kernel_object, soft_cap.kernel_object);
    assert_eq!(replayed.grid_size, soft_cap.grid_size);
    assert!(matches!(
        replayed.kernarg,
        KernargClass::ForeignOpaque { .. }
    ));

    rt.memory_free(kernarg).unwrap();
    rt.queue_destroy(q).unwrap();
    rt.shut_down().unwrap();
}

#[test]
fn field_mutation_overflow_rejected() {
    let mut bytes = golden_kernel_dispatch_1d(64, 256, 1, 0, 0);
    // grid_x < workgroup_x
    bytes[12..16].copy_from_slice(&32u32.to_le_bytes());
    let err = softgpu_core::parse_kernel_dispatch(&bytes, 0, |_| KernargClass::Null).unwrap_err();
    assert!(matches!(
        err,
        AqlParseError::GridSmallerThanWorkgroup { dim: 0 }
    ));

    // dims=1 with workgroup_y != 1
    let mut bytes = golden_kernel_dispatch_1d(64, 256, 1, 0, 0);
    bytes[6..8].copy_from_slice(&2u16.to_le_bytes());
    assert!(softgpu_core::parse_kernel_dispatch(&bytes, 0, |_| KernargClass::Null).is_err());
}
