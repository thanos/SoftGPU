# SoftGPU architecture (Phase 0)

## Long-term stack

```text
HIP application and kernels
        |
        v
AMD compiler + real HIP runtime
        |
        v
SoftGPU ROCr/HSA compatibility adapter   <-- Phase 1+
        |
        +--> agent, region/pool, queue, signal, executable, loader
        +--> AQL packet observation and validation
        |
        v
Vendor-neutral SoftGPU core
        |
        +--> functional execution engine
        +--> deterministic scheduler
        +--> memory/race/barrier sanitizers
        +--> debugger and structured traces
        +--> optional analytical performance model
        |
        v
AMD code-object frontend and, eventually, gfx1201 ISA interpreter
```

Phase 0 implements **none** of the adapter or engines. It only defines contracts, vocabulary, and a profile schema so later phases fail closed consistently.

## Boundaries

| Layer | Owns | Must not own |
| --- | --- | --- |
| ROCr/HSA adapter (future) | ABI, handles, lifecycle, error mapping, queues/signals | Instruction semantics |
| Vendor-neutral core (future) | scheduling, abstract memory, diagnostics, sanitizer events | AMD names/encodings as dependencies |
| AMD frontends (future) | code objects, AQL, gfx12/gfx1201 | Silent success on unknown encodings |
| Profiles | versioned identity/limits/provenance | Invented marketing numbers |

## Device profile schema (v1)

Profiles live under `profiles/*.json` and are validated by `softgpu::profile::DeviceProfile`.

Required ideas:

- `schema_version` (currently `1`)
- `profile_id` / `profile_revision`
- identity (`vendor`, `product_name`, `architecture_family`, `llvm_target` with provenance)
- `max_fidelity` and `conformance_allowed`
- resource limits as `ProfileField<T>` with provenance `verified|observed|inferred|unknown|provisional`
- analytical performance section kept empty/non-authoritative in Phase 0

**Unknown is valid.** `conformance_allowed=true` is rejected unless identity/limit fields used for legality have verified or observed provenance, and never for `hardware-conformant` until Phase 12.

## Error taxonomy

Categories: `config`, `profile`, `unsupported`, `validation`, `internal`, `io`.

CLI maps categories to nonzero exit codes. Future HSA status mapping happens only at the ABI boundary.

## Fidelity

See `softgpu::fidelity::FidelityLevel` and README. Rust host memory safety is **not** a fidelity level.

## Crate layout policy

Start as one crate (`softgpu`). Split `cdylib` HSA adapter and libraries only when Phase 1 load proof demands it. No speculative workspace sprawl in Phase 0.
