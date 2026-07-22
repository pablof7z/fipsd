use crate::AuditError;
use fips_artifact::RunArtifact;
use fips_atlas::qualify;
use fips_query::{EventQuery, analyze, query_events};
use fips_scale::{BASELINE_VARIANT, CohortEngine};
use serde::{Deserialize, Serialize};
use std::hint::black_box;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkCase {
    pub name: String,
    pub iterations: usize,
    pub p50_ns: u128,
    pub p95_ns: u128,
    pub input_bytes: u64,
    pub output_bytes: u64,
    pub memory_evidence: String,
    pub fidelity: String,
    pub represented_nodes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub api_version: String,
    pub os: String,
    pub architecture: String,
    pub cases: Vec<BenchmarkCase>,
    pub claim_boundary: String,
    pub resource_failure_policy: String,
}

pub fn benchmark(iterations: usize) -> Result<BenchmarkReport, AuditError> {
    let iterations = iterations.clamp(3, 1_000);
    let bytes = include_bytes!("../../../fixtures/m2/root-ratchet-recovery-artifact.json");
    let artifact: RunArtifact = serde_json::from_slice(bytes)?;
    let analysis = analyze(&artifact)?;
    let analysis_bytes = serde_json::to_vec(&analysis)?.len() as u64;
    let mut cases = Vec::new();
    cases.push(measure(
        "artifact-analysis",
        iterations,
        bytes.len() as u64,
        analysis_bytes,
        || black_box(analyze(&artifact).expect("validated fixture")),
    ));
    let billion_source = include_str!("../../../examples/m4/billion-root-ratchet.yaml");
    let million_source = billion_source.replace("nodes: 1000000000", "nodes: 1000000");
    for (name, nodes, source) in [
        ("million-node-cohort", 1_000_000, million_source.as_str()),
        ("billion-node-cohort", 1_000_000_000, billion_source),
    ] {
        let plan = fips_model::normalize_str(source)?;
        let sample = CohortEngine
            .run(&plan, BASELINE_VARIANT)
            .expect("qualified cohort plan");
        let output_bytes = sample.artifact.to_canonical_json()?.len() as u64;
        let mut case = measure(name, iterations, source.len() as u64, output_bytes, || {
            black_box(
                CohortEngine
                    .run(&plan, BASELINE_VARIANT)
                    .expect("qualified cohort plan"),
            )
        });
        case.fidelity = "measured cohort execution with declared deterministic bounds".to_owned();
        case.represented_nodes = Some(nodes);
        cases.push(case);
    }
    cases.push(measure(
        "bounded-event-query",
        iterations,
        bytes.len() as u64,
        100,
        || {
            black_box(
                query_events(
                    &artifact,
                    &EventQuery {
                        maximum_results: 100,
                        ..EventQuery::default()
                    },
                )
                .expect("validated fixture"),
            )
        },
    ));
    let atlas_bytes = serde_json::to_vec(&qualify()?)?.len() as u64;
    cases.push(measure(
        "ten-family-atlas",
        iterations,
        10,
        atlas_bytes,
        || black_box(qualify().expect("embedded campaigns")),
    ));
    Ok(BenchmarkReport {
        api_version: "experiments.fips.network/benchmark/v1alpha1".to_owned(),
        os: std::env::consts::OS.to_owned(),
        architecture: std::env::consts::ARCH.to_owned(),
        cases,
        claim_boundary: "timings apply only to this recorded host/build; input/output bytes are measured, peak RSS is not observed by the portable harness".to_owned(),
        resource_failure_policy: "bounded allocation is checked before execution; failures return typed errors and retain prior immutable evidence".to_owned(),
    })
}

fn measure<T>(
    name: &str,
    iterations: usize,
    input_bytes: u64,
    output_bytes: u64,
    mut operation: impl FnMut() -> T,
) -> BenchmarkCase {
    operation();
    let mut samples = (0..iterations)
        .map(|_| {
            let start = Instant::now();
            operation();
            start.elapsed().as_nanos()
        })
        .collect::<Vec<_>>();
    samples.sort_unstable();
    BenchmarkCase {
        name: name.to_owned(),
        iterations,
        p50_ns: percentile(&samples, 50),
        p95_ns: percentile(&samples, 95),
        input_bytes,
        output_bytes,
        memory_evidence: format!(
            "portable lower bound: {} input plus {} output bytes; peak RSS unobserved",
            input_bytes, output_bytes
        ),
        fidelity: "wall-clock measured on recorded host; no cross-host extrapolation".to_owned(),
        represented_nodes: None,
    }
}

fn percentile(samples: &[u128], percentile: usize) -> u128 {
    samples[(samples.len() - 1) * percentile / 100]
}
