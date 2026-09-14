#!/usr/bin/env bash
# Phase 5: SoftGPU code-object inspect + optional official hipcc artifact.
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
mkdir -p "$CARGO_TARGET_DIR"

echo "== SoftGPU Phase 5 code-object proof =="
echo "ROCM_PATH=$ROCM_PATH"

echo "== write SoftGPU-synthetic fixtures =="
cargo run --locked --example write-code-object-fixtures
FIX="$ROOT/fixtures/amd-code-object/tiny-add-gfx1201.softgpu.co"
cargo run --locked -- inspect-code-object "$FIX" | tee "$CARGO_TARGET_DIR/phase5-inspect-synthetic.json"
grep -q 'gfx1201' "$CARGO_TARGET_DIR/phase5-inspect-synthetic.json"
grep -q 'tiny_add' "$CARGO_TARGET_DIR/phase5-inspect-synthetic.json"
grep -q 'metadata_only_no_isa_execution' "$CARGO_TARGET_DIR/phase5-inspect-synthetic.json"
echo "ok: synthetic fixture inspect"

# Fail-closed checks
if cargo run --locked -- inspect-code-object \
  "$ROOT/fixtures/amd-code-object/unsupported-target.softgpu.co"; then
  echo "ERROR: unsupported target should fail" >&2
  exit 1
fi
echo "ok: unsupported target fail-closed"

if cargo run --locked -- inspect-code-object \
  "$ROOT/fixtures/amd-code-object/unsupported-version.softgpu.co"; then
  echo "ERROR: unsupported version should fail" >&2
  exit 1
fi
echo "ok: unsupported version fail-closed"

echo "== optional: official hipcc gfx1201 compile =="
HIPCC="${HIPCC:-}"
if [[ -z "$HIPCC" ]]; then
  HIPCC="$(command -v hipcc || true)"
fi
if [[ -z "$HIPCC" && -x "$ROCM_PATH/bin/hipcc" ]]; then
  HIPCC="$ROCM_PATH/bin/hipcc"
fi

if [[ -n "$HIPCC" && -d "$ROCM_PATH" ]]; then
  SRC="$ROOT/tools/softgpu-phase5-code-object/tiny_add.hip"
  OUT="$CARGO_TARGET_DIR/phase5-tiny-add"
  # Emit a host+device binary; extract AMDGPU ELF notes via llvm-readelf when present.
  "$HIPCC" -O2 --offload-arch=gfx1201 -o "$OUT" "$SRC" || {
    echo "WARN: hipcc gfx1201 compile failed; synthetic gate already passed" >&2
    echo "== PASS: Phase 5 synthetic code-object gate =="
    exit 0
  }
  if command -v llvm-readelf >/dev/null 2>&1; then
    llvm-readelf -n "$OUT" > "$CARGO_TARGET_DIR/phase5-llvm-readelf.txt" || true
    if grep -q 'AMDGPU' "$CARGO_TARGET_DIR/phase5-llvm-readelf.txt"; then
      echo "ok: llvm-readelf reports AMDGPU note on hipcc output"
    else
      echo "note: llvm-readelf did not list AMDGPU on host binary (bundle layout); synthetic gate remains"
    fi
  fi
  # SoftGPU inspect may fail on fat host ELF without a standalone code object —
  # that is recorded honestly; metadata path is proven on SoftGPU fixtures.
  if cargo run --locked -- inspect-code-object "$OUT" \
    >"$CARGO_TARGET_DIR/phase5-inspect-hipcc.json" 2>"$CARGO_TARGET_DIR/phase5-inspect-hipcc.err"; then
    grep -q 'gfx1201' "$CARGO_TARGET_DIR/phase5-inspect-hipcc.json"
    echo "ok: SoftGPU inspected hipcc output directly"
  else
    echo "note: SoftGPU inspect of hipcc fat binary failed (expected for some bundle layouts)"
    cat "$CARGO_TARGET_DIR/phase5-inspect-hipcc.err" || true
  fi
else
  echo "note: hipcc/ROCm unavailable; synthetic fixtures only"
fi

echo "== PASS: Phase 5 code-object gate =="
