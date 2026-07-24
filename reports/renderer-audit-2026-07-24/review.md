# FIPS Wind Tunnel renderer audit

Date: 2026-07-24

## Verdict

The user's impression is justified. The engine event artifacts are deterministic
and internally coherent, but the current visualization is not a faithful visual
explanation of those events.

The Root adoption, Connectivity, and Shared media views project real state, but
use an arbitrary layout and collapse many ordered events into one display tick.
The Cohorts view is materially misleading as an animation: a small semantic
change can reorder hundreds of nodes on screen because cohort columns are ranked
and rebuilt on every frame. Bright lines that appear and disappear are usually
in-flight messages, not connections, but they are drawn as complete endpoint
lines and are easy to misread as topology changes.

The product should currently describe these views as debug projections, not as
precise protocol animation.

## Post-audit remediation status

The findings below describe the renderer that produced the committed audit
artifacts. The subsequent renderer-truth work changes the claim and the
evidence boundary:

- stable synthetic world coordinates now survive node-set changes, filtering,
  and viewport resizing;
- physical links, parent relations, application routes, and in-flight
  transmissions are separate typed primitives, and transmissions use short
  moving trails rather than complete bright endpoint lines;
- sparse sub-16 ms events are presented one ordered event per display update;
  dense timestamp or playback-window bursts are explicitly and exactly
  summarized with all event IDs, virtual times, ordinals, causal parents, and
  causal entry events;
- individual and cohort views derive from the same deterministic `RenderFrame`;
  cohort identity is anchored to root/depth/transport rather than current rank,
  and aggregate progress uses every matching transmission;
- native `render-frames.v1.jsonl` evidence records source mappings, visible
  primitives, frame deltas, fidelity labels, and reconciliation violations.

The remediation does not claim topology-derived distance, continuously tweened
root or parent transitions, or individual animation for every member of a dense
summary. Those remain explicit presentation boundaries. The original audit
artifacts are retained and replayed as regression inputs.

## Audit inputs

### Observed 1,000-node run

- Run: `run-987fd1bc7b3884705ce272d7`
- Artifact: `artifact-987fd1bc7b3884705ce272d77c39794f`
- Scenario: the exact manually configured 1,000-node run visible in the app
- Engine events: 35,219
- Virtual duration: 10.38823455 seconds
- Display cadence reconstructed: 16 ms
- Display frames: 651
- Fidelity: individual, semantic-exact, executable-codec
- Engine outcome: failed root agreement and obsolete-root-retention assertions

Artifacts:

- `observed-1000/evidence/artifact.json`
- `observed-1000/evidence/report.json`
- `observed-1000/frames.jsonl.gz`
- `observed-1000/timeline.tsv`
- `observed-1000/summary.json`

### Pinned 12-node control

- Run: `run-7ea2f69dc506554c2736acd9`
- Artifact: `artifact-7ea2f69dc506554c2736acd9fa499b38`
- Scenario: `examples/m1/root-ratchet-12.yaml`
- Engine events: 283
- Virtual duration: 5.50103616 seconds
- Display frames: 345

Artifacts:

- `root-ratchet-12/evidence/artifact.json`
- `root-ratchet-12/evidence/report.json`
- `root-ratchet-12/frames.jsonl.gz`
- `root-ratchet-12/timeline.tsv`
- `root-ratchet-12/summary.json`

## What the app actually displays each tick

The playback loop advances virtual time by 16 ms and applies every event whose
timestamp is at or before the new cursor. SwiftUI receives only the state after
the whole batch has been applied. There is no transition for node, root, parent,
edge, or cohort changes. Only message-dot position is interpolated between a due
time and delivery time.

For each display frame the audit records:

- all engine events applied during that tick;
- active state, root, parent, and display position for every node;
- persistent physical edges and their active state;
- derived parent links;
- in-flight control, data, Bloom, lookup, and session transmissions;
- interpolated transmission-dot positions;
- cohort membership, cohort position, and the ranked major-root list;
- every visible addition, removal, state change, and layout-only movement.

## Findings

### 1. Cohort motion is mostly layout motion, not network motion

`CohortLayout` ranks the seven largest roots on every render, assigns each rank
to a column, then rebuilds every bucket. When root counts cross, whole columns
swap.

In the observed run:

- 366 of 651 frames moved nodes in Cohorts view.
- At 5.968 s, only 3 nodes changed semantic state, but 291 nodes moved to a
  different screen position.
- At 5.952 s the major-root order was
  `[994, 993, 8, 12, 14, 5, 26]`.
- One frame later it was `[993, 994, 8, 14, 12, 5, 26]`.
- That rank change moved or regrouped 291 nodes even though the protocol did not
  move 291 nodes.

Verdict: the cohort counts are derived from state, but spatial continuity is
false. A viewer cannot interpret bubble motion as protocol behavior.

### 2. Appearing and disappearing bright lines are usually messages

Every in-flight transmission draws a complete colored line from sender to
receiver plus a moving dot. Delivery removes the transmission and therefore the
whole line. The underlying physical edge is a separate, much fainter line.

In the observed run:

- 16,563 deliveries were scheduled and represented as transient bright lines.
- Up to 3,923 transient lines started in one display frame.
- The line disappearance normally means message delivery or expiry, not link
  disappearance.

In the 12-node control, network latency was shorter than the display cadence:

- 139 messages were scheduled.
- Only 33 survived long enough to be visible in a rendered frame.
- 106 messages, 76.3%, were created and delivered inside one 16 ms display tick
  and were never shown.

Verdict: endpoint and timing data come from the engine, but the encoding is
ambiguous and temporally incomplete.

### 3. Ordered protocol events are collapsed into visually atomic jumps

The deterministic trace has an exact total order, but the UI applies all events
inside each 16 ms window before rendering.

Examples from the observed run:

- Frame 0 applies 3,937 events, including 3,936 announcement sends.
- Frame 3, at 0.048 s, applies 1,733 deliveries and changes 470 node states.
- Frame 32, at 0.512 s, applies 3,189 events.
- A single frame can therefore replace hundreds of parents and roots while
  adding or removing thousands of message paths.

Verdict: the final state of each tick is mechanically derived from the trace,
but the animation does not show the causal sequence that produced it.

### 4. Individual-node positions do not represent topology

The individual views place sorted node IDs on a golden-angle disk. Position is
not derived from links, parents, transport, latency, coordinates, or any FIPS
property.

For the audited runs, the fixed-size individual layout did not move between
simulation frames because all future nodes were already present as inactive
nodes. However:

- changing the window or inspector width moves every node;
- switching to Anomalies recomputes positions using only the filtered subset;
- adding a node not predeclared in the initial topology changes the denominator
  and moves existing nodes.

Verdict: edges and node state are real, but geometry is decorative. Spatial
distance and motion must not be interpreted as network behavior.

### 5. Root and parent state is mostly projected correctly

For supported event types, the app applies `tree-announce.delivered` by updating
the receiver's advertised root, parent, and sequence. Root adoption colors use
that root and orange lines use that parent.

The observed 1,000-node run ends with 99 advertised root groups in the renderer.
That apparent chaos is consistent with the engine's failed root-agreement and
obsolete-root-retention assertions. It is not fabricated by the renderer.

Verdict: state mapping is credible, but presentation makes real non-convergence
indistinguishable from layout churn.

### 6. Cohort message animation is not deterministically selected

Cohort transmissions are grouped by source bucket, destination bucket, and
plane. The dot animation then uses `flights.first` from values originating in a
Swift dictionary. Dictionary iteration order is not a stable rendering
contract, especially as transmissions are inserted and removed.

Verdict: group counts are real, but the representative dot can follow an
arbitrary member flight. This conflicts with deterministic replay expectations.

### 7. Scale changes semantics without enough explanation

Runs with more than 500 nodes automatically switch to Cohorts after initial
topology. The view then:

- shows only seven roots as individually ranked columns;
- merges all remaining roots into an unlabeled remainder column;
- labels depth bands but not root identities;
- changes column identity whenever rank changes.

Verdict: aggregation is necessary at this scale, but this aggregation is not
stable or sufficiently labeled to be interpretable.

## Accuracy by visual element

| Visual element | Source accuracy | Temporal accuracy | Interpretability |
| --- | --- | --- | --- |
| Node active/root/parent state | Good for supported events | Batched at 16 ms | Poor without stable labels |
| Faint physical edges | Good | Changes on modeled link events | Too easy to confuse with messages |
| Orange parent links | Good | Teleports after event batches | No causal transition shown |
| Bright message lines | Correct endpoints | Incomplete for sub-frame traffic | Misleading as connection lines |
| Moving message dot | Linear due-to-delivery interpolation | Good when visible | Overwhelmed at scale |
| Individual node location | Not protocol-derived | Moves on layout resize/filter | Decorative |
| Cohort membership/count | Derived from current state | Recomputed every tick | Unstable |
| Cohort screen position | Rank-derived | Can move hundreds at once | Misleading |
| Cohort representative dot | Arbitrary sampled member | Dictionary-order dependent | Not reproducible enough |

## Required renderer changes at audit time

1. Give every node and cohort a stable visual identity and stable world
   coordinate across the run.
2. Separate physical links, parent relationships, routes, and message traffic
   into unmistakably different primitives with a visible legend.
3. Replace 16 ms state batching with an event-aware animation scheduler or an
   explicit "compressed events" representation.
4. Show when a frame summarizes many events, including counts and the causal
   initiating event.
5. Anchor cohort columns to root identity rather than current rank; move rank
   into a metric, not a coordinate.
6. Use deterministic aggregate flow animation instead of `flights.first`.
7. Preserve world coordinates when panels resize.
8. Add renderer-trace regression tests that reconcile every visible change to
   one or more engine events and separately label layout-only changes.

## Reproduction

```sh
node scripts/render-trace-audit.mjs \
  reports/renderer-audit-2026-07-24/observed-1000/evidence/artifact.json \
  reports/renderer-audit-2026-07-24/observed-1000 \
  800 600
```

The gzipped JSONL file is the detailed frame log. The TSV is the compact
one-row-per-display-frame index. Reproduction emits uncompressed JSONL, which
can be compressed with `gzip -9`.
