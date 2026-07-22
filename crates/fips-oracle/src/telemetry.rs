use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;

pub const TELEMETRY_ADAPTER_VERSION: &str = "fips-control-80c956a/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ObservationStatus {
    Observed,
    Sampled,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation<T> {
    pub status: ObservationStatus,
    pub value: Option<T>,
    pub raw_source: Option<String>,
    pub sampling_window_ns: Option<(u64, u64)>,
}

impl<T> Observation<T> {
    pub fn unknown() -> Self {
        Self {
            status: ObservationStatus::Unknown,
            value: None,
            raw_source: None,
            sampling_window_ns: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawTelemetrySource {
    pub id: String,
    pub kind: String,
    pub captured_at_ns: u64,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TelemetryInput {
    pub adapter_version: String,
    pub sources: Vec<RawTelemetrySource>,
    pub clock_offset_ns: i64,
    pub clock_uncertainty_ns: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeObservation {
    pub node_id: String,
    pub root: Observation<String>,
    pub parent: Observation<String>,
    pub ancestry: Observation<Vec<String>>,
    pub bloom_occupancy_ppb: Observation<u64>,
    pub cache_entries: Observation<u64>,
    pub sessions: Observation<u64>,
    pub lookup_count: Observation<u64>,
    pub signal_count: Observation<u64>,
    pub queue_bytes: Observation<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturedFrame {
    pub id: String,
    pub source: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub executable_codec_commit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedTelemetry {
    pub kind: String,
    pub adapter_version: String,
    pub nodes: Vec<NodeObservation>,
    pub metrics: BTreeMap<String, Observation<u64>>,
    pub frames: Vec<CapturedFrame>,
    pub assertion_results: BTreeMap<String, Observation<String>>,
    pub raw_source_ids: Vec<String>,
    pub clock_offset_ns: i64,
    pub clock_uncertainty_ns: u64,
    pub unobservable_fields: Vec<String>,
}

pub fn normalize_telemetry(input: TelemetryInput) -> Result<NormalizedTelemetry, TelemetryError> {
    if input.adapter_version != TELEMETRY_ADAPTER_VERSION {
        return Err(TelemetryError::Version {
            expected: TELEMETRY_ADAPTER_VERSION.to_owned(),
            actual: input.adapter_version,
        });
    }
    let mut nodes = BTreeMap::<String, NodeObservation>::new();
    let mut metrics = BTreeMap::new();
    let mut frames = Vec::new();
    let mut assertions = BTreeMap::new();
    for source in &input.sources {
        match source.kind.as_str() {
            "control-snapshot" => ingest_control(source, &mut nodes),
            "iperf-json" => {
                let value = source
                    .payload
                    .pointer("/end/sum_received/bytes")
                    .and_then(Value::as_u64);
                metrics.insert(
                    "traffic.useful-payload-bytes".to_owned(),
                    observed(value, source, true),
                );
            }
            "frame-capture" => {
                if let (Some(id), Some(sha), Some(size)) = (
                    source.payload.get("id").and_then(Value::as_str),
                    source.payload.get("sha256").and_then(Value::as_str),
                    source.payload.get("size_bytes").and_then(Value::as_u64),
                ) {
                    frames.push(CapturedFrame {
                        id: id.to_owned(),
                        source: source.id.clone(),
                        sha256: sha.to_owned(),
                        size_bytes: size,
                        executable_codec_commit: source
                            .payload
                            .get("codec_commit")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                    });
                }
            }
            "assertions" => {
                if let Some(object) = source.payload.as_object() {
                    for (id, value) in object {
                        assertions.insert(
                            id.clone(),
                            observed(value.as_str().map(str::to_owned), source, false),
                        );
                    }
                }
            }
            "runner-log" | "daemon-log" | "mmp-stats" => {}
            other => return Err(TelemetryError::SourceKind(other.to_owned())),
        }
    }
    let mut nodes = nodes.into_values().collect::<Vec<_>>();
    nodes.sort_by(|left, right| left.node_id.cmp(&right.node_id));
    Ok(NormalizedTelemetry {
        kind: "normalized-daemon-telemetry/v1alpha1".to_owned(),
        adapter_version: TELEMETRY_ADAPTER_VERSION.to_owned(),
        nodes,
        metrics,
        frames,
        assertion_results: assertions,
        raw_source_ids: input
            .sources
            .iter()
            .map(|source| source.id.clone())
            .collect(),
        clock_offset_ns: input.clock_offset_ns,
        clock_uncertainty_ns: input.clock_uncertainty_ns,
        unobservable_fields: vec![
            "internal-scheduler-stage".to_owned(),
            "unlogged-cache-invalidation-cause".to_owned(),
        ],
    })
}

fn ingest_control(source: &RawTelemetrySource, nodes: &mut BTreeMap<String, NodeObservation>) {
    let id = source
        .payload
        .get("node_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned();
    nodes.insert(
        id.clone(),
        NodeObservation {
            node_id: id,
            root: observed(
                source
                    .payload
                    .get("root")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                source,
                false,
            ),
            parent: observed(
                source
                    .payload
                    .get("parent")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                source,
                false,
            ),
            ancestry: observed(
                source
                    .payload
                    .get("ancestry")
                    .and_then(Value::as_array)
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    }),
                source,
                false,
            ),
            bloom_occupancy_ppb: observed(
                source
                    .payload
                    .pointer("/bloom/occupancy_ppb")
                    .and_then(Value::as_u64),
                source,
                true,
            ),
            cache_entries: observed(
                source
                    .payload
                    .pointer("/cache/entries")
                    .and_then(Value::as_u64),
                source,
                true,
            ),
            sessions: observed(
                source
                    .payload
                    .pointer("/sessions/count")
                    .and_then(Value::as_u64),
                source,
                true,
            ),
            lookup_count: observed(
                source
                    .payload
                    .pointer("/stats/lookup/count")
                    .and_then(Value::as_u64),
                source,
                true,
            ),
            signal_count: observed(
                source
                    .payload
                    .pointer("/mmp/signals")
                    .and_then(Value::as_u64),
                source,
                true,
            ),
            queue_bytes: observed(
                source
                    .payload
                    .pointer("/stats/queue_bytes")
                    .and_then(Value::as_u64),
                source,
                true,
            ),
        },
    );
}

fn observed<T>(value: Option<T>, source: &RawTelemetrySource, sampled: bool) -> Observation<T> {
    match value {
        Some(value) => Observation {
            status: if sampled {
                ObservationStatus::Sampled
            } else {
                ObservationStatus::Observed
            },
            value: Some(value),
            raw_source: Some(source.id.clone()),
            sampling_window_ns: sampled.then_some((
                source.captured_at_ns.saturating_sub(1_000_000_000),
                source.captured_at_ns,
            )),
        },
        None => Observation::unknown(),
    }
}

#[derive(Debug, Error)]
pub enum TelemetryError {
    #[error("telemetry adapter version drift: expected {expected}, got {actual}")]
    Version { expected: String, actual: String },
    #[error("unsupported telemetry source kind {0}")]
    SourceKind(String),
}
