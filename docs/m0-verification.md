# M0 verification map

This page maps the M0 epic and its child issues to reviewable repository
evidence. It is deliberately limited to contracts, deterministic tooling, and
the pinned-codec proof. M0 contains no product UI work: the browser application
begins at M6, and the existing empty macOS shell is outside this milestone.

| Issue | Acceptance evidence |
| --- | --- |
| #2 Product-fork ADRs | [`docs/adr`](adr) records B3, incremental A3, C3, D2, and the CLI/artifact-first surface, including reversal triggers and evidence. |
| #3 FIPS seam spike | [`fips-seam-inventory.md`](fips-seam-inventory.md) maps the pinned `80c956a6fdb85dde1450969a21891c1158e43267` source, clock and telemetry boundaries, the smallest upstream proposal, the no-change fallback, and shared-bug controls. |
| #4 Fidelity and provenance | [`fidelity-and-provenance.md`](fidelity-and-provenance.md), the artifact schema, and `fips-artifact` reject unsupported combinations and generate the plain-language fidelity statement. |
| #5 Campaign v1alpha1 | The strict Campaign schema, semantic validator, ten flagship examples, and invalid fixtures cover the required model, units, defaults, extensions, and actionable failure paths. |
| #6 Artifacts and reproduction | The two artifact schemas and golden fixtures prove deterministic round trips, stable manifest and event ordering, and verified out-of-line blobs. |
| #7 Workspace and CI | The `fips-model`, `fips-engine-api`, `fips-cli`, `fips-artifact`, and `fips-adapter` crates provide the M0 spine. `scripts/check.sh`, `deny.toml`, and the Linux/macOS workflow enforce formatting, linting, tests, schemas, dependencies, and licenses. |
| #8 Pinned codec harness | `scripts/check-fips-codecs.sh --check` compiles an executable probe against the pinned FIPS revision and compares TreeAnnounce, Filter, lookup, routing, FMP, and FSP bytes with the checked-in golden manifest. |

## Local acceptance commands

Run these commands from a clean checkout:

```bash
scripts/check.sh
cargo deny check
scripts/check-fips-codecs.sh --check
```

The codec manifest includes the exact TreeAnnounce boundary depths `0`, `35`,
`64`, `65`, `2000`, and `2043`, which is the maximum encodable FMP depth under
the production `u16` payload length. It also records exact Filter and FMP frame
overhead. Any byte or source-hash drift fails the codec gate.

The CI workflow repeats the repository gate on Linux and macOS, runs dependency
and license policy on Linux, and runs the executable codec check with FIPS's
pinned Rust toolchain.
