# Article 12 — The first real gfx1201 kernel in SoftGPU

- **Audience:** SoftGPU contributors wiring ISA execution to AQL
- **Prerequisites:** Articles 1–11
- **Evidence:** SoftGPU Phase 11 `softgpu-gfx1201-e2e-tiny-v1`, `phase11_kernel`, `phase11_aql_isa`
- **Access date:** 2026-09-16

## Problem

Phase 10 proved SoftGPU can decode and step a narrow SALU subset. Developers
still need a **real dispatch → machine code → memory result** path without SoftGPU
pretending to run arbitrary hipcc fat binaries.

## What SoftGPU claims in Phase 11

| Claim | SoftGPU meaning |
| --- | --- |
| **Kernel** | `tiny_add`: `b[i] = a[i] + 1` for `i32` elements |
| **Machine code** | llvm-mc `-mcpu=gfx1201` bytes (`TINY_ADD_TEXT`) |
| **Subset** | `softgpu-gfx1201-e2e-tiny-v1` (SMEM/VOP2/GLOBAL + prior SALU) |
| **Calling convention** | SoftGPU sets `s[4:5]=kernarg`, `v0=global_id_x`, EXEC |
| **AQL** | Registered `kernel_object` + SoftGPU kernarg → `softgpu_kernel_success` |
| **Fidelity** | **Architectural ISA** for that subset only |

Unregistered kernels still complete as `diagnostic_complete_no_execution`.

```text
AQL KERNEL_DISPATCH
        |
        v
Runtime: registered SoftGPU ISA image?
        |
        +-- no  --> diagnostic_complete_no_execution
        +-- yes --> run_code_1d (waves) on SoftGPU allocations
                        |
                        v
                 softgpu_kernel_success + completion signal 0
```

## Instruction coverage (enumerated)

- `s_load_b64`, `s_waitcnt`, `s_endpgm`
- `v_lshlrev_b32_e32`, `v_add_nc_u32_e32`
- `global_load_b32`, `global_store_b32`
- (Phase 10 SALU remains available)

Everything else traps.

## Differential

ISA `tiny_add` matches SoftGPU host reference (`tiny_add_host_ref`) and the
Functional IR `tiny_add` semantics (`b[i]=a[i]+1`). Hardware differential is
deferred to Phase 12.

## Provenance

| Artifact | Source |
| --- | --- |
| Encodings / `.text` | llvm-mc (Homebrew LLVM 21.1.8), 2026-09-16 |
| Docs cross-check | LLVM AMDGPUUsage |
| SoftGPU packaging | Hand-maintained tables + `TINY_ADD_TEXT` |

## Commands

```bash
cargo test -p softgpu-amd-isa --test phase11_kernel --locked
cargo test -p softgpu-core --test phase11_aql_isa --locked
cargo run --locked -- run-kernel --builtin tiny_add --n 64
```

## Honesty limits

- Not a claim of full gfx1201 or unrestricted HIP `hipLaunchKernel` success.
- SoftGPU calling convention is SoftGPU-defined for this tiny kernel.
- `s_waitcnt` remains a SoftGPU no-op under the sequential interpreter.

## Next gate

Phase 12 — R9700 hardware conformance and profile hardening.

## References

- [`docs/isa-path.md`](../isa-path.md)
- [`docs/aql-diagnostic-contract.md`](../aql-diagnostic-contract.md)
- `crates/softgpu-amd-isa/src/kernel.rs`
- `crates/softgpu-core/tests/phase11_aql_isa.rs`
