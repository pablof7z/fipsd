---
name: fips-wind-tunnel
description: Operate and explain the FIPS protocol and FIPS Wind Tunnel through MCP. Use for FIPS architecture, spanning-tree roots and parents, coordinates, Bloom filters, discovery, routing, sessions, transports, MTU and security; for Campaign v1alpha1 schema authoring; and for starting, changing, replaying, analyzing, or explaining deterministic Wind Tunnel experiments.
---

# FIPS Wind Tunnel

Treat the MCP server as both the control plane for the visible application and
the source-backed knowledge interface for FIPS. Never infer current run state
from an earlier tool response.

## Choose the shortest correct workflow

The embedded Wind Tunnel agent receives this entire skill in its initial system
prompt. Never call `wind_tunnel_get_skill` from that agent.

For routine experiment control, playback, saving, listing, and rerunning, use
the structured tools directly. Do not call `wind_tunnel_list_knowledge` or
`wind_tunnel_read_knowledge` merely to rediscover tool usage. Tool descriptions
and this skill are sufficient for those operations.

Call `wind_tunnel_get_state` before discussing a current run or before a
mutation whose relationship to the current run matters. A direct request to
create or rerun an experiment already authorizes replacing the visible run.

Use targeted knowledge resources only when the task requires:

- protocol or daemon facts;
- exact raw Campaign or event schema details;
- source-backed fidelity or architecture claims;
- interpretation of unfamiliar evidence or a failed direct tool call.

Preserve returned fidelity, approximation, provenance, and evidence labels in
every conclusion.

Use these maps when choosing sources:

- Read `fips-wind-tunnel://skill/reference/fips-protocol-map` for protocol
  concepts and canonical FIPS source routing.
- Read `fips-wind-tunnel://skill/reference/wind-tunnel-model` for simulator
  architecture, fidelity, causal accounting, and visualization.
- Read `fips-wind-tunnel://skill/reference/campaigns-and-events` only when
  authoring raw Campaign JSON or injecting an event whose exact contract is not
  already known.
- Read `fips-wind-tunnel://product/schemas/campaign-v1alpha1.schema.json`
  whenever exact schema validity matters.

## Answer FIPS questions

Search the `protocol` knowledge collection, read the relevant checked-in FIPS
design or reference documents, and distinguish:

- protocol semantics from a particular daemon implementation;
- design prose from executable codecs and tests;
- control-plane behavior from application payload behavior;
- direct links from shared media, transport profiles, and network zones.

When sources disagree, prefer executable behavior and wire codecs, then current
design/reference documentation. State drift explicitly.

## Design and run experiments

Agents with their own reasoning layer must use structured tools directly:

- `wind_tunnel_set_parameters` for direct-control scenarios;
- `wind_tunnel_run_campaign` for a complete schema-valid Campaign;
- `wind_tunnel_inject_event` for an exact forward-only change.

`wind_tunnel_start_experiment` and `wind_tunnel_amend_experiment` are convenience
tools for MCP clients without their own reasoning layer. The embedded agent must
not call them because they invoke another model.

For direct controls, `nodes` is the final total population and `arrivals` is the
number entering after virtual time zero. `interval_seconds` is their cadence.
Arrivals receive successively lower addresses and therefore become the new root.
For example, “five nodes total, with a new root every five seconds” maps to:

```json
{
  "parameters": {
    "nodes": 5,
    "arrivals": 4,
    "interval_seconds": 5
  },
  "run": true
}
```

After starting a run, use `wind_tunnel_wait_until_idle`, then inspect state or
analysis as needed.

Use `wind_tunnel_save_experiment` to preserve the exact active Campaign. Use
`wind_tunnel_list_experiments` to resolve saved IDs and
`wind_tunnel_rerun_experiment` to execute the checksum-verified saved Campaign.

## Inspect and explain

Use playback controls to pause or seek to the causal boundary of interest.
Then call `wind_tunnel_get_state`, `wind_tunnel_get_analysis`, and
`wind_tunnel_explain`.

Explain the chain from initiating event through semantic state changes,
constructed/coalesced/queued/transmitted/delivered work, resource pressure, and
application impact. Name heavy nodes or links and quantify payload versus wire
cost when evidence provides it.

Do not call a run converged merely because engine execution ended. Check
quiescence, root agreement, queues, pending deliveries, and invariant results.

## Safety and fidelity

- Ask before stopping or replacing a run unless the user directly requested a
  new or rerun experiment.
- Do not rewrite rendered history; branch forward from the cursor.
- Do not describe semantic models as wire-exact.
- Do not describe cohorts or analytical projections as individually executed
  nodes.
- Do not claim real-daemon agreement without oracle evidence and pinned
  provenance.
- Preserve reproducible artifacts, seeds, normalized cases, and evidence paths.
