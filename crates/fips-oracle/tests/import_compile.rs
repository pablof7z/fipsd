mod common;

use fips_oracle::{
    MappingDisposition, PINNED_FIPS_COMMIT, compile_to_chaos, import_chaos_yaml, to_yaml,
};

#[test]
fn six_pinned_chaos_families_import_validate_and_report_loss_field_by_field() {
    for name in [
        "smoke.yaml",
        "cost.yaml",
        "mixed-transport.yaml",
        "congestion.yaml",
        "churn.yaml",
        "bloom-storm.yaml",
    ] {
        let bytes = common::fixture(name);
        let imported =
            import_chaos_yaml(&format!("testing/chaos/scenarios/{name}"), &bytes).unwrap();
        assert_eq!(imported.source.fips_commit, PINNED_FIPS_COMMIT);
        assert!(imported.source.source_path.ends_with(name));
        assert_eq!(
            imported.plan.api_version,
            fips_model::NORMALIZED_PLAN_VERSION
        );
        assert!(!imported.diagnostics.is_empty());
        assert!(
            imported
                .diagnostics
                .iter()
                .all(|item| !item.source_path.is_empty())
        );
        if matches!(name, "congestion.yaml" | "churn.yaml" | "bloom-storm.yaml") {
            assert!(
                imported
                    .diagnostics
                    .iter()
                    .any(|item| item.disposition == MappingDisposition::PreservedMetadata)
            );
        }
        let harness = compile_to_chaos(&imported.plan).unwrap();
        assert_eq!(harness.fips_commit, PINNED_FIPS_COMMIT);
        assert_eq!(
            harness.deterministic_identity_ids.len(),
            imported
                .plan
                .campaign
                .pointer("/scale/nodes")
                .unwrap()
                .as_u64()
                .unwrap() as usize
        );
        let yaml = to_yaml(&harness).unwrap();
        let parsed: serde_json::Value = serde_yaml::from_slice(&yaml).unwrap();
        assert_eq!(
            parsed
                .pointer("/scenario/seed")
                .and_then(serde_json::Value::as_u64),
            Some(imported.plan.seed)
        );
    }
}

#[test]
fn round_trip_subset_preserves_normalized_seed_scale_topology_and_fault_midpoints() {
    let imported = import_chaos_yaml("smoke-10.yaml", &common::fixture("smoke.yaml")).unwrap();
    let harness = compile_to_chaos(&imported.plan).unwrap();
    let round_trip = import_chaos_yaml("compiled.yaml", &to_yaml(&harness).unwrap()).unwrap();
    assert_eq!(round_trip.plan.seed, imported.plan.seed);
    assert_eq!(
        round_trip.plan.campaign.pointer("/scale/nodes"),
        imported.plan.campaign.pointer("/scale/nodes")
    );
    assert_eq!(
        round_trip
            .plan
            .campaign
            .pointer("/traffic/parameters/flow_count"),
        imported
            .plan
            .campaign
            .pointer("/traffic/parameters/flow_count")
    );
    assert_eq!(
        round_trip.plan.campaign.pointer("/links/loss_ppm"),
        imported.plan.campaign.pointer("/links/loss_ppm")
    );
}

#[test]
fn compiler_maps_uniform_and_mixed_transports_without_silent_fallback() {
    let smoke = import_chaos_yaml("smoke.yaml", &common::fixture("smoke.yaml")).unwrap();
    let smoke_bundle = compile_to_chaos(&smoke.plan).unwrap();
    assert_eq!(
        smoke_bundle
            .scenario
            .pointer("/topology/default_transport")
            .and_then(serde_json::Value::as_str),
        Some("udp")
    );
    let mut mixed = smoke.plan.clone();
    *mixed
        .campaign
        .pointer_mut("/transports/assignment")
        .unwrap() = serde_json::json!("heterogeneous");
    let mixed_bundle = compile_to_chaos(&mixed).unwrap();
    assert!(
        mixed_bundle
            .scenario
            .pointer("/topology/transport_mix")
            .is_some()
    );
    assert!(
        mixed_bundle
            .diagnostics
            .iter()
            .any(|item| item.source_path == "/transports/assignment"
                && item.disposition == MappingDisposition::Approximated)
    );
}

#[test]
fn unsupported_dynamic_identity_and_cohort_features_fail_with_reductions() {
    let root = common::repository();
    let m3 =
        fips_model::normalize_path(&root.join("examples/m2/root-ratchet-recovery.yaml")).unwrap();
    let error = compile_to_chaos(&m3).unwrap_err().to_string();
    assert!(error.contains("identities/arrivals") && error.contains("cannot add identities"));
    let m4 =
        fips_model::normalize_path(&root.join("examples/m4/billion-root-ratchet.yaml")).unwrap();
    let error = compile_to_chaos(&m4).unwrap_err().to_string();
    assert!(error.contains("reduce to at most 256") || error.contains("cohort/hybrid"));

    let mut unsupported = import_chaos_yaml("smoke.yaml", &common::fixture("smoke.yaml"))
        .unwrap()
        .plan;
    *unsupported
        .campaign
        .pointer_mut("/protocol/variant")
        .unwrap() = serde_json::json!("uncompiled-variant");
    assert!(
        compile_to_chaos(&unsupported)
            .unwrap_err()
            .to_string()
            .contains("variant")
    );
    *unsupported
        .campaign
        .pointer_mut("/protocol/variant")
        .unwrap() = serde_json::json!("fips-80c956a-baseline");
    *unsupported
        .campaign
        .pointer_mut("/adversaries/mode")
        .unwrap() = serde_json::json!("authenticated-byzantine");
    assert!(
        compile_to_chaos(&unsupported)
            .unwrap_err()
            .to_string()
            .contains("adversaries")
    );
}
