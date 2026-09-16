# SoftGPU support matrix

Machine-readable companion: [`support-matrix.json`](support-matrix.json).

Allowed cell states: `implemented-unverified`, `verified-unit`, `verified-integration`, `hardware-differential`, `experimental`, `unsupported`.

## Host / toolchain

| Item | State | Notes |
| --- | --- | --- |
| macOS Apple Silicon + Rust 1.85 (`cargo test --workspace --locked`) | `verified-unit` | Phase 10 fast loop |
| Linux x86-64 + Rust 1.85 (no ROCm) | `verified-unit` | CI core job |
| Linux x86-64 + pinned ROCm HIP/ROCr integration | `verified-integration` (CI) | Required `rocm-integration`; ROCm **7.14.0** |

## Runtime / ABI

| Item | State | Notes |
| --- | --- | --- |
| HSA cdylib: init/shutdown/agents + fail-closed stubs | `verified-unit` / CI | Phase 1 load proof |
| Layout probe vs vendored `hsa.h` | `verified-unit` | `tools/hsa-layout-probe` |
| HIP-linked SoftGPU load proof (anti-system-ROCr) | `verified-integration` | CI harness |
| Agent discovery (virtual GPU, `KERNEL_DISPATCH`) | `verified-unit` (+ CI) | queue + AQL intercept claim |
| Path C memory / signals / queues / AQL diagnostic | `verified-unit` (+ CI) | Phases 3–4 |
| AMDGPU code-object metadata (`gfx1201` subset) | `verified-unit` (+ CI script) | `softgpu-amd-code-object` |
| SoftGPU Functional IR CPU execution | `verified-unit` | Phase 6–9; not gfx1201 ISA |
| SoftGPU waves/group/barriers/atomics | `verified-unit` | software semantics; Article 8 |
| SoftGPU memory/race/barrier sanitizer | `verified-unit` | declared SoftGPU HB subset; Article 9 |
| SoftGPU debugger / traces / explore | `verified-unit` | softgpu-debug-trace-v1; Article 10 |
| SoftGPU gfx1201 SALU subset (`softgpu-gfx1201-salu-v1`) | `verified-unit` | Phase 10; llvm-mc goldens; Article 11 |
| SoftGPU gfx1201 e2e tiny (`tiny_add`) | `verified-unit` | Phase 11; AQL softgpu_kernel_success; Article 12 |
| Full gfx1201 ISA / unrestricted HIP launch | `unsupported` | Phase 12+ / not claimed |

## Profiles

| Profile | State | Notes |
| --- | --- | --- |
| `softgpu-generic` rev 0 | `verified-unit` | |
| `amd-radeon-ai-pro-r9700-gfx1201` rev 0 | `verified-unit` | identity only; limits unknown |
