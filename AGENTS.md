# Agent instructions

## Mission

Build FIPS Wind Tunnel as a deterministic, multi-fidelity protocol
experimentation system. Do not let the real-daemon Docker harness, the browser
UI, or a small-graph representation define the engine architecture.

## Sources of truth

Use this order when sources disagree:

1. Checked-in tests and executable behavior in the pinned FIPS revision.
2. FIPS wire codecs and sans-I/O protocol modules.
3. Current FIPS design/reference documentation.
4. This repository's accepted ADRs and schemas.
5. Roadmap prose and issue descriptions.

Record drift instead of averaging conflicting eras into a vague model.

## Workspace boundary

- Keep changes for this product in this repository.
- Do not edit `../fips` unless an issue explicitly requires an upstream change.
- Put upstream changes on a separate FIPS branch/PR and pin the tested commit.
- Preserve unrelated or uncommitted work. Stage explicit paths.

## Non-negotiable contracts

- Protocol time is injected virtual time.
- Event ordering is stable and independent of worker count.
- Every result declares fidelity and provenance.
- Wire-exact claims come from executable codecs or verified generated schemas.
- Approximate Bloom, cohort, calibrated, and sampled results stay labeled.
- Causal stages distinguish requested, performed, constructed, superseded,
  coalesced, serialized, queued, transmitted, and delivered work.
- Ledger projections reconcile or state their excluded mass.
- Failures produce replayable artifacts and should be shrunk before promotion.
- Malformed-wire fuzzing and authenticated protocol-valid adversaries are
  different subsystems.

## Delivery workflow

Work from the earliest unblocked milestone issue. Keep one issue-sized semantic
change per pull request where practical. Update fixtures, schema, documentation,
and fidelity/provenance metadata in the same change that alters their behavior.

Do not claim scale, speed, compatibility, or real-daemon agreement without a
reproducible artifact and the environment needed to interpret it.
