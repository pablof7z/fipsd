use crate::document::{AnalysisDocument, AnalysisError, analyze};
use fips_artifact::RunArtifact;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetricDelta {
    pub metric: String,
    pub left: Option<String>,
    pub right: Option<String>,
    pub absolute: Option<String>,
    pub classification: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comparison {
    pub left: AnalysisDocument,
    pub right: AnalysisDocument,
    pub compatible: bool,
    pub compatibility_reason: String,
    pub deltas: Vec<MetricDelta>,
    pub first_semantic_divergence: Option<String>,
}

pub fn compare(left: &RunArtifact, right: &RunArtifact) -> Result<Comparison, AnalysisError> {
    let left_document = analyze(left)?;
    let right_document = analyze(right)?;
    let compatible = left_document.represented_nodes == right_document.represented_nodes;
    let compatibility_reason = if compatible {
        "represented node populations match".to_owned()
    } else {
        "represented node populations differ; deltas are informational only".to_owned()
    };
    let left_metrics = metric_map(&left_document);
    let right_metrics = metric_map(&right_document);
    let mut names = left_metrics
        .keys()
        .chain(right_metrics.keys())
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    let deltas = names
        .into_iter()
        .map(|name| delta(name, left_metrics.get(name), right_metrics.get(name)))
        .collect();
    let first_semantic_divergence = left
        .event_trace
        .iter()
        .zip(&right.event_trace)
        .find(|(a, b)| a != b)
        .map(|(event, _)| event.event_id.clone())
        .or_else(|| {
            (left.event_trace.len() != right.event_trace.len()).then(|| {
                format!(
                    "event-index-{}",
                    left.event_trace.len().min(right.event_trace.len())
                )
            })
        });
    Ok(Comparison {
        left: left_document,
        right: right_document,
        compatible,
        compatibility_reason,
        deltas,
        first_semantic_divergence,
    })
}

fn metric_map(document: &AnalysisDocument) -> BTreeMap<&str, &str> {
    document
        .metrics
        .iter()
        .filter_map(|metric| {
            metric
                .last
                .as_deref()
                .map(|value| (metric.name.as_str(), value))
        })
        .collect()
}

fn delta(name: &str, left: Option<&&str>, right: Option<&&str>) -> MetricDelta {
    let numeric = left
        .and_then(|value| value.parse::<i128>().ok())
        .zip(right.and_then(|value| value.parse::<i128>().ok()))
        .map(|(a, b)| b.saturating_sub(a).to_string());
    let classification = match (left, right) {
        (Some(a), Some(b)) if a == b => "match",
        (Some(_), Some(_)) => "semantic-divergence",
        (None, Some(_)) => "left-unobserved",
        (Some(_), None) => "right-unobserved",
        (None, None) => "unobserved",
    };
    MetricDelta {
        metric: name.to_owned(),
        left: left.map(ToString::to_string),
        right: right.map(ToString::to_string),
        absolute: numeric,
        classification: classification.to_owned(),
    }
}
