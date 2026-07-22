#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
checkout="${1:-}"
output="${2:-}"
repeats="${FIPSD_M5_REPEATS:-3}"

if [ -z "$checkout" ] || [ ! -d "$checkout/.git" ]; then
  echo "usage: scripts/check-m5-daemon.sh <pinned-fips-checkout> [output-directory]" >&2
  exit 2
fi

expected_commit="80c956a6fdb85dde1450969a21891c1158e43267"
actual_commit="$(git -C "$checkout" rev-parse HEAD)"
if [ "$actual_commit" != "$expected_commit" ]; then
  echo "expected FIPS $expected_commit, got $actual_commit" >&2
  exit 1
fi
if [ ! -x "$checkout/testing/docker/fips" ]; then
  echo "missing Linux daemon binary; run the pinned testing/scripts/build.sh first" >&2
  exit 1
fi
docker info >/dev/null
python3 -c 'import jinja2, yaml'
if [ -z "$output" ]; then
  output="$(mktemp -d /tmp/fipsd-m5-daemon.XXXXXX)"
fi

cd "$repository_root"
cargo run --quiet -p fips-cli --bin fips-wind-tunnel -- oracle run-chaos \
  fixtures/m5/chaos/smoke.yaml --fips-checkout "$checkout" --repeats "$repeats" \
  --duration-seconds 10 --output "$output"

report="$output/oracle-report.json"
jq -e --arg commit "$expected_commit" '
  .backend == "pinned-fips-docker-chaos" and
  .stable == true and
  .dominant_classification == "unsupported-observation" and
  .dominant_confidence_ppm == 1000000 and
  ([.repeats[].exit_code] | all(. == 0)) and
  ([.attached_daemon_evidence[].telemetry.nodes | length] | all(. == 10)) and
  ([.attached_daemon_evidence[].provenance.fips_commit] | all(. == $commit)) and
  ([.attached_daemon_evidence[].provenance.binary_sha256] | all(length == 64)) and
  ([.attached_daemon_evidence[].provenance.image_digest] | all(startswith("sha256:")))
' "$report" >/dev/null

echo "M5 live daemon evidence: $report"
