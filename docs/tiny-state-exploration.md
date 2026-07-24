# Exhaustive tiny-state exploration

`explore tiny` enumerates every permutation of supported authored input actions
for a concrete individual-node campaign. It is intended for networks small
enough that factorial enumeration is honest and useful.

```bash
fips-wind-tunnel explore tiny examples/tiny-lifecycle-orders.yaml \
  --maximum-nodes 4 \
  --maximum-actions 2 \
  --output runs/tiny-lifecycle
```

The explorer:

- rejects unresolved campaign axes;
- enforces explicit node and action bounds before running;
- schedules each permutation in the same authored timing window with fixed
  virtual-time spacing;
- runs the normal individual semantic engine for every order;
- records every terminal-state signature and artifact identity;
- emits versioned counterexamples for failed assertions or runtime invariants;
- declares coverage exhaustive only when the executed permutation count equals
  the factorial action count.

`report.json` retains the normalized source plan, every action order, terminal
signatures, violations, fidelity, and enumeration counts. Each failure is also
written under `counterexamples/` and can be checked independently:

```bash
fips-wind-tunnel explore replay \
  runs/tiny-lifecycle/counterexamples/counterexample-ID.json
```

The supported alphabet currently includes manual lower-root arrival, node
disappearance/reappearance, partition/merge, link condition change/restore,
synchronized session rekey, and the deliberate loop-injection test action. The
cache-expiry and simultaneous-lookup inputs are also included when the campaign
enables graph-native recovery, and transport-class failure/restoration is
included for mixed-profile campaigns. The scheduled descending-root process
remains fixed campaign context rather than a permuted action.

This mode is exact only over the declared finite action alphabet, node bound,
and timing-window semantics. It does not claim exhaustive exploration of
unbounded identities, payload bytes, wall-clock interleavings, or unsupported
protocol actions.
