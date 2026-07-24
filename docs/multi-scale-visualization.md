# Multi-scale visualization

The native workbench exposes three representations of the same persisted event
stream:

- **Root adoption** renders individual nodes colored by their adopted root.
- **Connectivity** renders individual nodes by assigned endpoint media.
- **Cohorts** groups nodes by root identity, four-level depth band,
  and transport profile.

Runs with more than 500 represented nodes select cohort view when their initial
topology reaches playback. The 10K preset selects it before execution. Users
can still request either exact-node view explicitly; changing the view never
changes simulation state or fidelity.

Every retained root identity has a stable hashed world coordinate; population
rank never controls position. Circle area follows cohort population, opacity
follows active share, color identifies transport, and the label gives
represented node count. Concurrent frames with the same source cohort,
destination cohort, and plane are collapsed into one path whose width grows
logarithmically with frame count. Its progress is the deterministic mean over
every matching transmission rather than a dictionary-selected representative.
Control, Bloom, lookup, session, and payload planes retain distinct colors.

This is display aggregation, not cohort simulation. The run still declares
individual scale fidelity, every underlying event remains in the artifact, and
the timeline and causal inspector remain event-addressable. Sparse events inside
a wall-clock frame are presented individually; dense windows are exactly
summarized with their full ordered event list. Native renderer JSONL records the
cohort membership, aggregate progress, source mapping, and fidelity boundary.
The canvas consumes only that `RenderFrame`, and an independent raw-artifact
oracle compares cohort membership and aggregate flights across every scheduled
frame in both committed renderer-audit artifacts. Imported analytical cohort
marks are separately compared with their raw artifact JSON.
Analytical or cohort-engine results must continue to declare their own
non-individual scale fidelity rather than borrowing this view label.

The cohort canvas is not physical geometry and does not claim continuous
protocol motion. Percentile distributions, congestion groups, heavy-link
samples, and causal analysis live in the evidence inspector rather than being
implied by bubble position.
