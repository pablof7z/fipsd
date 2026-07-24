#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
manifest="$repository_root/FIPSDPackage/Tests/FIPSDFeatureTests/Resources/upstream-render-vectors.json"
fips_commit="$(jq -r .fips_commit "$manifest")"
temporary_directory="$(mktemp -d)"
trap 'rm -rf "$temporary_directory"' EXIT
fips_checkout="$temporary_directory/fips"
mkdir -p "$fips_checkout"

candidate="${FIPSD_FIPS_SOURCE:-}"
if [[ -z "$candidate" && -d "$repository_root/../fips/.git" ]]; then
  candidate="$repository_root/../fips"
fi

if [[ -n "$candidate" ]] \
  && git -C "$candidate" cat-file -e "$fips_commit^{commit}" 2>/dev/null; then
  git -C "$candidate" archive "$fips_commit" | tar -x -C "$fips_checkout"
else
  git clone --quiet https://github.com/jmcorgan/fips.git "$fips_checkout"
  git -C "$fips_checkout" checkout --quiet "$fips_commit"
fi

while IFS=$'\t' read -r source_path expected_hash; do
  actual_hash="$(shasum -a 256 "$fips_checkout/$source_path" | awk '{print $1}')"
  if [[ "$actual_hash" != "$expected_hash" ]]; then
    echo "source hash mismatch for $source_path" >&2
    echo "expected $expected_hash" >&2
    echo "actual   $actual_hash" >&2
    exit 1
  fi
done < <(jq -r '.sources[] | [.path, .sha256] | @tsv' "$manifest")

while IFS= read -r test_name; do
  echo "running pinned FIPS vector: $test_name"
  cargo test \
    --quiet \
    --manifest-path "$fips_checkout/Cargo.toml" \
    --target-dir "$repository_root/target/fips-render-vectors" \
    --lib \
    "$test_name"
done < <(jq -r '.vectors[].source_test' "$manifest" | sort -u)
