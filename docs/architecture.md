# SoftGPU architecture (Phase 10)

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
AMD code-object frontend + gfx1201 ISA interpreter (narrow subset)
```

Phase 0–9 cover contracts through SoftGPU Functional IR debugging.
Phase 10 adds `softgpu-amd-isa`: a sourced, fail-closed decoder/interpreter for
the named SALU subset `softgpu-gfx1201-salu-v1` (Architectural ISA fidelity for
that subset only). End-to-end HIP AQL kernel success remains Phase 11.

## Boundaries

| Layer | Owns | Must not own |
| --- | --- | --- |
| ROCr/HSA adapter | ABI, handles, lifecycle, error mapping, queues/signals, AQL diagnostic | Instruction semantics / silent kernel success |
| Vendor-neutral core | scheduling contracts, traces, profiles, Path C memory | AMD names/encodings as dependencies |
| SoftGPU Functional IR | disclosed SFIR ops, CPU arena, deterministic WG schedule | gfx1201 encodings; claiming code-object recovery |
| AMD frontends | code-object metadata, AQL observation, gfx1201 ISA subset | Silent success on unknown encodings |
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

Workspace members:

- `softgpu` — CLI
- `softgpu-core` — vendor-neutral runtime, handles, agents, traces, profiles, Path C, AQL diagnostic
- `softgpu-amd-code-object` — bounded ELF64 + AMDHSA metadata inspect
- `softgpu-amd-isa` — gfx1201 SALU subset decoder/interpreter (Phase 10)
- `softgpu-functional` — SoftGPU Functional IR (`softgpu-sfir-v1`) CPU executor
- `softgpu-hsa` — Linux-oriented `cdylib` HSA adapter (`libhsa_runtime64`)
