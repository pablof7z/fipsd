use super::*;

fn provenance() -> ProvenanceEnvelope {
    ProvenanceEnvelope {
        engine_name: "fixture".to_owned(),
        engine_version: "0.1.0".to_owned(),
        engine_source_revision: "1111111111111111111111111111111111111111".to_owned(),
        schema_versions: BTreeMap::new(),
        seed: 7,
        normalized_plan_sha256: "a".repeat(64),
        fips_commit: Some(M0_FIPS_COMMIT.to_owned()),
        image_digest: None,
        hardware_profile: None,
    }
}

fn exact_fidelity() -> FidelityContract {
    FidelityContract {
        wire: WireFidelity::ExecutableCodec,
        protocol: ProtocolFidelity::SemanticExact,
        compute: ComputeFidelity::OperationCounted,
        scale: ScaleFidelity::Individual,
        bloom: BloomFidelity::ExactBits,
        represented_nodes: 10,
        approximations: Vec::new(),
        sampled_regions: Vec::new(),
    }
}

#[test]
fn fidelity_statement_is_self_contained() {
    let statement = exact_fidelity().plain_language_statement();
    assert!(statement.contains("executable production codec"));
    assert!(statement.contains("10 represented nodes"));
    assert!(statement.contains("No approximation metadata applies"));
}

#[test]
fn calibrated_compute_requires_hardware_profile() {
    let mut fidelity = exact_fidelity();
    fidelity.compute = ComputeFidelity::Calibrated;
    assert!(fidelity.validate(&provenance()).is_err());
}

#[test]
fn unsupported_fidelity_combinations_are_rejected() {
    let mut missing_pin = provenance();
    missing_pin.fips_commit = None;
    assert!(exact_fidelity().validate(&missing_pin).is_err());

    let mut cohort = exact_fidelity();
    cohort.scale = ScaleFidelity::Cohort;
    assert!(cohort.validate(&provenance()).is_err());

    let mut hybrid = exact_fidelity();
    hybrid.scale = ScaleFidelity::Hybrid;
    hybrid.approximations.push(Approximation {
        method: "fixture".to_owned(),
        parameters: BTreeMap::new(),
        validated_range: "fixture-only".to_owned(),
        uncertainty: "unknown".to_owned(),
    });
    assert!(hybrid.validate(&provenance()).is_err());

    let mut billion = exact_fidelity();
    billion.represented_nodes = 1_000_000_000;
    assert!(billion.validate(&provenance()).is_err());

    let mut sampled_bloom = exact_fidelity();
    sampled_bloom.bloom = BloomFidelity::SampledExact;
    assert!(sampled_bloom.validate(&provenance()).is_err());
}

#[test]
fn external_blob_verifies_hash_and_size() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("series.json"), b"[]\n").unwrap();
    let blob = ExternalBlob {
        role: "metric-series".to_owned(),
        path: "series.json".to_owned(),
        sha256: hex_lower(&Sha256::digest(b"[]\n")),
        size_bytes: 3,
        encoding: "identity".to_owned(),
    };
    blob.verify(directory.path()).unwrap();
}

#[test]
fn external_blob_rejects_parent_traversal() {
    let blob = ExternalBlob {
        role: "sample".to_owned(),
        path: "../secret".to_owned(),
        sha256: "0".repeat(64),
        size_bytes: 0,
        encoding: "identity".to_owned(),
    };
    assert!(matches!(
        blob.verify(Path::new(".")),
        Err(ArtifactError::UnsafeBlobPath(_))
    ));
}
