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

## Coverage

The normative Root Ratchet document plus the nine files under
`examples/campaigns/` cover the ten flagship families. They prove schema
representability only; M1–M7 own engine behavior, search, shrinking, daemon
reproduction, and campaign qualification.
