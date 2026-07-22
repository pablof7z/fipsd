use crate::AuditError;
use fips_artifact::RunArtifact;
use fips_atlas::qualify;
use fips_query::analyze;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeterminismDimension {
    pub dimension: String,
    pub values: Vec<String>,
    pub policy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeterminismAudit {
    pub repetitions: usize,
    pub artifact_sha256: String,
    pub analysis_sha256: String,
    pub atlas_sha256: String,
    pub dimensions: Vec<DeterminismDimension>,
    pub numeric_policy: String,
    pub passed: bool,
}

pub fn audit(artifact: &RunArtifact) -> Result<DeterminismAudit, AuditError> {
    artifact.validate()?;
    let repetitions = 3;
    let artifact_hashes = (0..repetitions)
        .map(|_| artifact.to_canonical_json().map(|bytes| hash(&bytes)))
        .collect::<Result<Vec<_>, _>>()?;
    let analysis_hashes = (0..repetitions)
        .map(|_| -> Result<String, AuditError> {
            let document = analyze(artifact)?;
            Ok(hash(&serde_json::to_vec(&document)?))
        })
        .collect::<Result<Vec<_>, AuditError>>()?;
    let atlas_hashes = (0..repetitions)
        .map(|_| -> Result<String, AuditError> {
            let atlas = qualify()?;
            Ok(hash(&serde_json::to_vec(&atlas)?))
        })
        .collect::<Result<Vec<_>, AuditError>>()?;
    let passed =
        identical(&artifact_hashes) && identical(&analysis_hashes) && identical(&atlas_hashes);
    Ok(DeterminismAudit {
        repetitions,
        artifact_sha256: artifact_hashes[0].clone(),
        analysis_sha256: analysis_hashes[0].clone(),
        atlas_sha256: atlas_hashes[0].clone(),
        dimensions: vec![
            dimension("operating-system", &["Linux", "macOS"], "canonical evidence must match in CI"),
            dimension("architecture", &["x86_64", "aarch64"], "integer semantic evidence must match"),
            dimension("worker-count", &["1", "2", "4"], "stable scheduler ordering is worker-independent"),
            dimension("toolchain", &["1.85 minimum", "stable"], "schema and semantic evidence must match"),
        ],
        numeric_policy: "protocol semantics use integer units and require exact equality; calibrated statistical outputs compare declared bounds, never hidden float tolerances".to_owned(),
        passed,
    })
}

fn dimension(name: &str, values: &[&str], policy: &str) -> DeterminismDimension {
    DeterminismDimension {
        dimension: name.to_owned(),
        values: values.iter().map(ToString::to_string).collect(),
        policy: policy.to_owned(),
    }
}

fn identical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] == pair[1])
}

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
