//! Normalized scenario algebra and stable experiment-case compilation.

use fips_model::{NORMALIZED_PLAN_VERSION, NormalizedPlan};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;

/// One stable selection from every normalized campaign axis.
pub type CaseSelection = BTreeMap<String, Value>;

/// Fully resolved executable experiment case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentCase {
    /// Formatting-independent content ID.
    pub case_id: String,
    /// Fully resolved normalized plan.
    pub plan: NormalizedPlan,
    /// Explicit selected axis values.
    pub selections: CaseSelection,
    /// Compiler-derived choices and defaults.
    pub derived: BTreeMap<String, Value>,
}

/// Guardrails for case compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompilerLimits {
    /// Maximum full Cartesian size accepted before planning.
    pub maximum_matrix_cases: u64,
}

impl Default for CompilerLimits {
    fn default() -> Self {
        Self {
            maximum_matrix_cases: 1_000_000,
        }
    }
}

/// Deterministic normalized-case compiler.
#[derive(Debug, Clone)]
pub struct CaseCompiler {
    limits: CompilerLimits,
}

impl CaseCompiler {
    /// Create a compiler with explicit matrix limits.
    pub fn new(limits: CompilerLimits) -> Self {
        Self { limits }
    }

    /// Validate full Cartesian size without materializing it.
    pub fn matrix_size(&self, plan: &NormalizedPlan) -> Result<u64, CompileError> {
        let mut size = 1_u64;
        for axis in &plan.axes {
            size = size.checked_mul(axis.values.len() as u64).ok_or(
                CompileError::ExplosiveMatrix {
                    cases: u64::MAX,
                    maximum: self.limits.maximum_matrix_cases,
                },
            )?;
            if size > self.limits.maximum_matrix_cases {
                return Err(CompileError::ExplosiveMatrix {
                    cases: size,
                    maximum: self.limits.maximum_matrix_cases,
                });
            }
        }
        Ok(size)
    }

    /// Resolve one complete selection into a stable executable case.
    pub fn compile(
        &self,
        source: &NormalizedPlan,
        selection: CaseSelection,
    ) -> Result<ExperimentCase, CompileError> {
        self.matrix_size(source)?;
        let mut campaign = source.campaign.clone();
        for axis in &source.axes {
            let selected = selection
                .get(&axis.path)
                .ok_or_else(|| CompileError::MissingAxis(axis.path.clone()))?;
            if !axis.values.contains(selected) {
                return Err(CompileError::UnknownChoice {
                    path: axis.path.clone(),
                    value: selected.clone(),
                });
            }
            replace_pointer(&mut campaign, &axis.path, selected.clone())?;
        }
        if let Some(extra) = selection
            .keys()
            .find(|path| !source.axes.iter().any(|axis| axis.path == **path))
        {
            return Err(CompileError::UnknownAxis(extra.clone()));
        }
        evaluate_constraints(&campaign)?;
        compatibility(&campaign)?;
        let campaign_bytes = serde_json::to_vec(&campaign)?;
        let campaign_sha256 = hex::encode(Sha256::digest(&campaign_bytes));
        let case_id = format!("case-{}", &campaign_sha256[..24]);
        let derived = [
            (
                "engine.variant".to_owned(),
                campaign
                    .pointer("/engine/variant")
                    .cloned()
                    .unwrap_or(Value::Null),
            ),
            (
                "topology.connected".to_owned(),
                campaign
                    .pointer("/topology/connected")
                    .cloned()
                    .unwrap_or(Value::Bool(true)),
            ),
        ]
        .into_iter()
        .collect();
        Ok(ExperimentCase {
            case_id,
            plan: NormalizedPlan {
                api_version: NORMALIZED_PLAN_VERSION.to_owned(),
                campaign_sha256,
                campaign,
                axes: Vec::new(),
                seed: source.seed,
            },
            selections: selection,
            derived,
        })
    }
}

impl Default for CaseCompiler {
    fn default() -> Self {
        Self::new(CompilerLimits::default())
    }
}

/// Case compilation failure.
#[derive(Debug, Error)]
pub enum CompileError {
    /// Matrix is too large to plan accidentally.
    #[error("campaign matrix has {cases} cases, above configured maximum {maximum}")]
    ExplosiveMatrix { cases: u64, maximum: u64 },
    /// Required axis was omitted.
    #[error("selection omits normalized dimension {0}")]
    MissingAxis(String),
    /// Selection names no normalized dimension.
    #[error("selection names unknown normalized dimension {0}")]
    UnknownAxis(String),
    /// Choice does not belong to the axis.
    #[error("selection {value} is not available for dimension {path}")]
    UnknownChoice { path: String, value: Value },
    /// JSON pointer cannot be resolved.
    #[error("cannot resolve normalized dimension {0}")]
    Pointer(String),
    /// User constraint failed with named dimensions.
    #[error("constraint failed between {left} and {right}: {expression}")]
    Constraint {
        expression: String,
        left: String,
        right: String,
    },
    /// Engine/fidelity selection is invalid.
    #[error("incompatible dimensions {left} and {right}: {message}")]
    Compatibility {
        left: String,
        right: String,
        message: String,
    },
    /// Canonical JSON failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

fn replace_pointer(
    root: &mut Value,
    pointer: &str,
    replacement: Value,
) -> Result<(), CompileError> {
    let slot = root
        .pointer_mut(pointer)
        .ok_or_else(|| CompileError::Pointer(pointer.to_owned()))?;
    *slot = replacement;
    Ok(())
}

fn evaluate_constraints(campaign: &Value) -> Result<(), CompileError> {
    let constraints = campaign
        .pointer("/objectives/constraints")
        .and_then(Value::as_array)
        .into_iter()
        .flatten();
    for value in constraints {
        let expression = value.as_str().unwrap_or_default();
        let parts = expression.split_whitespace().collect::<Vec<_>>();
        if parts.len() != 3 {
            continue;
        }
        let left = resolve_operand(campaign, parts[0]);
        let right = resolve_operand(campaign, parts[2]);
        let passed = match parts[1] {
            "==" => left == right,
            "!=" => left != right,
            "<" => numeric(&left) < numeric(&right),
            "<=" => numeric(&left) <= numeric(&right),
            ">" => numeric(&left) > numeric(&right),
            ">=" => numeric(&left) >= numeric(&right),
            _ => true,
        };
        if !passed {
            return Err(CompileError::Constraint {
                expression: expression.to_owned(),
                left: parts[0].to_owned(),
                right: parts[2].to_owned(),
            });
        }
    }
    Ok(())
}

fn resolve_operand(campaign: &Value, token: &str) -> Value {
    if token.starts_with('/') {
        campaign.pointer(token).cloned().unwrap_or(Value::Null)
    } else {
        serde_json::from_str(token).unwrap_or_else(|_| Value::String(token.to_owned()))
    }
}

fn numeric(value: &Value) -> f64 {
    value.as_f64().unwrap_or(f64::NAN)
}

fn compatibility(campaign: &Value) -> Result<(), CompileError> {
    let nodes = campaign.pointer("/scale/nodes").and_then(Value::as_u64);
    let mode = campaign.pointer("/engine/modes").and_then(Value::as_str);
    if nodes.is_some_and(|count| count > u64::from(u32::MAX))
        && mode == Some("compact-discrete-event")
    {
        return Err(CompileError::Compatibility {
            left: "/scale/nodes".to_owned(),
            right: "/engine/modes".to_owned(),
            message: "individual stable IDs are u32; select cohort-hybrid".to_owned(),
        });
    }
    Ok(())
}
