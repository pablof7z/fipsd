# Authenticated Sybil admission

`attach-authenticated-sybils` turns reserved stable node slots into
authenticated, protocol-valid participants. Each identity is a separate
virtual-time event with its own address, transport profile, bandwidth,
attachment edge, attacker debit, TreeAnnounce requests, Bloom requests, and
causal descendants.

The event accepts:

- `count`, bounded to 100,000 and to the available node population;
- `interval`, including zero for simultaneous admission;
- `attachment`: current root, hub, leaf, articulation point, or seeded random;
- `address_policy`: `uniform-valid` or `lower-than-current-root`;
- `operations_per_identity`.

Identity and operation totals are checked against `adversaries.budgets` before
the first event executes. Every accepted identity records
`authenticated-identities`, `attacker-operations`, and
`signature-verifications` in the causal ledger. Malformed-wire behavior is
explicitly false and remains outside this subsystem.

Authentication is operation-counted. The engine does not execute handshake
cryptography or admission-policy wall time, and the artifact declares that
boundary as `authenticated-sybil-admission-v1`.

This slice does not silently imply selective visibility or withheld
forwarding. Those remain explicit adversary policies in campaign search; they
need separate graph-native forwarding/admission interventions before the
animated individual engine can claim to execute them.
