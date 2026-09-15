# Article 8 — Grids, workgroups, waves, lanes, and SoftGPU synchronization

- **Audience:** SoftGPU contributors and GPU runtime engineers
- **Prerequisites:** Articles 1–7
- **Evidence:** SoftGPU Phase 7 `softgpu-functional` + `docs/functional-path.md`
- **Access date:** 2026-09-15

## Problem

“Wave,” “warp,” and “subgroup” mean different things on different vendors. SoftGPU
must define a **software** semantic machine for Phase 7 without pretending it is
gfx1201 silicon.

## SoftGPU vocabulary (Phase 7)

| Term | SoftGPU meaning |
| --- | --- |
| **Grid / workgroup / local id** | Same indexing formulas as Phase 6 |
| **Wave** | Contiguous block of `wave_size` flat local ids inside a workgroup |
| **Lane** | Index of a workitem inside its SoftGPU wave (`flat_local % wave_size`) |
| **Group memory** | Per-workgroup CPU arena; not R9700 LDS capacity evidence |
| **Barrier** | SoftGPU generation sync between barrier-separated program segments |
| **Divergence** | Structured `if`/`while` with per-lane masks; reconverge after the construct |

Partial waves (workgroup size not divisible by `wave_size`) are first-class: the last
wave simply has fewer live lanes.

## Barrier model

SoftGPU splits a program body on top-level `barrier` ops. For each segment, every
wave runs the segment to completion (lockstep within the wave for divergent `if`).
Then the next segment begins. That is a deterministic **wave_barrier** schedule.

Divergent barriers (barrier inside `if`/`while`) are **unsupported** and fail
validation.

## Atomics

`atomic_add` on global or group returns the previous i32 value. SoftGPU applies a
**functional** sequential atomic on the host arena. Named `scope`/`order` fields are
recorded in the IR for honesty but do **not** claim AMDGPU memory-model semantics.

## What this is not

- Not gfx1201 wavefront scheduling
- Not a proof of HIP kernel success via AQL
- Not a sanitizer completeness claim beyond Phase 8’s declared SoftGPU subset
  (see Article 9)

## References

- [`docs/functional-path.md`](../functional-path.md)
- `crates/softgpu-functional/tests/phase7_semantics.rs`
