use super::*;
use crate::{Flow, TrafficPlan};

pub(crate) const SESSION_DATA_OVERHEAD_BYTES: u64 = 106;

/// Reconciled primary-scheduler synthetic traffic accounting.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutedTrafficCounters {
    pub offered_flows: u64,
    pub delivered_flows: u64,
    pub rejected_flows: u64,
    pub offered_useful_bytes: u64,
    pub delivered_useful_bytes: u64,
    pub lost_useful_bytes: u64,
    pub transmitted_wire_bytes: u64,
    pub delivered_wire_bytes: u64,
    pub maximum_hops: u64,
    /// Longest virtual-time gap between useful deliveries after traffic starts.
    pub goodput_stall_ns: u64,
    /// Last virtual time at which routed payload work occurred.
    pub quiescence_ns: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct RoutedTrafficRuntime {
    pub plan: TrafficPlan,
    pub start_ns: u64,
    pub counters: RoutedTrafficCounters,
    pub last_useful_delivery_ns: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RoutedFrame {
    pub(super) flow: Flow,
    pub(super) path: Vec<NodeId>,
    pub(super) hop: usize,
    pub(super) frame_bytes: u64,
}
