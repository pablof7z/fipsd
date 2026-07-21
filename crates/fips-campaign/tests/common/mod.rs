#![allow(dead_code)]

use fips_model::NormalizedPlan;
use std::path::PathBuf;

pub fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn search_plan() -> NormalizedPlan {
    fips_model::normalize_path(&repository().join("examples/m3/root-ratchet-search.yaml")).unwrap()
}

pub fn recovery_plan() -> NormalizedPlan {
    fips_model::normalize_path(&repository().join("examples/m2/root-ratchet-recovery.yaml"))
        .unwrap()
}
