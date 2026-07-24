# FIPS Protocol Wind Tunnel

FIPS Wind Tunnel is a deterministic, multi-fidelity protocol experimentation
platform for constructing FIPS networks, injecting stochastic and adversarial
events, observing exact causal propagation, finding protocol limits, comparing
variants, and reducing large failures into replayable cases.

The primary product surface is a native macOS workbench. It renders the same
versioned event stream that is persisted in run evidence; the UI does not own or
reimplement protocol semantics. The existing static browser analysis remains a
secondary forensic surface for completed artifacts.

The authoritative [product specification](docs/product-spec.md) defines the
required engine, campaign, search, shrinking, visualization, validation, and
natural-language authoring capabilities. The executable
[capability audit](docs/product-capability-audit.md) records implemented,
modeled, partial, and missing behavior. A backend milestone or static export
must not be described as the complete product.

## Run the native workbench

```bash
scripts/prepare-macos-engine.sh
open FIPSD.xcworkspace
```

Select the `FIPSD` scheme and run it. The workbench supports direct experiment
controls, local Claude/Codex campaign authoring, deterministic event playback,
forward-only natural-language changes to the currently rendered run,
saved-artifact event-aware replay with exact dense-window summaries,
pause/step/scrub/speed controls, node inspection,
scheduled node failures and recoveries, manual lower-root injection, partitions
and merges, editable link conditions, parent/ancestry swaps, cost-driven parent
oscillation with hysteresis and hold-down, authenticated Sybil waves,
direct-link topology capture/editing, weighted per-node
Wi‑Fi/BLE/Tor/Ethernet profiles, authored network zones with real shared-medium
queue contention, configurable CPU/memory/cache/table/queue
limits and deterministic slow-node heterogeneity, exact
control-message propagation, graph-native split-horizon Bloom waves, routed
synthetic payload projection including persistent segmented streams and
synchronized burst processes, explicit source/destination object transfers
with automatic MTU packetization and delivered-byte progress, graph-native
lookup/cache/session recovery, and
honest one-billion-node analytical cohort rendering and evidence export. Its
analysis inspector ranks recorded link bottlenecks,
reconciles causal work by stage, shows root-arrival amplification, delivery and
Bloom-FPR percentiles, congestion groups, causal flame graphs, and
heavy-link anomaly samples while keeping fidelity labels beside every result.
A side-by-side runner holds seed, topology,
traffic, and interventions constant while comparing protocol timer variants.
The workbench also launches bounded pairwise adversarial searches and
automatically shrinks the highest-amplification reproduction while preserving
90% of its score. For tiny networks, it can
[exhaust every supported authored action order](docs/tiny-state-exploration.md)
and retain independently replayable counterexamples. The
[Bloom](docs/graph-native-bloom.md),
[payload](docs/routed-payload-stream.md),
[explicit object transfers](docs/explicit-application-transfers.md),
[natural-language amendments](docs/natural-language-amendments.md),
[lookup/session recovery](docs/graph-native-recovery.md),
[multi-scale visualization](docs/multi-scale-visualization.md), and
[mixed-connectivity](docs/mixed-node-connectivity.md),
[shared-medium zones](docs/shared-medium-zones.md), and
[interactive interventions](docs/interactive-interventions.md),
[session rekey waves](docs/session-rekey-waves.md),
[lookup storms](docs/lookup-storms.md),
[parent instability](docs/parent-instability.md),
[authenticated Sybil admission](docs/authenticated-sybil-admission.md), and
[transport-class failover](docs/transport-class-failover.md) fidelity
boundaries are documented explicitly.

## Control the app from agents

The [local MCP control server](docs/mcp-control-server.md) lets Claude, Codex,
and other MCP clients launch and manage the same visible workbench. Agents can
author or amend experiments, inject exact events, change configuration, control
playback, inspect current state, wait for completion, analyze artifacts, and
explain the active causal state without UI automation.

The macOS app also embeds [Claude over ACP](docs/acp-agent-sidebar.md) behind
the left sidebar's **Experiment / Agent** switch. That session receives the
same MCP server directly, runs in bypass-permissions mode, and renders streamed
agent replies as Markdown.

## Run from the CLI

```bash
cargo run -p fips-cli --bin fips-wind-tunnel -- \
  stream examples/m1/root-ratchet-12.yaml --output runs/root-ratchet-12
```

The stream is JSON Lines using
[`event-stream-v1alpha1`](schemas/event-stream-v1alpha1.schema.json). Its event
records are identical to the ordered trace written into `artifact.json`.
The native renderer separately records its source-mapped projection as
[`render-frame-v1alpha1`](schemas/render-frame-v1alpha1.schema.json) JSON Lines.
Its canvas consumes only those frame primitives; an independent raw-artifact
oracle regression-checks every visualization mode while keeping synthetic
layout and compressed-event boundaries explicit.

Other campaign, scale, oracle, analysis, atlas, and release commands remain
available through `fips-wind-tunnel --help`. Their verified scope and fidelity
are documented under [`docs/`](docs/).

## Grounding

The simulation is pinned to FIPS commit
`80c956a6fdb85dde1450969a21891c1158e43267`. Executable codecs and checked-in
tests outrank prose. Every result declares fidelity and provenance; aggregate,
sampled, calibrated, and unsupported behavior stays labeled.

## Development gates

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
scripts/check-loc.sh
```

Hand-written source files have a 300-line soft limit and a 600-line hard limit.
See [`AGENTS.md`](AGENTS.md) for the complete engineering contracts.

## License

FIPS Wind Tunnel is available under the [MIT License](LICENSE).
