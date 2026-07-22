use crate::PINNED_FIPS_COMMIT;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OracleSuiteKind {
    PullRequestSmoke,
    NightlyCalibration,
    HistoricalRegression,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleSuiteCase {
    pub id: String,
    pub kind: OracleSuiteKind,
    pub revision: String,
    pub expected: String,
    pub maximum_minutes: u64,
    pub cache_key: String,
    pub retain_failure_bundle: bool,
}

pub fn default_oracle_suites() -> Vec<OracleSuiteCase> {
    vec![
        OracleSuiteCase {
            id: "smoke-root-ratchet".to_owned(),
            kind: OracleSuiteKind::PullRequestSmoke,
            revision: PINNED_FIPS_COMMIT.to_owned(),
            expected: "unsupported-observation".to_owned(),
            maximum_minutes: 10,
            cache_key: format!("image:{PINNED_FIPS_COMMIT}"),
            retain_failure_bundle: true,
        },
        OracleSuiteCase {
            id: "nightly-bloom-storm".to_owned(),
            kind: OracleSuiteKind::NightlyCalibration,
            revision: PINNED_FIPS_COMMIT.to_owned(),
            expected: "classified".to_owned(),
            maximum_minutes: 45,
            cache_key: format!("image:{PINNED_FIPS_COMMIT}"),
            retain_failure_bundle: true,
        },
        OracleSuiteCase {
            id: "historical-known-good".to_owned(),
            kind: OracleSuiteKind::HistoricalRegression,
            revision: PINNED_FIPS_COMMIT.to_owned(),
            expected: "exact-match".to_owned(),
            maximum_minutes: 60,
            cache_key: format!("image:{PINNED_FIPS_COMMIT}"),
            retain_failure_bundle: true,
        },
        OracleSuiteCase {
            id: "historical-known-bad-fixture".to_owned(),
            kind: OracleSuiteKind::HistoricalRegression,
            revision: "fixture-known-bad-bloom-storm".to_owned(),
            expected: "implementation-bug".to_owned(),
            maximum_minutes: 60,
            cache_key: "fixture:known-bad-bloom-storm".to_owned(),
            retain_failure_bundle: true,
        },
    ]
}
