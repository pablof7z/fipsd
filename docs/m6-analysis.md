# M6 multi-resolution artifact analysis

`fips-query` is a read-only projection over validated run artifacts. It does
not depend on an engine crate. Exact graphs are selected through 200 represented
nodes; larger individual runs use aggregate views; cohort and hybrid artifacts
retain their declared representation and uncertainty.

The CLI provides deterministic `analyze index`, `query`, `compare`, and
`export` commands. Every metric summary and event query includes a collection,
source range, total count, and fidelity. Bounded event samples preserve the
first and last matched event. Incompatible population comparisons fail unless
the caller explicitly confirms a documented normalization.

The static browser imports local artifacts in a Web Worker, supports deep links,
and renders summary, separate quiescence markers, metrics, causal stages,
critical path, fidelity, and provenance. Static exports bundle the validated
source artifact and analysis JSON. They contain no engine or plugin code.

Acceptance evidence:

- `crates/fips-query/tests/m6_acceptance.rs`
- `fixtures/m6/root-ratchet-analysis.json`
- `web/index.html`, `web/app.js`, `web/worker.js`, and `web/styles.css`

Automated visual browser verification was unavailable on the 2026-07-22 build
host because no browser backend was exposed. Static generation, module syntax,
data contracts, and query behavior are covered by the release gates.
