# SoftGPU AMD code-object parsing (Phase 5)

**Access date:** 2026-09-14  
**Fidelity:** ABI (metadata inspection)  
**Does not claim:** gfx1201 ISA execution, HIP launch success, or full LLVM metadata coverage.

## Supported subset

| Item | SoftGPU behavior |
| --- | --- |
| Container | Little-endian ELF64 (`ET_DYN` / `ET_EXEC` / `ET_REL`) |
| Note | Owner `AMDGPU`, type `NT_AMDGPU_METADATA` (32), MessagePack descriptor |
| `amdhsa.version` | `[1,0]`, `[1,1]`, `[1,2]` only (fail closed otherwise) |
| `amdhsa.target` | Must contain `gfx1201` |
| Kernels | `.name`, `.symbol`, segment sizes/align, `.args` reflection fields |

Size / complexity caps live in `softgpu-amd-code-object` (16 MiB file, section/note/msgpack node limits).

## Fixtures and provenance

| Fixture | Provenance |
| --- | --- |
| SoftGPU synthetic ELF builders (`fixture::*`) | `softgpu-synthetic` — structurally valid notes for host tests |
| Optional ROCm-compiled HSACO | `official-hipcc` — regenerated in pinned ROCm env; see `fixtures/amd-code-object/PROVENANCE.md` |

Synthetic fixtures are lawful SoftGPU test inputs. They are **not** claimed to be bit-identical to a particular compiler revision unless PROVENANCE records a hash from the pinned toolchain.

## Inspection

```bash
softgpu inspect-code-object path/to/codeobject
```

Output is JSON with `fidelity=abi` and `note=metadata_only_no_isa_execution`.

## Fuzz floor

`tests/phase5_code_object.rs::fuzz_smoke_no_panic_bounded`: ≥100 mutations (budget up to 2000) within 500 ms; no panic; all paths fail closed.

## Cross-check

When `llvm-readelf` is available in the pinned ROCm image, SoftGPU’s note discovery is compared for presence of an `AMDGPU` metadata note (see `environments/rocm-x86_64/run-phase5-code-object.sh`).
