//! Seed-stable synthetic sessions and useful-payload traffic.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Executable-codec depth-zero session setup message 1.
pub const SESSION_SETUP_MESSAGE_BYTES: u64 = 76;
/// Executable-codec depth-zero session acknowledgement.
pub const SESSION_ACK_MESSAGE_BYTES: u64 = 100;

/// Supported synthetic traffic matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrafficModel {
    /// No useful traffic.
    Idle,
    /// Seeded source/destination pairs.
    UniformRandom,
    /// One-to-one permutation.
    Permutation,
    /// Every ordered pair.
    AllToAll,
    /// Skewed destination popularity.
    Zipf,
    /// Many sources to one destination.
    Incast,
    /// One source to many destinations.
    Outcast,
    /// Mixed large and small payloads.
    ElephantsAndMice,
    /// Cross a deterministic bisection.
    CrossCut,
    /// Repeated session setup and teardown.
    SessionChurn,
    /// Cycle payload sizes around MTU boundaries.
    PayloadSweep,
}

impl TrafficModel {
    /// Parse Campaign spelling.
    pub fn parse(value: &str) -> Result<Self, TrafficError> {
        match value {
            "idle" => Ok(Self::Idle),
            "uniform-random" => Ok(Self::UniformRandom),
            "permutation" => Ok(Self::Permutation),
            "all-to-all" => Ok(Self::AllToAll),
            "zipf" => Ok(Self::Zipf),
            "incast" => Ok(Self::Incast),
            "outcast" => Ok(Self::Outcast),
            "elephants-and-mice" => Ok(Self::ElephantsAndMice),
            "cross-min-cut" | "cross-cut" => Ok(Self::CrossCut),
            "session-churn" => Ok(Self::SessionChurn),
            "payload-sweep" => Ok(Self::PayloadSweep),
            other => Err(TrafficError::Unknown(other.to_owned())),
        }
    }
}

/// Session lifecycle action attached to a flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionAction {
    /// Establish a session before delivery.
    Setup,
    /// Existing session carries data.
    Reuse,
    /// Rekey hook executes before delivery.
    Rekey,
    /// Tear down after this packet.
    Teardown,
}

/// One offered useful-payload flow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flow {
    /// Stable flow ID.
    pub id: String,
    /// Source node.
    pub source: u32,
    /// Destination node.
    pub destination: u32,
    /// Offer time.
    pub offered_at_ns: u64,
    /// Application payload only.
    pub useful_payload_bytes: u64,
    /// Session lifecycle action.
    pub session_action: SessionAction,
}

/// Generator inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrafficConfig {
    /// Matrix.
    pub model: TrafficModel,
    /// Node count.
    pub nodes: u32,
    /// Number of offered flows; ignored for all-to-all.
    pub flow_count: u64,
    /// Base payload.
    pub payload_bytes: u64,
    /// Offer interval.
    pub interval_ns: u64,
    /// Seed.
    pub seed: u64,
}

/// Generated session/traffic accounting.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrafficPlan {
    /// Stable offered flows.
    pub flows: Vec<Flow>,
    /// Application payload offered.
    pub offered_useful_bytes: u64,
    /// Session setup operations.
    pub session_setups: u64,
    /// Session teardown operations.
    pub session_teardowns: u64,
    /// Abstract encryption/rekey hooks.
    pub rekeys: u64,
    /// Exact setup/ack message bytes, excluding transport/FMP overhead.
    pub setup_message_bytes: u64,
}

impl TrafficPlan {
    /// Generate a deterministic traffic plan and validate offered load.
    pub fn generate(config: &TrafficConfig) -> Result<Self, TrafficError> {
        if config.nodes < 2 && config.model != TrafficModel::Idle {
            return Err(TrafficError::TooFewNodes(config.nodes));
        }
        if config.payload_bytes == 0 && config.model != TrafficModel::Idle {
            return Err(TrafficError::ZeroPayload);
        }
        let count = match config.model {
            TrafficModel::Idle => 0,
            TrafficModel::AllToAll => {
                u64::from(config.nodes) * u64::from(config.nodes.saturating_sub(1))
            }
            _ => config.flow_count,
        };
        let mut plan = Self::default();
        for ordinal in 0..count {
            let (source, destination) = endpoints(config, ordinal);
            if source == destination {
                return Err(TrafficError::SelfFlow {
                    ordinal,
                    node: source,
                });
            }
            let session_action = match config.model {
                TrafficModel::SessionChurn => {
                    if ordinal % 2 == 0 {
                        SessionAction::Setup
                    } else {
                        SessionAction::Teardown
                    }
                }
                _ if ordinal % 17 == 0 => SessionAction::Setup,
                _ if ordinal % 101 == 0 => SessionAction::Rekey,
                _ => SessionAction::Reuse,
            };
            let useful_payload_bytes = match config.model {
                TrafficModel::ElephantsAndMice if ordinal % 10 == 0 => {
                    config.payload_bytes.saturating_mul(100)
                }
                TrafficModel::PayloadSweep => {
                    const SIZES: [u64; 8] = [64, 256, 1024, 1200, 1279, 1280, 1500, 9000];
                    SIZES[ordinal as usize % SIZES.len()]
                }
                _ => config.payload_bytes,
            };
            match session_action {
                SessionAction::Setup => plan.session_setups += 1,
                SessionAction::Teardown => plan.session_teardowns += 1,
                SessionAction::Rekey => plan.rekeys += 1,
                SessionAction::Reuse => {}
            }
            plan.offered_useful_bytes = plan
                .offered_useful_bytes
                .saturating_add(useful_payload_bytes);
            plan.flows.push(Flow {
                id: format!("flow-{ordinal:08}"),
                source,
                destination,
                offered_at_ns: ordinal.saturating_mul(config.interval_ns),
                useful_payload_bytes,
                session_action,
            });
        }
        plan.setup_message_bytes = plan
            .session_setups
            .saturating_mul(SESSION_SETUP_MESSAGE_BYTES + SESSION_ACK_MESSAGE_BYTES);
        let projected = plan
            .flows
            .iter()
            .map(|flow| flow.useful_payload_bytes)
            .sum::<u64>();
        if projected != plan.offered_useful_bytes {
            return Err(TrafficError::OfferedLoadDrift {
                recorded: plan.offered_useful_bytes,
                projected,
            });
        }
        Ok(plan)
    }
}

/// Traffic configuration error.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TrafficError {
    /// Unknown Campaign value.
    #[error("unknown traffic model {0}")]
    Unknown(String),
    /// Non-idle traffic requires at least two nodes.
    #[error("traffic requires at least two nodes, got {0}")]
    TooFewNodes(u32),
    /// Useful payload cannot be zero.
    #[error("non-idle traffic requires a positive payload")]
    ZeroPayload,
    /// Generator produced an invalid self-flow.
    #[error("traffic flow {ordinal} has identical source/destination {node}")]
    SelfFlow {
        /// Flow ordinal.
        ordinal: u64,
        /// Node.
        node: u32,
    },
    /// Aggregate does not equal flows.
    #[error("offered load drift: recorded {recorded}, projected {projected}")]
    OfferedLoadDrift {
        /// Recorded aggregate.
        recorded: u64,
        /// Per-flow projection.
        projected: u64,
    },
}

fn endpoints(config: &TrafficConfig, ordinal: u64) -> (u32, u32) {
    let nodes = u64::from(config.nodes);
    match config.model {
        TrafficModel::Idle => (0, 0),
        TrafficModel::UniformRandom => {
            let source = draw(config.seed, ordinal, 0) % nodes;
            let offset = 1 + draw(config.seed, ordinal, 1) % (nodes - 1);
            (source as u32, ((source + offset) % nodes) as u32)
        }
        TrafficModel::Permutation => {
            let source = ordinal % nodes;
            (source as u32, ((source + 1) % nodes) as u32)
        }
        TrafficModel::AllToAll => {
            let source = ordinal / (nodes - 1);
            let mut destination = ordinal % (nodes - 1);
            if destination >= source {
                destination += 1;
            }
            (source as u32, destination as u32)
        }
        TrafficModel::Zipf => {
            let source = ordinal % nodes;
            let draw = draw(config.seed, ordinal, 2) as f64 / u64::MAX as f64;
            let mut destination = (draw * draw * nodes as f64) as u64 % nodes;
            if destination == source {
                destination = (destination + 1) % nodes;
            }
            (source as u32, destination as u32)
        }
        TrafficModel::Incast => ((ordinal % (nodes - 1) + 1) as u32, 0),
        TrafficModel::Outcast => (0, (ordinal % (nodes - 1) + 1) as u32),
        TrafficModel::ElephantsAndMice | TrafficModel::PayloadSweep => {
            let source = ordinal % nodes;
            (
                source as u32,
                ((source + nodes / 2).max(source + 1) % nodes) as u32,
            )
        }
        TrafficModel::CrossCut => {
            let half = (nodes / 2).max(1);
            let source = ordinal % half;
            let destination = half + ordinal % (nodes - half);
            (source as u32, destination as u32)
        }
        TrafficModel::SessionChurn => {
            let pair = (ordinal / 2) % nodes;
            (pair as u32, ((pair + 1) % nodes) as u32)
        }
    }
}

fn draw(seed: u64, ordinal: u64, lane: u64) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(seed.to_le_bytes());
    hasher.update(ordinal.to_le_bytes());
    hasher.update(lane.to_le_bytes());
    let digest = hasher.finalize();
    u64::from_le_bytes(digest[0..8].try_into().expect("slice length"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(model: TrafficModel) -> TrafficConfig {
        TrafficConfig {
            model,
            nodes: 16,
            flow_count: 100,
            payload_bytes: 1_000,
            interval_ns: 1_000_000,
            seed: 77,
        }
    }

    #[test]
    fn every_traffic_model_is_seed_stable_and_has_no_self_flows() {
        let models = [
            TrafficModel::Idle,
            TrafficModel::UniformRandom,
            TrafficModel::Permutation,
            TrafficModel::AllToAll,
            TrafficModel::Zipf,
            TrafficModel::Incast,
            TrafficModel::Outcast,
            TrafficModel::ElephantsAndMice,
            TrafficModel::CrossCut,
            TrafficModel::SessionChurn,
            TrafficModel::PayloadSweep,
        ];
        for model in models {
            let first = TrafficPlan::generate(&config(model)).unwrap();
            let second = TrafficPlan::generate(&config(model)).unwrap();
            assert_eq!(first, second, "{model:?}");
            assert!(
                first
                    .flows
                    .iter()
                    .all(|flow| flow.source != flow.destination),
                "{model:?}"
            );
        }
    }

    #[test]
    fn control_only_and_saturated_data_baselines_are_reproducible() {
        let idle = TrafficPlan::generate(&config(TrafficModel::Idle)).unwrap();
        assert_eq!(idle.offered_useful_bytes, 0);
        let data = TrafficPlan::generate(&config(TrafficModel::AllToAll)).unwrap();
        assert_eq!(data.flows.len(), 16 * 15);
        assert_eq!(
            data.offered_useful_bytes,
            data.flows
                .iter()
                .map(|flow| flow.useful_payload_bytes)
                .sum::<u64>()
        );
        assert_ne!(data.offered_useful_bytes, data.setup_message_bytes);
    }

    #[test]
    fn session_churn_exposes_setup_and_teardown_separately() {
        let plan = TrafficPlan::generate(&config(TrafficModel::SessionChurn)).unwrap();
        assert_eq!(plan.session_setups, 50);
        assert_eq!(plan.session_teardowns, 50);
        assert_eq!(plan.setup_message_bytes, 50 * 176);
    }
}
