# Article 5 — HSA/AQL dispatch end to end

- **Audience:** runtime engineers and SoftGPU contributors
- **Prerequisites:** Articles 1–4
- **Evidence:** SoftGPU Phase 4 AQL parser + diagnostic contract + charter tests
- **Access date:** 2026-09-14

## Problem

A HIP “kernel launch” is not one API call. It is a chain that ends in an
**AQL packet** on a user-mode queue. Emulators that jump straight to “run ISA”
skip the packet contract that HIP and ROCr actually share.

## End-to-end path (ROCm/HIP view)

```text
HIP launch APIs
    → HIP runtime prepares kernel object + kernarg
    → producer writes hsa_kernel_dispatch_packet_t into queue ring
    → producer stores write_index, then doorbell
    → packet processor reads packets, runs (or rejects), stores completion
```

SoftGPU Phase 4 implements the **middle**: decode, validate, trace, and an
experimental **no-execution** completion. It does **not** run kernel code.

## Packet layout (kernel dispatch)

Under the large model, each AQL packet is **64 bytes**. SoftGPU’s parser follows
pinned ROCR `hsa.h` field offsets:

| Offset | Field |
| --- | --- |
| 0 | `header` (type in low 8 bits) |
| 2 | `setup` (dimensions in low bits) |
| 4–10 | workgroup size x/y/z |
| 12–24 | grid size x/y/z |
| 24 / 28 | private / group segment size |
| 32 | kernel object |
| 40 | kernarg address |
| 56 | completion signal |

Producers write the **type byte last** so observers never see a half-published
packet as `KERNEL_DISPATCH`.

## SoftGPU Phase 4 processor

```text
doorbell store
    → observe packets in [observed_through, write_index) once
    → parse_supported_packet (dispatch or minimal barrier)
    → on OK: DispatchValidated + capture bytes
            → store completion:=0 (diagnostic)
            → invalidate slot; advance read_index
            → label diagnostic_complete_no_execution / not_kernel_success
    → on Err: DispatchRejected
            → leave completion unchanged
            → still invalidate + advance read_index (reject contract)
```

See [`docs/aql-diagnostic-contract.md`](../aql-diagnostic-contract.md).

## Capture and replay

Validated packets keep an owned `[u8; 64]`. Offline `replay_dispatch` re-parses
those bytes without the original process and without depending on live host
pointers SoftGPU once classified. Replay reclassifies non-null kernarg as
`foreign_opaque`.

## What “complete” means here

In hardware ROCr, completion usually means the GPU finished the dispatch. In
SoftGPU Phase 4, completion means **only** that SoftGPU applied the documented
diagnostic contract. Logs and probes must print that distinction.

## What comes next

Phase 5 identifies kernels inside AMD code objects. Phase 6 begins functional
CPU execution of a tiny, disclosed subset — still not gfx1201 ISA emulation.

## References

- SoftGPU Phase 4 charter tests (`crates/softgpu-core/tests/phase4_aql.rs`)
- SoftGPU ROCm probe `tools/softgpu-phase4-aql-probe/`
- Article 4 (queues/signals), Article 3 (honesty)
