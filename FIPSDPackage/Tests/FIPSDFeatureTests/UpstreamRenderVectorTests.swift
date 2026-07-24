import Foundation
import Testing
@testable import FIPSDFeature

@Test func pinnedFIPSTopologyVectorsProjectWithoutInventedPrimitives() throws {
    let manifest = try loadUpstreamVectors()
    #expect(manifest.schemaVersion == "experiments.fips.network/render-vectors/v1alpha1")
    #expect(manifest.fipsCommit == "80c956a6fdb85dde1450969a21891c1158e43267")
    #expect(manifest.vectors.count >= 7)
    #expect(manifest.sources.allSatisfy { $0.sha256.count == 64 })

    for vector in manifest.vectors {
        var state = SimulationState()
        for id in 0..<vector.nodeCount {
            state.nodes[id] = NodeState(
                id: id,
                address: String(format: "%032x", id),
                active: true,
                root: id,
                parent: nil,
                sequence: 1
            )
        }
        for (id, endpoints) in vector.edges.enumerated() {
            state.edges[id] = EdgeState(
                id: id,
                from: endpoints[0],
                to: endpoints[1]
            )
        }
        if let path = vector.flowPath {
            for (index, endpoints) in zip(path, path.dropFirst()).enumerated() {
                let flight = Transmission(
                    id: "\(vector.id)-hop-\(index)",
                    from: endpoints.0,
                    to: endpoints.1,
                    startNS: 0,
                    endNS: 100,
                    frameBytes: 512,
                    copy: 0,
                    plane: "data"
                )
                state.transmissions[flight.id] = flight
            }
        }

        let frame = RenderFrame(state: state, virtualTimeNS: 50)

        #expect(frame.reconciliation.isExact, Comment(rawValue: vector.id))
        #expect(frame.nodes.count == vector.nodeCount, Comment(rawValue: vector.id))
        #expect(frame.physicalLinks.count == vector.edges.count, Comment(rawValue: vector.id))
        #expect(
            connectedComponents(nodeCount: vector.nodeCount, edges: vector.edges)
                == vector.expectedComponents,
            Comment(rawValue: vector.id)
        )
        #expect(
            frame.transmissions.count == max(0, (vector.flowPath?.count ?? 1) - 1),
            Comment(rawValue: vector.id)
        )
    }
}

private struct UpstreamVectorManifest: Decodable {
    let fipsCommit: String
    let schemaVersion: String
    let sources: [UpstreamSource]
    let vectors: [UpstreamVector]

    enum CodingKeys: String, CodingKey {
        case fipsCommit = "fips_commit"
        case schemaVersion = "schema_version"
        case sources, vectors
    }
}

private struct UpstreamSource: Decodable {
    let path: String
    let sha256: String
}

private struct UpstreamVector: Decodable {
    let id: String
    let sourcePath: String
    let sourceTest: String
    let fidelity: String
    let nodeCount: Int
    let edges: [[Int]]
    let expectedComponents: Int
    let flowPath: [Int]?

    enum CodingKeys: String, CodingKey {
        case id, fidelity, edges
        case sourcePath = "source_path"
        case sourceTest = "source_test"
        case nodeCount = "node_count"
        case expectedComponents = "expected_components"
        case flowPath = "flow_path"
    }
}

private func loadUpstreamVectors() throws -> UpstreamVectorManifest {
    let sourceURL = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .appendingPathComponent("Resources/upstream-render-vectors.json")
    let url = Bundle.module.url(
        forResource: "upstream-render-vectors",
        withExtension: "json"
    ) ?? sourceURL
    #expect(
        FileManager.default.fileExists(atPath: url.path),
        "upstream vector manifest must be present in the bundle or source tree"
    )
    return try JSONDecoder().decode(
        UpstreamVectorManifest.self,
        from: Data(contentsOf: url)
    )
}

private func connectedComponents(nodeCount: Int, edges: [[Int]]) -> Int {
    var adjacency = Array(repeating: [Int](), count: nodeCount)
    for edge in edges {
        adjacency[edge[0]].append(edge[1])
        adjacency[edge[1]].append(edge[0])
    }
    var visited = Set<Int>()
    var components = 0
    for node in 0..<nodeCount where !visited.contains(node) {
        components += 1
        var stack = [node]
        while let next = stack.popLast() {
            guard visited.insert(next).inserted else { continue }
            stack.append(contentsOf: adjacency[next])
        }
    }
    return components
}
