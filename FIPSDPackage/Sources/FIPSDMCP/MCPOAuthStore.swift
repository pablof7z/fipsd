import CryptoKit
import Foundation
import Security

struct OAuthClient: Codable {
    let id: String
    let name: String
    let redirectURIs: [String]
}

struct OAuthGrant: Codable {
    let clientID: String
    let redirectURI: String
    let state: String?
    let challenge: String
    let resource: String
    let scope: String
    let createdAt: Int64
}

struct OAuthAccess: Codable {
    let clientID: String
    let resource: String
    let scope: String
    let issuedAt: Int64
}

private struct OAuthPersistentState: Codable {
    var clients: [String: OAuthClient] = [:]
    var pending: [String: OAuthGrant] = [:]
    var codes: [String: OAuthGrant] = [:]
    var access: [String: OAuthAccess] = [:]
}

final class MCPOAuthStore {
    private let pairingCode: String
    private let stateURL: URL
    private var state: OAuthPersistentState
    private var failedPairings = 0
    private var lockedUntil: Date?

    init(pairingCode: String) throws {
        guard pairingCode.range(of: #"^\d{4}$"#, options: .regularExpression) != nil else {
            throw MCPOAuthError.invalidPairingCode
        }
        self.pairingCode = pairingCode
        stateURL = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".config/fips-wind-tunnel/oauth-state.json")
        if let data = try? Data(contentsOf: stateURL),
           let decoded = try? JSONDecoder().decode(OAuthPersistentState.self, from: data) {
            state = decoded
        } else {
            state = OAuthPersistentState()
        }
        purgeTransient()
    }

    func register(name: String, redirectURIs: [String]) throws -> OAuthClient {
        guard !name.isEmpty, name.count <= 200,
              !redirectURIs.isEmpty, redirectURIs.count <= 20,
              redirectURIs.allSatisfy(Self.validRedirectURI) else {
            throw MCPOAuthError.invalidClient
        }
        let client = OAuthClient(
            id: Self.randomToken(bytes: 24),
            name: name,
            redirectURIs: redirectURIs
        )
        state.clients[client.id] = client
        try save()
        return client
    }

    func client(id: String, redirectURI: String) throws -> OAuthClient {
        guard let client = state.clients[id],
              client.redirectURIs.contains(redirectURI) else {
            throw MCPOAuthError.invalidClient
        }
        return client
    }

    func createPending(_ grant: OAuthGrant) throws -> String {
        purgeTransient()
        let id = Self.randomToken(bytes: 24)
        state.pending[Self.digest(id)] = grant
        try save()
        return id
    }

    func approve(pendingID: String, pairingCode candidate: String) throws -> (String, OAuthGrant) {
        if let lockedUntil, lockedUntil > Date() {
            throw MCPOAuthError.pairingLocked
        }
        guard Self.constantTimeEqual(candidate, pairingCode) else {
            failedPairings += 1
            if failedPairings >= 5 {
                lockedUntil = Date().addingTimeInterval(60)
                failedPairings = 0
            }
            throw MCPOAuthError.pairingRejected
        }
        failedPairings = 0
        lockedUntil = nil
        purgeTransient()
        guard let grant = state.pending.removeValue(forKey: Self.digest(pendingID)) else {
            throw MCPOAuthError.invalidGrant
        }
        let code = Self.randomToken(bytes: 32)
        state.codes[Self.digest(code)] = grant
        try save()
        return (code, grant)
    }

    func exchange(
        code: String,
        clientID: String,
        redirectURI: String,
        verifier: String,
        resource: String
    ) throws -> String {
        purgeTransient()
        let key = Self.digest(code)
        guard let grant = state.codes.removeValue(forKey: key),
              grant.clientID == clientID,
              grant.redirectURI == redirectURI,
              grant.resource == resource,
              Self.pkceChallenge(verifier) == grant.challenge else {
            try? save()
            throw MCPOAuthError.invalidGrant
        }
        let token = Self.randomToken(bytes: 32)
        state.access[Self.digest(token)] = OAuthAccess(
            clientID: clientID,
            resource: resource,
            scope: grant.scope,
            issuedAt: Int64(Date().timeIntervalSince1970)
        )
        try save()
        return token
    }

    func authorizes(token: String, resource: String) -> Bool {
        guard let access = state.access[Self.digest(token)] else { return false }
        return access.resource == resource && access.scope.split(separator: " ").contains("mcp")
    }

    private func purgeTransient() {
        let now = Int64(Date().timeIntervalSince1970)
        state.pending = state.pending.filter { now - $0.value.createdAt <= 600 }
        state.codes = state.codes.filter { now - $0.value.createdAt <= 300 }
        try? save()
    }

    private func save() throws {
        try FileManager.default.createDirectory(
            at: stateURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try JSONEncoder().encode(state).write(to: stateURL, options: .atomic)
        try FileManager.default.setAttributes(
            [.posixPermissions: 0o600],
            ofItemAtPath: stateURL.path
        )
    }

    private static func validRedirectURI(_ raw: String) -> Bool {
        guard let url = URL(string: raw), url.fragment == nil else { return false }
        if url.scheme?.lowercased() == "https" { return url.host != nil }
        guard url.scheme?.lowercased() == "http" else { return false }
        return ["127.0.0.1", "::1", "localhost"].contains(url.host?.lowercased() ?? "")
    }

    static func pkceChallenge(_ verifier: String) -> String {
        Data(SHA256.hash(data: Data(verifier.utf8))).base64URLEncoded
    }

    private static func digest(_ value: String) -> String {
        SHA256.hash(data: Data(value.utf8)).map { String(format: "%02x", $0) }.joined()
    }

    private static func constantTimeEqual(_ left: String, _ right: String) -> Bool {
        let lhs = Data(left.utf8)
        let rhs = Data(right.utf8)
        guard lhs.count == rhs.count else { return false }
        return zip(lhs, rhs).reduce(UInt8(0)) { $0 | ($1.0 ^ $1.1) } == 0
    }

    private static func randomToken(bytes: Int) -> String {
        var data = Data(count: bytes)
        data.withUnsafeMutableBytes { buffer in
            _ = SecRandomCopyBytes(kSecRandomDefault, bytes, buffer.baseAddress!)
        }
        return data.base64URLEncoded
    }
}

private extension Data {
    var base64URLEncoded: String {
        base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }
}

enum MCPOAuthError: LocalizedError {
    case invalidPairingCode
    case invalidClient
    case invalidGrant
    case pairingRejected
    case pairingLocked

    var errorDescription: String? {
        switch self {
        case .invalidPairingCode: "OAuth pairing code must contain exactly four digits."
        case .invalidClient: "OAuth client or redirect URI is invalid."
        case .invalidGrant: "OAuth authorization grant is invalid or expired."
        case .pairingRejected: "Pairing code was not accepted."
        case .pairingLocked: "Too many pairing attempts. Try again in one minute."
        }
    }
}
