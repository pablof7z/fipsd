# M4 acceptance and verification map

| Issue | Acceptance evidence |
| --- | --- |
| [#38](https://github.com/pablof7z/fipsd/issues/38) | Cohort tests cover conserved depth/degree/transport/resource/region/state populations, closed-form projections, shared artifact metrics, methods, assumptions, and bounds. |
| [#39](https://github.com/pablof7z/fipsd/issues/39) | Bloom tests match analytical occupancy, bound FPR, retain exact sample bits, and reconcile exact/aggregate boundary population and lineage. |
| [#40](https://github.com/pablof7z/fipsd/issues/40) | Hybrid tests exercise all four sampling policies, standalone individual replay, sampled-region fidelity, causal transitions, and no-double-count population totals. |
| [#41](https://github.com/pablof7z/fipsd/issues/41) | Crypto tests cover all five modes, semantic equivalence, explicit execute-scale rejection, deterministic budgets, and complete host/sample/revision/date calibration provenance. |
| [#42](https://github.com/pablof7z/fipsd/issues/42) | One typed interface versions root eligibility/adoption, parent, Bloom, lookup, timer, and coordinate hooks; variant identity/parameters enter hashes and mixed versions fail. |
| [#43](https://github.com/pablof7z/fipsd/issues/43) | Baseline and two explicitly experimental variants share the engine, pass common assertions, and emit decision-attributed differential reports. |
| [#44](https://github.com/pablof7z/fipsd/issues/44) | Calibration covers matched seeds/topologies/scales and publishes error distributions, ranges, and automatic warnings for all seven headline metric families. |
| [#45](https://github.com/pablof7z/fipsd/issues/45) | The checked 18-scenario demo represents one billion nodes with at most 64 cohorts, sensitivity bounds, a replayable 16-node anomaly, and a 64 MiB/30 s budget. |

Run `scripts/check.sh` to regenerate and compare every checked M4 report and
artifact from a clean checkout.
