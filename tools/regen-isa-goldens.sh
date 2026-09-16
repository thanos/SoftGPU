#!/usr/bin/env bash
# Regenerate crates/softgpu-amd-isa/goldens/llvm-mc-gfx1201.json from llvm-mc.
# Requires: llvm-mc with AMDGPU target (e.g. Homebrew llvm).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MC="${LLVM_MC:-}"
if [[ -z "$MC" ]]; then
  for c in /opt/homebrew/opt/llvm/bin/llvm-mc llvm-mc; do
    if command -v "$c" >/dev/null 2>&1 || [[ -x "$c" ]]; then
      MC="$c"
      break
    fi
  done
fi
if [[ -z "${MC}" || ! -x "$(command -v "$MC" 2>/dev/null || true)" && ! -x "$MC" ]]; then
  echo "llvm-mc not found; set LLVM_MC=" >&2
  exit 1
fi

OUT="$ROOT/crates/softgpu-amd-isa/goldens/llvm-mc-gfx1201.json"
VER="$("$MC" --version | head -1)"
DATE="$(date -u +%Y-%m-%d)"

ASMS=(
  "s_nop 0"
  "s_nop 1"
  "s_endpgm"
  "s_sleep 1"
  "s_waitcnt 0"
  "s_mov_b32 s0, 0"
  "s_mov_b32 s0, 1"
  "s_mov_b32 s0, 64"
  "s_mov_b32 s0, -1"
  "s_mov_b32 s0, s1"
  "s_mov_b32 s5, s3"
  "s_add_co_u32 s0, s1, s2"
  "s_add_co_u32 s2, s0, s1"
  "s_add_co_u32 s3, s4, s5"
)

{
  echo '{'
  echo '  "schema": "softgpu-isa-golden-v1",'
  echo '  "arch": "gfx1201",'
  echo '  "tool": "llvm-mc",'
  echo "  \"tool_version\": \"$VER\","
  echo '  "triple": "amdgcn-amd-amdhsa",'
  echo '  "mcpu": "gfx1201",'
  echo "  \"access_date\": \"$DATE\","
  echo '  "source_docs": ['
  echo '    "https://llvm.org/docs/AMDGPUUsage.html",'
  echo '    "llvm-mc -arch=amdgcn -mcpu=gfx1201 -show-encoding"'
  echo '  ],'
  echo '  "license_note": "Encodings observed from LLVM tools (Apache-2.0 WITH LLVM-exception); SoftGPU tables are hand-maintained with provenance, not copied from restricted ISA manuals.",'
  echo '  "encodings": ['
  first=1
  for asm in "${ASMS[@]}"; do
    enc=$(printf '%s\n' "$asm" | "$MC" -arch=amdgcn -mcpu=gfx1201 -show-encoding 2>/dev/null \
      | sed -n 's/.*encoding: \[\(.*\)\]/\1/p')
    if [[ -z "$enc" ]]; then
      echo "failed to encode: $asm" >&2
      exit 1
    fi
    # enc like 0x00,0x00,0x80,0xbf
    IFS=', ' read -r b0 b1 b2 b3 <<<"$enc"
    word=$(printf '0x%02x%02x%02x%02x' "$((b3))" "$((b2))" "$((b1))" "$((b0))")
    if [[ $first -eq 0 ]]; then echo ','; fi
    first=0
    printf '    {"asm": "%s", "bytes_le": [%s], "word": "%s"}' "$asm" "$enc" "$word"
    echo
  done
  echo '  ]'
  echo '}'
} >"$OUT"

echo "wrote $OUT"
