use crate::PINNED_FIPS_COMMIT;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonProvenance {
    pub fips_commit: String,
    pub git_dirty: bool,
    pub patch_sha256: Option<String>,
    pub binary_sha256: String,
    pub binary_version: String,
    pub image_digest: String,
    pub generated_config_sha256: BTreeMap<String, String>,
    pub scenario_compiler_version: String,
    pub docker_runtime_version: String,
    pub host_profile: String,
    pub public_bundle_redactions: Vec<String>,
}

impl DaemonProvenance {
    pub fn validate_for_comparison(&self) -> Result<(), ProvenanceError> {
        for (field, value) in [
            ("fips_commit", self.fips_commit.as_str()),
            ("binary_sha256", self.binary_sha256.as_str()),
            ("image_digest", self.image_digest.as_str()),
            (
                "scenario_compiler_version",
                self.scenario_compiler_version.as_str(),
            ),
            (
                "docker_runtime_version",
                self.docker_runtime_version.as_str(),
            ),
            ("host_profile", self.host_profile.as_str()),
        ] {
            if value.is_empty() {
                return Err(ProvenanceError::Missing(field));
            }
        }
        if self.fips_commit.len() != 40 {
            return Err(ProvenanceError::Commit(self.fips_commit.clone()));
        }
        if self.git_dirty && self.patch_sha256.is_none() {
            return Err(ProvenanceError::DirtyPatch);
        }
        Ok(())
    }
}

pub fn fixture_provenance(
    binary: &[u8],
    image_digest: &str,
    configs: &BTreeMap<String, Vec<u8>>,
) -> DaemonProvenance {
    DaemonProvenance {
        fips_commit: PINNED_FIPS_COMMIT.to_owned(),
        git_dirty: false,
        patch_sha256: None,
        binary_sha256: hex::encode(Sha256::digest(binary)),
        binary_version: "fips 0.4.1".to_owned(),
        image_digest: image_digest.to_owned(),
        generated_config_sha256: configs
            .iter()
            .map(|(name, bytes)| (name.clone(), hex::encode(Sha256::digest(bytes))))
            .collect(),
        scenario_compiler_version: crate::CHAOS_ADAPTER_VERSION.to_owned(),
        docker_runtime_version: "fixture-docker-27.5.1".to_owned(),
        host_profile: "recorded-linux-arm64-2cpu-4gib".to_owned(),
        public_bundle_redactions: vec![
            "private_key".to_owned(),
            "secret".to_owned(),
            "token".to_owned(),
        ],
    }
}

pub fn redact_public_bundle(value: &mut Value) -> Vec<String> {
    let mut removed = Vec::new();
    redact(value, "", &mut removed);
    removed
}

fn redact(value: &mut Value, path: &str, removed: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            let secrets = object
                .keys()
                .filter(|key| is_secret(key))
                .cloned()
                .collect::<Vec<_>>();
            for key in secrets {
                object.remove(&key);
                removed.push(format!("{path}/{key}"));
            }
            for (key, child) in object {
                redact(child, &format!("{path}/{key}"), removed);
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter_mut().enumerate() {
                redact(child, &format!("{path}/{index}"), removed);
            }
        }
        _ => {}
    }
}

fn is_secret(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "private_key" | "private-key" | "secret" | "password" | "token" | "nsec"
    )
}

#[derive(Debug, Error)]
pub enum ProvenanceError {
    #[error("required daemon provenance field is missing: {0}")]
    Missing(&'static str),
    #[error("daemon provenance has invalid full commit {0}")]
    Commit(String),
    #[error("dirty FIPS build requires a patch digest")]
    DirtyPatch,
}
