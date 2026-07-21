# ADR 0005: CLI and immutable artifacts before browser UI (E1 toward E2)

- Status: accepted for P0
- Owner milestone: M6
- Decision key: E1 evolving toward E2

## Decision

Engine and CLI semantics lead. Immutable, versioned artifacts are the boundary
for analysis. Browser UI implementation begins only after stable artifact and
query fixtures exist.

## Rationale

The simulator must remain scriptable, reproducible, and reviewable without a
presentation runtime. A UI that owns ordering or derived truth would make the
scientific contract impossible to audit.

## Consequences

- M0 ships schemas, fixtures, validation, normalization, and CLI commands only.
- No M0 UI implementation is authorized by this ADR.
- Future renderers consume artifacts and may not mutate experiment results.
- Plain-language fidelity statements are derivable without presentation state.

## Reversal trigger

M6 owns reversal. Change the boundary only if measured artifact/query behavior
cannot support required interactive analysis without a versioned derived index.
The engine and canonical artifact remain independent in any replacement.

## Reversal evidence

A reversal requires a reproducible artifact, the failed query or latency
contract, and a proposed derived representation with deterministic provenance.
