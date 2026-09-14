//! Structured, bounded runtime traces.

use crate::fidelity::FidelityLevel;
use serde::Serialize;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Stable event categories for SoftGPU runtime observation.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TraceEvent {
    RuntimeInit {
        seq: u64,
        refcount: u32,
        profile_id: String,
        profile_revision: String,
        fidelity: String,
    },
    RuntimeShutdown {
        seq: u64,
        refcount: u32,
    },
    AgentIterateBegin {
        seq: u64,
        agent_count: usize,
    },
    AgentIterateVisit {
        seq: u64,
        agent_handle: u64,
        kind: String,
    },
    AgentGetInfo {
        seq: u64,
        agent_handle: u64,
        attribute: String,
        outcome: String,
    },
    MemoryAllocate {
        seq: u64,
        space_handle: u64,
        size: usize,
        ptr: u64,
        outcome: String,
    },
    MemoryFree {
        seq: u64,
        ptr: u64,
        outcome: String,
    },
    SignalCreate {
        seq: u64,
        signal_handle: u64,
        initial: i64,
    },
    SignalDestroy {
        seq: u64,
        signal_handle: u64,
    },
    QueueCreate {
        seq: u64,
        queue_id: u64,
        size: u32,
        agent_handle: u64,
    },
    QueueDestroy {
        seq: u64,
        queue_id: u64,
    },
    QueueDoorbell {
        seq: u64,
        queue_id: u64,
        value: i64,
    },
    QueueIndexStore {
        seq: u64,
        queue_id: u64,
        which: String,
        value: u64,
    },
    PacketObserved {
        seq: u64,
        queue_id: u64,
        packet_index: u64,
        packet_type: u16,
    },
    PacketValidateFailed {
        seq: u64,
        queue_id: u64,
        detail: String,
    },
    Unsupported {
        seq: u64,
        api: String,
        detail: String,
    },
}

/// Sink that records events without panicking the runtime.
pub trait TraceSink: Send {
    fn record(&mut self, event: TraceEvent);
}

/// In-memory ring with a fixed capacity.
#[derive(Debug, Default)]
pub struct TraceLog {
    seq: u64,
    events: Vec<TraceEvent>,
    capacity: usize,
}

impl TraceLog {
    pub fn new(capacity: usize) -> Self {
        Self {
            seq: 0,
            events: Vec::new(),
            capacity: capacity.max(1),
        }
    }

    pub fn next_seq(&mut self) -> u64 {
        self.seq = self.seq.saturating_add(1);
        self.seq
    }

    pub fn events(&self) -> &[TraceEvent] {
        &self.events
    }

    pub fn fidelity_label(level: FidelityLevel) -> String {
        level.as_str().to_string()
    }
}

impl TraceSink for TraceLog {
    fn record(&mut self, event: TraceEvent) {
        if self.events.len() >= self.capacity {
            self.events.remove(0);
        }
        self.events.push(event);
    }
}

/// Shared optional trace holder used by the runtime.
#[derive(Debug)]
pub struct SharedTrace {
    inner: Mutex<TraceLog>,
}

impl SharedTrace {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(TraceLog::new(capacity)),
        }
    }

    pub fn with_log<R>(&self, f: impl FnOnce(&mut TraceLog) -> R) -> R {
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&mut guard)
    }

    pub fn snapshot(&self) -> Vec<TraceEvent> {
        self.with_log(|log| log.events().to_vec())
    }
}

/// Wall-clock helper for optional JSON Lines tooling (not used as GPU time).
pub fn unix_millis_now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_drops_oldest() {
        let mut log = TraceLog::new(2);
        log.record(TraceEvent::Unsupported {
            seq: 1,
            api: "a".into(),
            detail: "1".into(),
        });
        log.record(TraceEvent::Unsupported {
            seq: 2,
            api: "b".into(),
            detail: "2".into(),
        });
        log.record(TraceEvent::Unsupported {
            seq: 3,
            api: "c".into(),
            detail: "3".into(),
        });
        assert_eq!(log.events().len(), 2);
        match &log.events()[0] {
            TraceEvent::Unsupported { api, .. } => assert_eq!(api, "b"),
            _ => panic!("unexpected"),
        }
    }
}
