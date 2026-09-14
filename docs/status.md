# SoftGPU status

**Access date for this revision:** 2026-09-14

## What works today?

| Capability | Environment | Fidelity | Verification |
| --- | --- | --- | --- |
| Cargo workspace build/test | macOS / Linux x86-64 (core) | n/a | `verified-unit` |
| Virtual GPU agent + advertised-field provenance | host tests | **ABI** | `verified-unit` |
| SoftGPU HSA cdylib + fail-closed stubs + `hsa_system_get_info` subset | host / ROCm CI | **ABI** | unit + required CI |
| HIP-linked load proof (anti-system-ROCr, `ROCR_1`) | pinned ROCm 7.14.0 CI | **ABI** | required `rocm-integration` |
| Path C memory, signals, queues, AQL diagnostic intercept | host + ROCm CI | **ABI/Protocol** | Phase 3–4 probes |
| AMDGPU ELF / `NT_AMDGPU_METADATA` inspect (`gfx1201` subset) | host + ROCm CI | **ABI** | `phase5_code_object` + Phase 5 script |
| Kernel execution / gfx1201 ISA | — | — | **unsupported** (Phase 6+) |

## Active phase

**Phase 5** — AMD code-object and kernel metadata handling: bounded ELF64 note
walker, MessagePack AMDHSA metadata for `amdhsa.version` `[1,0]`–`[1,2]` and
targets containing `gfx1201`, inspection CLI, synthetic fixtures + optional
official `hipcc` cross-check. See [`docs/code-object.md`](code-object.md) and
Article 6.

## Acceptance notes

| Gate | Status |
| --- | --- |
| Phase 0–4 | **Met** (see prior revisions) |
| Phase 5: metadata parse + fail-closed version/target + fuzz floor + Article 6 | **Met** (host unit; SoftGPU-synthetic fixtures; ROCm optional hipcc note) |
| Kernel ISA execution | **Not started** (Phase 6+) |

### Controlled subset (honest)

SoftGPU inspects AMDGPU metadata notes only. Synthetic fixtures are SoftGPU-built
ELF+msgpack with provenance `softgpu-synthetic`. Finding kernels is **not**
kernel execution. `FEATURE=KERNEL_DISPATCH` remains queue + AQL intercept.

## Next acceptance gate

Phase 6 — Minimal vendor-neutral functional execution (disclosed input path).

## High-risk assumptions remaining

1. Stub surface remains enough for HIP/HSA load (`ROCR_1`).
2. Fat HIP host binaries may embed code objects in layouts SoftGPU does not yet unpack; standalone / SoftGPU-synthetic ELF remains the primary metadata gate.
3. Numeric R9700 limits remain unknown.
4. Only `gfx1201` targets are accepted in Phase 5.
