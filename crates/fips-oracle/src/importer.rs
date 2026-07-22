use crate::PINNED_FIPS_COMMIT;
use fips_model::NormalizedPlan;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MappingDisposition {
    Exact,
    Approximated,
    PreservedMetadata,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MappingDiagnostic {
    pub source_path: String,
    pub disposition: MappingDisposition,
    pub target_path: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChaosSourceMetadata {
    pub source_path: String,
    pub fips_commit: String,
    pub source_sha256: String,
    pub preserved_fields: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportResult {
    pub kind: String,
    pub plan: NormalizedPlan,
    pub diagnostics: Vec<MappingDiagnostic>,
    pub source: ChaosSourceMetadata,
}

pub fn import_chaos_yaml(source_path: &str, bytes: &[u8]) -> Result<ImportResult, ImportError> {
    let raw: Value = serde_yaml::from_slice(bytes)?;
    let scenario = raw.get("scenario").cloned().unwrap_or_else(|| json!({}));
    let topology = raw.get("topology").cloned().unwrap_or_else(|| json!({}));
    let nodes = topology
        .get("num_nodes")
        .and_then(Value::as_u64)
        .unwrap_or(10);
    let seed = scenario.get("seed").and_then(Value::as_u64).unwrap_or(42);
    let name = scenario
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("imported-chaos");
    let algorithm = topology
        .get("algorithm")
        .and_then(Value::as_str)
        .unwrap_or("random_geometric");
    let (generator, topology_disposition) = match algorithm {
        "chain" => ("chain", MappingDisposition::Exact),
        "explicit" => ("explicit", MappingDisposition::Exact),
        "erdos_renyi" => ("random-regular", MappingDisposition::Approximated),
        "random_geometric" => ("random-regular", MappingDisposition::Approximated),
        _ => ("balanced-tree", MappingDisposition::Unsupported),
    };
    let adjacency = topology
        .pointer("/params/adjacency")
        .and_then(Value::as_array);
    let explicit_edges = adjacency.map(|edges| {
        edges
            .iter()
            .filter_map(parse_edge)
            .map(|(a, b)| json!([a, b]))
            .collect::<Vec<_>>()
    });
    let netem = raw.get("netem").cloned().unwrap_or_else(|| json!({}));
    let delay_ms = midpoint(netem.pointer("/default_policy/delay_ms")).unwrap_or(1.0);
    let loss_pct = midpoint(netem.pointer("/default_policy/loss_pct")).unwrap_or(0.0);
    let traffic_enabled = raw
        .pointer("/traffic/enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let flow_count = raw
        .pointer("/traffic/max_concurrent")
        .and_then(Value::as_u64)
        .unwrap_or(1);
    let transport = if topology.get("transport_mix").is_some() {
        "heterogeneous"
    } else {
        match topology
            .get("default_transport")
            .and_then(Value::as_str)
            .unwrap_or("udp")
        {
            "tcp" => "all-tcp",
            "ethernet" => "all-ethernet",
            _ => "all-udp",
        }
    };
    let mut campaign = json!({
        "apiVersion": "experiments.fips.network/v1alpha1",
        "kind": "Campaign",
        "metadata": {"name": format!("imported-{name}"), "description": format!("Imported from FIPS chaos scenario {source_path}")},
        "seed": seed,
        "engine": {"modes": "compact-discrete-event", "deterministic": true},
        "scale": {"nodes": nodes},
        "topology": {"generator": generator, "average_degree": 2, "connected": topology.get("ensure_connected").and_then(Value::as_bool).unwrap_or(true)},
        "identities": {"initial": {"distribution": "deterministic-chaos-seed"}, "arrivals": {"count": 0, "schedule": {"start": "2s", "interval": "500ms"}, "address_policy": "strictly-lower-than-current-root", "attachment": "current-root"}},
        "transports": {"assignment": transport},
        "links": {"latency": format!("{}us", (delay_ms * 1000.0).round() as u64), "bandwidth_bps": 1000000000, "loss_ppm": (loss_pct * 10000.0).round() as u64, "duplication_ppm": 0, "ordering": "stream", "mtu_bytes": 1500, "queue_bytes": 1048576, "drop_policy": "tail-drop"},
        "resources": {"assignment": "uniform", "node_profiles": [{"name": "daemon", "cpu_units": 1000, "memory_bytes": 1073741824, "queue_bytes": 1048576, "table_entries": 1000}]},
        "events": [{"id": "chaos-source", "action": "introduce-lower-root-identities"}],
        "adversaries": {"mode": "none", "actions": [], "budgets": {"operations": 0, "identities": 0}},
        "protocol": {"variant": "fips-80c956a-baseline", "parameters": {"tree_announce_debounce": "500ms", "bloom_update_debounce": "500ms", "bloom_max_fpr_ppm": 200000, "coord_cache_entries": 64, "coord_cache_ttl": "5s", "lookup_ttl": 64, "lookup_attempts": 3}},
        "traffic": {"model": if traffic_enabled {"session-churn"} else {"uniform-random"}, "rate_bps": if traffic_enabled {1000000} else {0}, "payload_bytes": 512, "parameters": {"flow_count": if traffic_enabled {flow_count} else {0}}},
        "fidelity": {"protocol": "semantic-exact", "serialization": "captured-wire", "bloom": "exact-bits", "crypto": "execute", "billion_node_representation": "not-requested"},
        "accounting": {"causal_lineage": true, "transport_overhead": true, "network_overhead": "configured", "reconcile_serialized_frames": true},
        "instrumentation": {"root_agreement_by_depth": true, "transition_stages": true, "causal_cost_ledger": true, "queue_wait": true, "control_and_useful_bytes": true, "quiescence_markers": ["root", "tree", "bloom", "lookup", "data-plane"]},
        "assertions": [{"always": {"condition": "no_forwarding_loops"}}],
        "objectives": {"maximize": ["control_bytes_per_root_arrival"]}
    });
    if let Some(edges) = explicit_edges {
        campaign
            .pointer_mut("/topology")
            .and_then(Value::as_object_mut)
            .unwrap()
            .insert("explicit_edges".to_owned(), Value::Array(edges));
    }
    let plan = fips_model::normalize_str(&serde_json::to_string(&campaign)?)?;
    let diagnostics = diagnostics(&raw, algorithm, topology_disposition, traffic_enabled);
    Ok(ImportResult {
        kind: "chaos-import/v1alpha1".to_owned(),
        plan,
        diagnostics,
        source: ChaosSourceMetadata {
            source_path: source_path.to_owned(),
            fips_commit: PINNED_FIPS_COMMIT.to_owned(),
            source_sha256: hex::encode(Sha256::digest(bytes)),
            preserved_fields: raw,
        },
    })
}

fn diagnostics(
    raw: &Value,
    algorithm: &str,
    topology_disposition: MappingDisposition,
    traffic: bool,
) -> Vec<MappingDiagnostic> {
    let mut output = vec![MappingDiagnostic {
        source_path: "/topology/algorithm".to_owned(),
        disposition: topology_disposition,
        target_path: Some("/topology/generator".to_owned()),
        message: format!("mapped chaos topology {algorithm}"),
    }];
    for (path, target) in [
        ("/netem/default_policy", "/links"),
        ("/traffic", "/traffic"),
    ] {
        output.push(MappingDiagnostic {
            source_path: path.to_owned(),
            disposition: if path == "/traffic" && !traffic {
                MappingDisposition::Exact
            } else {
                MappingDisposition::Approximated
            },
            target_path: Some(target.to_owned()),
            message: "stochastic ranges collapse to deterministic midpoint; raw source retained"
                .to_owned(),
        });
    }
    output.push(MappingDiagnostic {
        source_path: "/topology/transport_mix".to_owned(),
        disposition: if raw.pointer("/topology/transport_mix").is_some() {
            MappingDisposition::Approximated
        } else {
            MappingDisposition::Exact
        },
        target_path: Some("/transports/assignment".to_owned()),
        message: "transport ratios map to a deterministic assignment class; raw ratios retained"
            .to_owned(),
    });
    let known = BTreeSet::from([
        "scenario",
        "topology",
        "netem",
        "link_flaps",
        "traffic",
        "node_churn",
        "peer_churn",
        "bandwidth",
        "ingress",
        "link_swap",
        "assertions",
        "logging",
        "fips_overrides",
    ]);
    if let Some(object) = raw.as_object() {
        for key in object.keys().filter(|key| !known.contains(key.as_str())) {
            output.push(MappingDiagnostic {
                source_path: format!("/{key}"),
                disposition: MappingDisposition::PreservedMetadata,
                target_path: None,
                message: "Docker-only or unknown field retained in source metadata".to_owned(),
            });
        }
    }
    for key in [
        "link_flaps",
        "node_churn",
        "peer_churn",
        "bandwidth",
        "ingress",
        "link_swap",
        "fips_overrides",
    ] {
        if raw.get(key).is_some() {
            output.push(MappingDiagnostic { source_path: format!("/{key}"), disposition: MappingDisposition::PreservedMetadata, target_path: None, message: "harness behavior retained as source metadata; semantic model support is partial".to_owned() });
        }
    }
    output
}

fn parse_edge(edge: &Value) -> Option<(u64, u64)> {
    let pair = edge.as_array()?;
    Some((
        node_index(pair.first()?.as_str()?)?,
        node_index(pair.get(1)?.as_str()?)?,
    ))
}
fn node_index(value: &str) -> Option<u64> {
    value.strip_prefix('n')?.parse::<u64>().ok()?.checked_sub(1)
}
fn midpoint(value: Option<&Value>) -> Option<f64> {
    let value = value?;
    if let Some(array) = value.as_array() {
        return Some((array.first()?.as_f64()? + array.get(1)?.as_f64()?) / 2.0);
    }
    let object = value.as_object()?;
    Some((object.get("min")?.as_f64()? + object.get("max")?.as_f64()?) / 2.0)
}

#[derive(Debug, Error)]
pub enum ImportError {
    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Model(#[from] fips_model::ModelError),
}
