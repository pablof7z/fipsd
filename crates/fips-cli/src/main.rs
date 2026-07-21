use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;

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
    }
    Ok(())
}
