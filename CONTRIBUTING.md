# Contributing

FIPS Wind Tunnel is being built as an evidence-producing protocol instrument.
Correct-looking output is not enough: changes must preserve determinism, state
their fidelity, and reconcile their accounting.

## Choose work from the roadmap

1. Start with the earliest open [milestone](docs/roadmap.md) whose dependencies
   are satisfied.
2. Use the milestone epic to understand the demo and exit gate.
3. Use the child issue as the scope boundary. If a prerequisite is missing,
   open or link it instead of hiding it inside the implementation.
4. Keep pull requests small enough that their semantic and accounting effects
   can be reviewed independently.

Milestones are outcome gates, not date promises. Later work may be explored in
a branch, but it should not force abstractions into an earlier slice before the
earlier demo has produced evidence.

## Scientific contract

Every engine or report change must answer:

- What is represented exactly?
- What is counted but not executed?
- What is calibrated, sampled, probabilistic, or cohort-level?
- Which protocol version and FIPS commit define the semantics?
- What seed and event-order contract make the result replayable?
- Which totals reconcile mechanically, and what is excluded?
- What smallest fixture proves the behavior?

Never describe an approximate result as an exact replay. Never copy a wire-size
constant when an executable codec or generated schema can supply it.

## Determinism

- Use the injected virtual clock; protocol logic must not read wall time.
- Use stable IDs and explicit ordering. Hash-map iteration, worker completion,
  or host scheduling must not decide event order.
- Split random streams by named purpose so adding unrelated instrumentation
  does not perturb protocol outcomes.
- Commit a minimal golden fixture for a new semantic boundary.
- Report calibrated wall-clock estimates separately from deterministic virtual
  time.

## Causal accounting

New work must preserve the distinction between:

```text
requested → performed → constructed → signed → serialized
          → queued → transmitted → delivered
```

Semantic, framing, transport, reliability, useful payload, compute, state, and
time ledgers must not double count. Aggregates need a path back to source
records or an explicit analytical derivation.

## FIPS upstream boundary

The adjacent or separately checked-out FIPS repository is an oracle and source
of protocol truth; it is not vendored into this repository.

- Pin all conformance claims to a FIPS commit.
- Keep upstream API/refactor changes in a separate FIPS branch and pull request.
- Provide a no-upstream-change fallback for experiments until an upstream seam
  is accepted.
- A simulator/daemon match does not prove correctness when both share code;
  retain independent-model checks for load-bearing semantics.

## Validation

Run the checks appropriate to the affected slice. As the implementation lands,
the expected baseline will include formatting, linting, unit tests, schema and
golden-fixture validation, deterministic replay, accounting reconciliation,
and targeted benchmarks.

Attach the smallest useful evidence to the pull request: a test, artifact hash,
report excerpt, minimized reproduction, or benchmark with its environment.

## Security and disclosure

Do not publish private keys, credentials, private topology data, or unredacted
host paths in reproduction bundles. Follow [SECURITY.md](SECURITY.md) for
vulnerability reports.
