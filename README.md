# FIPS Wind Tunnel

**Find the protocol cliff before the network does.**

FIPS Wind Tunnel is a deterministic, multi-fidelity experimentation
system for generating, simulating, measuring, attacking, comparing, and
minimizing [FIPS](https://github.com/jmcorgan/fips) network behavior—from a
handful of real daemons to billion-node analytical cohorts.

> [!IMPORTANT]
> v0.1 implements the M0–M8 roadmap: scientific contracts, deterministic
> engines, causal accounting, replayable campaign search, honest cohort/hybrid
> scale, variants, a pinned real-daemon oracle, read-only artifact analysis, the
> ten-family qualification atlas, and reproducible release audits.

The flagship campaign is **Root Ratchet**, formally a Descending-Minimum Root
Cascade: authenticated identities with successively lower addresses arrive
above the current root. Each arrival can force a network-wide root transition
while adding another ancestor to every existing node.

At tree depth `d`, an executable-codec established FMP-framed TreeAnnounce is
`168 + 32d` bytes. That makes depth 35 an immediate 1,288-byte framing boundary, before underlying
transport overhead. The wind tunnel is being built to find those boundaries,
explain their causal cost, and reduce large failures into small real-daemon
reproductions.

The one-byte difference from the pinned FIPS wire-format prose is recorded as
[upstream documentation drift](docs/fips-seam-inventory.md#recorded-documentation-drift);
the executable codec is authoritative.

## What a useful result must explain

A report that only says “the network converged in 8.2 seconds” is not enough.
The system must show:

- which root generations were adopted, skipped, or coalesced;
- which nodes and depth bands lagged;
- which state transitions, signatures, frames, queues, and retries dominated;
- exact serialized bytes where production codecs are available;
- modeled compute, memory, table, and queue pressure;
- how useful goodput changed while the control plane converged;
- which conclusions are exact, calibrated, statistical, or cohort-level;
- whether a minimized case reproduces against real FIPS daemons.

## Fidelity is part of the result

| Contract | Meaning |
| --- | --- |
| Wire-exact | Bytes come from executable production codecs or verified generated schemas. |
| Semantically exact | Individual protocol states and event order are represented. |
| Operation-counted | Work is counted without executing every cryptographic or bitwise operation. |
| Calibrated estimate | Counts are converted through a versioned hardware profile. |
| Statistical approximation | Seeded probabilistic state replaces exact per-node detail. |
| Cohort analytical | Populations and distributions replace individual nodes. |
| Hybrid | Exact sampled regions are embedded in analytical cohorts. |

No view may present an approximate result as an exact replay.

## Experiment loop

v0.1 ships the loop through deterministic search, shrinking, variant/cohort
execution, pinned real-daemon comparison, analysis, and export:

```text
author or generate a campaign
  → compile it into deterministic experiment cases
  → run the appropriate fidelity engine
  → inspect causal costs and failed invariants
  → search for a worse boundary
  → shrink the failure
  → replay the minimized case against real FIPS daemons
  → compare a protocol variant
  → inspect and export provenance-linked evidence
```

The normative Root Ratchet campaign is checked in at
[`examples/root-ratchet.yaml`](examples/root-ratchet.yaml) and validated by the
published Campaign v1alpha1 schema.

## Quick start

```bash
cargo run -p fips-cli --bin fips-wind-tunnel -- \
  validate examples/root-ratchet.yaml

cargo run -p fips-cli --bin fips-wind-tunnel -- \
  normalize examples/root-ratchet.yaml --output root-ratchet.normalized.json

cargo run -p fips-cli --bin fips-wind-tunnel -- \
  atlas verify fixtures/m7/qualification-atlas.json

cargo run -p fips-cli --bin fips-wind-tunnel -- \
  release verify-audit fixtures/m8/release-audit.json
```

The same input and seed produce byte-identical normalized output. Run the full
local gate with `scripts/check.sh`; run the pinned production-codec drift gate
with `scripts/check-fips-codecs.sh --check`.

See the [v0.1 quick start](docs/quickstart.md) for run, report, search, shrink,
variant, oracle, custom-campaign, package, and clean-install workflows.

M0 contracts and evidence:

- [`schemas/campaign-v1alpha1.schema.json`](schemas/campaign-v1alpha1.schema.json)
- [`schemas/normalized-plan-v1alpha1.schema.json`](schemas/normalized-plan-v1alpha1.schema.json)
- [`schemas/run-artifact-v1alpha1.schema.json`](schemas/run-artifact-v1alpha1.schema.json)
- [`schemas/reproduction-bundle-v1alpha1.schema.json`](schemas/reproduction-bundle-v1alpha1.schema.json)
- [Fidelity and provenance](docs/fidelity-and-provenance.md)
- [Campaign semantics, units, defaults, and extensions](docs/campaign-v1alpha1.md)
- [Artifact format](docs/artifact-format.md)
- [Pinned FIPS seam inventory](docs/fips-seam-inventory.md)
- [M0 acceptance and verification map](docs/m0-verification.md)

M1's runnable individual-node loop is documented in
[M1 deterministic individual engine](docs/m1-individual-engine.md), with a
checked-in 12-node campaign, immutable artifact, reproduction bundle, and
plain-language report under `fixtures/m1/`. The requirement-by-requirement
evidence is indexed in the [M1 verification map](docs/m1-verification.md).

M2's coupled protocol/data-plane slice is documented in
[M2 recovery and causal accounting](docs/m2-recovery-accounting.md). Its small
active-traffic campaign and byte-stable evidence live under `examples/m2/` and
`fixtures/m2/`; the [M2 verification map](docs/m2-verification.md) ties every
child issue to executable evidence.

M3's bounded experiment loop is documented in
[M3 deterministic campaign search and shrinking](docs/m3-campaign-search.md).
Its pairwise plan, full search evidence, minimized replay, symbolic million-node
input, and corpus result live under `fixtures/m3/`; the
[M3 verification map](docs/m3-verification.md) maps every child issue to tests.

M4's bounded scale engine and variants are documented in
[M4 honest cohort/hybrid scale and protocol variants](docs/m4-scale-variants.md).
The one-billion-node campaign and byte-stable evidence live under `examples/m4/`
and `fixtures/m4/`; the [M4 verification map](docs/m4-verification.md) records
the fidelity, calibration, variant, and resource-budget proofs.

M5's pinned implementation oracle is documented in
[M5 pinned-daemon oracle](docs/m5-daemon-oracle.md). Six imported chaos families,
deterministic compiled/normalized/differential fixtures, suite manifests, and a
three-repeat live smoke summary live under `fixtures/m5/`; the
[M5 verification map](docs/m5-verification.md) ties each oracle contract to
tests and reproducible evidence.

M6's read-only query and static browser layers are documented in
[M6 multi-resolution analysis](docs/m6-analysis.md), with a checked-in analysis
fixture under `fixtures/m6/`.

M7's ten-family qualification evidence is documented in
[M7 campaign atlas](docs/m7-campaign-atlas.md) and checked in under
`fixtures/m7/`.

M8's determinism/accounting audit, measured performance, threat boundary,
support matrix, package manifest, SBOM/checksum workflow, and clean-install gate
are indexed in the [M8 verification map](docs/m8-verification.md).

## Architecture direction

The P0 roadmap starts with these explicit defaults:

- **Pluggable protocol variants** are a core requirement.
- **Dual-model validation** is the destination: an independent reference model
  plus production/shared-core adapters where upstream seams justify them.
- **Billion-node support means hybrid/cohort execution**, not a hidden claim of
  one billion individually allocated nodes.
- **Authenticated protocol-valid adversaries are in P0**. Malformed-wire
  fuzzing remains a connected, distinct backend.
- **Engine and CLI semantics lead**; the browser consumes immutable run
  artifacts and does not own simulation truth.

The current upstream FIPS tree already separates several sans-I/O protocol
cores, state, limits, and wire codecs, but the cores are crate-private. M0
therefore includes a source-grounded reuse spike before committing to an
upstream extraction. See [Architecture](docs/architecture.md).

## Milestones

| Milestone | Demonstrable gate | Epic |
| --- | --- | --- |
| M0 | Validate and normalize Root Ratchet; prove codec-derived accounting | [#1](https://github.com/pablof7z/fipsd/issues/1) |
| M1 | Deterministically run and replay an individually modeled Root Ratchet | [#9](https://github.com/pablof7z/fipsd/issues/9) |
| M2 | Explain Bloom, cache, lookup, session, resource, and queue amplification | [#18](https://github.com/pablof7z/fipsd/issues/18) |
| M3 | Search campaign space and shrink a failure to a small reproduction | [#27](https://github.com/pablof7z/fipsd/issues/27) |
| M4 | Compare variants through an honest billion-node cohort/hybrid run | [#37](https://github.com/pablof7z/fipsd/issues/37) |
| M5 | Reproduce a minimized case against pinned real FIPS daemons | [#46](https://github.com/pablof7z/fipsd/issues/46) |
| M6 | Inspect exact, aggregate, cohort, causal, and differential views | [#55](https://github.com/pablof7z/fipsd/issues/55) |
| M7 | Qualify all ten flagship campaigns | [#65](https://github.com/pablof7z/fipsd/issues/65) |
| M8 | Publish audited, reproducible v0.1 artifacts and campaign evidence | [#76](https://github.com/pablof7z/fipsd/issues/76) |

The [full roadmap](docs/roadmap.md) links every child issue and milestone gate.

## Grounding and project boundary

The initial plan was checked against FIPS commit
[`80c956a`](https://github.com/jmcorgan/fips/tree/80c956a6fdb85dde1450969a21891c1158e43267),
including its [`src/proto`](https://github.com/jmcorgan/fips/tree/80c956a6fdb85dde1450969a21891c1158e43267/src/proto)
sans-I/O modules, executable wire codecs, control-socket telemetry, and
[`testing/chaos`](https://github.com/jmcorgan/fips/tree/80c956a6fdb85dde1450969a21891c1158e43267/testing/chaos)
Docker harness.

FIPS Wind Tunnel is not an application sandbox, Docker farm, production fleet
manager, or graph canvas. Docker remains the highest-fidelity, lowest-scale
validation backend; it does not define the simulator architecture.

## Contributing

Start with the earliest open milestone whose dependencies are satisfied. Each
issue defines its outcome, bounded scope, acceptance criteria, and blockers.
See [CONTRIBUTING.md](CONTRIBUTING.md) for the scientific and engineering
expectations.

## License

FIPS Wind Tunnel is available under the [MIT License](LICENSE).
