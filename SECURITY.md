# Security policy

## Supported versions

| Version | Supported |
| --- | --- |
| 0.1.x | Yes |
| Pre-release snapshots | Best effort |

The v0.1 trust zones, parser/resource limits, container authority, and residual
risks are documented in [the threat model](docs/threat-model.md).

## Reporting a vulnerability

Use GitHub private vulnerability reporting for this repository. Please do not
open a public issue for a vulnerability that could expose a host, secret,
identity, or denial-of-service technique before maintainers can assess it.

Include the affected revision, environment, minimal reproduction, impact, and
whether untrusted campaign or artifact input is required. Avoid attaching real
private keys or credentials.

## Trust boundary

The v0.1 threat model covers:

- untrusted campaign YAML and run artifacts;
- archive extraction, path traversal, decompression, and allocation limits;
- protocol-variant or plugin code;
- malformed-wire fuzz inputs and crash artifacts;
- Docker-based execution of real FIPS daemons;
- generated identities and private key material;
- artifact export, redaction, and public reproduction bundles;
- CPU, memory, disk, queue, and telemetry exhaustion.

The semantic simulator is not a sandbox. The real-daemon backend can execute
containers and make local network changes. Those operations must be explicit,
bounded, and separated from read-only artifact analysis.

## Reproduction-bundle rule

Public bundles must exclude private keys, credentials, environment secrets,
private host paths, and private network data by default. Provenance should use
hashes, public commit IDs, image digests, and redacted hardware profiles.

Release packages reject symlinks, parent traversal, oversized files, and
checksum drift. Hosted packages require SHA-256 checksums, an SPDX SBOM, and
platform artifact attestation. These controls establish provenance; they do not
make untrusted engine or variant code safe to execute.
