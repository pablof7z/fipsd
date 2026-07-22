use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use fips_artifact::{LedgerEntry, ReproductionBundle, RunArtifact};
use fips_engine::{IndividualEngine, RecoveryReport, RootRatchetReport};
use fips_model::NormalizedPlan;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

mod campaign_commands;
use campaign_commands::CampaignCommand;
mod scale_commands;
use scale_commands::ScaleCommand;
mod oracle_commands;
use oracle_commands::OracleCommand;

#[derive(Debug, Parser)]
#[command(name = "fips-wind-tunnel")]
#[command(about = "Deterministic FIPS protocol experimentation")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Validate a Campaign v1alpha1 document.
    Validate {
        /// Campaign YAML path.
        campaign: PathBuf,
    },
    /// Emit the deterministic normalized plan as JSON.
    Normalize {
        /// Campaign YAML path.
        campaign: PathBuf,
        /// Write to this path instead of stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Run one resolved individual-node campaign and write immutable evidence.
    Run {
        /// Concrete Campaign YAML path (no unresolved value-set axes).
        campaign: PathBuf,
        /// Output directory containing artifact.json, reproduction.json, and report.json.
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Inspect an immutable artifact without simulation state or UI.
    Inspect {
        /// Run artifact JSON path.
        artifact: PathBuf,
        /// Restrict causal ledger output to this initiating causal ID.
        #[arg(long)]
        causal_id: Option<String>,
    },
    /// Replay a deterministic reproduction bundle.
    Replay {
        /// Reproduction bundle JSON path.
        bundle: PathBuf,
        /// Optional replay artifact output path.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// M1 compatibility command; M3 replaces this with hierarchical shrinking.
    MinimizeBundle {
        /// Existing reproduction bundle.
        bundle: PathBuf,
        /// Output path for the validated unchanged M1 bundle.
        #[arg(short, long)]
        output: PathBuf,
    },
    /// M3 campaign algebra, generation, search, shrinking, and corpus workflows.
    Campaign {
        #[command(subcommand)]
        command: CampaignCommand,
    },
    /// M4 cohort/hybrid scale, calibration, and protocol-variant workflows.
    Scale {
        #[command(subcommand)]
        command: ScaleCommand,
    },
    /// M5 real-daemon import, harness, telemetry, differential, and fuzz workflows.
    Oracle {
        #[command(subcommand)]
        command: OracleCommand,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Validate { campaign } => {
            fips_model::validate_path(&campaign)
                .with_context(|| format!("validation failed for {}", campaign.display()))?;
            println!("valid: {}", campaign.display());
        }
        Command::Normalize { campaign, output } => {
            let plan = fips_model::normalize_path(&campaign)
                .with_context(|| format!("normalization failed for {}", campaign.display()))?;
            let bytes = plan.to_canonical_json()?;
            if let Some(path) = output {
                fs::write(&path, bytes)
                    .with_context(|| format!("cannot write {}", path.display()))?;
            } else {
                print!(
                    "{}",
                    String::from_utf8(bytes).expect("canonical JSON is UTF-8")
                );
            }
        }
        Command::Run { campaign, output } => {
            let plan = fips_model::normalize_path(&campaign)
                .with_context(|| format!("normalization failed for {}", campaign.display()))?;
            let run = IndividualEngine
                .run_plan(&plan)
                .with_context(|| format!("run failed for {}", campaign.display()))?;
            fs::create_dir_all(&output)
                .with_context(|| format!("cannot create {}", output.display()))?;
            write_bytes(
                &output.join("artifact.json"),
                &run.artifact.to_canonical_json()?,
            )?;
            write_bytes(
                &output.join("reproduction.json"),
                &run.reproduction.to_canonical_json()?,
            )?;
            write_json(&output.join("report.json"), &run.report)?;
            if let Some(recovery) = &run.recovery_report {
                write_json(&output.join("recovery-report.json"), recovery)?;
            }
            println!(
                "run {}: {} nodes, {} arrivals, root {}, quiescent at {} ns",
                run.report.run_id,
                run.report.node_count,
                run.report.arrivals,
                run.report.final_root,
                run.report.quiescence_ns
            );
            println!("evidence: {}", output.display());
            if let Some(recovery) = &run.recovery_report {
                println!(
                    "M2 recovery: {} useful bytes delivered, Bloom/lookup/data quiescence at {}/{}/{} ns",
                    recovery.traffic.delivered_useful_bytes,
                    recovery.markers.bloom_ns,
                    recovery.markers.lookup_ns,
                    recovery.markers.throughput_ns
                );
            }
        }
        Command::Inspect {
            artifact,
            causal_id,
        } => {
            let bytes = fs::read(&artifact)
                .with_context(|| format!("cannot read {}", artifact.display()))?;
            let artifact_document: RunArtifact = serde_json::from_slice(&bytes)
                .with_context(|| format!("invalid artifact {}", artifact.display()))?;
            artifact_document.validate()?;
            let report = embedded_report(&artifact_document)?;
            let recovery = embedded_recovery_report(&artifact_document);
            let inspection = recovery.as_ref().map_or_else(
                || serde_json::to_value(&report),
                |recovery| {
                    serde_json::to_value(serde_json::json!({
                        "root": report,
                        "recovery": recovery,
                    }))
                },
            )?;
            let output = if let Some(causal_id) = causal_id {
                let entries = causal_subtree(&artifact_document.causal_ledger, &causal_id);
                if entries.is_empty() {
                    anyhow::bail!("artifact has no causal ledger entries for {causal_id}");
                }
                serde_json::json!({
                    "report": inspection,
                    "causal_root": causal_id,
                    "causal_tree": entries,
                })
            } else {
                inspection
            };
            println!(
                "{}",
                String::from_utf8(serde_json::to_vec_pretty(&output)?)?
            );
        }
        Command::Replay { bundle, output } => {
            let bytes =
                fs::read(&bundle).with_context(|| format!("cannot read {}", bundle.display()))?;
            let reproduction: ReproductionBundle = serde_json::from_slice(&bytes)
                .with_context(|| format!("invalid reproduction bundle {}", bundle.display()))?;
            reproduction.to_canonical_json()?;
            let plan: NormalizedPlan = serde_json::from_value(reproduction.normalized_plan.clone())
                .context("bundle normalized_plan is not a normalized plan")?;
            let run = IndividualEngine.run_plan(&plan)?;
            for expected in &reproduction.expected_assertions {
                if !run
                    .artifact
                    .assertion_results
                    .iter()
                    .any(|assertion| assertion.id == *expected && assertion.outcome == "pass")
                {
                    anyhow::bail!("replay did not satisfy expected assertion {expected}");
                }
            }
            if let Some(path) = output {
                write_bytes(&path, &run.artifact.to_canonical_json()?)?;
            }
            println!(
                "replayed {} as {}",
                reproduction.bundle_id, run.report.run_id
            );
        }
        Command::MinimizeBundle { bundle, output } => {
            let bytes =
                fs::read(&bundle).with_context(|| format!("cannot read {}", bundle.display()))?;
            let reproduction: ReproductionBundle = serde_json::from_slice(&bytes)
                .with_context(|| format!("invalid reproduction bundle {}", bundle.display()))?;
            write_bytes(&output, &reproduction.to_canonical_json()?)?;
            println!(
                "M1 placeholder preserved {} unchanged; hierarchical shrinking ships in M3",
                reproduction.bundle_id
            );
        }
        Command::Campaign { command } => campaign_commands::execute(command)?,
        Command::Scale { command } => scale_commands::execute(command)?,
        Command::Oracle { command } => oracle_commands::execute(command)?,
    }
    Ok(())
}

fn embedded_report(artifact: &RunArtifact) -> Result<RootRatchetReport> {
    artifact
        .samples
        .iter()
        .find_map(|sample| serde_json::from_value::<RootRatchetReport>(sample.clone()).ok())
        .context("artifact does not contain a root-ratchet report")
}

fn embedded_recovery_report(artifact: &RunArtifact) -> Option<RecoveryReport> {
    artifact
        .samples
        .iter()
        .find_map(|sample| serde_json::from_value::<RecoveryReport>(sample.clone()).ok())
}

fn causal_subtree<'a>(ledger: &'a [LedgerEntry], root: &str) -> Vec<&'a LedgerEntry> {
    let mut ids = BTreeSet::from([root.to_owned()]);
    loop {
        let before = ids.len();
        for entry in ledger {
            if entry
                .causal_parent
                .as_ref()
                .is_some_and(|parent| ids.contains(parent))
            {
                ids.insert(entry.causal_id.clone());
            }
        }
        if ids.len() == before {
            break;
        }
    }
    ledger
        .iter()
        .filter(|entry| ids.contains(&entry.causal_id))
        .collect()
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    write_bytes(path, &bytes)
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }
    fs::write(path, bytes).with_context(|| format!("cannot write {}", path.display()))
}
