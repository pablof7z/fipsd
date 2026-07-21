//! Campaign v1alpha1 validation and byte-stable normalization.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use thiserror::Error;

/// Campaign API version supported by M0.
pub const CAMPAIGN_API_VERSION: &str = "experiments.fips.network/v1alpha1";
/// Normalized plan schema version emitted by M0.
pub const NORMALIZED_PLAN_VERSION: &str = "experiments.fips.network/normalized-plan/v1alpha1";
/// Checked-in Campaign JSON Schema.
pub const CAMPAIGN_SCHEMA: &str = include_str!("../../../schemas/campaign-v1alpha1.schema.json");

/// Validation or normalization failure with an actionable document path.
#[derive(Debug, Error)]
pub enum ModelError {
    /// The source file could not be read.
    #[error("cannot read campaign {path}: {source}")]
    Read {
        /// Source path.
        path: String,
        /// I/O error.
        source: std::io::Error,
    },
    /// YAML syntax or representation failed.
    #[error("invalid YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),
    /// The checked-in JSON Schema is invalid.
    #[error("invalid embedded Campaign schema: {0}")]
    Schema(String),
    /// The campaign violated the structural schema.
    #[error("campaign schema violation at {path}: {message}")]
    Validation {
        /// JSON pointer-like instance path.
        path: String,
        /// Validator diagnostic.
        message: String,
    },
    /// The campaign used a structurally valid but unsupported combination.
    #[error("unsupported campaign combination at {path}: {message}")]
    Unsupported {
        /// JSON pointer-like instance path.
        path: String,
        /// Semantic diagnostic.
        message: String,
    },
    /// Canonical JSON serialization failed.
    #[error("cannot serialize normalized plan: {0}")]
    Json(#[from] serde_json::Error),
}

/// One deterministic value-set axis retained in a normalized plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanAxis {
    /// JSON pointer to the selector.
    pub path: String,
    /// Canonically sorted candidate values.
    pub values: Vec<Value>,
}

/// M0's deterministic, unexpanded campaign plan.
///
/// Cartesian/t-wise expansion belongs to M3. M0 records every value axis in a
/// stable order and hashes the canonical campaign so identical semantic input
/// and seed produce identical bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedPlan {
    /// Normalized plan schema version.
    pub api_version: String,
    /// Stable SHA-256 over the canonical campaign document.
    pub campaign_sha256: String,
    /// Campaign content with object keys, selectors, and durations normalized.
    pub campaign: Value,
    /// Stable selector inventory sorted by path.
    pub axes: Vec<PlanAxis>,
    /// Campaign seed.
    pub seed: u64,
}

impl NormalizedPlan {
    /// Serialize as canonical pretty JSON with one final LF.
    pub fn to_canonical_json(&self) -> Result<Vec<u8>, ModelError> {
        let mut bytes = serde_json::to_vec_pretty(self)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

/// Load, validate, and normalize a Campaign file.
pub fn normalize_path(path: &Path) -> Result<NormalizedPlan, ModelError> {
    let source = fs::read_to_string(path).map_err(|source| ModelError::Read {
        path: path.display().to_string(),
        source,
    })?;
    normalize_str(&source)
}

/// Load and validate a Campaign file without producing a plan.
pub fn validate_path(path: &Path) -> Result<(), ModelError> {
    let source = fs::read_to_string(path).map_err(|source| ModelError::Read {
        path: path.display().to_string(),
        source,
    })?;
    validate_str(&source).map(|_| ())
}

/// Parse and validate Campaign YAML, returning its JSON representation.
pub fn validate_str(source: &str) -> Result<Value, ModelError> {
    let document: Value = serde_yaml::from_str(source)?;
    validate_document(&document)?;
    Ok(document)
}

/// Parse, validate, and normalize Campaign YAML.
pub fn normalize_str(source: &str) -> Result<NormalizedPlan, ModelError> {
    let mut document = validate_str(source)?;
    apply_defaults(&mut document);
    let campaign = canonicalize(document);
    let seed = campaign
        .get("seed")
        .and_then(Value::as_u64)
        .expect("schema requires an unsigned seed");

    let canonical_campaign = serde_json::to_vec(&campaign)?;
    let campaign_sha256 = hex_lower(&Sha256::digest(canonical_campaign));
    let mut axes = Vec::new();
    collect_axes(&campaign, "", &mut axes);
    axes.sort_by(|left, right| left.path.cmp(&right.path));

    Ok(NormalizedPlan {
        api_version: NORMALIZED_PLAN_VERSION.to_owned(),
        campaign_sha256,
        campaign,
        axes,
        seed,
    })
}

fn apply_defaults(document: &mut Value) {
    if let Some(engine) = document.get_mut("engine").and_then(Value::as_object_mut) {
        engine
            .entry("variant")
            .or_insert_with(|| Value::String("fips-80c956a-baseline".to_owned()));
    }
    if let Some(topology) = document.get_mut("topology").and_then(Value::as_object_mut) {
        topology.entry("connected").or_insert(Value::Bool(true));
    }
}

fn validate_document(document: &Value) -> Result<(), ModelError> {
    let schema: Value = serde_json::from_str(CAMPAIGN_SCHEMA)
        .map_err(|error| ModelError::Schema(error.to_string()))?;
    let validator = jsonschema::validator_for(&schema)
        .map_err(|error| ModelError::Schema(error.to_string()))?;
    if let Some(error) = validator.iter_errors(document).next() {
        return Err(ModelError::Validation {
            path: printable_path(error.instance_path.as_str()),
            message: error.to_string(),
        });
    }

    let representation = document
        .pointer("/fidelity/billion_node_representation")
        .and_then(Value::as_str);
    let has_billion = selector_values(document.pointer("/scale/nodes"))
        .into_iter()
        .filter_map(Value::as_u64)
        .any(|nodes| nodes >= 1_000_000_000);
    if has_billion && representation != Some("cohort-with-sampled-exact-regions") {
        return Err(ModelError::Unsupported {
            path: "/fidelity/billion_node_representation".to_owned(),
            message: "one-billion-node cases require cohort-with-sampled-exact-regions".to_owned(),
        });
    }
    Ok(())
}

fn selector_values(value: Option<&Value>) -> Vec<&Value> {
    match value {
        Some(Value::Object(object)) => object
            .get("values")
            .and_then(Value::as_array)
            .map_or_else(Vec::new, |values| values.iter().collect()),
        Some(value) => vec![value],
        None => Vec::new(),
    }
}

fn canonicalize(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let is_selector = object.len() == 1 && object.contains_key("values");
            let mut output = Map::new();
            let mut entries: Vec<_> = object.into_iter().collect();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            for (key, child) in entries {
                let mut child = canonicalize(child);
                if is_selector && key == "values" {
                    if let Value::Array(values) = &mut child {
                        values.sort_by_key(canonical_sort_key);
                        values.dedup();
                    }
                }
                output.insert(key, child);
            }
            Value::Object(output)
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize).collect()),
        Value::String(text) => normalize_duration(&text).unwrap_or(Value::String(text)),
        scalar => scalar,
    }
}

fn normalize_duration(text: &str) -> Option<Value> {
    let units = [
        ("ns", 1_u64),
        ("us", 1_000),
        ("ms", 1_000_000),
        ("s", 1_000_000_000),
        ("m", 60_000_000_000),
        ("h", 3_600_000_000_000),
    ];
    for (suffix, multiplier) in units {
        if let Some(number) = text.strip_suffix(suffix) {
            let amount = number.parse::<u64>().ok()?;
            let nanos = amount.checked_mul(multiplier)?;
            let mut object = Map::new();
            object.insert("nanoseconds".to_owned(), Value::from(nanos));
            return Some(Value::Object(object));
        }
    }
    None
}

fn canonical_sort_key(value: &Value) -> String {
    serde_json::to_string(value).expect("JSON value serialization is infallible")
}

fn collect_axes(value: &Value, path: &str, axes: &mut Vec<PlanAxis>) {
    match value {
        Value::Object(object) => {
            if object.len() == 1 {
                if let Some(Value::Array(values)) = object.get("values") {
                    axes.push(PlanAxis {
                        path: printable_path(path),
                        values: values.clone(),
                    });
                    return;
                }
            }
            for (key, child) in object {
                let escaped = key.replace('~', "~0").replace('/', "~1");
                collect_axes(child, &format!("{path}/{escaped}"), axes);
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                collect_axes(child, &format!("{path}/{index}"), axes);
            }
        }
        _ => {}
    }
}

fn printable_path(path: &str) -> String {
    if path.is_empty() {
        "/".to_owned()
    } else {
        path.to_owned()
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equivalent_mapping_order_normalizes_identically() {
        let left = r#"
apiVersion: experiments.fips.network/v1alpha1
kind: Campaign
metadata: {name: tiny}
seed: 7
engine: {modes: {values: [compact-discrete-event]}, deterministic: true}
scale: {nodes: {values: [10]}}
topology: {generator: {values: [chain]}}
identities: {initial: {distribution: uniform-128}}
transports: {assignment: {values: [all-udp]}}
traffic: {model: {values: [idle]}}
fidelity: {protocol: semantic-exact, serialization: executable-codec, billion_node_representation: cohort-with-sampled-exact-regions}
instrumentation: {transition_stages: true}
assertions: []
objectives: {maximize: [control-bytes]}
"#;
        let right = left.replace(
            "metadata: {name: tiny}\nseed: 7",
            "seed: 7\nmetadata: {name: tiny}",
        );
        assert_eq!(
            normalize_str(left).unwrap().to_canonical_json().unwrap(),
            normalize_str(&right).unwrap().to_canonical_json().unwrap()
        );
    }

    #[test]
    fn duration_units_normalize_to_nanoseconds() {
        assert_eq!(
            normalize_duration("500ms"),
            Some(serde_json::json!({"nanoseconds": 500_000_000_u64}))
        );
    }

    #[test]
    fn omitted_defaults_match_explicit_defaults() {
        let implicit = r#"
apiVersion: experiments.fips.network/v1alpha1
kind: Campaign
metadata: {name: tiny}
seed: 7
engine: {modes: compact-discrete-event, deterministic: true}
scale: {nodes: 10}
topology: {generator: chain}
identities: {initial: {distribution: uniform-128}}
transports: {assignment: all-udp}
traffic: {model: idle}
fidelity: {protocol: semantic-exact, serialization: executable-codec, billion_node_representation: not-requested}
instrumentation: {transition_stages: true}
assertions: []
objectives: {maximize: [control-bytes]}
"#;
        let explicit = implicit
            .replace(
                "engine: {modes: compact-discrete-event, deterministic: true}",
                "engine: {modes: compact-discrete-event, deterministic: true, variant: fips-80c956a-baseline}",
            )
            .replace(
                "topology: {generator: chain}",
                "topology: {generator: chain, connected: true}",
            );
        assert_eq!(
            normalize_str(implicit)
                .unwrap()
                .to_canonical_json()
                .unwrap(),
            normalize_str(&explicit)
                .unwrap()
                .to_canonical_json()
                .unwrap()
        );
    }
}
