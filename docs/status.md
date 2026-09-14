# SoftGPU status

**Access date for this revision:** 2026-09-14

## What works today?

| Capability | Environment | Fidelity | Verification |
| --- | --- | --- | --- |
| Cargo workspace build/test | macOS / Linux x86-64 (core) | n/a | `verified-unit` |
| Virtual GPU agent + advertised-field provenance | host tests | **ABI** | `verified-unit` |
| SoftGPU HSA cdylib + fail-closed stubs + `hsa_system_get_info` subset | host / ROCm CI | **ABI** | unit + required CI |
| HIP-linked load proof (anti-system-ROCr, `ROCR_1`) | pinned ROCm 7.14.0 CI | **ABI** | required `rocm-integration` |
| HIP-linked HSA discovery (`FEATURE=KERNEL_DISPATCH`) | pinned ROCm 7.14.0 CI | **ABI** | discovery probe |
| Path C memory (regions + AMD pools), signals, queue create/observe | host + ROCm CI | **ABI** | unit + Phase 3 probe |
| Packet observe-once, wait cancel, concurrency charter tests | host | **ABI** | `phase3_charter` |
| AQL packet execution / kernels | — | — | **unsupported** (Phase 4) |

## Active phase

**Phase 3 charter-complete** — Path C memory, signals, queue ABI observe-once,
concurrency/cancel tests, Article 4. AQL execution remains Phase 4.

## Acceptance notes

| Gate | Status |
| --- | --- |
| Phase 0 | **Met** |
| Phase 1 HIP load + anti-system-ROCr | **Met** (required `rocm-integration` green) |
| Phase 2: HIP userspace reaches SoftGPU agent | **Met** |
| Phase 3: Path C memory + signals + queue observe | **Met** (host unit + charter tests; ROCm probe in harness) |
| Phase 3 charter: observe-once, cancel-on-destroy, stress, Article 4 | **Met** |
| Advertised fields have provenance + tests | **Met** |
| Unsupported attrs / handle misuse / traces | **Met** |

### Controlled subset (honest)

Phase 3 advertises `HSA_AGENT_FEATURE_KERNEL_DISPATCH` for **queue ABI + SoftGPU observation only**. SoftGPU does **not** execute AQL packets and does **not** claim kernel launch success or guaranteed `hipGetDeviceCount > 0`. Allocations are SoftGPU host (CPU) software memory with provenance `softgpu-software`, not R9700 VRAM.

## Next acceptance gate

Phase 4 — AQL packet observation → functional execution (still fail-closed for unsupported ISA).

## High-risk assumptions remaining

1. Stub surface remains enough for HIP/HSA libraries to load under SoftGPU substitution (ELF version node **`ROCR_1`**).
2. `TIMESTAMP_FREQUENCY=1e9` is an explicit SoftGPU software-clock provisional, not hardware.
3. Numeric R9700 limits remain unknown.
4. HIP may still need additional agent attributes before treating SoftGPU as a full device; SoftGPU adds only evidence-backed attrs and fails closed otherwise.
