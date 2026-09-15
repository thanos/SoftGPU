# Article 9 — Building GPU sanitizers on SoftGPU

- **Audience:** SoftGPU contributors and GPU correctness engineers
- **Prerequisites:** Articles 1–8
- **Evidence:** SoftGPU Phase 8 `softgpu-functional` sanitizer + `phase8_sanitize` tests
- **Access date:** 2026-09-15

## Problem

Hardware runs hide many memory and synchronization defects until a rare schedule
hits them. SoftGPU’s value for developers is to detect a **declared** class of
defects with workgroup/wave/lane context, without pretending to be an AMDGPU
memory-model oracle.

## SoftGPU sanitizer model (Phase 8)

| Concept | SoftGPU meaning |
| --- | --- |
| **Shadow** | Per-byte allocated / uninitialized / initialized / freed for global and group arenas |
| **Hard faults** | Out-of-bounds, use-after-free, shadow-size limit — always fail closed |
| **Soft findings** | Uninitialized read, race, missing barrier, cross-workgroup race — fail-fast or collect |
| **Happens-before** | SoftGPU barrier generations under `wave_barrier`; same generation ⇒ no SoftGPU sync |

Modes: `off`, `collect`, `fail_fast` (`SanitizeMode`). Findings carry actor
`WorkItemId` (workgroup, wave, lane, flat local) and optional `other` actor.

Replay: every finding can be packaged as `softgpu-sanitizer-replay-v1`
(`ReplayBundle`) with program name, provenance, and exec config.

## Declared race subset

SoftGPU reports a conflict when two **different** workitems touch overlapping
bytes with at least one non-atomic write, and:

- **same workgroup, same barrier generation** → `race` (global) or
  `missing_barrier` (group);
- **different workgroups** on global → `cross_workgroup_race`.

SoftGPU atomics on both sides of an access are treated as ordered by the
sequential interpreter (no SoftGPU race between those atomics).

## Blind spots (honest)

- Not AMDGPU acquire/release/scope evidence
- Not a claim about silent host-pointer aliasing outside SoftGPU arenas
- Not true hardware concurrency; schedule order alone does not define SoftGPU races
- Divergent barriers remain validate-time errors (Phase 7), not sanitizer findings

## What we verified / what remains assumed

| Verified | Assumed / out of scope |
| --- | --- |
| OOB, UAF, uninit group read, WG race, missing barrier, cross-WG race in SFIR suite | Hardware memory orders |
| Clean `tiny_add` / `group_exchange` / SoftGPU atomics under Collect | Full GPU sanitizer parity with vendor tools |
| Deterministic replay JSON for a finding | Source-level mapping |

## Failure cases

```bash
cargo test -p softgpu-functional --test phase8_sanitize --locked
cargo run --locked -- run-functional --builtin tiny_add --sanitize collect
```

## Next gate

Phase 9 — debugger and deterministic exploration on sanitizer findings.

## References

- [`docs/functional-path.md`](../functional-path.md)
- `crates/softgpu-functional/src/sanitize.rs`
- `crates/softgpu-functional/tests/phase8_sanitize.rs`
