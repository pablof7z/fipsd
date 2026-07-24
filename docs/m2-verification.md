# M2 acceptance and verification map

M2 is accepted by deterministic unit tests, the checked campaign and evidence
under `fixtures/m2/`, and `m2_acceptance.rs` replay/schema/causal-tree checks.

| Issue | Acceptance evidence |
| --- | --- |
| [#19](https://github.com/pablof7z/fipsd/issues/19) | Bloom tests prove exact/sparse agreement before crossover, occupancy against analytical fill and seeded ensembles, and explicit fidelity metadata in every report. |
| [#20](https://github.com/pablof7z/fipsd/issues/20) | Split-horizon parent/child/mesh counters cover 499/500/501 ms boundaries, one/two-direction waves, full-replacement codec bytes, coalescing, FPR rejection, and MTU rejection. |
| [#21](https://github.com/pablof7z/fipsd/issues/21) | Cache tests cover node/root/path invalidation, touch/expiry, capacity thrash, memory, misses, insertions, invalidations, and warmup bytes. |
| [#22](https://github.com/pablof7z/fipsd/issues/22) | Lookup tests cover deterministic retry parentage, dedup, backoff/jitter, rate-limited signals, production sizes and MTU, reverse-path failure, and TTL 63/64/65/beyond. |
| [#23](https://github.com/pablof7z/fipsd/issues/23) | Seed-stable tests cover all thirteen traffic models, including segmented streams and synchronized bursts, offered-load reconciliation, setup/teardown/rekey, idle and saturated baselines, and separate useful payload. |
| [#24](https://github.com/pablof7z/fipsd/issues/24) | Resource tests cover every configured dimension, receipts, CPU competition, typed causal exhaustion, heterogeneous slow-root/slow-leaf service, and pauses. |
| [#25](https://github.com/pablof7z/fipsd/issues/25) | The checked report has exact layer totals and message/edge/resource/depth projections; `inspect --causal-id` recursively expands a selected arrival through parent-linked ledger rows. |
| [#26](https://github.com/pablof7z/fipsd/issues/26) | The checked report publishes all five markers, depth adoption, intermediate roots, queue/goodput/frame/cache/control ratios, per-arrival amplification, a ledger-backed critical path, and post-convergence data progress. |

Run `scripts/check.sh` for the complete clean-checkout gate, including M2 run,
recursive inspection, replay, and byte comparison.
