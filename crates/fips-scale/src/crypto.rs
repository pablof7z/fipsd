use fips_artifact::ComputeFidelity;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CryptoMode {
    Execute,
    OperationCount,
    CalibratedCost,
    Unbounded,
    AdversarialBudget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalibrationProfile {
    pub id: String,
    pub benchmark_host: String,
    pub code_revision: String,
    pub benchmark_date: String,
    pub samples: u64,
    pub median_ns: BTreeMap<String, u64>,
    pub p95_ns: BTreeMap<String, u64>,
    pub uncertainty: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CryptoCostReport {
    pub mode: CryptoMode,
    pub fidelity: ComputeFidelity,
    pub semantic_outcome_digest: String,
    pub operation_counts: BTreeMap<String, u128>,
    pub calibrated_ns: Option<BTreeMap<String, u128>>,
    pub executed_operations: u128,
    pub budget: Option<u128>,
    pub exhausted: bool,
    pub profile: Option<CalibrationProfile>,
}

pub fn account_crypto(
    mode: CryptoMode,
    operations: &BTreeMap<String, u128>,
    represented_nodes: u64,
    profile: Option<CalibrationProfile>,
    budget: Option<u128>,
    semantic_outcome_digest: impl Into<String>,
) -> Result<CryptoCostReport, CryptoError> {
    let requested = operations.values().copied().sum::<u128>();
    if mode == CryptoMode::Execute && represented_nodes > 10_000 {
        return Err(CryptoError::ExecuteScale(represented_nodes));
    }
    if mode == CryptoMode::CalibratedCost && profile.is_none() {
        return Err(CryptoError::MissingProfile);
    }
    let calibrated_ns = profile.as_ref().map(|profile| {
        operations
            .iter()
            .map(|(name, count)| {
                let median = profile.median_ns.get(name).copied().unwrap_or(0);
                (name.clone(), count.saturating_mul(u128::from(median)))
            })
            .collect()
    });
    let allowed = if mode == CryptoMode::AdversarialBudget {
        budget.unwrap_or(0).min(requested)
    } else {
        requested
    };
    Ok(CryptoCostReport {
        mode,
        fidelity: match mode {
            CryptoMode::Execute => ComputeFidelity::Executed,
            CryptoMode::CalibratedCost => ComputeFidelity::Calibrated,
            CryptoMode::OperationCount | CryptoMode::AdversarialBudget => {
                ComputeFidelity::OperationCounted
            }
            CryptoMode::Unbounded => ComputeFidelity::None,
        },
        semantic_outcome_digest: semantic_outcome_digest.into(),
        operation_counts: operations.clone(),
        calibrated_ns,
        executed_operations: if mode == CryptoMode::Execute {
            allowed
        } else {
            0
        },
        budget,
        exhausted: mode == CryptoMode::AdversarialBudget && allowed < requested,
        profile,
    })
}

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("executed crypto is restricted to at most 10000 represented nodes, got {0}")]
    ExecuteScale(u64),
    #[error("calibrated-cost mode requires a versioned benchmark profile")]
    MissingProfile,
}
