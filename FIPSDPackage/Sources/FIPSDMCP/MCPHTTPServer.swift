import Foundation
import FIPSDMCPProtocol
import Network

final class MCPHTTPServer: @unchecked Sendable {
    private let port: NWEndpoint.Port
    private let bearerToken: String
    private let oauth: MCPOAuthService
    private let server = MCPServer()
    private let queue = DispatchQueue(label: "io.f7z.fipsd.mcp.http")

    init(port: UInt16, bearerToken: String, pairingCode: String) throws {
        guard let port = NWEndpoint.Port(rawValue: port) else {
            throw MCPHTTPError.invalidPort
        }
        guard bearerToken.utf8.count >= 32 else {
            throw MCPHTTPError.weakToken
        }
        self.port = port
        self.bearerToken = bearerToken
        oauth = try MCPOAuthService(pairingCode: pairingCode)
    }

    func run() throws -> Never {
        let parameters = NWParameters.tcp
        parameters.requiredLocalEndpoint = .hostPort(host: "127.0.0.1", port: port)
        let listener = try NWListener(using: parameters)
        let readiness = DispatchSemaphore(value: 0)
        let startup = HTTPStartupState()
        listener.stateUpdateHandler = { state in
            switch state {
            case .ready:
                readiness.signal()
            case let .failed(error):
                startup.error = error
                readiness.signal()
            default:
                break
            }
        }
        listener.newConnectionHandler = { [weak self] connection in
            self?.accept(connection)
        }
        listener.start(queue: queue)
        readiness.wait()
        if let error = startup.error { throw error }
        FileHandle.standardError.write(
            Data("FIPS Wind Tunnel MCP HTTP listening on 127.0.0.1:\(port)\n".utf8)
        )
        dispatchMain()
    }

    private func accept(_ connection: NWConnection) {
        connection.start(queue: queue)
        receive(connection, state: HTTPReceiveState())
    }

    private func receive(_ connection: NWConnection, state: HTTPReceiveState) {
        connection.receive(
            minimumIncompleteLength: 1,
            maximumLength: 1_048_576
        ) { [weak self] data, _, complete, error in
            guard let self else { return }
            if let data { state.data.append(data) }
            if state.data.count > 16_777_216 {
                send(connection, status: 413, body: ["error": "request too large"])
            } else if let request = HTTPRequest.parse(state.data) {
                handle(request, connection: connection)
            } else if error != nil || complete {
                send(connection, status: 400, body: ["error": "incomplete request"])
            } else {
                receive(connection, state: state)
            }
        }
    }

    private func handle(_ request: HTTPRequest, connection: NWConnection) {
        if let response = oauth.response(for: request) {
            send(connection, response: response)
            return
        }
        guard MCPHTTPRoute.accepts(request.path) else {
            if request.method == "GET", request.path == "/health" {
                send(connection, status: 200, body: ["status": "ok"])
            } else {
                send(connection, status: 404, body: ["error": "not found"])
            }
            return
        }
        guard request.method == "POST" else {
            send(
                connection,
                status: 405,
                body: ["error": "use POST for this sessionless MCP endpoint"],
                extraHeaders: ["Allow": "POST"]
            )
            return
        }
        guard request.headers["origin"] == nil else {
            send(connection, status: 403, body: ["error": "browser origins are not accepted"])
            return
        }
        let resource = "\(request.publicBaseURL)/mcp"
        guard adminAuthorized(request.headers["authorization"])
                || oauth.authorizes(request.headers["authorization"], resource: resource) else {
            send(
                connection,
                status: 401,
                body: ["error": "bearer token required"],
                extraHeaders: ["WWW-Authenticate": oauth.challenge(base: request.publicBaseURL)]
            )
            return
        }
        if let version = request.headers["mcp-protocol-version"],
           !["2025-06-18", "2025-03-26", "2024-11-05"].contains(version) {
            send(connection, status: 400, body: ["error": "unsupported MCP protocol version"])
            return
        }
        guard request.headers["content-type"]?.contains("application/json") == true,
              let object = try? JSONSerialization.jsonObject(with: request.body)
                as? [String: Any] else {
            send(connection, status: 400, body: ["error": "invalid JSON request"])
            return
        }
        guard let response = server.response(for: object) else {
            sendEmpty(connection, status: 202)
            return
        }
        send(connection, status: 200, body: response)
    }

    private func adminAuthorized(_ header: String?) -> Bool {
        guard let header, header.hasPrefix("Bearer ") else { return false }
        let candidate = Data(header.dropFirst(7).utf8)
        let expected = Data(bearerToken.utf8)
        guard candidate.count == expected.count else { return false }
        return zip(candidate, expected).reduce(UInt8(0)) { result, pair in
            result | (pair.0 ^ pair.1)
        } == 0
    }

    private func send(
        _ connection: NWConnection,
        status: Int,
        body: [String: Any],
        extraHeaders: [String: String] = [:]
    ) {
        send(connection, response: .json(status, body, headers: extraHeaders))
    }

    private func sendEmpty(_ connection: NWConnection, status: Int) {
        send(
            connection,
            response: HTTPServerResponse(
                status: status,
                data: Data(),
                contentType: "application/json",
                headers: [:]
            )
        )
    }

    private func send(_ connection: NWConnection, response: HTTPServerResponse) {
        let status = response.status
        let reason = [
            200: "OK", 201: "Created", 202: "Accepted", 302: "Found",
            400: "Bad Request", 401: "Unauthorized",
            403: "Forbidden", 404: "Not Found", 405: "Method Not Allowed",
            413: "Content Too Large"
        ][status] ?? "Error"
        var headers = [
            "HTTP/1.1 \(status) \(reason)",
            "Content-Type: \(response.contentType)",
            "Content-Length: \(response.data.count)",
            "Cache-Control: no-store",
            "Connection: close"
        ]
        headers += response.headers.sorted { $0.key < $1.key }.map {
            "\($0.key): \($0.value)"
        }
        var payload = Data((headers.joined(separator: "\r\n") + "\r\n\r\n").utf8)
        payload.append(response.data)
        connection.send(
            content: payload,
            isComplete: true,
            completion: .contentProcessed { _ in connection.cancel() }
        )
    }
}

private final class HTTPReceiveState: @unchecked Sendable {
    var data = Data()
}

private final class HTTPStartupState: @unchecked Sendable {
    var error: Error?
}

struct HTTPRequest {
    let method: String
    let path: String
    let query: [String: String]
    let headers: [String: String]
    let body: Data

    var form: [String: String] {
        guard let text = String(data: body, encoding: .utf8),
              let components = URLComponents(string: "https://local/?\(text)") else {
            return [:]
        }
        return Dictionary(
            components.queryItems?.compactMap { item in
                item.value.map { (item.name, $0) }
            } ?? [],
            uniquingKeysWith: { _, last in last }
        )
    }

    var publicBaseURL: String {
        let forwardedHost = headers["x-forwarded-host"] ?? headers["host"] ?? "127.0.0.1"
        let scheme = headers["x-forwarded-proto"]
            ?? (forwardedHost.hasPrefix("127.0.0.1") ? "http" : "https")
        return "\(scheme)://\(forwardedHost)"
    }

    static func parse(_ data: Data) -> Self? {
        let delimiter = Data("\r\n\r\n".utf8)
        guard let headerRange = data.range(of: delimiter),
              let headerText = String(
                data: data[..<headerRange.lowerBound],
                encoding: .utf8
              ) else { return nil }
        let lines = headerText.components(separatedBy: "\r\n")
        let requestLine = lines.first?.split(separator: " ") ?? []
        guard requestLine.count == 3 else { return nil }
        var headers: [String: String] = [:]
        for line in lines.dropFirst() {
            guard let separator = line.firstIndex(of: ":") else { continue }
            let name = line[..<separator].lowercased()
            let value = line[line.index(after: separator)...]
                .trimmingCharacters(in: .whitespaces)
            headers[name] = value
        }
        let length = Int(headers["content-length"] ?? "0") ?? 0
        let bodyStart = headerRange.upperBound
        guard length >= 0, data.count >= bodyStart + length else { return nil }
        let target = String(requestLine[1])
        let components = URLComponents(string: "https://local\(target)")
        let query = Dictionary(
            components?.queryItems?.compactMap { item in
                item.value.map { (item.name, $0) }
            } ?? [],
            uniquingKeysWith: { _, last in last }
        )
        return Self(
            method: String(requestLine[0]),
            path: components?.path ?? "/",
            query: query,
            headers: headers,
            body: Data(data[bodyStart..<(bodyStart + length)])
        )
    }
}

enum MCPHTTPError: LocalizedError {
    case invalidPort
    case weakToken

    var errorDescription: String? {
        switch self {
        case .invalidPort: "The MCP HTTP port is invalid."
        case .weakToken: "FIPS_WIND_TUNNEL_HTTP_TOKEN must contain at least 32 bytes."
        }
    }
}
