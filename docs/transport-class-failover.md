# Transport-class failure and restoration

`fail-transport-class` targets one authored per-node connectivity profile such
as `tor`, `bluetooth`, or `wifi`. At its virtual timestamp, every edge incident
to an assigned node in that class becomes unavailable in stable edge-ID order.
`restore-transport-class` removes that failure block later.

Availability blocks compose. An edge affected by both a network partition and a
transport-class failure does not reactivate until both causes are restored.
Each transition records its assigned nodes and the edges whose visible state
actually changed.

Failed edges leave the normal routing graph. Parent/root repair, Bloom peer
state, coordinate-cache invalidation, session disruption, and later synthetic
payload routing all use that updated graph. Restoration requests fresh tree and
Bloom exchange on every reactivated edge.

The class assignment and failure semantics are exact configured inputs.
Bandwidth, latency, jitter, loss, MTU, and queue values remain modeled rather
than measurements of a real transport implementation.

The native workbench exposes profile selection plus fail and restore buttons at
the timeline cursor. Changed edges dim immediately during replay and the
inspector counts currently failed classes.
