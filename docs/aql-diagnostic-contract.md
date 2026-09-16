# SoftGPU Phase 4 AQL diagnostic completion contract

**Access date:** 2026-09-14  
**Fidelity:** ABI (packet protocol only)  
**Does not claim:** kernel execution, gfx1201 ISA success, or HIP launch success.

## Purpose

Phase 4 intercepts AQL packets on SoftGPU queues, validates a supported
subset, traces a normalized dispatch descriptor, and may **diagnose-complete**
or **diagnose-reject** the packet **without executing kernel semantics**.

## Supported packet types

| Type | SoftGPU behavior |
| --- | --- |
| `KERNEL_DISPATCH` | Full field validation (dims, workgroup/grid, segments, kernarg class, completion) |
| `BARRIER_AND` / `BARRIER_OR` | Minimal: type + completion signal only (dependency signals unchecked) |
| `AGENT_DISPATCH` / other | Diagnostic reject |
| `INVALID` (still) | Observe error (`StillInvalid`) — producer must publish type last |

## Diagnostic complete (`diagnostic_complete_no_execution`)

When a packet validates:

1. SoftGPU records `DispatchValidated` with grid/workgroup/segments/kernel object/kernarg class/completion.
2. SoftGPU **captures** owned packet bytes for offline replay (`aql::replay_dispatch`).
3. If `completion_signal ≠ 0` and the handle is a live SoftGPU non-doorbell signal, SoftGPU stores **`0`** (HSA completion convention).
4. SoftGPU marks the ring slot `INVALID` and advances HSA `read_index` past the packet.
5. Trace records `DiagnosticComplete` with `note=not_kernel_success`.

**This is never unrestricted HIP kernel success.** Completion only means SoftGPU finished the
experimental no-execution protocol for that packet.

Phase 11 adds a separate contract, `softgpu_kernel_success`, when a SoftGPU-registered
ISA kernel image is dispatched with SoftGPU-owned kernarg memory. See Article 12.

## Diagnostic reject (`diagnostic_rejected`)

When a packet fails validation or is unsupported:

1. SoftGPU records `DispatchRejected` with the parse error.
2. SoftGPU **does not** store the completion signal as success (value unchanged).
3. SoftGPU still invalidates the slot and advances `read_index` so producers are not hung on a permanently stuck processor.

## Kernarg classification

| Class | Meaning |
| --- | --- |
| `null` | Address 0 |
| `softgpu` | Pointer owned by SoftGPU allocator (alloc_id when known) |
| `foreign_opaque` | Non-null pointer SoftGPU does not own |

Replay always treats non-null kernarg as `foreign_opaque` (no live ownership table).

## Honesty

- `FEATURE=KERNEL_DISPATCH` remains a **queue + interception** claim, not execution.
- Advancing `read_index` is packet-processor protocol progress, not “the kernel ran.”
- Functional kernel execution is a later phase.
