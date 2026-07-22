use crate::{CHAOS_ADAPTER_VERSION, MappingDiagnostic, MappingDisposition, PINNED_FIPS_COMMIT};
use fips_model::NormalizedPlan;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HarnessBundle {
    pub kind: String,
    pub adapter_version: String,
    pub fips_commit: String,
    pub normalized_plan_sha256: String,
    pub scenario: Value,
    pub deterministic_identity_ids: Vec<String>,
    pub diagnostics: Vec<MappingDiagnostic>,
}

pub fn compile_to_chaos(plan: &NormalizedPlan) -> Result<HarnessBundle, CompileError> {
    let campaign = &plan.campaign;
    let nodes = scalar_u64(campaign, "/scale/nodes")?;
    if nodes > 256 {
        return Err(CompileError::Unsupported(
            "/scale/nodes",
            "reduce to at most 256 exact daemon nodes".to_owned(),
        ));
    }
    if campaign
        .pointer("/fidelity/billion_node_representation")
        .and_then(Value::as_str)
        != Some("not-requested")
    {
        return Err(CompileError::Unsupported(
            "/fidelity/billion_node_representation",
            "shrink cohort/hybrid input to an individual sample".to_owned(),
        ));
    }
    if optional_u64(campaign, "/identities/arrivals/count").unwrap_or(0) > 0 {
        return Err(CompileError::Unsupported(
            "/identities/arrivals/count",
            "the pinned chaos harness cannot add identities at runtime; use an imported Bloom/churn case or shrink to a static exact topology".to_owned(),
        ));
    }
    ensure_representable_semantics(campaign)?;
    let generator = scalar_str(campaign, "/topology/generator")?;
    let (algorithm, params) = match generator {
        "chain" => ("chain", json!({})),
        "balanced-tree" => ("erdos_renyi", json!({"p": 0.3})),
        "random-regular" | "small-world" | "scale-free" => {
            ("random_geometric", json!({"radius": 0.5}))
        }
        "explicit" => (
            "explicit",
            json!({"adjacency": explicit_adjacency(campaign)}),
        ),
        other => {
            return Err(CompileError::Unsupported(
                "/topology/generator",
                format!("no chaos mapping for {other}; shrink to chain or explicit"),
            ));
        }
    };
    let latency_ns = duration(campaign, "/links/latency").unwrap_or(1_000_000);
    let loss_ppm = optional_u64(campaign, "/links/loss_ppm").unwrap_or(0);
    let traffic = optional_u64(campaign, "/traffic/parameters/flow_count").unwrap_or(0);
    let assignment = scalar_str(campaign, "/transports/assignment")?;
    let mut chaos_topology = json!({"num_nodes": nodes, "algorithm": algorithm, "params": params, "ensure_connected": true, "subnet": "172.31.0.0/24", "ip_start": 10});
    let topology = chaos_topology.as_object_mut().unwrap();
    match assignment {
        "all-udp" => topology.insert("default_transport".to_owned(), json!("udp")),
        "all-tcp" => topology.insert("default_transport".to_owned(), json!("tcp")),
        "all-ethernet" => topology.insert("default_transport".to_owned(), json!("ethernet")),
        "heterogeneous" => topology.insert(
            "transport_mix".to_owned(),
            json!({"udp": 0.34, "tcp": 0.33, "ethernet": 0.33}),
        ),
        other => {
            return Err(CompileError::Unsupported(
                "/transports/assignment",
                format!("no pinned chaos transport mapping for {other}"),
            ));
        }
    };
    let scenario = json!({
        "scenario": {"name": format!("compiled-{}", &plan.campaign_sha256[..12]), "seed": plan.seed, "duration_secs": 30},
        "topology": chaos_topology,
        "netem": {"enabled": latency_ns > 0 || loss_ppm > 0, "default_policy": {"delay_ms": [latency_ns as f64 / 1_000_000.0, latency_ns as f64 / 1_000_000.0], "jitter_ms": [0, 0], "loss_pct": [loss_ppm as f64 / 10_000.0, loss_ppm as f64 / 10_000.0]}},
        "link_flaps": {"enabled": false},
        "traffic": {"enabled": traffic > 0, "max_concurrent": traffic.min(16), "interval_secs": {"min": 1, "max": 1}, "duration_secs": {"min": 2, "max": 2}, "parallel_streams": 1},
        "logging": {"rust_log": "info", "output_dir": "./sim-results"},
        "fips_overrides": {}
    });
    let identities = (0..nodes)
        .map(|node| {
            let digest = Sha256::digest(
                [
                    plan.seed.to_le_bytes().as_slice(),
                    node.to_le_bytes().as_slice(),
                ]
                .concat(),
            );
            format!("identity-{}", &hex::encode(digest)[..24])
        })
        .collect();
    Ok(HarnessBundle {
        kind: "compiled-chaos-harness/v1alpha1".to_owned(),
        adapter_version: CHAOS_ADAPTER_VERSION.to_owned(),
        fips_commit: PINNED_FIPS_COMMIT.to_owned(),
        normalized_plan_sha256: plan.campaign_sha256.clone(),
        scenario,
        deterministic_identity_ids: identities,
        diagnostics: vec![
            MappingDiagnostic {
                source_path: "/topology/generator".to_owned(),
                disposition: if generator == algorithm || generator == "explicit" {
                    MappingDisposition::Exact
                } else {
                    MappingDisposition::Approximated
                },
                target_path: Some("/topology/algorithm".to_owned()),
                message: format!("compiled {generator} to chaos {algorithm}"),
            },
            MappingDiagnostic {
                source_path: "/transports/assignment".to_owned(),
                disposition: if assignment == "heterogeneous" {
                    MappingDisposition::Approximated
                } else {
                    MappingDisposition::Exact
                },
                target_path: Some("/topology/default_transport".to_owned()),
                message: format!("compiled deterministic transport assignment {assignment}"),
            },
        ],
    })
}

fn ensure_representable_semantics(campaign: &Value) -> Result<(), CompileError> {
    if campaign
        .pointer("/protocol/variant")
        .and_then(Value::as_str)
        != Some("fips-80c956a-baseline")
    {
        return Err(CompileError::Unsupported(
            "/protocol/variant",
            "use the pinned fips-80c956a-baseline daemon variant".to_owned(),
        ));
    }
    if campaign
        .pointer("/adversaries/mode")
        .and_then(Value::as_str)
        != Some("none")
        || campaign
            .pointer("/adversaries/actions")
            .and_then(Value::as_array)
            .is_some_and(|actions| !actions.is_empty())
    {
        return Err(CompileError::Unsupported(
            "/adversaries",
            "shrink authenticated semantic adversaries to a static daemon case".to_owned(),
        ));
    }
    Ok(())
}

pub fn to_yaml(bundle: &HarnessBundle) -> Result<Vec<u8>, serde_yaml::Error> {
    serde_yaml::to_string(&bundle.scenario).map(String::into_bytes)
}

fn scalar_u64(value: &Value, pointer: &'static str) -> Result<u64, CompileError> {
    value
        .pointer(pointer)
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            CompileError::Unsupported(
                pointer,
                "requires scalar integer after case compilation".to_owned(),
            )
        })
}
fn scalar_str<'a>(value: &'a Value, pointer: &'static str) -> Result<&'a str, CompileError> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| {
            CompileError::Unsupported(
                pointer,
                "requires scalar string after case compilation".to_owned(),
            )
        })
}
fn optional_u64(value: &Value, pointer: &str) -> Option<u64> {
    value.pointer(pointer).and_then(Value::as_u64)
}
fn duration(value: &Value, pointer: &str) -> Option<u64> {
    value
        .pointer(pointer)
        .and_then(|value| value.get("nanoseconds"))
        .and_then(Value::as_u64)
}
fn explicit_adjacency(campaign: &Value) -> Vec<Value> {
    campaign
        .pointer("/topology/explicit_edges")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|edge| {
            let pair = edge.as_array()?;
            Some(json!([
                format!("n{:02}", pair.first()?.as_u64()? + 1),
                format!("n{:02}", pair.get(1)?.as_u64()? + 1)
            ]))
        })
        .collect()
}

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("unsupported harness feature at {0}: {1}")]
    Unsupported(&'static str, String),
    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),
}
