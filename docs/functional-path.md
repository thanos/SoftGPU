# SoftGPU Functional IR path (Phases 6–7)

**Access date:** 2026-09-15  
**Fidelity:** `functional`  
**Marker:** `softgpu_functional_cpu_not_gfx1201_isa`  
**Does not claim:** gfx1201 ISA emulation, HIP AQL kernel success, or recovery of IR from arbitrary AMD code objects.

## Research gate (locked)

SoftGPU executes **SoftGPU Functional IR** (`softgpu-sfir-v1`):

- SoftGPU-owned JSON IR with an explicit schema
- Hand-translated or SoftGPU-authored programs with disclosed provenance
- Phase 6 ops: ids, const, add/sub/mul, kernarg load, global load/store, ret
- Phase 7 ops: lane/wave ids, group load/store, barrier, structured `if`/`while`,
  compares, `and`, selected `atomic_add`

## Path from source to result

```text
fixtures / SoftGPU-authored builders
        |  softgpu-sfir-v1
        v
softgpu-functional interpreter
  (lex workitem | wave_barrier schedule)
        v
CPU global (+ group) arena updates + RunReport
```

Verify:

```bash
cargo test -p softgpu-functional --locked
cargo run --locked -- run-functional --builtin tiny_add --n 256 --wg 64
```

## Address model

- **Global:** byte offsets into SoftGPU `GlobalArena`
- **Group:** per-workgroup arena (`program.group_bytes` / launch config); cleared each WG
- Kernarg is a separate host blob (pointer fields are arena offsets)

## Scheduler (Phase 7)

| Policy | Behavior |
| --- | --- |
| `lex_workitem` | Phase 6: each workitem runs to completion in lex order |
| `wave_barrier` | SoftGPU waves of `wave_size`; barrier-separated segments sync the WG |

Programs containing `barrier` **require** `wave_barrier`. Barriers inside `if`/`while`
are rejected (divergent barriers unsupported).

SoftGPU `wave_size` is a **software** parameter (default 32). It is not a claim about
R9700/gfx1201 wavefront hardware.

## Honesty

All reports include `note=not_gfx1201_isa_emulation` and
`mode=softgpu_functional_cpu_not_gfx1201_isa`. HIP/HSA AQL diagnostic complete
(Phase 4) remains separate and is still not kernel success.
