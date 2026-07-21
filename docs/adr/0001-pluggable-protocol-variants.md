# ADR 0001: Pluggable protocol variants (B3)

- Status: accepted for P0
- Owner milestone: M4
- Decision key: B3

## Decision

Protocol decisions are selected through versioned variant identifiers and
deterministic engine interfaces. The baseline FIPS behavior is one variant; it
is not compiled into scheduler, storage, artifact, or presentation code.

## Rationale

The wind tunnel exists to compare protocol changes. A hard-wired baseline
would make every experiment an engine fork and would make differential results
depend on unrelated implementation changes.

## Consequences

- Campaigns and artifacts carry a variant identifier and configuration digest.
- Variant hooks receive injected virtual time and deterministic inputs only.
- A variant cannot redefine artifact ordering, fidelity labels, or accounting.
- M0 defines the boundary; production variants are intentionally deferred.

## Reversal trigger

M4 owns validation. Reverse or narrow B3 if two independently implemented
variants cannot share the same deterministic engine API without variant-aware
scheduler or artifact branches, or if measured dispatch cost materially
changes the supported scale envelope.

## Reversal evidence

A reversal requires matched campaign artifacts showing the incompatible state
or ordering contract, plus a measured comparison of the proposed replacement.
