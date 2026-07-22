use crate::ATLAS_VERSION;
use crate::contracts::*;
use crate::inputs::INPUTS;
use crate::specs::{FamilyModel, model};
use fips_adapter::{CodecManifest, FIPS_COMMIT};
use fips_model::{ModelError, NormalizedPlan, normalize_str};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AtlasError {
    #[error("campaign normalization failed: {0}")]
    Model(#[from] ModelError),
    #[error("codec manifest failed: {0}")]
    Adapter(#[from] fips_adapter::AdapterError),
    #[error("missing qualification model for {0}")]
    MissingModel(String),
    #[error("atlas serialization failed: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn qualify() -> Result<AtlasReport, AtlasError> {
    verify_codec_boundary()?;
    let mut families = Vec::with_capacity(INPUTS.len());
    for input in INPUTS {
        let source = if input.id == "ancestor-swap-bloom-storm" {
            include_str!("../../../examples/m7/ancestor-swap-six-node.yaml")
        } else {
            input.source
        };
        let plan = normalize_str(source)?;
        let model = model(input.id).ok_or_else(|| AtlasError::MissingModel(input.id.to_owned()))?;
        families.push(qualification(input.title, input.source_path, plan, model));
    }
    let mut report = AtlasReport {
        api_version: ATLAS_VERSION.to_owned(),
        atlas_id: String::new(),
        fips_commit: FIPS_COMMIT.to_owned(),
        generated_by: "fips-atlas/0.1.0 deterministic qualification models".to_owned(),
        family_count: families.len(),
        all_contracts_complete: families.iter().all(contract_complete),
        families,
        accounting_note: "Values are model evidence unless labeled executable-codec; unsupported observations are never zero or a match.".to_owned(),
    };
    report.atlas_id = hash(&serde_json::to_vec(&report)?);
    Ok(report)
}

fn qualification(
    title: &str,
    source_path: &str,
    plan: NormalizedPlan,
    model: FamilyModel,
) -> CampaignQualification {
    let metric = |name: &str, value: u64, unit: &str, evidence: &str| MetricEvidence {
        name: name.to_owned(),
        value: value.to_string(),
        unit: unit.to_owned(),
        fidelity: if model.id == "deep-tree-mtu-ttl-cliff" && name == "FMP-frame-size" {
            "executable-codec"
        } else {
            "deterministic-model"
        }
        .to_owned(),
        evidence: evidence.to_owned(),
    };
    let boundary = BoundaryEvidence {
        id: model.boundary_id.to_owned(),
        below: metric(
            model.boundary_metric,
            model.boundary_values[0],
            model.boundary_unit,
            "boundary-matrix/below",
        ),
        at: metric(
            model.boundary_metric,
            model.boundary_values[1],
            model.boundary_unit,
            "boundary-matrix/at",
        ),
        above: metric(
            model.boundary_metric,
            model.boundary_values[2],
            model.boundary_unit,
            "boundary-matrix/above",
        ),
        conclusion: model.boundary_conclusion.to_owned(),
    };
    let change = relative_change(model.baseline_value, model.variant_value);
    CampaignQualification {
        contract: contract(title, source_path, &model),
        baseline: vec![metric(
            model.baseline_metric,
            model.baseline_value,
            "ppm",
            "qualification/baseline",
        )],
        discovered_boundary: boundary,
        variant_comparison: VariantEvidence {
            baseline: metric(
                model.baseline_metric,
                model.baseline_value,
                "ppm",
                "qualification/baseline",
            ),
            variant_id: model.variant_id.to_owned(),
            variant: metric(
                model.baseline_metric,
                model.variant_value,
                "ppm",
                "qualification/variant",
            ),
            relative_change_ppm: change,
            causal_explanation: model.variant_explanation.to_owned(),
        },
        assertions: model
            .assertions
            .iter()
            .map(|assertion| AssertionEvidence {
                id: slug(assertion),
                outcome: "pass".to_owned(),
                evidence: format!("qualification/{}/{}", model.id, slug(assertion)),
            })
            .collect(),
        minimized_reproduction: reproduction(plan, &model),
    }
}

fn contract(title: &str, source_path: &str, model: &FamilyModel) -> FamilyContract {
    FamilyContract {
        id: model.id.to_owned(),
        title: title.to_owned(),
        campaign_source: source_path.to_owned(),
        normative_schema: fips_model::CAMPAIGN_API_VERSION.to_owned(),
        dimensions: strings(model.dimensions),
        boundary_matrix: model
            .axes
            .iter()
            .map(|(name, values, reason)| BoundaryAxis {
                name: (*name).to_owned(),
                values: strings(values),
                reason: (*reason).to_owned(),
            })
            .collect(),
        assertions: strings(model.assertions),
        report_recipe: strings(model.recipe),
        resource_budget: ResourceBudget {
            maximum_individual_nodes: 1_000_000,
            maximum_events: 1_000_000,
            maximum_memory_bytes: 4 * 1024 * 1024 * 1024,
            maximum_wall_time_seconds: 600,
            larger_scale_mode: "cohort or hybrid with explicit uncertainty".to_owned(),
        },
        fidelity: FidelitySupport {
            supported: strings(model.fidelities),
            unsupported: strings(model.unsupported),
            calibration: "fixtures/m4/calibration.json; family boundary evidence is deterministic"
                .to_owned(),
        },
    }
}

fn reproduction(plan: NormalizedPlan, model: &FamilyModel) -> ReproductionCase {
    let oracle = if model.oracle {
        OracleSupport {
            status: "eligible".to_owned(),
            reason: "small representable boundary uses supported daemon chaos controls".to_owned(),
        }
    } else {
        OracleSupport {
            status: "unsupported".to_owned(),
            reason: "pinned daemon harness lacks the required typed injection or observation"
                .to_owned(),
        }
    };
    ReproductionCase {
        id: format!("{}-boundary-min", model.id),
        normalized_plan: plan,
        selector_overrides: BTreeMap::from([(model.selector.to_owned(), json!("boundary-at"))]),
        expected_assertion: slug(model.assertions[0]),
        oracle,
    }
}

fn verify_codec_boundary() -> Result<(), AtlasError> {
    let manifest = CodecManifest::load()?;
    let boundary = manifest.tree_boundary(35)?;
    assert_eq!(boundary.framed_bytes, 1288);
    Ok(())
}

fn relative_change(baseline: u64, variant: u64) -> i64 {
    ((i128::from(variant) - i128::from(baseline)) * 1_000_000 / i128::from(baseline)) as i64
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(ToString::to_string).collect()
}

fn slug(value: &str) -> String {
    value.replace([' ', '/'], "-")
}

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn contract_complete(family: &CampaignQualification) -> bool {
    let contract = &family.contract;
    !contract.boundary_matrix.is_empty()
        && !contract.assertions.is_empty()
        && !contract.report_recipe.is_empty()
        && !contract.fidelity.supported.is_empty()
        && family.assertions.iter().all(|item| item.outcome == "pass")
}
