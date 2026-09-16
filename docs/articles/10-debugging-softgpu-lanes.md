# Article 10 — Debugging a machine made of SoftGPU lanes

- **Audience:** SoftGPU contributors and GPU correctness engineers
- **Prerequisites:** Articles 1–9
- **Evidence:** SoftGPU Phase 9 `softgpu-functional` debugger + `phase9_debug` tests
- **Access date:** 2026-09-15

## Problem

A sanitizer finding is only useful if another engineer can reproduce it, stop at
the responsible SoftGPU step, and inspect wave/lane state—without SoftGPU
inventing source locations it does not have.

## SoftGPU debugger model (Phase 9)

| Concept | SoftGPU meaning |
| --- | --- |
| **Trace** | Versioned JSONL `softgpu-debug-trace-v1` with a hard event budget |
| **Breakpoint** | `after_step` (SoftGPU global step) or `on_memory_access` |
| **Snapshot** | Actor id, registers, op label, program `source_provenance` only |
| **Schedule seed** | `schedule_seed` bit0 reverses SoftGPU wave order under `wave_barrier` |
| **Explore** | Bounded seeded search for SoftGPU race/missing-barrier findings + minimize WG/step |

```text
SFIR program + ExecConfig(+schedule_seed)
        |
        v
interpreter + ExecObserver (DebugSession)
        |
        +--> TraceLog (JSONL)
        +--> DebugStop / SanitizeFinding
        v
inspect / explore_and_minimize_race
```

## Hostile traces

`parse_trace_jsonl` treats input as untrusted: size limits, per-line limits,
schema check, and parse failures—no panics on junk.

## Source mapping policy

SoftGPU **never invents** file/line mappings. `StateSnapshot.source_mapping` is
always `None` unless a future phase verifies a real SFIR→source map. The only
source string is `Program.source_provenance`.

## What we verified / what remains assumed

| Verified | Assumed / out of scope |
| --- | --- |
| Breakpoint accuracy, snapshots, JSONL replay | Interactive TUI debugger |
| Corrupt/oversized traces fail closed | Hardware schedule fidelity |
| Seeded explore finds and minimizes a SoftGPU race | AMDGPU memory-order debugging |

## Commands

```bash
cargo test -p softgpu-functional --test phase9_debug --locked
cargo run --locked -- debug-functional --builtin tiny_add --break-step 5
```

## Next gate

Phase 11 — first end-to-end gfx1201 kernel (after Phase 10 ISA foundation).

## References

- [`docs/functional-path.md`](../functional-path.md)
- `crates/softgpu-functional/src/debug.rs`
- `crates/softgpu-functional/tests/phase9_debug.rs`
