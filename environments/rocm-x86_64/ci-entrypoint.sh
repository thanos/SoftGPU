#!/usr/bin/env bash
# Entrypoint used by CI/docker for Phase 1 load proof inside pinned ROCm image.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

cargo_env() {
  # Honor CARGO_HOME (CI uses /tmp/cargo); fall back to the rustup default.
  local env_file="${CARGO_HOME:-$HOME/.cargo}/env"
  if [[ -f "$env_file" ]]; then
    # shellcheck disable=SC1090
    source "$env_file"
  fi
}

if ! command -v rustc >/dev/null 2>&1; then
  echo "== install Rust 1.85.0 =="
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.85.0 -c rustfmt,clippy
  cargo_env
fi

# Ensure PATH sees a just-installed or volume-mounted toolchain.
if ! command -v rustc >/dev/null 2>&1; then
  cargo_env
fi

rustc --version
cargo --version

# Ensure generator outputs exist (committed, but regenerate for hygiene).
perl tools/generate-hsa-stubs.pl \
  third_party/rocr-headers/hsa/hsa.h \
  crates/softgpu-hsa/src/generated_stubs.c \
  '#include "hsa.h"'
perl tools/generate-hsa-stubs.pl \
  third_party/rocr-headers/hsa/hsa_ext_amd.h \
  crates/softgpu-hsa/src/generated_amd_stubs.c \
  '#include "hsa_ext_amd.h"'

chmod +x environments/rocm-x86_64/run-phase1-load-proof.sh \
  environments/rocm-x86_64/run-phase-hip-load.sh \
  crates/softgpu-hsa/link-cdylib.sh
bash environments/rocm-x86_64/run-phase1-load-proof.sh
bash environments/rocm-x86_64/run-phase-hip-load.sh
