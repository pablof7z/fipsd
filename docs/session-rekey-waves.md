# Synchronized session rekey waves

`synchronized-session-rekey` is a replayable individual-engine intervention.
At its virtual timestamp, the engine snapshots every live session in stable
source/destination order. Each entry consumes two hash work units on the source
node and schedules a causally linked completion at the resource-ready time.

The input event records requested, scheduled, and rejected work plus the first
and last possible completion timestamps. A completion records its path and
whether the original session still exists. If topology or lifecycle disruption
removes that session before completion, the cryptographic work remains
`performed` and the result is also charged as `superseded`.

This behavior is operation-counted. It does not execute session keys, epochs,
wire frames, replay windows, or cipher state, and the event data therefore says
`operation-counted-no-wire-frame`. Replays preserve the exact request snapshot,
resource schedule, causal parentage, and retained/superseded outcome.

The graph-native runtime currently requires:

- `transports.assignment: random-mixed` with explicit profiles;
- non-idle traffic capable of establishing sessions; and
- `lookup` in `instrumentation.quiescence_markers`.

The native workbench exposes the same intervention at the current timeline
cursor. Completion briefly pulses the source node and the inspector reports the
total completed work, including work whose session was later superseded.
