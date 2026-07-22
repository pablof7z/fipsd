#![allow(dead_code)]

use fips_model::NormalizedPlan;
use std::path::PathBuf;

pub fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn billion_plan() -> NormalizedPlan {
    fips_model::normalize_path(&repository().join("examples/m4/billion-root-ratchet.yaml")).unwrap()
}
