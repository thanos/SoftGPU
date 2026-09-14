# Article 3 — Impersonating a GPU without lying

- **Audience:** runtime/ABI engineers
- **Prerequisites:** Articles 1–2
- **Evidence:** SoftGPU Phase 2 agent discovery
- **Access date:** 2026-09-13

## Problem

Discovery APIs tempt emulators to invent CU counts, wave sizes, and queue limits. SoftGPU treats identity as a **contract with provenance**.

## Design

```text
DeviceProfile (JSON, provenance fields)
        |
        v
VirtualAgent (name, vendor, device=GPU, feature=0)
        |
        v
PackedHandle [kind|generation|index]
        |
        v
hsa_iterate_agents / hsa_agent_get_info
```

### Advertised agent fields (Phase 2)

| Attribute | Value source | Provenance |
| --- | --- | --- |
| `NAME` | profile `product_name` | profile identity |
| `VENDOR_NAME` | profile `vendor` | profile identity |
| `DEVICE` | GPU | SoftGPU virtual agent policy |
| `FEATURE` | `0` | explicit — no dispatch claim |
| `VERSION_MAJOR/MINOR` | `1` / `2` | HSA Runtime 1.2 family target |

Everything else returns `HSA_STATUS_ERROR_INVALID_ARGUMENT`.

### Controlled HIP subset

A HIP-linked probe loads SoftGPU (Phase 1), then discovers the agent via **HSA** APIs. Because `FEATURE=0`, SoftGPU does not promise `hipGetDeviceCount > 0`.

## Reproducible example

```bash
cargo test -p softgpu-hsa --locked
cargo test -p softgpu-core --locked runtime::
# Linux x86_64 + ROCm (CI):
#   bash environments/rocm-x86_64/ci-entrypoint.sh
```

Traces record `profile_id`, revision, and fidelity (`abi`) on init.

## What we verified / what remains assumed

| Verified | Assumed / CI |
| --- | --- |
| Stale/forged handles fail; unsupported attrs fail closed | Live ROCm CI green for discovery probe |
| Every advertised field locked by `advertised_fields` tests | hip device enumeration under FEATURE=0 stays zero by design |
| Trace contains profile + fidelity | |

## Failure cases

- Wavefront size query → invalid argument (not a guessed 32/64)
- Handle after shutdown/re-init → invalid agent
- Queues → unsupported / stubbed fail-closed

## References

- ADR-0002, `profiles/amd-radeon-ai-pro-r9700-gfx1201-v0.json`
- Pinned `hsa.h` agent info enumerators
- `tools/softgpu-hip-discovery-probe/main.cpp`
