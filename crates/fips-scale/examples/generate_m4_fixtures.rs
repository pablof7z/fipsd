use fips_scale::{
    BASELINE_VARIANT, CohortEngine, HybridEngine, SamplingPolicy, billion_node_demo, calibrate,
    compare_variants,
};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let plan =
        fips_model::normalize_path(&repository.join("examples/m4/billion-root-ratchet.yaml"))?;
    let cohort = CohortEngine.run(&plan, BASELINE_VARIANT)?;
    let comparison = compare_variants(&plan)?;
    let hybrid = HybridEngine.run(&plan, BASELINE_VARIANT, SamplingPolicy::AnomalyDriven, 16)?;
    let calibration = calibrate(&plan)?;
    let demo = billion_node_demo(&plan)?;
    let output = repository.join("fixtures/m4");
    fs::create_dir_all(&output)?;
    fs::write(
        output.join("billion-cohort-artifact.json"),
        cohort.artifact.to_canonical_json()?,
    )?;
    write_pretty(&output.join("billion-cohort-report.json"), &cohort.report)?;
    write_pretty(&output.join("variant-comparison.json"), &comparison)?;
    write_pretty(&output.join("hybrid-anomaly.json"), &hybrid.report)?;
    write_pretty(&output.join("calibration.json"), &calibration)?;
    write_pretty(&output.join("billion-demo.json"), &demo)?;
    Ok(())
}

fn write_pretty(path: &Path, value: &impl serde::Serialize) -> Result<(), Box<dyn Error>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    fs::write(path, bytes)?;
    Ok(())
}
