//! Deterministic qualification atlas for the ten flagship campaign families.

mod contracts;
mod evaluate;
mod inputs;
mod specs;

pub use contracts::{
    AssertionEvidence, AtlasReport, BoundaryAxis, BoundaryEvidence, CampaignQualification,
    FamilyContract, FidelitySupport, MetricEvidence, OracleSupport, ReproductionCase,
    ResourceBudget, VariantEvidence,
};
pub use evaluate::{AtlasError, qualify};

pub const ATLAS_VERSION: &str = "experiments.fips.network/qualification-atlas/v1alpha1";
