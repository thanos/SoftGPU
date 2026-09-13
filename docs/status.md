# SoftGPU status

**Access date for this revision:** 2026-09-13

## What works today?

| Capability | Environment | Fidelity | Verification |
| --- | --- | --- | --- |
| Cargo build + unit/integration tests | macOS Apple Silicon, Linux x86-64 (core) | n/a (host tooling) | `verified-unit` via `cargo test --locked` |
| Device profile schema validate/load | any host with Rust 1.85+ | n/a | `verified-unit` |
| CLI `info` / `validate-profile` / `check-config` | any host with Rust 1.85+ | n/a | `verified-unit` |
| ROCr/HSA shared library | — | — | **unsupported** |
| Agent discovery / queues / AQL / kernels | — | — | **unsupported** |

## Active phase

**Phase 0 — Charter, evidence baseline, and skeleton** (implementation complete locally; awaiting explicit Phase 0 acceptance before Phase 1 ABI work)

## Phase 0 acceptance checklist

| Criterion | Status |
| --- | --- |
| `cargo test --locked` documented for macOS + Linux | Met (README); executed on this Apple Silicon host under pinned 1.85.0; Linux covered by CI workflow |
| Scope / fidelity / unsupported policy in README | Met |
| Sources ledger for HSA, ROCr, HIP, LLVM/AMDGPU, Rust, Apple container | Met (`docs/sources.md`) |
| No invented R9700 numeric specs | Met (limits `unknown`) |
| Toolchain/MSRV/lockfile/`unsafe` policy; stable-only | Met |
| CI distinguishes skipped vs passed (ROCm job `if: false`) | Met |
| Article 1 + Article 19 draft | Met |

## Next acceptance gate

**Human acceptance of Phase 0**, then Phase 1: ROCr/HSA ABI reconnaissance and load proof on Linux x86-64 with pinned ROCm.

## High-risk assumptions remaining

1. The practical SoftGPU ↔ HIP userspace boundary remains ROCr/HSA substitution (see ADR-0001); not yet proven with a load test.
2. Official ROCm packages for gfx1201 / R9700 integration are x86-64 Linux–centric; Apple Silicon cannot be the canonical ROCm gate.
3. Numeric R9700 resource limits are **unknown** in SoftGPU profiles until measured; do not invent them.
4. rustc `amdgcn-amd-amdhsa` is Tier 3 and is **not** the Phase 1–6 compatibility path.

## Phase 0 execution note (this session)

| Item | Content |
| --- | --- |
| **Objective** | Establish pinned Rust skeleton, truthful docs, profile provenance schema, CI smoke path |
| **Evidence** | Primary sources recorded in `docs/sources.md` (ROCm/ROCR docs 7.14, HSA Runtime 1.2, LLVM AMDGPUUsage, Apple container, rustc book) |
| **Assumptions** | Single crate until Phase 1 proves cdylib boundary; MSRV 1.85 / toolchain 1.85.0 |
| **Test-first** | Profile schema negatives + CLI config negatives before expanding modules |
| **Risks** | Accidental marketing numbers in R9700 profile; CI marking skipped ROCm jobs as success |
| **Files** | `src/*`, `profiles/*`, `tests/*`, `docs/*`, `.github/workflows/ci.yml`, policy files |
