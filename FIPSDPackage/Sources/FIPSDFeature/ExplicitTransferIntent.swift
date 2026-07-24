import Foundation

struct ExplicitTransferIntent: Equatable, Sendable {
    let nodeCount: Int
    let totalBytes: Int
    let preserveRequestedConnectivity: Bool

    static func parse(_ prompt: String) -> Self? {
        let text = prompt.lowercased()
        guard text.contains("download")
                || text.contains("transfer")
                || text.contains("send"),
              let nodeCount = parseNodeCount(text),
              nodeCount >= 3,
              let totalBytes = parseBytes(text) else { return nil }
        let connectivityTerms = [
            "random bandwidth", "randomize", "randomised", "randomized",
            "mixed transport", "bluetooth", "wifi", "wi-fi", "tor", "nym"
        ]
        return Self(
            nodeCount: nodeCount,
            totalBytes: totalBytes,
            preserveRequestedConnectivity: connectivityTerms.contains { text.contains($0) }
        )
    }

    func applying(to data: Data) throws -> Data {
        guard var campaign = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return data
        }
        campaign["scale"] = ["nodes": nodeCount]
        campaign["topology"] = [
            "generator": "explicit",
            "average_degree": 2,
            "explicit_edges": (0..<(nodeCount - 1)).map { [$0, $0 + 1] }
        ]
        if var identities = campaign["identities"] as? [String: Any],
           var arrivals = identities["arrivals"] as? [String: Any] {
            arrivals["count"] = 0
            if var budget = arrivals["attacker_budget"] as? [String: Any] {
                budget["operations"] = 0
                budget["identities"] = 0
                arrivals["attacker_budget"] = budget
            }
            identities["arrivals"] = arrivals
            campaign["identities"] = identities
        }
        campaign["traffic"] = [
            "model": "explicit-transfers",
            "rate_bps": 100_000_000,
            "transfers": [[
                "id": "requested-download",
                "source": 0,
                "destination": nodeCount - 1,
                "total_bytes": totalBytes,
                "visualization_chunk_bytes": min(1_000_000, totalBytes),
                "start": "1s"
            ]]
        ]
        campaign["events"] = []
        campaign["objectives"] = ["maximize": ["useful_payload_delivered"]]
        if !preserveRequestedConnectivity {
            campaign["links"] = [
                "bandwidth_bps": 100_000_000,
                "latency": "1ms",
                "jitter": "0ms",
                "loss_ppm": 0,
                "duplication_ppm": 0,
                "mtu_bytes": 1_500,
                "ordering": "stream",
                "queue_bytes": 8_000_000,
                "drop_policy": "tail-drop"
            ]
            campaign["transports"] = [
                "assignment": "all-ethernet",
                "profiles": [[
                    "name": "ethernet",
                    "type": "ethernet",
                    "bandwidth_bps": 100_000_000,
                    "latency": "1ms",
                    "jitter": "0ms",
                    "loss_ppm": 0,
                    "mtu_bytes": 1_500,
                    "queue_bytes": 8_000_000,
                    "weight": 1
                ]]
            ]
        }
        if var metadata = campaign["metadata"] as? [String: Any] {
            metadata["description"] =
                "\(totalBytes)-byte transfer across a \(nodeCount)-node explicit chain"
            campaign["metadata"] = metadata
        }
        return try JSONSerialization.data(
            withJSONObject: campaign,
            options: [.prettyPrinted, .sortedKeys]
        )
    }

    private static func parseNodeCount(_ text: String) -> Int? {
        let words = [
            "two": 2, "three": 3, "four": 4, "five": 5, "six": 6,
            "seven": 7, "eight": 8, "nine": 9, "ten": 10
        ]
        if let match = firstMatch(#"\b(\d+)\s+nodes?\b"#, in: text),
           let count = Int(match) {
            return count
        }
        return words.first { word, _ in
            text.range(of: #"\b\#(word)\s+nodes?\b"#, options: .regularExpression) != nil
        }?.value
    }

    private static func parseBytes(_ text: String) -> Int? {
        let pattern =
            #"\b(\d+(?:\.\d+)?)\s*(kb|mb|gb|kib|mib|gib|kilobytes?|megabytes?|gigabytes?)\b"#
        guard let match = try? NSRegularExpression(pattern: pattern).firstMatch(
            in: text,
            range: NSRange(text.startIndex..., in: text)
        ), match.numberOfRanges == 3,
        let amountRange = Range(match.range(at: 1), in: text),
        let unitRange = Range(match.range(at: 2), in: text),
        let amount = Double(text[amountRange]) else { return nil }
        let unit = String(text[unitRange])
        let multiplier: Double = switch unit {
        case "kb", "kilobyte", "kilobytes": 1_000
        case "kib": 1_024
        case "mb", "megabyte", "megabytes": 1_000_000
        case "mib": 1_048_576
        case "gb", "gigabyte", "gigabytes": 1_000_000_000
        case "gib": 1_073_741_824
        default: 1
        }
        let bytes = amount * multiplier
        guard bytes >= 1, bytes <= Double(Int.max) else { return nil }
        return Int(bytes.rounded())
    }

    private static func firstMatch(_ pattern: String, in text: String) -> String? {
        guard let match = try? NSRegularExpression(pattern: pattern).firstMatch(
            in: text,
            range: NSRange(text.startIndex..., in: text)
        ), match.numberOfRanges > 1,
        let range = Range(match.range(at: 1), in: text) else { return nil }
        return String(text[range])
    }
}
