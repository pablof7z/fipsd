# M5 pinned-daemon oracle

M5 closes the model-to-implementation loop without pretending that model state
and daemon telemetry are identical surfaces. It imports existing FIPS chaos
scenarios, compiles the representable Campaign subset back to the pinned Docker
harness, runs real daemons, normalizes observed evidence, and classifies the
differences.

The adapter is pinned to FIPS commit
`80c956a6fdb85dde1450969a21891c1158e43267`. Every comparison requires the full
commit, binary hash and version, Docker image ID, generated-config hashes,
adapter version, runtime version, host profile, and a patch digest when the
checkout is dirty. A missing required field refuses comparison.

## Honest comparison boundary

The importer emits a diagnostic for every exact, approximated, metadata-only,
or unsupported mapping. Random geometric chaos topology is represented by the
individual engine's seeded random-regular topology and compiles back to the
harness's random-geometric topology. Stochastic ranges collapse to deterministic
midpoints; the complete source YAML and its hash remain attached.

The six checked inputs are byte-for-byte copies of pinned upstream
`smoke-10`, `cost-stability`, `mixed-technology`, `congestion-stress`,
`churn-mixed`, and `bloom-storm`. Their shorter local names only group the
acceptance families.

The compiler rejects cohort/hybrid scale, more than 256 exact daemons, dynamic
identity arrivals, and topology features the pinned harness cannot express. It
never silently drops those semantics.

Telemetry uses `observed`, `sampled`, and `unknown` values with raw-source IDs
and sampling windows. Unknown values remain absent, never zero. Frame equality
requires executable-codec model evidence and captured-wire daemon evidence.
The differential report records the first divergent transition and classifies
semantic, frame, metric, timing, environmental, and unsupported-observation
outcomes.

## Commands

Import or inspect deterministic suite artifacts:

```bash
cargo run -p fips-cli --bin fips-wind-tunnel -- oracle import \
  fixtures/m5/chaos/smoke.yaml --output import.json

cargo run -p fips-cli --bin fips-wind-tunnel -- oracle compile \
  import.json --output compiled-smoke.yaml

cargo run -p fips-cli --bin fips-wind-tunnel -- oracle suites \
  --output oracle-suites.json
```

Run the pinned live gate after building Linux FIPS binaries and installing the
chaos harness's Python dependencies (`PyYAML` and `Jinja2`):

```bash
scripts/check-m5-daemon.sh /path/to/pinned/fips /path/to/evidence
```

That command imports, models, compiles, runs three daemon repeats, ingests final
control snapshots, compares evidence, and checks provenance. `scripts/check.sh`
keeps the normal PR gate Docker-independent by regenerating all recorded M5
fixtures byte-for-byte. Recorded fixtures say `recorded-fixture-not-live-daemon`
and are never promoted as daemon confirmation.

Malformed-wire fuzzing remains a separate adapter. Crash and hang results carry
standalone replay bytes, backend and coverage provenance, and cannot enter the
semantic simulator.

## Verified live smoke

The checked [live smoke summary](../fixtures/m5/live-smoke-summary.json) records
the 2026-07-22 arm64 Docker run. All three repeats exited zero, ingested all ten
tree snapshots, and matched the root-agreement invariant. The overall result is
`unsupported-observation` at 1,000,000 ppm confidence because the daemon does
not expose every model metric. That classification is intentional: unavailable
evidence is not called agreement.
