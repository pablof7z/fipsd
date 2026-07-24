import Foundation

struct BridgeReplacementIntent: Equatable, Sendable {
    let source: Int
    let destination: Int
    let bridge: Int
    let removalDelayNS: UInt64

    static func parse(_ prompt: String, renderedState: Data) -> Self? {
        let text = prompt.lowercased()
        let addsNode = text.contains("new node")
            && (text.contains("join") || text.contains("connect"))
        let removesBridge = text.contains("bridge")
            && (text.contains("disappear") || text.contains("leave") || text.contains("remove"))
        guard addsNode, removesBridge,
              let context = try? JSONSerialization.jsonObject(with: renderedState)
                as? [String: Any],
              let transfers = context["application_transfers"] as? [[String: Any]],
              let transfer = transfers.first,
              let source = transfer["source"] as? Int,
              let destination = transfer["destination"] as? Int,
              let path = transfer["path"] as? [Int],
              let bridge = path.dropFirst().dropLast().first else {
            return nil
        }
        return Self(
            source: source,
            destination: destination,
            bridge: bridge,
            removalDelayNS: parseDelayNS(text) ?? 10_000_000_000
        )
    }

    func applying(
        to campaign: Data,
        at timeNS: UInt64,
        realizedArrivals: Int
    ) throws -> Data {
        let suffix = String(timeNS)
        let amendment = try JSONSerialization.data(withJSONObject: [
            "events": [
                [
                    "id": "prompt-replacement-bridge-\(suffix)",
                    "at": "\(timeNS)ns",
                    "action": "introduce-node",
                    "parameters": ["attachments": [source, destination]]
                ],
                [
                    "id": "prompt-old-bridge-leaves-\(suffix)",
                    "at": "\(timeNS.saturatingAdd(removalDelayNS))ns",
                    "action": "disappear-node",
                    "target": bridge
                ]
            ]
        ])
        return try CampaignAmendment.applying(
            amendment,
            to: campaign,
            noEarlierThan: timeNS,
            realizedArrivals: realizedArrivals,
            attachmentNode: source
        )
    }

    private static func parseDelayNS(_ text: String) -> UInt64? {
        let pattern =
            #"\bafter\s+([0-9]+(?:\.[0-9]+)?)\s*(ms|milliseconds?|s|seconds?|m|minutes?)\b"#
        guard let expression = try? NSRegularExpression(pattern: pattern),
              let match = expression.firstMatch(
                in: text,
                range: NSRange(text.startIndex..., in: text)
              ),
              let amountRange = Range(match.range(at: 1), in: text),
              let unitRange = Range(match.range(at: 2), in: text),
              let amount = Double(text[amountRange]) else {
            return nil
        }
        let unit = text[unitRange]
        let multiplier: Double
        if unit.hasPrefix("ms") || unit.hasPrefix("millisecond") {
            multiplier = 1_000_000
        } else if unit == "m" || unit.hasPrefix("minute") {
            multiplier = 60_000_000_000
        } else {
            multiplier = 1_000_000_000
        }
        return UInt64((amount * multiplier).rounded())
    }
}
