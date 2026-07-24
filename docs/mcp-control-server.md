# MCP app control server

FIPS Wind Tunnel exposes its visible state and controls through a local MCP
server. Each agent launches a small stdio MCP process. Those processes connect
to one authenticated loopback endpoint owned by the running macOS app, so
multiple agents can inspect and control the same experiment without automating
the UI.

## Install

Build the app, then install the MCP executable and record the app path:

```bash
scripts/install-mcp-server.sh \
  --app "/absolute/path/to/FIPSD.app"
```

The executable is installed as `~/.local/bin/fips-wind-tunnel-mcp`. The app
path is stored in `~/.config/fips-wind-tunnel/app-path`, allowing the
`wind_tunnel_launch` tool to open the app when necessary.

## Configure agents

Claude Code:

```bash
claude mcp add --scope user fips-wind-tunnel -- \
  ~/.local/bin/fips-wind-tunnel-mcp
```

Codex:

```bash
codex mcp add fips-wind-tunnel -- \
  ~/.local/bin/fips-wind-tunnel-mcp
```

Generic MCP clients can use:

```json
{
  "mcpServers": {
    "fips-wind-tunnel": {
      "command": "/Users/YOU/.local/bin/fips-wind-tunnel-mcp",
      "args": []
    }
  }
}
```

The server implements MCP revision `2025-06-18` over newline-delimited JSON-RPC
2.0 stdio or sessionless Streamable HTTP and negotiates the common
`2025-03-26` and `2024-11-05` revisions.

## Skill and knowledge

The MCP server provides a source-backed FIPS Wind Tunnel expert skill through
all three MCP knowledge surfaces:

- Resource `fips-wind-tunnel://skill` contains the complete `SKILL.md`.
- Prompt `fips_wind_tunnel_expert` applies the skill to a caller-supplied task.
- Tools `wind_tunnel_get_skill`, `wind_tunnel_list_knowledge`, and
  `wind_tunnel_read_knowledge` support clients that expose only tools.

The knowledge catalog includes checked-in FIPS protocol design and reference
documents, Wind Tunnel product documentation and ADRs, every JSON schema, and
the experiment examples. Resources are read from the repository path recorded
by the installer; arbitrary filesystem paths are never accepted.

## HTTPS transport

Run a bearer-protected local HTTP endpoint:

```bash
FIPS_WIND_TUNNEL_HTTP_TOKEN="a-random-secret-of-at-least-32-bytes" \
FIPS_WIND_TUNNEL_PAIRING_CODE="1234" \
  ~/.local/bin/fips-wind-tunnel-mcp --http 8765
```

Its MCP URL is `http://127.0.0.1:8765/mcp`; `GET /health` is an unauthenticated
health check. Put an HTTPS reverse tunnel in front of this loopback listener
and configure clients with:

```json
{
  "url": "https://YOUR-TUNNEL.example/mcp",
  "headers": {
    "Authorization": "Bearer YOUR_TOKEN"
  }
}
```

The HTTP server is sessionless. The canonical MCP path is `/mcp`; `/` is an
authenticated compatibility alias for ChatGPT custom apps registered with only
the public origin. Both paths return JSON responses to POST requests and `202`
for notifications. MCP GET returns `405` because the server does not provide a
server-initiated SSE stream.

### OAuth pairing

HTTP clients can discover and complete an OAuth 2.1 authorization-code flow
without receiving the administrative bearer token. The server provides:

- RFC 9728 protected-resource metadata and a matching `WWW-Authenticate`
  challenge.
- RFC 8414 authorization-server metadata.
- RFC 7591 dynamic client registration.
- Exact registered redirect matching and mandatory PKCE S256.
- A browser approval screen requiring the configured four-digit pairing code.
- Resource-bound opaque access tokens stored only as SHA-256 digests.

Authorization and token requests must carry the exact MCP `resource` URI.
Authorization codes are single-use and expire after five minutes. Issued access
tokens intentionally have no expiration and survive service restarts; the token
response therefore omits `expires_in`. Delete an access entry from
`~/.config/fips-wind-tunnel/oauth-state.json` to revoke it.

The pairing endpoint locks for one minute after five incorrect codes. Four
digits remain suitable only for attended pairing; keep the pairing code private.
The long administrative bearer token remains available for emergency access.

## Tools

- `wind_tunnel_launch` opens the configured app.
- `wind_tunnel_get_state` returns rendered time, topology, roots, traffic,
  transfers, configuration, run status, evidence location, and the last event.
- `wind_tunnel_start_experiment` authors and starts a natural-language
  experiment with Auto, Sonnet, Haiku, Opus, or Codex.
- `wind_tunnel_amend_experiment` sends the prompt plus exact rendered state to
  the chosen local model and branches forward from the current cursor.
- `wind_tunnel_playback` plays, pauses, stops, seeks, steps, or changes speed.
- `wind_tunnel_set_parameters` changes direct configuration and can start it.
- `wind_tunnel_run_campaign` runs a complete Campaign JSON document.
- `wind_tunnel_inject_event` schedules any supported amendment event after the
  cursor, replays immutable history, and continues the visible branch.
- `wind_tunnel_save_experiment` saves the exact active Campaign with checksum,
  source-run, and fidelity provenance.
- `wind_tunnel_list_experiments` lists the durable local experiment library.
- `wind_tunnel_rerun_experiment` reruns the exact saved Campaign selected by ID.
- `wind_tunnel_get_analysis` returns fidelity, causal stages, heavy links,
  root-arrival impacts, diagnostics, and evidence paths.
- `wind_tunnel_explain` returns an evidence-grounded description together with
  the state and analysis used to construct it.
- `wind_tunnel_wait_until_idle` waits for authoring and engine execution.

Use natural-language amendment for semantic changes such as “make the old
nodes disappear until four remain.” Use event injection when the caller knows
the exact Campaign action, target, timing, and parameters. Use a full Campaign
when the caller needs complete declarative control.

Saved experiments live under
`~/Library/Application Support/FIPSD/Experiments/<id>/`. Each immutable entry
contains the exact `campaign.json` bytes and a versioned `manifest.json`.
Rerunning verifies the Campaign SHA-256 before passing it back through the normal
engine validation and execution path; rendered state and model prose are not
used as replay inputs.

## Local security and concurrency

The app binds only to `127.0.0.1` on an ephemeral port. Every app launch creates
a random token and publishes its port, PID, and token in
`~/Library/Application Support/FIPSD/control-endpoint.json` with mode `0600`.
The token never travels over MCP stdio or appears in tool results.

Every MCP process re-reads the endpoint before each call. Several agents may
therefore share one app. Commands are serialized by the app's main actor, and
the resulting Campaign and evidence preserve which changes came through MCP.

MCP access is powerful: configured agents can start or stop experiments and
alter their future. The MCP host remains responsible for presenting tool
approval according to the user's policy.
