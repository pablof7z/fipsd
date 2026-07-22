# P0 roadmap

The roadmap is organized as nine demonstrable outcome gates. GitHub milestones
hold scheduling state; one epic per milestone holds the exit criteria; child
issues define reviewable implementation slices and blockers.

Milestones intentionally have no due dates yet. Performance and schedule
estimates should follow the M0/M1 evidence rather than precede it.

## Sequence

| Milestone | Gate | Epic | GitHub milestone |
| --- | --- | --- | --- |
| M0 | Campaign/schema/artifact contracts and executable-codec proof | [#1](https://github.com/pablof7z/fipsd/issues/1) | [M0](https://github.com/pablof7z/fipsd/milestone/1) |
| M1 | Deterministic Root Ratchet vertical slice | [#9](https://github.com/pablof7z/fipsd/issues/9) | [M1](https://github.com/pablof7z/fipsd/milestone/2) |
| M2 | Coupled protocol breadth and causal accounting | [#18](https://github.com/pablof7z/fipsd/issues/18) | [M2](https://github.com/pablof7z/fipsd/milestone/3) |
| M3 | Generative campaigns, search, and shrinking | [#27](https://github.com/pablof7z/fipsd/issues/27) | [M3](https://github.com/pablof7z/fipsd/milestone/4) |
| M4 | Cohort/hybrid scale and protocol variants | [#37](https://github.com/pablof7z/fipsd/issues/37) | [M4](https://github.com/pablof7z/fipsd/milestone/5) |
| M5 | Real-daemon oracle and differential validation | [#46](https://github.com/pablof7z/fipsd/issues/46) | [M5](https://github.com/pablof7z/fipsd/milestone/6) |
| M6 | Multi-resolution artifact analysis UI | [#55](https://github.com/pablof7z/fipsd/issues/55) | [M6](https://github.com/pablof7z/fipsd/milestone/7) |
| M7 | Ten flagship campaign acceptance suites | [#65](https://github.com/pablof7z/fipsd/issues/65) | [M7](https://github.com/pablof7z/fipsd/milestone/8) |
| M8 | Determinism/accounting audits and v0.1 release | [#76](https://github.com/pablof7z/fipsd/issues/76) | [M8](https://github.com/pablof7z/fipsd/milestone/9) |

M0→M5 are the load-bearing engine and scientific path. M6 begins once immutable
artifact/query fixtures exist and may then proceed alongside later engine work.
M7 qualifies the product boundary; M8 converts that evidence into a supported
release.

The source tree now contains executable evidence for M0 through M8. GitHub issue
and milestone state remains the public review/closure record; the verification
maps and checked-in fixtures are the implementation record.

## M0 — Scientific contract & repository spine

Demo: validate Root Ratchet v1alpha1, emit a byte-stable normalized plan, and
derive exact boundary sizes through a pinned FIPS codec seam.

M0 locks the same named defaults used by the README and architecture: B3
pluggable protocol variants, incremental A3 independent-reference-model first,
C3 cohort/hybrid billion-node representation, D2 authenticated protocol-valid
adversaries, and E1 evolving toward E2 with CLI/immutable artifacts before UI.

- [#2](https://github.com/pablof7z/fipsd/issues/2) — Lock P0 architecture forks and reversal points
- [#3](https://github.com/pablof7z/fipsd/issues/3) — Inventory FIPS semantic, codec, clock, and telemetry seams
- [#4](https://github.com/pablof7z/fipsd/issues/4) — Define the fidelity contract and provenance envelope
- [#5](https://github.com/pablof7z/fipsd/issues/5) — Specify Campaign v1alpha1 and publish its JSON Schema
- [#6](https://github.com/pablof7z/fipsd/issues/6) — Specify deterministic run and reproduction artifacts
- [#7](https://github.com/pablof7z/fipsd/issues/7) — Scaffold the Rust workspace, CLI, schemas, and CI gates
- [#8](https://github.com/pablof7z/fipsd/issues/8) — Prove executable-codec accounting against pinned FIPS

## M1 — Deterministic Root Ratchet vertical slice

Demo: run descending roots over individually represented nodes/edges, replay the
same seed exactly, and explain root adoption, ancestry growth, TreeAnnounce
stages, bytes, queues, and quiescence.

- [#10](https://github.com/pablof7z/fipsd/issues/10) — Deterministic virtual clock and event scheduler
- [#11](https://github.com/pablof7z/fipsd/issues/11) — Compact individual-node and edge storage
- [#12](https://github.com/pablof7z/fipsd/issues/12) — Initial topology generators and attachment selectors
- [#13](https://github.com/pablof7z/fipsd/issues/13) — Current-FIPS root election and parent selection
- [#14](https://github.com/pablof7z/fipsd/issues/14) — TreeAnnounce lifecycle, debounce, and exact bytes
- [#15](https://github.com/pablof7z/fipsd/issues/15) — Link behavior and queues
- [#16](https://github.com/pablof7z/fipsd/issues/16) — Descending-root identity and arrival policies
- [#17](https://github.com/pablof7z/fipsd/issues/17) — CLI replay, invariants, and first report

## M2 — Protocol breadth & causal accounting

Demo: run Root Ratchet under useful traffic and identify the causal path that
dominates Bloom convergence, lookup recovery, queueing, and goodput restoration.

- [#19](https://github.com/pablof7z/fipsd/issues/19) — Exact, sparse, and occupancy Bloom modes
- [#20](https://github.com/pablof7z/fipsd/issues/20) — Split-horizon propagation, antipoison, and debounce
- [#21](https://github.com/pablof7z/fipsd/issues/21) — Coordinate cache, invalidation, freshness, and warmup
- [#22](https://github.com/pablof7z/fipsd/issues/22) — Discovery, lookup, retry, dedup, TTL, and routing signals
- [#23](https://github.com/pablof7z/fipsd/issues/23) — Synthetic sessions and payload traffic
- [#24](https://github.com/pablof7z/fipsd/issues/24) — Node resources, tables, scheduler share, and queues
- [#25](https://github.com/pablof7z/fipsd/issues/25) — Exact causal cost ledger and reconciliation
- [#26](https://github.com/pablof7z/fipsd/issues/26) — Full Root Ratchet recovery and starvation report

## M3 — Campaign algebra, search & shrinking

Demo: automatically discover a high-amplification authenticated campaign and
shrink it across traffic, topology, nodes, events, timing, and transports.

- [#28](https://github.com/pablof7z/fipsd/issues/28) — Scenario algebra compiler
- [#29](https://github.com/pablof7z/fipsd/issues/29) — Cartesian, pairwise/t-wise, and Monte Carlo planners
- [#30](https://github.com/pablof7z/fipsd/issues/30) — Property-based topology and event generation
- [#31](https://github.com/pablof7z/fipsd/issues/31) — Mixed transport and media profiles
- [#32](https://github.com/pablof7z/fipsd/issues/32) — Authenticated protocol-valid adversaries
- [#33](https://github.com/pablof7z/fipsd/issues/33) — Adversarial objective search
- [#34](https://github.com/pablof7z/fipsd/issues/34) — Hierarchical failure trace shrinking
- [#35](https://github.com/pablof7z/fipsd/issues/35) — Parallel execution, checkpointing, and budgets
- [#36](https://github.com/pablof7z/fipsd/issues/36) — Deterministic regression corpus

## M4 — Multi-fidelity scale & protocol variants

Demo: run a billion-node cohort Root Ratchet, instantiate an anomalous exact
region, and compare current FIPS with root-dampening and Bloom-delta variants.

- [#38](https://github.com/pablof7z/fipsd/issues/38) — Cohort analytical engine
- [#39](https://github.com/pablof7z/fipsd/issues/39) — Cohort-FPR and sampled-exact Bloom fidelity
- [#40](https://github.com/pablof7z/fipsd/issues/40) — Exact sampled regions inside analytical cohorts
- [#41](https://github.com/pablof7z/fipsd/issues/41) — Crypto execution and cost modes
- [#42](https://github.com/pablof7z/fipsd/issues/42) — Pluggable protocol-variant interface
- [#43](https://github.com/pablof7z/fipsd/issues/43) — Versioned baseline and reference variants
- [#44](https://github.com/pablof7z/fipsd/issues/44) — Cross-engine calibration
- [#45](https://github.com/pablof7z/fipsd/issues/45) — Honest one-billion-node demonstration

## M5 — Real-daemon oracle & differential validation

Demo: compile a minimized bundle to the Docker chaos harness, run pinned FIPS
binaries, normalize telemetry, and locate the first model/daemon divergence.

- [#47](https://github.com/pablof7z/fipsd/issues/47) — Import existing chaos YAML
- [#48](https://github.com/pablof7z/fipsd/issues/48) — Compile representable campaigns to chaos YAML
- [#49](https://github.com/pablof7z/fipsd/issues/49) — Normalize daemon telemetry
- [#50](https://github.com/pablof7z/fipsd/issues/50) — Capture binary/image/commit/config/host provenance
- [#51](https://github.com/pablof7z/fipsd/issues/51) — Semantic, transition, frame, and metric differentials
- [#52](https://github.com/pablof7z/fipsd/issues/52) — Automated minimized real-daemon reproductions
- [#53](https://github.com/pablof7z/fipsd/issues/53) — Invalid-wire fuzz result integration
- [#54](https://github.com/pablof7z/fipsd/issues/54) — Smoke, nightly, and historical oracle suites

## M6 — Multi-resolution analysis UI

Demo: open two immutable artifacts, inspect fidelity and the root wave, follow
the causal critical path, and compare a variant with a real-daemon result.

- [#56](https://github.com/pablof7z/fipsd/issues/56) — Artifact query and downsampling layer
- [#57](https://github.com/pablof7z/fipsd/issues/57) — Browser analysis shell and run library
- [#58](https://github.com/pablof7z/fipsd/issues/58) — Summary, fidelity, provenance, and quiescence views
- [#59](https://github.com/pablof7z/fipsd/issues/59) — Exact small-topology graph and event inspection
- [#60](https://github.com/pablof7z/fipsd/issues/60) — Aggregated network, distribution, matrix, and cohort views
- [#61](https://github.com/pablof7z/fipsd/issues/61) — Root lineage and propagation wavefront
- [#62](https://github.com/pablof7z/fipsd/issues/62) — Causal amplification and critical path
- [#63](https://github.com/pablof7z/fipsd/issues/63) — Protocol-variant and daemon differential comparison
- [#64](https://github.com/pablof7z/fipsd/issues/64) — Shareable reports and saved reproduction bundles

## M7 — Flagship campaign acceptance

Demo: publish a campaign atlas with a baseline, discovered boundary, variant
comparison, and minimized reproduction for every family.

- [#66](https://github.com/pablof7z/fipsd/issues/66) — Root Ratchet
- [#67](https://github.com/pablof7z/fipsd/issues/67) — Competing Partition Roots
- [#68](https://github.com/pablof7z/fipsd/issues/68) — Bloom Saturation Accession
- [#69](https://github.com/pablof7z/fipsd/issues/69) — Ancestor-Swap Bloom Storm
- [#70](https://github.com/pablof7z/fipsd/issues/70) — Deep-Tree MTU and TTL Cliff
- [#71](https://github.com/pablof7z/fipsd/issues/71) — Lookup Thundering Herd
- [#72](https://github.com/pablof7z/fipsd/issues/72) — Parent Hysteresis Oscillator
- [#73](https://github.com/pablof7z/fipsd/issues/73) — Mixed-Transport Failover
- [#74](https://github.com/pablof7z/fipsd/issues/74) — Synchronized Rekey Avalanche
- [#75](https://github.com/pablof7z/fipsd/issues/75) — Authenticated Sybil Pressure

## M8 — P0 hardening & v0.1 release

Demo: from a clean release install, run a flagship case, inspect and compare it,
export a reproduction, and verify every headline against provenance.

- [#77](https://github.com/pablof7z/fipsd/issues/77) — Cross-host determinism audit
- [#78](https://github.com/pablof7z/fipsd/issues/78) — Accounting reconciliation audit
- [#79](https://github.com/pablof7z/fipsd/issues/79) — Measured performance and resource envelopes
- [#80](https://github.com/pablof7z/fipsd/issues/80) — Tutorials, schema, campaign, and interpretation guides
- [#81](https://github.com/pablof7z/fipsd/issues/81) — Threat model and safe-execution boundaries
- [#82](https://github.com/pablof7z/fipsd/issues/82) — Reproducible CLI and browser release artifacts
- [#83](https://github.com/pablof7z/fipsd/issues/83) — v0.1 campaign atlas, benchmark report, and release
