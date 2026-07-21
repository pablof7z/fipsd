#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repository_root"

cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run --quiet -p fips-cli --bin fips-wind-tunnel -- validate examples/root-ratchet.yaml

first="$(mktemp)"
second="$(mktemp)"
trap 'rm -f "$first" "$second"' EXIT
cargo run --quiet -p fips-cli --bin fips-wind-tunnel -- normalize examples/root-ratchet.yaml --output "$first"
cargo run --quiet -p fips-cli --bin fips-wind-tunnel -- normalize examples/root-ratchet.yaml --output "$second"
cmp "$first" "$second"
cmp "$first" fixtures/normalized/root-ratchet.json

