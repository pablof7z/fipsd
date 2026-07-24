import Foundation

enum EngineClientError: LocalizedError {
    case executableMissing
    case failed(Int32, String)
    case invalidLine(String)

    var errorDescription: String? {
        switch self {
        case .executableMissing:
            "The bundled simulation engine is missing. Run scripts/prepare-macos-engine.sh."
        case let .failed(status, message):
            "Simulation engine exited with status \(status): \(message)"
        case let .invalidLine(line):
            "The engine emitted invalid event JSON: \(line.prefix(160))"
        }
    }
}

struct EngineClient: Sendable {
    let executableURL: URL

    static func bundled() throws -> Self {
        guard let url = Bundle.module.url(
            forResource: "fips-wind-tunnel",
            withExtension: nil,
            subdirectory: "Resources/bin"
        ) ?? Bundle.module.url(
            forResource: "fips-wind-tunnel",
            withExtension: nil,
            subdirectory: "bin"
        ) else { throw EngineClientError.executableMissing }
        return Self(executableURL: url)
    }

    func stream(
        campaignURL: URL,
        evidenceURL: URL,
        receive: @escaping @MainActor @Sendable ([StreamEnvelope]) -> Void
    ) async throws {
        let managed = ManagedProcess()
        let process = managed.process
        let output = Pipe()
        let errors = Pipe()
        process.executableURL = executableURL
        process.arguments = [
            "stream", campaignURL.path,
            "--output", evidenceURL.path
        ]
        process.standardOutput = output
        process.standardError = errors
        try await withTaskCancellationHandler {
            try process.run()
            let decoder = JSONDecoder()
            var batch: [StreamEnvelope] = []
            batch.reserveCapacity(256)
            for try await line in output.fileHandleForReading.bytes.lines {
                try Task.checkCancellation()
                guard let data = line.data(using: .utf8),
                      let envelope = try? decoder.decode(StreamEnvelope.self, from: data) else {
                    managed.terminate()
                    throw EngineClientError.invalidLine(line)
                }
                batch.append(envelope)
                if batch.count == 256 {
                    await receive(batch)
                    batch.removeAll(keepingCapacity: true)
                }
            }
            if !batch.isEmpty { await receive(batch) }
            process.waitUntilExit()
            try Task.checkCancellation()
            if process.terminationStatus != 0 {
                let data = try errors.fileHandleForReading.readToEnd() ?? Data()
                let message = String(decoding: data, as: UTF8.self)
                throw EngineClientError.failed(process.terminationStatus, message)
            }
        } onCancel: {
            managed.terminate()
        }
    }

    func scaleRun(campaignURL: URL, evidenceURL: URL) async throws {
        try await run(arguments: [
            "scale", "run", campaignURL.path, "--output", evidenceURL.path
        ])
    }

    func scaleBillionDemo(campaignURL: URL, outputURL: URL) async throws {
        try await run(arguments: [
            "scale", "billion-demo", campaignURL.path, "--output", outputURL.path
        ])
    }

    func run(arguments: [String]) async throws {
        let process = Process()
        let errors = Pipe()
        process.executableURL = executableURL
        process.arguments = arguments
        process.standardOutput = Pipe()
        process.standardError = errors
        try process.run()
        process.waitUntilExit()
        guard process.terminationStatus == 0 else {
            let data = try errors.fileHandleForReading.readToEnd() ?? Data()
            throw EngineClientError.failed(
                process.terminationStatus, String(decoding: data, as: UTF8.self)
            )
        }
    }
}

private final class ManagedProcess: @unchecked Sendable {
    let process = Process()

    func terminate() {
        if process.isRunning { process.terminate() }
    }
}
