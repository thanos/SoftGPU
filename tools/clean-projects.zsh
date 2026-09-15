#!/usr/bin/env zsh
# Recurse one level under ROOT and clean Rust / Elixir / Zig / Nim projects.
# Usage: clean-projects.zsh [ROOT]
# Default ROOT: directory containing this script's parent siblings, or $HOME/work.
set -euo pipefail

ROOT="${1:-${HOME}/work}"
if [[ ! -d "$ROOT" ]]; then
  print -u2 "error: not a directory: $ROOT"
  exit 1
fi

has_cmd() { command -v "$1" >/dev/null 2>&1 }

clean_rust() {
  local dir=$1
  if [[ -f "$dir/Cargo.toml" ]]; then
    print "rust:  $dir"
    if has_cmd cargo; then
      (cd "$dir" && cargo clean)
    else
      print -u2 "  warn: cargo not installed; rm -rf target"
      rm -rf "$dir/target"
    fi
  fi
}

clean_elixir() {
  local dir=$1
  if [[ -f "$dir/mix.exs" ]]; then
    print "mix:   $dir"
    # if has_cmd mix; then
    #   (cd "$dir" && mix clean)
    # else
      print -u2 "  warn: mix not installed; rm -rf _build"
      rm -rf "$dir/_build"
    # fi
  fi
}

clean_zig() {
  local dir=$1
  if [[ -f "$dir/build.zig" || -f "$dir/build.zig.zon" ]]; then
    print "zig:   $dir"
    # Zig has no stable universal "clean"; wipe caches/artifacts.
    rm -rf "$dir/zig-cache" "$dir/.zig-cache" "$dir/zig-out"
  fi
}

clean_nim() {
  local dir=$1
  local nimble
  nimble=("$dir"/*.nimble(N))
  if [[ -f "$dir/nimble.toml" || ${#nimble} -gt 0 ]]; then
    print "nim:   $dir"
    if has_cmd nimble; then
      (cd "$dir" && nimble clean) || true
    fi
    # Always clear common caches/artifacts (nimble clean is not always enough).
    rm -rf "$dir/nimcache" "$dir/bin" "$dir/htmldocs"
  fi
}

print "cleaning one level under: $ROOT"
# (/N) = directories only, null if none
for dir in "$ROOT"/*(/N); do
  clean_rust "$dir"
  clean_elixir "$dir"
  clean_zig "$dir"
  clean_nim "$dir"
done
print "done"
