# M4 honest cohort/hybrid scale and protocol variants

M4 adds a separate analytical engine instead of teaching the individual engine
to pretend that aggregate state is exact. Both consume normalized plans and
emit immutable run artifacts with the same root, tree, control, Bloom, queue,
useful-payload, and quiescence projections.

The checked billion-node Root Ratchet allocates at most 64 cohort records and
one 16-node exact anomaly region. Cohorts are keyed by bounded depth range,
degree, transport, resource class, region, and protocol state. Population mass
is conserved through every transition. Decimal estimates can exceed `u64` and
always include method, assumptions, validation range, and uncertainty bounds.

## Reproduce the demo

```bash
cargo run -p fips-cli --bin fips-wind-tunnel -- scale run \
  examples/m4/billion-root-ratchet.yaml --output /tmp/m4-run

cargo run -p fips-cli --bin fips-wind-tunnel -- scale compare \
  examples/m4/billion-root-ratchet.yaml --output /tmp/m4-variants.json

cargo run -p fips-cli --bin fips-wind-tunnel -- scale billion-demo \
  examples/m4/billion-root-ratchet.yaml --output /tmp/m4-billion.json
```

The demo runs two topology structures, three arrival cadences, and three
variants: pinned current FIPS, an experimental root-tenure dampener, and an
experimental incremental Bloom delta. Differential results attribute cost and
state changes to the specific variant decision. Experimental variants are
comparison proposals, never upstream recommendations.

## Fidelity boundaries

- Cohort Bloom uses analytical occupancy/FPR distributions; sampled regions
  retain exact bit indices and explicit boundary accounting.
- Root-spine, bottleneck-cut, selected-subtree, and anomaly-driven policies can
  instantiate standalone exact reproduction bundles.
- Execute, operation-count, calibrated-cost, unbounded, and adversarial-budget
  crypto modes retain the same semantic outcome digest. Executed crypto is
  rejected above 10,000 represented nodes.
- Calibration publishes full matched error distributions across 8-64 nodes for
  every headline metric, plus machine-readable warnings outside envelopes.
- No individual-node claim applies outside explicitly named sampled regions.
