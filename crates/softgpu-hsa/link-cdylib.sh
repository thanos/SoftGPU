#!/usr/bin/env bash
# Replace rustc's anonymous cdylib version script with SoftGPU's named map —
# but only when linking SoftGPU's libhsa_runtime64. GNU ld rejects combining
# rustc's script with a second --version-script. Do not touch other links
# (proc-macros, binaries, dependency cdylibs).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
VERSION_SCRIPT="$ROOT/hsa-runtime64.version"
REAL_LINKER="${SOFTGPU_HSA_REAL_LINKER:-cc}"

is_hsa_cdylib=0
for arg in "$@"; do
  case "$arg" in
    *libhsa_runtime64*) is_hsa_cdylib=1 ;;
  esac
done

args=()
replaced=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    -Wl,--version-script=*|--version-script=*)
      if [[ "$is_hsa_cdylib" -eq 1 ]]; then
        args+=("-Wl,--version-script=${VERSION_SCRIPT}")
        replaced=1
      else
        args+=("$1")
      fi
      shift
      ;;
    -Wl,--version-script|--version-script)
      if [[ "$is_hsa_cdylib" -eq 1 ]]; then
        args+=("-Wl,--version-script=${VERSION_SCRIPT}")
        replaced=1
        shift
        if [[ $# -gt 0 && "$1" != -* ]]; then
          shift
        fi
      else
        args+=("$1")
        shift
      fi
      ;;
    *)
      args+=("$1")
      shift
      ;;
  esac
done

if [[ "$is_hsa_cdylib" -eq 1 && "$replaced" -eq 0 ]]; then
  args+=("-Wl,--version-script=${VERSION_SCRIPT}")
fi

exec "$REAL_LINKER" "${args[@]}"
