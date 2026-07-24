# Graph-native lookup and session recovery

When a campaign uses `random-mixed` transport assignment and includes `lookup`
in `instrumentation.quiescence_markers`, useful traffic enters the primary
individual-node recovery path before payload forwarding.

## Causal sequence

1. `data.flow-offered` checks the source node's bounded coordinate cache.
2. A miss schedules `lookup.attempt-started`, followed by one
   `lookup.frame-due` request per selected path edge.
3. The destination constructs an executable-codec-sized response. The response
   traverses the exact reverse path through `lookup.frame-due` and
   `lookup.frame-delivered` events.
4. A successful response inserts the coordinate with its root generation and
   stable path. Root replacement, node disappearance, and path-node removal
   invalidate precisely scoped entries and disrupt affected sessions.
5. If no reusable session exists, `session-setup` crosses the forward path and
   `session-ack` crosses the reverse path. Only then is `session-data` handed
   to the routed payload stream.

Lookup TTL, attempt count, deterministic exponential backoff and jitter, cache
capacity and TTL, and per-node resource budgets come from the normalized
campaign. Cache hits, misses, expiry, eviction, invalidation, setup, ack, rekey,
teardown, CPU wait, and typed resource exhaustion remain separately counted.

## Shared mixed-profile links

Every lookup and session hop enters the same directional `LinkService` as
TreeAnnounce, Bloom, and payload traffic. The effective edge takes the
bottleneck bandwidth and MTU plus the combined latency, loss, queue, ordering,
and overhead of its endpoint profiles. A rejected event preserves the edge,
frame size, Wi-Fi/BLE/Tor/Ethernet identities, bandwidth, latency, MTU, and
retry outcome for inspection and animation.

Wire accounting reconciles transmitted recovery bytes to delivered plus lost
bytes. Logical lookup accounting reconciles lookups to success plus failure and
attempts to initial lookups plus retries. Root/tree, Bloom, lookup/session, and
payload quiescence are independent clocks.

## Fidelity boundary

Lookup request/response and session setup/ack message sizes are derived from
the pinned executable codecs. Routing and session state are semantically
modeled. Rekey currently consumes deterministic operation-counted work without
claiming a byte-executed wire frame. Artifacts declare
`graph-native-lookup-session-v1` with that uncertainty.

Uniform transport lookup campaigns continue to use the verified legacy M2
coupled recovery model during migration. They emit a separate recovery report;
the graph-native report is never silently substituted.
