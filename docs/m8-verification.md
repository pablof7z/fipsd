# M8 v0.1 verification map

| Contract | Executable evidence |
| --- | --- |
| Cross-host/worker/toolchain determinism policy | `fips-audit::DeterminismAudit`, CI matrix, three-repeat hashes |
| Accounting across fidelities | `fips-audit::AccountingAudit`, M2/M4/M5 assertions and explicit exclusions |
| Measured performance | `release benchmark`, `fixtures/m8/benchmark-macos-aarch64.json` |
| Install, tutorial, schemas, interpretation | `docs/quickstart.md`, checked-in schemas, `scripts/check-clean-install.sh` |
| Threat model and untrusted inputs | `docs/threat-model.md`, package and artifact path/size tests |
| CLI/browser package, SBOM, checksums | `scripts/package-release.sh`, `release manifest`, `release verify-package` |
| Campaign atlas and limitations | `fixtures/m7/qualification-atlas.json`, `docs/support-matrix.md` |

The release gate is:

```bash
scripts/check.sh
scripts/check-clean-install.sh
cargo deny check
scripts/check-fips-codecs.sh --check
```

`fixtures/m8/release-audit.json` is byte-stable and is regenerated and compared
by the normal check. A hosted release additionally requires platform binaries,
GitHub artifact attestation, checksums, the SPDX SBOM, and the release notes.
