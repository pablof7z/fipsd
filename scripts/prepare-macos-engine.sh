#!/usr/bin/env bash
set -euo pipefail

repository="$(cd "$(dirname "$0")/.." && pwd)"
destination="$repository/FIPSDPackage/Sources/FIPSDFeature/Resources/bin"

cargo build --release -p fips-cli --bin fips-wind-tunnel \
  --manifest-path "$repository/Cargo.toml"
mkdir -p "$destination"
install -m 0755 "$repository/target/release/fips-wind-tunnel" \
  "$destination/fips-wind-tunnel"

echo "Prepared $destination/fips-wind-tunnel"
