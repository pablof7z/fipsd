# Campaigns and live events

Campaign v1alpha1 is the durable experiment definition. Read the live schema
resource before emitting complete JSON.

Core dimensions include metadata, scale, topology, identities, transports,
links, protocol parameters, traffic, resources, events, assertions, objectives,
fidelity, and seeds. Use explicit transfers for concrete application payloads.

Supported live amendment actions include:

- `set-link-conditions`, `restore-link-conditions`
- `introduce-lower-root-node`, `introduce-node`
- `disappear-node`, `reappear-node`
- `partition-network`, `merge-network`
- `fail-transport-class`, `restore-transport-class`
- `synchronized-session-rekey`
- `expire-coordinate-cache`, `simultaneous-lookups`
- `swap-parent-ancestry`, `alternate-parent-quality`
- `attach-authenticated-sybils`

Schedule injected events after the current virtual cursor. Use stable unique
event IDs and explicit virtual times. Node-arrival events require a sufficient
identity and attacker-operation budget; the application adjusts this budget for
supported MCP amendments.

Prefer natural-language amendment when intent is semantic or refers to the
rendered state, such as “make the old nodes disappear until four remain.”
Prefer exact event injection when the caller already knows the action, target,
timing, and parameters.

Relevant product resources include `docs/campaign-v1alpha1.md`,
`docs/natural-language-amendments.md`, `docs/interactive-interventions.md`,
`docs/explicit-application-transfers.md`, and the files under `examples/`.
