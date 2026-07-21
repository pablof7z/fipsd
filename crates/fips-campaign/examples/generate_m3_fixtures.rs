use fips_campaign::{
    CampaignPlanner, DaemonConfirmation, HierarchicalShrinker, MetricConstraint,
    ObjectiveDirection, ObjectiveSpec, PlannerRequest, PlanningMode, SearchEngine, SearchRequest,
    corpus_entry, generate_input, promote, replay_corpus,
};
use fips_engine::IndividualEngine;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source =
        fips_model::normalize_path(&repository.join("examples/m3/root-ratchet-search.yaml"))?;
    let manifest = CampaignPlanner::default().plan(
        &source,
        PlannerRequest {
            mode: PlanningMode::Covering,
            strength: 2,
            seed: source.seed,
            ..PlannerRequest::default()
        },
    )?;
    let request = search_request(manifest.cases.len());
    let search = SearchEngine.search(&manifest, request, None)?;
    let best = &search.checkpoint.evaluated[&search.best_case_ids[0]];
    let threshold = best.metrics["amplification-ppm"] * 9 / 10;
    let reproduction = best
        .reproduction
        .as_ref()
        .ok_or("best case has no reproduction")?;
    let plan = serde_json::from_value(reproduction.normalized_plan.clone())?;
    let shrink = HierarchicalShrinker { worker_count: 4 }.shrink(
        reproduction.bundle_id.clone(),
        plan,
        (
            "amplification-at-least-90-percent",
            move |candidate: &fips_model::NormalizedPlan| {
                IndividualEngine.run_plan(candidate).is_ok_and(|run| {
                    run.recovery_report
                        .is_some_and(|report| report.costs.amplification_ppm >= threshold)
                })
            },
        ),
    )?;
    let generated = generate_input(
        27,
        fips_campaign::GeneratorConstraints {
            nodes: 1_000_000,
            connectivity: fips_campaign::ConnectivityClass::Connected,
            maximum_degree: 4,
            event_count: 32,
            minimum_interval_ns: 1_000_000,
        },
    )?;

    let output = repository.join("fixtures/m3");
    fs::create_dir_all(&output)?;
    write_pretty(&output.join("covering-plan.json"), &manifest)?;
    write_pretty(&output.join("search-result.json"), &search)?;
    write_pretty(&output.join("shrink-result.json"), &shrink)?;
    write_pretty(
        &output.join("million-node-generated-input.json"),
        &generated,
    )?;

    let mut entry = corpus_entry(
        &shrink,
        "M3 root-ratchet campaign search",
        ">=0.1.0,<0.2.0",
        DaemonConfirmation::ModelOnly,
    )?;
    entry.id = "m3-root-ratchet".to_owned();
    promote(&repository.join("fixtures/corpus"), &entry, false)?;
    let corpus_report = replay_corpus(&repository.join("fixtures/corpus"))?;
    write_pretty(&output.join("corpus-report.json"), &corpus_report)?;
    Ok(())
}

fn search_request(maximum_evaluations: usize) -> SearchRequest {
    SearchRequest {
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
    }
}

fn write_pretty(path: &Path, value: &impl serde::Serialize) -> Result<(), Box<dyn Error>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    fs::write(path, bytes)?;
    Ok(())
}
