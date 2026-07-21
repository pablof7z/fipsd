//! Versioned, deterministic run and reproduction artifacts.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path};
use thiserror::Error;

/// Full run-artifact schema version.
pub const RUN_ARTIFACT_VERSION: &str = "experiments.fips.network/run-artifact/v1alpha1";
/// Compact reproduction-bundle schema version.
pub const REPRODUCTION_BUNDLE_VERSION: &str =
    "experiments.fips.network/reproduction-bundle/v1alpha1";
/// Required pinned FIPS revision for the M0 executable-codec manifest.
pub const M0_FIPS_COMMIT: &str = "80c956a6fdb85dde1450969a21891c1158e43267";

/// Wire evidence origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WireFidelity {
    /// Production codec executed at the recorded revision.
    ExecutableCodec,
    /// Captured bytes from a real implementation.
    CapturedWire,
    /// Independently modeled encoding.
    Modeled,
    /// No wire claim.
    None,
}

/// Protocol-state representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProtocolFidelity {
    /// Individual semantic state and order.
    SemanticExact,
    /// Operations counted without full execution.
    OperationCounted,
    /// Seeded statistical approximation.
    Statistical,
    /// Population/cohort state.
    Cohort,
}

/// Compute-cost representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComputeFidelity {
    /// Work executed directly.
    Executed,
    /// Operations counted only.
    OperationCounted,
    /// Counts converted through a hardware profile.
    Calibrated,
    /// No compute claim.
    None,
}

/// Scale representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScaleFidelity {
    /// Every represented node is individual.
    Individual,
    /// Populations replace individual nodes.
    Cohort,
    /// Cohorts contain sampled exact regions.
    Hybrid,
}

/// Bloom representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BloomFidelity {
    /// Exact bit vectors.
    ExactBits,
    /// Sparse exact bit indices.
    SparseBits,
    /// Seeded occupancy approximation.
    Occupancy,
    /// Cohort false-positive-rate model.
    CohortFpr,
    /// Exact sampled regions inside a hybrid run.
    SampledExact,
}

/// Approximation method and its declared validity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Approximation {
    /// Versioned method identifier.
    pub method: String,
    /// Machine-readable parameters.
    pub parameters: BTreeMap<String, String>,
    /// Human-readable calibrated/validated range.
    pub validated_range: String,
    /// Human-readable uncertainty statement.
    pub uncertainty: String,
}

/// Exact region embedded in a hybrid/cohort result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SampledRegion {
    /// Stable region identifier.
    pub id: String,
    /// Selection method.
    pub selection: String,
    /// Exact node count.
    pub node_count: u64,
}

/// Self-contained fidelity contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FidelityContract {
    /// Wire evidence origin.
    pub wire: WireFidelity,
    /// Protocol representation.
    pub protocol: ProtocolFidelity,
    /// Compute representation.
    pub compute: ComputeFidelity,
    /// Scale representation.
    pub scale: ScaleFidelity,
    /// Bloom representation.
    pub bloom: BloomFidelity,
    /// Represented node count.
    pub represented_nodes: u64,
    /// Approximation metadata.
    #[serde(default)]
    pub approximations: Vec<Approximation>,
    /// Sampled exact regions.
    #[serde(default)]
    pub sampled_regions: Vec<SampledRegion>,
}

impl FidelityContract {
    /// Reject unsupported combinations rather than silently degrading them.
    pub fn validate(&self, provenance: &ProvenanceEnvelope) -> Result<(), ArtifactError> {
        if self.wire == WireFidelity::ExecutableCodec
            && provenance
                .fips_commit
                .as_deref()
                .is_none_or(|commit| commit.len() != 40)
        {
            return Err(ArtifactError::InvalidFidelity(
                "executable-codec wire fidelity requires a full FIPS commit".to_owned(),
            ));
        }
        if self.compute == ComputeFidelity::Calibrated && provenance.hardware_profile.is_none() {
            return Err(ArtifactError::InvalidFidelity(
                "calibrated compute fidelity requires a hardware profile".to_owned(),
            ));
        }
        let approximate_protocol = matches!(
            self.protocol,
            ProtocolFidelity::Statistical | ProtocolFidelity::Cohort
        );
        let approximate_scale = matches!(self.scale, ScaleFidelity::Cohort | ScaleFidelity::Hybrid);
        if (approximate_protocol || approximate_scale) && self.approximations.is_empty() {
            return Err(ArtifactError::InvalidFidelity(
                "statistical/cohort/hybrid fidelity requires approximation metadata".to_owned(),
            ));
        }
        if self.scale == ScaleFidelity::Hybrid && self.sampled_regions.is_empty() {
            return Err(ArtifactError::InvalidFidelity(
                "hybrid fidelity requires at least one sampled exact region".to_owned(),
            ));
        }
        if self.scale == ScaleFidelity::Individual && self.represented_nodes >= 1_000_000_000 {
            return Err(ArtifactError::InvalidFidelity(
                "a billion-node result cannot claim individual representation without a later measured contract".to_owned(),
            ));
        }
        if self.bloom == BloomFidelity::SampledExact && self.scale != ScaleFidelity::Hybrid {
            return Err(ArtifactError::InvalidFidelity(
                "sampled-exact Bloom fidelity requires hybrid scale".to_owned(),
            ));
        }
        Ok(())
    }

    /// Baseline plain-language statement renderable without UI state.
    pub fn plain_language_statement(&self) -> String {
        let qualifier = if self.approximations.is_empty() {
            "No approximation metadata applies.".to_owned()
        } else {
            let methods = self
                .approximations
                .iter()
                .map(|item| item.method.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!("Approximation methods: {methods}.")
        };
        let wire = match self.wire {
            WireFidelity::ExecutableCodec => "executable production codec",
            WireFidelity::CapturedWire => "captured implementation bytes",
            WireFidelity::Modeled => "modeled serialization",
            WireFidelity::None => "no wire-byte claim",
        };
        let protocol = match self.protocol {
            ProtocolFidelity::SemanticExact => "individually modeled semantic state",
            ProtocolFidelity::OperationCounted => "operation-counted protocol behavior",
            ProtocolFidelity::Statistical => "statistical protocol approximation",
            ProtocolFidelity::Cohort => "cohort protocol representation",
        };
        let compute = match self.compute {
            ComputeFidelity::Executed => "executed compute",
            ComputeFidelity::OperationCounted => "operation-counted compute",
            ComputeFidelity::Calibrated => "hardware-calibrated compute",
            ComputeFidelity::None => "no compute-cost claim",
        };
        let scale = match self.scale {
            ScaleFidelity::Individual => "individual nodes",
            ScaleFidelity::Cohort => "analytical cohorts",
            ScaleFidelity::Hybrid => "cohorts with sampled exact regions",
        };
        let bloom = match self.bloom {
            BloomFidelity::ExactBits => "exact Bloom bits",
            BloomFidelity::SparseBits => "sparse exact Bloom bits",
            BloomFidelity::Occupancy => "seeded Bloom occupancy",
            BloomFidelity::CohortFpr => "cohort Bloom false-positive rate",
            BloomFidelity::SampledExact => "exact Bloom bits in sampled regions",
        };
        format!(
            "Wire evidence uses {wire}; protocol fidelity is {protocol}; compute fidelity is {compute}; scale uses {scale} across {} represented nodes; Bloom fidelity uses {bloom}. {qualifier}",
            self.represented_nodes
        )
    }
}

/// Versioned hardware calibration identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwareProfile {
    /// Profile identifier.
    pub id: String,
    /// Calibration-data version.
    pub calibration_version: String,
}

/// Provenance required to interpret a result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceEnvelope {
    /// Engine implementation name.
    pub engine_name: String,
    /// Engine semantic version.
    pub engine_version: String,
    /// Engine source revision.
    pub engine_source_revision: String,
    /// Schema versions by contract name.
    pub schema_versions: BTreeMap<String, String>,
    /// Seed.
    pub seed: u64,
    /// SHA-256 of normalized plan bytes.
    pub normalized_plan_sha256: String,
    /// Pinned FIPS revision when applicable.
    pub fips_commit: Option<String>,
    /// Container image digest when applicable.
    pub image_digest: Option<String>,
    /// Hardware profile for calibrated results.
    pub hardware_profile: Option<HardwareProfile>,
}

/// Deterministic artifact manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunManifest {
    /// Artifact schema version.
    pub api_version: String,
    /// Stable artifact identifier.
    pub artifact_id: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Fidelity contract.
    pub fidelity: FidelityContract,
    /// Provenance envelope.
    pub provenance: ProvenanceEnvelope,
}

/// Totally ordered event record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventRecord {
    /// Stable event ID.
    pub event_id: String,
    /// Virtual time in nanoseconds.
    pub virtual_time_ns: u64,
    /// Tie-break ordinal at the same time.
    pub ordinal: u64,
    /// Event kind.
    pub kind: String,
    /// Stable parent causal ID.
    pub causal_parent: Option<String>,
    /// Event data.
    pub data: Value,
}

/// One metric point with explicit string encoding for non-integral values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetricPoint {
    /// Virtual time in nanoseconds.
    pub virtual_time_ns: u64,
    /// Base-10 value string.
    pub value: String,
}

/// Ordered metric series.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetricSeries {
    /// Stable metric name.
    pub name: String,
    /// Unit identifier.
    pub unit: String,
    /// Ordered points.
    pub points: Vec<MetricPoint>,
}

/// A causal accounting stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerEntry {
    /// Stable causal ID.
    pub causal_id: String,
    /// Stable parent causal ID when this cost was caused by another action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub causal_parent: Option<String>,
    /// Requested/performed/constructed/etc.
    pub stage: String,
    /// Count at this stage.
    pub count: u64,
    /// Serialized-frame or other evidence IDs.
    pub evidence: Vec<String>,
}

/// Assertion outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssertionResult {
    /// Stable assertion ID.
    pub id: String,
    /// Pass/fail/unknown.
    pub outcome: String,
    /// Diagnostic.
    pub message: String,
}

/// Reference to stored bytes outside the manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalBlob {
    /// Semantic role.
    pub role: String,
    /// Relative normalized path.
    pub path: String,
    /// Stored-byte SHA-256.
    pub sha256: String,
    /// Stored-byte length.
    pub size_bytes: u64,
    /// `identity`, `gzip`, or `zstd`.
    pub encoding: String,
}

impl ExternalBlob {
    /// Verify path safety, byte count, and checksum.
    pub fn verify(&self, base: &Path) -> Result<(), ArtifactError> {
        let relative = Path::new(&self.path);
        if relative.is_absolute()
            || relative.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return Err(ArtifactError::UnsafeBlobPath(self.path.clone()));
        }
        let bytes = fs::read(base.join(relative)).map_err(|source| ArtifactError::BlobRead {
            path: self.path.clone(),
            source,
        })?;
        if bytes.len() as u64 != self.size_bytes {
            return Err(ArtifactError::BlobSize {
                path: self.path.clone(),
                expected: self.size_bytes,
                actual: bytes.len() as u64,
            });
        }
        let actual = hex_lower(&Sha256::digest(&bytes));
        if actual != self.sha256 {
            return Err(ArtifactError::BlobChecksum {
                path: self.path.clone(),
                expected: self.sha256.clone(),
                actual,
            });
        }
        Ok(())
    }
}

/// Full immutable run artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunArtifact {
    /// Deterministic manifest.
    pub manifest: RunManifest,
    /// Normalized campaign/plan.
    pub normalized_plan: Value,
    /// Ordered trace.
    pub event_trace: Vec<EventRecord>,
    /// Metric series.
    pub metric_series: Vec<MetricSeries>,
    /// Causal ledger.
    pub causal_ledger: Vec<LedgerEntry>,
    /// Assertion results.
    pub assertion_results: Vec<AssertionResult>,
    /// Sampled subgraphs or other inline samples.
    pub samples: Vec<Value>,
    /// Structured logs.
    pub logs: Vec<Value>,
    /// Out-of-line data.
    pub external_blobs: Vec<ExternalBlob>,
}

impl RunArtifact {
    /// Validate fidelity and deterministic event ordering.
    pub fn validate(&self) -> Result<(), ArtifactError> {
        self.manifest.fidelity.validate(&self.manifest.provenance)?;
        if self.manifest.api_version != RUN_ARTIFACT_VERSION {
            return Err(ArtifactError::Version(self.manifest.api_version.clone()));
        }
        if !self
            .event_trace
            .windows(2)
            .all(|pair| event_key(&pair[0]) <= event_key(&pair[1]))
        {
            return Err(ArtifactError::EventOrder);
        }
        Ok(())
    }

    /// Canonical pretty JSON with a final LF.
    pub fn to_canonical_json(&self) -> Result<Vec<u8>, ArtifactError> {
        self.validate()?;
        let mut bytes = serde_json::to_vec_pretty(self)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

/// Minimal deterministic reproduction bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproductionBundle {
    /// Bundle schema version.
    pub api_version: String,
    /// Stable bundle ID.
    pub bundle_id: String,
    /// Normalized/minimized plan.
    pub normalized_plan: Value,
    /// Seed.
    pub seed: u64,
    /// Target engine.
    pub engine: String,
    /// Target protocol variant.
    pub variant: String,
    /// Required fidelity.
    pub fidelity: FidelityContract,
    /// Required provenance subset.
    pub provenance: ProvenanceEnvelope,
    /// Assertions expected to fail/reproduce.
    pub expected_assertions: Vec<String>,
    /// Required external blobs only.
    pub external_blobs: Vec<ExternalBlob>,
}

impl ReproductionBundle {
    /// Validate and serialize deterministically.
    pub fn to_canonical_json(&self) -> Result<Vec<u8>, ArtifactError> {
        if self.api_version != REPRODUCTION_BUNDLE_VERSION {
            return Err(ArtifactError::Version(self.api_version.clone()));
        }
        self.fidelity.validate(&self.provenance)?;
        let mut bytes = serde_json::to_vec_pretty(self)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

/// Artifact validation failure.
#[derive(Debug, Error)]
pub enum ArtifactError {
    /// Unsupported schema version.
    #[error("unsupported artifact version: {0}")]
    Version(String),
    /// Fidelity combination is invalid.
    #[error("invalid fidelity contract: {0}")]
    InvalidFidelity(String),
    /// Events are not in total order.
    #[error("event trace is not ordered by virtual_time_ns, ordinal, event_id")]
    EventOrder,
    /// Blob path escapes the artifact directory.
    #[error("unsafe external blob path: {0}")]
    UnsafeBlobPath(String),
    /// Blob read failed.
    #[error("cannot read external blob {path}: {source}")]
    BlobRead {
        /// Relative path.
        path: String,
        /// I/O error.
        source: std::io::Error,
    },
    /// Blob length mismatch.
    #[error("external blob {path} size mismatch: expected {expected}, got {actual}")]
    BlobSize {
        /// Relative path.
        path: String,
        /// Declared bytes.
        expected: u64,
        /// Actual bytes.
        actual: u64,
    },
    /// Blob checksum mismatch.
    #[error("external blob {path} checksum mismatch: expected {expected}, got {actual}")]
    BlobChecksum {
        /// Relative path.
        path: String,
        /// Declared checksum.
        expected: String,
        /// Actual checksum.
        actual: String,
    },
    /// JSON serialization failure.
    #[error("artifact JSON failure: {0}")]
    Json(#[from] serde_json::Error),
}

fn event_key(event: &EventRecord) -> (u64, u64, &str) {
    (
        event.virtual_time_ns,
        event.ordinal,
        event.event_id.as_str(),
    )
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
#[path = "artifact_tests.rs"]
mod tests;
