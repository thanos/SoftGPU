# SoftGPU status

**Access date for this revision:** 2026-09-15

## What works today?

| Capability | Environment | Fidelity | Verification |
| --- | --- | --- | --- |
| Cargo workspace build/test | macOS / Linux x86-64 (core) | n/a | `verified-unit` |
| SoftGPU HSA cdylib + Path C / signals / queues / AQL diagnostic | host + ROCm CI | **ABI/Protocol** | Phase 1–4 probes |
| AMDGPU ELF metadata inspect (`gfx1201` subset) | host + ROCm CI | **ABI** | Phase 5 |
| SoftGPU Functional IR (`softgpu-sfir-v1`) CPU execution | host | **Functional** | `phase6_functional` + `run-functional` |
| gfx1201 ISA execution / HIP AQL kernel success | — | — | **unsupported** |

## Active phase

**Phase 6** — Minimal vendor-neutral functional execution via disclosed SoftGPU
Functional IR. See [`docs/functional-path.md`](functional-path.md) and Article 7.
This is **not** gfx1201 ISA emulation.

## Acceptance notes

| Gate | Status |
| --- | --- |
| Phase 0–5 | **Met** |
| Phase 6: SFIR path locked, tiny_add/copy/index, bounds, deterministic rerun, Article 7 | **Met** |
| HIP/HSA AQL kernel execution | **Not claimed** (diagnostic complete ≠ success) |
| gfx1201 ISA | **Not started** (Phase 10+) |

### Controlled subset (honest)

- Functional mode runs SoftGPU-owned SFIR on a CPU arena with GPU-like ids.
- Provenance for `tiny_add` is hand translation from `tiny_add.ref.c`.
- Arbitrary AMD code objects are **not** treated as executable IR sources.

## Next acceptance gate

Phase 7 — GPU execution semantics v1 (waves/lanes, group memory, barriers).

## High-risk assumptions remaining

1. Stub HSA surface remains enough for HIP load (`ROCR_1`).
2. Functional IR coverage is intentionally tiny; unsupported ops fail closed.
3. Numeric R9700 limits remain unknown.
