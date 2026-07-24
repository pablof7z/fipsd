# Multi-scale visualization

The native workbench exposes three representations of the same persisted event
stream:

- **Root adoption** renders individual nodes colored by their adopted root.
- **Connectivity** renders individual nodes by assigned endpoint media.
- **Cohorts** groups nodes by dominant root generation, four-level depth band,
  and transport profile.

Runs with more than 500 represented nodes select cohort view when their initial
topology reaches playback. The 10K preset selects it before execution. Users
can still request either exact-node view explicitly; changing the view never
changes simulation state or fidelity.

The cohort view retains the seven largest root populations and folds smaller
root populations into an explicit remainder column. Circle area follows cohort
population, opacity follows active share, color identifies transport, and the
label gives represented node count. Concurrent frames with the same source
cohort, destination cohort, and plane are collapsed into one path whose width
grows logarithmically with frame count. Control, Bloom, lookup, session, and
payload planes retain distinct colors.

This is display aggregation, not cohort simulation. The run still declares
individual scale fidelity, every underlying event remains in the artifact, and
the timeline and causal inspector remain event-addressable. Analytical or
cohort-engine results must continue to declare their own non-individual scale
fidelity rather than borrowing this view label.

The current grouping is the first useful large-run representation. Remaining
product-spec work includes heatmaps, percentile distributions, congestion
matrices, heavy-hitter rankings, sampled anomaly subgraphs, and causal flame
graphs.
