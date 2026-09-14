# SoftGPU

SoftGPU is a **Rust-first**, developer-oriented GPU **emulation, testing, debugging, sanitization, and CI** runtime. It aims to let real AMD HIP userspace talk to a SoftGPU ROCr/HSA compatibility adapter, then execute and diagnose kernels on a vendor-neutral core—without pretending to be a cycle-accurate Radeon AI PRO R9700 or inventing undocumented AMD behavior.

> **Phase 1 status:** SoftGPU ships a minimal `libhsa-runtime64` (implemented APIs + fail-closed stubs) and a HIP-linked load-proof harness for pinned ROCm **7.14.0**. Queues/AQL/kernels remain unsupported. Treat HIP integration as proven only when the `rocm-integration` CI job is green.

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

## Quick start (Phase 2)

Pinned toolchain: **Rust 1.85.0** (`rust-toolchain.toml`). MSRV: **1.85**. Nightly host features: **prohibited**.

```bash
# One documented command for Apple Silicon macOS and Linux x86-64:
cargo test --workspace --locked

# Also useful:
cargo run --locked -- info
cargo run --locked -- validate-profile profiles/softgpu-generic-v0.json
cargo build -p softgpu-hsa --locked
cc -I third_party/rocr-headers -o target/hsa-layout-probe tools/hsa-layout-probe/probe.c && ./target/hsa-layout-probe
# Linux x86_64 + ROCm 7.14 (CI / docker):
#   bash environments/rocm-x86_64/ci-entrypoint.sh
cargo fmt --check
cargo clippy --workspace --locked --all-targets -- -D warnings
```

Cargo emits `libhsa_runtime64`. For ROCr-style substitution on Linux, the Phase 1 script stages `libhsa-runtime64.so`.

## Repository map (Phase 2)

```text
crates/softgpu-core/   # handles, runtime, agents, traces, profiles
crates/softgpu-hsa/    # cdylib HSA adapter (minimal exports)
src/                   # softgpu CLI
profiles/              # versioned device profiles with provenance
third_party/rocr-headers/  # pinned hsa.h for probes (NCSA)
tests/                 # CLI/profile tests
docs/                  # architecture, status, sources, ADRs, articles
```

## Documentation

- [Status](docs/status.md) — what works, active phase, next gate
- [Architecture](docs/architecture.md)
- [Support matrix](docs/support-matrix.md)
- [Sources ledger](docs/sources.md)
- [Unsafe / FFI policy](docs/unsafe-ffi-policy.md)
- [ADR-0001](docs/adr/0001-rocr-hsa-substitution-boundary.md) · [ADR-0002](docs/adr/0002-generation-safe-handles.md)
- [Article 1](docs/articles/01-why-developer-oriented-virtual-gpu.md) · [Article 2](docs/articles/02-gpu-stack-hip-to-silicon.md) · [Article 3](docs/articles/03-impersonating-a-gpu-without-lying.md)
- [Article 19 (draft)](docs/articles/19-why-rust-for-software-gpu.md)

## License

Dual-licensed under Apache-2.0 OR MIT. See `LICENSE`.
