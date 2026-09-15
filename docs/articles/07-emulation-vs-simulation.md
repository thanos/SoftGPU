# Article 7 — Emulation versus simulation and SoftGPU’s functional path

- **Audience:** runtime engineers and SoftGPU contributors
- **Prerequisites:** Articles 1–6
- **Evidence:** SoftGPU Phase 6 `softgpu-functional` + `docs/functional-path.md`
- **Access date:** 2026-09-15

## Problem

People say “GPU emulator” when they mean very different things. SoftGPU must
name which kind of execution it is performing, or it will silently claim
hardware fidelity it does not have.

## Emulation vs simulation (SoftGPU vocabulary)

| Word people use | SoftGPU meaning |
| --- | --- |
| **ABI / protocol observation** | Call the same APIs; validate packets; do not run kernels (Phases 1–5) |
| **Functional execution** | Run a disclosed IR on the CPU with GPU-like ids/memory; results are semantic, not ISA evidence (Phase 6) |
| **Architectural ISA** | Decode/execute real gfx1201 encodings with sourced tables (Phase 10+) |
| **Cycle-accurate simulation** | Not SoftGPU’s goal |

Phase 6 is **functional execution**, not ISA emulation.

## Why SoftGPU Functional IR (not “run the code object”)

AMD code objects are ELF + MessagePack metadata + machine code. SoftGPU can
**inspect** metadata (Phase 5). Claiming that those binaries contain a portable
high-level IR SoftGPU can execute would be dishonest without toolchain proof.

So Phase 6 locks a **SoftGPU-owned** IR (`softgpu-sfir-v1`):

1. Tiny reference C (`tiny_add.ref.c`) defines intended semantics.
2. A hand translation produces SFIR (JSON + Rust builder).
3. The CPU interpreter runs SFIR under a deterministic workgroup schedule.
4. Every report is labeled `not_gfx1201_isa_emulation`.

## What “global id” means here

SFIR exposes `global_id` / `local_id` / `workgroup_id` as SoftGPU software
identifiers. They match the usual HIP/OpenCL indexing formulas for the launch
configuration SoftGPU was given. They are not evidence of how a particular
wavefront scheduler would order memory on silicon.

## What comes next

Phase 7 adds waves/lanes, group memory, and barriers on top of a semantic
machine. ISA interpretation remains later and must stay evidence-driven.

## References

- [`docs/functional-path.md`](../functional-path.md)
- `crates/softgpu-functional/`
- `fixtures/functional/`
