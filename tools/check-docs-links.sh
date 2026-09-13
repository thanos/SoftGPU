#!/usr/bin/env bash
# Check that relative markdown links in docs/ and top-level markdown resolve.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

fail=0
files=(README.md SECURITY.md CONTRIBUTING.md)
while IFS= read -r -d '' f; do
  files+=("$f")
done < <(find docs -type f -name '*.md' -print0)

for file in "${files[@]}"; do
  # Extract markdown links: [text](target)
  while IFS= read -r target; do
    [[ -z "$target" ]] && continue
    # Skip URLs, mailto, anchors-only, and absolute paths outside repo
    if [[ "$target" =~ ^https?:// ]] || [[ "$target" =~ ^mailto: ]] || [[ "$target" =~ ^# ]]; then
      continue
    fi
    # Strip anchor
    path="${target%%#*}"
    [[ -z "$path" ]] && continue
    dir="$(dirname "$file")"
    resolved="$dir/$path"
    if [[ ! -e "$resolved" ]]; then
      # Also try from repo root for README-style links
      if [[ ! -e "$path" ]]; then
        echo "BROKEN LINK in $file -> $target"
        fail=1
      fi
    fi
  done < <(grep -oE '\[[^]]+\]\([^)]+\)' "$file" | sed -E 's/.*\(([^)]+)\)$/\1/' || true)
done

if [[ "$fail" -ne 0 ]]; then
  echo "doc link check failed"
  exit 1
fi
echo "doc link check passed (${#files[@]} files)"
