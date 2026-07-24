use super::*;

#[derive(Debug, Clone)]
pub(super) struct LifecycleInput {
    pub at_ns: u64,
    pub node: NodeId,
    pub reappear: bool,
}

#[derive(Debug, Clone)]
pub(super) struct ManualArrivalInput {
    pub at_ns: u64,
    pub lower_root: bool,
    pub targets: Vec<NodeId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct NetworkCutInput {
    pub id: String,
    pub at_ns: u64,
    pub nodes: Vec<NodeId>,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct LinkUpdateInput {
    pub id: String,
    pub at_ns: u64,
    pub edge: EdgeId,
    pub restore: bool,
    pub bandwidth_bps: Option<u64>,
    pub latency_ns: Option<u64>,
    pub jitter_ns: Option<u64>,
    pub loss_ppm: Option<u32>,
    pub mtu_bytes: Option<u64>,
    pub queue_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct SessionRekeyInput {
    pub id: String,
    pub at_ns: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct SessionRekeyCompletion {
    pub cause: String,
    pub source: NodeId,
    pub destination: NodeId,
    pub path: Vec<NodeId>,
    pub requested_at_ns: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct CacheExpiryInput {
    pub id: String,
    pub at_ns: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct LookupWaveInput {
    pub id: String,
    pub at_ns: u64,
    pub count: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct TransportClassInput {
    pub id: String,
    pub at_ns: u64,
    pub profile: String,
    pub restore: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ParentCostInput {
    pub id: String,
    pub at_ns: u64,
    pub target: Option<NodeId>,
    pub phase: u32,
    pub action: String,
    pub preferred_cost_ppm: u64,
    pub degraded_cost_ppm: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct SybilArrivalInput {
    pub id: String,
    pub at_ns: u64,
    pub ordinal: u32,
    pub address_policy: String,
    pub attachment: AttachmentSelector,
    pub operations: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct TreeSnapshot {
    pub root: NodeId,
    pub root_address: NodeAddress,
    pub parent: Option<NodeId>,
    pub sequence: u64,
    pub ancestry: Vec<NodeId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum SimEvent {
    InitialAnnounce,
    Activate {
        node: NodeId,
        ordinal: u32,
        lower_root: bool,
        targets: Vec<NodeId>,
    },
    AnnounceDue {
        from: NodeId,
        to: NodeId,
        cause: String,
    },
    DeliverAnnounce {
        delivery: Delivery,
        snapshot: TreeSnapshot,
        cause: String,
    },
    InjectParentLoop,
    Deactivate {
        node: NodeId,
    },
    Reappear {
        node: NodeId,
    },
    NetworkCut {
        input: NetworkCutInput,
    },
    LinkUpdate {
        input: LinkUpdateInput,
    },
    SessionRekey {
        input: SessionRekeyInput,
    },
    SessionRekeyCompleted {
        completion: SessionRekeyCompletion,
    },
    ExpireCoordinateCache {
        input: CacheExpiryInput,
    },
    LookupWave {
        input: LookupWaveInput,
    },
    TransportClass {
        input: TransportClassInput,
    },
    ParentCost {
        input: ParentCostInput,
    },
    SybilArrival {
        input: SybilArrivalInput,
        node: NodeId,
    },
    TrafficOffer {
        index: usize,
    },
    TrafficHopDue {
        frame: RoutedFrame,
    },
    DeliverTraffic {
        delivery: Delivery,
        frame: RoutedFrame,
        forward_copy: u8,
    },
    BloomDue {
        from: NodeId,
        to: NodeId,
        cause: String,
    },
    DeliverBloom {
        delivery: Delivery,
        snapshot: BloomSnapshot,
        cause: String,
    },
    LookupStart {
        index: usize,
        attempt: u32,
    },
    RecoveryHopDue {
        frame: RecoveryFrame,
    },
    DeliverRecovery {
        delivery: Delivery,
        frame: RecoveryFrame,
        forward_copy: u8,
    },
}

impl SimEvent {
    pub(super) fn kind(&self) -> &'static str {
        match self {
            Self::InitialAnnounce => "input.initial-topology",
            Self::Activate {
                lower_root: true, ..
            } => "input.descending-root-arrival",
            Self::Activate { .. } => "input.node-arrived",
            Self::AnnounceDue { .. } => "tree-announce.due",
            Self::DeliverAnnounce { .. } => "tree-announce.delivered",
            Self::InjectParentLoop => "fault.inject-parent-loop",
            Self::Deactivate { .. } => "input.node-disappeared",
            Self::Reappear { .. } => "input.node-reappeared",
            Self::NetworkCut { input } if input.enabled => "input.network-merged",
            Self::NetworkCut { .. } => "input.network-partitioned",
            Self::LinkUpdate { input } if input.restore => "input.link-conditions-restored",
            Self::LinkUpdate { .. } => "input.link-conditions-changed",
            Self::SessionRekey { .. } => "input.session-rekey-wave",
            Self::SessionRekeyCompleted { .. } => "session.rekey-completed",
            Self::ExpireCoordinateCache { .. } => "input.coordinate-cache-expired",
            Self::LookupWave { .. } => "input.lookup-wave",
            Self::TransportClass { input } if input.restore => "input.transport-class-restored",
            Self::TransportClass { .. } => "input.transport-class-failed",
            Self::ParentCost { input } if input.action == "swap-parent-ancestry" => {
                "input.parent-ancestry-swapped"
            }
            Self::ParentCost { .. } => "input.parent-quality-alternated",
            Self::SybilArrival { .. } => "input.authenticated-sybil-arrived",
            Self::TrafficOffer { .. } => "data.flow-offered",
            Self::TrafficHopDue { .. } => "data.frame-due",
            Self::DeliverTraffic { .. } => "data.frame-delivered",
            Self::BloomDue { .. } => "bloom.filter-due",
            Self::DeliverBloom { .. } => "bloom.filter-delivered",
            Self::LookupStart { .. } => "lookup.attempt-started",
            Self::RecoveryHopDue { frame } => match frame.phase.plane() {
                "lookup" => "lookup.frame-due",
                _ => "session.frame-due",
            },
            Self::DeliverRecovery { frame, .. } => match frame.phase.plane() {
                "lookup" => "lookup.frame-delivered",
                _ => "session.frame-delivered",
            },
        }
    }
}
