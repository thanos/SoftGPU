# Article 19 — Why Rust for a software GPU, and what Rust cannot prove

- **Status:** early draft — revise at ABI, queue, parser, and ISA milestones
- **Audience:** engineers choosing an implementation language for GPU systems tooling
- **Prerequisites:** basic Rust and C ABI familiarity
- **Access date:** 2026-09-13

## Two different “Rust GPU” stories

| Story | What it means | SoftGPU stance |
| --- | --- | --- |
| Rust **host** runtime | Emulator, adapters, parsers, sanitizers, CLI in Rust | **Primary path** |
| Rust **device** kernels | `amdgcn-amd-amdhsa` / rust-gpu style kernels | Optional research; **not** Phase 1–6 gate |

The rustc book currently labels `amdgcn-amd-amdhsa` as **Tier 3** with special build requirements ([rustc platform support](https://doc.rust-lang.org/rustc/platform-support/amdgcn-amd-amdhsa.html), accessed 2026-09-13). SoftGPU’s compatibility story uses the **real HIP compiler** and real HIP userspace.

## Why Rust fits this project

- Explicit ownership helps manage handle tables, packet buffers, and trace lifetimes—once those exist.
- Enums/newtypes make fidelity levels, error categories, and profile provenance harder to mix up accidentally (see Phase 0 code).
- A small, auditable `unsafe` surface at the future cdylib boundary is preferable to a large implicit unsafe culture—**if** reviewed per `docs/unsafe-ffi-policy.md`.
- Ecosystem tooling (`clippy`, rustfmt, fuzzers, Miri where applicable) supports a fail-closed CI culture.

## What Rust cannot prove

Rust’s type system does **not** prove:

- ROCr/HSA **C ABI** layouts, symbol versions, or calling conventions;
- AQL producer/consumer memory ordering against a foreign runtime;
- gfx1201 instruction semantics;
- that a kernel is data-race-free on a GPU memory model;
- employment outcomes or “we replace an R9700.”

Those require specifications, independent C probes, traces, differential tests, and eventually hardware.

## Rust vs Zig (for SoftGPU specifically)

Zig is excellent for explicit memory and cross-compilation ergonomics. SoftGPU chooses Rust for the host runtime to leverage algebraic types, a strong testing culture, and ecosystem sanitizers/fuzzers for a long-lived evidence system. This is a project-local choice, not a universal ranking. Existing Zig inference work (e.g. Zynfer) remains a **separate** client/baseline, not something SoftGPU rewrites.

## Phase 0 “unsafe” case study

Phase 0 deliberately contains **no** `unsafe` and **no** FFI exports. The educational point is negative space: do not open an ABI surface until layout probes and panic policy exist (Phase 1). A before/after defect study will be added when the first reviewed `unsafe` block lands.

## Reproducible commands

```bash
cargo test --locked
cargo run --locked -- info
# Expect: rocr_hsa_library=not-implemented
```

## What we verified / what remains assumed

| Verified | Assumed / later |
| --- | --- |
| Stable Rust 1.85 builds the skeleton without nightly | cdylib export strategy details |
| Policy docs for FFI/`unsafe` exist before ABI code | Panic containment strategy under real HIP load |
| Article distinguishes host Rust vs device Rust | Concrete unsafe audit of queue atomics (Phase 3) |

## References

- https://doc.rust-lang.org/nomicon/
- https://doc.rust-lang.org/rustc/platform-support/amdgcn-amd-amdhsa.html
- SoftGPU `docs/unsafe-ffi-policy.md`, ADR-0001

## Next revision triggers

Revise this article when Phase 1 lands the first exported function, when queues introduce foreign-memory atomics, when parsers add `unsafe`, and when any ISA decoder tables are generated.
