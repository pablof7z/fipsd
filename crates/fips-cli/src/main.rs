use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use fips_artifact::{ReproductionBundle, RunArtifact};
use fips_engine::{IndividualEngine, RootRatchetReport};
use fips_model::NormalizedPlan;
use std::fs;
use std::path::{Path, PathBuf};

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
            println!(
                "run {}: {} nodes, {} arrivals, root {}, quiescent at {} ns",
                run.report.run_id,
                run.report.node_count,
                run.report.arrivals,
                run.report.final_root,
                run.report.quiescence_ns
            );
            println!("evidence: {}", output.display());
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
            println!(
                "{}",
                String::from_utf8(serde_json::to_vec_pretty(&report)?)?
            );
            if let Some(causal_id) = causal_id {
                let entries = artifact_document
                    .causal_ledger
                    .iter()
                    .filter(|entry| entry.causal_id == causal_id)
                    .collect::<Vec<_>>();
                if entries.is_empty() {
                    anyhow::bail!("artifact has no causal ledger entries for {causal_id}");
                }
                println!(
                    "{}",
                    String::from_utf8(serde_json::to_vec_pretty(&entries)?)?
                );
            }
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
