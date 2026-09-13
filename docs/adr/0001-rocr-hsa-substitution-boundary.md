# ADR-0001: ROCr/HSA substitution boundary

- Status: Accepted (Phase 0 architectural intent; load proof deferred to Phase 1)
- Date: 2026-09-13
- Deciders: SoftGPU Phase 0 charter

## Context

SoftGPU needs a boundary where real AMD HIP applications can run against a virtual GPU without rewriting application source. Candidate seams include mocking HIP, substituting ROCr/HSA, emulating the kernel driver/device, or executing code objects out-of-band.

## Evidence

- AMD documents ROCR as AMD’s HSA runtime implementation exposing user-mode interfaces used with the AMDGPU/ROCK stack ([ROCR docs 7.14.0](https://rocm.docs.amd.com/projects/ROCR-Runtime/en/docs-7.14.0/index.html)).
- HSA Runtime 1.2 defines the portable runtime programmer’s model SoftGPU must not invent from memory ([HSA Runtime 1.2 PDF](http://hsafoundation.com/wp-content/uploads/2021/02/HSA-Runtime-1.2.pdf)).
- SoftGPU’s mission retains the **real** HIP compiler/runtime and targets interception/substitution at the lowest practical **user-space hardware-facing** boundary.
- Kernel-mode or PCIe/MMIO emulation is explicitly out of initial scope (charter §1).

## Decision

**Primary integration boundary:** SoftGPU will provide a Linux `cdylib` that implements a verified subset of the ROCr/HSA C ABI, selected so real HIP userspace resolves SoftGPU instead of system ROCr for the launched process.

SoftGPU will **not** (initially):

- ship a replacement HIP SDK or mock HIP API surface as the compatibility path;
- implement a kernel-mode AMDGPU driver or PCIe/MMIO/firmware device model;
- claim success by directly executing code objects without the real HIP→ROCr dispatch path, except as explicitly labeled research spikes.

Phase 1 must prove load/symbol resolution with pinned ROCm before any broad ISA work.

## Alternatives considered

### A. Fake / mock HIP

- **Pros:** Familiar API; possibly fewer packet-level details early.
- **Cons:** Diverges from the supported application stack; duplicates a large surface; weakens “same binary vs SoftGPU or R9700” goal; encourages inventing HIP behavior.
- **Rejected** as the primary path.

### B. ROCr/HSA substitution (chosen)

- **Pros:** Keeps official HIP compiler/runtime; targets the user-space hardware-facing library HIP already uses; matches charter.
- **Cons:** ABI/symbol-version fragility; requires careful export control and honest unsupported errors; Linux/ROCm-centric proof.
- **Accepted** pending Phase 1 load proof.

### C. Kernel-driver / device emulation

- **Pros:** Could intercept below userspace completely.
- **Cons:** Enormous scope; firmware/MMIO fidelity; not a developer-first sanitizer path; charter excludes it initially.
- **Rejected** for foundation phases.

### D. Direct code-object execution only

- **Pros:** Faster path to “run a kernel” demos.
- **Cons:** Skips queues/signals/AQL ownership lessons; weak integration evidence; easy to over-claim compatibility.
- **Rejected** as the primary compatibility story; may appear later as an explicit non-conformance tool mode.

## Consequences

- Phase 1 prioritizes header/symbol/layout probes and a minimal exported library.
- Vendor-neutral core must not depend on HSA types; adapters translate.
- CI’s canonical integration gate is Linux x86-64 + pinned ROCm, not macOS.
- Documentation must say “ROCr/HSA subset at fidelity ABI/Protocol” rather than “HIP compatible” until evidence exists.

## Validation plan

1. Phase 1: HIP-linked probe loads SoftGPU; prove which library/symbols resolved; accidental system ROCr fails the test.
2. Independent C layout probes vs pinned headers.
3. Support matrix cells move from `unsupported` → `verified-integration` only with that evidence.

## Reconsider if

- Pinned HIP stops linking against a substitutable ROCr/HSA userspace library.
- Legal/distribution constraints prevent shipping a compatible adapter.
- A narrower, officially supported interception point appears and is evidenced to be more faithful.
