# Shared-medium network zones

Individual-node campaigns may add `topology.media_zones`. A zone names a
non-overlapping set of stable node IDs and defines bandwidth, propagation
latency, loss, MTU, and queue capacity. The native workbench can partition the
configured population into deterministic round-robin zones.

Every graph edge whose endpoints are in the same zone uses the conservative
combination of endpoint and zone limits. More importantly, all such edges use
one half-duplex serialization domain and one queue. Concurrent control, Bloom,
lookup, session, and payload frames therefore delay or reject one another even
when they traverse different graph edges. Per-edge counters remain separate;
their queue occupancy records the shared medium pressure observed at enqueue.

Cross-zone edges retain endpoint-derived conditions because the current schema
models intra-zone media, not an implicit routed backbone. A node cannot belong
to more than one zone. Invalid membership, non-positive capacity, or an
out-of-range node fails before execution.

The event stream exposes `media_zone` on nodes and `shared_medium_group` on
edges. The workbench's Shared media view colors zone membership and emphasizes
contending edges. Individual runs provide exact deterministic enqueue order.
Cohort runs aggregate zone contention and carry the
`shared-media-zone-aggregation/v1` approximation label.
