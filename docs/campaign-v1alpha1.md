# Campaign v1alpha1 contract

Campaign v1alpha1 is the canonical, engine-independent scenario document. It
describes experiment intent; Docker lifecycle, netem commands, process IDs, and
host paths are intentionally absent.

## Units and numeric boundaries

- `seed`, node/edge/case counts, rates, byte counts, operation budgets, and
  table capacities are non-negative JSON-compatible integers.
- Durations are a whole non-negative integer followed by exactly one of `ns`,
  `us`, `ms`, `s`, `m`, or `h`. Decimal durations and whitespace are invalid.
- Normalization converts all durations to checked integer nanoseconds. Overflow
  fails normalization.
- Loss and duplication use integer parts per million in `0..=1_000_000`.
- Transport MTU profile values are `68..=65_535` bytes.
- A selector is either one explicit value or `{ values: [...] }`. Value sets are
  non-empty, duplicate-free, and normalized into a canonical sorted order.

## Defaults

Defaults are materialized in the normalized plan:

- `engine.variant`: `fips-80c956a-baseline`;
- `topology.connected`: `true`.

`engine.deterministic` is required and must be `true`; it is not an implicit
fallback. Omitted optional sections mean that capability is not requested or
instrumented. They do not silently acquire engine-specific behavior.

## Unknown fields and extensions

Every named contract object uses `additionalProperties: false`. A misspelled or
new field therefore fails with its instance path. Extensible protocol,
topology, traffic, event, and scenario data belongs only in explicitly named
`parameters`/`overrides` maps; keys must begin with a lowercase letter and may
contain lowercase letters, digits, `_`, `.`, and `-`.

Schema evolution adds a new API/schema version. Readers do not discard fields
from a newer version.

## Fidelity guardrails

A campaign that includes a one-billion-node case must declare
`cohort-with-sampled-exact-regions`. The validator rejects an individual or
unlabeled representation. Run artifacts apply stricter cross-field checks for
production codec pins, calibrated hardware profiles, approximation metadata,
and sampled exact regions.

## Per-node connectivity

`transports.assignment: random-mixed` assigns profiles to stable node IDs with
a seed-stable weighted draw. Profile `weight` values define the distribution;
zero-weight profiles are retained in the campaign but never assigned. Each
profile may override `bandwidth_bps`, `latency`, `jitter`, `loss_ppm`, `mtu_bytes`, and
`queue_bytes` for its endpoint. Supported media families are `udp`, `tcp`,
`ethernet`, `wifi`, `ble`, `tor`, and `nym`.

The effective symmetric edge conditions are derived from the shared link and
both endpoints. See [mixed-node-connectivity.md](mixed-node-connectivity.md)
for the deterministic formulas and fidelity boundary.

`topology.media_zones` optionally assigns non-overlapping node sets to authored
shared media. Every intra-zone edge uses the zone's bandwidth, latency, loss,
MTU, and queue bounds and contends on one deterministic half-duplex
serialization queue. Cross-zone edges retain endpoint-derived link conditions.
Cohort runs aggregate this contention and declare that approximation; only
individual runs claim exact enqueue order.

## Temporal traffic parameters

`traffic.model: persistent-streams` uses `parameters.flow_count` as the stream
count and requires a positive `parameters.segments_per_stream`. `payload_bytes`
is the size of each segment. The individual engine caps the product of streams
and segments at 100,000 routed offers.

`traffic.model: bursty` requires positive `parameters.burst_size` and
`parameters.burst_interval_ns`. Members of a burst are offered at the same
virtual timestamp; consecutive bursts are separated by that explicit interval.
All generated offers retain their process lineage in the durable event trace.

`traffic.model: explicit-transfers` uses a `transfers` array instead of a
generated endpoint matrix. Every entry declares `id`, `source`, `destination`,
`total_bytes`, optional `visualization_chunk_bytes`, and optional `start`.
Endpoints are stable numeric node IDs. The individual engine computes the
active route and caps the total number of visible chunks at 100,000. Each chunk
is packetized per traversed link MTU and retains its byte range in the event
trace. See
[explicit-application-transfers.md](explicit-application-transfers.md).

## Interactive and authored interventions

The compact discrete-event engine accepts these replayable event actions:

- `introduce-lower-root-node` reserves one stable node slot and activates it at
  `at` with an address below the then-visible minimum root.
- `disappear-node` and `reappear-node` use an integer node `target`.
- `partition-network` and `merge-network` use `parameters.nodes` to name one
  side of a cut. All crossing edges change availability together.
- `set-link-conditions` and `restore-link-conditions` use an integer edge
  `target`. Overrides may include `bandwidth_bps`, `latency`, `jitter`, `loss_ppm`,
  `mtu_bytes`, and `queue_bytes`.
- `synchronized-session-rekey` snapshots all live sessions at `at`, charges
  deterministic cryptographic work to each source, and preserves a causal
  completion or supersession for every accepted operation.
- `expire-coordinate-cache` invalidates every live coordinate cache entry at
  `at`. A same-time `simultaneous-lookups` event runs afterward, using
  `parameters.count` replayable endpoint probes from the configured traffic
  population.
- `fail-transport-class` and `restore-transport-class` target one authored
  profile name. Every incident edge participates in the same deterministic
  availability transition while overlapping partitions retain their own block.
- `swap-parent-ancestry` changes one eligible same-root parent decision using
  authored fixed-point MMP costs. `alternate-parent-quality` repeats that
  re-evaluation with bounded `cycles` and an explicit `interval`; optional
  `target` selects the node, otherwise stable node order chooses it.
- `attach-authenticated-sybils` requires
  `adversaries.mode: authenticated-protocol-valid`, an identity budget, and an
  operation budget. Each reserved identity activates as an individual node at
  the authored cadence and attachment policy; `address_policy` is
  `uniform-valid` or `lower-than-current-root`.

The native workbench authors the same campaign events used by CLI and CI. See
[interactive-interventions.md](interactive-interventions.md) for runtime and
in-flight delivery semantics and [session-rekey-waves.md](session-rekey-waves.md)
for the rekey fidelity boundary. Unsupported or misspelled actions fail with
their exact `/events/N` path instead of silently disappearing.

## Coverage

The normative Root Ratchet document plus the nine files under
`examples/campaigns/` cover the ten flagship families. They prove schema
representability only; M1–M7 own engine behavior, search, shrinking, daemon
reproduction, and campaign qualification.
