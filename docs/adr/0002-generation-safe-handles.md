# ADR-0002: Generation-safe packed handles

- Status: Accepted
- Date: 2026-09-13
- Deciders: SoftGPU Phase 2

## Context

HSA exposes agents (and later queues/signals) as opaque `uint64_t` handles. SoftGPU needs deterministic errors for stale, forged, and cross-kind values after recycle, without leaking raw table indices.

## Evidence

- Pinned ROCR `hsa.h`: `typedef struct hsa_agent_s { uint64_t handle; } hsa_agent_t;`
- Charter requires generation-safe handles and fail-closed misuse tests.

## Decision

Pack SoftGPU handles as `[kind:8][generation:24][index:32]`. Maintain a monotonic generation allocator so teardown/re-init cannot revive prior values. Kind bits reject cross-type misuse once more object types exist.

## Alternatives considered

- Raw index handles — rejected (use-after-free / recycle ambiguity).
- Pointer-as-handle — rejected (ASLR noise; harder validation; host pointer leakage).

## Consequences

- ABI edge converts `hsa_agent_t.handle` ↔ `PackedHandle`.
- Tests cover forged and post-shutdown handles.

## Validation plan

Unit tests in `softgpu-core`; HSA API smoke tests in `softgpu-hsa`.

## Reconsider if

HSA extensions require non-opaque handle layouts (unexpected) or SoftGPU needs >2^24 live generation epochs without wrap policy changes.
