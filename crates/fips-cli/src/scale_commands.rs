use anyhow::Result;
use clap::Subcommand;
use fips_scale::{
    BASELINE_VARIANT, CohortEngine, HybridEngine, SamplingPolicy, billion_node_demo, calibrate,
    compare_variants,
};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Subcommand)]
pub enum ScaleCommand {
    /// Run one cohort analytical case and emit artifact plus report.
    Run {
        campaign: PathBuf,
        #[arg(long, default_value = BASELINE_VARIANT)]
        variant: String,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Compare baseline, root dampening, and Bloom delta variants.
    Compare {
        campaign: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Calibrate cohort and hybrid projections against matched individual runs.
    Calibrate {
        campaign: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Run the bounded billion-node sensitivity demo and exact anomaly sample.
    BillionDemo {
        campaign: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Instantiate a replayable exact region inside a cohort run.
    Sample {
        campaign: PathBuf,
        #[arg(long, default_value_t = 16)]
        nodes: u64,
        #[arg(short, long)]
        output: PathBuf,
    },
}

pub fn execute(command: ScaleCommand) -> Result<()> {
    match command {
        ScaleCommand::Run {
            campaign,
            variant,
            output,
        } => {
            let plan = fips_model::normalize_path(&campaign)?;
            let run = CohortEngine.run(&plan, &variant)?;
            fs::create_dir_all(&output)?;
            fs::write(
                output.join("artifact.json"),
                run.artifact.to_canonical_json()?,
            )?;
            write_json(&output.join("report.json"), &run.report)?;
        }
        ScaleCommand::Compare { campaign, output } => {
            let plan = fips_model::normalize_path(&campaign)?;
            write_json(&output, &compare_variants(&plan)?)?;
        }
        ScaleCommand::Calibrate { campaign, output } => {
            let plan = fips_model::normalize_path(&campaign)?;
            write_json(&output, &calibrate(&plan)?)?;
        }
        ScaleCommand::BillionDemo { campaign, output } => {
            let plan = fips_model::normalize_path(&campaign)?;
            write_json(&output, &billion_node_demo(&plan)?)?;
        }
        ScaleCommand::Sample {
            campaign,
            nodes,
            output,
        } => {
            let plan = fips_model::normalize_path(&campaign)?;
            let run = HybridEngine.run(
                &plan,
                BASELINE_VARIANT,
                SamplingPolicy::AnomalyDriven,
                nodes,
            )?;
            fs::create_dir_all(&output)?;
            fs::write(
                output.join("artifact.json"),
                run.artifact.to_canonical_json()?,
            )?;
            fs::write(
                output.join("reproduction.json"),
                run.report.exact.reproduction.to_canonical_json()?,
            )?;
            write_json(&output.join("report.json"), &run.report)?;
        }
    }
    Ok(())
}

fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    fs::write(path, bytes)?;
    Ok(())
}
