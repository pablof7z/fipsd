# ADR 0004: Authenticated protocol-valid adversaries in P0 (D2)

- Status: accepted for P0
- Owner milestones: M3 and M5
- Decision key: D2

## Decision

P0 adversaries possess valid identities and emit protocol-valid behavior under
explicit operation, identity, byte, compute, and time budgets. Malformed-wire
fuzzing is a connected but separate subsystem.

## Rationale

Protocol-valid abuse exercises admission, topology, state, and economic
boundaries that packet fuzzers do not. Combining the two would blur trust,
fidelity, and safety assumptions.

## Consequences

- Campaigns separate authenticated adversary actions from malformed inputs.
- Invalid signatures and malformed frames cannot be counted as D2 behavior.
- Damage is reported beside attacker cost and accepted/rejected outcomes.
- Fuzz findings may enter differential reports but not the semantic engine.

## Reversal trigger

M3 may narrow D2 if deterministic budgeting cannot bound adversary search. M5
may unify result envelopes only if daemon evidence can preserve the semantic
versus malformed distinction without ambiguity.

## Reversal evidence

The proposal must show a minimized case, its trust boundary, and how the new
model keeps attacker cost and malformed-input provenance distinct.

