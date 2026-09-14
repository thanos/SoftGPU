# Changelog

All notable changes to SoftGPU are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project aims to follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Phase 4: AQL packet header/type parser, kernel-dispatch validation, minimal barriers, normalized `DispatchDescriptor`, capture/replay, and documented diagnostic complete/reject contract ([`docs/aql-diagnostic-contract.md`](docs/aql-diagnostic-contract.md)).
- SoftGPU-native Phase 4 tests and ROCm `softgpu-phase4-aql-probe` (SoftGPU-controlled golden packet).
- Article 5: HSA/AQL dispatch end to end.

### Changed

- Doorbell observe path validates packets, may advance HSA `read_index` / invalidate slots, and may store completion `0` under `diagnostic_complete_no_execution` — never claimed as kernel success.
- `ACTIVE_PHASE` → `phase-4`; status/support-matrix/README honesty updated.

### Notes (Phase 3 retained)

- Phase 3: Path C memory, signals, queue create/observe, charter stress, Article 4.

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
