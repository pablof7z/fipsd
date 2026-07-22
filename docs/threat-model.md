# Threat model and safe-execution boundary

## Trust zones

The validator, artifact reader, static analysis browser, package verifier, and
daemon oracle have different authority. Read-only analysis never inherits the
container or host-network authority of the oracle.

| Surface | Input | Authority | Primary controls |
| --- | --- | --- | --- |
| Campaign parser | Untrusted YAML | Process memory and output path | Schema, unknown-field rejection, bounded values |
| Artifact query | Untrusted JSON and declared blobs | Read-only artifact directory | Version validation, normalized relative paths, checksums, 64 MiB browser limit |
| Static export | Validated artifact | Explicit output directory | No plugins, size limit, escaped text, no engine execution |
| Reproduction bundle | Untrusted JSON | Selected engine | Schema/fidelity validation and explicit backend selection |
| Daemon oracle | Campaign plus pinned FIPS checkout | Docker, processes, loopback network | Explicit command, pinned commit/image, timeouts, bounded repetitions |
| Release package | Staged files | Read-only inventory plus manifest output | No symlinks, per-file/total limits, SHA-256, SBOM, attestation |

## Protected assets

Private keys, OAuth tokens, environment secrets, private host paths, Docker
credentials, unpublished network addresses, and raw host profiles must not enter
public artifacts. Public provenance uses public commits, digests, schema
versions, seeds, and redacted hardware profile identifiers.

## Parser and resource controls

- Campaigns reject unsupported scale/fidelity combinations.
- Artifact external blobs reject absolute paths, parent traversal, size drift,
  and checksum drift.
- Browser imports reject files above 64 MiB and parse in a Web Worker.
- Static export defaults to 64 MiB and never follows artifact-declared paths.
- Package inventory rejects symlinks, files above 256 MiB, and packages above
  1 GiB.
- Engine, search, oracle, and package workflows use explicit event, evaluation,
  repetition, wall-time, or allocation budgets.
- Partial immutable evidence is retained when a bounded run fails; a resource
  failure is not silently converted to a result.

## Plugins and variants

The static browser does not execute plugins. Rust protocol variants are trusted
code built into the CLI and identified in provenance. A future dynamic plugin
surface would require a separate signing, capability, and sandbox design; v0.1
does not claim one.

## Containers and network

The semantic engines do not need network access. The real-daemon oracle may
build software, run containers, bind local ports, and create an isolated test
network. Operators must inspect the pinned checkout and command before use.
Production identities, relays, or networks are out of scope.

## Residual risks

JSON/YAML parsers can still consume CPU before semantic limits are known. The
portable benchmark does not measure peak RSS. Host Docker configuration and
third-party toolchains remain outside the semantic determinism boundary. Report
security defects through the private process in the repository `SECURITY.md`.
