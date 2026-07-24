import Foundation

final class RenderFrameLogWriter: @unchecked Sendable {
    let outputURL: URL
    private let queue = DispatchQueue(label: "network.fips.render-frame-log")
    private var writeFailure: Error?

    init(directory: URL) throws {
        try FileManager.default.createDirectory(
            at: directory,
            withIntermediateDirectories: true
        )
        outputURL = directory.appendingPathComponent("render-frames.v1.jsonl")
        try Data().write(to: outputURL, options: .atomic)
    }

    func append(_ record: RenderFrameEvidence) throws {
        queue.async { [self] in
            guard writeFailure == nil else { return }
            do {
                let encoder = JSONEncoder()
                encoder.keyEncodingStrategy = .convertToSnakeCase
                encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
                var line = try encoder.encode(record)
                line.append(0x0A)
                let handle = try FileHandle(forWritingTo: outputURL)
                try handle.seekToEnd()
                try handle.write(contentsOf: line)
                try handle.close()
            } catch {
                writeFailure = error
            }
        }
    }

    func flush() async throws {
        try await withCheckedThrowingContinuation {
            (continuation: CheckedContinuation<Void, Error>) in
            queue.async { [self] in
                if let writeFailure {
                    continuation.resume(throwing: writeFailure)
                } else {
                    continuation.resume()
                }
            }
        }
    }
}
