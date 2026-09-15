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
4. Runs discovery: HSA iterate from the HIP-linked process must see the SoftGPU GPU with `FEATURE=KERNEL_DISPATCH` (queue + AQL intercept)
5. Runs Phase 3 probe: Path C memory allocate + queue create/observe
6. Runs Phase 4 probe: SoftGPU-controlled golden AQL packet → diagnostic complete
7. Runs Phase 5: code-object metadata inspect (synthetic fixtures; optional hipcc)
8. Sets `SOFTGPU_PROFILE` to the R9700 identity profile for discovery

No kernels are executed by SoftGPU.

## Local CI (match GitHub Actions)

The `rocm-integration` job runs the pinned image as **`linux/amd64`**. On Apple Silicon that means Rosetta/QEMU translation — expect it to be slower than a native x86_64 host. GitHub Actions still uses Docker on Linux runners; the commands below are for local reproduction only.

Shared setup (repo root):

```bash
source environments/rocm-x86_64/PINNED
chmod +x environments/rocm-x86_64/ci-entrypoint.sh \
         environments/rocm-x86_64/run-phase1-load-proof.sh \
         crates/softgpu-hsa/link-cdylib.sh
```

`CARGO_HOME` / `RUSTUP_HOME` under `/tmp` match CI so rustup does not write into a missing `~/.cargo` inside the container.

### Docker

Closest drop-in match to [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml):

```bash
docker pull "${ROCM_IMAGE}@${ROCM_IMAGE_DIGEST}"

docker run --rm \
  --platform linux/amd64 \
  -v "$PWD:/src:rw" \
  -w /src \
  -e CARGO_HOME=/tmp/cargo \
  -e RUSTUP_HOME=/tmp/rustup \
  "${ROCM_IMAGE}@${ROCM_IMAGE_DIGEST}" \
  bash environments/rocm-x86_64/ci-entrypoint.sh
```

Requires [Docker Desktop](https://www.docker.com/products/docker-desktop/) (or another Docker Engine) with amd64 emulation enabled on Apple Silicon.

### Apple Container

[Apple’s `container` CLI](https://github.com/apple/container) runs OCI Linux images in lightweight VMs on Apple Silicon (macOS 26 recommended). Install, then start the system once:

```bash
brew install container
container system start
# container system kernel set --recommended   # first-time setup
```

Then:

```bash
container pull --platform linux/amd64 "${ROCM_IMAGE}@${ROCM_IMAGE_DIGEST}"

container run --rm \
  --platform linux/amd64 \
  -v "$PWD:/src" \
  -w /src \
  -e CARGO_HOME=/tmp/cargo \
  -e RUSTUP_HOME=/tmp/rustup \
  "${ROCM_IMAGE}@${ROCM_IMAGE_DIGEST}" \
  bash environments/rocm-x86_64/ci-entrypoint.sh
```

Notes:

- `-v` / `-w` / `-e` mirror Docker; `--mount type=bind,source=...,target=/src` also works if you prefer explicit bind mounts.
- amd64 ROCm images run under **Rosetta** on Apple Silicon (`container system property` documents `build.rosetta`).
- Flag names can differ slightly by `container` version — use `container run --help` if a flag is rejected.
- This does **not** replace CI’s Docker path on `ubuntu-24.04`; it only helps you reproduce the gate locally without Docker Desktop.
