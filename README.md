# SoftGPU

[![CI](https://github.com/thanos/SoftGPU/actions/workflows/ci.yml/badge.svg)](https://github.com/thanos/SoftGPU/actions/workflows/ci.yml)
[![Coverage](https://coveralls.io/repos/github/thanos/SoftGPU/badge.svg?branch=main)](https://coveralls.io/github/thanos/SoftGPU?branch=main)
[![Code quality](https://github.com/thanos/SoftGPU/actions/workflows/code-quality.yml/badge.svg)](https://github.com/thanos/SoftGPU/actions/workflows/code-quality.yml)
[![Dependencies](https://github.com/thanos/SoftGPU/actions/workflows/dependencies.yml/badge.svg)](https://github.com/thanos/SoftGPU/actions/workflows/dependencies.yml)
[![Crates.io](https://img.shields.io/crates/v/softgpu.svg)](https://crates.io/crates/softgpu)
[![docs.rs](https://docs.rs/softgpu/badge.svg)](https://docs.rs/softgpu)
[![License](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue.svg)](LICENSE)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-informational.svg)](rust-toolchain.toml)

SoftGPU is a **Rust-first**, developer-oriented GPU **emulation, testing, debugging, sanitization, and CI** runtime. It aims to let real AMD HIP userspace talk to a SoftGPU ROCr/HSA compatibility adapter, then execute and diagnose kernels on a vendor-neutral core—without pretending to be a cycle-accurate Radeon AI PRO R9700 or inventing undocumented AMD behavior.

> **Active — Phase 6:** SoftGPU Functional IR (`softgpu-sfir-v1`) runs on the CPU.
> This is **not** gfx1201 ISA emulation. HIP/HSA AQL remains diagnostic-only for
> kernels. See [docs/status.md](docs/status.md),
> [docs/functional-path.md](docs/functional-path.md), and [CHANGELOG.md](CHANGELOG.md).

## What SoftGPU is (and is not)

**Is:** a fail-closed, evidence-driven path from ABI/protocol observation → functional execution → sanitizers → a verified gfx1201 subset → hardware differential tests.

**Is not (initially):** a HIP SDK replacement, kernel-mode AMDGPU driver, PCIe/MMIO/firmware device model, cycle-accurate simulator, graphics API, performance oracle, or permission to return success for unimplemented behavior.

## Fidelity vocabulary

Every run, diagnostic, and public report must name its fidelity level:

| Level | Meaning |
| --- | --- |
| **ABI** | Declared HSA/ROCr subset with verified C ABI behavior |
| **Protocol** | Queues, signals, AQL, registration, dispatch flow (declared subset) |
| **Functional** | CPU-backed semantic engine; **not** gfx1201 ISA evidence |
| **Architectural ISA** | Verified gfx1201 instruction/state semantics |
| **Sanitized** | Extra checking that may perturb scheduling/timing |
| **Analytical performance** | Estimates/counters only; not cycle accuracy |
| **Hardware-conformant** | Named SoftGPU + toolchain + real hardware evidence |

Do not use “R9700 emulator,” “gfx1201 compatible,” or “conformant” without attaching scope and evidence.

## Unsupported behavior policy

Unsupported or malformed input must produce a stable error category, human context, and (where applicable) structured diagnostics. SoftGPU **never silently fakes** unsupported behavior. Optional permissive/approximation modes (future) must warn loudly, mark non-conformance, and stay out of default CI.

## Release roadmap

Releases are organized around **what a developer can accomplish**. Engineering **phases** are gates inside those releases (see the staged implementation charter). Phase numbers express dependency, not calendar dates. No tagged release without the cross-phase gates in the charter (CI green, support matrix match, fail-closed unsupported paths, provenance, etc.).

| Release | Scope |
| --- | --- |
| **v0.1.0 — Runtime foundation** | Library substitution, init, agent discovery, profile provenance, explicit unsupported errors. **Phases 0–2.** Basic packaging: reproducible install, one documented launch path, process-scoped runtime selection. |
| **v0.2.0 — Dispatch inspector** | Memory pools, signals, queues, packet validation, code-object metadata, structured dispatch traces. **Phases 3–5.** |
| **v0.3.0 — Functional execution preview** | Explicit functional input format, basic arithmetic/loads/stores/indexing, deterministic scheduling. **Phase 6** (after the functional-input research gate). |
| **v0.4.0 — Correctness alpha** | Shared memory, divergence, barriers, selected atomics; memory/race/barrier checks for a declared subset. **Phases 7–8.** Event/replay scaffolding starts with the functional engine, not only at 0.5. |
| **v0.5.0 — Reproducible debugging beta** | Replay bundles, stepping, state inspection, controlled schedule exploration, failure minimization. **Phase 9.** |
| **v0.6.0 — Native gfx1201 preview** | Narrow ISA decoder/interpreter, code-object loading, required launch state and instruction families. **Phases 10–11.** Collect hardware observations for each new semantic/instruction family when hardware is available. |
| **v0.9.0 — Hardware-validated RC** | Differential hardware suite, verified profile fields, FP comparison rules, discrepancy tracking. **Phase 12** consolidates earlier evidence. |
| **v1.0.0 — Supported developer tool** | Reliable install, stable diagnostic contracts, versioned replay/profile formats, documented compatibility and upgrade policy. **Phase 13** plus hardening. |

## Quick start

Pinned toolchain: **Rust 1.85.0** (`rust-toolchain.toml`). MSRV: **1.85**. Nightly host features: **prohibited**.

```bash
# Install / run (crates.io, once published):
#   cargo install softgpu --locked
# From a checkout:
cargo test --workspace --locked
cargo run --locked -- info
cargo run --locked -- validate-profile profiles/softgpu-generic-v0.json
cargo build -p softgpu-hsa --locked
cc -I third_party/rocr-headers -o target/hsa-layout-probe tools/hsa-layout-probe/probe.c && ./target/hsa-layout-probe
# Linux x86_64 + ROCm 7.14 load proof (CI): see environments/rocm-x86_64/README.md
cargo fmt --check
cargo clippy --workspace --locked --all-targets -- -D warnings
```

Cargo emits `libhsa_runtime64`. For ROCr-style substitution on Linux, the Phase 1 script stages `libhsa-runtime64.so`.

ROCm integration details and local reproduction (Docker / Apple Container): [environments/rocm-x86_64/README.md](environments/rocm-x86_64/README.md).

## Repository map

```text
crates/softgpu-core/   # handles, runtime, agents, traces, profiles
crates/softgpu-hsa/    # cdylib HSA adapter (minimal exports)
src/                   # softgpu CLI
profiles/              # versioned device profiles with provenance
third_party/rocr-headers/  # pinned hsa.h for probes (NCSA)
tests/                 # CLI/profile tests
docs/                  # architecture, status, sources, ADRs, articles
```

## CI workflows

| Workflow | Purpose |
| --- | --- |
| [CI](.github/workflows/ci.yml) | Format, tests, clippy, layout probe, ROCm/HIP load+discovery gate |
| [Coverage](.github/workflows/coverage.yml) | `tools/coverage.sh` (`cargo llvm-cov`, line/function floors) → Coveralls + HTML artifact |
| [Code quality](.github/workflows/code-quality.yml) | fmt, clippy `-D warnings`, `cargo doc -D warnings` |
| [Dependencies](.github/workflows/dependencies.yml) | `cargo-deny` (licenses, advisories, sources) + Dependabot |
| [Release](.github/workflows/release.yml) | Tag `vX.Y.Z` → crates.io publish + GitHub Release |

**Release secrets:** set repository secret `CARGO_REGISTRY_TOKEN` (crates.io API token) before tagging `v0.2.0`. Coveralls uses `GITHUB_TOKEN` via the Coveralls GitHub App (enable the repo on [coveralls.io](https://coveralls.io)).

Linux builds of `softgpu-hsa` need the workspace [`.cargo/config.toml`](.cargo/config.toml) linker wrapper (or an equivalent) so the cdylib advertises ELF version **`ROCR_1`** for HIP. Cloning this repo already includes that config.

## Documentation

- [Status](docs/status.md) — what works, active phase, next gate
- [Changelog](CHANGELOG.md)
- [Architecture](docs/architecture.md)
- [Support matrix](docs/support-matrix.md)
- [Sources ledger](docs/sources.md)
- [Unsafe / FFI policy](docs/unsafe-ffi-policy.md)
- [ROCm x86_64 env](environments/rocm-x86_64/README.md) — pinned image; local CI via Docker or Apple Container
- [ADR-0001](docs/adr/0001-rocr-hsa-substitution-boundary.md) · [ADR-0002](docs/adr/0002-generation-safe-handles.md)
- [Article 1](docs/articles/01-why-developer-oriented-virtual-gpu.md) · [Article 2](docs/articles/02-gpu-stack-hip-to-silicon.md) · [Article 3](docs/articles/03-impersonating-a-gpu-without-lying.md) · [Article 4](docs/articles/04-hsa-queues-and-signals.md) · [Article 5](docs/articles/05-hsa-aql-dispatch.md) · [Article 6](docs/articles/06-fat-binaries-elf-code-objects.md) · [Article 7](docs/articles/07-emulation-vs-simulation.md)
- [Article 19 (draft)](docs/articles/19-why-rust-for-software-gpu.md)
- [Phase 3 concurrency invariants](docs/concurrency-phase3.md)
- [AQL diagnostic contract](docs/aql-diagnostic-contract.md)
- [Code-object metadata](docs/code-object.md)
- [Functional IR path](docs/functional-path.md)

## License

Dual-licensed under Apache-2.0 OR MIT. See [`LICENSE`](LICENSE) and [`LICENSE-MIT`](LICENSE-MIT).
