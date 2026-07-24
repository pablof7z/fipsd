use super::*;

pub(super) fn resource_profiles(
    plan: &NormalizedPlan,
    nodes: u32,
) -> Result<Vec<ResourceProfile>, RunError> {
    let baseline = resource_profile(plan)?;
    let heterogeneous = plan
        .campaign
        .pointer("/resources/assignment")
        .and_then(Value::as_str)
        == Some("heterogeneous");
    Ok((0..nodes)
        .map(|node| {
            let mut profile = baseline.clone();
            if heterogeneous && (node == 0 || node + 1 == nodes) {
                profile.name = if node == 0 { "slow-root" } else { "slow-leaf" }.to_owned();
                profile.cpu_units_per_ms = profile.cpu_units_per_ms.max(10) / 10;
            }
            profile
        })
        .collect())
}

fn resource_profile(plan: &NormalizedPlan) -> Result<ResourceProfile, RunError> {
    let mut profile = ResourceProfile::baseline();
    profile.cpu_units_per_ms = integer(plan, "/resources/node_profiles/0/cpu_units", 1_000)?;
    for (kind, pointer, default) in [
        (
            ResourceKind::AllocationBytes,
            "/resources/node_profiles/0/memory_bytes",
            1 << 30,
        ),
        (
            ResourceKind::QueueBytes,
            "/resources/node_profiles/0/queue_bytes",
            1 << 20,
        ),
        (
            ResourceKind::CacheEntries,
            "/resources/node_profiles/0/table_entries",
            100_000,
        ),
    ] {
        profile
            .capacities
            .insert(kind, integer(plan, pointer, default)?);
    }
    Ok(profile)
}

fn integer(plan: &NormalizedPlan, pointer: &str, default: u64) -> Result<u64, RunError> {
    Ok(plan
        .campaign
        .pointer(pointer)
        .and_then(Value::as_u64)
        .unwrap_or(default))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fips_model::normalize_str;

    #[test]
    fn heterogeneous_assignment_slows_root_and_leaf_only() {
        let yaml = r#"
apiVersion: experiments.fips.network/v1alpha1
kind: Campaign
metadata: {name: resources}
seed: 1
engine: {modes: compact-discrete-event, deterministic: true, variant: baseline}
scale: {nodes: 4}
topology: {generator: chain}
identities: {initial: {distribution: uniform-128}}
transports: {assignment: all-udp}
links: {latency: 1ms, bandwidth_bps: 1000000, loss_ppm: 0, duplication_ppm: 0, ordering: datagram, mtu_bytes: 1500, queue_bytes: 1024, drop_policy: tail-drop}
resources: {assignment: heterogeneous, node_profiles: [{name: base, cpu_units: 1000, memory_bytes: 1000, queue_bytes: 1000, table_entries: 1000}]}
protocol: {variant: baseline, parameters: {tree_announce_debounce: 1ms}}
traffic: {model: idle}
fidelity: {protocol: semantic-exact, serialization: executable-codec, bloom: exact-bits, crypto: operation-count, billion_node_representation: not-requested}
accounting: {causal_lineage: true, transport_overhead: true, network_overhead: configured, reconcile_serialized_frames: true}
instrumentation: {root_agreement_by_depth: true, transition_stages: true, causal_cost_ledger: true, queue_wait: true, control_and_useful_bytes: true, quiescence_markers: [root, tree]}
assertions: []
objectives: {maximize: [control-bytes]}
"#;
        let plan = normalize_str(yaml).unwrap();
        let profiles = resource_profiles(&plan, 4).unwrap();
        assert_eq!(profiles[0].cpu_units_per_ms, 100);
        assert_eq!(profiles[1].cpu_units_per_ms, 1_000);
        assert_eq!(profiles[2].cpu_units_per_ms, 1_000);
        assert_eq!(profiles[3].cpu_units_per_ms, 100);
    }
}
