@testable import FIPSDFeature

extension RenderFrame {
    var semanticVisibleManifest: SemanticVisibleManifest {
        SemanticVisibleManifest(
            nodes: nodes.map {
                SemanticNodeMark(
                    id: $0.state.id,
                    active: $0.state.active,
                    root: $0.state.root,
                    parent: $0.state.parent,
                    sequence: $0.state.sequence,
                    transport: $0.state.transportType,
                    mediaZone: $0.state.mediaZone
                )
            },
            links: physicalLinks.map {
                SemanticLinkMark(
                    id: $0.edge.id,
                    from: $0.edge.from,
                    to: $0.edge.to,
                    active: $0.edge.active,
                    sharedMediumGroup: $0.edge.sharedMediumGroup
                )
            },
            parents: parentRelations.map { ($0.child, $0.parent) },
            routes: routes.map {
                let transfer = $0.transfer
                return SemanticRouteMark(
                    id: transfer.id,
                    source: transfer.source,
                    destination: transfer.destination,
                    path: transfer.path,
                    totalBytes: transfer.totalBytes,
                    offeredBytes: transfer.offeredBytes,
                    deliveredBytes: transfer.deliveredBytes,
                    startedAtNS: transfer.startedAtNS,
                    lastDeliveryNS: transfer.lastDeliveryNS
                )
            },
            flights: transmissions.map {
                let transmission = $0.transmission
                return SemanticFlightMark(
                    id: transmission.id,
                    from: transmission.from,
                    to: transmission.to,
                    startNS: transmission.startNS,
                    endNS: transmission.endNS,
                    plane: transmission.plane,
                    progress: $0.progress
                )
            },
            pulses: pulses.map {
                SemanticPulseMark(
                    nodeID: $0.nodeID,
                    kind: $0.kind.rawValue,
                    occurredAtNS: $0.occurredAtNS,
                    progress: $0.progress
                )
            },
            cohorts: cohorts.map {
                SemanticCohortMark(
                    key: Self.cohortKey($0.key),
                    nodeIDs: $0.nodeIDs,
                    activeNodes: $0.activeNodes
                )
            }.sorted { $0.key < $1.key },
            cohortFlights: cohortTransmissions.map {
                SemanticCohortFlightMark(
                    from: Self.cohortKey($0.key.from),
                    to: Self.cohortKey($0.key.to),
                    plane: $0.key.plane,
                    count: $0.count,
                    meanProgress: $0.meanProgress
                )
            }.sorted {
                ($0.from, $0.to, $0.plane)
                    < ($1.from, $1.to, $1.plane)
            }
        )
    }

    private static func cohortKey(_ key: CohortKey) -> String {
        "\(key.root):\(key.depthBand):\(key.transport)"
    }
}
