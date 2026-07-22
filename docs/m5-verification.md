# M5 verification map

M5's epic is [#46](https://github.com/pablof7z/fipsd/issues/46). The normal gate
is `scripts/check.sh`; the real-daemon gate is `scripts/check-m5-daemon.sh`.

| Issue | Acceptance evidence |
| --- | --- |
| [#47 Import chaos scenarios](https://github.com/pablof7z/fipsd/issues/47) | `importer.rs`, six `fixtures/m5/chaos/*.yaml` families, and `import_compile.rs` prove deterministic import, source commit/hash, raw preservation, and field-level loss diagnostics. |
| [#48 Compile to chaos](https://github.com/pablof7z/fipsd/issues/48) | `compiler.rs` and compile tests prove deterministic topology, identities, timing, transport, faults, and actionable rejection of dynamic identity and cohort semantics. |
| [#49 Normalize telemetry](https://github.com/pablof7z/fipsd/issues/49) | `telemetry.rs`, `normalized-telemetry.json`, and telemetry tests cover tree, parent, ancestry, Bloom, cache, sessions, MMP, queue, traffic, frames, assertions, raw pointers, clocks, unknown values, and adapter-version drift. |
| [#50 Capture provenance](https://github.com/pablof7z/fipsd/issues/50) | `provenance.rs`, the process backend, provenance tests, and `live-smoke-summary.json` require exact commit, dirty patch digest, binary/version/image/config/runtime/host identity, and recursive public-bundle redaction. |
| [#51 Differential oracle](https://github.com/pablof7z/fipsd/issues/51) | `differential.rs`, `differential.json`, and differential tests cover first transition, semantic/frame/metric/timing evidence, implementation/model/environment classifications, and the rule that unobserved is never a match. |
| [#52 One-command reproduction](https://github.com/pablof7z/fipsd/issues/52) | `oracle run-chaos`, `oracle.rs`, `process_backend.rs`, recorded repeat tests, and the three-repeat live summary prove compile/run/ingest/compare, confidence, attached evidence, and stable-only corpus confirmation. |
| [#53 Invalid-wire fuzz adapter](https://github.com/pablof7z/fipsd/issues/53) | `fuzz.rs`, `fuzz-crash.json`, and fuzz tests prove backend/codec/corpus/coverage provenance, standalone crash/hang replay, and separation from semantic execution. |
| [#54 Continuous oracle suites](https://github.com/pablof7z/fipsd/issues/54) | `suites.rs`, `suites.json`, suite tests, normal CI, and `oracle.yml` define PR smoke, nightly Bloom calibration, known-good/known-bad history, budgets, cache keys, classifications, and failure retention. |

The generated files under `fixtures/m5/generated/` are regenerated in a fresh
directory and recursively diffed on every normal gate. The live summary is a
small, secret-free provenance record; raw live logs remain CI artifacts or in
the explicitly selected local output directory.
