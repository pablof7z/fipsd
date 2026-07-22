use crate::{
    ComparableEvidence, ComparableTransition, DaemonEvidence, DaemonProvenance, HarnessBundle,
    NormalizedTelemetry, OracleBackend, OracleError, RawTelemetrySource, TELEMETRY_ADAPTER_VERSION,
    TelemetryInput, normalize_telemetry, to_yaml,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct ChaosProcessBackend {
    pub fips_checkout: PathBuf,
    pub output_root: PathBuf,
    pub duration_seconds: u64,
}

impl OracleBackend for ChaosProcessBackend {
    fn name(&self) -> &str {
        "pinned-fips-docker-chaos"
    }

    fn run(&self, bundle: &HarnessBundle, repeat: usize) -> Result<DaemonEvidence, OracleError> {
        let output = self.output_root.join(format!("repeat-{repeat:03}"));
        fs::create_dir_all(&output)?;
        let scenario_path = output.join("scenario.yaml");
        let mut scenario = bundle.scenario.clone();
        if let Some(slot) = scenario.pointer_mut("/logging/output_dir") {
            *slot = Value::String(output.display().to_string());
        }
        fs::write(
            &scenario_path,
            to_yaml(&HarnessBundle {
                scenario,
                ..bundle.clone()
            })?,
        )?;
        let script = self.fips_checkout.join("testing/chaos/scripts/chaos.sh");
        let process = Command::new(script)
            .arg(&scenario_path)
            .arg("--duration")
            .arg(self.duration_seconds.to_string())
            .current_dir(self.fips_checkout.join("testing/chaos"))
            .output()?;
        let raw = [process.stdout.as_slice(), process.stderr.as_slice()].concat();
        let telemetry = collect_chaos_telemetry(&output)?;
        Ok(DaemonEvidence {
            kind: "real-daemon-evidence/v1alpha1".to_owned(),
            comparable: comparable_from_telemetry(&telemetry),
            telemetry,
            provenance: capture_process_provenance(&self.fips_checkout, &output)?,
            raw_output_sha256: hex::encode(Sha256::digest(raw)),
            exit_code: process.status.code().unwrap_or(-1),
        })
    }
}

fn collect_chaos_telemetry(output: &Path) -> Result<NormalizedTelemetry, OracleError> {
    let mut sources = Vec::new();
    if let Some(tree_path) = latest_named_file(output, "tree-snapshot-final.json")? {
        let trees: Value = serde_json::from_slice(&fs::read(&tree_path)?)?;
        if let Some(object) = trees.as_object() {
            for (node_id, payload) in object {
                let mut payload = payload.clone();
                if let Some(object) = payload.as_object_mut() {
                    object.insert("node_id".to_owned(), Value::String(node_id.clone()));
                }
                sources.push(RawTelemetrySource {
                    id: format!("tree-final:{node_id}"),
                    kind: "control-snapshot".to_owned(),
                    captured_at_ns: 0,
                    payload,
                });
            }
        }
    }
    if let Some(iperf_path) = latest_named_file(output, "iperf3-results.json")? {
        sources.push(RawTelemetrySource {
            id: "iperf-final".to_owned(),
            kind: "iperf-json".to_owned(),
            captured_at_ns: 0,
            payload: serde_json::from_slice(&fs::read(&iperf_path)?)?,
        });
    }
    Ok(normalize_telemetry(TelemetryInput {
        adapter_version: TELEMETRY_ADAPTER_VERSION.to_owned(),
        sources,
        clock_offset_ns: 0,
        clock_uncertainty_ns: 1_000_000_000,
    })?)
}

fn comparable_from_telemetry(telemetry: &NormalizedTelemetry) -> ComparableEvidence {
    let roots = telemetry
        .nodes
        .iter()
        .filter_map(|node| node.root.value.as_ref())
        .collect::<BTreeSet<_>>();
    let evidence = telemetry
        .nodes
        .iter()
        .filter_map(|node| node.root.raw_source.as_ref())
        .cloned()
        .collect::<Vec<_>>()
        .join(",");
    let transitions = (!roots.is_empty()).then(|| ComparableTransition {
        ordinal: 0,
        kind: "root-agreement".to_owned(),
        node: "network".to_owned(),
        state: if roots.len() == 1 { "pass" } else { "fail" }.to_owned(),
        at_ns: 0,
        evidence,
    });
    ComparableEvidence {
        transitions: transitions.into_iter().collect(),
        frames: telemetry
            .frames
            .iter()
            .map(|frame| crate::ComparableFrame {
                id: frame.id.clone(),
                sha256: frame.sha256.clone(),
                size_bytes: frame.size_bytes,
                evidence_kind: "captured-wire".to_owned(),
            })
            .collect(),
        metrics: telemetry
            .metrics
            .iter()
            .filter_map(|(name, value)| value.value.map(|value| (name.clone(), value)))
            .collect(),
        unsupported_fields: BTreeSet::new(),
    }
}

fn capture_process_provenance(
    checkout: &Path,
    output: &Path,
) -> Result<DaemonProvenance, OracleError> {
    let binary = fs::read(checkout.join("testing/docker/fips"))?;
    let commit = command(checkout, &["git", "rev-parse", "HEAD"])?;
    let dirty = !command(checkout, &["git", "status", "--porcelain"])?.is_empty();
    let patch = dirty
        .then(|| command(checkout, &["git", "diff"]).map(|v| hex::encode(Sha256::digest(v))))
        .transpose()?;
    let image_digest = image_identity(checkout)?;
    let docker = command(
        checkout,
        &["docker", "version", "--format", "{{.Server.Version}}"],
    )?;
    let mut configs = BTreeMap::new();
    collect_config_hashes(output, output, &mut configs)?;
    let generated_root = checkout.join("testing/chaos/generated-configs/sim");
    let mut generated = BTreeMap::new();
    collect_config_hashes(&generated_root, &generated_root, &mut generated)?;
    configs.extend(
        generated
            .into_iter()
            .map(|(name, digest)| (format!("generated-configs/{name}"), digest)),
    );
    let provenance = DaemonProvenance {
        fips_commit: commit,
        git_dirty: dirty,
        patch_sha256: patch,
        binary_sha256: hex::encode(Sha256::digest(binary)),
        binary_version: command(
            checkout,
            &[
                "docker",
                "run",
                "--rm",
                "--entrypoint",
                "/usr/local/bin/fips",
                "fips-test:latest",
                "--version",
            ],
        )?,
        image_digest,
        generated_config_sha256: configs,
        scenario_compiler_version: crate::CHAOS_ADAPTER_VERSION.to_owned(),
        docker_runtime_version: docker,
        host_profile: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        public_bundle_redactions: vec!["private_key".to_owned(), "secret".to_owned()],
    };
    provenance.validate_for_comparison()?;
    Ok(provenance)
}

fn image_identity(checkout: &Path) -> Result<String, OracleError> {
    let id = command(
        checkout,
        &[
            "docker",
            "image",
            "inspect",
            "fips-test:latest",
            "--format",
            "{{.Id}}",
        ],
    )?;
    if id.is_empty() {
        return Err(ChaosProcessBackendError::MissingImageIdentity.into());
    }
    Ok(id)
}

fn latest_named_file(root: &Path, name: &str) -> Result<Option<PathBuf>, std::io::Error> {
    let mut matches = Vec::new();
    collect_named_files(root, name, &mut matches)?;
    matches.sort();
    Ok(matches.pop())
}

fn collect_named_files(
    path: &Path,
    name: &str,
    output: &mut Vec<PathBuf>,
) -> Result<(), std::io::Error> {
    if !path.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_named_files(&path, name, output)?;
        } else if path.file_name().and_then(|value| value.to_str()) == Some(name) {
            output.push(path);
        }
    }
    Ok(())
}

fn collect_config_hashes(
    root: &Path,
    path: &Path,
    output: &mut BTreeMap<String, String>,
) -> Result<(), std::io::Error> {
    if !path.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.path().is_dir() {
            collect_config_hashes(root, &entry.path(), output)?;
        } else if entry
            .path()
            .extension()
            .and_then(|v| v.to_str())
            .is_some_and(|ext| matches!(ext, "yaml" | "yml" | "env"))
        {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .display()
                .to_string();
            output.insert(relative, hex::encode(Sha256::digest(fs::read(path)?)));
        }
    }
    Ok(())
}

fn command(cwd: &Path, command: &[&str]) -> Result<String, OracleError> {
    let output = Command::new(command[0])
        .args(&command[1..])
        .current_dir(cwd)
        .output()?;
    if !output.status.success() {
        return Err(OracleError::Command(format!(
            "{}: {}",
            command.join(" "),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

#[derive(Debug, thiserror::Error)]
pub enum ChaosProcessBackendError {
    #[error("pinned daemon image has no inspectable image identity")]
    MissingImageIdentity,
}
