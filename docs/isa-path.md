# SoftGPU gfx1201 ISA path (Phase 10)

Fidelity: **Architectural ISA** for the named SoftGPU subset
`softgpu-gfx1201-salu-v1` only. This is **not** HIP/HSA AQL kernel success
(Phase 11) and **not** a claim of full gfx1201 coverage.

## Crate

`softgpu-amd-isa` provides:

- LE instruction-word fetch
- `gfx1201` architecture gate
- Decoder for SOPP / SOP1 / SOP2 instructions in the subset
- Explicit `MachineState` (PC, SGPR, VGPR storage, SCC, EXEC, VCC, lanes)
- Step/run with fail-closed traps
- Disassembler aligned to llvm-mc golden asm strings

## Provenance

Encoding bytes are recorded in
`crates/softgpu-amd-isa/goldens/llvm-mc-gfx1201.json` (tool: llvm-mc, mcpu
gfx1201). See Article 11 and `docs/sources.md`.

## CLI

```bash
softgpu decode-isa 0xbf800000 0xbe800081
softgpu run-isa --words 0xbe800081,0xbe810082,0x80020100,0xbfb00000
```
