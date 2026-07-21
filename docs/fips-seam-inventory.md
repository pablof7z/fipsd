# FIPS seam inventory at `80c956a`

This inventory is pinned to FIPS commit
`80c956a6fdb85dde1450969a21891c1158e43267`. Paths and visibility were checked
against that tree. “Public” means reachable by a downstream crate through
`fips::`; Rust `pub` items inside the crate-private `fips::proto` module are not
public downstream seams.

## Protocol surfaces

| Area | Semantic/core and state | Wire and limits | Downstream classification | Gap |
| --- | --- | --- | --- | --- |
| STP | `src/proto/stp/core.rs`, `state.rs`, `declaration.rs`, `limits.rs` | `src/proto/stp/wire.rs`, `src/proto/coord.rs` | `TreeState`, `ParentDeclaration`, coordinates, and `TreeAnnounce` are re-exported; decision core and dampener are crate-private | No public pure classify/election API; signing and send effects stay in `src/node/tree.rs` |
| Bloom | `src/proto/bloom/core.rs`, `state.rs` | `src/proto/bloom/wire.rs`, `limits.rs` | Filter/state/wire types are re-exported and reusable | Propagation and peer effects remain in `src/node/bloom.rs`; debounce decisions are not exposed as a stable adapter API |
| Lookup | `src/proto/lookup/core.rs`, `state.rs`, `limits.rs` | `src/proto/lookup/wire.rs` | `LookupRequest` and `LookupResponse` are re-exported; planners, routing-view trait, dedup, retries, and rate limits are crate-private | Public wire seam only; handler effects remain in `src/node/handlers/lookup.rs` |
| Routing | `src/proto/routing/core.rs`, `state.rs`, `limits.rs` | `src/proto/routing/wire.rs` | `CoordsRequired`, `PathBroken`, `MtuExceeded`, and fixed sizes are re-exported | Routing planners/state are crate-private; forwarding and cache-touch effects remain in the node/dataplane shell |
| FMP | `src/proto/fmp/core.rs`, `state.rs`, `limits.rs` | `src/proto/fmp/wire.rs` | Only `HandshakeMessageType`, `PromotionResult`, and `cross_connection_winner` are re-exported | Established framing constants/builders and the FMP state machine are crate-private; crypto/queue effects live in peer/node workers |
| FSP | `src/proto/fsp/core.rs`, `limits.rs` | `src/proto/fsp/wire.rs` | Session setup/ack/flags/message types are re-exported | Rekey/epoch/queue core is crate-private; crypto session state lives in `src/node/session/mod.rs` and handlers |
| MMP | `src/proto/mmp/core.rs`, `state.rs`, algorithms/metrics/sender/receiver/path-MTU modules | `src/proto/mmp/wire.rs`, `limits.rs` | No downstream seam because `proto` is crate-private and MMP types are not re-exported from `lib.rs` | Report codecs and pure planners require a minimal public adapter; send/backoff effects remain in `src/node/handlers/mmp.rs` |
| Coordinate cache | `src/cache/coord_cache.rs`, `entry.rs` | N/A | `CoordCache`, entries, errors, and stats are public/reusable data structures | TTL touches, invalidation triggers, and warmup choreography are node-shell coupled |
| Sessions | `src/node/session/mod.rs`, `src/noise/session.rs` | FSP wire above | Noise/session crypto types are public, but live session registry and lifecycle are shell-coupled | No pure public session transition seam; normalize FSP actions rather than sharing the registry |

The crate boundary is the principal blocker: `src/lib.rs` declares
`pub(crate) mod proto`, and `src/proto/mod.rs` declares each subsystem
`pub(crate)`. Selected data/wire types are deliberately re-exported, but the
pure decision cores are not.

## Clock and side-effect boundary

- `src/time.rs` owns process time and is private. The shell calls `mono_ms()`.
- Sans-I/O cores accept injected `now_ms: u64`; STP hold-down/flap logic,
  lookup retry/rate limiting, FSP timers, and MMP reporting therefore have a
  usable deterministic clock seam even where visibility is crate-private.
- Network sends, signing/encryption, registry mutation, queues, metrics, and
  tracing remain in `src/node/**`, `src/peer/**`, and worker modules. They are
  shell effects, not reusable protocol decisions.

## Telemetry and oracle inputs

The control protocol is under `src/control/{commands,queries,snapshot}.rs`.
Versioned example snapshots already cover status, tree, Bloom, cache, identity
cache, routing, sessions, MMP, links, peers, transports, connections, ACL, and
statistics under `src/control/snapshots/*.json`.

The real-daemon harness is documentation/runner input rather than an engine
API. `testing/chaos/` and `testing/mesh-lab/` own topology/container lifecycle,
fault injection, runner logs, and assertions. They are retained for M5 oracle
validation and must not define Campaign v1alpha1.

## Smallest upstream exposure proposal

Add a public, versioned `protocol_api` facade rather than making all of `proto`
public. Initially it should re-export:

1. FMP/FSP header sizes and pure build/parse helpers;
2. link, STP, Bloom, lookup, routing, and MMP wire codecs;
3. narrow snapshot/action types and pure `classify`/`plan` functions for one
   subsystem at a time;
4. the injected monotonic-millisecond input type/contract.

This leaves node, peer, crypto ownership, transport, logging, and registries
private. Each added surface must have a conformance fixture and can be promoted
incrementally.

## No-upstream-change fallback

M0 uses a generated conformance harness against a clean checkout of the pinned
commit. The harness temporarily exposes only the private protocol modules in
that disposable checkout, runs the production encoders, and compares their
JSON output with the checked-in manifest. The product crates consume the
manifest, never copied wire constants. A pin or codec change therefore fails
the drift gate until the manifest and formulas are reviewed together.

## Shared-bug risk and normalization

Production codec bytes are authoritative for wire size, but shared semantic
code is not its own oracle. Independent and shared-core models emit normalized
transitions containing input event, pre-state digest, decision, ordered
effects, post-state digest, virtual time, and semantic-version/FIPS commit.
Differential reports compare that vocabulary and identify the first divergence.
Minimized cases are then checked against a real daemon. A shared-core match can
establish consistency, not correctness by itself.

## Recorded documentation drift

At the pinned commit, executable FMP construction adds 16 outer-header bytes,
4 timestamp bytes, and a 16-byte AEAD tag to an already message-typed plaintext:
36 bytes total. `docs/reference/wire-formats.md` describes 37 bytes by counting
the message-type byte again. Consequently executable sizes are
`168 + 32 * depth` for TreeAnnounce and 1,071 bytes for FilterAnnounce, while
that document lists `169 + 32 * depth` and 1,072. The M0 codec manifest and
tests use executable results and preserve the mismatch as upstream drift.

