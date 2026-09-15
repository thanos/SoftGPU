# SoftGPU Functional IR path (Phases 6–8)

**Access date:** 2026-09-15  
**Fidelity:** `functional` / `sanitized`  
**Marker:** `softgpu_functional_cpu_not_gfx1201_isa`  
**Does not claim:** gfx1201 ISA emulation, HIP AQL kernel success, or AMDGPU
memory-model equivalence.

## Research gate (locked)

SoftGPU executes **SoftGPU Functional IR** (`softgpu-sfir-v1`):

- SoftGPU-owned JSON IR with an explicit schema
- Hand-translated or SoftGPU-authored programs with disclosed provenance
- Phase 6 ops: ids, const, add/sub/mul, kernarg load, global load/store, ret
- Phase 7 ops: lane/wave ids, group load/store, barrier, structured `if`/`while`,
  compares, `and`, selected `atomic_add`
- Phase 8: optional SoftGPU shadow / happens-before sanitizer on those accesses

## Path from source to result

```text
fixtures / SoftGPU-authored builders
        |  softgpu-sfir-v1
        v
softgpu-functional interpreter (+ optional sanitizer)
  (lex workitem | wave_barrier schedule)
        v
CPU arena updates + RunReport [+ SanitizeReport]
```

Verify:

```bash
cargo test -p softgpu-functional --locked
cargo run --locked -- run-functional --builtin tiny_add --n 256 --wg 64
cargo run --locked -- run-functional --builtin tiny_add --sanitize collect
```

## Address model

- **Global:** byte offsets into SoftGPU `GlobalArena` (host-visible bytes start
  SoftGPU-initialized for sanitizer shadow)
- **Group:** per-workgroup arena; shadow starts uninitialized each WG
- Kernarg is a separate host blob (pointer fields are arena offsets)

## Scheduler (Phase 7)

| Policy | Behavior |
| --- | --- |
| `lex_workitem` | Phase 6: each workitem runs to completion in lex order |
| `wave_barrier` | SoftGPU waves of `wave_size`; barrier-separated segments sync the WG |

Programs containing `barrier` **require** `wave_barrier`. Barriers inside `if`/`while`
are rejected (divergent barriers unsupported).

## Sanitizer (Phase 8)

| Mode | Behavior |
| --- | --- |
| `off` | No shadow / race instrumentation (default) |
| `collect` | Record soft findings; hard faults still fail |
| `fail_fast` | Stop at the first finding |

Finding classes in the declared subset: out-of-bounds, use-after-free,
uninitialized read, race, missing barrier, cross-workgroup race, shadow limit.
See Article 9.

## Honesty

All reports include `note=not_gfx1201_isa_emulation`. HIP/HSA AQL diagnostic
complete (Phase 4) remains separate and is still not kernel success.
