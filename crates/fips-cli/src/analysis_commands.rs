use anyhow::{Context, Result, bail};
use clap::Subcommand;
use fips_artifact::RunArtifact;
use fips_query::{EventQuery, ExportLimits, analyze, compare, export_static, query_events};
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Subcommand)]
pub enum AnalysisCommand {
    /// Build a deterministic analysis index without changing the source artifact.
    Index {
        artifact: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Query an artifact event window with deterministic bounded sampling.
    Query {
        artifact: PathBuf,
        #[arg(long)]
        start_ns: Option<u64>,
        #[arg(long)]
        end_ns: Option<u64>,
        #[arg(long = "kind")]
        kinds: Vec<String>,
        #[arg(long, default_value_t = 1_000)]
        maximum_results: usize,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Compare compatible artifact metrics and the first semantic divergence.
    Compare {
        left: PathBuf,
        right: PathBuf,
        /// Confirm an external normalization made different populations comparable.
        #[arg(long)]
        normalized: bool,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Export a self-contained static report and cited source artifact.
    Export {
        artifact: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, default_value_t = 64)]
        maximum_mebibytes: u64,
    },
}

pub fn execute(command: AnalysisCommand) -> Result<()> {
    match command {
        AnalysisCommand::Index { artifact, output } => {
            let document = analyze(&read_artifact(&artifact)?)?;
            write_json(&output, &document)?;
            println!("analysis index: {}", output.display());
        }
        AnalysisCommand::Query {
            artifact,
            start_ns,
            end_ns,
            kinds,
            maximum_results,
            output,
        } => {
            let result = query_events(
                &read_artifact(&artifact)?,
                &EventQuery {
                    start_ns,
                    end_ns,
                    kinds: BTreeSet::from_iter(kinds),
                    maximum_results,
                },
            )?;
            write_json(&output, &result)?;
            println!("event query: {} matches", result.matched);
        }
        AnalysisCommand::Compare {
            left,
            right,
            normalized,
            output,
        } => {
            let document = compare(&read_artifact(&left)?, &read_artifact(&right)?)?;
            if !document.compatible && !normalized {
                bail!(
                    "incompatible artifacts: {}; pass --normalized only after applying a documented normalization",
                    document.compatibility_reason
                );
            }
            write_json(&output, &document)?;
            println!("comparison: {}", output.display());
        }
        AnalysisCommand::Export {
            artifact,
            output,
            maximum_mebibytes,
        } => {
            let limit = maximum_mebibytes
                .checked_mul(1024 * 1024)
                .context("export size limit overflow")?;
            export_static(
                &read_artifact(&artifact)?,
                &output,
                ExportLimits {
                    maximum_artifact_bytes: limit,
                },
            )?;
            println!("static analysis: {}", output.join("index.html").display());
        }
    }
    Ok(())
}

fn read_artifact(path: &Path) -> Result<RunArtifact> {
    let bytes = fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    let artifact: RunArtifact = serde_json::from_slice(&bytes)
        .with_context(|| format!("invalid artifact {}", path.display()))?;
    artifact.validate()?;
    Ok(artifact)
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    fs::write(path, bytes).with_context(|| format!("cannot write {}", path.display()))
}
