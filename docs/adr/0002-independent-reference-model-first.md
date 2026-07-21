# ADR 0002: Independent reference model before shared core (incremental A3)

- Status: accepted for P0
- Owner milestones: M0 and M5
- Decision key: incremental A3

## Decision

Ship the compact, versioned independent reference model before requiring a
shared-core FIPS dependency. Reuse production codecs immediately where the
public seam permits it, expose the smallest deterministic upstream seam when
justified, and retain real-daemon comparison as the final oracle.

## Rationale

Shared code improves fidelity but cannot detect a bug it shares with the
implementation under test. The independent model is also easier to instrument,
shrink, and represent compactly. The pinned FIPS tree already has useful
sans-I/O cores, but most are crate-private.

## Consequences

- Independent transitions are normalized before comparison with shared cores.
- Production wire bytes outrank copied constants and prose.
- Shared-core integration is incremental, never a prerequisite for the first
  reference model.
- Every comparison records the exact FIPS commit.

## Reversal trigger

M0 may require a shared wire seam if an exact codec cannot be exercised or
generated reliably. M5 may promote additional shared cores if differential
evidence shows the independent model cannot stay aligned at reasonable cost.

## Reversal evidence

The evidence must include the first divergent normalized transition, the
shared-bug risk introduced, and a no-shared-core fallback assessment.

