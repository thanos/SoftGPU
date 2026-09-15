#!/usr/bin/env bash
# SoftGPU coverage harness (local + CI).
#
# Produces:
#   lcov.info
#   target/llvm-cov/html/
#   target/llvm-cov/summary.json
#   target/llvm-cov/summary.txt
#
# Env:
#   SOFTGPU_COV_FAIL_UNDER_LINES  minimum line % (default: 75)
#   SOFTGPU_COV_FAIL_UNDER_FUNCS  minimum function % (default: 74)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

FAIL_UNDER_LINES="${SOFTGPU_COV_FAIL_UNDER_LINES:-75}"
FAIL_UNDER_FUNCS="${SOFTGPU_COV_FAIL_UNDER_FUNCS:-74}"
# Exclude fixture/example generators and build scripts from the denominator.
IGNORE_REGEX='/(examples|benches)/|/build\.rs$'

mkdir -p target/llvm-cov

if ! rustup component list --installed | grep -q '^llvm-tools'; then
  rustup component add llvm-tools-preview
fi

if ! command -v cargo-llvm-cov >/dev/null 2>&1; then
  echo "error: cargo-llvm-cov is not installed (cargo install cargo-llvm-cov)" >&2
  exit 1
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "error: jq is required to enforce coverage floors" >&2
  exit 1
fi

COMMON=(
  --workspace
  --locked
  --ignore-filename-regex "${IGNORE_REGEX}"
)

echo "==> instrument + test (JSON summary)"
cargo llvm-cov "${COMMON[@]}" \
  --json --summary-only \
  --output-path target/llvm-cov/summary.json \
  "$@"

echo "==> export LCOV + HTML (no re-run)"
# Prefer --workspace --no-run over `cargo llvm-cov report`, which only sees the
# root package in current cargo-llvm-cov versions.
cargo llvm-cov "${COMMON[@]}" --no-run --lcov --output-path lcov.info
cargo llvm-cov "${COMMON[@]}" --no-run --html --output-dir target/llvm-cov

LINES_PCT="$(jq -r '.data[0].totals.lines.percent' target/llvm-cov/summary.json)"
FUNCS_PCT="$(jq -r '.data[0].totals.functions.percent' target/llvm-cov/summary.json)"
LINES_HIT="$(jq -r '.data[0].totals.lines.covered' target/llvm-cov/summary.json)"
LINES_TOT="$(jq -r '.data[0].totals.lines.count' target/llvm-cov/summary.json)"
FUNCS_HIT="$(jq -r '.data[0].totals.functions.covered' target/llvm-cov/summary.json)"
FUNCS_TOT="$(jq -r '.data[0].totals.functions.count' target/llvm-cov/summary.json)"

{
  echo "SoftGPU coverage summary"
  echo "  lines:     ${LINES_HIT}/${LINES_TOT} (${LINES_PCT}%)  floor=${FAIL_UNDER_LINES}%"
  echo "  functions: ${FUNCS_HIT}/${FUNCS_TOT} (${FUNCS_PCT}%)  floor=${FAIL_UNDER_FUNCS}%"
  echo
  echo "Top gaps (lowest line % among SoftGPU crates):"
  jq -r '
    .data[0].files
    | map(select(.filename | test("SoftGPU/(crates|src)/")))
    | sort_by(.summary.lines.percent)
    | .[0:12][]
    | "  \(.summary.lines.percent | floor)%\t\(.filename | sub(".*/SoftGPU/"; ""))"
  ' target/llvm-cov/summary.json
} | tee target/llvm-cov/summary.txt

awk -v lines="$LINES_PCT" -v funcs="$FUNCS_PCT" \
    -v min_l="$FAIL_UNDER_LINES" -v min_f="$FAIL_UNDER_FUNCS" '
  BEGIN {
    err = 0
    if (lines + 0 < min_l + 0) {
      printf "error: line coverage %.2f%% is below floor %s%%\n", lines, min_l > "/dev/stderr"
      err = 1
    }
    if (funcs + 0 < min_f + 0) {
      printf "error: function coverage %.2f%% is below floor %s%%\n", funcs, min_f > "/dev/stderr"
      err = 1
    }
    exit err
  }
'

echo "==> coverage artifacts: lcov.info, target/llvm-cov/html/, target/llvm-cov/summary.{json,txt}"
