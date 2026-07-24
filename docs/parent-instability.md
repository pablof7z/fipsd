# Parent and ancestry instability

The individual-node engine applies the pinned FIPS parent ranking:

`effective depth = peer tree depth + MMP link cost`

Costs are deterministic integer millionths, avoiding worker- or platform-order
differences. Root correction and a missing parent remain mandatory. Same-root
improvements are discretionary and pass through the configured
`parent_hysteresis_ppm` and `parent_hold_down` gates.

`swap-parent-ancestry` performs one re-evaluation. `alternate-parent-quality`
performs a bounded series of re-evaluations. Each pulse makes one eligible
alternate peer preferred and the current parent degraded. The target must have
two converged, loop-free peer views of the same root. Without a target, the
first eligible node in stable ID order is used.

An accepted change goes through the normal tree transition path. It increments
the declaration sequence, invalidates coordinate paths containing the changed
node, requests TreeAnnounce and Bloom updates, traverses configured queues and
links, and causes descendants to learn the new ancestry from delivered
announcements. Suppressed pulses remain visible as `suppressed` causal-ledger
entries.

The authored cost is a modeled MMP snapshot, not an executed MMP measurement.
Artifacts declare `modeled-mmp-link-cost-snapshot-v1`; SRTT and ETX estimation
remain outside this slice. Parent choice and all downstream event ordering are
still deterministic for the supplied snapshot.
