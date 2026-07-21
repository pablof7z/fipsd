# Fidelity and provenance contract v1alpha1

Every run artifact contains one machine-readable `fidelity` object and one
`provenance` object. Missing fields are never interpreted as exactness.

## Fidelity dimensions

| Dimension | Values | Meaning |
| --- | --- | --- |
| Wire | `executable-codec`, `captured-wire`, `modeled`, `none` | Origin of serialized byte claims |
| Protocol | `semantic-exact`, `operation-counted`, `statistical`, `cohort` | State-transition representation |
| Compute | `executed`, `operation-counted`, `calibrated`, `none` | How compute cost was obtained |
| Scale | `individual`, `cohort`, `hybrid` | Node/edge representation |
| Bloom | `exact-bits`, `sparse-bits`, `occupancy`, `cohort-fpr`, `sampled-exact` | Bloom representation |

`hybrid` is represented by `scale: hybrid` plus non-empty sampled-region and
approximation metadata. It is not a synonym for semantic exactness.

## Required provenance

- engine name, version, and source revision;
- Campaign, normalized-plan, artifact, and fidelity schema versions;
- seed and normalized-plan SHA-256;
- exact FIPS commit for production-derived semantics or bytes;
- image digest for container/daemon evidence when applicable;
- hardware-profile identifier and calibration version for calibrated compute;
- approximation method, parameters, validation range, and uncertainty for
  statistical/cohort/hybrid results.

## Invalid combinations

Validation rejects, rather than downgrades:

- `wire: executable-codec` without a full FIPS commit;
- `compute: calibrated` without a hardware profile;
- statistical/cohort protocol or cohort/hybrid scale without approximation
  metadata;
- hybrid scale without at least one sampled exact region;
- individual billion-node representation;
- `bloom: sampled-exact` outside hybrid scale;
- exact wire totals whose ledger does not cite serialized-frame evidence.

## Rendering rule

Renderers must generate the leading fidelity statement from the object alone.
They may add detail but cannot replace or soften it. Approximate values carry
their method and uncertainty in text, tables, exports, and accessible labels.
No color, tooltip, or UI-only state may be the sole fidelity disclosure.

The Rust `FidelityContract::plain_language_statement` implementation is the
canonical baseline renderer and is covered by tests.

