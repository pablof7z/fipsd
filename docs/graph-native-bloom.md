# Graph-native Bloom propagation

Bloom-only recovery experiments run inside the primary individual-node
scheduler. They no longer collapse every FilterAnnounce onto one synthetic
shared link.

Each node owns its local identity filter and retains the latest filter received
from every active neighbor. An outgoing replacement unions the local filter
with every peer view except the destination peer. This split-horizon rule is
evaluated at the event's virtual time, so tree changes, joins, disappearance,
reappearance, debounce, and earlier deliveries affect the next replacement.

## Observable events

- `bloom.filter-due` records the exact established-FMP frame size, occupied
  bits, estimated cardinality, FPR, peer role, effective edge media, bandwidth,
  latency, MTU, queue occupancy, loss, and rejection reason.
- `bloom.filter-delivered` records the delivered copy and whether it changed
  the receiver's peer view. A fresh view causally requests replacements toward
  the receiver's other peers.

TreeAnnounce, Bloom, and payload frames enter the same directional
`LinkService`. A 1,071-byte FilterAnnounce may therefore cross a Wi-Fi edge,
queue behind payload, be lost on a configured link, or be rejected by a
244-byte BLE MTU. The event trace rendered by the workbench is the trace stored
in the artifact.

Exact-bit and sparse-bit modes preserve their declared representations.
Occupancy mode is explicitly labeled `seeded-bloom-occupancy-v1`. Every run
reconciles requests, coalescing, construction, sends, rejection, transmitted
wire bytes, delivery, and loss.

## Present boundary

Adding `bloom` to `instrumentation.quiescence_markers` selects this graph-native
path. It can be combined with `lookup` when transport assignment is
`random-mixed`; Bloom, lookup, session, tree, and payload frames then share the
same graph edges and queues. Uniform lookup campaigns retain the verified M2
coupled model during migration and remain explicitly represented by a separate
recovery report.
