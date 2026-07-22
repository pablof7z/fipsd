use crate::causal::{CausalSummary, summarize_causal};
use crate::network::{NetworkView, summarize_network};
use crate::root_wave::{RootWave, summarize_root_wave};
use fips_artifact::{MetricSeries, RunArtifact, ScaleFidelity};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;

pub const ANALYSIS_VERSION: &str = "experiments.fips.network/analysis/v1alpha1";
pub const EXACT_NODE_LIMIT: u64 = 200;
pub const AGGREGATE_NODE_LIMIT: u64 = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Representation {
    ExactGraph,
    Aggregated,
    Cohort,
    Hybrid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRange {
    pub collection: String,
    pub start: usize,
    pub end_exclusive: usize,
    pub total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FidelityLabel {
    pub exact: bool,
    pub statement: String,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetricSummary {
    pub name: String,
    pub unit: String,
    pub first: Option<String>,
    pub last: Option<String>,
    pub minimum: Option<String>,
    pub maximum: Option<String>,
    pub source: SourceRange,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuiescenceSummary {
    pub root_ns: Option<u64>,
    pub tree_ns: Option<u64>,
    pub bloom_ns: Option<u64>,
    pub lookup_ns: Option<u64>,
    pub data_plane_ns: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisDocument {
    pub api_version: String,
    pub artifact_id: String,
    pub run_id: String,
    pub represented_nodes: u64,
    pub representation: Representation,
    pub representation_boundaries: BTreeMap<String, u64>,
    pub fidelity: FidelityLabel,
    pub provenance: fips_artifact::ProvenanceEnvelope,
    pub assertions: BTreeMap<String, String>,
    pub metrics: Vec<MetricSummary>,
    pub quiescence: QuiescenceSummary,
    pub causal: CausalSummary,
    pub network: NetworkView,
    pub root_wave: RootWave,
    pub sample_count: usize,
    pub event_count: usize,
    pub normalized_plan: Value,
}

#[derive(Debug, Error)]
pub enum AnalysisError {
    #[error("invalid source artifact: {0}")]
    Artifact(#[from] fips_artifact::ArtifactError),
    #[error("cannot serialize analysis: {0}")]
    Json(#[from] serde_json::Error),
    #[error("analysis export path is unsafe: {0}")]
    UnsafePath(String),
    #[error("analysis export exceeds {limit} bytes: {actual}")]
    SizeLimit { limit: u64, actual: u64 },
    #[error("cannot access {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
}

pub fn analyze(artifact: &RunArtifact) -> Result<AnalysisDocument, AnalysisError> {
    artifact.validate()?;
    let fidelity = &artifact.manifest.fidelity;
    let representation = match fidelity.scale {
        ScaleFidelity::Cohort => Representation::Cohort,
        ScaleFidelity::Hybrid => Representation::Hybrid,
        ScaleFidelity::Individual if fidelity.represented_nodes <= EXACT_NODE_LIMIT => {
            Representation::ExactGraph
        }
        ScaleFidelity::Individual => Representation::Aggregated,
    };
    let uncertainty = fidelity
        .approximations
        .iter()
        .map(|item| format!("{}: {}", item.method, item.uncertainty))
        .collect::<Vec<_>>();
    let metrics = artifact.metric_series.iter().map(metric_summary).collect();
    let quiescence = quiescence(&artifact.metric_series);
    Ok(AnalysisDocument {
        api_version: ANALYSIS_VERSION.to_owned(),
        artifact_id: artifact.manifest.artifact_id.clone(),
        run_id: artifact.manifest.run_id.clone(),
        represented_nodes: fidelity.represented_nodes,
        representation,
        representation_boundaries: BTreeMap::from([
            ("exact_graph_max_nodes".to_owned(), EXACT_NODE_LIMIT),
            ("aggregate_max_nodes".to_owned(), AGGREGATE_NODE_LIMIT),
        ]),
        fidelity: FidelityLabel {
            exact: uncertainty.is_empty(),
            statement: fidelity.plain_language_statement(),
            uncertainty,
        },
        provenance: artifact.manifest.provenance.clone(),
        assertions: artifact
            .assertion_results
            .iter()
            .map(|item| (item.id.clone(), item.outcome.clone()))
            .collect(),
        metrics,
        quiescence,
        causal: summarize_causal(&artifact.causal_ledger),
        network: summarize_network(artifact, representation),
        root_wave: summarize_root_wave(artifact),
        sample_count: artifact.samples.len(),
        event_count: artifact.event_trace.len(),
        normalized_plan: artifact.normalized_plan.clone(),
    })
}

fn metric_summary(series: &MetricSeries) -> MetricSummary {
    let values = series
        .points
        .iter()
        .filter_map(|point| point.value.parse::<i128>().ok())
        .collect::<Vec<_>>();
    MetricSummary {
        name: series.name.clone(),
        unit: series.unit.clone(),
        first: series.points.first().map(|point| point.value.clone()),
        last: series.points.last().map(|point| point.value.clone()),
        minimum: values.iter().min().map(ToString::to_string),
        maximum: values.iter().max().map(ToString::to_string),
        source: SourceRange {
            collection: format!("metric_series/{}", series.name),
            start: 0,
            end_exclusive: series.points.len(),
            total: series.points.len(),
        },
    }
}

fn quiescence(series: &[MetricSeries]) -> QuiescenceSummary {
    let value = |name: &str| {
        series
            .iter()
            .find(|item| item.name == name)
            .and_then(|item| item.points.last())
            .and_then(|point| point.value.parse().ok())
    };
    QuiescenceSummary {
        root_ns: value("quiescence.root"),
        tree_ns: value("quiescence.tree"),
        bloom_ns: value("quiescence.bloom"),
        lookup_ns: value("quiescence.lookup"),
        data_plane_ns: value("quiescence.data-plane"),
    }
}
