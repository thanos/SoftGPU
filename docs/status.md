# SoftGPU status

**Release / access date for this revision:** 2026-09-16 (**v0.6.0**)

## What works today?

| Capability | Environment | Fidelity | Verification |
| --- | --- | --- | --- |
| Cargo workspace build/test | macOS / Linux x86-64 (core) | n/a | `verified-unit` |
| SoftGPU HSA cdylib + Path C / signals / queues / AQL | host + ROCm CI | **ABI/Protocol** | Phase 1–4 |
| AMDGPU ELF metadata inspect (`gfx1201` subset) | host + ROCm CI | **ABI** | Phase 5 |
| SoftGPU Functional IR (`softgpu-sfir-v1`) | host | **Functional** | Phases 6–9 |
| SoftGPU gfx1201 e2e tiny ISA (`tiny_add`) | host | **Architectural ISA** | `phase11_kernel` / `phase11_aql_isa` |
| Unregistered AQL kernels | host | diagnostic only | Phase 4 contract |
| Full gfx1201 / unrestricted HIP launch | — | — | **unsupported** |

## Quick start (v0.6.0)

```bash
cargo test --workspace --locked
cargo run --locked -- info   # active_phase=phase-11
cargo run --locked -- run-kernel --builtin tiny_add --n 64
```

## Active phase

**Phase 11** — first end-to-end SoftGPU gfx1201 tiny kernel (llvm-mc text + AQL
registration). See [`docs/isa-path.md`](isa-path.md) and Article 12.

## Acceptance notes

| Gate | Status |
| --- | --- |
| Phase 0–10 | **Met** |
| Phase 11: tiny kernel ISA + AQL `softgpu_kernel_success` + differential + Article 12 | **Met** |
| Full HIP userspace / arbitrary hipcc kernels | **Not claimed** |

## Next acceptance gate

Phase 12 — R9700 hardware conformance and profile hardening.
