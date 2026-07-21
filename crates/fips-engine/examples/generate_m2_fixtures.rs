use fips_engine::IndividualEngine;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let plan =
        fips_model::normalize_path(&repository.join("examples/m2/root-ratchet-recovery.yaml"))?;
    let run = IndividualEngine.run_plan(&plan)?;
    let recovery = run
        .recovery_report
        .as_ref()
        .ok_or("M2 campaign did not produce a recovery report")?;
    let output = repository.join("fixtures/m2");
    fs::create_dir_all(&output)?;
    fs::write(
        output.join("root-ratchet-recovery-artifact.json"),
        run.artifact.to_canonical_json()?,
    )?;
    fs::write(
        output.join("root-ratchet-recovery-reproduction.json"),
        run.reproduction.to_canonical_json()?,
    )?;
    write_pretty(&output.join("root-ratchet-recovery-report.json"), recovery)?;
    Ok(())
}

fn write_pretty(path: &Path, value: &impl serde::Serialize) -> Result<(), Box<dyn Error>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    fs::write(path, bytes)?;
    Ok(())
}
