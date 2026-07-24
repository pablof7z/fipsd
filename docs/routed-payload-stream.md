# Routed synthetic payload stream

The individual-node engine schedules synthetic payload flows in the same
injected-time event queue as root changes and TreeAnnounce traffic. Each flow
resolves a stable shortest active path, then traverses that path one edge at a
time. The native workbench projects retained in-flight transmissions with
linear due-to-delivery progress. Sparse events are presented individually and
dense windows are explicitly summarized; no aggregate counter is presented as
an executed frame.

## Traffic processes

In addition to one-shot matrices, two temporal processes execute directly:

- `persistent-streams` treats `flow_count` as the number of streams and emits
  `segments_per_stream` separately queued payloads for each one. Segment offers
  are interleaved at the configured aggregate `rate_bps`; the first segment
  establishes the session, intermediate segments reuse it, and the final
  segment tears it down. Every event carries the stream ID and zero-based
  segment position.
- `bursty` emits `burst_size` flows at exactly the same virtual time, then
  advances by `burst_interval_ns`. A partial final burst retains its actual
  member count. Every event carries the burst and member position.

Both are seed-stable. They are temporal input models, not post-processing
labels: simultaneous offers and stream segments enter the primary scheduler,
compete with protocol control frames, and can be delayed, lost, or rejected by
the configured link and node limits.

## Ordered events

- `data.flow-offered` records source, destination, useful bytes, stream/burst
  lineage, and the full selected path.
- `data.frame-due` records one attempted hop, effective endpoint media, MTU,
  bandwidth, latency, queue occupancy, transmitted bytes, loss, and rejection.
- `data.frame-delivered` records the exact edge copy delivered, whether it is
  the copy selected to continue, and whether useful payload reached its final
  destination.

Event IDs and causal parents connect the offer to every hop and delivery. The
event stream is byte-for-byte the trace persisted in the run artifact.

## Shared bottlenecks

Control and data use the same per-edge `LinkService`. They therefore compete
for the same directional queue and configured bandwidth. The effective link
also applies the endpoint profiles described in
[`mixed-node-connectivity.md`](mixed-node-connectivity.md). A BLE endpoint can,
for example, reject a payload that crosses its effective MTU while another
Wi-Fi path succeeds.

The routed-traffic report reconciles offered flows to delivered plus rejected
flows and offered useful bytes to delivered plus lost useful bytes. Wire bytes
also reconcile through the shared per-edge counters. Root/tree quiescence and
payload quiescence remain separate measurements.

## Fidelity and present limits

TreeAnnounce frames retain executable-codec-derived sizing. Synthetic
session-data framing is currently semantically modeled with a declared
106-byte overhead, so any artifact containing routed payload declares overall
wire fidelity `modeled` and the approximation method
`routed-synthetic-session-data-v1`.

The primary individual scheduler accepts at most 100,000 routed flows per run.
Larger workloads must select a cohort or analytical engine rather than imply
that individual flows were executed. Random-mixed lookup campaigns resolve
coordinates and establish sessions on the same per-edge scheduler before
payload motion; see
[`graph-native-recovery.md`](graph-native-recovery.md). Uniform lookup campaigns
retain the separately labeled legacy M2 coupled model during migration.
