# SoftGPU AMD code-object parsing (Phase 5 + v0.8 load)

**Access date:** 2026-09-16  
**Fidelity:** ABI (metadata) + SoftGPU executable `.text` extract for declared fixtures  
**Does not claim:** unrestricted hipcc execute, full AMDHSA `.kd` preload, or full LLVM metadata coverage.

## Supported subset

| Item | SoftGPU behavior |
| --- | --- |
| Container | Little-endian ELF64 (`ET_DYN` / `ET_EXEC` / `ET_REL`) |
| Note | Owner `AMDGPU`, type `NT_AMDGPU_METADATA` (32), MessagePack descriptor |
| `amdhsa.version` | `[1,0]`, `[1,1]`, `[1,2]` only (fail closed otherwise) |
| `amdhsa.target` | Must contain `gfx1201` |
| Kernels | `.name`, `.symbol`, segment sizes/align, `.args` reflection fields |
| `.text` (v0.8) | Extracted for SoftGPU executable load; single-kernel SoftGPU fixtures |

Size / complexity caps live in `softgpu-amd-code-object` (16 MiB file, section/note/msgpack node limits).

## Inspection vs load

```bash
softgpu inspect-code-object path/to/codeobject   # metadata only
# HSA: hsa_code_object_reader_create_from_memory → load_agent → freeze → symbol
```

SoftGPU executable load uses SoftGPU CC (`s[4:5]` kernarg) for SoftGPU fixtures.
Arbitrary official-compiler `.text` may trap until ISA coverage grows.

## Fixtures and provenance

| Fixture | Provenance |
| --- | --- |
| SoftGPU synthetic ELF builders (`fixture::*`) | `softgpu-synthetic` |
| `fixture_tiny_add_with_text` | metadata + llvm-mc `tiny_add` `.text` |
| Optional ROCm-compiled HSACO | `official-hipcc` — see `fixtures/amd-code-object/PROVENANCE.md` |
