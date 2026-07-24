use anyhow::{Context, Result};
use clap::{Subcommand, ValueEnum};
use fips_artifact::ReproductionBundle;
use fips_campaign::{
    CampaignCheckpoint, CampaignPlanner, CampaignRunner, CancellationToken, CorpusReport,
    DaemonConfirmation, ExecutionBudgets, GeneratorConstraints, HierarchicalShrinker,
    MetricConstraint, ObjectiveDirection, ObjectiveSpec, PlanManifest, PlannerRequest,
    PlanningMode, SearchEngine, SearchRequest, ShrinkResult, corpus_entry, generate_input, promote,
    replay_corpus,
};
use fips_engine::IndividualEngine;
use fips_model::NormalizedPlan;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum PlanModeArg {
    Cartesian,
    Covering,
    MonteCarlo,
}

impl From<PlanModeArg> for PlanningMode {
    fn from(value: PlanModeArg) -> Self {
        match value {
            PlanModeArg::Cartesian => Self::Cartesian,
            PlanModeArg::Covering => Self::Covering,
            PlanModeArg::MonteCarlo => Self::MonteCarlo,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum CampaignCommand {
    /// Compile a normalized Cartesian, covering, or Monte Carlo case plan.
    Plan {
        campaign: PathBuf,
        #[arg(long, value_enum, default_value = "cartesian")]
        mode: PlanModeArg,
        #[arg(long, default_value_t = 2)]
        strength: usize,
        #[arg(long, default_value_t = 100)]
        runs: usize,
        #[arg(long)]
        seed: Option<u64>,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Generate a constrained property-based topology and event sequence.
    Generate {
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        nodes: u64,
        #[arg(long, default_value_t = true)]
        connected: bool,
        #[arg(long, default_value_t = 32)]
        events: usize,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Execute a plan with deterministic workers, checkpointing, and budgets.
    Execute {
        manifest: PathBuf,
        #[arg(long)]
        checkpoint: Option<PathBuf>,
        #[arg(long, default_value_t = 1)]
        workers: usize,
        #[arg(long, default_value_t = usize::MAX)]
        maximum_cases: usize,
        #[arg(long, default_value_t = u64::MAX)]
        memory_bytes: u64,
        #[arg(long, default_value_t = u64::MAX)]
        disk_bytes: u64,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Search planned cases for high amplification and starvation.
    Search {
        manifest: PathBuf,
        #[arg(long)]
        checkpoint: Option<PathBuf>,
        #[arg(long, default_value_t = usize::MAX)]
        maximum_evaluations: usize,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Hierarchically shrink a replayable case while preserving a metric threshold.
    Shrink {
        reproduction: PathBuf,
        #[arg(long, default_value = "amplification-ppm")]
        metric: String,
        #[arg(long)]
        minimum: u64,
        #[arg(long, default_value_t = 1)]
        workers: usize,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Promote a minimized bundle into the reviewed regression corpus.
    Promote {
        shrink: PathBuf,
        corpus: PathBuf,
        #[arg(long)]
        first_seen: String,
        #[arg(long, default_value = ">=0.1.0,<0.2.0")]
        semantic_version_range: String,
        #[arg(long)]
        daemon_confirmed: bool,
        #[arg(long)]
        update_expectations: bool,
    },
    /// Replay every active corpus entry at its declared fidelity.
    ReplayCorpus {
        corpus: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

pub fn execute(command: CampaignCommand) -> Result<()> {
    match command {
        CampaignCommand::Plan {
            campaign,
            mode,
            strength,
            runs,
            seed,
            output,
        } => {
            let plan = fips_model::normalize_path(&campaign)?;
            let manifest = CampaignPlanner::default().plan(
                &plan,
                PlannerRequest {
                    mode: mode.into(),
                    strength,
                    run_count: runs,
                    seed: seed.unwrap_or(plan.seed),
                    stratified: true,
                    confidence_target: None,
                    stopping_criterion: None,
                },
            )?;
            write_json(&output, &manifest)?;
            println!(
                "planned {} cases at {}",
                manifest.cases.len(),
                output.display()
            );
        }
        CampaignCommand::Generate {
            seed,
            nodes,
            connected,
            events,
            output,
        } => {
            let input = generate_input(
                seed,
                GeneratorConstraints {
                    nodes,
                    connectivity: if connected {
                        fips_campaign::ConnectivityClass::Connected
                    } else {
                        fips_campaign::ConnectivityClass::Disconnected
                    },
                    maximum_degree: 4,
                    event_count: events,
                    minimum_interval_ns: 1_000_000,
                },
            )?;
            write_json(&output, &input)?;
        }
        CampaignCommand::Execute {
            manifest,
            checkpoint,
            workers,
            maximum_cases,
            memory_bytes,
            disk_bytes,
            output,
        } => execute_plan(
            &manifest,
            checkpoint.as_deref(),
            &output,
            ExecutionBudgets {
                worker_count: workers,
                maximum_cases,
                maximum_memory_bytes: memory_bytes,
                maximum_disk_bytes: disk_bytes,
            },
        )?,
        CampaignCommand::Search {
            manifest,
            checkpoint,
            maximum_evaluations,
            output,
        } => search(
            &manifest,
            checkpoint.as_deref(),
            maximum_evaluations,
            &output,
        )?,
        CampaignCommand::Shrink {
            reproduction,
            metric,
            minimum,
            workers,
            output,
        } => shrink(&reproduction, &metric, minimum, workers, &output)?,
        CampaignCommand::Promote {
            shrink,
            corpus,
            first_seen,
            semantic_version_range,
            daemon_confirmed,
            update_expectations,
        } => {
            let shrink: ShrinkResult = read_json(&shrink)?;
            let entry = corpus_entry(
                &shrink,
                first_seen,
                semantic_version_range,
                if daemon_confirmed {
                    DaemonConfirmation::DaemonConfirmed
                } else {
                    DaemonConfirmation::ModelOnly
                },
            )?;
            let path = promote(&corpus, &entry, update_expectations)?;
            println!("promoted {}", path.display());
        }
        CampaignCommand::ReplayCorpus { corpus, output } => {
            let report: CorpusReport = replay_corpus(&corpus)?;
            if let Some(output) = output {
                write_json(&output, &report)?;
            } else {
                print!("{}", String::from_utf8(pretty(&report)?)?);
            }
            if report.failed > 0 {
                anyhow::bail!("{} corpus entries failed", report.failed);
            }
        }
    }
    Ok(())
}

fn execute_plan(
    manifest_path: &Path,
    checkpoint_path: Option<&Path>,
    output: &Path,
    budgets: ExecutionBudgets,
) -> Result<()> {
    let manifest: PlanManifest = read_json(manifest_path)?;
    let prior = checkpoint_path
        .filter(|path| path.exists())
        .map(CampaignCheckpoint::load)
        .transpose()?;
    let report = CampaignRunner.run(
        &manifest.campaign_sha256,
        &manifest.cases,
        budgets,
        prior,
        CancellationToken::default(),
    )?;
    write_json(output, &report)?;
    if let Some(path) = checkpoint_path {
        report.checkpoint.save(path)?;
    }
    Ok(())
}

fn search(
    manifest_path: &Path,
    checkpoint_path: Option<&Path>,
    maximum_evaluations: usize,
    output: &Path,
) -> Result<()> {
    let manifest: PlanManifest = read_json(manifest_path)?;
    let prior = checkpoint_path
        .filter(|path| path.exists())
        .map(read_json)
        .transpose()?;
    let request = SearchRequest {
        objectives: vec![
            ObjectiveSpec {
                metric: "amplification-ppm".to_owned(),
                direction: ObjectiveDirection::Maximize,
            },
            ObjectiveSpec {
                metric: "goodput-stall-ns".to_owned(),
                direction: ObjectiveDirection::Maximize,
            },
        ],
        constraints: vec![MetricConstraint {
            metric: "starved-flows".to_owned(),
            maximum: None,
            minimum: Some(0),
        }],
        maximum_evaluations,
        maximum_attacker_operations: Some(1_000_000),
    };
    let result = SearchEngine.search(&manifest, request, prior)?;
    write_json(output, &result)?;
    if let Some(path) = checkpoint_path {
        write_json(path, &result.checkpoint)?;
    }
    Ok(())
}

fn shrink(
    bundle_path: &Path,
    metric: &str,
    minimum: u64,
    workers: usize,
    output: &Path,
) -> Result<()> {
    let bundle: ReproductionBundle = read_json(bundle_path)?;
    let plan: NormalizedPlan = serde_json::from_value(bundle.normalized_plan.clone())?;
    let metric_name = metric.to_owned();
    let predicate_name: &'static str = Box::leak(format!("{metric}>={minimum}").into_boxed_str());
    let result = HierarchicalShrinker {
        worker_count: workers,
    }
    .shrink(
        bundle.bundle_id,
        plan,
        (predicate_name, move |candidate: &NormalizedPlan| {
            IndividualEngine.run_plan(candidate).is_ok_and(|run| {
                fips_campaign::individual_metric_value(&run, &metric_name)
                    .is_some_and(|value| value >= minimum)
            })
        }),
    )?;
    write_json(output, &result)
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    serde_json::from_slice(
        &fs::read(path).with_context(|| format!("cannot read {}", path.display()))?,
    )
    .with_context(|| format!("invalid JSON {}", path.display()))
}

fn pretty(value: &impl serde::Serialize) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, pretty(value)?)?;
    Ok(())
}
