# Security Policy

## Supported versions

SoftGPU is pre-release (Phase 0). Only the default branch receives security-related fixes.

## Reporting a vulnerability

Email or open a **private** security advisory with:

- SoftGPU commit hash / version;
- host OS and architecture;
- reproduction steps and impact;
- whether the issue involves hostile artifacts (ELF, code objects, packets, traces).

Do **not** file public issues for exploitable vulnerabilities until a fix or coordinated disclosure plan exists.

## Threat posture (Phase 0)

SoftGPU is a **developer tool**. Until proven otherwise, it must not run hostile kernels inside a sensitive host process. There is no security boundary merely because execution will eventually be emulated.

See [docs/threat-model.md](docs/threat-model.md).
