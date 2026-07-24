use super::*;

impl Simulation {
    pub(super) fn fidelity_approximations(
        &self,
        routed_traffic: bool,
        graph_recovery: bool,
    ) -> Vec<Approximation> {
        let mut result = Vec::new();
        if self.transports.is_mixed() {
            result.push(Approximation {
                method: "abstract-endpoint-media-profile-v1".to_owned(),
                parameters: [("assignment".to_owned(), "seeded-weighted-per-node".to_owned())]
                    .into_iter()
                    .collect(),
                validated_range: "configured endpoint bandwidth, latency, loss, MTU, queue, and overhead bounds"
                    .to_owned(),
                uncertainty: "access-media values are abstract inputs, not measurements of Tor, Bluetooth, Wi-Fi, or Ethernet implementations"
                    .to_owned(),
            });
        }
        if routed_traffic {
            result.push(Approximation {
                method: "routed-synthetic-session-data-v1".to_owned(),
                parameters: [("routing".to_owned(), "stable-shortest-active-path".to_owned())]
                    .into_iter()
                    .collect(),
                validated_range: "individual-node topologies within the declared 100000-flow budget"
                    .to_owned(),
                uncertainty: "session-data framing is semantically modeled; per-hop queueing, loss, MTU, and delivery follow configured link inputs"
                    .to_owned(),
            });
        }
        if self.traffic.as_ref().is_some_and(|runtime| {
            runtime
                .plan
                .flows
                .iter()
                .any(|flow| matches!(&flow.shape, crate::FlowShape::ApplicationTransfer { .. }))
        }) {
            result.push(Approximation {
                method: "aggregated-reliable-stream-packetization-v1".to_owned(),
                parameters: [
                    ("routing".to_owned(), "stable-shortest-active-path".to_owned()),
                    (
                        "visualization".to_owned(),
                        "bounded-byte-range-events".to_owned(),
                    ),
                ]
                .into_iter()
                .collect(),
                validated_range:
                    "explicit individual-node transfers within the 100000 visualization-chunk budget"
                        .to_owned(),
                uncertainty: "packet counts, MTU overhead, queue occupancy, bandwidth serialization, and projected retransmissions are modeled per hop; individual packets are aggregated into visible byte ranges"
                    .to_owned(),
            });
        }
        if graph_recovery {
            result.push(Approximation {
                method: "graph-native-lookup-session-v1".to_owned(),
                parameters: [
                    ("routing".to_owned(), "stable-shortest-active-path".to_owned()),
                    ("rekey".to_owned(), "operation-counted-no-wire-frame".to_owned()),
                ]
                .into_iter()
                .collect(),
                validated_range: "mixed-profile individual-node campaigns within the routed-flow budget"
                    .to_owned(),
                uncertainty: "lookup/setup/ack sizes are executable-codec-derived; rekey is operation-counted and session cryptography is not byte-executed"
                    .to_owned(),
            });
        }
        if !self.config.parent_costs.is_empty() {
            result.push(Approximation {
                method: "modeled-mmp-link-cost-snapshot-v1".to_owned(),
                parameters: [
                    ("cost-unit".to_owned(), "millionths".to_owned()),
                    (
                        "selection".to_owned(),
                        "peer-depth-plus-link-cost".to_owned(),
                    ),
                ]
                .into_iter()
                .collect(),
                validated_range: "positive authored costs through the deterministic individual-node parent-selection path"
                    .to_owned(),
                uncertainty: "cost snapshots are authored MMP inputs; MMP report generation and SRTT/ETX estimation are not executed"
                    .to_owned(),
            });
        }
        if !self.config.sybils.is_empty() {
            result.push(Approximation {
                method: "authenticated-sybil-admission-v1".to_owned(),
                parameters: [
                    ("trust".to_owned(), "authenticated-protocol-valid".to_owned()),
                    ("crypto".to_owned(), "operation-counted".to_owned()),
                ]
                .into_iter()
                .collect(),
                validated_range: "bounded individual identities and stable attachment policies"
                    .to_owned(),
                uncertainty: "identity authentication is operation-counted; handshake cryptography and admission-policy wall time are not byte-executed"
                    .to_owned(),
            });
        }
        if self
            .bloom
            .as_ref()
            .is_some_and(|runtime| runtime.mode() == crate::BloomMode::Occupancy)
        {
            result.push(Approximation {
                method: "seeded-bloom-occupancy-v1".to_owned(),
                parameters: [
                    ("bits".to_owned(), "8192".to_owned()),
                    ("hash-count".to_owned(), "5".to_owned()),
                ]
                .into_iter()
                .collect(),
                validated_range: "seeded ensembles through the configured FPR cap".to_owned(),
                uncertainty: "absent-item membership is a deterministic draw at modeled FPR"
                    .to_owned(),
            });
        }
        result
    }
}
