# SoftGPU support matrix

Machine-readable companion: [`support-matrix.json`](support-matrix.json).

Allowed cell states: `implemented-unverified`, `verified-unit`, `verified-integration`, `hardware-differential`, `experimental`, `unsupported`.

## Host / toolchain

| Item | State | Notes |
| --- | --- | --- |
| macOS Apple Silicon + Rust 1.85 (`cargo test --workspace --locked`) | `verified-unit` | Phase 2 fast loop |
| Linux x86-64 + Rust 1.85 (no ROCm) | `verified-unit` | CI core job |
| Linux x86-64 + pinned ROCm HIP/ROCr integration | `implemented-unverified` → CI | Required `rocm-integration` job; ROCm **7.14.0** digest in `PINNED` |

## Runtime / ABI

| Item | State | Notes |
| --- | --- | --- |
| HSA cdylib: init/shutdown/agents + fail-closed stubs | `verified-unit` / CI `verified-integration` | Phase 1 load proof |
| Layout probe vs vendored `hsa.h` | `verified-unit` | `tools/hsa-layout-probe` |
| HIP-linked SoftGPU load proof (anti-system-ROCr) | `implemented-unverified` | harness ready; promote on CI green |
| Agent discovery (virtual GPU) | `verified-unit` (+ CI harness) | Phase 2; HIP-linked HSA iterate |
| Advertised agent field provenance | `verified-unit` | `tests/advertised_fields.rs` |
| Queues / signals / AQL | `unsupported` | Phase 3+ |

## Profiles

| Profile | State | Notes |
| --- | --- | --- |
| `softgpu-generic` rev 0 | `verified-unit` | |
| `amd-radeon-ai-pro-r9700-gfx1201` rev 0 | `verified-unit` | identity only; limits unknown |
