# SoftGPU gfx1201 ISA path (v0.7 / `softgpu-gfx1201-compute-v2`)

Fidelity: **Architectural ISA** for `softgpu-gfx1201-compute-v2`
(includes nested Phase 11 `softgpu-gfx1201-e2e-tiny-v1`).

## Subset

SALU (SOPP/SOPC/SOPK/SOP1/SOP2) plus VOP1/VOP2/VOPC and SMEM/GLOBAL ops
needed by SoftGPU builtins. Machine code: llvm-mc `-mcpu=gfx1201`
(`TINY_ADD_TEXT`, `CLAMP64_TEXT`, `SELECT_GT50_TEXT`).

## Calling convention (SoftGPU-defined)

- `s[4:5]` = kernarg base
- `v0` = `global_id_x`
- EXEC = active lanes

AMDHSA user-SGPR preload for arbitrary hipcc modules remains limited; SoftGPU
loaded fixtures use SoftGPU CC (`s[4:5]`). Full `.kd` binary preload is future work.

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
softgpu run-kernel --builtin clamp64 --n 64
softgpu run-kernel --builtin select_gt50 --n 64
```

See Article 12 and the SoftGPU gap maps:
[`docs/ISA-gap-analysis.md`](ISA-gap-analysis.md),
[`docs/HIP-gap-analysis.md`](HIP-gap-analysis.md).
