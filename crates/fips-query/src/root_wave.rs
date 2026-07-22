use fips_artifact::RunArtifact;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootWavePoint {
    pub causal_id: String,
    pub root: String,
    pub arrival_ns: u64,
    pub propagated_events: u64,
    pub status: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootWave {
    pub points: Vec<RootWavePoint>,
    pub final_consensus_root: Option<String>,
    pub fidelity: String,
}

pub fn summarize_root_wave(artifact: &RunArtifact) -> RootWave {
    let descendants = artifact
        .event_trace
        .iter()
        .filter_map(|event| event.causal_parent.as_deref())
        .fold(BTreeMap::<&str, u64>::new(), |mut counts, parent| {
            *counts.entry(parent).or_default() += 1;
            counts
        });
    let points = artifact
        .event_trace
        .iter()
        .filter(|event| event.kind == "input.descending-root-arrival")
        .map(|event| {
            let propagated_events = descendants
                .get(event.event_id.as_str())
                .copied()
                .unwrap_or(0);
            RootWavePoint {
                causal_id: event.event_id.clone(),
                root: event
                    .data
                    .get("address")
                    .and_then(|value| value.as_str())
                    .unwrap_or("unknown")
                    .to_owned(),
                arrival_ns: event.virtual_time_ns,
                propagated_events,
                status: if propagated_events == 0 {
                    "coalesced"
                } else {
                    "adopted"
                }
                .to_owned(),
            }
        })
        .collect();
    let final_consensus_root = artifact.samples.iter().find_map(|sample| {
        sample
            .get("final_root")
            .and_then(|value| value.as_str())
            .map(ToString::to_string)
    });
    RootWave {
        points,
        final_consensus_root,
        fidelity: "arrival lineage is exact; propagation counts include direct causal descendants recorded by the source artifact".to_owned(),
    }
}
