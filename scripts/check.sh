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
m3_run="$(mktemp -d)"
m4_run="$(mktemp -d)"
m5_run="$(mktemp -d)"
trap 'rm -f "$first" "$second"; rm -rf "$m1_run" "$m2_run" "$m3_run" "$m4_run" "$m5_run"' EXIT
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

cargo run --quiet -p fips-cli --bin fips-wind-tunnel -- campaign plan \
  examples/m3/root-ratchet-search.yaml --mode covering --strength 2 \
  --output "$m3_run/plan.json"
cmp "$m3_run/plan.json" fixtures/m3/covering-plan.json
cargo run --quiet -p fips-cli --bin fips-wind-tunnel -- campaign search \
  "$m3_run/plan.json" --maximum-evaluations 6 --output "$m3_run/search.json"
cmp "$m3_run/search.json" fixtures/m3/search-result.json
cargo run --quiet -p fips-cli --bin fips-wind-tunnel -- campaign execute \
  "$m3_run/plan.json" --workers 2 --maximum-cases 3 \
  --checkpoint "$m3_run/checkpoint.json" --output "$m3_run/partial.json"
cargo run --quiet -p fips-cli --bin fips-wind-tunnel -- campaign execute \
  "$m3_run/plan.json" --workers 3 --maximum-cases 6 \
  --checkpoint "$m3_run/checkpoint.json" --output "$m3_run/resumed.json"
cargo run --quiet -p fips-cli --bin fips-wind-tunnel -- campaign replay-corpus \
  fixtures/corpus --output "$m3_run/corpus-report.json"
cmp "$m3_run/corpus-report.json" fixtures/m3/corpus-report.json

cargo run --quiet -p fips-cli --bin fips-wind-tunnel -- scale run \
  examples/m4/billion-root-ratchet.yaml --output "$m4_run/cohort"
cmp "$m4_run/cohort/artifact.json" fixtures/m4/billion-cohort-artifact.json
cmp "$m4_run/cohort/report.json" fixtures/m4/billion-cohort-report.json
cargo run --quiet -p fips-cli --bin fips-wind-tunnel -- scale compare \
  examples/m4/billion-root-ratchet.yaml --output "$m4_run/variants.json"
cmp "$m4_run/variants.json" fixtures/m4/variant-comparison.json
cargo run --quiet -p fips-cli --bin fips-wind-tunnel -- scale calibrate \
  examples/m4/billion-root-ratchet.yaml --output "$m4_run/calibration.json"
cmp "$m4_run/calibration.json" fixtures/m4/calibration.json
cargo run --quiet -p fips-cli --bin fips-wind-tunnel -- scale billion-demo \
  examples/m4/billion-root-ratchet.yaml --output "$m4_run/billion.json"
cmp "$m4_run/billion.json" fixtures/m4/billion-demo.json

cargo run --quiet -p fips-oracle --example generate_m5_fixtures -- "$m5_run/generated"
diff -ru fixtures/m5/generated "$m5_run/generated"
cargo run --quiet -p fips-cli --bin fips-wind-tunnel -- oracle import \
  fixtures/m5/chaos/smoke.yaml --output "$m5_run/imported-smoke.json"
cmp "$m5_run/imported-smoke.json" fixtures/m5/generated/imported-smoke.json
cargo run --quiet -p fips-cli --bin fips-wind-tunnel -- oracle compile \
  "$m5_run/imported-smoke.json" --output "$m5_run/compiled-smoke.yaml"
cmp "$m5_run/compiled-smoke.yaml" fixtures/m5/generated/compiled-smoke.yaml
cmp "$m5_run/compiled-smoke.manifest.json" \
  fixtures/m5/generated/compiled-smoke-manifest.json
cargo run --quiet -p fips-cli --bin fips-wind-tunnel -- oracle suites \
  --output "$m5_run/suites.json"
cmp "$m5_run/suites.json" fixtures/m5/generated/suites.json
cargo run --quiet -p fips-cli --bin fips-wind-tunnel -- oracle fuzz-result \
  --backend cargo-fuzz --outcome crash --input-hex ff00 \
  --corpus-sha256 corpus-v1 --coverage-edges 42 \
  --output "$m5_run/fuzz-crash.json"
cmp "$m5_run/fuzz-crash.json" fixtures/m5/generated/fuzz-crash.json
