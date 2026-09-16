# SoftGPU HIP/ROCr path ↔ real gfx1201 card comparative study

**Access date:** 2026-09-16  
**SoftGPU revision:** v0.8.0 / `ACTIVE_PHASE=phase-hip-load`  
**SoftGPU boundary:** ROCr/HSA `cdylib` substitution under **real** HIP (ADR-0001)  
**Hardware reference:** AMD Radeon AI PRO R9700 / LLVM target **gfx1201** + pinned ROCm **7.14.0** stack  

This document compares what SoftGPU implements and verifies on the HIP→ROCr
path with what a developer typically gets from a **full gfx1201 card** under
official ROCm/HIP. It is a gap / comparative map, not a claim SoftGPU will
match the full stack.

Companion: [`docs/ISA-gap-analysis.md`](ISA-gap-analysis.md) (instruction surface).

## 1. How to read this comparison

| Side | Meaning |
| --- | --- |
| **SoftGPU** | SoftGPU `libhsa-runtime64` + SoftGPU core + optional SoftGPU ISA, loaded *instead of* system ROCr for a process |
| **Real gfx1201** | Physical R9700 (or equivalent gfx1201) + AMD ROCr + AMDGPU driver + full ROCm HIP userspace |

SoftGPU **does not replace HIP**. Applications still use the real HIP runtime;
SoftGPU substitutes the **HSA/ROCr** library HIP talks to (ADR-0001). Gaps
therefore appear as: missing HSA APIs, SoftGPU-only semantics, or HIP features
that never reach a working SoftGPU backend.

## 2. One-line verdict

SoftGPU is a **verified ROCr/HSA protocol sandbox** (plus a SoftGPU-owned tiny
gfx1201 ISA path). A real gfx1201 card is a **full device + AMDHSA + ROCm
library ecosystem**. Do not equate SoftGPU `FEATURE=KERNEL_DISPATCH`, AQL
completion-signal `0`, or `softgpu_kernel_success` with unrestricted
`hipLaunchKernel` success on hardware.

## 3. SoftGPU inventory (what we have)

### 3.1 Integration model

| Item | SoftGPU |
| --- | --- |
| Primary seam | ROCr/HSA substitution (`softgpu-hsa` cdylib, `ROCR_1`) |
| HIP SDK | Real AMD HIP (not mocked) |
| CI proof | Linux x86-64 + ROCm 7.14.0 image; SoftGPU mapped, system HSA not mapped |
| Profile | Optional R9700/gfx1201 **identity** string; limits largely `unknown`; `conformance_allowed: false` |

### 3.2 HSA/ROCr APIs with SoftGPU implementations

Allowlisted exports (bodies in `crates/softgpu-hsa`, logic in `softgpu-core`):

| Category | SoftGPU coverage |
| --- | --- |
| **Init / system** | `hsa_init` / `hsa_shut_down`; limited `hsa_system_get_info`; `hsa_status_string` |
| **Agents** | Iterate virtual CPU+GPU; limited `hsa_agent_get_info` (name, vendor, FEATURE, device, versions, queue attrs) |
| **Memory (Path C)** | Legacy regions + AMD pools; allocate/free/copy on **SoftGPU host allocator** (not VRAM) |
| **Signals** | Create/destroy; load/store; wait (software) |
| **Queues** | Create/destroy; read/write index load/store; doorbell observe |
| **AQL** | Intercept/validate kernel-dispatch (+ minimal barriers); complete or reject |
| **Code objects (HSA)** | SoftGPU-supported gfx1201 agent images (metadata + `.text`) |
| **Executables (HSA)** | SoftGPU subset: create / load_agent / freeze / symbol |
| **ISA query (`hsa_isa_*`)** | **Stubs** |

Offline SoftGPU tools (not HSA executable APIs):

- `softgpu inspect-code-object` — ELF metadata (`gfx1201` gate), no ISA run  
- `softgpu run-kernel` / AQL + `Runtime::register_isa_kernel` — SoftGPU ISA sandbox  

### 3.3 AQL / kernel honesty contracts

| Contract | SoftGPU meaning |
| --- | --- |
| `FEATURE=KERNEL_DISPATCH` | Agent supports **queues + AQL intercept**, not “kernels always run” |
| `diagnostic_complete_no_execution` | Packet validated; signal→0; ring advanced; **no kernel semantics** (`not_kernel_success`) |
| `softgpu_kernel_success` | SoftGPU-**registered** ISA image + SoftGPU kernarg → ISA engine ran (`tiny_add` class) |
| Unregistered dispatch | Still diagnose-complete (no execution) |

### 3.4 Stub / fail-closed surface (scale)

From SoftGPU stub generation (`tools/generate-hsa-stubs.pl`):

| File | Approx. count | Behavior |
| --- | --- | --- |
| `generated_stubs.c` | ~78 core HSA APIs | `HSA_STATUS_ERROR` / no-op / zero |
| `generated_amd_stubs.c` | ~57 AMD extension APIs | Same |

Notable stub clusters: executable/code-object load, ISA iterate, signal atomics,
async copy/SDMA/migrate/IPC/SVM, AMD profiling, CU masks, images, soft queues,
many `get_info` extensions.

### 3.5 What ROCm CI actually gates

`environments/rocm-x86_64/run-phase1-load-proof.sh` (through Phase 5):

| Verified | Not verified by that gate |
| --- | --- |
| SoftGPU loads under HIP-linked process | Unrestricted HIP device success as a product claim |
| SoftGPU ahead of `/opt/rocm` HSA | Full ROCr parity |
| Agent discovery + `FEATURE=KERNEL_DISPATCH` | Packet → real kernel for arbitrary apps |
| Path C memory + queue observe | Device VRAM / SDMA |
| Golden AQL → diagnostic complete | `softgpu_kernel_success` (Phase 11 is host unit tests) |
| Synthetic code-object metadata inspect | HSA executable load of hipcc HSACO |

Phase 11 ISA e2e (`phase11_aql_isa`, `run-kernel`) is **host SoftGPU evidence**,
not “HIP app on R9700.”

## 4. Real gfx1201 + ROCm offering (what a card gives you)

Typical developer-facing stack on a real gfx1201 system:

| Layer | What you get |
| --- | --- |
| **Device** | Physical GPU: VRAM, CUs, clocks, topology, firmware, driver |
| **HIP** | Full HIP runtime: devices, streams, events, modules, `hipLaunchKernel`, graphs (as supported by ROCm version) |
| **ROCr/HSA** | Full agent/ISA/memory/queue/executable surface HIP relies on |
| **AMDHSA** | Real code objects, kernel descriptors, user SGPRs, ABI matching hipcc |
| **ISA** | Full gfx1201 compute (and graphics where used) |
| **Libraries** | rocBLAS, MIOpen, hipFFT, RCCL, … |
| **Tools** | rocprof, ROCm debugger, sanitizers as shipped by AMD |
| **Multi-GPU** | Peer access, multi-agent discovery (hardware-dependent) |
| **Performance** | Real bandwidth/latency/throughput (measurable) |

SoftGPU intentionally excludes kernel-mode/PCIe/firmware emulation (charter).

## 5. Side-by-side gap matrix

### 5.1 Application journey

| Step | Real gfx1201 | SoftGPU today | Gap |
| --- | --- | --- | --- |
| Build with `hipcc --offload-arch=gfx1201` | yes | yes (compiler is real) | SoftGPU doesn’t need to own hipcc |
| Process loads HSA | AMD ROCr | SoftGPU ROCr subset | Symbol/version subset only |
| `hipInit` / device count | Real devices | HIP may call SoftGPU; SoftGPU does **not** claim `hipGetDeviceCount > 0` as a product gate | Large for “HIP app just works” |
| Discover GPU agent | Real attrs/limits | Virtual agent; few attrs; profile limits unknown | Large |
| Allocate device memory | VRAM/GTT semantics | SoftGPU **host** allocator behind Path C names | Large (semantics) |
| Create stream / event | HIP streams on real queues | No SoftGPU HIP stream model; HSA signals are software | Large |
| Load module / HSACO | Executable APIs work | SoftGPU-supported agent `.text` + SoftGPU CC | Medium (arbitrary hipcc still large) |
| Launch kernel | Full AMDHSA + ISA | SoftGPU-registered/loaded tiny ISA **or** diagnose-complete | Medium–Large |
| Check results | Device memory | SoftGPU host memory | SoftGPU-only |
| Use rocBLAS / MIOpen | yes | no (needs real dispatch + memory) | Large |
| Profile / debug HW | rocprof / ROCm tools | SoftGPU SFIR debugger; AMD profiling stubs | Large |
| Claim conformance | HW matrix | `conformance_allowed: false`; Phase 12 planned | Large |

### 5.2 Runtime / ABI categories

| Category | Real gfx1201 | SoftGPU | Gap severity |
| --- | --- | --- | --- |
| Init / substitution | Full ROCr | Proven load subset | Medium (proven path is narrow) |
| Agent / device info | Rich ISA/wave/cache/limits | Minimal + stubs for ISA iterate | Large |
| Memory kinds | Device, host, SVM, migrate, IPC, async copy | Path C host views only | Large |
| Signals / sync | HW-backed + rich atomics | Software signals; atomics stubbed | Medium–Large |
| Queues / AQL | Full packet processor + execute | Observe + SoftGPU execute for registered only | Large |
| Code object / executable | Full | Stubs (+ SoftGPU offline metadata) | **Critical** for HIP modules |
| Kernel ISA | Full gfx1201 | ~11 forms, SoftGPU CC | Critical (see ISA gap doc) |
| AMD extensions | Broad | Mostly stubs | Large |
| Multi-GPU | Hardware topology | Single virtual GPU story | Large |
| Performance / PMCs | Real | Explicitly non-authoritative | Permanent SoftGPU stance until analytical subsystem |

### 5.3 “HIP feature” vs SoftGPU seam (important subtlety)

Many HIP APIs are implemented **inside AMD’s HIP runtime**, which then calls
ROCr. SoftGPU can break HIP features in three ways:

1. **HIP never calls SoftGPU** for that feature (SoftGPU irrelevant).  
2. **HIP calls a SoftGPU stub** → `HSA_STATUS_ERROR` → HIP fails.  
3. **HIP calls SoftGPU successfully** but SoftGPU semantics ≠ hardware (e.g.
   Path C “device” memory is host RAM; diagnostic complete ≠ kernel ran).

A full gfx1201 card clears (2) and (3) for the supported ROCm matrix. SoftGPU
today clears only a **declared** slice of (2)/(3).

## 6. Quantitative sketch (order of magnitude)

| Metric | SoftGPU | Full gfx1201 ROCm offering |
| --- | --- | --- |
| HSA exports with SoftGPU bodies | Tens (allowlist) | Full ROCr surface HIP needs |
| Auto-stubs (core + AMD) | ~135 | ~0 for supported apps |
| Kernels SoftGPU can execute via AQL | SoftGPU-registered tiny set (`tiny_add`) | Compiler output for gfx1201 |
| ROCm CI kernel execution gate | No | N/A (hardware CI is SoftGPU Phase 12+) |
| Device memory | SoftGPU host pool (256 MiB SoftGPU default) | Card VRAM + driver pools |
| Math libraries | 0 | Full ROCm library set |

Again: not a completion percentage — SoftGPU ships **named verified subsets**.

## 7. Suggested SoftGPU expansion ladder (HIP path)

Aligned with “better HIP load” and ADR-0001:

1. **Executable subset** — implement `hsa_code_object_reader_*` / `hsa_executable_*` for SoftGPU-supported gfx1201 artifacts only; fail-closed elsewhere  
2. **AMDHSA launch state** — parse `.kd`, preload user SGPRs matching a pinned hipcc ABI  
3. **Grow ISA** to match chosen official-compiler kernels (see ISA gap analysis)  
4. **HIP e2e CI** — tiny HIP app: alloc → module load → launch → result check under SoftGPU (still declared subset)  
5. **Richer agent info** — only advertise attrs SoftGPU can back  
6. **Memory honesty** — keep Path C labels; optionally document SoftGPU “device” = host; never invent VRAM bandwidth  
7. **Hardware differential** — Phase 12 for claims that touch real card behavior  

## 8. Explicit SoftGPU non-goals (near term)

- Replacing the HIP SDK or mocking the entire HIP API  
- Kernel-mode AMDGPU / PCIe / firmware device model  
- Shipping rocBLAS/MIOpen “on SoftGPU” without real dispatch evidence  
- Advertising R9700 performance numbers from SoftGPU wall-clock  
- Silent success when executable/ISA APIs are unimplemented  

## 9. Summary table (executive)

| Developer expectation | Real gfx1201 | SoftGPU v0.6 |
| --- | --- | --- |
| “HIP finds my GPU” | Yes | SoftGPU virtual agent; HIP device-count not a SoftGPU success claim |
| “I can malloc device memory” | VRAM semantics | SoftGPU host memory under HSA region/pool APIs |
| “I can load my hipcc module” | Yes | SoftGPU fixture / SoftGPU-supported agent image only |
| “I can launch a kernel” | Yes | SoftGPU-registered/loaded compute-v2 **or** diagnose-complete |
| “Results match the GPU” | Hardware truth | SoftGPU architectural/functional subsets only |
| “I can use rocBLAS” | Yes | No |
| “I can profile CUs” | Yes | No (AMD profiling stubs) |

## References

- [`docs/adr/0001-rocr-hsa-substitution-boundary.md`](adr/0001-rocr-hsa-substitution-boundary.md)  
- [`docs/status.md`](status.md)  
- [`docs/support-matrix.md`](support-matrix.md) / [`docs/support-matrix.json`](support-matrix.json)  
- [`docs/aql-diagnostic-contract.md`](aql-diagnostic-contract.md)  
- [`docs/what-feature-means.md`](what-feature-means.md)  
- [`docs/memory-api.md`](memory-api.md)  
- [`docs/isa-path.md`](isa-path.md) · [`docs/ISA-gap-analysis.md`](ISA-gap-analysis.md)  
- [`docs/articles/12-first-gfx1201-kernel.md`](articles/12-first-gfx1201-kernel.md)  
- [`environments/rocm-x86_64/README.md`](../environments/rocm-x86_64/README.md)  
- `crates/softgpu-hsa/` · `tools/generate-hsa-stubs.pl`
