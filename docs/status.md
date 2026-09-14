# SoftGPU status

**Access date for this revision:** 2026-09-13

## What works today?

| Capability | Environment | Fidelity | Verification |
| --- | --- | --- | --- |
| Cargo workspace build/test | macOS / Linux x86-64 (core) | n/a | `verified-unit` |
| Virtual GPU agent + advertised-field provenance | host tests | **ABI** | `verified-unit` |
| SoftGPU HSA cdylib + fail-closed stubs + `hsa_system_get_info` subset | host / ROCm CI | **ABI** | unit + CI harness |
| HIP-linked load proof (anti-system-ROCr) | pinned ROCm 7.14.0 CI | **ABI** | required `rocm-integration` |
| HIP-linked HSA discovery of SoftGPU GPU (`FEATURE=0`) | pinned ROCm 7.14.0 CI | **ABI** | Phase 2 probe in same script |
| Queues / AQL / kernels | — | — | **unsupported** |

## Active phase

**Phase 2 — Virtual agent discovery** (implementation complete; integration verification via required ROCm CI job)

## Acceptance notes

| Gate | Status |
| --- | --- |
| Phase 0 | Met |
| Phase 1 HIP load + anti-system-ROCr | Harness + required CI |
| Phase 2: HIP userspace reaches SoftGPU agent (controlled subset: HSA iterate from HIP-linked process) | Harness + unit provenance tests; CI runs discovery probe |
| Advertised fields have provenance + tests | Met |
| Unsupported attrs / handle misuse / traces | Met |

### Controlled subset (honest)

Phase 2 advertises `FEATURE=0` (no dispatch). Therefore SoftGPU does **not** claim `hipGetDeviceCount > 0`. Discovery evidence is: a **HIP-linked** process loads SoftGPU as `libhsa-runtime64`, then `hsa_iterate_agents` / `hsa_agent_get_info` observe the profile-backed GPU agent.

## Next acceptance gate

Phase 3 — memory regions/pools, signals, and queue mechanics (only after CI remains green on Phase 1/2 probes).

## High-risk assumptions remaining

1. Stub surface is enough for HIP/HSA libraries to load under SoftGPU substitution.
2. `TIMESTAMP_FREQUENCY=1e9` is an explicit SoftGPU software-clock provisional, not hardware.
3. Numeric R9700 limits remain unknown.
