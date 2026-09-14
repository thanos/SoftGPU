# Article 3 — Impersonating a GPU without lying

- **Audience:** runtime/ABI engineers
- **Prerequisites:** Articles 1–2
- **Evidence:** SoftGPU Phase 2–3 agent discovery and queue ABI
- **Access date:** 2026-09-14

## Problem

Discovery APIs tempt emulators to invent CU counts, wave sizes, and queue limits. SoftGPU treats identity as a **contract with provenance**.

## Design

```text
DeviceProfile (JSON, provenance fields)
        |
        v
VirtualAgent (name, vendor, device=GPU, FEATURE=KERNEL_DISPATCH)
        |
        v
PackedHandle [kind|generation|index]
        |
        v
hsa_iterate_agents / hsa_agent_get_info
        |
        +--> regions/pools (Path C) --> SoftGPU host allocator
        +--> signals / queues (observe only; no packet execution)
```

### Advertised agent fields (Phase 3)

| Attribute | Value source | Provenance |
| --- | --- | --- |
| `NAME` | profile `product_name` | profile identity |
| `VENDOR_NAME` | profile `vendor` | profile identity |
| `DEVICE` | GPU | SoftGPU virtual agent policy |
| `FEATURE` | `KERNEL_DISPATCH` | SoftGPU Phase 4 **queue + AQL intercept** — not kernel execution |
| `QUEUE_*` | SoftGPU software defaults | SoftGPU software limits |
| `VERSION_MAJOR/MINOR` | `1` / `2` | HSA Runtime 1.2 family target |

Unsupported attributes still return `HSA_STATUS_ERROR_INVALID_ARGUMENT`.

### Controlled HIP subset

A HIP-linked probe loads SoftGPU, discovers the agent via **HSA** APIs, and may create SoftGPU queues. SoftGPU does **not** promise packet execution or guaranteed `hipGetDeviceCount > 0`.

## Reproducible example

```bash
cargo test -p softgpu-hsa --locked
cargo test -p softgpu-core --locked runtime::
# Linux x86_64 + ROCm (CI):
#   bash environments/rocm-x86_64/ci-entrypoint.sh
```

Traces record profile + fidelity on init, plus memory/queue/doorbell events.

## What we verified / what remains assumed

| Verified | Assumed / CI |
| --- | --- |
| Stale/forged handles fail; unsupported attrs fail closed | Live ROCm CI green for discovery + Phase 3 probe |
| FEATURE/queue honesty locked by unit + probes | HIP may need more attrs before full device use |
| Path C allocate/free + queue observe | No R9700 VRAM sizes invented |

## Failure cases

- Wavefront size query → invalid argument (not a guessed 32/64)
- Handle after shutdown/re-init → invalid agent
- Packet execution / code objects → still unsupported (Phase 4)

## References

- ADR-0002, `docs/memory-api.md`, `docs/what-feature-means.md`
- `profiles/amd-radeon-ai-pro-r9700-gfx1201-v0.json`
