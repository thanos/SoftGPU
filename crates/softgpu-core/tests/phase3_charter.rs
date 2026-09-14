//! Phase 3 charter completion tests: wraparound, ordering, exhaustion,
//! timeouts, stale handles, destroy-during-wait, concurrent stress, caps.

use softgpu_core::aql::golden_kernel_dispatch_1d;
use softgpu_core::handle::{HandleKind, PackedHandle};
use softgpu_core::memory::{SoftGpuAllocator, SOFTGPU_ALLOC_ALIGNMENT, SOFTGPU_ALLOC_GRANULE};
use softgpu_core::profile::{ProfileField, ProfileIdentity, ResourceLimits};
use softgpu_core::queue::{QUEUE_TYPE_MULTI, SOFTGPU_QUEUES_MAX, SOFTGPU_QUEUE_MIN_SIZE};
use softgpu_core::runtime::{Runtime, RuntimeError};
use softgpu_core::signal::SignalWaitOutcome;
use softgpu_core::{DeviceProfile, FidelityLevel};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::Duration;

fn test_profile() -> DeviceProfile {
    DeviceProfile {
        schema_version: 1,
        profile_id: "charter-test".into(),
        profile_revision: "0".into(),
        identity: ProfileIdentity {
            vendor: "SoftGPU".into(),
            product_name: "Charter GPU".into(),
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

#[test]
fn malformed_queue_sizes_fail_closed() {
    let mut rt = Runtime::new(test_profile()).unwrap();
    rt.init().unwrap();
    let agent = gpu_agent(&rt);
    assert_eq!(
        rt.queue_create(agent, 0, QUEUE_TYPE_MULTI).unwrap_err(),
        RuntimeError::InvalidArgument
    );
    assert_eq!(
        rt.queue_create(agent, 3, QUEUE_TYPE_MULTI).unwrap_err(),
        RuntimeError::InvalidArgument
    );
    assert_eq!(
        rt.queue_create(agent, SOFTGPU_QUEUE_MIN_SIZE, 2)
            .unwrap_err(),
        RuntimeError::InvalidQueueCreation
    );
    rt.shut_down().unwrap();
}

#[test]
fn allocation_exhaustion_and_lookup_metadata() {
    let mut alloc = SoftGpuAllocator::new(SOFTGPU_ALLOC_GRANULE * 2);
    let space = PackedHandle::pack(HandleKind::Region, 1, 0);
    let a = alloc
        .allocate(
            space,
            SOFTGPU_ALLOC_GRANULE,
            SOFTGPU_ALLOC_GRANULE,
            SOFTGPU_ALLOC_ALIGNMENT,
            SOFTGPU_ALLOC_GRANULE * 2,
        )
        .unwrap();
    let _b = alloc
        .allocate(
            space,
            SOFTGPU_ALLOC_GRANULE,
            SOFTGPU_ALLOC_GRANULE,
            SOFTGPU_ALLOC_ALIGNMENT,
            SOFTGPU_ALLOC_GRANULE * 2,
        )
        .unwrap();
    assert!(alloc
        .allocate(
            space,
            SOFTGPU_ALLOC_GRANULE,
            SOFTGPU_ALLOC_GRANULE,
            SOFTGPU_ALLOC_ALIGNMENT,
            SOFTGPU_ALLOC_GRANULE * 2,
        )
        .is_err());
    let meta = alloc.lookup(a).unwrap();
    assert!(meta.host_readable && meta.host_writable);
    assert_eq!(meta.space, space);
    alloc.free(a).unwrap();
}

#[test]
fn signal_timeout_and_stale_after_destroy() {
    let mut rt = Runtime::new(test_profile()).unwrap();
    rt.init().unwrap();
    let sig = rt.signal_create(0).unwrap();
    match rt.signal_wait(sig, 0, 1, 500_000, 1).unwrap() {
        SignalWaitOutcome::TimedOut(v) => assert_eq!(v, 0),
        other => panic!("expected timeout, got {other:?}"),
    }
    rt.signal_destroy(sig).unwrap();
    assert_eq!(
        rt.signal_load(sig).unwrap_err(),
        RuntimeError::InvalidSignal
    );
    rt.shut_down().unwrap();
}

#[test]
fn queue_destroy_cancels_doorbell_waiters() {
    let mut rt = Runtime::new(test_profile()).unwrap();
    rt.init().unwrap();
    let agent = gpu_agent(&rt);
    let q = rt
        .queue_create(agent, SOFTGPU_QUEUE_MIN_SIZE, QUEUE_TYPE_MULTI)
        .unwrap();
    let doorbell = PackedHandle::from_raw(unsafe { (*q).doorbell_signal });
    let sig = rt.signal_arc(doorbell).unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let b2 = Arc::clone(&barrier);
    let waiter = thread::spawn(move || {
        b2.wait();
        sig.wait(softgpu_core::SignalCondition::Eq, 99, u64::MAX, 1)
    });
    barrier.wait();
    thread::sleep(Duration::from_millis(5));
    rt.queue_destroy(q).unwrap();
    let outcome = waiter.join().unwrap();
    assert!(matches!(outcome, SignalWaitOutcome::Cancelled(_)));
    rt.shut_down().unwrap();
}

#[test]
fn packet_observed_exactly_once_with_ordering() {
    let mut rt = Runtime::new(test_profile()).unwrap();
    rt.init().unwrap();
    let agent = gpu_agent(&rt);
    let q = rt
        .queue_create(agent, SOFTGPU_QUEUE_MIN_SIZE, QUEUE_TYPE_MULTI)
        .unwrap();
    let doorbell = PackedHandle::from_raw(unsafe { (*q).doorbell_signal });
    let pkt = golden_kernel_dispatch_1d(64, 256, 0x1000, 0, 0);
    unsafe {
        let base = (*q).base_address;
        std::ptr::copy_nonoverlapping(pkt.as_ptr(), base, 64);
        std::ptr::copy_nonoverlapping(pkt.as_ptr(), base.add(64), 64);
    }
    rt.queue_store_write_index(q, 2).unwrap();
    rt.signal_store(doorbell, 1).unwrap();
    let observed: Vec<_> = rt
        .trace_snapshot()
        .iter()
        .filter_map(|e| match e {
            softgpu_core::TraceEvent::PacketObserved { packet_index, .. } => Some(*packet_index),
            _ => None,
        })
        .collect();
    assert_eq!(observed, vec![0, 1]);
    rt.signal_store(doorbell, 2).unwrap();
    let observed2: Vec<_> = rt
        .trace_snapshot()
        .iter()
        .filter_map(|e| match e {
            softgpu_core::TraceEvent::PacketObserved { packet_index, .. } => Some(*packet_index),
            _ => None,
        })
        .collect();
    assert_eq!(observed2, vec![0, 1]);
    rt.queue_destroy(q).unwrap();
    rt.shut_down().unwrap();
}

#[test]
fn wraparound_packet_indexes() {
    let mut rt = Runtime::new(test_profile()).unwrap();
    rt.init().unwrap();
    let agent = gpu_agent(&rt);
    let q = rt
        .queue_create(agent, SOFTGPU_QUEUE_MIN_SIZE, QUEUE_TYPE_MULTI)
        .unwrap();
    let doorbell = PackedHandle::from_raw(unsafe { (*q).doorbell_signal });
    let n = u64::from(SOFTGPU_QUEUE_MIN_SIZE);
    let pkt = golden_kernel_dispatch_1d(64, 256, 0x1000, 0, 0);
    unsafe {
        let base = (*q).base_address;
        for i in 0..n {
            let p = base.add((i as usize) * 64);
            std::ptr::copy_nonoverlapping(pkt.as_ptr(), p, 64);
        }
    }
    rt.queue_store_write_index(q, n).unwrap();
    rt.signal_store(doorbell, 1).unwrap();
    unsafe {
        let base = (*q).base_address;
        std::ptr::copy_nonoverlapping(pkt.as_ptr(), base, 64);
    }
    rt.queue_store_write_index(q, n + 1).unwrap();
    rt.signal_store(doorbell, 2).unwrap();
    let idxs: Vec<_> = rt
        .trace_snapshot()
        .iter()
        .filter_map(|e| match e {
            softgpu_core::TraceEvent::PacketObserved { packet_index, .. } => Some(*packet_index),
            _ => None,
        })
        .collect();
    assert_eq!(idxs.len(), (n + 1) as usize);
    assert_eq!(*idxs.last().unwrap(), n);
    rt.queue_destroy(q).unwrap();
    rt.shut_down().unwrap();
}

#[test]
fn queue_cap_resource_limit() {
    let mut rt = Runtime::new(test_profile()).unwrap();
    rt.init().unwrap();
    let agent = gpu_agent(&rt);
    let mut queues = Vec::new();
    for _ in 0..SOFTGPU_QUEUES_MAX {
        queues.push(
            rt.queue_create(agent, SOFTGPU_QUEUE_MIN_SIZE, QUEUE_TYPE_MULTI)
                .unwrap(),
        );
    }
    assert_eq!(
        rt.queue_create(agent, SOFTGPU_QUEUE_MIN_SIZE, QUEUE_TYPE_MULTI)
            .unwrap_err(),
        RuntimeError::OutOfResources
    );
    for q in queues {
        rt.queue_destroy(q).unwrap();
    }
    rt.shut_down().unwrap();
}

#[test]
fn concurrent_producer_consumer_stress() {
    let mut rt = Runtime::new(test_profile()).unwrap();
    rt.init().unwrap();
    let agent = gpu_agent(&rt);
    let q = rt
        .queue_create(agent, SOFTGPU_QUEUE_MIN_SIZE, QUEUE_TYPE_MULTI)
        .unwrap();
    let doorbell = PackedHandle::from_raw(unsafe { (*q).doorbell_signal });
    let q_addr = q as usize;
    let shared = Mutex::new(rt);
    let barrier = Barrier::new(3);
    thread::scope(|scope| {
        scope.spawn(|| {
            barrier.wait();
            let q = q_addr as *mut softgpu_core::HsaQueueAbi;
            let pkt = golden_kernel_dispatch_1d(64, 256, 0x1000, 0, 0);
            for i in 0..32u64 {
                let mut guard = shared.lock().unwrap();
                unsafe {
                    let base = (*q).base_address;
                    let slot = (i as usize) % SOFTGPU_QUEUE_MIN_SIZE as usize;
                    let p = base.add(slot * 64);
                    std::ptr::copy_nonoverlapping(pkt.as_ptr(), p, 64);
                }
                guard.queue_store_write_index(q, i + 1).unwrap();
                guard.signal_store(doorbell, (i + 1) as i64).unwrap();
            }
        });
        scope.spawn(|| {
            barrier.wait();
            let q = q_addr as *mut softgpu_core::HsaQueueAbi;
            for _ in 0..32 {
                let guard = shared.lock().unwrap();
                let _ = guard.queue_load_write_index(q);
                let _ = guard.queue_doorbell_count(q);
            }
        });
        scope.spawn(|| {
            barrier.wait();
            let q = q_addr as *const softgpu_core::HsaQueueAbi;
            for _ in 0..16 {
                let guard = shared.lock().unwrap();
                let _ = guard.queue_observe(q);
            }
        });
    });
    let mut rt = shared.into_inner().unwrap();
    let q = q_addr as *mut softgpu_core::HsaQueueAbi;
    let count = rt
        .trace_snapshot()
        .iter()
        .filter(|e| matches!(e, softgpu_core::TraceEvent::PacketObserved { .. }))
        .count();
    assert_eq!(count, 32);
    rt.queue_destroy(q).unwrap();
    rt.shut_down().unwrap();
}

#[test]
fn destroy_signal_cancels_waiter() {
    let mut rt = Runtime::new(test_profile()).unwrap();
    rt.init().unwrap();
    let sig_h = rt.signal_create(0).unwrap();
    let sig = rt.signal_arc(sig_h).unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let b2 = Arc::clone(&barrier);
    let waiter = thread::spawn(move || {
        b2.wait();
        sig.wait(softgpu_core::SignalCondition::Eq, 7, u64::MAX, 1)
    });
    barrier.wait();
    thread::sleep(Duration::from_millis(5));
    rt.signal_destroy(sig_h).unwrap();
    assert!(matches!(
        waiter.join().unwrap(),
        SignalWaitOutcome::Cancelled(_)
    ));
    rt.shut_down().unwrap();
}
