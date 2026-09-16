# SoftGPU status

**Access date for this revision:** 2026-09-15

## What works today?

| Capability | Environment | Fidelity | Verification |
| --- | --- | --- | --- |
| Cargo workspace build/test | macOS / Linux x86-64 (core) | n/a | `verified-unit` |
| SoftGPU HSA cdylib + Path C / signals / queues / AQL diagnostic | host + ROCm CI | **ABI/Protocol** | Phase 1–4 probes |
| AMDGPU ELF metadata inspect (`gfx1201` subset) | host + ROCm CI | **ABI** | Phase 5 |
| SoftGPU Functional IR (`softgpu-sfir-v1`) CPU execution | host | **Functional** | `phase6_functional` + `run-functional` |
| SoftGPU waves/lanes, group memory, barriers, selected atomics | host | **Functional** | `phase7_semantics` |
| SoftGPU memory/race/barrier sanitizer (declared subset) | host | **Sanitized** | `phase8_sanitize` |
| SoftGPU debugger / traces / seeded schedule explore | host | **Functional** | `phase9_debug` |
| gfx1201 ISA execution / HIP AQL kernel success | — | — | **unsupported** |

## Active phase

**Phase 9** — Debugger and deterministic exploration on SoftGPU Functional IR.
See [`docs/functional-path.md`](functional-path.md) and Article 10. This is
**not** gfx1201 ISA emulation.

## Acceptance notes

| Gate | Status |
| --- | --- |
| Phase 0–8 | **Met** |
| Phase 9: traces, breakpoints, snapshots, hostile reader, explore/minimize, Article 10 | **Met** |
| HIP/HSA AQL kernel execution | **Not claimed** |
| gfx1201 ISA | **Not started** (Phase 10+) |

### Controlled subset (honest)

- SoftGPU `wave_size` is a software parameter (default 32), not R9700 wavefront evidence.
- Barriers are segment sync under `wave_barrier` schedule; divergent barriers unsupported.
- Atomics are functional sequential ops on SoftGPU arenas.
- Sanitizer races are SoftGPU barrier-generation happens-before, not hardware concurrency.
- Debugger never invents SFIR→source line maps.

## Next acceptance gate

Phase 10 — gfx1201 ISA foundation.

## High-risk assumptions remaining

1. Stub HSA surface remains enough for HIP load (`ROCR_1`).
2. Functional IR coverage remains a disclosed subset; unsupported ops fail closed.
3. Numeric R9700 limits remain unknown.
