//! Access to the generated, pinned production-codec conformance manifest.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

/// Pinned FIPS revision exercised by the generated harness.
pub const FIPS_COMMIT: &str = "80c956a6fdb85dde1450969a21891c1158e43267";
/// Checked-in output of the production codec harness.
pub const CODEC_MANIFEST_JSON: &str = include_str!("../../../fixtures/codecs/fips-80c956a.json");

/// One TreeAnnounce boundary fixture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeBoundary {
    /// Tree depth (root is zero).
    pub depth: u32,
    /// Encoded TreeAnnounce plaintext bytes, including message type.
    pub message_bytes: u64,
    /// Established FMP bytes including header, timestamp, and tag.
    pub framed_bytes: u64,
}

/// Generated codec conformance output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodecManifest {
    /// Full upstream revision.
    pub fips_commit: String,
    /// SHA-256 by authoritative source file.
    pub source_sha256: BTreeMap<String, String>,
    /// Fixed FMP established framing components.
    pub fmp_established_header_bytes: u64,
    /// Session-relative timestamp bytes prepended to a link message.
    pub fmp_timestamp_bytes: u64,
    /// AEAD tag bytes.
    pub aead_tag_bytes: u64,
    /// TreeAnnounce boundary fixtures.
    pub tree_announce: Vec<TreeBoundary>,
    /// Other production encoder sizes.
    pub encoded_sizes: BTreeMap<String, u64>,
    /// Maximum depth whose FMP encrypted-payload length fits in u16.
    pub maximum_safe_tree_depth: u32,
}

impl CodecManifest {
    /// Load and verify the embedded manifest pin.
    pub fn load() -> Result<Self, AdapterError> {
        let manifest: Self = serde_json::from_str(CODEC_MANIFEST_JSON)?;
        if manifest.fips_commit != FIPS_COMMIT {
            return Err(AdapterError::Commit {
                expected: FIPS_COMMIT.to_owned(),
                actual: manifest.fips_commit,
            });
        }
        Ok(manifest)
    }

    /// Return the executable fixture for a required depth.
    pub fn tree_boundary(&self, depth: u32) -> Result<&TreeBoundary, AdapterError> {
        self.tree_announce
            .iter()
            .find(|fixture| fixture.depth == depth)
            .ok_or(AdapterError::MissingDepth(depth))
    }
}

/// Adapter/manifest failure.
#[derive(Debug, Error)]
pub enum AdapterError {
    /// Manifest JSON is invalid.
    #[error("invalid codec manifest: {0}")]
    Json(#[from] serde_json::Error),
    /// Manifest pin drifted.
    #[error("codec manifest FIPS commit mismatch: expected {expected}, got {actual}")]
    Commit {
        /// Required commit.
        expected: String,
        /// Manifest commit.
        actual: String,
    },
    /// A required boundary fixture is absent.
    #[error("codec manifest lacks TreeAnnounce depth {0}")]
    MissingDepth(u32),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_tree_boundaries_are_production_generated() {
        let manifest = CodecManifest::load().unwrap();
        for depth in [0, 35, 64, 65, 2000, manifest.maximum_safe_tree_depth] {
            let fixture = manifest.tree_boundary(depth).unwrap();
            assert_eq!(fixture.message_bytes, 132 + 32 * u64::from(depth));
            assert_eq!(fixture.framed_bytes, 168 + 32 * u64::from(depth));
        }
    }

    #[test]
    fn filter_and_framing_overhead_reconcile() {
        let manifest = CodecManifest::load().unwrap();
        assert_eq!(manifest.encoded_sizes["filter_announce_message"], 1035);
        assert_eq!(manifest.encoded_sizes["filter_announce_fmp_frame"], 1071);
        assert_eq!(
            manifest.fmp_established_header_bytes
                + manifest.fmp_timestamp_bytes
                + manifest.aead_tag_bytes,
            36
        );
    }
}
