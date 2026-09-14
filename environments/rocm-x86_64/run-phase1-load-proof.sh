#!/usr/bin/env bash
# Phase 1: build SoftGPU HSA cdylib, stage as libhsa-runtime64.so, run HIP load proof.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

PIN_FILE="$ROOT/environments/rocm-x86_64/PINNED"
if [[ -f "$PIN_FILE" ]]; then
  # shellcheck disable=SC1090
  source "$PIN_FILE"
fi

: "${ROCM_PATH:=/opt/rocm}"
: "${CARGO_TARGET_DIR:=$ROOT/target}"
# Fresh CI checkouts (and Docker mounts) may not have target/ yet; probes write here
# before cargo creates it.
mkdir -p "$CARGO_TARGET_DIR"

echo "== SoftGPU Phase 1 ROCm load proof =="
echo "ROCM_PATH=$ROCM_PATH"
echo "ROCM_VERSION=${ROCM_VERSION:-unknown}"
uname -m
if [[ "$(uname -m)" != "x86_64" ]]; then
  echo "ERROR: Phase 1 load proof requires Linux x86_64 (got $(uname -m))" >&2
  exit 2
fi

if [[ ! -d "$ROCM_PATH" ]]; then
  echo "ERROR: ROCm not found at $ROCM_PATH" >&2
  exit 2
fi

export PATH="$ROCM_PATH/bin:${PATH:-}"

echo "== layout probe =="
if [[ -f "$ROCM_PATH/include/hsa/hsa.h" ]]; then
  cc -I"$ROCM_PATH/include" -o "$CARGO_TARGET_DIR/hsa-layout-probe-rocm" \
    tools/hsa-layout-probe/probe.c
  "$CARGO_TARGET_DIR/hsa-layout-probe-rocm"
else
  cc -I third_party/rocr-headers/hsa -o "$CARGO_TARGET_DIR/hsa-layout-probe" \
    tools/hsa-layout-probe/probe.c
  # probe.c includes "hsa/hsa.h" — adjust if using flat include
  if ! "$CARGO_TARGET_DIR/hsa-layout-probe" 2>/dev/null; then
    cc -I third_party/rocr-headers -o "$CARGO_TARGET_DIR/hsa-layout-probe" \
      tools/hsa-layout-probe/probe.c
    "$CARGO_TARGET_DIR/hsa-layout-probe"
  fi
fi

echo "== build SoftGPU HSA cdylib =="
cargo build -p softgpu-hsa --release --locked

LIBDIR="$CARGO_TARGET_DIR/softgpu-hsa-stage"
rm -rf "$LIBDIR"
mkdir -p "$LIBDIR"

SRC_SO="$(find "$CARGO_TARGET_DIR/release" -maxdepth 2 -name 'libhsa_runtime64.so' -print -quit)"
if [[ -z "$SRC_SO" || ! -f "$SRC_SO" ]]; then
  echo "ERROR: libhsa_runtime64.so not found under $CARGO_TARGET_DIR/release" >&2
  find "$CARGO_TARGET_DIR/release" -name '*.so' 2>/dev/null | awk 'NR<=50' >&2 || true
  exit 1
fi
cp -f "$SRC_SO" "$LIBDIR/libhsa-runtime64.so"
ln -sfn libhsa-runtime64.so "$LIBDIR/libhsa-runtime64.so.1"
cp -f "$SRC_SO" "$LIBDIR/libhsa_runtime64.so"
echo "Staged SoftGPU HSA libs in $LIBDIR"
ls -la "$LIBDIR"
echo "== exported hsa_* symbols (sample) =="
# Avoid `... | head` under `set -o pipefail` (SIGPIPE → exit 141).
nm -D --defined-only "$LIBDIR/libhsa-runtime64.so" \
  | awk '/ T hsa_/ { c++; if (c <= 40) print } END { print "(count)", c+0 }'

echo "== verify ELF version node ROCR_1 (required by libamdhip64) =="
if ! nm -D --defined-only "$LIBDIR/libhsa-runtime64.so" | grep -q '@@ROCR_1'; then
  echo "ERROR: SoftGPU libhsa-runtime64.so lacks symbol version ROCR_1" >&2
  echo "HIP will fail at load with: version \`ROCR_1' not found" >&2
  if command -v readelf >/dev/null 2>&1; then
    readelf -V "$LIBDIR/libhsa-runtime64.so" >&2 || true
  fi
  exit 1
fi
echo "ok: found @@ROCR_1 on exported hsa_* symbols"

echo "== build HIP-linked probe =="
HIPCC="${HIPCC:-}"
if [[ -z "$HIPCC" ]]; then
  HIPCC="$(command -v hipcc || true)"
fi
if [[ -z "$HIPCC" && -x "$ROCM_PATH/bin/hipcc" ]]; then
  HIPCC="$ROCM_PATH/bin/hipcc"
fi
if [[ -z "$HIPCC" ]]; then
  echo "ERROR: hipcc not found" >&2
  exit 2
fi
"$HIPCC" -O2 -o "$CARGO_TARGET_DIR/softgpu-hip-load-probe" \
  tools/softgpu-hip-load-probe/main.cpp

echo "== NEGATIVE: system ROCr must be detected as failure mode =="
# Intentionally put only ROCm libs on the path — SoftGPU must NOT be selected.
if SOFTGPU_HSA_LIBDIR="/nonexistent-softgpu-path" \
  LD_LIBRARY_PATH="$ROCM_PATH/lib:${LD_LIBRARY_PATH:-}" \
  "$CARGO_TARGET_DIR/softgpu-hip-load-probe"; then
  echo "ERROR: probe unexpectedly passed without SoftGPU library dir" >&2
  exit 1
else
  echo "ok: probe failed closed without SoftGPU (expected)"
fi

echo "== POSITIVE: SoftGPU ahead of system ROCr =="
export SOFTGPU_HSA_LIBDIR="$LIBDIR"
export LD_LIBRARY_PATH="$LIBDIR:$ROCM_PATH/lib:${LD_LIBRARY_PATH:-}"
export SOFTGPU_PROFILE="$ROOT/profiles/amd-radeon-ai-pro-r9700-gfx1201-v0.json"
"$CARGO_TARGET_DIR/softgpu-hip-load-probe"

echo "== Phase 2/3: HIP-linked HSA agent discovery (FEATURE=KERNEL_DISPATCH) =="
# Discovery probe calls HSA APIs directly; link SoftGPU's staged libhsa-runtime64
# (same SONAME HIP uses). Runtime LD_LIBRARY_PATH already prefers SoftGPU.
"$HIPCC" -O2 -o "$CARGO_TARGET_DIR/softgpu-hip-discovery-probe" \
  tools/softgpu-hip-discovery-probe/main.cpp \
  -L"$LIBDIR" -Wl,-rpath-link,"$LIBDIR" -lhsa-runtime64
"$CARGO_TARGET_DIR/softgpu-hip-discovery-probe"

echo "== Phase 3: memory Path C + queue observe =="
cc -O2 -I"$ROCM_PATH/include" -o "$CARGO_TARGET_DIR/softgpu-phase3-queue-probe" \
  tools/softgpu-phase3-queue-probe/main.c \
  -L"$LIBDIR" -Wl,-rpath-link,"$LIBDIR" -lhsa-runtime64
"$CARGO_TARGET_DIR/softgpu-phase3-queue-probe"

echo "== PASS: Phase 1 load proof + Phase 2/3 discovery + Phase 3 queue observe =="
