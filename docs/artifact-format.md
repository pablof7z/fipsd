# Deterministic artifact and reproduction format v1alpha1

## Full run artifact

A run artifact contains:

1. a manifest with artifact/run IDs, plan digest, fidelity, and provenance;
2. the normalized campaign/plan input;
3. an event trace ordered by `(virtual_time_ns, ordinal, event_id)`;
4. metric series with explicit units and ordered samples;
5. causal ledger stages and evidence references;
6. assertion results;
7. optional sampled subgraphs and structured logs;
8. optional out-of-line blobs with role, relative path, byte length, encoding,
   and SHA-256.

## Deterministic encoding

- Canonical committed documents are UTF-8 JSON with LF newlines and one final
  newline.
- Object keys and map-backed collections are lexicographically ordered.
- Ordered semantic collections are arrays with the ordering rule stated by the
  schema; unordered sets are sorted before serialization.
- Integers are base-10 JSON integers. Protocol and virtual time use integer
  nanoseconds. Non-integral measurements use decimal strings with a declared
  unit/scale; NaN and infinities are forbidden.
- IDs and checksums are lowercase ASCII; SHA-256 is 64 lowercase hex digits.
- Compression applies only to out-of-line blobs. `identity`, `gzip`, and `zstd`
  are named explicitly; checksums cover stored bytes.
- Unknown fields are rejected within a schema major version. Readers may
  accept a newer minor version only after validating its declared compatible
  feature set; they never silently discard fields.

Manifest and event-order bytes depend only on normalized input, versioned
engine behavior, and seed. Wall-clock collection time is excluded from those
sections.

## Out-of-line evidence

Blob paths must be relative, normalized, remain below the artifact directory,
and match both declared byte length and SHA-256 before use. Large metric series,
pcaps, sampled subgraphs, and logs can therefore be streamed without loading
the full artifact into memory while remaining addressable from aggregates.

## Reproduction bundle

A reproduction bundle is deliberately smaller than a run artifact. It contains
the normalized plan or minimized campaign, seed, target engine/variant, required
fidelity/provenance subset, expected failing assertions, and only the external
blobs needed to replay. It does not contain unrelated metric history, rendered
views, or host-local paths.

The JSON Schemas under `schemas/` and golden fixtures under `fixtures/artifacts/`
are executable parts of this specification.
