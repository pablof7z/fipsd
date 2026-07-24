import Foundation

extension WorkbenchModel {
    var selectedNode: NodeState? { selectedNodeID.flatMap { state.nodes[$0] } }

    var selectedIncidentEdges: [Int] {
        guard let node = selectedNodeID else { return [] }
        return state.edges.values
            .filter { $0.from == node || $0.to == node }
            .map(\.id)
            .sorted()
    }

    func targetFirstIncidentEdge() {
        if let edge = selectedIncidentEdges.first { interventionEdgeID = edge }
    }

    func scheduleSelectedNode(active: Bool) {
        guard let node = selectedNodeID else { return }
        configuration.lifecycleEvents.append(LifecycleEvent(
            atSeconds: interventionTime,
            action: active ? "reappear-node" : "disappear-node",
            node: node
        ))
        runConfigured()
    }

    func scheduleLowerRoot() {
        configuration.manualRootTimes.append(interventionTime)
        runConfigured()
    }

    func scheduleSessionRekey() {
        configuration.sessionRekeyTimes.append(interventionTime)
        runConfigured()
    }

    func scheduleLookupStorm() {
        configuration.lookupStorms.append(LookupStormIntervention(
            atSeconds: interventionTime,
            count: configuration.lookupStormCount
        ))
        runConfigured()
    }

    func scheduleTransportClass(restore: Bool) {
        configuration.transportClassEvents.append(TransportClassIntervention(
            atSeconds: interventionTime,
            profile: configuration.interventionTransportProfile,
            restore: restore
        ))
        runConfigured()
    }

    func scheduleParentIntervention(_ action: String) {
        configuration.parentEvents.append(ParentIntervention(
            atSeconds: interventionTime,
            action: action,
            node: selectedNodeID,
            cycles: action == "alternate-parent-quality"
                ? configuration.parentOscillationCycles : 1
        ))
        runConfigured()
    }

    func scheduleAuthenticatedSybils() {
        configuration.sybilEvents.append(SybilIntervention(
            atSeconds: interventionTime,
            count: configuration.sybilCount,
            intervalMilliseconds: configuration.sybilIntervalMilliseconds,
            attachment: configuration.sybilAttachment,
            rootGrinding: configuration.sybilRootGrinding
        ))
        runConfigured()
    }

    func scheduleSelectedPartition(merge: Bool) {
        guard let node = selectedNodeID else { return }
        configuration.networkEvents.append(NetworkIntervention(
            atSeconds: interventionTime,
            action: merge ? "merge-network" : "partition-network",
            nodes: [node]
        ))
        runConfigured()
    }

    func scheduleLinkChange(restore: Bool) {
        configuration.linkEvents.append(LinkIntervention(
            atSeconds: interventionTime,
            action: restore ? "restore-link-conditions" : "set-link-conditions",
            edge: interventionEdgeID,
            bandwidthMbps: restore ? nil : interventionBandwidthMbps,
            latencyMilliseconds: restore ? nil : interventionLatencyMilliseconds,
            jitterMilliseconds: restore ? nil : interventionJitterMilliseconds,
            lossPPM: restore ? nil : interventionLossPPM,
            mtuBytes: restore ? nil : interventionMTUBytes
        ))
        runConfigured()
    }

    private var interventionTime: Double {
        Double(virtualTimeNS) / 1_000_000_000 + 0.1
    }
}
