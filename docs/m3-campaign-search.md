# M3 deterministic campaign search and shrinking

M3 turns one normalized campaign into a bounded, replayable search. It adds a
stable case algebra, Cartesian and covering-array planners, seeded stratified
Monte Carlo sampling, property-generated topologies and event sequences,
transport and authenticated-adversary policies, resumable objective search,
parallel budgeted execution, hierarchical shrinking, and a reviewed regression
corpus.

The checked Root Ratchet search has five two-value axes. Its full matrix is 32
cases; the deterministic pairwise covering plan executes six cases while
covering all 40 requested interactions. Every successful evaluation retains the
complete run artifact and standalone reproduction bundle. Search checkpoints
are append-only by case ID, so resuming cannot alter completed evidence.

## Run the milestone loop

```bash
cargo run -p fips-cli --bin fips-wind-tunnel -- campaign plan \
  examples/m3/root-ratchet-search.yaml --mode covering --strength 2 \
  --output /tmp/m3-plan.json

cargo run -p fips-cli --bin fips-wind-tunnel -- campaign search \
  /tmp/m3-plan.json --maximum-evaluations 6 --output /tmp/m3-search.json

cargo run -p fips-cli --bin fips-wind-tunnel -- campaign replay-corpus \
  fixtures/corpus --output /tmp/m3-corpus-report.json
```

`campaign execute` accepts worker, case-count, memory, disk, cancellation, and
checkpoint controls. `campaign shrink` preserves a named metric threshold while
trying traffic, topology regions, nodes, edges, root transitions, event timing,
protocol parameters, resource classes, and transports in a fixed hierarchy.
The result includes every predicate trial and a standalone minimized bundle.

## Scientific boundaries

- Large generated populations are symbolic: the checked million-node input
  materializes at most 256 sample nodes and labels that representation.
- Adversaries remain authenticated and protocol-valid. Each action records its
  policy, budget debit, acceptance or rejection, and interpretive disposition.
- Transport profiles are versioned abstractions with source provenance,
  effective MTU/overhead, assignment policy, and failover lineage.
- Regression entries distinguish model-only cases from daemon-confirmed cases.
  M5, not M3, supplies the daemon confirmation.
- Promotion never changes expected assertions without an explicit reviewed
  update flag; retired entries remain documented but do not run.
