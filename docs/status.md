# SoftGPU status

**Release / access date for this revision:** 2026-09-16 (**v0.8.0**)

## What works today?

| Capability | Environment | Fidelity | Verification |
| --- | --- | --- | --- |
| Cargo workspace build/test | macOS / Linux x86-64 (core) | n/a | `verified-unit` |
| SoftGPU HSA cdylib + Path C / signals / queues / AQL | host + ROCm CI | **ABI/Protocol** | Phase 1–4 |
| AMDGPU ELF metadata + SoftGPU `.text` load | host + ROCm CI | **ABI** | Phase 5 + v0.8 |
| SoftGPU Functional IR (`softgpu-sfir-v1`) | host | **Functional** | Phases 6–9 |
| SoftGPU gfx1201 compute ISA (`compute-v2`) | host | **Architectural ISA** | `phase_isa_v2` |
| HSA executable → AQL ISA success (SoftGPU fixture) | host (+ ROCm script) | architectural | `phase_hip_load` |
| Unregistered AQL kernels | host | diagnostic only | Phase 4 contract |
| Arbitrary hipcc / unrestricted HIP | — | — | **unsupported** |
| Hardware differential | — | — | **unsupported** (v0.9 / Phase 12) |

## Quick start (v0.8.0)

```bash
cargo test --workspace --locked
cargo run --locked -- info   # active_phase=phase-hip-load
cargo run --locked -- run-kernel --builtin tiny_add --n 64
cargo test -p softgpu-core --test phase_hip_load --locked
```

## Active phase

**phase-hip-load** — HSA executable subset + SoftGPU agent `.text` load.
See [`docs/HIP-gap-analysis.md`](HIP-gap-analysis.md) and [`docs/isa-path.md`](isa-path.md).

## Acceptance notes

| Gate | Status |
| --- | --- |
| Phase 0–11 + v0.7 compute-v2 | **Met** |
| v0.8 executable load + AQL success (SoftGPU fixture) | **Met** |
| Arbitrary hipcc kernels / full AMDHSA | **Not claimed** |
| Hardware differential | **Not claimed** (Phase 12 / v0.9) |

## Next acceptance gate

Phase 12 / **v0.9.0** — R9700 hardware conformance and profile hardening.
