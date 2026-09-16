# Changelog

All notable changes to SoftGPU are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project aims to follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Phase 10: `softgpu-amd-isa` sourced gfx1201 SALU foundation (`softgpu-gfx1201-salu-v1`), machine state, fail-closed traps, llvm-mc goldens, fuzz, `softgpu decode-isa` / `run-isa`, Article 11, [`docs/isa-path.md`](docs/isa-path.md).

### Changed

- `ACTIVE_PHASE` → `phase-10`.

### Notes

- Architectural ISA fidelity applies only to the named SALU subset.
- HIP/HSA AQL kernel success remains **unsupported** (Phase 11).
- Workspace version remains `0.5.0` until the v0.6.0 release (Phases 10–11).

## [0.5.0] — 2026-09-16

### Added

- Phase 9: SoftGPU debugger (`softgpu-debug-trace-v1`), breakpoints, state snapshots, hostile JSONL reader, `schedule_seed` wave-order exploration, race minimize; `softgpu debug-functional`; Article 10; `phase9_debug` tests.
- Coverage-focused tests for ELF fail-closed paths, Phase 9 debug edges, and CLI smoke (`debug-functional`, `--sanitize collect`, `check-config`).

### Changed

- `ACTIVE_PHASE` → `phase-9`.
- Workspace / crates.io package version → `0.5.0` (all SoftGPU crates via `[workspace.package]`).

### Notes

- Debugger never invents SFIR→source line maps (provenance string only).
- HIP/HSA AQL kernel success remains **unsupported**.
- gfx1201 ISA execution remains **unsupported** (Phase 10+).
- Tag `v0.5.0` must match root `Cargo.toml` `[workspace.package] version = "0.5.0"` (release workflow gate).

## [0.4.0] — 2026-09-15

### Added

- Phase 8: SoftGPU functional sanitizer (shadow state, SoftGPU happens-before races, missing barrier, OOB/UAF/uninit, replay bundles); `--sanitize` on `run-functional`; Article 9; `phase8_sanitize` tests.

### Changed

- `ACTIVE_PHASE` → `phase-8`.

### Notes

- HIP/HSA AQL kernel success remains **unsupported** (diagnostic complete ≠ success).
- gfx1201 ISA execution remains **unsupported** (Phase 10+).
- Fidelity claimed for Phase 8 path: **Sanitized** (declared SoftGPU HB subset on SFIR).

## [0.3.0] — 2026-09-15

### Added

- Phase 7: SoftGPU software waves/lanes, group memory, barrier segments, structured `if`/`while`, compares/`and`, selected `atomic_add`; `wave_barrier` schedule; Article 8; `phase7_semantics` tests.
- Phase 6: `softgpu-functional` SoftGPU Functional IR (`softgpu-sfir-v1`) CPU interpreter, `softgpu run-functional`, fixtures under `fixtures/functional/`, [`docs/functional-path.md`](docs/functional-path.md), Article 7.
- Coverage harness: shared `tools/coverage.sh`, enforced line/function floors, HTML+LCOV artifacts, `llvm-tools-preview` in toolchain.
- Tests: SoftGPU-owned fake SFIR programs + forged/null HSA handle fail-closed checks (no mock framework); CLI smoke for Phase 5/6 commands.

### Changed

- `ACTIVE_PHASE` → `phase-7` (then Phase 6 markers in earlier notes).
- Honesty: functional CPU execution ≠ gfx1201 ISA emulation; HIP/HSA AQL still diagnostic-only for kernels.
- Release publish order includes `softgpu-functional`.

### Fixed

- Release workflow: skip `cargo publish --dry-run` for crates whose SoftGPU sibling deps are not yet on crates.io; use `cargo info --registry crates-io` so local workspace versions are not mistaken for published crates.

### Notes

- HIP/HSA AQL kernel success remains **unsupported** (diagnostic complete ≠ success).
- gfx1201 ISA execution remains **unsupported** (Phase 10+).
- Fidelity claimed for Phase 6–7 path: **Functional** (disclosed SFIR on CPU).

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

[Unreleased]: https://github.com/thanos/SoftGPU/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/thanos/SoftGPU/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/thanos/SoftGPU/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/thanos/SoftGPU/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/thanos/SoftGPU/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/thanos/SoftGPU/releases/tag/v0.1.0
