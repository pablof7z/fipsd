# Product capability audit

This document maps the product specification to executable behavior. `Exact`
means the repository has a test or runnable artifact for that behavior. `Modeled`
means the behavior executes but carries an approximation or operation-count
label. `Partial` names the missing boundary. A qualification-atlas row is not
treated as executable evidence unless it actually invokes an engine.

## Experiment authoring and execution

| Capability | State | Executable evidence and boundary |
| --- | --- | --- |
| Declarative campaigns and deterministic normalization | Exact | `fips-model`, the Campaign schema, canonical hashes, and CLI validate/normalize commands |
| Virtual time and stable total event order | Exact | individual engine scheduler, replay fixtures, and worker-count campaign tests |
| Natural-language authoring | Exact at the trust boundary | Native Claude/Codex adapters emit only a Campaign; schema and semantic validation run before execution |
| Interactive controls and manual input events | Exact for supported actions | Native controls create new campaigns for lower-root arrival, failure/recovery, partition/merge, link changes, transport outages, parent instability, rekey/lookup waves, and authenticated Sybils |
| Saved artifact replay | Exact | Native replay and CLI reproduction replay consume the persisted ordered trace/bundle |
| Extensible protocol action vocabulary | Exact boundary | Supported actions execute through typed inputs; unknown actions fail with an exact `/events/N` path instead of becoming decorative metadata |

## Protocol and network behavior

| Capability | State | Executable evidence and boundary |
| --- | --- | --- |
| Identity, root, parent, tree, joins, failure, and recovery | Exact semantic model | `fips-engine` graph and event tests include cost-aware same-root parent changes, ancestry propagation, hysteresis, and hold-down; wire claims remain limited to verified codecs |
| Partition and merge | Exact semantic model | Explicit input events mutate edges and disrupt affected sessions |
| Bloom construction, split-horizon replacement, propagation, and FPR | Exact bits for individual runs; modeled for cohorts | Graph-native Bloom tests and fidelity metadata |
| Coordinates, lookup, cache, and sessions | Exact semantic model with executable-codec setup/lookup sizes | Recovery tests cover path hops, global cache expiry, simultaneous lookup waves, retry, reuse, teardown, and disruption |
| Rekey and crypto | Operation-counted | Synchronized waves snapshot live sessions, consume per-session hash resources, and retain causal completion or supersession; session crypto and replay-window wire behavior are not byte-executed |
| Routed useful payload | Exact route and queue model | Every hop, queue, loss, rejection, forwarding action, and useful delivery is an ordered event |
| Latency, jitter, loss, duplication, bandwidth, MTU, order, and queues | Exact configured integer model | Base links and per-node profiles author independent deterministic jitter bounds |
| Per-node mixed connectivity | Exact configured integer model | Explicit or seed-weighted Wi-Fi, BLE, Tor, and Ethernet profiles retain assignments and effective edge bottlenecks; class-wide failure/restoration reroutes topology and disrupts affected state |
| Network zones and shared media | Exact individual model | Intra-zone edges share one half-duplex serialization/queue domain; cohort runs label aggregation |
| CPU, memory, cache, table, session, and queue limits | Mixed | Limits and operation receipts execute; wall-clock CPU and allocator RSS require calibration and remain labeled |

## Traffic, campaigns, and failures

| Capability | State | Executable evidence and boundary |
| --- | --- | --- |
| Uniform, permutation, all-to-all, Zipf, incast, fanout, elephant/mice, cross-cut, session churn, and MTU sweep traffic | Exact deterministic generator | `traffic.rs` seed-stability and per-model tests |
| Long-lived segmented streams and explicit burst-process models | Exact deterministic generator | Persistent streams emit session-bounded, interleaved segments; burst processes emit simultaneous members at explicit virtual-time intervals with durable lineage |
| Cartesian, t-wise, and Monte Carlo planning | Exact | `fips-campaign` planners and byte-stable fixtures |
| Property and stochastic generation | Exact within declared generator bounds | Seeded generated topology/event inputs are replayable |
| Authenticated adversary budgets | Exact policy model plus executable admission | Accepted/rejected work and attacker spend are distinct; bounded Sybil identities join as individual rendered nodes with their real transport profiles and Tree/Bloom consequences; selective visibility and withheld forwarding remain labeled policy assumptions; malformed-wire fuzzing remains in the oracle subsystem |
| Objective search and resume | Exact for exported metrics | Native/CLI search currently ranks amplification, goodput stall, and starvation |
| Hierarchical shrinking and corpus promotion | Exact for supported predicates | Node/traffic/event/timing/transport reductions are replayed against the selected predicate |

## Scale, variants, visualization, and validation

| Capability | State | Executable evidence and boundary |
| --- | --- | --- |
| Individual discrete-event simulation | Exact semantic model | Small through bounded large individual-node campaigns |
| Cohort analytical and hybrid sampled-exact modes | Modeled | Deterministic bounds and exact sample regions are explicit in artifacts |
| Billion-node exploration | Modeled | Native sensitivity matrix uses cohorts and never claims one billion executed nodes |
| Exhaustive tiny-state exploration | Exact within declared finite bounds | Every ordering of supported authored actions is executed by the individual engine; coverage counts and replayable counterexamples are retained |
| Protocol variants | Supported versioned hooks | Baseline, cohort root dampening, cohort Bloom delta, and individual timer/parent-policy parameters compare under common seed/topology/traffic; unknown or mixed variant IDs are rejected |
| Small-network visualization | Exact projection of artifact events | Node-link animation, root adoption, connectivity, shared media, timeline, node/link conditions, and causal event inspection |
| Large-network visualization | Supported multi-scale projection | Cohorts, depth/transport layout, sensitivity bounds, root wavefronts, queue histograms, latency and Bloom-FPR percentiles, congestion groups, heavy hitters, causal flame graphs, and anomaly-focused heavy-link samples derive from retained evidence |
| Real-daemon validation | Supported by observable telemetry | Import/compiler/process/telemetry/differential/fuzz backends exist; unavailable daemon measurements remain unsupported, never zero |
| Durable artifacts, provenance, and CLI/CI parity | Exact | Campaign, normalized plan, seed, fidelity, trace, ledger, metrics, assertions, failures, provenance, replay bundle, and release audit are persisted |

## Qualification result

The product specification now has an executable surface for every load-bearing
workflow: author, validate, run, animate, intervene, inspect causality, search,
shrink, replay, compare fidelity/variants, aggregate to massive scale, and
validate against available daemon evidence. Boundaries that cannot honestly be
executed—real access-media behavior, full cryptography, selective visibility,
withheld forwarding, calibrated wall-clock CPU, and unavailable daemon
telemetry—are explicit fidelity or policy assumptions, never silently implied.

Future protocol actions and visual diagnostics remain extensions, not blockers
to the defined workbench workflow. New actions must enter the typed event
parser, native authoring surface, deterministic tests, and fidelity contract
together.
