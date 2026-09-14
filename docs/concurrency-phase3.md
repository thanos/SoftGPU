# SoftGPU Phase 3 concurrency and `unsafe` invariants

**Access date:** 2026-09-14  
**Scope:** SoftGPU host runtime only (not AQL hardware memory ordering).

## Process mutex

The SoftGPU process runtime is behind a single `Mutex`. Create/destroy of agents,
regions/pools, signals, and queues, plus allocate/free and queue observe traces,
run under that lock.

## Signals (`Arc<SoftGpuSignal>`)

- Signal values and cancel flags are atomics (`SeqCst`).
- Waiters **clone** the `Arc` under the runtime lock, then **spin outside** the
  lock so destroy/cancel can proceed.
- Destroy / queue teardown / shutdown call `cancel()` so waiters exit with
  `SignalWaitOutcome::Cancelled`.
- SoftGPU does **not** claim HSA system-scope memory-model completeness for
  foreign AQL producers; `SeqCst` is SoftGPU software policy for host waits.

## Queues and packet rings

- Packet buffers are SoftGPU-owned; indexes use atomics.
- Observation uses a separate `observed_through` cursor and an
  `observed_ids` set so each logical packet index is recorded **exactly once**.
- SoftGPU does **not** advance HSA `read_index` as kernel completion (that would
  over-claim execution). Phase 4 owns completion semantics.
- `unsafe impl Send` on queue ABI / packet ownership: exclusive SoftGPU ownership
  plus runtime-mutex serialization of create/destroy. Documented in
  `crates/softgpu-core/src/queue.rs` and covered by concurrent producer/consumer
  stress tests in `tests/phase3_charter.rs`.

## Allocations

- Every SoftGPU allocation has lifetime metadata (`AllocationMeta`: space,
  size, alignment, alloc id, host r/w).
- Free of unknown pointers fails closed.
- Capacity exhaustion returns `OutOfResources` (deterministic).

## What tests cover

| Concern | Test location |
| --- | --- |
| Observe-once / ordering | `phase3_charter::packet_observed_exactly_once_with_ordering` |
| Wraparound indexes | `phase3_charter::wraparound_packet_indexes` |
| Wait timeout | `phase3_charter::signal_timeout_and_stale_after_destroy` |
| Destroy during wait | `phase3_charter::queue_destroy_cancels_doorbell_waiters` |
| Concurrent stress | `phase3_charter::concurrent_producer_consumer_stress` |
| Queue / pool caps | `phase3_charter::queue_cap_resource_limit`, memory exhaustion unit tests |

## Explicit non-claims

- Safe Rust is not proof of AQL producer/consumer memory ordering on real HIP.
- SoftGPU observe-only / diagnostic-complete modes are not kernel success.
- Packet ring validation progressed in Phase 4 to full kernel-dispatch field
  checks plus the documented diagnostic contract
  (`docs/aql-diagnostic-contract.md`).
