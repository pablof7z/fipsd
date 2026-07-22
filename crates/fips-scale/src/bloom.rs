use crate::Cohort;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Analytical Bloom occupancy and false-positive rate for one cohort.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CohortBloom {
    pub cohort_id: String,
    pub population: u64,
    pub bits: u64,
    pub hashes: u32,
    pub insertions_per_member: u64,
    pub occupancy_ppb: u64,
    pub fpr_ppb: u64,
    pub method: String,
    pub uncertainty_ppb: u64,
}

/// Exact bit sample embedded in an analytical run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExactBloomSample {
    pub region_id: String,
    pub bits: u64,
    pub set_indices: Vec<u64>,
    pub sample_population: u64,
    pub source_population: u64,
}

/// Boundary conversion proof between exact and cohort state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BloomBoundary {
    pub cohort_id: String,
    pub incoming_population: u64,
    pub exact_sample_population: u64,
    pub aggregate_population: u64,
    pub causal_input: String,
    pub causal_output: String,
    pub reconciles: bool,
}

pub fn cohort_bloom(cohorts: &[Cohort], bits: u64, hashes: u32) -> Vec<CohortBloom> {
    cohorts
        .iter()
        .map(|cohort| {
            let insertions = cohort.key.depth_end.saturating_add(1);
            let exponent = -(f64::from(hashes) * insertions as f64 / bits as f64);
            let occupancy = 1.0 - exponent.exp();
            let fpr = occupancy.powi(hashes as i32);
            CohortBloom {
                cohort_id: cohort.id.clone(),
                population: cohort.population,
                bits,
                hashes,
                insertions_per_member: insertions,
                occupancy_ppb: (occupancy * 1_000_000_000.0).round() as u64,
                fpr_ppb: (fpr * 1_000_000_000.0).round() as u64,
                method: "bloom-independent-bit-occupancy/v1".to_owned(),
                uncertainty_ppb: (fpr * 25_000_000.0).round() as u64,
            }
        })
        .collect()
}

pub fn exact_bloom_sample(
    region_id: &str,
    source_population: u64,
    sample_population: u64,
    bits: u64,
    hashes: u32,
    seed: u64,
) -> ExactBloomSample {
    let mut set_indices = Vec::new();
    for member in 0..sample_population {
        for lane in 0..hashes {
            let digest = Sha256::digest(
                [
                    seed.to_le_bytes().as_slice(),
                    member.to_le_bytes().as_slice(),
                    &lane.to_le_bytes(),
                ]
                .concat(),
            );
            let index = u64::from_le_bytes(digest[..8].try_into().expect("8 bytes")) % bits;
            set_indices.push(index);
        }
    }
    set_indices.sort_unstable();
    set_indices.dedup();
    ExactBloomSample {
        region_id: region_id.to_owned(),
        bits,
        set_indices,
        sample_population,
        source_population,
    }
}

pub fn translate_boundary(cohort: &Cohort, sample: &ExactBloomSample) -> BloomBoundary {
    let aggregate_population = cohort.population.saturating_sub(sample.sample_population);
    BloomBoundary {
        cohort_id: cohort.id.clone(),
        incoming_population: cohort.population,
        exact_sample_population: sample.sample_population,
        aggregate_population,
        causal_input: format!("cohort:{}", cohort.id),
        causal_output: format!("exact:{}", sample.region_id),
        reconciles: aggregate_population + sample.sample_population == cohort.population,
    }
}
