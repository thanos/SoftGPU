# Threat model (Phase 0)

## Assets

- Host process integrity and confidentiality when SoftGPU later loads untrusted GPU artifacts.
- Developer machines and CI runners.
- Integrity of SoftGPU evidence (profiles, traces, conformance bundles).

## Trust boundaries (current and near-term)

| Boundary | Trust |
| --- | --- |
| SoftGPU source + pinned deps | Trusted after review |
| Device profiles / CLI args / env | Untrusted |
| Future: ELF/code objects, AQL packets, traces | Untrusted |
| Future: HIP applications | Untrusted |
| Official ROCm packages in CI | Trusted distribution channel, still validate digests when pinned |

## Adversaries

- Malformed or hostile binaries/packets crafted to crash SoftGPU, exhaust memory, or escape via bugs.
- Accidental over-privilege: treating SoftGPU as a sandbox for hostile kernels.

## Mitigations started in Phase 0

- Fail-closed unsupported behavior.
- Profile schema rejects empty/invalid and conformance lies.
- Resource budgets and parser hardening are mandatory when parsers appear (Phase 5+).
- `SECURITY.md` reporting path.

## Non-goals

SoftGPU does **not** currently claim isolation equivalent to a VM or kernel sandbox.
