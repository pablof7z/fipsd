# M1 acceptance and verification map

M1 is accepted by executable tests and checked artifacts. This map ties each
child issue to the evidence that closes its acceptance surface.

| Issue | Acceptance evidence |
| --- | --- |
| [#10](https://github.com/pablof7z/fipsd/issues/10) | `scheduler` tests stable ordering at 499/500/501 ms, keyed coalescing, recursive cancellation, and bounded pending capacity. |
| [#11](https://github.com/pablof7z/fipsd/issues/11) | `graph` uses stable integer IDs and structure-of-arrays columns, rejects dangling/duplicate/cyclic state, serializes deterministically, and reports the 39-byte/node plus 8-byte/edge fixed-width footprint. |
| [#12](https://github.com/pablof7z/fipsd/issues/12) | Explicit, chain, balanced-tree, seeded random-regular, and scale-free generators plus all arrival attachment selectors have deterministic tests and checked golden hashes. |
| [#13](https://github.com/pablof7z/fipsd/issues/13) | The pinned-FIPS root/parent/ancestry model tests loop rejection, root agreement, hysteresis/hold-down, and mandatory better-root bypass. |
| [#14](https://github.com/pablof7z/fipsd/issues/14) | TreeAnnounce tracks requested, superseded/coalesced, cancelled, constructed, signed, serialized, queued, transmitted, delivered, and rejected stages; tests enforce the exact debounce boundary and codec-derived `168 + 32d` bytes. |
| [#15](https://github.com/pablof7z/fipsd/issues/15) | The shared link service tests bandwidth serialization, latency, MTU, bounded queues, deterministic loss/duplication, ordering, byte reconciliation, and separate control/useful accounting. |
| [#16](https://github.com/pablof7z/fipsd/issues/16) | Tests cover one-lower generation, strict and precomputed ladders, bounded attacker work, simultaneous arrivals, deterministic attachment, and disappearance/reappearance convergence. |
| [#17](https://github.com/pablof7z/fipsd/issues/17) | `run`, `inspect`, and `replay` produce schema-valid immutable evidence; the checked valid fixture replays byte-for-byte and the broken fixture fails the named loop-freedom invariant. |

Run `scripts/check.sh` for the complete local gate. It includes format and
lint checks, every workspace test, deterministic M0 normalization, an M1 run
and replay byte comparison, and proof that the deliberately broken campaign
fails loudly.
