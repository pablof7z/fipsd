# v0.1 quick start

FIPS Wind Tunnel requires Rust 1.85 or newer. The release package contains the
`fips-wind-tunnel` CLI, static browser assets, schemas, documentation, and
qualification evidence. The CLI never starts a server unless the operator does
so explicitly.

## Build or install

From a source checkout:

```bash
cargo build --locked --release -p fips-cli --bin fips-wind-tunnel
install -m 0755 target/release/fips-wind-tunnel "$HOME/.local/bin/"
fips-wind-tunnel --help
```

To validate the exact packaged surface, run `scripts/check-clean-install.sh`.
It creates an isolated temporary prefix, exercises the installed binary, and
removes it.

## First run and report

```bash
fips-wind-tunnel validate examples/m1/root-ratchet-12.yaml
fips-wind-tunnel run examples/m1/root-ratchet-12.yaml --output run-12
fips-wind-tunnel inspect run-12/artifact.json
fips-wind-tunnel analyze export run-12/artifact.json --output run-12-report
```

Open `run-12-report/index.html` directly. The browser reads immutable evidence;
it does not execute protocol behavior. Exact, aggregate, cohort, and hybrid
representations remain labeled.

## Sweeps, search, and shrinking

```bash
fips-wind-tunnel campaign plan examples/m3/root-ratchet-search.yaml \
  --mode covering --strength 2 --output plan.json
fips-wind-tunnel campaign search plan.json \
  --maximum-evaluations 6 --output search.json
fips-wind-tunnel campaign shrink search.json --output minimized.json
```

Run `fips-wind-tunnel campaign --help` for Cartesian, covering, Monte Carlo,
checkpoint, corpus, and shrink commands. Generated cases are ordered by stable
case identity, not worker completion order.

## Scale and variants

```bash
fips-wind-tunnel scale run examples/m4/billion-root-ratchet.yaml --output cohort
fips-wind-tunnel scale compare examples/m4/billion-root-ratchet.yaml \
  --output variants.json
fips-wind-tunnel scale sample examples/m4/billion-root-ratchet.yaml \
  --nodes 16 --output sampled-region
```

One-billion-node runs are cohort or hybrid evidence. They are never described
as one billion allocated individual nodes.

## Daemon oracle

The real-daemon backend is explicit because it builds and runs pinned external
software and containers:

```bash
fips-wind-tunnel oracle import fixtures/m5/chaos/smoke.yaml \
  --output imported.json
fips-wind-tunnel oracle compile imported.json --output compiled.yaml
scripts/check-m5-daemon.sh /path/to/pinned-fips /tmp/fips-oracle
```

An absent daemon observation is `unsupported-observation`, not a match or zero.

## Atlas and release evidence

```bash
fips-wind-tunnel atlas verify fixtures/m7/qualification-atlas.json
fips-wind-tunnel release verify-audit fixtures/m8/release-audit.json
fips-wind-tunnel release benchmark --iterations 30 --output benchmark.json
scripts/package-release.sh /tmp/fips-wind-tunnel-0.1.0
```

The package command emits `release-manifest.json`, `checksums.sha256`, and an
SPDX 2.3 SBOM. Set `FIPSD_COSIGN_KEY` to sign the checksum file with `cosign`.
Hosted release artifacts require platform attestation.

## Custom campaigns and artifact API

Start with `examples/root-ratchet.yaml` or one of the ten files under
`examples/campaigns/`. The normative API is
`experiments.fips.network/v1alpha1`; unknown fields fail validation. Normalize
before inspecting selectors:

```bash
fips-wind-tunnel normalize custom.yaml --output custom.normalized.json
```

Run artifacts are immutable JSON documents. Consumers should validate
`manifest.api_version`, fidelity, provenance, total event order, external blob
paths, sizes, and checksums before querying. See [Artifact format](artifact-format.md)
and [Fidelity and provenance](fidelity-and-provenance.md).
