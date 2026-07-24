# Natural-language run amendments

The native workbench supports two distinct prompt operations:

- **Start new** authors and validates a complete Campaign.
- **Change current run** authors a forward-only amendment at the current
  virtual-time cursor.

Before a follow-up prompt is sent to the selected local model, the workbench
captures a semantic snapshot of what is currently rendered:

- current virtual time and event cursor;
- active and inactive node IDs, human labels, addresses, roots, parents, and
  join times;
- exact edge IDs and current bandwidth, latency, jitter, loss, MTU, and queue
  conditions;
- active application transfers, routes, byte totals, and progress;
- the last rendered event;
- already-scheduled campaign events; and
- the complete active Campaign.

The model does not receive authority to rewrite the Campaign. It returns a
bounded amendment containing new events, optional future-event cancellations,
and an optional request to stop not-yet-realized scheduled arrivals. Every new
event must use a supported intervention action and occur after the supplied
cursor time.

The host applies the amendment, schema-validates the resulting Campaign, and
deterministically replays its immutable past to the same cursor before
continuing. This is currently a replay-backed branch, not an engine checkpoint.
The rendered result remains smooth, while the artifact retains the prompt,
rendered-state context, amended Campaign, seed, and ordered causal trace.

For example, after a run reaches 15 active nodes, a follow-up such as:

> Stop new arrivals, then remove the oldest active node every five seconds
> until four nodes remain.

can be compiled into a stopped future arrival schedule plus explicit
`disappear-node` events targeting the oldest active numeric IDs from the
snapshot.

An arriving ordinary node uses `introduce-node` with an explicit attachment
list. This allows a follow-up such as:

> Add a new node joining the two endpoints of the active transfer, then remove
> the old bridge ten seconds later.

to add both dormant endpoint links, activate them when the node arrives, and
remove the intermediate node at the requested later virtual time. Transfer
chunks offered after that removal are routed over the replacement bridge.
`introduce-lower-root-node` is reserved for arrivals that must become root.

Model output remains untrusted. Unsupported actions, duplicate IDs, events
before the cursor, excessive event counts, and invalid Campaigns are rejected.
