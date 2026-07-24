use super::*;
use serde_json::json;
use sha2::{Digest, Sha256};

pub(super) fn event_time(event: &Value) -> Option<u64> {
    event.pointer("/at/nanoseconds").and_then(Value::as_u64)
}

pub(super) fn action_id(event: &Value) -> String {
    let action = event
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("action");
    match event.get("id").and_then(Value::as_str) {
        Some(id) if id != action => format!("{action} [{id}]"),
        _ => action.to_owned(),
    }
}

pub(super) fn candidate_plan(
    source: &NormalizedPlan,
    fixed: &[Value],
    actions: &[Value],
    order: &[usize],
    start_ns: u64,
    step_ns: u64,
) -> Result<NormalizedPlan, TinyExplorerError> {
    let mut campaign = source.campaign.clone();
    let mut events = fixed.to_vec();
    for (position, index) in order.iter().enumerate() {
        let mut event = actions[*index].clone();
        let offset = (position as u64)
            .checked_mul(step_ns)
            .ok_or(TinyExplorerError::Arithmetic)?;
        event["at"] = json!({"nanoseconds": start_ns.checked_add(offset)
            .ok_or(TinyExplorerError::Arithmetic)?});
        events.push(event);
    }
    campaign["events"] = Value::Array(events);
    let campaign_sha256 = digest(&serde_json::to_vec(&campaign)?);
    Ok(NormalizedPlan {
        campaign_sha256,
        campaign,
        ..source.clone()
    })
}

pub(super) fn permutations(values: &mut [usize], position: usize, output: &mut Vec<Vec<usize>>) {
    if position == values.len() {
        output.push(values.to_vec());
        return;
    }
    for index in position..values.len() {
        values.swap(position, index);
        permutations(values, position + 1, output);
        values.swap(position, index);
    }
}

pub(super) fn factorial(value: usize) -> Result<u64, TinyExplorerError> {
    (1..=value)
        .try_fold(1_u64, |product, item| product.checked_mul(item as u64))
        .ok_or(TinyExplorerError::Arithmetic)
}

pub(super) fn terminal_signature(
    report: &fips_engine::RootRatchetReport,
) -> Result<String, TinyExplorerError> {
    Ok(digest(&serde_json::to_vec(&json!({
        "root": report.final_root, "graph": report.graph_sha256,
        "depth": report.maximum_depth, "parents": report.parent_transitions,
        "assertions": report.assertions,
    }))?))
}

pub(super) fn counterexample(
    plan: &NormalizedPlan,
    action_order: Vec<String>,
    failure: String,
) -> Result<TinyCounterexample, TinyExplorerError> {
    let id = format!(
        "counterexample-{}",
        &digest(&serde_json::to_vec(plan)?)[..24]
    );
    Ok(TinyCounterexample {
        api_version: TINY_COUNTEREXAMPLE_VERSION.to_owned(),
        id,
        fidelity: "individual semantic engine; exact enumerated input order".to_owned(),
        action_order,
        normalized_plan: plan.clone(),
        failure,
    })
}

pub(super) fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
