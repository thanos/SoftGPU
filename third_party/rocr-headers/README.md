# SoftGPU-vendored ROCR public headers (probe / stub generation)

| Field | Value |
| --- | --- |
| Upstream | https://github.com/ROCm/rocm-systems |
| Path | `projects/rocr-runtime/runtime/hsa-runtime/inc/` |
| Commit | see `COMMIT.txt` |
| Access date | see `ACCESS_DATE.txt` |
| SoftGPU use | Phase 1+ ABI layout probes and fail-closed stub generation; **not** a redistribution of the full ROCR runtime |
| License | NCSA as stated in header files |

Headers currently vendored under `hsa/`: `hsa.h`, `hsa_ext_amd.h`, and transitive includes required to compile AMD extension stubs.

Do not treat these as a substitute for a pinned ROCm sysroot in integration CI. Prefer `/opt/rocm/include` from the pinned image when available, and diff against this commit.
