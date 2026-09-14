# Changelog

All notable changes to SoftGPU are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project aims to follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Phase 3: Path C memory (legacy regions + AMD pools on one SoftGPU host allocator), signals, queue create/destroy/indexes with doorbell observation.
- Agent `FEATURE=KERNEL_DISPATCH` for queue ABI only; AQL packet execution remains unsupported.
- SoftGPU-native Phase 3 tests and ROCm `softgpu-phase3-queue-probe`.
- Phase 3 charter completion: allocation lifetime metadata, packet observe-once validation, wait cancel on destroy/shutdown, concurrency stress tests, [`docs/concurrency-phase3.md`](docs/concurrency-phase3.md), Article 4.

### Changed

- Discovery probe expects `KERNEL_DISPATCH`; docs/status/support-matrix honesty updated for queue-ABI-only claim.
- Signal waits clone `Arc` and spin outside the process mutex so destroy can cancel waiters.

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
