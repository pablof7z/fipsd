use anyhow::{Context, Result, bail};
use clap::Subcommand;
use fips_atlas::{AtlasReport, qualify};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Subcommand)]
pub enum AtlasCommand {
    /// Qualify all ten normative campaign families and write immutable evidence.
    Build {
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Verify a checked-in qualification atlas and every family contract.
    Verify { atlas: PathBuf },
}

pub fn execute(command: AtlasCommand) -> Result<()> {
    match command {
        AtlasCommand::Build { output } => {
            let report = qualify()?;
            write(&output, &report)?;
            println!(
                "atlas {}: {} qualified families",
                report.atlas_id, report.family_count
            );
        }
        AtlasCommand::Verify { atlas } => {
            let expected = qualify()?;
            let actual: AtlasReport = serde_json::from_slice(
                &fs::read(&atlas).with_context(|| format!("cannot read {}", atlas.display()))?,
            )
            .with_context(|| format!("invalid atlas {}", atlas.display()))?;
            if actual != expected {
                bail!("atlas differs from deterministic qualification output");
            }
            if actual.family_count != 10 || !actual.all_contracts_complete {
                bail!("atlas is incomplete");
            }
            println!("verified {} campaign families", actual.family_count);
        }
    }
    Ok(())
}

fn write(path: &Path, report: &AtlasReport) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(report)?;
    bytes.push(b'\n');
    fs::write(path, bytes).with_context(|| format!("cannot write {}", path.display()))
}
