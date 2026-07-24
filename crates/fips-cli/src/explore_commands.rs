use anyhow::{Context, Result};
use clap::Subcommand;
use fips_campaign::{
    TINY_COUNTEREXAMPLE_VERSION, TinyCounterexample, TinyExplorerConfig, TinyStateExplorer,
};
use fips_engine::IndividualEngine;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Subcommand)]
pub enum ExploreCommand {
    /// Exhaustively enumerate every ordering of supported authored actions.
    Tiny {
        /// Concrete individual-node Campaign with no unresolved axes.
        campaign: PathBuf,
        /// Hard node bound; the campaign is rejected above it.
        #[arg(long, default_value_t = 8)]
        maximum_nodes: u64,
        /// Hard action bound; factorial growth is rejected above it.
        #[arg(long, default_value_t = 7)]
        maximum_actions: usize,
        /// Virtual-time spacing between actions in each enumerated order.
        #[arg(long, default_value_t = 1_000_000)]
        step_ns: u64,
        /// Evidence directory for report.json and counterexamples.
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Replay one tiny-state counterexample and verify its failure.
    Replay {
        /// Counterexample JSON emitted by `explore tiny`.
        counterexample: PathBuf,
        /// Optional artifact output when the violation is a failed assertion.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

pub fn execute(command: ExploreCommand) -> Result<()> {
    match command {
        ExploreCommand::Tiny {
            campaign,
            maximum_nodes,
            maximum_actions,
            step_ns,
            output,
        } => {
            let plan = fips_model::normalize_path(&campaign)
                .with_context(|| format!("normalization failed for {}", campaign.display()))?;
            let report = TinyStateExplorer.explore(
                &plan,
                TinyExplorerConfig {
                    maximum_nodes,
                    maximum_actions,
                    step_ns,
                },
            )?;
            fs::create_dir_all(output.join("counterexamples"))?;
            write_json(&output.join("report.json"), &report)?;
            for counterexample in &report.counterexamples {
                write_json(
                    &output
                        .join("counterexamples")
                        .join(format!("{}.json", counterexample.id)),
                    counterexample,
                )?;
            }
            println!(
                "explored {}/{} action orders: {} terminal states, {} counterexamples",
                report.explored_permutations,
                report.expected_permutations,
                report.terminal_states.len(),
                report.counterexamples.len()
            );
            println!("evidence: {}", output.display());
        }
        ExploreCommand::Replay {
            counterexample,
            output,
        } => {
            let expected: TinyCounterexample = serde_json::from_slice(
                &fs::read(&counterexample)
                    .with_context(|| format!("cannot read {}", counterexample.display()))?,
            )
            .with_context(|| format!("invalid counterexample {}", counterexample.display()))?;
            anyhow::ensure!(
                expected.api_version == TINY_COUNTEREXAMPLE_VERSION,
                "unsupported tiny counterexample version {}",
                expected.api_version
            );
            match IndividualEngine.run_plan(&expected.normalized_plan) {
                Ok(run) => {
                    let actual = run
                        .artifact
                        .assertion_results
                        .iter()
                        .find(|assertion| assertion.outcome != "pass")
                        .map(|assertion| {
                            format!("assertion {}: {}", assertion.id, assertion.outcome)
                        })
                        .context("counterexample replay unexpectedly passed every assertion")?;
                    anyhow::ensure!(actual == expected.failure, "counterexample failure drift");
                    if let Some(path) = output {
                        write_bytes(&path, &run.artifact.to_canonical_json()?)?;
                    }
                }
                Err(error) => {
                    anyhow::ensure!(
                        error.to_string() == expected.failure,
                        "counterexample runtime failure drift"
                    );
                }
            }
            println!("reproduced {}: {}", expected.id, expected.failure);
        }
    }
    Ok(())
}

fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    write_bytes(path, &bytes)
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes).with_context(|| format!("cannot write {}", path.display()))
}
