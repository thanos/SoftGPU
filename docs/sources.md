# SoftGPU source-of-truth ledger

Facts SoftGPU depends on must be recorded here with provenance. Secondary blogs may explain concepts but must not establish ABI or ISA facts.

**Ledger access date:** 2026-09-13

## How to read an entry

| Field | Meaning |
| --- | --- |
| Status | `normative` / `observed` / `inferred` / `provisional` |
| SoftGPU use | What we derive |
| Test/artifact | How we check it |

---

## HSA Foundation — Runtime Programmer’s Reference 1.2

| | |
| --- | --- |
| Primary URL | http://hsafoundation.com/wp-content/uploads/2021/02/HSA-Runtime-1.2.pdf |
| Index | https://hsafoundation.com/standards/ |
| Version | HSA Runtime Programmer’s Reference Manual 1.2 (issue date 2018-05-02 per document) |
| Access date | 2026-09-13 |
| Installed package | not installed in Phase 0 host environments |
| SoftGPU use | Normative concepts for runtime API families SoftGPU may eventually implement; **no ABI layouts transcribed in Phase 0** |
| Status | normative (spec); SoftGPU implementation status = unsupported |
| License/provenance | HSA Foundation copyright; do not redistribute the PDF in-tree; link only |
| Test/artifact | deferred to Phase 1 layout/symbol probes against pinned ROCm headers |

---

## AMD ROCR (ROCm HSA runtime) documentation

| | |
| --- | --- |
| Primary URL | https://rocm.docs.amd.com/projects/ROCR-Runtime/en/docs-7.14.0/index.html |
| API reference | https://rocm.docs.amd.com/projects/ROCR-Runtime/en/docs-7.14.0/api-reference/api.html |
| Version | ROCm docs branch `docs-7.14.0`; ROCR docs label **1.21.0** |
| Access date | 2026-09-13 |
| Source tree (current) | AMD documents ROCR source under the ROCm systems monorepo; exact commit pin deferred to Phase 1 CI image |
| SoftGPU use | Defines the intended substitution boundary (ADR-0001); Phase 1 will pin headers/symbols from an official image |
| Status | normative for AMD’s ROCR behavior relative to HSA + extensions |
| License/provenance | AMD/ROCm documentation terms; headers/source license reviewed before copy |
| Test/artifact | Phase 1 loader proof + layout probes |

---

## AMD HIP documentation

| | |
| --- | --- |
| Primary URL | https://rocm.docs.amd.com/projects/HIP/en/docs-7.14.0/ |
| Version | HIP docs labeled **7.14.60850** under ROCm docs `docs-7.14.0` (re-pin in Phase 1 CI) |
| Access date | 2026-09-13 |
| SoftGPU use | Application-facing stack SoftGPU does **not** mock; real HIP userspace is required for integration |
| Status | normative for HIP userspace expectations; SoftGPU does not re-implement HIP |
| Test/artifact | Phase 1+ HIP-linked host probes in x86-64 ROCm CI |

---

## ROCm compatibility — Radeon AI PRO R9700 / gfx1201

| | |
| --- | --- |
| Primary URLs | https://rocm.docs.amd.com/projects/install-on-linux/en/docs-7.14.0/reference/system-requirements.html (prefer release-matched docs; develop tree also lists the device) |
| Observed fact | AMD lists **AMD Radeon AI PRO R9700** with architecture **RDNA4** and LLVM target **gfx1201** |
| Access date | 2026-09-13 |
| SoftGPU use | Profile identity `amd-radeon-ai-pro-r9700-gfx1201` llvm_target=`gfx1201` with provenance `verified` for the **name→target mapping only** |
| Status | verified (identity mapping); **numeric limits remain unknown** |
| Test/artifact | `profiles/amd-radeon-ai-pro-r9700-gfx1201-v0.json` + `tests/profile_schema.rs` |

---

## LLVM AMDGPU backend usage

| | |
| --- | --- |
| Primary URL | https://llvm.org/docs/AMDGPUUsage.html |
| ROCm-packaged mirror | https://rocm.docs.amd.com/projects/llvm-project/en/latest/LLVM/llvm/html/AMDGPUUsage.html |
| Version | LLVM docs as of access date (site may show in-development version numbers) |
| Access date | 2026-09-13 |
| SoftGPU use | Target ID / code-object concepts for later phases; **no decoder tables vendored in Phase 0** |
| Status | normative for LLVM’s documented AMDGPU target behavior |
| Test/artifact | Phase 5+ golden code objects vs `llvm-readelf` / `llvm-objdump` |

---

## Rust language / toolchain

| | |
| --- | --- |
| Primary URLs | https://doc.rust-lang.org/reference/ ; https://doc.rust-lang.org/nomicon/ ; https://doc.rust-lang.org/cargo/ ; https://doc.rust-lang.org/rustc/ |
| SoftGPU pin | `rust-toolchain.toml` channel **1.85.0**; Cargo `rust-version` **1.85** |
| Access date | 2026-09-13 |
| SoftGPU use | Host runtime implementation language; MSRV and stable-only policy |
| Status | normative for host language rules |
| Test/artifact | `cargo test --locked` on pinned toolchain |

### rustc AMDGPU compute target (non-path for Phase 0–6 gates)

| | |
| --- | --- |
| Primary URL | https://doc.rust-lang.org/rustc/platform-support/amdgcn-amd-amdhsa.html |
| Access date | 2026-09-13 |
| Observed | Target is **Tier 3**; requires special `build-std` / nightly ABI features for kernels |
| SoftGPU use | Explicitly **out of** the real-HIP integration path; optional research only |
| Status | observed |

---

## Apple `container` (dev infrastructure)

| | |
| --- | --- |
| Primary URL | https://github.com/apple/container |
| Docs | https://apple.github.io/container/documentation/ |
| Access date | 2026-09-13 |
| SoftGPU use | Optional Linux ARM64 laboratory on Apple Silicon; **not** a SoftGPU runtime dependency; **not** the canonical ROCm integration gate |
| Status | observed (tooling); requirements (e.g. macOS version) must be re-checked before recipes land |
| Test/artifact | `environments/apple-container/README.md` placeholder; recipes in a later phase |

---

## ELF

| | |
| --- | --- |
| Primary | System V ABI / ELF specification (use the edition cited when Phase 5 lands) |
| SoftGPU use | Bounded code-object parsing (Phase 5); nothing in Phase 0 |
| Status | deferred |

---

## Conflicts and resolutions

None recorded in Phase 0. When sources disagree: preserve both, prefer pinned toolchain behavior, add an executable probe, isolate version-dependent behavior, document narrowly.
