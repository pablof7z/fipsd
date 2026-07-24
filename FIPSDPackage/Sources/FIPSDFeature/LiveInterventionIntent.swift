import Foundation

struct LiveInterventionIntent: Equatable, Sendable {
    let spottyEndpoints: (Int, Int)?
    let introducesLowerRoot: Bool

    static func == (lhs: Self, rhs: Self) -> Bool {
        lhs.spottyEndpoints?.0 == rhs.spottyEndpoints?.0
            && lhs.spottyEndpoints?.1 == rhs.spottyEndpoints?.1
            && lhs.introducesLowerRoot == rhs.introducesLowerRoot
    }

    static func parse(_ prompt: String) -> Self? {
        let text = prompt.lowercased()
        let spotty = text.contains("spotty")
            || text.contains("unstable")
            || text.contains("unreliable")
            || text.contains("degrade")
        let endpoints = spotty ? parseEndpoints(text) : nil
        let lowerRoot = text.contains("root")
            && (text.contains("new node") || text.contains("add") || text.contains("introduce"))
        guard endpoints != nil || lowerRoot else { return nil }
        return Self(spottyEndpoints: endpoints, introducesLowerRoot: lowerRoot)
    }

    func applying(to data: Data, at timeNS: UInt64, attachmentNode: Int = 0) throws -> Data {
        guard var campaign = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return data
        }
        var events = campaign["events"] as? [[String: Any]] ?? []
        let previousManualArrivals = events.filter {
            ["introduce-lower-root-node", "introduce-node"].contains(
                $0["action"] as? String
            )
        }.count
        if let endpoints = spottyEndpoints {
            let edge = try edgeIndex(between: endpoints, campaign: campaign)
            events.append([
                "id": uniqueID("prompt-link", in: events),
                "at": "\(timeNS)ns",
                "action": "set-link-conditions",
                "target": edge,
                "parameters": [
                    "bandwidth_bps": 1_000_000,
                    "latency": "200ms",
                    "jitter": "100ms",
                    "loss_ppm": 100_000
                ]
            ])
        }
        if introducesLowerRoot {
            events.append([
                "id": uniqueID("prompt-root", in: events),
                "at": "\(timeNS)ns",
                "action": "introduce-lower-root-node",
                "target": attachmentNode
            ])
        }
        campaign["events"] = events
        let currentManualArrivals = events.filter {
            ["introduce-lower-root-node", "introduce-node"].contains(
                $0["action"] as? String
            )
        }.count
        CampaignIdentityBudget.adjustScale(
            in: &campaign,
            previousManualArrivals: previousManualArrivals,
            currentManualArrivals: currentManualArrivals,
            newAttachmentTargets: introducesLowerRoot ? [[attachmentNode]] : []
        )
        CampaignIdentityBudget.reserveManualArrivals(in: &campaign)
        return try JSONSerialization.data(
            withJSONObject: campaign,
            options: [.prettyPrinted, .sortedKeys]
        )
    }

    private func edgeIndex(
        between endpoints: (Int, Int),
        campaign: [String: Any]
    ) throws -> Int {
        let topology = campaign["topology"] as? [String: Any]
        let edges = topology?["explicit_edges"] as? [[Int]] ?? []
        guard let index = edges.firstIndex(where: { edge in
            edge.count == 2 && Set(edge) == Set([endpoints.0, endpoints.1])
        }) else {
            throw LiveInterventionError.edgeNotFound(endpoints.0, endpoints.1)
        }
        return index
    }

    private func uniqueID(_ prefix: String, in events: [[String: Any]]) -> String {
        let used = Set(events.compactMap { $0["id"] as? String })
        var index = 0
        while used.contains("\(prefix)-\(index)") { index += 1 }
        return "\(prefix)-\(index)"
    }

    private static func parseEndpoints(_ text: String) -> (Int, Int)? {
        let pattern = #"\bbetween\s+(?:node\s+)?([a-z]|\d+)\s+and\s+(?:node\s+)?([a-z]|\d+)\b"#
        guard let expression = try? NSRegularExpression(pattern: pattern),
              let match = expression.firstMatch(
                in: text,
                range: NSRange(text.startIndex..., in: text)
              ),
              let firstRange = Range(match.range(at: 1), in: text),
              let secondRange = Range(match.range(at: 2), in: text),
              let first = nodeID(String(text[firstRange])),
              let second = nodeID(String(text[secondRange])),
              first != second else { return nil }
        return (first, second)
    }

    private static func nodeID(_ token: String) -> Int? {
        if let number = Int(token) { return number }
        guard let scalar = token.unicodeScalars.first, token.unicodeScalars.count == 1 else {
            return nil
        }
        return Int(scalar.value) - Int(UnicodeScalar("a").value)
    }
}

enum LiveInterventionError: LocalizedError {
    case edgeNotFound(Int, Int)

    var errorDescription: String? {
        switch self {
        case let .edgeNotFound(first, second):
            "The active topology has no direct edge between node \(first) and node \(second)."
        }
    }
}
