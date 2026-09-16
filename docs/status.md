# SoftGPU status

**Access date for this revision:** 2026-09-16

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
| SoftGPU gfx1201 SALU subset (`softgpu-gfx1201-salu-v1`) | host | **Architectural ISA** | `phase10_isa` + llvm-mc goldens |
| HIP AQL kernel success / full gfx1201 ISA | — | — | **unsupported** (Phase 11+) |

## Active phase

**Phase 10** — gfx1201 ISA foundation (sourced decoder + narrow SALU interpreter).
See [`docs/isa-path.md`](isa-path.md) and Article 11. This is **not** end-to-end
HIP kernel execution.

## Acceptance notes

| Gate | Status |
| --- | --- |
| Phase 0–9 | **Met** |
| Phase 10: sourced decoder, machine state, traps, goldens, fuzz, Article 11 | **Met** |
| HIP/HSA AQL kernel execution | **Not claimed** |
| Full gfx1201 ISA | **Not claimed** |

### Controlled subset (honest)

- SoftGPU `wave_size` is a software parameter (default 32), not R9700 wavefront evidence.
- Phase 10 ISA coverage is only `softgpu-gfx1201-salu-v1` (listed in Article 11).
- `s_waitcnt` / `s_sleep` are SoftGPU no-ops under the sequential interpreter.
- Debugger never invents SFIR→source line maps.

## Next acceptance gate

Phase 11 — first end-to-end gfx1201 kernel through the real dispatch path.

## High-risk assumptions remaining

1. Stub HSA surface remains enough for HIP load (`ROCR_1`).
2. Functional IR coverage remains a disclosed subset; unsupported ops fail closed.
3. Numeric R9700 limits remain unknown.
4. llvm-mc goldens remain the SoftGPU encoding source of truth for Phase 10 tables.
