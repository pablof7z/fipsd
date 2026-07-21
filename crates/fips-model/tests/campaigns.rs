use fips_model::{ModelError, normalize_path, validate_path};
use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

#[test]
fn every_flagship_campaign_validates() {
    let root = workspace_root();
    validate_path(&root.join("examples/root-ratchet.yaml")).unwrap();
    let mut campaigns: Vec<_> = fs::read_dir(root.join("examples/campaigns"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "yaml")
        })
        .collect();
    campaigns.sort();
    assert_eq!(campaigns.len(), 9);
    for campaign in campaigns {
        validate_path(&campaign)
            .unwrap_or_else(|error| panic!("{} did not validate: {error}", campaign.display()));
    }
}

#[test]
fn invalid_fixtures_fail_with_paths() {
    let root = workspace_root();
    let cases = [
        ("unknown-field.yaml", "/"),
        (
            "bad-duration.yaml",
            "/identities/arrivals/schedule/interval",
        ),
        (
            "billion-individual.yaml",
            "/fidelity/billion_node_representation",
        ),
    ];
    for (name, expected_path) in cases {
        let error = validate_path(&root.join("fixtures/campaign/invalid").join(name)).unwrap_err();
        match error {
            ModelError::Validation { path, .. } | ModelError::Unsupported { path, .. } => {
                assert_eq!(path, expected_path, "fixture {name}");
            }
            other => panic!("unexpected failure for {name}: {other}"),
        }
    }
}

#[test]
fn root_ratchet_normalization_matches_golden_bytes() {
    let root = workspace_root();
    let actual = normalize_path(&root.join("examples/root-ratchet.yaml"))
        .unwrap()
        .to_canonical_json()
        .unwrap();
    let expected = fs::read(root.join("fixtures/normalized/root-ratchet.json")).unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn normalized_plan_matches_published_schema() {
    let root = workspace_root();
    let schema: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("schemas/normalized-plan-v1alpha1.schema.json")).unwrap(),
    )
    .unwrap();
    let plan: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("fixtures/normalized/root-ratchet.json")).unwrap(),
    )
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<_> = validator
        .iter_errors(&plan)
        .map(|error| format!("{}: {error}", error.instance_path))
        .collect();
    assert!(errors.is_empty(), "{}", errors.join("; "));
}
