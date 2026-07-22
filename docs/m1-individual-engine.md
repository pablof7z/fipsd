# M1 deterministic individual engine

M1 is a headless, compact discrete-event implementation of the Root Ratchet
vertical slice. It uses stable integer node and edge IDs, structure-of-arrays
state, injected nanosecond virtual time, and a monotonically assigned ordinal
to make every tie deterministic. No browser or app state participates in a
run.

## Current-FIPS semantic boundary

The independent model is versioned against FIPS commit
`80c956a6fdb85dde1450969a21891c1158e43267`. It implements minimum-address root
selection, loop-free ancestry, root-correcting mandatory parent changes,
depth/cost ranking for discretionary changes, hysteresis, and hold-down. A
better root bypasses discretionary suppression, matching the pinned STP core.

TreeAnnounce requests are tracked separately from superseded, coalesced,
cancelled-before-construction, constructed, signed, serialized, queued,
transmitted, and delivered stages. The per-peer boundary is exactly 500 ms.
Serialized bytes use the executable codec formula `168 + 32d`; link service
then adds the declared transport overhead before checking MTU, serialization
delay, and queue capacity.

## Compact graph storage

The fixed-width columns account for 39 bytes per node and 8 bytes per edge on
the supported 64-bit Rust targets. Variable ancestry storage is reported from
actual vector capacity in every run. The checked-in 12-node chain allocates 772
bytes for graph columns and ancestry after convergence; peer views, scheduler
events, and artifact inspection records are separate and explicitly measured
by later performance work.

## Reproducible demo

```bash
cargo run -p fips-cli --bin fips-wind-tunnel -- \
  run examples/m1/root-ratchet-12.yaml --output runs/m1-root-ratchet-12

cargo run -p fips-cli --bin fips-wind-tunnel -- \
  replay runs/m1-root-ratchet-12/reproduction.json \
  --output runs/m1-root-ratchet-12/replay.json

cmp runs/m1-root-ratchet-12/artifact.json \
  runs/m1-root-ratchet-12/replay.json
```

The fixture activates eight initial nodes and four strictly descending roots.
Its run reports root agreement, adopted root generations, maximum depth, parent
transitions, every TreeAnnounce stage and byte, per-edge queues, quiescence, and
all required invariants. `root-ratchet-12-broken.yaml` injects a named ancestry
loop and must fail with the `loop-freedom` invariant.

The M1 `minimize-bundle` command only validates and preserves a bundle. The
hierarchical shrinker replaces that compatibility behavior in M3.
