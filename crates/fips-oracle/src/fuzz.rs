use crate::PINNED_FIPS_COMMIT;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FuzzOutcome {
    Pass,
    Crash,
    Hang,
    AllocationLimit,
    CodecDifferential,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FuzzArtifact {
    pub kind: String,
    pub backend: String,
    pub backend_version: String,
    pub protocol_version: String,
    pub codec_commit: String,
    pub corpus_sha256: String,
    pub coverage_edges: u64,
    pub outcome: FuzzOutcome,
    pub minimized_input_hex: Option<String>,
    pub standalone_replay: Option<Vec<String>>,
    pub routed_to_semantic_engine: bool,
}

pub fn adapt_fuzz_result(
    backend: &str,
    outcome: FuzzOutcome,
    input: &[u8],
    corpus_sha256: &str,
    coverage_edges: u64,
) -> FuzzArtifact {
    let failure = outcome != FuzzOutcome::Pass;
    FuzzArtifact {
        kind: "invalid-wire-fuzz-artifact/v1alpha1".to_owned(),
        backend: backend.to_owned(),
        backend_version: "pinned-v1".to_owned(),
        protocol_version: "fmp-80c956a".to_owned(),
        codec_commit: PINNED_FIPS_COMMIT.to_owned(),
        corpus_sha256: corpus_sha256.to_owned(),
        coverage_edges,
        outcome,
        minimized_input_hex: failure.then(|| hex::encode(input)),
        standalone_replay: failure
            .then(|| vec!["fips-codec-fuzz-replay".to_owned(), hex::encode(input)]),
        routed_to_semantic_engine: false,
    }
}
