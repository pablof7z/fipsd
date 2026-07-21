#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repository_root"

scripts/check-loc.sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run --quiet -p fips-cli --bin fips-wind-tunnel -- validate examples/root-ratchet.yaml

first="$(mktemp)"
second="$(mktemp)"
m1_run="$(mktemp -d)"
m2_run="$(mktemp -d)"
trap 'rm -f "$first" "$second"; rm -rf "$m1_run" "$m2_run"' EXIT
cargo run --quiet -p fips-cli --bin fips-wind-tunnel -- normalize examples/root-ratchet.yaml --output "$first"
cargo run --quiet -p fips-cli --bin fips-wind-tunnel -- normalize examples/root-ratchet.yaml --output "$second"
cmp "$first" "$second"
cmp "$first" fixtures/normalized/root-ratchet.json

cargo run --quiet -p fips-cli --bin fips-wind-tunnel -- \
  run examples/m1/root-ratchet-12.yaml --output "$m1_run"
cargo run --quiet -p fips-cli --bin fips-wind-tunnel -- \
  replay "$m1_run/reproduction.json" --output "$m1_run/replay.json"
cmp "$m1_run/artifact.json" "$m1_run/replay.json"

if cargo run --quiet -p fips-cli --bin fips-wind-tunnel -- \
  run examples/m1/root-ratchet-12-broken.yaml --output "$m1_run/broken"; then
  echo "broken M1 fixture unexpectedly succeeded" >&2
  exit 1
fi

cargo run --quiet -p fips-cli --bin fips-wind-tunnel -- \
  run examples/m2/root-ratchet-recovery.yaml --output "$m2_run"
cargo run --quiet -p fips-cli --bin fips-wind-tunnel -- \
  inspect "$m2_run/artifact.json" --causal-id input:arrival-0000 \
  > "$m2_run/inspection.json"
cargo run --quiet -p fips-cli --bin fips-wind-tunnel -- \
  replay "$m2_run/reproduction.json" --output "$m2_run/replay.json"
cmp "$m2_run/artifact.json" "$m2_run/replay.json"
