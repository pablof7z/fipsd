# Coordinate-cache expiry and lookup storms

`expire-coordinate-cache` and `simultaneous-lookups` are separate replayable
inputs. Scheduling them at the same virtual timestamp is deterministic: global
cache invalidation executes first, then the lookup wave schedules every probe
at that exact timestamp.

The expiry event records the exact number of removed entries. The wave takes
`parameters.count` stable endpoint pairs from the configured synthetic traffic
population, gives each probe a unique causal ID, and sends it through the normal
lookup, retry, session, payload, resource, link, MTU, loss, and queue machinery.
The original traffic plus probes may not exceed the individual engine's
100,000-flow bound.

Lookup request and response frame sizes are executable-codec-derived. Endpoint
selection, path routing, resource work, retries, and virtual-time ordering are
semantic-exact. Transport conditions remain configured inputs rather than
measurements of real Wi-Fi, Bluetooth, Tor, or Ethernet implementations.

The native workbench exposes one control that authors both inputs at the current
timeline cursor. Lookup frames animate through the graph, and the inspector
reports wave and cache-invalidation totals.
