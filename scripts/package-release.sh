#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repository_root"

if (( $# != 1 )); then
  echo "usage: scripts/package-release.sh OUTPUT_DIRECTORY" >&2
  exit 2
fi

output="$1"
if [[ -z "$output" || "$output" == "/" || "$output" == "." ]]; then
  echo "refusing unsafe output directory: $output" >&2
  exit 2
fi
if [[ -e "$output" ]]; then
  echo "output already exists: $output" >&2
  exit 2
fi
if ! command -v jq >/dev/null; then
  echo "jq is required to generate the SPDX SBOM" >&2
  exit 2
fi

target_directory="$(
  cargo metadata --format-version 1 --locked --no-deps |
    jq -r '.target_directory'
)"
binary="$target_directory/release/fips-wind-tunnel"
cargo build --locked --release -p fips-cli --bin fips-wind-tunnel
mkdir -p "$output/bin" "$output/web" "$output/schemas" "$output/docs" "$output/evidence"
cp "$binary" "$output/bin/"
cp web/index.html web/app.js web/worker.js web/styles.css web/data.js "$output/web/"
cp schemas/*.json "$output/schemas/"
cp README.md SECURITY.md LICENSE "$output/"
cp docs/quickstart.md docs/artifact-format.md docs/fidelity-and-provenance.md \
  docs/renderer-evidence.md docs/threat-model.md docs/support-matrix.md \
  docs/performance.md "$output/docs/"
cp fixtures/m7/qualification-atlas.json fixtures/m8/release-audit.json "$output/evidence/"

cargo metadata --format-version 1 --locked | jq '{
  spdxVersion: "SPDX-2.3",
  dataLicense: "CC0-1.0",
  SPDXID: "SPDXRef-DOCUMENT",
  name: "fips-wind-tunnel-0.1.0",
  documentNamespace: "https://github.com/pablof7z/fipsd/releases/tag/v0.1.0",
  creationInfo: {creators: ["Tool: cargo-metadata+jq"], created: "1970-01-01T00:00:00Z"},
  packages: [.packages[] | {
    name, SPDXID: ("SPDXRef-Package-" + (.id | gsub("[^A-Za-z0-9.-]"; "-"))),
    versionInfo: .version, downloadLocation: (.source // "NOASSERTION"),
    licenseConcluded: (.license // "NOASSERTION"), filesAnalyzed: false
  }]
}' > "$output/sbom.spdx.json"

"$binary" release manifest "$output" \
  --output "$output/release-manifest.json"
"$binary" release verify-package "$output" \
  "$output/release-manifest.json"

if [[ -n "${FIPSD_COSIGN_KEY:-}" ]]; then
  if ! command -v cosign >/dev/null; then
    echo "FIPSD_COSIGN_KEY is set but cosign is unavailable" >&2
    exit 2
  fi
  cosign sign-blob --yes --key "$FIPSD_COSIGN_KEY" \
    --output-signature "$output/checksums.sha256.sig" "$output/checksums.sha256"
fi

echo "release package: $output"
