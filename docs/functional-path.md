# SoftGPU Functional IR path (Phase 6)

**Access date:** 2026-09-15  
**Fidelity:** `functional`  
**Marker:** `softgpu_functional_cpu_not_gfx1201_isa`  
**Does not claim:** gfx1201 ISA emulation, HIP AQL kernel success, or recovery of IR from arbitrary AMD code objects.

## Research gate (locked)

SoftGPU Phase 6 executes **SoftGPU Functional IR** (`softgpu-sfir-v1`):

- SoftGPU-owned JSON IR with an explicit schema
- Hand-translated from tiny reference C sources (or SoftGPU-authored)
- Fully disclosed ops: ids, const, add/sub/mul, kernarg load, global load/store, ret

SoftGPU does **not** assert that final AMDGPU ELF/code objects contain executable
high-level IR. Code-object parsing (Phase 5) remains metadata-only.

## Path from source to result

```text
fixtures/functional/tiny_add.ref.c     (host reference semantics)
        |  hand translation (documented)
        v
softgpu-sfir-v1 JSON / Rust builder
        |  softgpu-functional interpreter
        v
CPU global arena updates + RunReport (fidelity=functional)
```

Verify:

```bash
cargo test -p softgpu-functional --locked
cargo run --locked -- run-functional --builtin tiny_add --n 256 --wg 64
cargo run --locked -- run-functional fixtures/functional/tiny_add.sfir.json
```

## Address model

Functional “pointers” are **byte offsets** into a SoftGPU `GlobalArena`.
Kernarg is a separate host blob (pointer fields are arena offsets).

## Scheduler

Deterministic: workgroups in lexicographic order, then local ids lexicographic.
No wave/lane model yet (Phase 7).

## Honesty

All reports include `note=not_gfx1201_isa_emulation` and
`mode=softgpu_functional_cpu_not_gfx1201_isa`. HIP/HSA AQL diagnostic complete
(Phase 4) remains separate and is still not kernel success.
