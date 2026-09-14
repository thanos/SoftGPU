# SoftGPU support matrix

Machine-readable companion: [`support-matrix.json`](support-matrix.json).

Allowed cell states: `implemented-unverified`, `verified-unit`, `verified-integration`, `hardware-differential`, `experimental`, `unsupported`.

## Host / toolchain

| Item | State | Notes |
| --- | --- | --- |
| macOS Apple Silicon + Rust 1.85 (`cargo test --workspace --locked`) | `verified-unit` | Phase 3 fast loop |
| Linux x86-64 + Rust 1.85 (no ROCm) | `verified-unit` | CI core job |
| Linux x86-64 + pinned ROCm HIP/ROCr integration | `verified-integration` (CI) | Required `rocm-integration`; ROCm **7.14.0** |

## Runtime / ABI

| Item | State | Notes |
| --- | --- | --- |
| HSA cdylib: init/shutdown/agents + fail-closed stubs | `verified-unit` / CI | Phase 1 load proof |
| Layout probe vs vendored `hsa.h` | `verified-unit` | `tools/hsa-layout-probe` |
| HIP-linked SoftGPU load proof (anti-system-ROCr) | `verified-integration` | CI harness |
| Agent discovery (virtual GPU, `KERNEL_DISPATCH`) | `verified-unit` (+ CI) | Phase 3 FEATURE = queue ABI only |
| Advertised agent field provenance | `verified-unit` | `tests/advertised_fields.rs` |
| Path C regions + AMD pools allocate/free | `verified-unit` (+ CI probe) | SoftGPU host memory |
| Signals create/wait/store | `verified-unit` | CPU atomics |
| Queue create/destroy/indexes/doorbell observe | `verified-unit` (+ CI probe) | Observe-once; no packet execution |
| Phase 3 charter stress (wraparound, cancel, caps) | `verified-unit` | `tests/phase3_charter.rs` + `docs/concurrency-phase3.md` |
| AQL packet execution / kernels | `unsupported` | Phase 4+ |

## Profiles

| Profile | State | Notes |
| --- | --- | --- |
| `softgpu-generic` rev 0 | `verified-unit` | |
| `amd-radeon-ai-pro-r9700-gfx1201` rev 0 | `verified-unit` | identity only; limits unknown |
