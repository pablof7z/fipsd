# M2 coupled recovery and causal accounting

M2 extends the deterministic individual engine across Bloom replacement,
coordinate-cache invalidation, lookup, sessions, useful payload, node resource
service, and the same bounded link queue used by control traffic. It remains
headless: the engine emits immutable JSON and the CLI only runs, replays, or
inspects that evidence.

## Reproducible demo

```bash
cargo run -p fips-cli --bin fips-wind-tunnel -- \
  run examples/m2/root-ratchet-recovery.yaml --output runs/m2-recovery

cargo run -p fips-cli --bin fips-wind-tunnel -- \
  replay runs/m2-recovery/reproduction.json \
  --output runs/m2-recovery/replay.json

cmp runs/m2-recovery/artifact.json runs/m2-recovery/replay.json

cargo run -p fips-cli --bin fips-wind-tunnel -- \
  inspect runs/m2-recovery/artifact.json \
  --causal-id input:arrival-0000
```

The fixture carries useful traffic beyond final root convergence. Its report
separates root, tree, Bloom, lookup, and throughput quiescence; per-arrival
Bloom and cache amplification; lookup outcomes and signals; goodput and
framing; CPU/state/queue budgets; and the dominant critical path.

## Fidelity boundary

`exact-bits` stores the production-sized packed filter, `sparse-bits` stores
exact set-bit indices until its published crossover, and `occupancy` uses
seeded statistical membership draws. The artifact always records the selected
mode. Wire sizes are executable-codec values pinned to FIPS commit
`80c956a6fdb85dde1450969a21891c1158e43267`; cryptographic and Bloom work is
operation-counted, not wall-clock calibrated.

## Ledger reconciliation

Stable causal IDs and `causal_parent` links connect arrivals, traffic flows,
lookup retries, frames, resource receipts, state changes, and aggregate time.
The report reconciles semantic actions, offered payload, FSP bytes, FMP bytes,
transport overhead, network and reliability bytes, useful delivery, compute,
state, lower bounds, duplicates, retransmissions, and superseded work. Message,
edge, resource-kind, and depth-band projections are checked independently.

The `continuous-control-eventual-data-progress` assertion requires a useful
delivery after root convergence whenever the offered workload extends beyond
that marker. Cache invalidations are compared with inserted entries, not byte
counts, and every successful modeled resource consumption has a receipt.
