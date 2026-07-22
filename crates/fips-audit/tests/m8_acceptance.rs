use fips_audit::{audit_release, benchmark, build_manifest, verify_manifest};
use std::fs;

#[test]
fn deterministic_and_accounting_audits_pass_without_hiding_unknowns() {
    let audit = audit_release().expect("audit");
    assert!(audit.ready);
    assert!(audit.determinism.passed);
    assert!(audit.accounting.passed);
    assert!(!audit.accounting.unobserved.is_empty());
    assert!(
        audit
            .support_matrix
            .iter()
            .any(|item| item.status == "unsupported")
    );
}

#[test]
fn benchmark_records_measured_host_and_claim_boundary() {
    let report = benchmark(3).expect("benchmark");
    assert_eq!(report.cases.len(), 5);
    assert!(report.cases.iter().all(|item| item.p50_ns > 0));
    assert!(report.claim_boundary.contains("this recorded host"));
    assert!(
        report
            .cases
            .iter()
            .any(|item| item.represented_nodes == Some(1_000_000))
    );
    assert!(
        report
            .cases
            .iter()
            .any(|item| item.represented_nodes == Some(1_000_000_000))
    );
}

#[test]
fn package_manifest_detects_mutation() {
    let root = std::env::temp_dir().join(format!("fips-audit-{}", std::process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("remove old temp");
    }
    fs::create_dir_all(root.join("bin")).expect("mkdir");
    fs::write(root.join("bin/fips-wind-tunnel"), b"binary").expect("write");
    let manifest = build_manifest(&root).expect("manifest");
    verify_manifest(&root, &manifest).expect("verify");
    fs::write(root.join("bin/fips-wind-tunnel"), b"changed").expect("mutate");
    assert!(verify_manifest(&root, &manifest).is_err());
    fs::remove_dir_all(root).expect("cleanup");
}
