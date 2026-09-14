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
| Path C memory, signals, queue create/observe | host + ROCm CI | **ABI** | unit + Phase 3 probe |
| AQL parse/validate + diagnostic complete/reject + capture/replay | host + ROCm CI | **ABI/Protocol** | `phase4_aql` + Phase 4 probe |
| Kernel execution / gfx1201 ISA | — | — | **unsupported** (Phase 6+) |

## Active phase

**Phase 4** — AQL dispatch interception: validate kernel-dispatch (and minimal
barriers), trace normalized descriptors, experimental diagnostic completion
(**not** kernel success), packet capture/replay. See
[`docs/aql-diagnostic-contract.md`](aql-diagnostic-contract.md) and Article 5.

## Acceptance notes

| Gate | Status |
| --- | --- |
| Phase 0 | **Met** |
| Phase 1 HIP load + anti-system-ROCr | **Met** (required `rocm-integration` green) |
| Phase 2: HIP userspace reaches SoftGPU agent | **Met** |
| Phase 3: Path C memory + signals + queue observe | **Met** |
| Phase 3 charter: observe-once, cancel-on-destroy, stress, Article 4 | **Met** |
| Phase 4: AQL validate + diagnostic contract + capture/replay + Article 5 | **Met** (host unit; SoftGPU-controlled ROCm probe) |
| Advertised fields have provenance + tests | **Met** |
| Unsupported attrs / handle misuse / traces | **Met** |

### Controlled subset (honest)

SoftGPU advertises `HSA_AGENT_FEATURE_KERNEL_DISPATCH` for **queue ABI + AQL
interception**. SoftGPU may store a completion signal under the documented
`diagnostic_complete_no_execution` contract. That is **never** kernel launch
success and does **not** claim guaranteed `hipGetDeviceCount > 0`. Allocations
remain SoftGPU host (CPU) software memory with provenance `softgpu-software`.

## Next acceptance gate

Phase 5 — AMD code-object and kernel metadata handling.

## High-risk assumptions remaining

1. Stub surface remains enough for HIP/HSA libraries to load under SoftGPU substitution (ELF version node **`ROCR_1`**).
2. `TIMESTAMP_FREQUENCY=1e9` is an explicit SoftGPU software-clock provisional, not hardware.
3. Numeric R9700 limits remain unknown.
4. Real HIP launches may still need more SoftGPU surface before a tiny HIP kernel reaches SoftGPU AQL; Phase 4 acceptance uses a SoftGPU-controlled golden packet when HIP is not ready.
5. Barrier packets accept type/completion only; dependency signals are unchecked.
