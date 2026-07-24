import Foundation

extension WorkbenchModel {
    func controlSnapshot(limit requestedLimit: Int) -> JSONValue {
        let limit = min(max(requestedLimit, 0), 1_000)
        let orderedNodes = state.nodes.values.sorted { $0.id < $1.id }
        let orderedEdges = state.edges.values.sorted { $0.id < $1.id }
        let rootCounts = Dictionary(grouping: orderedNodes.filter(\.active), by: \.root)
        let nodes = orderedNodes.prefix(limit).map { node in
            JSONValue.object([
                "id": .integer(Int64(node.id)),
                "active": .bool(node.active),
                "address": .string(node.address),
                "root": .integer(Int64(node.root)),
                "parent": node.parent.map { .integer(Int64($0)) } ?? .null,
                "transport_profile": .string(node.transportProfile),
                "transport_type": .string(node.transportType),
                "bandwidth_bps": .integer(Int64(node.bandwidthBPS)),
                "latency_ns": jsonUInt(node.latencyNS),
                "mtu_bytes": .integer(Int64(node.mtuBytes))
            ])
        }
        let edges = orderedEdges.prefix(limit).map { edge in
            JSONValue.object([
                "id": .integer(Int64(edge.id)),
                "from": .integer(Int64(edge.from)),
                "to": .integer(Int64(edge.to)),
                "active": .bool(edge.active),
                "bandwidth_bps": .integer(Int64(edge.bandwidthBPS)),
                "latency_ns": jsonUInt(edge.latencyNS),
                "jitter_ns": jsonUInt(edge.jitterNS),
                "loss_ppm": .integer(Int64(edge.lossPPM)),
                "mtu_bytes": .integer(Int64(edge.mtuBytes)),
                "queue_bytes": .integer(Int64(edge.queueBytes))
            ])
        }
        let roots = rootCounts.keys.sorted().map {
            JSONValue.object([
                "root_node": .integer(Int64($0)),
                "active_nodes": .integer(Int64(rootCounts[$0]?.count ?? 0))
            ])
        }
        return .object([
            "status": .string(status),
            "error": errorMessage.map(JSONValue.string) ?? .null,
            "is_running": .bool(isRunning),
            "is_playing": .bool(isPlaying),
            "stream_complete": .bool(streamComplete),
            "provider": .string(provider.rawValue),
            "prompt": .string(prompt),
            "virtual_time_ns": jsonUInt(virtualTimeNS),
            "duration_ns": jsonUInt(durationNS),
            "cursor": .integer(Int64(cursor)),
            "event_count": .integer(Int64(events.count)),
            "speed": .number(speed),
            "visualization_mode": .string(visualizationMode.rawValue),
            "run": controlRunSummary(),
            "network": .object([
                "node_count": .integer(Int64(orderedNodes.count)),
                "active_node_count": .integer(Int64(orderedNodes.count(where: \.active))),
                "edge_count": .integer(Int64(orderedEdges.count)),
                "root_groups": .array(roots),
                "nodes": .array(nodes),
                "edges": .array(edges),
                "sample_limit": .integer(Int64(limit)),
                "truncated": .bool(
                    orderedNodes.count > limit || orderedEdges.count > limit
                )
            ]),
            "traffic": controlTrafficSummary(),
            "last_event": controlEvent(state.lastEvent),
            "configuration": controlConfiguration()
        ])
    }

    func controlAnalysis() -> JSONValue {
        .object([
            "fidelity": .string(analysis.fidelity),
            "represented_nodes": jsonUInt(analysis.representedNodes),
            "event_count": .integer(Int64(analysis.eventCount)),
            "ledger_entries": .integer(Int64(analysis.ledgerEntries)),
            "longest_causal_chain": .integer(Int64(analysis.longestCausalChain)),
            "stages": .array(analysis.stages.prefix(20).map {
                .object([
                    "stage": .string($0.stage),
                    "count": jsonUInt($0.count),
                    "entries": .integer(Int64($0.entries))
                ])
            }),
            "top_edges": .array(analysis.topEdges.prefix(20).map {
                .object([
                    "from": .integer(Int64($0.from)),
                    "to": .integer(Int64($0.to)),
                    "frames": jsonUInt($0.frames),
                    "bytes": jsonUInt($0.bytes),
                    "peak_queue_bytes": jsonUInt($0.peakQueueBytes)
                ])
            }),
            "root_impacts": .array(analysis.rootImpacts.prefix(20).map {
                .object([
                    "event_id": .string($0.id),
                    "root": .string($0.root),
                    "arrival_ns": jsonUInt($0.arrivalNS),
                    "recorded_consequences": .integer(Int64($0.consequences))
                ])
            }),
            "diagnostics": .object([
                "anomaly_node_ids": .array(
                    analysis.anomalyNodeIDs.sorted().map { .integer(Int64($0)) }
                )
            ]),
            "evidence_path": evidenceURL.map { .string($0.path) } ?? .null
        ])
    }

    func controlExplanation(focus: String?) -> JSONValue {
        let active = state.nodes.values.count(where: \.active)
        let roots = Set(state.nodes.values.filter(\.active).map(\.root))
        let transfer = state.applicationTransfers.values.sorted { $0.id < $1.id }.first
        var sentences = [
            "At \(timeLabel), \(active) of \(state.nodes.count) nodes are active.",
            roots.count == 1
                ? "Active nodes currently agree on root #\(roots.first ?? 0)."
                : "Active nodes currently report \(roots.count) different roots."
        ]
        if let event = state.lastEvent {
            sentences.append("The last rendered event is \(event.kind) at \(format(event.timeNS)).")
        }
        if let transfer {
            sentences.append(
                "\(transfer.id) follows \(transfer.routeLabel) and has delivered "
                    + "\(transfer.deliveredBytes) of \(transfer.totalBytes) useful bytes."
            )
        }
        if state.flowsRejected > 0 {
            sentences.append("\(state.flowsRejected) payload flows have been rejected.")
        }
        if !summary.outcome.isEmpty {
            sentences.append("The recorded run outcome is \(summary.outcome).")
        }
        if let focus, !focus.isEmpty {
            sentences.append("Requested focus: \(focus).")
        }
        return .object([
            "explanation": .string(sentences.joined(separator: " ")),
            "state": controlSnapshot(limit: 50),
            "analysis": controlAnalysis()
        ])
    }

    private func controlRunSummary() -> JSONValue {
        .object([
            "run_id": .string(summary.runID),
            "artifact_id": .string(summary.artifactID),
            "outcome": .string(summary.outcome),
            "final_root": .string(summary.finalRoot),
            "quiescence_ns": jsonUInt(summary.quiescenceNS),
            "fidelity": .string(summary.fidelity),
            "evidence_path": evidenceURL.map { .string($0.path) } ?? .null
        ])
    }

    private func controlTrafficSummary() -> JSONValue {
        let transfers = state.applicationTransfers.values.sorted { $0.id < $1.id }.map {
            JSONValue.object([
                "id": .string($0.id),
                "source": .integer(Int64($0.source)),
                "destination": .integer(Int64($0.destination)),
                "path": .array($0.path.map { .integer(Int64($0)) }),
                "total_bytes": .integer(Int64($0.totalBytes)),
                "offered_bytes": .integer(Int64($0.offeredBytes)),
                "delivered_bytes": .integer(Int64($0.deliveredBytes)),
                "progress": .number($0.progress)
            ])
        }
        return .object([
            "flows_delivered": .integer(Int64(state.flowsDelivered)),
            "flows_rejected": .integer(Int64(state.flowsRejected)),
            "useful_bytes_delivered": .integer(Int64(state.usefulBytesDelivered)),
            "transmitted_bytes": .integer(Int64(state.transmittedBytes)),
            "peak_queue_bytes": .integer(Int64(state.queuePeakBytes)),
            "transfers": .array(transfers)
        ])
    }

    private func controlEvent(_ event: SimulationEvent?) -> JSONValue {
        guard let event else { return .null }
        return .object([
            "id": .string(event.id),
            "kind": .string(event.kind),
            "virtual_time_ns": jsonUInt(event.timeNS),
            "causal_parent": event.causalParent.map(JSONValue.string) ?? .null,
            "data": .object(event.data)
        ])
    }

    private func format(_ time: UInt64) -> String {
        String(format: "%.3f s", Double(time) / 1e9)
    }
}

private func jsonUInt(_ value: UInt64) -> JSONValue {
    .integer(Int64(clamping: value))
}
