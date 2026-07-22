use anyhow::Result;
use clap::Subcommand;
use fips_oracle::{
    ChaosProcessBackend, FuzzOutcome, ImportResult, adapt_fuzz_result, comparable_from_artifact,
    compile_to_chaos, default_oracle_suites, import_chaos_yaml, normalize_telemetry, run_oracle,
    to_yaml,
};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Subcommand)]
pub enum OracleCommand {
    /// Import pinned FIPS chaos YAML with field-level compatibility diagnostics.
    Import {
        chaos: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Compile a representable Campaign to deterministic pinned chaos YAML.
    Compile {
        campaign: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Compile, run, ingest, compare, and classify one chaos case on real daemons.
    RunChaos {
        chaos: PathBuf,
        #[arg(long)]
        fips_checkout: PathBuf,
        #[arg(long, default_value_t = 3)]
        repeats: usize,
        #[arg(long, default_value_t = 10)]
        duration_seconds: u64,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Normalize raw versioned daemon telemetry JSON.
    NormalizeTelemetry {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Emit smoke, nightly, and historical suite manifests.
    Suites {
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Adapt one invalid-wire backend result without entering semantic execution.
    FuzzResult {
        #[arg(long)]
        backend: String,
        #[arg(long)]
        outcome: String,
        #[arg(long)]
        input_hex: String,
        #[arg(long, default_value = "cli-corpus")]
        corpus_sha256: String,
        #[arg(long, default_value_t = 0)]
        coverage_edges: u64,
        #[arg(short, long)]
        output: PathBuf,
    },
}

pub fn execute(command: OracleCommand) -> Result<()> {
    match command {
        OracleCommand::Import { chaos, output } => {
            let bytes = fs::read(&chaos)?;
            write_json(
                &output,
                &import_chaos_yaml(&chaos.display().to_string(), &bytes)?,
            )?;
        }
        OracleCommand::Compile { campaign, output } => {
            let bytes = fs::read(&campaign)?;
            let plan = match serde_json::from_slice::<ImportResult>(&bytes) {
                Ok(imported) => imported.plan,
                Err(_) => fips_model::normalize_path(&campaign)?,
            };
            let bundle = compile_to_chaos(&plan)?;
            fs::write(&output, to_yaml(&bundle)?)?;
            write_json(&output.with_extension("manifest.json"), &bundle)?;
        }
        OracleCommand::RunChaos {
            chaos,
            fips_checkout,
            repeats,
            duration_seconds,
            output,
        } => {
            let imported = import_chaos_yaml(&chaos.display().to_string(), &fs::read(&chaos)?)?;
            let model = fips_engine::IndividualEngine
                .run_plan(&imported.plan)
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            let backend = ChaosProcessBackend {
                fips_checkout,
                output_root: output.join("daemon-runs"),
                duration_seconds,
            };
            let report = run_oracle(
                &imported.plan,
                &comparable_from_artifact(&model.artifact),
                &backend,
                repeats,
                800_000,
            )?;
            fs::create_dir_all(&output)?;
            write_json(&output.join("import.json"), &imported)?;
            write_json(&output.join("oracle-report.json"), &report)?;
        }
        OracleCommand::NormalizeTelemetry { input, output } => {
            let input = serde_json::from_slice(&fs::read(input)?)?;
            write_json(&output, &normalize_telemetry(input)?)?;
        }
        OracleCommand::Suites { output } => write_json(&output, &default_oracle_suites())?,
        OracleCommand::FuzzResult {
            backend,
            outcome,
            input_hex,
            corpus_sha256,
            coverage_edges,
            output,
        } => {
            let outcome = match outcome.as_str() {
                "pass" => FuzzOutcome::Pass,
                "crash" => FuzzOutcome::Crash,
                "hang" => FuzzOutcome::Hang,
                "allocation-limit" => FuzzOutcome::AllocationLimit,
                "codec-differential" => FuzzOutcome::CodecDifferential,
                other => anyhow::bail!("unsupported fuzz outcome {other}"),
            };
            let input = hex::decode(input_hex)?;
            write_json(
                &output,
                &adapt_fuzz_result(&backend, outcome, &input, &corpus_sha256, coverage_edges),
            )?;
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
