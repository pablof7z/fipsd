# Mixed per-node connectivity

Campaigns can assign different abstract connectivity profiles to individual
nodes. This supports experiments where some nodes use Wi‑Fi, BLE, Tor,
Ethernet, UDP, TCP, or Nym-like media with different bandwidth, latency, loss,
MTU, and queue limits.

```yaml
transports:
  assignment: random-mixed
  profiles:
    - name: wifi
      type: wifi
      mtu_bytes: 1500
      latency: 8ms
      jitter: 2ms
      bandwidth_bps: 100000000
      loss_ppm: 1000
      queue_bytes: 1048576
      weight: 50
    - name: bluetooth
      type: ble
      mtu_bytes: 244
      latency: 20ms
      jitter: 5ms
      bandwidth_bps: 1000000
      loss_ppm: 5000
      queue_bytes: 262144
      weight: 15
```

## Deterministic assignment

`random-mixed` is a weighted, seed-stable assignment. A node's profile is a
pure function of campaign seed, stable node ID, and the ordered profile
weights. A zero-weight profile is never selected. Replaying the normalized
campaign assigns the same profile to every node independent of worker count.

The initial topology event records each node's assigned profile, media family,
bandwidth, latency, jitter, and MTU. These fields are therefore present in both live
playback and the persisted event trace.

## Effective edge conditions

An edge between differently connected nodes uses an explicit conservative
model:

- bandwidth is the minimum of the base link and both endpoint profiles;
- MTU and queue capacity are the corresponding minima;
- one-way latency is base-link latency plus both endpoint access latencies;
- datagram jitter is the authored base jitter plus both endpoint jitter bounds;
- independent endpoint and base loss probabilities are composed;
- ordering is stream if either endpoint profile requires stream ordering;
- lower-layer overhead is the larger endpoint overhead.

Every topology snapshot includes the effective bandwidth, latency, jitter, MTU,
and both endpoint profile names. The UI can therefore explain why a particular
frame is delayed or rejected without reverse-engineering configuration.

## Fidelity boundary

Built-in media values are versioned abstract profiles, not measurements of a
specific Wi‑Fi network, BLE radio, Tor circuit, or host. Campaign-provided
values remain semantically modeled unless a later calibrated profile supplies
measurement provenance. FMP serialization still comes from the executable
codec; effective access-media overhead and performance are modeled.
