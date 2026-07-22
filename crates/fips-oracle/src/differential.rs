use crate::{DaemonProvenance, NormalizedTelemetry, ObservationStatus, ProvenanceError};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparableTransition {
    pub ordinal: u64,
    pub kind: String,
    pub node: String,
    pub state: String,
    pub at_ns: u64,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparableFrame {
    pub id: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub evidence_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparableEvidence {
    pub transitions: Vec<ComparableTransition>,
    pub frames: Vec<ComparableFrame>,
    pub metrics: BTreeMap<String, u64>,
    pub unsupported_fields: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DifferenceDisposition {
    ExactMatch,
    ToleratedTimingVariance,
    TimingDivergence,
    Unobservable,
    Unsupported,
    SemanticDivergence,
    FrameDivergence,
    MetricDivergence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Difference {
    pub path: String,
    pub disposition: DifferenceDisposition,
    pub model: Option<String>,
    pub daemon: Option<String>,
    pub model_evidence: Option<String>,
    pub daemon_evidence: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OracleClassification {
    ExactMatch,
    ToleratedNondeterminism,
    ModelDrift,
    ImplementationBug,
    NondeterministicEnvironment,
    UnsupportedObservation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DifferentialReport {
    pub kind: String,
    pub classification: OracleClassification,
    pub first_divergent_transition: Option<u64>,
    pub differences: Vec<Difference>,
    pub comparative_claim_allowed: bool,
    pub timing_tolerance_ns: u64,
}

pub fn compare_evidence(
    model: &ComparableEvidence,
    daemon: &ComparableEvidence,
    telemetry: &NormalizedTelemetry,
    provenance: &DaemonProvenance,
) -> Result<DifferentialReport, ProvenanceError> {
    provenance.validate_for_comparison()?;
    let tolerance = telemetry.clock_uncertainty_ns.max(1_000_000);
    let mut differences = Vec::new();
    let mut first = None;
    let transition_count = model.transitions.len().max(daemon.transitions.len());
    for index in 0..transition_count {
        match (model.transitions.get(index), daemon.transitions.get(index)) {
            (Some(left), Some(right))
                if left.kind == right.kind
                    && left.node == right.node
                    && left.state == right.state =>
            {
                let delta = left.at_ns.abs_diff(right.at_ns);
                let disposition = if delta == 0 {
                    DifferenceDisposition::ExactMatch
                } else if delta <= tolerance {
                    DifferenceDisposition::ToleratedTimingVariance
                } else {
                    first.get_or_insert(index as u64);
                    DifferenceDisposition::TimingDivergence
                };
                differences.push(Difference {
                    path: format!("/transitions/{index}"),
                    disposition,
                    model: Some(left.state.clone()),
                    daemon: Some(right.state.clone()),
                    model_evidence: Some(left.evidence.clone()),
                    daemon_evidence: Some(right.evidence.clone()),
                    message: format!("aligned transition; time delta {delta} ns"),
                });
            }
            (Some(left), Some(right)) => {
                first.get_or_insert(index as u64);
                differences.push(Difference {
                    path: format!("/transitions/{index}"),
                    disposition: DifferenceDisposition::SemanticDivergence,
                    model: Some(format!("{}:{}:{}", left.kind, left.node, left.state)),
                    daemon: Some(format!("{}:{}:{}", right.kind, right.node, right.state)),
                    model_evidence: Some(left.evidence.clone()),
                    daemon_evidence: Some(right.evidence.clone()),
                    message: "first state-transition divergence".to_owned(),
                });
            }
            (Some(left), None) => {
                differences.push(Difference {
                    path: format!("/transitions/{index}"),
                    disposition: DifferenceDisposition::Unobservable,
                    model: Some(left.state.clone()),
                    daemon: None,
                    model_evidence: Some(left.evidence.clone()),
                    daemon_evidence: None,
                    message: "daemon did not expose this transition; not called a match".to_owned(),
                });
            }
            (None, Some(right)) => {
                first.get_or_insert(index as u64);
                differences.push(Difference {
                    path: format!("/transitions/{index}"),
                    disposition: DifferenceDisposition::SemanticDivergence,
                    model: None,
                    daemon: Some(right.state.clone()),
                    model_evidence: None,
                    daemon_evidence: Some(right.evidence.clone()),
                    message: "daemon emitted an extra transition".to_owned(),
                });
            }
            (None, None) => {}
        }
    }
    compare_frames(model, daemon, &mut differences);
    compare_metrics(model, daemon, telemetry, &mut differences);
    for field in model.unsupported_fields.union(&daemon.unsupported_fields) {
        differences.push(Difference {
            path: format!("/unsupported/{field}"),
            disposition: DifferenceDisposition::Unsupported,
            model: None,
            daemon: None,
            model_evidence: None,
            daemon_evidence: None,
            message: "comparison adapter does not support this field".to_owned(),
        });
    }
    let classification = classify(&differences);
    Ok(DifferentialReport {
        kind: "daemon-differential-report/v1alpha1".to_owned(),
        classification,
        first_divergent_transition: first,
        comparative_claim_allowed: !matches!(
            classification,
            OracleClassification::UnsupportedObservation
        ),
        differences,
        timing_tolerance_ns: tolerance,
    })
}

fn compare_frames(
    model: &ComparableEvidence,
    daemon: &ComparableEvidence,
    output: &mut Vec<Difference>,
) {
    let count = model.frames.len().max(daemon.frames.len());
    for index in 0..count {
        let (left, right) = (model.frames.get(index), daemon.frames.get(index));
        let captured = left.is_some_and(|frame| frame.evidence_kind == "executable-codec")
            && right.is_some_and(|frame| frame.evidence_kind == "captured-wire");
        output.push(Difference {
            path: format!("/frames/{index}"),
            disposition: match (left, right, captured) {
                (Some(left), Some(right), true)
                    if left.sha256 == right.sha256 && left.size_bytes == right.size_bytes =>
                {
                    DifferenceDisposition::ExactMatch
                }
                (Some(_), Some(_), true) => DifferenceDisposition::FrameDivergence,
                _ => DifferenceDisposition::Unobservable,
            },
            model: left.map(|frame| format!("{}:{}", frame.sha256, frame.size_bytes)),
            daemon: right.map(|frame| format!("{}:{}", frame.sha256, frame.size_bytes)),
            model_evidence: left.map(|frame| frame.id.clone()),
            daemon_evidence: right.map(|frame| frame.id.clone()),
            message: if captured {
                "byte comparison uses executable and captured evidence"
            } else {
                "missing executable/captured frame evidence; not called a match"
            }
            .to_owned(),
        });
    }
}

fn compare_metrics(
    model: &ComparableEvidence,
    daemon: &ComparableEvidence,
    telemetry: &NormalizedTelemetry,
    output: &mut Vec<Difference>,
) {
    let keys = model
        .metrics
        .keys()
        .chain(daemon.metrics.keys())
        .collect::<BTreeSet<_>>();
    for key in keys {
        let left = model.metrics.get(key);
        let right = daemon.metrics.get(key);
        let observed = telemetry
            .metrics
            .get(key)
            .is_some_and(|value| value.status != ObservationStatus::Unknown);
        output.push(Difference {
            path: format!("/metrics/{key}"),
            disposition: match (left, right, observed) {
                (Some(a), Some(b), true) if a == b => DifferenceDisposition::ExactMatch,
                (Some(_), Some(_), true) => DifferenceDisposition::MetricDivergence,
                _ => DifferenceDisposition::Unobservable,
            },
            model: left.map(u64::to_string),
            daemon: right.map(u64::to_string),
            model_evidence: Some(format!("model-metric:{key}")),
            daemon_evidence: telemetry
                .metrics
                .get(key)
                .and_then(|value| value.raw_source.clone()),
            message: if observed {
                "sampled metric compared in aligned window"
            } else {
                "metric unobserved or unknown; not called a match"
            }
            .to_owned(),
        });
    }
}

fn classify(differences: &[Difference]) -> OracleClassification {
    if differences
        .iter()
        .any(|item| item.disposition == DifferenceDisposition::SemanticDivergence)
    {
        return OracleClassification::ImplementationBug;
    }
    if differences
        .iter()
        .any(|item| item.disposition == DifferenceDisposition::FrameDivergence)
    {
        return OracleClassification::ModelDrift;
    }
    if differences
        .iter()
        .any(|item| item.disposition == DifferenceDisposition::MetricDivergence)
    {
        return OracleClassification::NondeterministicEnvironment;
    }
    if differences
        .iter()
        .any(|item| item.disposition == DifferenceDisposition::TimingDivergence)
    {
        return OracleClassification::NondeterministicEnvironment;
    }
    if differences.iter().any(|item| {
        matches!(
            item.disposition,
            DifferenceDisposition::Unobservable | DifferenceDisposition::Unsupported
        )
    }) {
        return OracleClassification::UnsupportedObservation;
    }
    if differences
        .iter()
        .any(|item| item.disposition == DifferenceDisposition::ToleratedTimingVariance)
    {
        return OracleClassification::ToleratedNondeterminism;
    }
    OracleClassification::ExactMatch
}
