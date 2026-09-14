# Article 4 — HSA queues and signals

- **Audience:** runtime engineers and SoftGPU contributors
- **Prerequisites:** Articles 1–3
- **Evidence:** SoftGPU Phase 3 queue ABI + charter tests
- **Access date:** 2026-09-14

## Problem

HIP/ROCm eventually submit work through **user-mode queues**: a ring of AQL
packets, write/read indexes, and a **doorbell signal**. Emulators that skip
concurrency and ownership tests before execution hide the hardest bugs.

## Queue ring (SoftGPU Phase 3)

```text
producer writes packet[i] (type last)
producer stores write_index = i+1
producer stores doorbell
        |
        v
SoftGPU observes packets in [observed_through, write_index)
  - validate header type
  - record packet index exactly once
  - do NOT execute kernels
  - do NOT pretend HSA read_index advanced as completion
```

SoftGPU advertises `FEATURE=KERNEL_DISPATCH` so queue create is meaningful.
Phase 4 adds AQL validation and an experimental **diagnostic** completion —
still **not** kernel execution. Observation remains an honesty boundary:
SoftGPU saw and classified the packet; SoftGPU did not run it as a GPU kernel.

### Wraparound

Queue size is a power of two. Logical indexes grow monotonically; the physical
slot is `index % size`. SoftGPU tests walk past one full ring so wraparound is
covered before any packet processor exists.

## Signals and waiting

Signals are SoftGPU host atomics. Wait conditions (`EQ` / `NE` / `LT` / `GTE`)
spin with an optional timeout. Destroy, queue teardown, and runtime shutdown
**cancel** waiters so “queue destroy during wait” is deterministic.

HSA wait APIs clone the signal `Arc` under SoftGPU’s process mutex, then wait
**outside** the lock so cancel can proceed.

## Ownership

| Object | SoftGPU rule |
| --- | --- |
| Allocation | Tracked base pointer + metadata; unknown free fails closed |
| Signal | Generation-safe handle; doorbells owned by their queue |
| Queue | SoftGPU owns ABI struct + packet buffer; destroy frees both |
| Packet | Observed once by SoftGPU; not “completed” as kernel success |

## Why concurrency tests precede execution

Packet processors that run kernels before proving:

- observe-once under producer/consumer stress,
- wait cancel on destroy,
- wraparound and resource caps,

will mis-attribute races as “ISA bugs.” SoftGPU’s charter therefore places
queue/signal stress in Phase 3 and AQL interception (not kernel exec) in Phase 4.

## Reproducible example

```bash
cargo test -p softgpu-core --locked --test phase3_charter
cargo test -p softgpu-hsa --locked --test phase3_memory_queue
# Linux x86_64 + ROCm:
#   bash environments/rocm-x86_64/run-phase1-load-proof.sh
```

See also [`docs/concurrency-phase3.md`](../concurrency-phase3.md).

## What we verified / what remains assumed

| Verified | Later |
| --- | --- |
| SoftGPU creates memory/signal/queue resources and observes packets once | HIP itself may need more attrs before creating SoftGPU queues |
| Destroy-during-wait cancels | Full AQL field validation / completion (Phase 4) |
| Concurrent producer/consumer stress under SoftGPU mutex policy | Hardware AQL memory ordering |

## Failure cases

- Write index advanced over still-`INVALID` packets → validate fail + trace
- Free of non-SoftGPU pointer → invalid argument
- Queue create with non-power-of-two size → invalid argument
- Exceed SoftGPU queue cap → out of resources

## References

- HSA Runtime Programmer’s Reference (queues/signals)
- SoftGPU Phase 3 plan: Path C memory + FEATURE Option 3
- `docs/concurrency-phase3.md`, `docs/what-feature-means.md`
