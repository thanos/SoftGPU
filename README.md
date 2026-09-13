# SoftGPU

SoftGPU is a **Rust-first**, developer-oriented GPU **emulation, testing, debugging, sanitization, and CI** runtime. It aims to let real AMD HIP userspace talk to a SoftGPU ROCr/HSA compatibility adapter, then execute and diagnose kernels on a vendor-neutral core—without pretending to be a cycle-accurate Radeon AI PRO R9700 or inventing undocumented AMD behavior.

> **Phase 0 status:** charter, evidence baseline, and skeleton only. There is **no** ROCr/HSA shared library yet. Do not claim HIP/ROCm compatibility from this tree.

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

## Quick start (Phase 0)

Pinned toolchain: **Rust 1.85.0** (`rust-toolchain.toml`). MSRV: **1.85**. Nightly host features: **prohibited**.

```bash
# One documented command for Apple Silicon macOS and Linux x86-64:
cargo test --locked

# Also useful:
cargo run --locked -- info
cargo run --locked -- validate-profile profiles/softgpu-generic-v0.json
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
```

## Repository map (Phase 0)

```text
src/                 # single crate skeleton (errors, fidelity, profiles, CLI)
profiles/            # versioned device profiles with provenance
tests/               # profile schema + negative CLI tests
docs/                # architecture, status, sources, ADRs, articles
ci/                  # CI notes; GitHub Actions under .github/workflows/
baoulo/prompts/      # execution charter (not a runtime dependency)
```

Crate splits (`softgpu-hsa`, `softgpu-core`, …) wait until Phase 1+ proves a boundary.

## Documentation

- [Status](docs/status.md) — what works, active phase, next gate
- [Architecture](docs/architecture.md)
- [Support matrix](docs/support-matrix.md)
- [Sources ledger](docs/sources.md)
- [Unsafe / FFI policy](docs/unsafe-ffi-policy.md)
- [ADR-0001: ROCr/HSA substitution boundary](docs/adr/0001-rocr-hsa-substitution-boundary.md)
- [Article 1](docs/articles/01-why-developer-oriented-virtual-gpu.md)
- [Article 19 (draft)](docs/articles/19-why-rust-for-software-gpu.md)

## License

Dual-licensed under Apache-2.0 OR MIT. See `LICENSE`.
