#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repository_root"

scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
package="$scratch/fips-wind-tunnel-0.1.0"
prefix="$scratch/prefix"

CARGO_TARGET_DIR="$scratch/cargo-target" scripts/package-release.sh "$package"
mkdir -p "$prefix/bin"
cp "$package/bin/fips-wind-tunnel" "$prefix/bin/"

"$prefix/bin/fips-wind-tunnel" validate examples/m1/root-ratchet-12.yaml
"$prefix/bin/fips-wind-tunnel" atlas verify "$package/evidence/qualification-atlas.json"
"$prefix/bin/fips-wind-tunnel" release verify-audit "$package/evidence/release-audit.json"
"$prefix/bin/fips-wind-tunnel" release verify-package \
  "$package" "$package/release-manifest.json"

rm "$prefix/bin/fips-wind-tunnel"
test ! -e "$prefix/bin/fips-wind-tunnel"
echo "clean install and uninstall passed"
