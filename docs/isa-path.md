# SoftGPU gfx1201 ISA path (Phases 10–11)

Fidelity: **Architectural ISA** for `softgpu-gfx1201-e2e-tiny-v1` only.

## Subset

SALU (Phase 10) plus SMEM/VOP2/GLOBAL ops required by SoftGPU `tiny_add`
(`b[i]=a[i]+1`, i32). Machine code: llvm-mc `-mcpu=gfx1201` (`TINY_ADD_TEXT`).

## Calling convention (SoftGPU-defined)

- `s[4:5]` = kernarg base
- `v0` = `global_id_x`
- EXEC = active lanes

## AQL

`Runtime::register_isa_kernel` / `register_builtin_tiny_add`. When an AQL
dispatch references a registered object with SoftGPU kernarg memory, SoftGPU
runs the ISA engine and records `softgpu_kernel_success`. Otherwise the Phase 4
diagnostic no-execution contract still applies.

## CLI

```bash
softgpu decode-isa 0xbf800000
softgpu run-isa --words 0xbe800081,0xbfb00000
softgpu run-kernel --builtin tiny_add --n 64
```

See Article 12.
