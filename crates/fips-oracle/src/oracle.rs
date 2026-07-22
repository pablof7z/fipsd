use crate::{
    ComparableEvidence, ComparableTransition, DaemonProvenance, DifferentialReport, HarnessBundle,
    NormalizedTelemetry, OracleClassification, compare_evidence, compile_to_chaos,
};
use fips_artifact::{ReproductionBundle, RunArtifact};
use fips_campaign::{CorpusEntry, DaemonConfirmation};
use fips_model::NormalizedPlan;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DaemonEvidence {
    pub kind: String,
    pub comparable: ComparableEvidence,
    pub telemetry: NormalizedTelemetry,
    pub provenance: DaemonProvenance,
    pub raw_output_sha256: String,
    pub exit_code: i32,
}

pub trait OracleBackend {
    fn name(&self) -> &str;
    fn run(&self, bundle: &HarnessBundle, repeat: usize) -> Result<DaemonEvidence, OracleError>;
}

#[derive(Debug, Clone)]
pub struct RecordedBackend {
    pub id: String,
    pub evidence: Vec<DaemonEvidence>,
}

impl OracleBackend for RecordedBackend {
    fn name(&self) -> &str {
        &self.id
    }
    fn run(&self, _bundle: &HarnessBundle, repeat: usize) -> Result<DaemonEvidence, OracleError> {
        if self.evidence.is_empty() {
            return Err(OracleError::NoEvidence);
        }
        self.evidence
            .get(repeat % self.evidence.len())
            .cloned()
            .ok_or(OracleError::NoEvidence)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OracleRepeat {
    pub ordinal: usize,
    pub classification: OracleClassification,
    pub evidence_sha256: String,
    pub differential: DifferentialReport,
    pub exit_code: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OracleRunReport {
    pub kind: String,
    pub backend: String,
    pub repeats: Vec<OracleRepeat>,
    pub dominant_classification: OracleClassification,
    pub dominant_confidence_ppm: u64,
    pub stable: bool,
    pub oracle_predicate_held: bool,
    pub attached_daemon_evidence: Vec<DaemonEvidence>,
}

pub fn run_oracle<B: OracleBackend>(
    plan: &NormalizedPlan,
    model: &ComparableEvidence,
    backend: &B,
    repeats: usize,
    confidence_threshold_ppm: u64,
) -> Result<OracleRunReport, OracleError> {
    if repeats == 0 {
        return Err(OracleError::NoRepeats);
    }
    let bundle = compile_to_chaos(plan)?;
    let mut outcomes = Vec::new();
    let mut evidence = Vec::new();
    for ordinal in 0..repeats {
        let daemon = backend.run(&bundle, ordinal)?;
        let differential = compare_evidence(
            model,
            &daemon.comparable,
            &daemon.telemetry,
            &daemon.provenance,
        )?;
        let bytes = serde_json::to_vec(&daemon)?;
        outcomes.push(OracleRepeat {
            ordinal,
            classification: differential.classification,
            evidence_sha256: hex::encode(Sha256::digest(bytes)),
            differential,
            exit_code: daemon.exit_code,
        });
        evidence.push(daemon);
    }
    let mut counts = BTreeMap::new();
    for outcome in &outcomes {
        *counts
            .entry(format!("{:?}", outcome.classification))
            .or_insert(0_u64) += 1;
    }
    let dominant_name = counts
        .iter()
        .max_by_key(|(_, count)| *count)
        .map(|(name, _)| name.clone())
        .unwrap();
    let dominant = outcomes
        .iter()
        .find(|outcome| format!("{:?}", outcome.classification) == dominant_name)
        .unwrap()
        .classification;
    let confidence = counts[&dominant_name] * 1_000_000 / repeats as u64;
    let stable = confidence >= confidence_threshold_ppm;
    Ok(OracleRunReport {
        kind: "daemon-oracle-run/v1alpha1".to_owned(),
        backend: backend.name().to_owned(),
        repeats: outcomes,
        dominant_classification: dominant,
        dominant_confidence_ppm: confidence,
        stable,
        oracle_predicate_held: stable
            && matches!(
                dominant,
                OracleClassification::ImplementationBug | OracleClassification::ModelDrift
            ),
        attached_daemon_evidence: evidence,
    })
}

pub fn confirm_corpus_entry(
    entry: &mut CorpusEntry,
    report: &OracleRunReport,
) -> Result<(), OracleError> {
    if !report.stable {
        return Err(OracleError::Flaky(report.dominant_confidence_ppm));
    }
    entry.metadata.daemon_confirmation = DaemonConfirmation::DaemonConfirmed;
    Ok(())
}

pub fn comparable_from_artifact(artifact: &RunArtifact) -> ComparableEvidence {
    let root_agreement = artifact
        .assertion_results
        .iter()
        .find(|assertion| assertion.id == "root-agreement");
    ComparableEvidence {
        transitions: root_agreement
            .map(|assertion| ComparableTransition {
                ordinal: 0,
                kind: assertion.id.clone(),
                node: "network".to_owned(),
                state: assertion.outcome.clone(),
                at_ns: 0,
                evidence: format!("model-assertion:{}", assertion.id),
            })
            .into_iter()
            .collect(),
        frames: Vec::new(),
        metrics: artifact
            .metric_series
            .iter()
            .filter_map(|series| {
                series
                    .points
                    .last()
                    .and_then(|point| point.value.parse().ok())
                    .map(|value| (series.name.clone(), value))
            })
            .collect(),
        unsupported_fields: BTreeSet::new(),
    }
}

pub fn attach_daemon_evidence(
    bundle: ReproductionBundle,
    report: OracleRunReport,
) -> (ReproductionBundle, OracleRunReport) {
    (bundle, report)
}

#[derive(Debug, Error)]
pub enum OracleError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),
    #[error(transparent)]
    Compile(#[from] crate::CompileError),
    #[error(transparent)]
    Telemetry(#[from] crate::TelemetryError),
    #[error(transparent)]
    Provenance(#[from] crate::ProvenanceError),
    #[error(transparent)]
    Process(#[from] crate::ChaosProcessBackendError),
    #[error("oracle backend has no recorded evidence")]
    NoEvidence,
    #[error("oracle requires at least one repeat")]
    NoRepeats,
    #[error("daemon outcome is flaky at {0} ppm confidence")]
    Flaky(u64),
    #[error("external oracle command failed: {0}")]
    Command(String),
}
