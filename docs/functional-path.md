# SoftGPU Functional IR path (Phases 6–9)

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
- Phase 9: versioned traces, breakpoints, inspection, seeded schedule exploration

## Path from source to result

```text
fixtures / SoftGPU-authored builders
        |  softgpu-sfir-v1
        v
softgpu-functional interpreter (+ optional sanitizer / debugger)
  (lex workitem | wave_barrier schedule; optional schedule_seed)
        v
CPU arena updates + RunReport [+ SanitizeReport / DebugRunReport]
```

Verify:

```bash
cargo test -p softgpu-functional --locked
cargo run --locked -- run-functional --builtin tiny_add --n 256 --wg 64
cargo run --locked -- run-functional --builtin tiny_add --sanitize collect
cargo run --locked -- debug-functional --builtin tiny_add --break-step 5
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

`schedule_seed` bit0 reverses SoftGPU wave order under `wave_barrier` (Phase 9).

Programs containing `barrier` **require** `wave_barrier`. Barriers inside `if`/`while`
are rejected (divergent barriers unsupported).

## Sanitizer (Phase 8)

| Mode | Behavior |
| --- | --- |
| `off` | No shadow / race instrumentation (default) |
| `collect` | Record soft findings; hard faults still fail |
| `fail_fast` | Stop at the first finding |

## Debugger (Phase 9)

| Feature | Behavior |
| --- | --- |
| Trace | `softgpu-debug-trace-v1` JSONL with event budget |
| Breakpoints | SoftGPU `after_step` / `on_memory_access` |
| Inspection | Registers + actor; provenance only (no invented source lines) |
| Explore | Seeded search + minimize SoftGPU race findings |

See Articles 9–10.

## Honesty

All reports include `note=not_gfx1201_isa_emulation`. HIP/HSA AQL diagnostic
complete (Phase 4) remains separate and is still not kernel success.
