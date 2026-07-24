use fips_engine::IndividualRun;
use std::collections::BTreeMap;

pub(super) fn collect(run: &IndividualRun) -> BTreeMap<String, u64> {
    let mut metrics = BTreeMap::from([
        ("convergence-time-ns".to_owned(), run.report.quiescence_ns),
        (
            "control-bytes".to_owned(),
            run.report.tree_announce.transmitted_frame_bytes,
        ),
        (
            "parent-transitions".to_owned(),
            run.report.parent_transitions,
        ),
    ]);
    for name in [
        "amplification-ppm",
        "peak-queue-bytes",
        "goodput-stall-ns",
        "starved-flows",
        "cache-invalidations",
    ] {
        if let Some(value) = individual_metric_value(run, name) {
            metrics.insert(name.to_owned(), value);
        }
    }
    metrics
}

/// Read a search/shrink metric identically across coupled and graph-native recovery.
pub fn individual_metric_value(run: &IndividualRun, metric: &str) -> Option<u64> {
    if let Some(recovery) = &run.recovery_report {
        return match metric {
            "amplification-ppm" => Some(recovery.costs.amplification_ppm),
            "peak-queue-bytes" => Some(recovery.peak_queue_bytes),
            "goodput-stall-ns" => Some(recovery.traffic.goodput_stall_ns),
            "starved-flows" => Some(recovery.traffic.starved_flows),
            "cache-invalidations" => Some(recovery.cache.invalidations),
            _ => base_metric(run, metric),
        };
    }
    let recovery = run.report.graph_recovery.as_ref()?;
    let traffic = run.report.routed_traffic.as_ref();
    match metric {
        "amplification-ppm" => {
            let transmitted: u64 = run
                .report
                .links
                .values()
                .map(|link| link.transmitted_bytes)
                .sum();
            let useful = traffic.map_or(0, |value| value.delivered_useful_bytes);
            Some(
                transmitted
                    .saturating_mul(1_000_000)
                    .checked_div(useful)
                    .unwrap_or(0),
            )
        }
        "peak-queue-bytes" => run
            .report
            .links
            .values()
            .map(|link| link.peak_queue_bytes)
            .max(),
        "goodput-stall-ns" => Some(traffic.map_or(0, |value| value.goodput_stall_ns)),
        "starved-flows" => Some(traffic.map_or(0, |value| value.rejected_flows)),
        "cache-invalidations" => Some(recovery.cache.invalidations),
        _ => base_metric(run, metric),
    }
}

fn base_metric(run: &IndividualRun, metric: &str) -> Option<u64> {
    match metric {
        "convergence-time-ns" => Some(run.report.quiescence_ns),
        "control-bytes" => Some(run.report.tree_announce.transmitted_frame_bytes),
        _ => None,
    }
}
