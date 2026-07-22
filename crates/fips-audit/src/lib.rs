//! Release-readiness audits, measured envelopes, and package verification.

mod accounting;
mod benchmark;
mod determinism;
mod manifest;

pub use accounting::{AccountingAudit, AccountingProjection};
pub use benchmark::{BenchmarkCase, BenchmarkReport, benchmark};
pub use determinism::{DeterminismAudit, DeterminismDimension};
pub use manifest::{PackageFile, PackageManifest, build_manifest, verify_manifest};

use fips_artifact::RunArtifact;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const RELEASE_AUDIT_VERSION: &str = "experiments.fips.network/release-audit/v1alpha1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseAudit {
    pub api_version: String,
    pub release: String,
    pub determinism: DeterminismAudit,
    pub accounting: AccountingAudit,
    pub support_matrix: Vec<SupportEntry>,
    pub threat_controls: Vec<String>,
    pub limitations: Vec<String>,
    pub ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupportEntry {
    pub surface: String,
    pub status: String,
    pub evidence: String,
}

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("invalid artifact: {0}")]
    Artifact(#[from] fips_artifact::ArtifactError),
    #[error("atlas audit failed: {0}")]
    Atlas(#[from] fips_atlas::AtlasError),
    #[error("campaign model audit failed: {0}")]
    Model(#[from] fips_model::ModelError),
    #[error("analysis audit failed: {0}")]
    Query(#[from] fips_query::AnalysisError),
    #[error("serialization failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("package path is unsafe: {0}")]
    UnsafePath(String),
    #[error("package file exceeds limit: {0}")]
    SizeLimit(String),
    #[error("package checksum mismatch: {0}")]
    Checksum(String),
    #[error("cannot access {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
}

pub fn audit_release() -> Result<ReleaseAudit, AuditError> {
    let artifact: RunArtifact = serde_json::from_slice(include_bytes!(
        "../../../fixtures/m2/root-ratchet-recovery-artifact.json"
    ))?;
    let determinism = determinism::audit(&artifact)?;
    let accounting = accounting::audit(&artifact)?;
    let support_matrix = vec![
        support(
            "macOS arm64/x86_64 CLI",
            "supported",
            "CI plus clean-install script",
        ),
        support(
            "Linux arm64/x86_64 CLI",
            "supported",
            "CI plus release matrix",
        ),
        support(
            "static analysis browser",
            "supported",
            "local-file and static-export tests",
        ),
        support(
            "Windows",
            "unsupported",
            "not in v0.1 build or determinism matrix",
        ),
        support(
            "real Tor/Nym/BLE calibration",
            "unsupported",
            "abstract transport only",
        ),
    ];
    let ready = determinism.passed && accounting.passed;
    Ok(ReleaseAudit {
        api_version: RELEASE_AUDIT_VERSION.to_owned(),
        release: "0.1.0".to_owned(),
        determinism,
        accounting,
        support_matrix,
        threat_controls: vec![
            "artifact size limits and schema validation".to_owned(),
            "relative normalized bundle paths only".to_owned(),
            "no plugin execution in the analysis browser".to_owned(),
            "container harness has explicit network and host boundaries".to_owned(),
            "release bundles exclude secrets and host paths".to_owned(),
        ],
        limitations: vec![
            "daemon agreement is limited to metrics exposed by the pinned harness".to_owned(),
            "one-billion-node results are cohort or hybrid, never individual".to_owned(),
            "calibrated performance applies only to its recorded hardware profile".to_owned(),
        ],
        ready,
    })
}

fn support(surface: &str, status: &str, evidence: &str) -> SupportEntry {
    SupportEntry {
        surface: surface.to_owned(),
        status: status.to_owned(),
        evidence: evidence.to_owned(),
    }
}
