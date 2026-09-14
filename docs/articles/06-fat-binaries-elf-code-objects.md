# Article 6 — Fat binaries, ELF, code objects, metadata, and gfx targets

- **Audience:** runtime engineers and SoftGPU contributors
- **Prerequisites:** Articles 1–5
- **Evidence:** SoftGPU Phase 5 `softgpu-amd-code-object` + fixtures
- **Access date:** 2026-09-14

## Problem

HIP applications ship **device binaries**, not source. SoftGPU cannot “run a
kernel” until it can **safely identify** which ELF notes and metadata describe
that kernel — without inventing fields or decoding ISA prematurely.

## Fat binary vs code object

```text
HIP fat binary / bundle
    └── one or more device images
            └── AMDGPU code object (ELF)
                    ├── machine code sections (ISA — SoftGPU does not execute yet)
                    └── notes: AMDGPU / NT_AMDGPU_METADATA (MessagePack)
```

A **code object** is the ELF image the HSA loader registers. SoftGPU Phase 5
reads the **metadata note**, not the ISA bytes.

## ELF notes SoftGPU cares about

For code-object V3+ (LLVM AMDGPUUsage):

| Owner | Type | Payload |
| --- | --- | --- |
| `AMDGPU` | `NT_AMDGPU_METADATA` (32) | MessagePack map |

Important keys:

- `amdhsa.version` — SoftGPU accepts `[1,0]`…`[1,2]`
- `amdhsa.target` — SoftGPU requires `gfx1201` substring
- `amdhsa.kernels[]` — name, symbol, kernarg/group/private sizes, `.args`

## Why bounds matter

Code objects are untrusted input. SoftGPU uses a **bounded** ELF walker and
MessagePack decoder (file size, section count, note count, string/map/array
limits, recursion depth). Oversized or exotic encodings fail closed.

## gfx targets

`amdhsa.target` strings look like `amdgcn-amd-amdhsa--gfx1201`. SoftGPU’s Phase 5
gate is intentionally narrow: **gfx1201 only**. Other targets are rejected with
`UnsupportedTarget`, not silently coerced.

## Inspection without lying

`softgpu inspect-code-object` prints JSON labeled `metadata_only_no_isa_execution`.
Finding a kernel name is not proof SoftGPU can execute it.

## What comes next

Phase 6 chooses a **disclosed functional** execution path for a tiny kernel —
still not gfx1201 ISA emulation.

## References

- [`docs/code-object.md`](../code-object.md)
- LLVM AMDGPUUsage (code-object metadata) — see [`docs/sources.md`](../sources.md)
- SoftGPU fixtures under `fixtures/amd-code-object/`
