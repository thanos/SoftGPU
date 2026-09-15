# Changelog

All notable changes to SoftGPU are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project aims to follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] — 2026-09-15

### Added

- Phase 3: Path C memory (legacy regions + AMD pools), signals, queue create/observe, charter stress, Article 4.
- Phase 4: AQL packet validate/trace, diagnostic complete/reject contract, capture/replay, Article 5, [`docs/aql-diagnostic-contract.md`](docs/aql-diagnostic-contract.md).
- Phase 5: `softgpu-amd-code-object` bounded ELF64 + MessagePack AMDHSA metadata (`NT_AMDGPU_METADATA`, `gfx1201` gate), `softgpu inspect-code-object`, fixtures/provenance, Article 6, [`docs/code-object.md`](docs/code-object.md).
- ROCm probes through Phase 5 (`phase3`/`phase4`/`phase5` helpers in the load-proof harness).

### Changed

- Agent `FEATURE=KERNEL_DISPATCH` means queue + AQL intercept + metadata inspect — **not** kernel execution.
- `ACTIVE_PHASE` → `phase-5`; CI `push` triggers narrowed to `main`/`master` (PR runs once via `pull_request`).

### Notes

- Kernel / gfx1201 ISA execution remains **unsupported** (Phase 6+).
- Diagnostic AQL completion is never claimed as kernel success.
- Fidelity claimed: **ABI** / protocol observation (see README and `docs/status.md`).

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

[Unreleased]: https://github.com/thanos/SoftGPU/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/thanos/SoftGPU/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/thanos/SoftGPU/releases/tag/v0.1.0
