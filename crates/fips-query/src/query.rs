use crate::document::{AnalysisError, SourceRange};
use fips_artifact::{EventRecord, RunArtifact};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventQuery {
    pub start_ns: Option<u64>,
    pub end_ns: Option<u64>,
    pub kinds: BTreeSet<String>,
    pub maximum_results: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventQueryResult {
    pub events: Vec<EventRecord>,
    pub matched: usize,
    pub truncated: bool,
    pub fidelity: String,
    pub source: SourceRange,
}

pub fn query_events(
    artifact: &RunArtifact,
    query: &EventQuery,
) -> Result<EventQueryResult, AnalysisError> {
    artifact.validate()?;
    let maximum = query.maximum_results.max(1);
    let matches = artifact
        .event_trace
        .iter()
        .enumerate()
        .filter(|(_, event)| {
            query
                .start_ns
                .is_none_or(|start| event.virtual_time_ns >= start)
        })
        .filter(|(_, event)| query.end_ns.is_none_or(|end| event.virtual_time_ns <= end))
        .filter(|(_, event)| query.kinds.is_empty() || query.kinds.contains(&event.kind))
        .collect::<Vec<_>>();
    let matched = matches.len();
    let selected = downsample(&matches, maximum);
    let start = selected.first().map_or(0, |(index, _)| *index);
    let end_exclusive = selected.last().map_or(0, |(index, _)| index + 1);
    Ok(EventQueryResult {
        events: selected
            .into_iter()
            .map(|(_, event)| event.clone())
            .collect(),
        matched,
        truncated: matched > maximum,
        fidelity: if matched > maximum {
            "deterministic-even-sample; first and last preserved".to_owned()
        } else {
            "exact".to_owned()
        },
        source: SourceRange {
            collection: "event_trace".to_owned(),
            start,
            end_exclusive,
            total: artifact.event_trace.len(),
        },
    })
}

fn downsample<'a>(
    matches: &[(usize, &'a EventRecord)],
    maximum: usize,
) -> Vec<(usize, &'a EventRecord)> {
    if matches.len() <= maximum {
        return matches.to_vec();
    }
    if maximum == 1 {
        return vec![matches[0]];
    }
    (0..maximum)
        .map(|slot| {
            let index = slot * (matches.len() - 1) / (maximum - 1);
            matches[index]
        })
        .collect()
}
