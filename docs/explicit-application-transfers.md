# Explicit application transfers

`traffic.model: explicit-transfers` represents one or more concrete application
objects rather than a generated traffic matrix. Each transfer declares its
source, destination, total useful bytes, start time, and bounded visualization
chunk size.

The engine computes the stable shortest active path. An explicit three-node
chain with endpoints 0 and 2 therefore routes through node 1 without requiring
an authored `via` hint.

Each visible chunk is packetized against every traversed link's MTU. The engine
accounts for packet count, protocol and transport overhead, bandwidth
serialization, bounded queue occupancy, and projected reliable-stream
retransmissions. Chunks are offered over virtual time at `traffic.rate_bps`, so
link and topology interventions during a transfer affect the remaining bytes.
Individual packets are aggregated into byte-range events; the artifact labels
that approximation explicitly.

Routes are resolved when each chunk is offered. If an intermediate node leaves,
chunks already queued on that route are rejected with their lost useful bytes
and causal reason recorded; later chunks use the newly available shortest path.
This is transport-stream packetization and retransmission accounting, not yet
application-level resumption of bytes lost to a route disappearing.

The native workbench renders each in-flight chunk on its current hop and shows
transfer route, delivered bytes, total bytes, and percentage complete.

See `examples/three-node-file-transfer.yaml`.
