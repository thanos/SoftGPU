# ROCm x86_64 integration environment

## Pin

See [`PINNED`](PINNED) for the SoftGPU-supported ROCm image/version.

| Field | Value |
| --- | --- |
| ROCm | **7.14.0** |
| Image | `rocm/dev-ubuntu-24.04:7.14.0-full` |
| Digest | `sha256:439edaa8f0c4be4a3728e528f87b8a2ea1f051f34cf10b27caa4bd94f562eda7` |
| Access date | 2026-09-13 |

## Phase 1 load proof

```bash
# On Linux x86_64 with the pinned ROCm image or an equivalent /opt/rocm:
bash environments/rocm-x86_64/run-phase1-load-proof.sh
```

The script:

1. Builds SoftGPU `libhsa_runtime64` and stages it as `libhsa-runtime64.so`
2. Compiles HIP-linked **load** and **discovery** probes with `hipcc`
3. Runs load proof with SoftGPU ahead of `/opt/rocm` (and a negative control)
4. Runs Phase 2 discovery: HSA iterate from the HIP-linked process must see the SoftGPU GPU (`FEATURE=0`)
5. Sets `SOFTGPU_PROFILE` to the R9700 identity profile for discovery

No kernels are launched.


## Docker

```bash
docker run --rm --platform linux/amd64 \
  -v "$PWD:/src" -w /src \
  rocm/dev-ubuntu-24.04@sha256:439edaa8f0c4be4a3728e528f87b8a2ea1f051f34cf10b27caa4bd94f562eda7 \
  bash environments/rocm-x86_64/ci-entrypoint.sh
```
