# Interactive network interventions

The workbench can schedule an event 100 ms after the current timeline cursor
and immediately rerun the experiment. This is deterministic replay, not a UI
mutation hidden from the artifact. The resulting campaign, ordered event
stream, causal ledger, artifact, and reproduction bundle retain the event.

## Supported controls

- Add one identity lower than the root visible at its exact virtual time.
- Fail or recover a selected node.
- Isolate a selected node, or partition any campaign-authored node group.
- Merge the same cut back into the active graph.
- Change or restore an edge's bandwidth, latency, jitter, loss, MTU, and queue limit.

Partitions preserve stable node and edge IDs. Disabled edges are excluded from
parent selection, shortest-path routing, Bloom propagation, lookups, sessions,
and payload forwarding. The renderer shows them as red dashed links. A merge
causes both sides to announce current tree and Bloom state over each restored
edge, making the adoption wave inspectable.

Cached coordinate paths and established sessions are invalidated only when
their recorded path crosses a disabled edge. Overlapping partitions use an
edge block count, so one merge cannot accidentally reopen a link still held by
another cut.

Frames already serialized before a partition keep their scheduled delivery.
This models an in-flight packet rather than retroactive cancellation. Work due
after the cut is cancelled or rejected with its causal stage recorded. Link
condition changes likewise preserve existing queue history; they affect newly
enqueued work. These boundaries are semantic models, not claims that a real
transport stack was executed.
