use super::*;

pub(super) fn parse_sybils(
    campaign: &Value,
    event: &Value,
    index: usize,
    nodes: u64,
    output: &mut Vec<SybilArrivalInput>,
) -> Result<(), RunError> {
    if campaign
        .pointer("/adversaries/mode")
        .and_then(Value::as_str)
        != Some("authenticated-protocol-valid")
    {
        return Err(RunError::Unsupported(
            "attach-authenticated-sybils requires adversaries.mode authenticated-protocol-valid"
                .to_owned(),
        ));
    }
    let budget_identities = campaign
        .pointer("/adversaries/budgets/identities")
        .and_then(Value::as_u64);
    let count = intervention_config::parameter_u64(event, "count")
        .or(budget_identities)
        .ok_or_else(|| {
            RunError::Unsupported(
                "attach-authenticated-sybils requires parameters.count or an identity budget"
                    .to_owned(),
            )
        })?;
    if count == 0 || count > 100_000 || count >= nodes {
        return Err(RunError::Unsupported(
            "authenticated Sybil count must be in 1..=100000 and below scale".to_owned(),
        ));
    }
    if budget_identities.is_some_and(|budget| count > budget) {
        return Err(RunError::BudgetExhausted {
            required: count,
            available: budget_identities.unwrap(),
        });
    }
    let operations =
        intervention_config::parameter_u64(event, "operations_per_identity").unwrap_or(1);
    let required_operations = count.saturating_mul(operations);
    let operation_budget = campaign
        .pointer("/adversaries/budgets/operations")
        .and_then(Value::as_u64);
    if operation_budget.is_some_and(|budget| required_operations > budget) {
        return Err(RunError::BudgetExhausted {
            required: required_operations,
            available: operation_budget.unwrap(),
        });
    }
    let address_policy = event
        .pointer("/parameters/address_policy")
        .and_then(Value::as_str)
        .unwrap_or("uniform-valid");
    if !matches!(address_policy, "uniform-valid" | "lower-than-current-root") {
        return Err(RunError::Unsupported(format!(
            "unsupported authenticated Sybil address policy {address_policy}"
        )));
    }
    let attachment = event
        .pointer("/parameters/attachment")
        .and_then(Value::as_str)
        .unwrap_or("hub");
    let attachment = AttachmentSelector::parse(attachment)?;
    let interval_ns = event
        .pointer("/parameters/interval/nanoseconds")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let at_ns = intervention_config::required_time(event, "attach-authenticated-sybils")?;
    let id = intervention_config::event_id(event, index, "attach-authenticated-sybils");
    for ordinal in 0..count {
        output.push(SybilArrivalInput {
            id: id.clone(),
            at_ns: at_ns
                .checked_add(interval_ns.saturating_mul(ordinal))
                .ok_or(RunError::Arithmetic)?,
            ordinal: ordinal as u32,
            address_policy: address_policy.to_owned(),
            attachment,
            operations,
        });
    }
    Ok(())
}

pub(super) fn parse_parent_costs(
    event: &Value,
    index: usize,
    action: &str,
    nodes: u64,
    output: &mut Vec<ParentCostInput>,
) -> Result<(), RunError> {
    let target = event
        .get("target")
        .and_then(Value::as_u64)
        .or_else(|| event.pointer("/parameters/node").and_then(Value::as_u64));
    if target.is_some_and(|node| node >= nodes) {
        return Err(RunError::Unsupported(format!(
            "{action} targets a node outside scale {nodes}"
        )));
    }
    let cycles = if action == "alternate-parent-quality" {
        intervention_config::parameter_u64(event, "cycles").unwrap_or(4)
    } else {
        1
    };
    if !(1..=1_000).contains(&cycles) {
        return Err(RunError::Unsupported(format!(
            "{action} cycles must be in 1..=1000"
        )));
    }
    let interval_ns = event
        .pointer("/parameters/interval/nanoseconds")
        .and_then(Value::as_u64)
        .unwrap_or(250_000_000);
    let preferred_cost_ppm =
        intervention_config::parameter_u64(event, "preferred_cost_ppm").unwrap_or(1_000_000);
    let degraded_cost_ppm =
        intervention_config::parameter_u64(event, "degraded_cost_ppm").unwrap_or(6_000_000);
    if preferred_cost_ppm == 0 || degraded_cost_ppm <= preferred_cost_ppm {
        return Err(RunError::Unsupported(format!(
            "{action} requires 0 < preferred_cost_ppm < degraded_cost_ppm"
        )));
    }
    let at_ns = intervention_config::required_time(event, action)?;
    let id = intervention_config::event_id(event, index, action);
    for phase in 0..cycles {
        output.push(ParentCostInput {
            id: id.clone(),
            at_ns: at_ns
                .checked_add(interval_ns.saturating_mul(phase))
                .ok_or(RunError::Arithmetic)?,
            target: target.map(|node| node as NodeId),
            phase: phase as u32,
            action: action.to_owned(),
            preferred_cost_ppm,
            degraded_cost_ppm,
        });
    }
    Ok(())
}
