# M3 acceptance and verification map

M3 is accepted by the `fips-campaign` integration suite, checked evidence under
`fixtures/m3/`, the active regression under `fixtures/corpus/`, and the CLI
loop in `scripts/check.sh`.

| Issue | Acceptance evidence |
| --- | --- |
| [#28](https://github.com/pablof7z/fipsd/issues/28) | Algebra tests prove normalized axis resolution, stable IDs independent of selection order, derived defaults, compatibility/constraint diagnostics, and bounded matrix compilation. |
| [#29](https://github.com/pablof7z/fipsd/issues/29) | Planner tests prove 32-case Cartesian enumeration, complete 40-interaction pairwise coverage in six cases, and seed-exact stratified Monte Carlo replay. |
| [#30](https://github.com/pablof7z/fipsd/issues/30) | Generator tests prove connected/disconnected degree constraints, every event family, exact seed replay, composable shrinking, and bounded symbolic million-node generation. |
| [#31](https://github.com/pablof7z/fipsd/issues/31) | Policy tests cover versioned UDP, TCP, Ethernet, BLE, Tor, and Nym profiles, provenance, assignment strategies, effective MTU, and explicit failover lineage. |
| [#32](https://github.com/pablof7z/fipsd/issues/32) | Adversary tests cover every authenticated policy, free/operation/calibrated/bounded budgets, deterministic exhaustion, and accepted/rejected/dishonest dispositions. |
| [#33](https://github.com/pablof7z/fipsd/issues/33) | Search tests prove objective and constraint handling, immutable resume, protocol-valid filtering, Pareto ranking, and complete artifact/reproduction provenance. |
| [#34](https://github.com/pablof7z/fipsd/issues/34) | Shrink tests exercise all nine ordered dimensions, parallel cached trials, million-node reduction, metric preservation, and standalone replay. |
| [#35](https://github.com/pablof7z/fipsd/issues/35) | Runner tests prove stable outcomes across worker counts, case/memory/disk budgets, cancellation, panic isolation, deduplication, checkpoints, resume, and honest partial reports. |
| [#36](https://github.com/pablof7z/fipsd/issues/36) | Corpus tests prove promotion provenance, fidelity/daemon labels, batch replay, semantic ranges, retirement, and explicit expectation-change approval. |

Run `scripts/check.sh` for the complete clean-checkout gate. It regenerates the
covering plan and search result byte-for-byte, exercises partial/resumed parallel
execution, and replays every active regression entry.
