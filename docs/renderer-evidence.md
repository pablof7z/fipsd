# Renderer evidence

The native workbench writes `render-frames.v1.jsonl` beside fresh run evidence.
Imported artifacts are replayed into a temporary renderer-evidence directory so
the selected source remains read-only.

Each line conforms to
`experiments.fips.network/render-frame/v1alpha1` and records:

- the ordered events crossed by the display update, including virtual time,
  ordinal, causal parent, causal entry events, and any exact-summary reason;
- the declared or completed source fidelity plus the renderer's temporal, layout, visible-state,
  and cohort projection boundaries;
- visible nodes, physical links, parent relations, application routes,
  individual transmissions, cohorts, and deterministic aggregate transmissions;
- a source-state path for every primitive;
- reconciliation counts and intentionally omitted mass;
- frame-to-frame state, relation, transmission, cohort, and layout-only deltas;
- violations for malformed sources, unattributed structural changes,
  non-total event order, or layout motion without a source-state change.

Sparse events inside a 16 ms wall-clock interval are presented one ordered event
at a time. When more than eight events are already due, the renderer applies
them in source order and emits one exact-summary frame containing every event.
That frame claims the final retained state and the complete event list, not
unobserved intermediate visual states.

The layout is stable and deterministic but synthetic. Pixel distance is never a
protocol metric. Cohorts are a declared root × depth-band × transport
aggregation, and their flight progress is the deterministic mean over every
matching retained transmission.

Regression coverage replays both committed renderer-audit artifacts:

```sh
xcodebuildmcp swift-package test \
  --package-path FIPSDPackage \
  --filter RenderFrameEvidenceTests
```

The schema is `schemas/render-frame-v1alpha1.schema.json`.
