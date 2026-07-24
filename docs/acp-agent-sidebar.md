# Embedded Claude agent

The macOS workbench embeds Claude in the left sidebar. Use the
**Experiment / Agent** segmented control to switch between deterministic
campaign controls and the conversation. The earlier local-model prompt
authoring form is intentionally absent; natural-language experiment work now
goes through the agent and the app's MCP boundary.

## Runtime boundary

The app launches the pinned official Claude ACP adapter:

```text
npx --yes @agentclientprotocol/claude-agent-acp@0.61.0
```

It negotiates ACP v1 over newline-delimited JSON-RPC stdio, opens one session,
and passes `fips-wind-tunnel` as a session-scoped stdio MCP server. The MCP
process still connects to the authenticated loopback endpoint owned by the
visible app. ACP is a product integration boundary; it does not define engine
time, ordering, graph representation, fidelity, or provenance.

Every new session selects ACP mode `bypassPermissions`, which is the adapter's
equivalent of Claude Code's `--dangerously-skip-permissions`. Exceptional
permission callbacks are resolved automatically and never interrupt the
sidebar conversation.

Claude is the only reasoning layer in the embedded path. The ACP session
disallows `wind_tunnel_start_experiment` and `wind_tunnel_amend_experiment`
because those tools ask a second local model to interpret natural language.
Before `session/new`, the app loads the complete checked-in
`skills/fips-wind-tunnel/SKILL.md` and appends it to the Claude Code system
prompt. The embedded session also disallows `wind_tunnel_get_skill`, so Claude
cannot waste a tool round trip reloading guidance already in context.
Instead, Claude calls structured tools directly:

- `wind_tunnel_set_parameters` for direct-control changes;
- `wind_tunnel_run_campaign` for a complete declarative campaign;
- `wind_tunnel_inject_event` for a forward-only change at the cursor;
- save, list, and rerun tools for durable exact-Campaign experiment reuse;
- playback, state, wait, analysis, and explain tools for observation.

The two natural-language authoring tools remain available to other MCP clients;
they are excluded only from this embedded ACP session.

Routine direct controls, playback, and saved-experiment operations must not
search the knowledge catalog. Targeted knowledge reads remain available for
protocol facts, raw schema authoring, unfamiliar evidence, and source-backed
architecture or fidelity claims.

Responses stream into the transcript. Agent messages use a full Markdown
renderer, including headings, lists, links, emphasis, tables, and fenced code
blocks. Tool calls remain compact status rows rather than being folded into
the model's prose.

## Requirements

1. Claude Code must already be authenticated on the Mac.
2. Node.js and `npx` must be installed.
3. Install the Wind Tunnel MCP helper:

   ```bash
   scripts/install-mcp-server.sh --app "/absolute/path/to/FIPSD.app"
   ```

The app searches the active `PATH`, Homebrew locations, and installed NVM node
versions for `npx`. These environment overrides are available for development:

- `FIPS_WIND_TUNNEL_NPX`
- `FIPS_WIND_TUNNEL_MCP`
- `FIPS_WIND_TUNNEL_WORKSPACE`

The sidebar reports an actionable error and offers Retry when either executable
is unavailable or ACP negotiation fails. Starting a new conversation terminates
the current adapter, creates a fresh ACP session, reattaches the MCP server, and
reapplies bypass mode.
