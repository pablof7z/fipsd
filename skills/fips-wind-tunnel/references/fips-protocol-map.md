# FIPS protocol source map

Use MCP knowledge resources with the `protocol` prefix for authoritative FIPS
material. Prefer executable tests and codecs when prose and behavior disagree.

## Concepts and architecture

- `docs/design/fips-concepts.md`: identities, addresses, coordinates, and terms.
- `docs/design/fips-architecture.md`: system and layer boundaries.
- `docs/design/fips-mesh-layer.md`: mesh-level forwarding behavior.
- `docs/design/fips-mesh-operation.md`: end-to-end mesh operation.

## Tree, routing, and discovery

- `docs/design/fips-spanning-tree.md`: root and parent construction.
- `docs/design/spanning-tree-dynamics.md`: joins, churn, and convergence.
- `docs/design/fips-bloom-filters.md`: reachability summaries and false positives.
- `docs/design/fips-nostr-discovery.md`: discovery integration.
- `docs/reference/wire-formats.md`: encoded protocol forms.

## Sessions, transports, and boundaries

- `docs/design/fips-session-layer.md`: sessions and cryptographic lifecycle.
- `docs/design/fips-transport-layer.md`: transport abstraction and behavior.
- `docs/design/fips-mtu.md`: framing and MTU constraints.
- `docs/reference/transports.md`: supported transport configuration.
- `docs/design/port-advertisement-and-nat-traversal.md`: reachability and NAT.

## Security

- `docs/design/fips-security.md`: threat and security design.
- `docs/reference/security.md`: operational security reference.

When explaining a simulation, also read the Wind Tunnel fidelity and seam
inventory resources. Modeled protocol behavior may intentionally be less exact
than the checked-in FIPS implementation.
