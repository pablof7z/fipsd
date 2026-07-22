use fips_engine::{GraphStore, IndividualEngine, TopologyKind};
use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let plan = fips_model::normalize_path(&repository.join("examples/m1/root-ratchet-12.yaml"))?;
    let run = IndividualEngine.run_plan(&plan)?;
    let output = repository.join("fixtures/m1");
    fs::create_dir_all(&output)?;
    fs::write(
        output.join("root-ratchet-12-artifact.json"),
        run.artifact.to_canonical_json()?,
    )?;
    fs::write(
        output.join("root-ratchet-12-reproduction.json"),
        run.reproduction.to_canonical_json()?,
    )?;
    let mut report = serde_json::to_vec_pretty(&run.report)?;
    report.push(b'\n');
    fs::write(output.join("root-ratchet-12-report.json"), report)?;
    let topology_hashes = [
        ("chain", TopologyKind::Chain, 2),
        ("balanced-tree", TopologyKind::BalancedTree, 2),
        ("random-regular", TopologyKind::RandomRegular, 4),
        ("scale-free", TopologyKind::ScaleFree, 4),
    ]
    .into_iter()
    .map(|(name, kind, degree)| {
        let graph = GraphStore::generate(kind, 20, degree, 424242, &[])?;
        Ok((name, graph.graph_sha256()))
    })
    .collect::<Result<BTreeMap<_, _>, Box<dyn Error>>>()?;
    let mut hashes = serde_json::to_vec_pretty(&topology_hashes)?;
    hashes.push(b'\n');
    fs::write(output.join("topology-hashes.json"), hashes)?;
    Ok(())
}
