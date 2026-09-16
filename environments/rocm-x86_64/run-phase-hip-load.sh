#!/usr/bin/env bash
# SoftGPU v0.8: HSA executable load proof under SoftGPU ROCr (host unit + optional HIP).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

echo "== SoftGPU phase-hip-load (executable subset) =="
cargo test -p softgpu-core --test phase_hip_load --locked

# Optional: if hipcc is present, compile a tiny gfx1201 HSACO for inspect-only honesty.
if command -v hipcc >/dev/null 2>&1; then
  echo "== optional hipcc gfx1201 compile (inspect metadata; SoftGPU execute uses SoftGPU fixture) =="
  TMP="$(mktemp -d)"
  cat >"$TMP/tiny_add.cpp" <<'HIP'
#include <hip/hip_runtime.h>
extern "C" __global__ void tiny_add(const int* a, int* b) {
  size_t i = hipBlockIdx_x * hipBlockDim_x + hipThreadIdx_x;
  b[i] = a[i] + 1;
}
HIP
  if hipcc --genco --offload-arch=gfx1201 -o "$TMP/tiny_add.hsaco" "$TMP/tiny_add.cpp"; then
    cargo run --locked --quiet -- inspect-code-object "$TMP/tiny_add.hsaco" || true
    echo "NOTE: official hipcc .text may require further SoftGPU ISA growth; SoftGPU CI gate is phase_hip_load fixture."
  else
    echo "hipcc compile skipped/failed (acceptable when arch/tooling unavailable)"
  fi
  rm -rf "$TMP"
else
  echo "hipcc not on PATH; SoftGPU executable gate is cargo test phase_hip_load"
fi

echo "OK: phase-hip-load"
