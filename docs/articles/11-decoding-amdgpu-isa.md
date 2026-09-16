# Article 11 — Decoding an AMD GPU ISA responsibly

- **Audience:** SoftGPU contributors building architectural ISA support
- **Prerequisites:** Articles 1–10, especially Article 6 (code objects) and 7 (emulation vs simulation)
- **Evidence:** SoftGPU Phase 10 `softgpu-amd-isa` + `phase10_isa` goldens from `llvm-mc`
- **Access date:** 2026-09-16

## Problem

ISA tables are easy to invent and hard to defend. SoftGPU must never claim
“gfx1201 compatible” from blog posts, guessed bitfields, or restricted manuals
copied into the tree. Encoding facts need provenance, narrow subsets, and
fail-closed traps.

## SoftGPU Phase 10 approach

| Piece | SoftGPU meaning |
| --- | --- |
| **Target** | `gfx1201` only (`Arch::Gfx1201`) |
| **Subset** | `softgpu-gfx1201-salu-v1`: `s_nop`, `s_endpgm`, `s_sleep`, `s_waitcnt`, `s_mov_b32`, `s_add_co_u32` |
| **Goldens** | LE words observed with `llvm-mc -arch=amdgcn -mcpu=gfx1201 -show-encoding` |
| **Docs cross-check** | Public [LLVM AMDGPUUsage](https://llvm.org/docs/AMDGPUUsage.html) (layout vocabulary; goldens win for bytes) |
| **Fidelity** | **Architectural ISA** for the named subset only |
| **Trap** | Unknown encoding/opcode/operand → error before SGPR/SCC corruption |

```text
LE instruction words
        |
        v
fetch_word → decode_word (SOPP / SOP1 / SOP2 keys)
        |
        +--> Inst (named subset) --> step/run on MachineState
        +--> TrapKind (unsupported) --> IsaError (fail closed)
        v
disasm_word  (matches golden asm strings)
```

## Machine state (explicit)

SoftGPU models PC, SGPR file, per-lane VGPR storage (unused in Phase 10),
SCC, EXEC, VCC, halt, and SoftGPU `wave_size` (32|64 software parameter—not
R9700 wavefront evidence). SALU ops in this subset update SGPRs/SCC only.

Honest SoftGPU treatments:

- `s_waitcnt` / `s_sleep` decode and execute as **no-ops** under SoftGPU’s
  sequential interpreter (no HW memory pipeline / sleep).
- Immediate scalar operands: SGPR `0..=105`, inline `0..=64` and `-1` only.

## Regenerating goldens

```bash
# Optional: requires LLVM AMDGPU llvm-mc
./tools/regen-isa-goldens.sh
cargo test -p softgpu-amd-isa --test phase10_isa --locked
```

Checked-in JSON under `crates/softgpu-amd-isa/goldens/` keeps CI independent of
a local LLVM install. Tables are hand-maintained with provenance comments—not
copied from restricted AMD ISA PDFs.

## What we verified / what remains assumed

| Verified | Assumed / out of scope |
| --- | --- |
| Byte goldens for the SALU subset vs llvm-mc | Full gfx1201 ISA / VALU / memory ops |
| Trap-before-corrupt on unknown words | Cycle accuracy / scoreboard timing |
| `s_add_co_u32` carry → SoftGPU SCC | Hardware SCC side effects beyond u32 carry |
| Disasm strings match golden asm | llvm-objdump pretty-print variants for waitcnt |

## Commands

```bash
cargo test -p softgpu-amd-isa --locked
cargo run --locked -- decode-isa 0xbe800081 0xbfb00000
cargo run --locked -- run-isa --words 0xbe800081,0xbe810082,0x80020100,0xbfb00000
```

## Next gate

Phase 11 — first end-to-end SoftGPU gfx1201 tiny kernel (done; see Article 12).

## References

- [`crates/softgpu-amd-isa`](../../crates/softgpu-amd-isa)
- [`docs/sources.md`](../sources.md)
- LLVM AMDGPUUsage: https://llvm.org/docs/AMDGPUUsage.html
- Article 12: [`12-first-gfx1201-kernel.md`](12-first-gfx1201-kernel.md)