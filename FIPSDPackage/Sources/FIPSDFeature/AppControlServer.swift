import Foundation
import Network

final class AppControlServer: @unchecked Sendable {
    private weak var model: WorkbenchModel?
    private let queue = DispatchQueue(label: "io.f7z.fipsd.control")
    private let token = UUID().uuidString.replacingOccurrences(of: "-", with: "")
    private var listener: NWListener?

    init(model: WorkbenchModel) {
        self.model = model
    }

    func start() throws {
        guard listener == nil else { return }
        let parameters = NWParameters.tcp
        parameters.requiredLocalEndpoint = .hostPort(host: "127.0.0.1", port: .any)
        let listener = try NWListener(using: parameters)
        listener.newConnectionHandler = { [weak self] in self?.accept($0) }
        listener.stateUpdateHandler = { [weak self, weak listener] state in
            guard case .ready = state, let self, let port = listener?.port else { return }
            do {
                try self.publish(port: port.rawValue)
            } catch {
                let message = error.localizedDescription
                Task { @MainActor [weak self] in
                    self?.model?.status = "App control endpoint failed: \(message)"
                }
            }
        }
        listener.start(queue: queue)
        self.listener = listener
    }

    func stop() {
        listener?.cancel()
        listener = nil
        guard let data = try? Data(contentsOf: AppControlEndpoint.fileURL),
              let endpoint = try? JSONDecoder().decode(AppControlEndpoint.self, from: data),
              endpoint.pid == ProcessInfo.processInfo.processIdentifier else { return }
        try? FileManager.default.removeItem(at: AppControlEndpoint.fileURL)
    }

    deinit {
        stop()
    }

    private func accept(_ connection: NWConnection) {
        connection.start(queue: queue)
        receive(on: connection, accumulated: Data())
    }

    private func receive(on connection: NWConnection, accumulated: Data) {
        connection.receive(
            minimumIncompleteLength: 1,
            maximumLength: 1_048_576
        ) { [weak self] data, _, complete, error in
            guard let self else { connection.cancel(); return }
            var buffer = accumulated
            if let data { buffer.append(data) }
            if let newline = buffer.firstIndex(of: 0x0A) {
                self.handle(Data(buffer[..<newline]), on: connection)
            } else if complete || error != nil {
                self.handle(buffer, on: connection)
            } else {
                self.receive(on: connection, accumulated: buffer)
            }
        }
    }

    private func handle(_ data: Data, on connection: NWConnection) {
        guard let request = try? JSONDecoder().decode(AppControlRequest.self, from: data) else {
            send(.failure("unknown", AppControlError.invalidRequest.localizedDescription), on: connection)
            return
        }
        guard request.token == token else {
            send(.failure(request.id, "App control authentication failed."), on: connection)
            return
        }
        Task { @MainActor [weak self] in
            guard let model = self?.model else {
                self?.send(.failure(request.id, "The app model is unavailable."), on: connection)
                return
            }
            self?.send(model.handleControl(request), on: connection)
        }
    }

    private func send(_ response: AppControlResponse, on connection: NWConnection) {
        guard var data = try? JSONEncoder().encode(response) else {
            connection.cancel()
            return
        }
        data.append(0x0A)
        connection.send(
            content: data,
            contentContext: .defaultMessage,
            isComplete: false,
            completion: .contentProcessed { [weak self] error in
                if error == nil {
                    self?.receive(on: connection, accumulated: Data())
                } else {
                    connection.cancel()
                }
            }
        )
    }

    private func publish(port: UInt16) throws {
        let endpoint = AppControlEndpoint(
            version: 1,
            pid: ProcessInfo.processInfo.processIdentifier,
            port: port,
            token: token
        )
        let url = AppControlEndpoint.fileURL
        try FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try JSONEncoder().encode(endpoint).write(to: url, options: .atomic)
        try FileManager.default.setAttributes(
            [.posixPermissions: 0o600],
            ofItemAtPath: url.path
        )
    }
}
