# ADR 0003: Billion-node results use cohort/hybrid representation (C3)

- Status: accepted for P0
- Owner milestone: M4
- Decision key: C3

## Decision

A billion-node result means analytical cohorts with explicitly identified
sampled exact regions. It never means one billion individually allocated nodes
unless a later artifact proves that representation and its resource cost.

## Rationale

Population-scale structural questions do not require pretending that every
node has exact state. Explicit representation keeps memory and compute claims
honest while allowing anomalies to be instantiated in exact subgraphs.

## Consequences

- Every result displays its scale representation and approximation metadata.
- Cohort totals include uncertainty or error bounds.
- Sampled regions identify their selection method and relationship to cohorts.
- Any unlabeled billion-node claim is invalid.

## Reversal trigger

M4 owns reversal. Reconsider C3 if cross-engine calibration shows cohort/hybrid
results are not predictive for the flagship metrics, or measured individual
state becomes practical and materially more accurate at the target scale.

## Reversal evidence

A reversal requires matched seeds across individual, cohort, and hybrid
engines, published error distributions, and reproducible resource measurements.
