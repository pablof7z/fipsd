use fips_campaign::{
    CampaignPlanner, DaemonConfirmation, HierarchicalShrinker, MetricConstraint,
    ObjectiveDirection, ObjectiveSpec, PlannerRequest, PlanningMode, SearchEngine, SearchRequest,
    load_corpus, replay_corpus,
};
use fips_engine::IndividualEngine;
use serde::Serialize;
use std::{fs, path::Path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = fips_model::normalize_path(&root.join("examples/m3/root-ratchet-search.yaml"))?;
    let manifest = CampaignPlanner::default().plan(
        &source,
        PlannerRequest {
            mode: PlanningMode::Covering,
            strength: 2,
            seed: source.seed,
            ..PlannerRequest::default()
        },
    )?;
    write_json(&root.join("fixtures/m3/covering-plan.json"), &manifest)?;

    let search = SearchEngine.search(
        &manifest,
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
            maximum_evaluations: manifest.cases.len(),
            maximum_attacker_operations: Some(1_000_000),
        },
        None,
    )?;
    write_json(&root.join("fixtures/m3/search-result.json"), &search)?;

    let best = &search.checkpoint.evaluated[&search.best_case_ids[0]];
    let initial = best.reproduction.as_ref().unwrap();
    let threshold = best.metrics["amplification-ppm"] * 9 / 10;
    let plan = serde_json::from_value(initial.normalized_plan.clone())?;
    let shrink = HierarchicalShrinker { worker_count: 4 }.shrink(
        initial.bundle_id.clone(),
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
    write_json(&root.join("fixtures/m3/shrink-result.json"), &shrink)?;
    refresh_corpus(&root, &shrink)?;
    Ok(())
}

fn refresh_corpus(
    root: &Path,
    shrink: &fips_campaign::ShrinkResult,
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = root.join("fixtures/corpus/m3-root-ratchet");
    let mut metadata: fips_campaign::CorpusMetadata =
        serde_json::from_slice(&fs::read(directory.join("metadata.json"))?)?;
    let reproduction = shrink.reproduction.as_ref().unwrap();
    metadata.minimized_from = shrink.source_case_id.clone();
    metadata.expected_assertions = reproduction.expected_assertions.clone();
    metadata.fidelity = reproduction.fidelity.clone();
    metadata.daemon_confirmation = DaemonConfirmation::ModelOnly;
    write_json(&directory.join("metadata.json"), &metadata)?;
    fs::write(
        directory.join("reproduction.json"),
        reproduction.to_canonical_json()?,
    )?;
    let corpus = load_corpus(&root.join("fixtures/corpus"))?;
    assert_eq!(corpus.len(), 1);
    let report = replay_corpus(&root.join("fixtures/corpus"))?;
    write_json(&root.join("fixtures/m3/corpus-report.json"), &report)?;
    Ok(())
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    fs::write(path, bytes)?;
    Ok(())
}
