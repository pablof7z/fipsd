use crate::VariantIdentity;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const BASELINE_VARIANT: &str = "fips-80c956a-baseline";
pub const DAMPENING_VARIANT: &str = "root-dampening-v1alpha1";
pub const BLOOM_DELTA_VARIANT: &str = "bloom-delta-v1alpha1";

/// Deterministic protocol decision input independent of engine state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VariantContext {
    pub root_generation: u64,
    pub since_last_root_ns: u64,
    pub full_bloom_bytes: u128,
    pub changed_bloom_bits: u64,
}

/// Variant decisions used by all fidelity engines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariantDecision {
    pub adopt_root: bool,
    pub bloom_bytes: u128,
    pub decision: &'static str,
}

/// Versioned decision hooks; schedulers and artifact order remain engine-owned.
pub trait ProtocolVariant: Send + Sync {
    fn identity(&self) -> VariantIdentity;
    fn hooks(&self) -> VariantHooks;
    fn decide(&self, context: VariantContext) -> VariantDecision;
    fn supports(&self, fidelity: &str) -> bool;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariantHooks {
    pub state_version: String,
    pub root_eligibility: String,
    pub root_adoption: String,
    pub parent_choice: String,
    pub bloom_update: String,
    pub bloom_acceptance: String,
    pub lookup: String,
    pub timers: String,
    pub coordinate_strategy: String,
    pub mixed_version_support: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariantDivergence {
    pub variant: String,
    pub root_adoption_delta: i128,
    pub control_byte_delta: i128,
    pub bloom_fpr_delta_ppb: i128,
    pub attributed_to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariantComparison {
    pub kind: String,
    pub baseline: crate::CohortReport,
    pub candidates: Vec<crate::CohortReport>,
    pub divergences: Vec<VariantDivergence>,
    pub experimental_notice: String,
}

pub fn compare_variants(
    plan: &fips_model::NormalizedPlan,
) -> Result<VariantComparison, crate::ScaleError> {
    let baseline = crate::CohortEngine.run(plan, BASELINE_VARIANT)?.report;
    let candidates = [DAMPENING_VARIANT, BLOOM_DELTA_VARIANT]
        .into_iter()
        .map(|variant| crate::CohortEngine.run(plan, variant).map(|run| run.report))
        .collect::<Result<Vec<_>, _>>()?;
    let divergences = candidates
        .iter()
        .map(|candidate| VariantDivergence {
            variant: candidate.variant.id.clone(),
            root_adoption_delta: decimal(&candidate.metrics.root_adoptions.value)
                - decimal(&baseline.metrics.root_adoptions.value),
            control_byte_delta: decimal(&candidate.metrics.control_bytes.value)
                - decimal(&baseline.metrics.control_bytes.value),
            bloom_fpr_delta_ppb: decimal(&candidate.metrics.bloom_fpr_ppb.value)
                - decimal(&baseline.metrics.bloom_fpr_ppb.value),
            attributed_to: if candidate.variant.id == DAMPENING_VARIANT {
                "root eligibility/adoption tenure decisions"
            } else {
                "incremental Bloom construction decisions"
            }
            .to_owned(),
        })
        .collect();
    Ok(VariantComparison {
        kind: "protocol-variant-comparison/v1alpha1".to_owned(),
        baseline,
        candidates,
        divergences,
        experimental_notice:
            "Experimental variants are comparison proposals, not upstream recommendations."
                .to_owned(),
    })
}

fn decimal(value: &str) -> i128 {
    value.parse().unwrap_or(i128::MAX)
}

#[derive(Debug, Clone)]
struct ReferenceVariant {
    identity: VariantIdentity,
}

impl ProtocolVariant for ReferenceVariant {
    fn identity(&self) -> VariantIdentity {
        self.identity.clone()
    }

    fn hooks(&self) -> VariantHooks {
        let dampened = self.identity.id == DAMPENING_VARIANT;
        let delta = self.identity.id == BLOOM_DELTA_VARIANT;
        VariantHooks {
            state_version: if self.identity.experimental {
                "experimental-state/v1alpha1"
            } else {
                "fips-state/80c956a"
            }
            .to_owned(),
            root_eligibility: if dampened {
                "tenure-gated"
            } else {
                "minimum-address"
            }
            .to_owned(),
            root_adoption: if dampened {
                "750ms-hold-down"
            } else {
                "immediate-better-root"
            }
            .to_owned(),
            parent_choice: "lowest-cost-valid-parent".to_owned(),
            bloom_update: if delta {
                "incremental-delta"
            } else {
                "full-replacement"
            }
            .to_owned(),
            bloom_acceptance: "authenticated-antipoison-threshold".to_owned(),
            lookup: "fips-lookup-v1".to_owned(),
            timers: "injected-virtual-time".to_owned(),
            coordinate_strategy: "ancestry-derived-coordinate".to_owned(),
            mixed_version_support: false,
        }
    }

    fn decide(&self, context: VariantContext) -> VariantDecision {
        match self.identity.id.as_str() {
            DAMPENING_VARIANT => {
                let adopt =
                    context.root_generation == 0 || context.since_last_root_ns >= 750_000_000;
                VariantDecision {
                    adopt_root: adopt,
                    bloom_bytes: context.full_bloom_bytes,
                    decision: if adopt {
                        "tenure-expired"
                    } else {
                        "root-coalesced"
                    },
                }
            }
            BLOOM_DELTA_VARIANT => VariantDecision {
                adopt_root: true,
                bloom_bytes: u128::from(context.changed_bloom_bits).div_ceil(8) + 24,
                decision: "incremental-bloom-delta",
            },
            _ => VariantDecision {
                adopt_root: true,
                bloom_bytes: context.full_bloom_bytes,
                decision: "current-fips",
            },
        }
    }

    fn supports(&self, fidelity: &str) -> bool {
        matches!(fidelity, "individual" | "cohort" | "hybrid")
    }
}

/// Resolve a pinned reference variant and reject unknown/mixed versions.
pub fn resolve_variant(
    id: &str,
    parameters: &Value,
) -> Result<Box<dyn ProtocolVariant>, VariantError> {
    let (version, experimental) = match id {
        BASELINE_VARIANT => ("80c956a6fdb85dde1450969a21891c1158e43267", false),
        DAMPENING_VARIANT | BLOOM_DELTA_VARIANT => ("1alpha1", true),
        other if other.contains(',') || other.contains('+') => {
            return Err(VariantError::MixedVersion(other.to_owned()));
        }
        other => return Err(VariantError::Unknown(other.to_owned())),
    };
    let parameter_sha256 = hex::encode(Sha256::digest(serde_json::to_vec(parameters)?));
    Ok(Box::new(ReferenceVariant {
        identity: VariantIdentity {
            id: id.to_owned(),
            version: version.to_owned(),
            parameter_sha256,
            experimental,
        },
    }))
}

#[derive(Debug, Error)]
pub enum VariantError {
    #[error("unknown protocol variant {0}")]
    Unknown(String),
    #[error("mixed-version variant combinations are unsupported: {0}")]
    MixedVersion(String),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
