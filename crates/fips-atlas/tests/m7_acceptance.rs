use fips_atlas::qualify;
use std::collections::BTreeSet;

#[test]
fn all_ten_flagship_families_have_complete_contracts() {
    let atlas = qualify().expect("atlas");
    assert_eq!(atlas.family_count, 10);
    assert!(atlas.all_contracts_complete);
    let ids = atlas
        .families
        .iter()
        .map(|family| family.contract.id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(ids.len(), 10);
    for family in &atlas.families {
        assert!(!family.contract.boundary_matrix.is_empty());
        assert!(!family.contract.assertions.is_empty());
        assert!(!family.contract.report_recipe.is_empty());
        assert!(!family.contract.fidelity.supported.is_empty());
        assert!(!family.minimized_reproduction.selector_overrides.is_empty());
    }
}

#[test]
fn atlas_is_byte_stable_and_unknown_oracle_support_is_explicit() {
    let first = serde_json::to_vec(&qualify().expect("atlas")).expect("json");
    let second = serde_json::to_vec(&qualify().expect("atlas")).expect("json");
    assert_eq!(first, second);
    let atlas = qualify().expect("atlas");
    assert!(
        atlas
            .families
            .iter()
            .any(|family| { family.minimized_reproduction.oracle.status == "eligible" })
    );
    assert!(
        atlas
            .families
            .iter()
            .any(|family| { family.minimized_reproduction.oracle.status == "unsupported" })
    );
}

#[test]
fn deep_tree_boundary_is_executable_codec_evidence() {
    let atlas = qualify().expect("atlas");
    let family = atlas
        .families
        .iter()
        .find(|family| family.contract.id == "deep-tree-mtu-ttl-cliff")
        .expect("deep-tree family");
    assert_eq!(family.discovered_boundary.at.value, "1288");
    assert_eq!(family.discovered_boundary.at.fidelity, "executable-codec");
}
