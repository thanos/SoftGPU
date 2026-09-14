//! SoftGPU host signals (CPU atomics). No hardware interrupts.
//!
//! # Concurrency invariants
//!
//! - `value` and `cancelled` are atomics; waiters poll both.
//! - Destroy / queue teardown sets `cancelled` so waits exit deterministically
//!   without requiring the process mutex while spinning.
//! - SoftGPU does not claim HSA memory-model completeness for foreign AQL
//!   producers; SeqCst is a SoftGPU software policy for host-side waits.

use crate::handle::PackedHandle;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::time::{Duration, Instant};

/// HSA signal condition codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SignalCondition {
    Eq = 0,
    Ne = 1,
    Lt = 2,
    Gte = 3,
}

impl SignalCondition {
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Eq),
            1 => Some(Self::Ne),
            2 => Some(Self::Lt),
            3 => Some(Self::Gte),
            _ => None,
        }
    }

    pub fn satisfied(self, observed: i64, compare: i64) -> bool {
        match self {
            Self::Eq => observed == compare,
            Self::Ne => observed != compare,
            Self::Lt => observed < compare,
            Self::Gte => observed >= compare,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalWaitOutcome {
    Satisfied(i64),
    TimedOut(i64),
    Cancelled(i64),
}

#[derive(Debug)]
pub struct SoftGpuSignal {
    pub handle: PackedHandle,
    value: AtomicI64,
    cancelled: AtomicBool,
    /// True when this signal is owned as a queue doorbell (observe stores).
    pub is_doorbell: bool,
    pub queue_id: Option<u64>,
}

impl SoftGpuSignal {
    pub fn new(handle: PackedHandle, initial: i64) -> Self {
        Self {
            handle,
            value: AtomicI64::new(initial),
            cancelled: AtomicBool::new(false),
            is_doorbell: false,
            queue_id: None,
        }
    }

    pub fn new_doorbell(handle: PackedHandle, initial: i64, queue_id: u64) -> Self {
        Self {
            handle,
            value: AtomicI64::new(initial),
            cancelled: AtomicBool::new(false),
            is_doorbell: true,
            queue_id: Some(queue_id),
        }
    }

    pub fn load(&self) -> i64 {
        self.value.load(Ordering::SeqCst)
    }

    pub fn store(&self, value: i64) {
        self.value.store(value, Ordering::SeqCst);
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub fn wait(
        &self,
        condition: SignalCondition,
        compare: i64,
        timeout_hint_ns: u64,
        _wait_state_hint: u32,
    ) -> SignalWaitOutcome {
        let deadline = if timeout_hint_ns == u64::MAX {
            None
        } else {
            Some(Instant::now() + Duration::from_nanos(timeout_hint_ns))
        };
        loop {
            if self.is_cancelled() {
                return SignalWaitOutcome::Cancelled(self.load());
            }
            let observed = self.load();
            if condition.satisfied(observed, compare) {
                return SignalWaitOutcome::Satisfied(observed);
            }
            if let Some(deadline) = deadline {
                if Instant::now() >= deadline {
                    return SignalWaitOutcome::TimedOut(observed);
                }
            }
            std::thread::yield_now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handle::HandleKind;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn wait_eq_returns_when_satisfied() {
        let sig = SoftGpuSignal::new(PackedHandle::pack(HandleKind::Signal, 1, 0), 0);
        sig.store(7);
        match sig.wait(SignalCondition::Eq, 7, 1_000_000_000, 1) {
            SignalWaitOutcome::Satisfied(v) => assert_eq!(v, 7),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn wait_times_out() {
        let sig = SoftGpuSignal::new(PackedHandle::pack(HandleKind::Signal, 1, 0), 0);
        match sig.wait(SignalCondition::Eq, 1, 1_000_000, 1) {
            SignalWaitOutcome::TimedOut(v) => assert_eq!(v, 0),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn wait_cancelled_during_spin() {
        let sig = SoftGpuSignal::new(PackedHandle::pack(HandleKind::Signal, 1, 0), 0);
        let handle = sig.handle;
        thread::scope(|s| {
            s.spawn(|| {
                thread::sleep(Duration::from_millis(5));
                // Reconstruct via shared reference — signal lives on stack.
                let _ = handle;
            });
            // Cancel from this thread after starting wait in child would need Arc;
            // exercise cancel-before-wait and cancel-mid-wait with local cancel.
            let sig_ref = &sig;
            s.spawn(|| {
                thread::sleep(Duration::from_millis(2));
                sig_ref.cancel();
            });
            match sig_ref.wait(SignalCondition::Eq, 99, u64::MAX, 1) {
                SignalWaitOutcome::Cancelled(_) => {}
                other => panic!("unexpected {other:?}"),
            }
        });
    }
}
