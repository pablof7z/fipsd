import Foundation
import Network

enum ControlClientError: LocalizedError {
    case appUnavailable
    case invalidEndpoint
    case connectionFailed(String)
    case timeout
    case invalidResponse
    case appError(String)
    case appPathMissing

    var errorDescription: String? {
        switch self {
        case .appUnavailable:
            "FIPS Wind Tunnel is not running. Call wind_tunnel_launch first."
        case .invalidEndpoint:
            "The FIPS Wind Tunnel control endpoint is invalid."
        case let .connectionFailed(message):
            "Could not connect to FIPS Wind Tunnel: \(message)"
        case .timeout:
            "FIPS Wind Tunnel did not answer the control request in time."
        case .invalidResponse:
            "FIPS Wind Tunnel returned an invalid control response."
        case let .appError(message):
            message
        case .appPathMissing:
            "Set FIPS_WIND_TUNNEL_APP or run the MCP installer with --app."
        }
    }
}

final class AppControlClient: @unchecked Sendable {
    private struct Endpoint: Decodable {
        let port: UInt16
        let token: String
    }

    private struct Response: Decodable {
        let ok: Bool
        let result: AnyJSON?
        let error: String?
    }

    private let queue = DispatchQueue(label: "io.f7z.fipsd.mcp.client")
    private var connection: NWConnection?
    private var connectionPort: UInt16?

    func call(
        command: String,
        arguments: [String: Any] = [:],
        timeout: TimeInterval = 10
    ) throws -> Any {
        var lastError: Error?
        for attempt in 0..<25 {
            do {
                return try callOnce(
                    command: command,
                    arguments: arguments,
                    timeout: timeout
                )
            } catch let error as ControlClientError {
                guard case .connectionFailed = error, attempt < 24 else { throw error }
                lastError = error
                Thread.sleep(forTimeInterval: 0.1)
            }
        }
        throw lastError ?? ControlClientError.appUnavailable
    }

    private func callOnce(
        command: String,
        arguments: [String: Any],
        timeout: TimeInterval
    ) throws -> Any {
        let endpoint = try loadEndpoint()
        let id = UUID().uuidString
        let request: [String: Any] = [
            "id": id,
            "token": endpoint.token,
            "command": command,
            "arguments": arguments
        ]
        var payload = try JSONSerialization.data(withJSONObject: request)
        payload.append(0x0A)
        let data = try exchange(
            payload,
            port: endpoint.port,
            timeout: timeout
        )
        guard let response = try? JSONDecoder().decode(Response.self, from: data) else {
            throw ControlClientError.invalidResponse
        }
        guard response.ok else {
            throw ControlClientError.appError(response.error ?? "Unknown app control error.")
        }
        return response.result?.value ?? [:]
    }

    func launch() throws -> Any {
        if let state = try? call(command: "get_state", arguments: ["limit": 0]) {
            return state
        }
        let path = try configuredAppPath()
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/open")
        process.arguments = [path]
        try process.run()
        process.waitUntilExit()
        guard process.terminationStatus == 0 else {
            throw ControlClientError.connectionFailed("open exited \(process.terminationStatus)")
        }
        let deadline = Date().addingTimeInterval(15)
        repeat {
            Thread.sleep(forTimeInterval: 0.2)
            if let state = try? call(command: "get_state", arguments: ["limit": 0]) {
                return state
            }
        } while Date() < deadline
        throw ControlClientError.appUnavailable
    }

    private func exchange(
        _ payload: Data,
        port: UInt16,
        timeout: TimeInterval
    ) throws -> Data {
        let connection = try connected(to: port, timeout: timeout)
        let exchange = ExchangeState()
        connection.send(
            content: payload,
            contentContext: .defaultMessage,
            isComplete: false,
            completion: .contentProcessed { [weak self] error in
                if let error {
                    exchange.failure = error
                    exchange.completed.signal()
                } else {
                    self?.receive(connection, state: exchange)
                }
            }
        )
        guard exchange.completed.wait(timeout: .now() + timeout) == .success else {
            resetConnection()
            throw ControlClientError.timeout
        }
        if let failure = exchange.failure {
            resetConnection()
            throw ControlClientError.connectionFailed(failure.localizedDescription)
        }
        guard let newline = exchange.result.firstIndex(of: 0x0A) else {
            resetConnection()
            throw ControlClientError.invalidResponse
        }
        return Data(exchange.result[..<newline])
    }

    private func connected(
        to port: UInt16,
        timeout: TimeInterval
    ) throws -> NWConnection {
        if let connection, connectionPort == port { return connection }
        resetConnection()
        guard let endpointPort = NWEndpoint.Port(rawValue: port) else {
            throw ControlClientError.invalidEndpoint
        }
        let connection = NWConnection(
            host: NWEndpoint.Host("127.0.0.1"),
            port: endpointPort,
            using: .tcp
        )
        let readiness = ExchangeState()
        connection.stateUpdateHandler = { [weak self] connectionState in
            switch connectionState {
            case .ready:
                readiness.completed.signal()
            case let .failed(error):
                readiness.failure = error
                readiness.completed.signal()
                self?.resetConnection()
            case .cancelled:
                if readiness.failure == nil {
                    readiness.completed.signal()
                }
            default:
                break
            }
        }
        connection.start(queue: queue)
        guard readiness.completed.wait(timeout: .now() + timeout) == .success else {
            connection.cancel()
            throw ControlClientError.timeout
        }
        if let failure = readiness.failure {
            connection.cancel()
            throw ControlClientError.connectionFailed(failure.localizedDescription)
        }
        self.connection = connection
        connectionPort = port
        return connection
    }

    private func receive(_ connection: NWConnection, state: ExchangeState) {
        connection.receive(
            minimumIncompleteLength: 1,
            maximumLength: 1_048_576
        ) { [weak self] data, _, complete, error in
            if let data { state.result.append(data) }
            if let error {
                state.failure = error
                state.completed.signal()
            } else if state.result.contains(0x0A) || complete {
                state.completed.signal()
            } else if let self {
                self.receive(connection, state: state)
            }
        }
    }

    private func resetConnection() {
        connection?.cancel()
        connection = nil
        connectionPort = nil
    }

    private func loadEndpoint() throws -> Endpoint {
        let url = Self.endpointURL
        guard let data = try? Data(contentsOf: url) else {
            throw ControlClientError.appUnavailable
        }
        guard let endpoint = try? JSONDecoder().decode(Endpoint.self, from: data) else {
            throw ControlClientError.invalidEndpoint
        }
        return endpoint
    }

    private func configuredAppPath() throws -> String {
        if let value = ProcessInfo.processInfo.environment["FIPS_WIND_TUNNEL_APP"],
           !value.isEmpty {
            return value
        }
        let url = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".config/fips-wind-tunnel/app-path")
        guard let value = try? String(contentsOf: url, encoding: .utf8)
            .trimmingCharacters(in: .whitespacesAndNewlines),
              !value.isEmpty else {
            throw ControlClientError.appPathMissing
        }
        return value
    }

    private static var endpointURL: URL {
        FileManager.default.urls(
            for: .applicationSupportDirectory,
            in: .userDomainMask
        )[0]
        .appendingPathComponent("FIPSD/control-endpoint.json")
    }
}

private final class ExchangeState: @unchecked Sendable {
    let completed = DispatchSemaphore(value: 0)
    var result = Data()
    var failure: Error?
}

private struct AnyJSON: Decodable {
    let value: Any

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() { value = NSNull() }
        else if let item = try? container.decode(Bool.self) { value = item }
        else if let item = try? container.decode(Int64.self) { value = item }
        else if let item = try? container.decode(Double.self) { value = item }
        else if let item = try? container.decode(String.self) { value = item }
        else if let item = try? container.decode([AnyJSON].self) {
            value = item.map(\.value)
        } else {
            value = try container.decode([String: AnyJSON].self).mapValues(\.value)
        }
    }
}
