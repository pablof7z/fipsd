pub struct FamilyInput {
    pub id: &'static str,
    pub title: &'static str,
    pub source: &'static str,
    pub source_path: &'static str,
}

pub const INPUTS: &[FamilyInput] = &[
    FamilyInput {
        id: "root-ratchet",
        title: "Descending-Minimum Root Ratchet",
        source: include_str!("../../../examples/root-ratchet.yaml"),
        source_path: "examples/root-ratchet.yaml",
    },
    FamilyInput {
        id: "competing-partition-roots",
        title: "Competing Partition Roots",
        source: include_str!("../../../examples/campaigns/competing-partition-roots.yaml"),
        source_path: "examples/campaigns/competing-partition-roots.yaml",
    },
    FamilyInput {
        id: "bloom-saturation-accession",
        title: "Bloom Saturation under Accession",
        source: include_str!("../../../examples/campaigns/bloom-saturation-accession.yaml"),
        source_path: "examples/campaigns/bloom-saturation-accession.yaml",
    },
    FamilyInput {
        id: "ancestor-swap-bloom-storm",
        title: "Ancestor-Swap Bloom Storm",
        source: include_str!("../../../examples/campaigns/ancestor-swap-bloom-storm.yaml"),
        source_path: "examples/campaigns/ancestor-swap-bloom-storm.yaml",
    },
    FamilyInput {
        id: "deep-tree-mtu-ttl-cliff",
        title: "Deep-Tree MTU and TTL Cliff",
        source: include_str!("../../../examples/campaigns/deep-tree-mtu-ttl-cliff.yaml"),
        source_path: "examples/campaigns/deep-tree-mtu-ttl-cliff.yaml",
    },
    FamilyInput {
        id: "lookup-thundering-herd",
        title: "Lookup Thundering Herd",
        source: include_str!("../../../examples/campaigns/lookup-thundering-herd.yaml"),
        source_path: "examples/campaigns/lookup-thundering-herd.yaml",
    },
    FamilyInput {
        id: "parent-hysteresis-oscillator",
        title: "Parent Hysteresis Oscillator",
        source: include_str!("../../../examples/campaigns/parent-hysteresis-oscillator.yaml"),
        source_path: "examples/campaigns/parent-hysteresis-oscillator.yaml",
    },
    FamilyInput {
        id: "mixed-transport-failover",
        title: "Mixed-Transport Failover",
        source: include_str!("../../../examples/campaigns/mixed-transport-failover.yaml"),
        source_path: "examples/campaigns/mixed-transport-failover.yaml",
    },
    FamilyInput {
        id: "synchronized-rekey-avalanche",
        title: "Synchronized Rekey Avalanche",
        source: include_str!("../../../examples/campaigns/synchronized-rekey-avalanche.yaml"),
        source_path: "examples/campaigns/synchronized-rekey-avalanche.yaml",
    },
    FamilyInput {
        id: "authenticated-sybil-pressure",
        title: "Authenticated Sybil Pressure",
        source: include_str!("../../../examples/campaigns/authenticated-sybil-pressure.yaml"),
        source_path: "examples/campaigns/authenticated-sybil-pressure.yaml",
    },
];
