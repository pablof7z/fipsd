import Foundation

extension WorkbenchModel {
    var canAddExplicitEdge: Bool {
        let nodes = configuration.nodes
        guard explicitEdgeFrom >= 0, explicitEdgeTo >= 0,
              explicitEdgeFrom < nodes, explicitEdgeTo < nodes,
              explicitEdgeFrom != explicitEdgeTo else { return false }
        let candidate = ManualEdge(explicitEdgeFrom, explicitEdgeTo)
        return !configuration.explicitEdges.contains(candidate)
    }

    func captureRenderedTopology() {
        let nodeCount = state.nodes.count
        guard nodeCount >= 2, !state.edges.isEmpty else { return }
        configuration.nodes = nodeCount
        configuration.topology = "explicit"
        configuration.explicitEdges = state.edges.values
            .map { ManualEdge($0.from, $0.to) }
            .sorted { ($0.from, $0.to) < ($1.from, $1.to) }
        status = "Captured \(configuration.explicitEdges.count) rendered links for editing."
    }

    func addExplicitEdge() {
        guard canAddExplicitEdge else { return }
        configuration.explicitEdges.append(ManualEdge(explicitEdgeFrom, explicitEdgeTo))
        configuration.explicitEdges.sort { ($0.from, $0.to) < ($1.from, $1.to) }
    }

    func removeExplicitEdge(at index: Int) {
        guard configuration.explicitEdges.indices.contains(index) else { return }
        configuration.explicitEdges.remove(at: index)
    }

    func resetExplicitTopologyToChain() {
        configuration.topology = "explicit"
        configuration.explicitEdges = (1..<max(2, configuration.nodes)).map {
            ManualEdge($0 - 1, $0)
        }
    }
}
