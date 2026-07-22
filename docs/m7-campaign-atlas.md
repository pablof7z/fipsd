# M7 ten-family qualification atlas

`fips-atlas` compiles the ten normative Campaign v1alpha1 sources into one
byte-stable qualification document. Each family contains dimensions, a boundary
matrix, assertions, report recipe, resource budget, supported and unsupported
fidelities, a baseline, a discovered boundary, a protocol-variant comparison,
and a minimized reproduction descriptor.

Evidence is labeled `deterministic-model` unless it comes from a stronger seam.
The deep-tree 1,288-byte boundary is labeled `executable-codec` and checked
against the pinned FIPS codec manifest. Reproductions are separately marked
daemon `eligible` or `unsupported`; a missing daemon observation is not a model
pass.

```bash
fips-wind-tunnel atlas build --output atlas.json
fips-wind-tunnel atlas verify fixtures/m7/qualification-atlas.json
```

The checked-in atlas ID is
`9b8ab65ea994b4d52feda1b67161a64694d1504ac3edbe3d6ed96a8492882121`.
Acceptance evidence is in `crates/fips-atlas/tests/m7_acceptance.rs`.
