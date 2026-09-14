# Changelog

All notable changes to SoftGPU are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project aims to follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Phase 5: `softgpu-amd-code-object` bounded ELF64 + MessagePack AMDHSA metadata parser (`NT_AMDGPU_METADATA`), `gfx1201` target gate, `softgpu inspect-code-object`, synthetic fixtures, fuzz smoke floor, Article 6, [`docs/code-object.md`](docs/code-object.md).
- ROCm helper `environments/rocm-x86_64/run-phase5-code-object.sh` (synthetic gate + optional hipcc/llvm-readelf).

### Changed

- `ACTIVE_PHASE` → `phase-5`; status/support-matrix/README honesty updated.
- Phase 4 AQL diagnostic interception retained (completion ≠ kernel success).

## [0.1.0] — 2026-09-14

### Added

- Phase 0 charter: fidelity vocabulary, fail-closed unsupported policy, profile schema with provenance.
- Phase 1: SoftGPU `libhsa_runtime64` (`softgpu-hsa`) with fail-closed HSA stubs, `ROCR_1` ELF version node, pinned ROCm **7.14.0** HIP load proof in CI.
- Phase 2: generation-safe handles, one virtual GPU agent (`FEATURE=0`), profile-backed identity, HIP-linked HSA discovery probe.
- Workspace crates: `softgpu` (CLI), `softgpu-core`, `softgpu-hsa`.
- CI: core (macOS/Linux), ROCm integration gate, coverage → Coveralls, code quality, dependency (`cargo-deny`) checks, tag-driven crates.io release workflow.

### Notes

- Queues, AQL, kernels, and `hipGetDeviceCount > 0` remain **unsupported** / out of scope for 0.1.0.
- Fidelity claimed: **ABI** (see README and `docs/status.md`).

[Unreleased]: https://github.com/thanos/SoftGPU/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/thanos/SoftGPU/releases/tag/v0.1.0
