extension SimulationState {
    mutating func applyParentCost(_ event: SimulationEvent) {
        parentQualityPulses += 1
        if event.data["suppressed"]?.bool == true { parentSwitchesSuppressed += 1 }
        guard let id = event.data["node"]?.int, var node = nodes[id] else { return }
        if let parent = event.data["new_parent"]?.int { node.parent = parent }
        if event.data["switched"]?.bool == true {
            node.sequence += 1
            lastParentSwitchAtNS[id] = event.timeNS
        }
        nodes[id] = node
        if let edgeID = event.data["old_parent_edge"]?.int, var edge = edges[edgeID] {
            edge.parentCostPPM = event.data["degraded_cost_ppm"]?.int ?? edge.parentCostPPM
            edges[edgeID] = edge
        }
        if let edgeID = event.data["preferred_parent_edge"]?.int, var edge = edges[edgeID] {
            edge.parentCostPPM = event.data["preferred_cost_ppm"]?.int ?? edge.parentCostPPM
            edges[edgeID] = edge
        }
    }
}
