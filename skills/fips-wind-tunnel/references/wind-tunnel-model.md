# Wind Tunnel model

FIPS Wind Tunnel is a deterministic, multi-fidelity protocol experimentation
system. It is not a daemon launcher or a decorative graph.

Its product boundary includes a discrete-event engine, campaign generation and
search, shrinking and replay, causal cost accounting, protocol variants,
multi-scale visualization, artifacts, and a small real-daemon oracle.

## Fidelity

- Individual semantic simulation tracks nodes, links, events, routes, and
  modeled protocol operations.
- Cohort and analytical modes represent populations and distributions.
- Hybrid modes embed exact or individual regions inside aggregate populations.
- Real-daemon evidence is a separate validation backend.

Always report which behavior is exact, modeled, probabilistic, calibrated,
aggregated, sampled, or unsupported.

## Causality

Keep requested, performed, constructed, superseded, coalesced, serialized,
queued, transmitted, and delivered stages distinct. Follow causal parents from
the initiating event through state changes, control cost, resource pressure,
and application consequences.

## Visualization

Small runs use node-link state, animated message and payload movement, routes,
roots, parents, direct inspection, and exact timelines. Larger runs use
collapsed regions, depth bands, heatmaps, distributions, heavy hitters,
wavefronts, and anomaly samples. Massive runs must not pretend every node was
individually executed or drawn.

Read the product resources `docs/product-spec.md`, `docs/architecture.md`,
`docs/fidelity-and-provenance.md`, `docs/artifact-format.md`,
`docs/multi-scale-visualization.md`, and `docs/fips-seam-inventory.md` for the
complete checked-in contracts.
