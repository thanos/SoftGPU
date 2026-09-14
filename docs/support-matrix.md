# SoftGPU support matrix

Machine-readable companion: [`support-matrix.json`](support-matrix.json).

Allowed cell states: `implemented-unverified`, `verified-unit`, `verified-integration`, `hardware-differential`, `experimental`, `unsupported`.

## Host / toolchain

| Item | State | Notes |
| --- | --- | --- |
| macOS Apple Silicon + Rust 1.85 (`cargo test --workspace --locked`) | `verified-unit` | Phase 5 fast loop |
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
| AMDGPU code-object metadata (`gfx1201` subset) | `verified-unit` (+ CI script) | `softgpu-amd-code-object`; see `docs/code-object.md` |
| Kernel execution / gfx1201 ISA | `unsupported` | Phase 6+ |

## Profiles

| Profile | State | Notes |
| --- | --- | --- |
| `softgpu-generic` rev 0 | `verified-unit` | |
| `amd-radeon-ai-pro-r9700-gfx1201` rev 0 | `verified-unit` | identity only; limits unknown |
