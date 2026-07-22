use super::*;

pub(super) fn aggregate_resources(pools: &[ResourcePool]) -> ResourceCounters {
    let mut total = ResourceCounters::default();
    for pool in pools {
        for (kind, units) in &pool.counters.consumed {
            *total.consumed.entry(*kind).or_default() += units;
        }
        total.queue_wait_ns = total
            .queue_wait_ns
            .saturating_add(pool.counters.queue_wait_ns);
        total.maximum_queue_wait_ns = total
            .maximum_queue_wait_ns
            .max(pool.counters.maximum_queue_wait_ns);
        total.exhaustion_count = total
            .exhaustion_count
            .saturating_add(pool.counters.exhaustion_count);
    }
    total
}

pub(super) fn resource_receipts_reconcile(
    pools: &[ResourcePool],
    projection: &BTreeMap<ResourceKind, u64>,
) -> bool {
    let mut receipts = BTreeMap::new();
    for pool in pools {
        for receipt in &pool.receipts {
            *receipts.entry(receipt.kind).or_default() += receipt.units;
        }
    }
    &receipts == projection
}

pub(super) fn merge_lookup(total: &mut LookupCounters, item: &LookupCounters) {
    total.lookups += item.lookups;
    total.attempts += item.attempts;
    total.successes += item.successes;
    total.failures += item.failures;
    total.retries += item.retries;
    total.deduplicated += item.deduplicated;
    total.false_positive_candidates += item.false_positive_candidates;
    total.request_message_bytes += item.request_message_bytes;
    total.response_message_bytes += item.response_message_bytes;
    total.signals_rate_limited += item.signals_rate_limited;
    total.recovery_time_ns = total.recovery_time_ns.max(item.recovery_time_ns);
    for (signal, count) in &item.signals {
        *total.signals.entry(*signal).or_default() += count;
    }
}

pub(super) fn critical_path(
    resources: &ResourceCounters,
    link: &LinkCounters,
    root_recovery_ns: u64,
    lookup_ns: u64,
    bloom_debounce_ns: u64,
    maximum_flow_latency_ns: u64,
) -> CriticalPath {
    let candidates = [
        ("root-convergence", root_recovery_ns),
        ("bloom-debounce", bloom_debounce_ns),
        ("resource-queue", resources.maximum_queue_wait_ns),
        ("lookup-retry", lookup_ns),
        ("useful-flow-latency", maximum_flow_latency_ns),
    ];
    let (component, duration_ns) = candidates
        .into_iter()
        .max_by_key(|(component, duration)| (*duration, *component))
        .unwrap_or(("none", 0));
    CriticalPath {
        component: component.to_owned(),
        duration_ns,
        explanation: format!(
            "{component} dominated recovery; the shared link transmitted {} bytes while resource queues contributed up to {} ns wait",
            link.transmitted_bytes, resources.maximum_queue_wait_ns
        ),
        evidence: vec![
            "input:arrival-0000".to_owned(),
            "edge:0".to_owned(),
            "aggregate:m2".to_owned(),
        ],
    }
}

pub(super) fn depth_adoption(root: &RootRatchetReport, root_ns: u64) -> BTreeMap<String, u64> {
    let mut result = BTreeMap::new();
    let max = root.maximum_depth.max(1);
    let bands = max.div_ceil(8);
    for band in 0..bands {
        result.insert(
            format!("{}-{}", band * 8, (band * 8 + 7).min(max)),
            root_ns.saturating_mul(band + 1) / bands,
        );
    }
    result
}

pub(super) fn assertion(id: &str, passed: bool, message: &str) -> AssertionResult {
    AssertionResult {
        id: id.to_owned(),
        outcome: if passed { "pass" } else { "fail" }.to_owned(),
        message: message.to_owned(),
    }
}

pub(super) fn ledger(cause: &str, stage: &str, count: u64, evidence: &str) -> LedgerEntry {
    ledger_child(cause, None, stage, count, evidence)
}

pub(super) fn ledger_child(
    cause: &str,
    parent: Option<&str>,
    stage: &str,
    count: u64,
    evidence: &str,
) -> LedgerEntry {
    LedgerEntry {
        causal_id: cause.to_owned(),
        causal_parent: parent.map(str::to_owned),
        stage: stage.to_owned(),
        count,
        evidence: vec![evidence.to_owned()],
    }
}

pub(super) fn stage_total(ledger: &[LedgerEntry], stage: &str) -> u64 {
    ledger
        .iter()
        .filter(|entry| entry.stage == stage)
        .map(|entry| entry.count)
        .sum()
}

pub(super) fn ratio_ppm(numerator: u64, denominator: u64) -> u64 {
    if denominator == 0 {
        return 0;
    }
    numerator.saturating_mul(1_000_000) / denominator
}

pub(super) fn node_key(node: u32) -> [u8; 16] {
    let mut key = [0; 16];
    key[12..].copy_from_slice(&node.to_be_bytes());
    key
}

pub(super) fn chain_path(node: u32) -> Vec<u32> {
    (0..=node.min(64)).rev().collect()
}

pub(super) fn depth_band(depth: u32) -> &'static str {
    match depth {
        0..=7 => "0-7",
        8..=15 => "8-15",
        16..=31 => "16-31",
        32..=63 => "32-63",
        _ => "64+",
    }
}

pub(super) fn hex_16(value: &str) -> Result<[u8; 16], RecoveryError> {
    let bytes = hex::decode(value).map_err(|_| RecoveryError::RootAddress(value.to_owned()))?;
    bytes
        .try_into()
        .map_err(|_| RecoveryError::RootAddress(value.to_owned()))
}

pub(super) fn string_at<'a>(campaign: &'a Value, pointer: &str) -> Option<&'a str> {
    campaign.pointer(pointer).and_then(Value::as_str)
}

pub(super) fn u64_at(campaign: &Value, pointer: &str) -> Option<u64> {
    campaign.pointer(pointer).and_then(Value::as_u64)
}

pub(super) fn decimal_at(campaign: &Value, pointer: &str) -> Option<f64> {
    campaign.pointer(pointer).and_then(Value::as_f64)
}

pub(super) fn duration_at(campaign: &Value, pointer: &str) -> Option<u64> {
    campaign
        .pointer(pointer)
        .and_then(|value| value.get("nanoseconds"))
        .and_then(Value::as_u64)
}
