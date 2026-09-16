# SoftGPU ↔ gfx1201 ISA gap analysis

**Access date:** 2026-09-16  
**SoftGPU revision:** v0.8.0 / `ACTIVE_PHASE=phase-hip-load`  
**SoftGPU subset:** `softgpu-gfx1201-compute-v2` (includes nested `e2e-tiny-v1`)  
**Target claimed:** LLVM/ROCm `gfx1201` (RDNA 4 family)  
**Fidelity today:** **Architectural ISA** for the named subset only  

This document compares what SoftGPU implements and verifies today against what a
**complete** gfx1201 architectural ISA surface would require. It is an
engineering gap map, not a claim that SoftGPU will (or must) implement every
instruction.

Companion: [`docs/HIP-gap-analysis.md`](HIP-gap-analysis.md) (HIP/ROCr vs real card).

## 1. Method and sources

| Source | SoftGPU use |
| --- | --- |
| SoftGPU `SUPPORTED_FAMILIES` / `Inst` / `TINY_ADD_TEXT` | Inventory of implemented ops |
| SoftGPU Articles 11–12, `docs/isa-path.md` | Declared calling convention and honesty limits |
| [LLVM AMDGPUUsage](https://llvm.org/docs/AMDGPUUsage.html) | Public encoding/vocabulary |
| [LLVM GFX12 instruction syntax](https://llvm.org/docs/AMDGPU/AMDGPUAsmGFX12.html) | Family taxonomy for “complete” surface |
| llvm-mc / llvm-objdump goldens (checked in) | Byte-level SoftGPU evidence |

**Important:** SoftGPU does **not** treat restricted AMD ISA PDFs as an in-tree
source. “Complete gfx1201” here means: enough of the public LLVM/AMDGPU
`gfx1201`/`gfx12` instruction and state model that typical official-compiler
kernels can decode and execute with fail-closed residuals — plus AMDHSA launch
conventions. Exact opcode counts evolve with LLVM; families below are stable
enough for planning.

**Order-of-magnitude scale:** LLVM’s GFX12 asm syntax tables list on the order
of **~10³** instruction forms (mnemonics × variants). SoftGPU currently
implements on the order of **~50** named forms in `compute-v2`. Coverage is
intentionally still a narrow verified subset.

## 2. SoftGPU baseline (what we have)

### 2.1 Implemented instruction forms

From `crates/softgpu-amd-isa` (`SUPPORTED_FAMILIES`):

| Family (SoftGPU label) | Mnemonics | Notes |
| --- | --- | --- |
| SOPP | `s_nop`, `s_endpgm`, `s_sleep`, `s_waitcnt` | `sleep` / `waitcnt` = SoftGPU **no-ops** |
| SOP1 | `s_mov_b32` | Inline constants `0..=64`, `-1`; SGPR `0..=105` |
| SOP2 | `s_add_co_u32` | Carry → SoftGPU `scc` |
| SMEM | `s_load_b64` | SoftGPU field map from llvm-mc differentials |
| VOP2 | `v_lshlrev_b32_e32`, `v_add_nc_u32_e32` | Imm src0 only in tiny path |
| GLOBAL | `global_load_b32`, `global_store_b32` | SoftGPU arena / allocator VAs |

Everything else **traps** before silently corrupting state.

### 2.2 SoftGPU machine state (present vs used)

| State | Present | Used by tiny subset |
| --- | --- | --- |
| PC | yes | yes |
| SGPR file (106) | yes | yes |
| Per-lane VGPR (256) | yes | yes (`v0`–`v2`) |
| SCC | yes | `s_add_co_u32` only |
| EXEC | yes | wave launch mask |
| VCC | yes | **unused** by subset |
| Mode / STATUS / trap / MODE / HW regs | no | — |
| LDS / GDS / scratch | no | — |
| FP / special regs | no | — |
| Memory scopes / caches | no | sequential SoftGPU mem |

`wave_size` is a **SoftGPU software parameter** (32|64), not R9700 wavefront
evidence.

### 2.3 SoftGPU calling convention (not full AMDHSA)

For `tiny_add` SoftGPU **defines**:

- `s[4:5]` = kernarg base  
- `v0` = `global_id_x`  
- EXEC = active lanes  

This is **not** a claim of AMD user-SGPR / kernel-descriptor / `s_getpc` /
compiler ABI compatibility for arbitrary hipcc kernels.

### 2.4 Dispatch / tooling surface

| Capability | SoftGPU today |
| --- | --- |
| Offline decode / SALU step | `decode-isa`, `run-isa` |
| Offline tiny kernel | `run-kernel --builtin tiny_add` |
| AQL registered ISA image | `softgpu_kernel_success` |
| Unregistered AQL | `diagnostic_complete_no_execution` |
| HSA executable load of real HSACO | **stubs / unsupported** |
| hipcc fat binary → SoftGPU ISA | **not claimed** |
| Hardware differential | Phase 12+ |

## 3. “Complete gfx1201” — target surface (families)

Using LLVM GFX12 asm documentation family sections as the planning taxonomy
(names are LLVM’s; SoftGPU maps them to gfx1201 via `-mcpu=gfx1201`):

| Family | Role (short) | SoftGPU status |
| --- | --- | --- |
| **SMEM** | Scalar memory (kernarg, const, scalar loads/stores) | **Partial** (`s_load_b64` only) |
| **SOP1** | Scalar unary | **Partial** (`s_mov_b32`) |
| **SOP2** | Scalar binary | **Partial** (`s_add_co_u32`) |
| **SOPC** | Scalar compare → SCC | **Missing** |
| **SOPK** | Scalar with 16-bit imm | **Missing** |
| **SOPP** | Program control / wait / barriers-ish | **Partial** (4 ops; waits are no-ops) |
| **VOP1** | Vector unary | **Missing** |
| **VOP2** | Vector binary (e32) | **Partial** (2 ops) |
| **VOP3 / VOP3P** | Vector 3-operand / packed | **Missing** |
| **VOPC** | Vector compare → VCC/EXEC | **Missing** |
| **VOPDX / VOPDY** | Dual-issue / dependent forms | **Missing** |
| **VGLOBAL** | Global memory | **Partial** (b32 load/store only) |
| **VFLAT** | Flat addressing | **Missing** |
| **VBUFFER** | Buffer (resource) memory | **Missing** |
| **VSCRATCH** | Scratch / private | **Missing** |
| **VDS / VDSDIR** | LDS / shared data | **Missing** |
| **VIMAGE / VSAMPLE** | Image / sample | **Missing** (graphics/compute media) |
| **VINTERP / VEXPORT** | Graphics export / interp | **Missing** (out of SoftGPU compute scope) |

SoftGPU may forever treat image/export/interp as **unsupported** for a
developer-oriented compute runtime; “complete gfx1201 **compute**” is the
practical bar for SoftGPU, not a full graphics ISA.

## 4. Gap matrix by concern

### 4.1 Decode / encode completeness

| Concern | SoftGPU | Complete gfx1201 (compute) | Gap |
| --- | --- | --- | --- |
| Named opcode tables | Hand-maintained ~11 forms | Full family tables (SOP*/VOP*/SMEM/VMEM/…) | Vast |
| Multi-word fetch | 4/8/12 for known forms | All legal lengths / literals | Large |
| Literal 32-bit immediates | Not general | Common in VOP/SOP | Large |
| Modifiers (abs/neg/omod/clamp, SDWA/DPP if any) | None | Widespread on VALU | Large |
| Assembler round-trip | llvm-mc goldens for subset | Full llvm-mc / MC tests | Large |
| Disasm fidelity | Matches SoftGPU goldens | Match llvm-objdump broadly | Large |

### 4.2 Execution semantics

| Concern | SoftGPU | Complete | Gap |
| --- | --- | --- | --- |
| Integer ALU | Tiny | Full i32/i64/bitwise/shift/mul/div | Large |
| FP16/32/64, packed math | None | Large VALU surface | Large |
| Branches / EXEC divergence | None | `s_cbranch_*`, `v_cmp`+mask, reconverge | Large |
| Barriers / waves | SoftGPU Functional IR only (SFIR); ISA barrier ops absent | ISA + HSA wave sync | Large |
| Atomics | SFIR only | GLOBAL/FLAT/DS atomics | Large |
| SoftGPU `s_waitcnt` | No-op | Real counters / GFX12 wait splits (`s_wait_loadcnt`, …) | Large |
| Memory consistency | Sequential SoftGPU arena | GFX12 AMDHSA memory model scopes | Large |

### 4.3 Architectural state

| Concern | SoftGPU | Complete | Gap |
| --- | --- | --- | --- |
| SGPR / VGPR files | Fixed SoftGPU sizes | Real limits + allocation from `.kd` / metadata | Medium |
| EXEC / VCC / SCC | Partial | Full update rules | Medium–Large |
| Special SGPRs (M0, NULL, …) | Unsupported operands trap | Required | Large |
| TTMP / ABI temp regs | Not modeled as AMD ABI | Compiler uses them | Large |
| Trap / exception model | SoftGPU IsaError traps | HW/trap handlers | Large (may stay SoftGPU-defined) |

### 4.4 Memory spaces

| Space | SoftGPU | Complete compute | Gap |
| --- | --- | --- | --- |
| Global (SoftGPU host/alloc VA) | yes (tiny path) | yes + scopes | Medium |
| Flat | no | yes | Large |
| Buffer / SRD | no | yes | Large |
| LDS (group) | SFIR only | DS ops | Large |
| Scratch / private | no | yes | Large |
| Constant / kernarg via SMEM | `s_load_b64` only | Full SMEM suite | Large |
| Image | no | optional for SoftGPU | Likely permanent unsupported |

### 4.5 Launch / ABI / HIP path

| Concern | SoftGPU | Complete path | Gap |
| --- | --- | --- | --- |
| SoftGPU-defined CC for tiny_add | yes | — | SoftGPU-only |
| AMDHSA kernel descriptor (`.kd`) | metadata inspect only | Full parse + preload | Large |
| User SGPR setup from AQL/dispatch | SoftGPU registers only | Real packet → SGPRs | Large |
| Workitem/workgroup ID sources | SoftGPU sets `v0` | Compiler ABI (often SGPRs / special) | Large |
| Code-object `.text` extract | SoftGPU-owned llvm-mc blob | Official hipcc HSACO / fat bin | Large |
| HSA executable APIs | stubs | Implemented subset | Large |
| Unrestricted `hipLaunchKernel` | unsupported | Declared SoftGPU subset | Large |

### 4.6 Verification ladder

| Level | SoftGPU today | Complete target |
| --- | --- | --- |
| Unit goldens (llvm-mc) | subset | per family |
| Host differential | `tiny_add` vs host/SFIR | many microkernels |
| Official-compiler kernels | not claimed | yes (declared list) |
| Hardware differential (R9700) | Phase 12 planned | required for conformance claims |

## 5. Quantitative sketch (honest, approximate)

| Metric | SoftGPU now | “Complete compute gfx1201” (planning) |
| --- | --- | --- |
| Named instruction forms | **11** | **O(10²)–O(10³)** (LLVM tables; depends on counting aliases/variants) |
| Instruction families with any support | **6** labels | **~15+** compute-relevant families |
| Memory spaces | 1 (SoftGPU global) | 5+ (global/flat/buffer/lds/scratch) |
| Launch ABIs | SoftGPU-only | SoftGPU + AMDHSA |
| HIP end-to-end | load/discovery + SoftGPU register path | load → execute declared kernels |

Do not treat the ratio “11 / 1000” as a project completion percentage. SoftGPU’s
charter is **narrow verified subsets**, not percentage of the ISA manual.

## 6. Suggested expansion ladder (not a commitment)

Ordered so each step stays fail-closed and testable:

1. **Broader SALU** — SOPC/SOPK, more SOP1/SOP2, real branch basics  
2. **Broader VALU** — VOP1/more VOP2, VOPC → VCC/EXEC  
3. **Memory realism** — waitcnt semantics (SoftGPU model), flat, LDS/DS, atomics  
4. **AMDHSA launch** — `.kd` + user SGPR preload matching a pinned hipcc ABI  
5. **Official-compiler kernels** — extract `.text` from gfx1201 HSACO; grow ops to match  
6. **HIP executable path** — replace stubs for load/freeze/symbol for SoftGPU-supported artifacts  
7. **Hardware differential** — Phase 12 R9700 evidence for each claimed family  

Optional product tags (0.7/0.8) could sit between v0.6 and v0.9 if SoftGPU
ships intermediate “broader ISA” or “better HIP load” milestones; the charter’s
next **named** release after 0.6 remains **v0.9.0** (hardware RC).

## 7. Explicit SoftGPU non-goals (likely permanent or deferred)

- Cycle-accurate R9700 timing / performance oracle  
- Full graphics pipeline (export, interpolate, image sampling) as a SoftGPU priority  
- Silent success on unknown opcodes  
- Claiming “gfx1201 compatible” without attaching subset + evidence  
- Using restricted AMD ISA PDFs as in-tree tables  

## 8. Summary

SoftGPU v0.6.0 proves a **vertical ISA slice**: decode → SoftGPU state → memory →
AQL success for one llvm-mc `tiny_add` under a SoftGPU calling convention.
Against a **complete gfx1201 compute ISA**, the gap is **almost all families,
almost all opcodes, memory model, and AMDHSA/HIP launch realism**. Closing that
gap is multi-phase work; SoftGPU should continue to expand **named, sourced
subsets** rather than chase completeness as a single milestone.

## References

- [`docs/isa-path.md`](isa-path.md)  
- [`docs/articles/11-decoding-amdgpu-isa.md`](articles/11-decoding-amdgpu-isa.md)  
- [`docs/articles/12-first-gfx1201-kernel.md`](articles/12-first-gfx1201-kernel.md)  
- [`docs/status.md`](status.md)  
- `crates/softgpu-amd-isa/src/provenance.rs`  
- LLVM: [AMDGPUUsage](https://llvm.org/docs/AMDGPUUsage.html), [GFX12 asm syntax](https://llvm.org/docs/AMDGPU/AMDGPUAsmGFX12.html)
