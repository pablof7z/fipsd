# Security policy

## Project status

FIPS Wind Tunnel is currently a roadmap-stage research and engineering project.
There are no supported releases yet. Security boundaries will evolve before
v0.1, but reports concerning repository automation, campaign/artifact parsing,
or the planned daemon oracle are welcome now.

## Reporting a vulnerability

Use GitHub private vulnerability reporting for this repository. Please do not
open a public issue for a vulnerability that could expose a host, secret,
identity, or denial-of-service technique before maintainers can assess it.

Include the affected revision, environment, minimal reproduction, impact, and
whether untrusted campaign or artifact input is required. Avoid attaching real
private keys or credentials.

## Planned trust boundary

The P0 threat model covers:

- untrusted campaign YAML and run artifacts;
- archive extraction, path traversal, decompression, and allocation limits;
- protocol-variant or plugin code;
- malformed-wire fuzz inputs and crash artifacts;
- Docker-based execution of real FIPS daemons;
- generated identities and private key material;
- artifact export, redaction, and public reproduction bundles;
- CPU, memory, disk, queue, and telemetry exhaustion.

The semantic simulator is not a sandbox. The real-daemon backend may execute
containers and make local network changes. Those operations must be explicit,
bounded, and separated from read-only artifact analysis.

## Reproduction-bundle rule

Public bundles must exclude private keys, credentials, environment secrets,
private host paths, and private network data by default. Provenance should use
hashes, public commit IDs, image digests, and redacted hardware profiles.
