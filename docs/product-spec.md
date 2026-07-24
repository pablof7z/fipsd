# FIPS Protocol Wind Tunnel — Product Specification

## Definition

**FIPS Protocol Wind Tunnel is a deterministic, multi-fidelity protocol
experimentation platform that lets FIPS developers generate networks at
arbitrary scale, subject them to stochastic and adversarial conditions, measure
every meaningful control-plane and data-plane consequence, identify structural
and implementation bottlenecks, compare protocol variants, and reduce large
failures into small reproducible cases validated against the real FIPS
implementation.**

It is the conceptual equivalent of Polar only in the sense that it makes a
network system constructible, observable, reproducible, and inspectable. It is
not primarily a visual launcher for a handful of FIPS daemons. Its purpose is to
investigate how FIPS behaves under complex, large-scale, stochastic, and
adversarial conditions.

## Primary user and questions

The initial product is for FIPS protocol developers and researchers. It should
help answer:

- Where does the protocol stop scaling?
- Which operations create disproportionate control traffic?
- Which nodes or links become bottlenecks?
- How does topology churn affect routing, data transfer, and convergence?
- What pathological sequences can jam or destabilize the network?
- Which limits are inherent to protocol design, and which are implementation problems?
- What attacks can authenticated or otherwise protocol-valid participants perform?
- How do proposed protocol changes alter those outcomes?

## Core purpose and scale

A developer can define or generate a FIPS network, subject it to arbitrary
event sequences, simulate protocol and payload activity, and measure resulting
behavior with precise causal accounting.

Experiments range from a few nodes to thousands, millions, and potentially
billions. Small runs support inspection of individual nodes, links, messages,
state transitions, and routes. Massive runs use simulation, aggregation,
cohort, or analytical representations rather than pretending a literal graph
is useful. Every result states its fidelity. A billion-node analytical result
must not look like one billion complete implementations were executed.

## Modeled behavior

The system models:

- Node identities, root selection and replacement, and parent selection.
- Tree construction, reconstruction, joins, removals, failures, and recoveries.
- Network partitions and merges.
- Bloom-filter construction, replacement, propagation, saturation, and false positives.
- Coordinate generation, propagation, invalidation, and caching.
- Discovery, lookup, routing, forwarding, session construction, and session churn.
- Rekeying, replay behavior, and control-plane/data-plane interaction.
- Link latency, jitter, packet loss, bandwidth, MTU, ordering, and queueing.
- CPU, memory, cache, session, and queue limits.
- Cryptographic operations and their cost.
- Synthetic application payloads and traffic patterns.
- Mixed transports and mixed network environments.

Per-node connectivity may be explicitly assigned or seed-randomized from
weighted profiles. Each profile can define bandwidth, latency, loss, MTU, and
queue limits; runs retain the assignments and effective edge bottlenecks.

## Synthetic traffic

The product does not primarily launch real applications that happen to use
FIPS. It simulates application traffic directly so experiments can scale
without millions of processes or containers. Traffic models include:

- Uniform random and all-to-all traffic.
- Hotspot or Zipf-distributed traffic.
- Many-to-one incast and one-to-many fanout.
- Large persistent flows and many short-lived flows.
- Bursty or synchronized traffic.
- Traffic crossing selected cuts, roots, bridges, or bottlenecks.
- Payload-size sweeps around framing and MTU boundaries.

Real FIPS daemons and the Docker harness remain a small-scale validation
backend, not the primary scale engine.

## Scenario generation

The scenario system combines topology, scale, identity distribution, transport
assignment, link conditions, node capabilities, event timing, churn, traffic,
protocol parameters, adversarial behavior, assertions, and random seed.

It supports explicit scenarios, deterministic regressions, Cartesian sweeps,
pairwise or higher-order coverage, Monte Carlo campaigns, property-based and
stochastic generation, adversarial optimization, protocol-variant comparison,
and automatic failure minimization. The goal is to explore many meaningful
combinations rather than maintain disconnected scenario files.

## Natural-language experiment authoring

The native workbench accepts a description such as:

> Show a network of 10,000 nodes with a new lower root added at different
> points at one node per second.

A provider-neutral local adapter may invoke `claude -p` or `codex exec` with a
constrained authoring prompt. The model may produce only a declarative Campaign
document. Model output is untrusted: it is schema-validated, semantically
validated, normalized, budget-checked, made inspectable, and only then run. It
has no tool or shell authority. The authoring provider and generated document
are retained as provenance.

The workbench starts rendering as soon as the normalized run emits events.
Configuration changes create a new deterministic run. Events presented as live
mutations are explicit, totally ordered simulation inputs—not hidden changes to
history.

Follow-up prompts receive the active Campaign plus a semantic snapshot of the
current rendered cursor: node identities and join order, roots and parents,
edge IDs and conditions, transfer progress, and scheduled future events. They
produce a forward-only amendment rather than a replacement Campaign. The host
rejects events before the cursor, applies only supported future changes, and
replays deterministic history to the same point before continuing.

## Flagship example: cascading lower roots

Successively lower hashed public keys join the network. Each becomes the root;
another joins shortly afterward, perhaps every second, millisecond, or near a
coalescing/debounce boundary. The product determines:

- How many nodes recompute root, parent, ancestry, and coordinates.
- How many announcements and Bloom replacements are constructed.
- Which work is coalesced, superseded, queued, transmitted, or discarded.
- Whether control traffic grows linearly, quadratically, or otherwise.
- Whether data traffic is delayed or starved and resources saturate.
- Whether the topology reaches quiescence and how tree depth changes.
- Where messages cross MTU, encoding, TTL, or payload boundaries.
- Whether identity grinding can deliberately produce the behavior.

This is one flagship, not the product definition. Other families include churn,
partitions, Bloom saturation, lookup storms, transport failures, rekey waves,
resource exhaustion, and protocol-valid adversarial behavior.

## Precise causal accounting

The product accounts for semantic transitions; messages created, signed,
serialized, coalesced, superseded, queued, transmitted, and delivered; protocol,
framing, and transport bytes; retransmissions; cryptographic and Bloom work;
memory/cache changes; queue occupancy; CPU or calibrated CPU cost; and useful
payload delivered.

It preserves causal relationships. One root arrival can be traced through
parent changes, coordinate reconstruction, announcements, Bloom recomputation,
cache invalidation, session disruption, lookup retries, queue pressure, and
payload degradation. Failures explain their dominant resources and causes.

In the individual-node engine, synthetic flows expose each selected route and
every per-edge queue, transmission, loss, rejection, and delivery as ordered
artifact events. Control and payload share link capacity, and their mass is
reconciled independently. Current executable behavior and its fidelity limits
are documented in [`routed-payload-stream.md`](routed-payload-stream.md).

Bloom-only experiments likewise retain per-neighbor split-horizon state and
expose every replacement, edge transmission, rejection, and fresh receipt in
the ordered trace. The executable boundary is documented in
[`graph-native-bloom.md`](graph-native-bloom.md).

## Boundaries, bottlenecks, and hostile objectives

The system identifies size cliffs, MTU boundaries, routing-depth and TTL limits,
Bloom saturation, CPU/crypto bottlenecks, memory/cache exhaustion, queue buildup,
root-proximal congestion, control amplification, data starvation, parent
instability, convergence failure, attack amplification, pathological timer
interactions, and failure to return to baseline.

Search objectives include maximizing control bytes per valid join, convergence
time, queue occupancy, parent changes, or Bloom false-positive work; minimizing
useful throughput; and finding the smallest invariant-violating event sequence.

## Failure reduction

Large stochastic failures are reduced by removing irrelevant topology regions
and traffic, lowering node count, shortening the event sequence, simplifying
transports and timing, and retaining only necessary adversarial behavior. The
reduced deterministic case is rerun at higher fidelity or against real daemons.

## Protocol variants

Experiments can compare root-selection policies, root dampening or tenure,
parent rules, debounce intervals, incremental Bloom updates, Bloom sizes and
folding, lookup strategies, coordinate systems, cache policies, and session or
rekey behavior. Alternatives are made testable and falsifiable, not assumed
better.

## Fidelity levels

Execution modes include exhaustive tiny-state exploration, high-fidelity state
machines, compact discrete-event simulation of individual nodes, cohort and
analytical simulation, hybrid exact/aggregate regions, and actual FIPS daemons.

Every result states which behavior was exact, semantically modeled,
probabilistic, operation-counted, calibrated, aggregated, or unsupported.

## Visualization

The visual interface changes with scale.

Small networks provide node-link and shared-medium views, root/parent
relationships, message propagation, session paths, node/link inspection, manual
event injection, exact causal timelines, and direct-link or network-zone
authoring.

Larger networks use hierarchical aggregation, collapsed subtrees and
communities, depth bands, transport groups, heatmaps, histograms, percentiles,
root-adoption wavefronts, congestion matrices, Bloom false-positive
distributions, heavy hitters, bottleneck rankings, anomaly-focused sampled
subgraphs, causal flame graphs, and side-by-side variant comparisons.

Billion-node views show cohorts, distributions, sensitivity analyses, phase
transitions, confidence ranges, and scaling laws—not decorative billions of
dots.

## Reproducibility

Every run produces a durable artifact containing its scenario, normalized case,
scale/topology, seeds, protocol version and variant, fidelity, event sequence,
traffic, assertions, measurements, causal accounting, logs and sampled traces,
failures, provenance, and daemon/image revisions where applicable.

The same scenario runs through CLI, campaigns, CI, and the visual interface.

## Relationship to existing FIPS tests

The product lives in its own repository. It imports existing FIPS scenario
formats where practical, especially chaos tests, but defines a general
experiment model rather than inheriting a Docker-oriented schema. Existing
scenarios become fixtures, regressions, and daemon-validation cases.

## Product components

1. Protocol and network simulation engine.
2. Campaign and scenario-generation system.
3. Adversarial and stochastic search engine.
4. Deterministic replay and shrinking system.
5. Precise causal cost ledger.
6. Multi-scale visualization and analysis interface.
7. Protocol-variant comparison framework.
8. Small-scale real-FIPS validation backend.
9. Reproducible artifact format.
10. CLI and automation surface for CI and research workflows.

## Non-goals

It is not primarily a production fleet manager, public-network operations
console, application hosting platform, Docker GUI, static topology editor,
packet animation toy, one-number benchmark, or simulator that hides
approximations behind authoritative-looking results.
