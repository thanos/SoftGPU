# SoftGPU support matrix

Machine-readable companion: [`support-matrix.json`](support-matrix.json).

Allowed cell states: `implemented-unverified`, `verified-unit`, `verified-integration`, `hardware-differential`, `experimental`, `unsupported`.

## Host / toolchain

| Item | State | Notes |
| --- | --- | --- |
| macOS Apple Silicon + Rust 1.85 (`cargo test --locked`) | `verified-unit` | Phase 0 primary fast loop |
| Linux x86-64 + Rust 1.85 (no ROCm) | `verified-unit` | Portability / CI |
| Linux ARM64 core tests | `experimental` | Optional via Apple container later |
| Linux x86-64 + pinned ROCm HIP/ROCr integration | `unsupported` | Phase 1+; CI job must report **skipped** until implemented |
| Nightly Rust host features | `unsupported` | Prohibited |

## Runtime / ABI

| Item | State | Notes |
| --- | --- | --- |
| HSA/ROCr cdylib exports | `unsupported` | Phase 1 |
| Agent discovery | `unsupported` | Phase 2 |
| Memory regions/pools, signals, queues | `unsupported` | Phase 3 |
| AQL dispatch interception | `unsupported` | Phase 4 |
| AMD code-object parse | `unsupported` | Phase 5 |
| Functional execution | `unsupported` | Phase 6 |
| gfx1201 ISA execution | `unsupported` | Phase 10–11 |
| R9700 hardware differential | `unsupported` | Phase 12 |

## Profiles

| Profile | State | Notes |
| --- | --- | --- |
| `softgpu-generic` rev 0 | `verified-unit` | Schema only |
| `amd-radeon-ai-pro-r9700-gfx1201` rev 0 | `verified-unit` | Identity `gfx1201` verified via ROCm docs; limits **unknown**; conformance **false** |

## SoftGPU crate features (Phase 0)

| Feature | State |
| --- | --- |
| Error taxonomy | `verified-unit` |
| Fidelity enum | `verified-unit` |
| Profile schema validation | `verified-unit` |
| CLI negative config handling | `verified-unit` |
