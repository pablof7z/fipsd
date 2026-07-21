#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repository_root"

soft_limit=300
hard_limit=600
hard_failures=0

while IFS= read -r -d '' path; do
  if head -n 5 "$path" | rg -q '@generated|DO NOT EDIT'; then
    continue
  fi
  lines="$(wc -l < "$path" | tr -d ' ')"
  if (( lines > hard_limit )); then
    echo "LOC hard limit: $path has $lines lines (maximum $hard_limit)" >&2
    hard_failures=$((hard_failures + 1))
  elif (( lines > soft_limit )); then
    echo "LOC soft warning: $path has $lines lines (target $soft_limit)" >&2
  fi
done < <(
  find crates tools scripts web \
    -type f \
    \( -name '*.rs' -o -name '*.sh' -o -name '*.ts' -o -name '*.tsx' \
       -o -name '*.js' -o -name '*.jsx' -o -name '*.css' -o -name '*.html' \) \
    -print0 2>/dev/null || true
)

if (( hard_failures > 0 )); then
  exit 1
fi
