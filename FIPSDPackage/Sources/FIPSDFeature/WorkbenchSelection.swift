import Foundation

extension WorkbenchModel {
    var selectedIncidentEdge: EdgeState? {
        guard let edge = selectedIncidentEdges.first else { return nil }
        return state.edges[edge]
    }

    var selectedSharedMediumCapacityBPS: Int? {
        let capacities = selectedIncidentEdges.compactMap { id -> Int? in
            guard let edge = state.edges[id], edge.sharedMediumGroup != nil else { return nil }
            return edge.bandwidthBPS
        }
        return capacities.min()
    }

    var selectedEffectiveBandwidthBPS: Int? {
        selectedIncidentEdges.compactMap { state.edges[$0]?.bandwidthBPS }.min()
    }
}
