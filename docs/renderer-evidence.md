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
  individual transmissions, source-timed node pulses, imported analytical
  cohorts, deterministic in-memory cohorts, and aggregate transmissions;
- the selected visualization mode and node selection used by presentation-only
  rings, plus the exact independently checked anomaly-node filter;
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

`NetworkCanvas` and its drawing helpers accept only `RenderFrame`. They do not
accept `SimulationState`, `CohortArtifactState`, or analysis data, and a source
boundary test rejects reintroduction of those inputs. View and selection changes
publish a distinct `view-change` evidence frame with no simulation events.

The layout is stable and deterministic but synthetic. Pixel distance is never a
protocol metric. Cohorts are a declared root × depth-band × transport
aggregation, and their flight progress is the deterministic mean over every
matching retained transmission.

The independent renderer oracle reads raw artifact JSON and intentionally does
not use `SimulationEvent`, `SimulationState`, `RenderFrame`,
`RenderSourceProjection`, or `CohortProjection`. It replays both committed
renderer-audit artifacts and compares the semantic marks emitted in all five
visualization modes at every scheduled display update. Separate cases cover
rekey, parent-switch, authenticated-Sybil, shared-medium, selection, and
billion-node analytical-cohort marks.

Run the focused proof with:

```sh
xcodebuildmcp swift-package test \
  --package-path FIPSDPackage \
  --filter Independent
```

This proves the source-to-semantic-mark boundary for the committed inputs. It
does not claim that synthetic coordinates are network geometry, that compressed
bursts had unrendered intermediate frames, or that a pixel raster is a formal
proof of the engine.

The schema is `schemas/render-frame-v1alpha1.schema.json`.
