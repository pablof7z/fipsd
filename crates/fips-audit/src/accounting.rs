use crate::AuditError;
use fips_artifact::RunArtifact;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountingProjection {
    pub name: String,
    pub observed: Option<u64>,
    pub unit: String,
    pub status: String,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountingAudit {
    pub fidelity: String,
    pub stage_totals: BTreeMap<String, u64>,
    pub projections: Vec<AccountingProjection>,
    pub exclusions: Vec<String>,
    pub unobserved: Vec<String>,
    pub passed: bool,
}

pub fn audit(artifact: &RunArtifact) -> Result<AccountingAudit, AuditError> {
    artifact.validate()?;
    let mut totals = BTreeMap::<String, u64>::new();
    for entry in &artifact.causal_ledger {
        let value = totals.entry(entry.stage.clone()).or_default();
        *value = value.saturating_add(entry.count);
    }
    let assertion = |id: &str| {
        artifact
            .assertion_results
            .iter()
            .any(|item| item.id == id && item.outcome == "pass")
    };
    let projections = vec![
        projection(
            "serialized frame bytes",
            totals.get("serialized").copied(),
            "bytes",
            assertion("causal-ledger-frame-reconciliation"),
            "causal ledger and executable frame evidence",
        ),
        projection(
            "transmitted wire bytes",
            totals.get("transmitted").copied(),
            "bytes",
            assertion("byte-reconciliation"),
            "per-edge transmitted equals delivered plus lost",
        ),
        projection(
            "useful payload",
            totals.get("useful-payload").copied(),
            "bytes",
            assertion("continuous-control-eventual-data-progress"),
            "application payload excludes protocol framing",
        ),
        projection(
            "resource work",
            totals.get("compute").copied(),
            "units",
            assertion("modeled-work-has-resource-receipts"),
            "each executed work item has a resource receipt",
        ),
    ];
    let passed = projections.iter().all(|item| item.status == "reconciled")
        && assertion("tree-lifecycle-reconciliation")
        && assertion("bloom-replacement-reconciliation");
    Ok(AccountingAudit {
        fidelity: artifact.manifest.fidelity.plain_language_statement(),
        stage_totals: totals,
        projections,
        exclusions: vec![
            "lower-layer host networking not represented by configured transport overhead"
                .to_owned(),
            "daemon metrics absent from telemetry remain unobserved".to_owned(),
        ],
        unobserved: vec![
            "host scheduler CPU time".to_owned(),
            "allocator peak RSS".to_owned(),
        ],
        passed,
    })
}

fn projection(
    name: &str,
    observed: Option<u64>,
    unit: &str,
    reconciled: bool,
    evidence: &str,
) -> AccountingProjection {
    AccountingProjection {
        name: name.to_owned(),
        observed,
        unit: unit.to_owned(),
        status: if observed.is_none() {
            "unobserved"
        } else if reconciled {
            "reconciled"
        } else {
            "failed"
        }
        .to_owned(),
        evidence: evidence.to_owned(),
    }
}
