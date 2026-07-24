import Foundation

struct HTTPServerResponse {
    let status: Int
    let data: Data
    let contentType: String
    let headers: [String: String]

    static func json(
        _ object: [String: Any],
        headers: [String: String] = [:]
    ) -> Self {
        json(200, object, headers: headers)
    }

    static func json(
        _ status: Int = 200,
        _ object: [String: Any],
        headers: [String: String] = [:]
    ) -> Self {
        Self(
            status: status,
            data: (try? JSONSerialization.data(withJSONObject: object)) ?? Data(),
            contentType: "application/json",
            headers: headers
        )
    }

    static func html(_ status: Int = 200, _ html: String) -> Self {
        Self(
            status: status,
            data: Data(html.utf8),
            contentType: "text/html; charset=utf-8",
            headers: [:]
        )
    }

    static func html(_ html: String) -> Self {
        Self.html(200, html)
    }

    static func redirect(_ location: String) -> Self {
        Self(status: 302, data: Data(), contentType: "text/plain", headers: [
            "Location": location
        ])
    }
}

final class MCPOAuthService {
    private let store: MCPOAuthStore

    init(pairingCode: String) throws {
        store = try MCPOAuthStore(pairingCode: pairingCode)
    }

    func handles(_ path: String) -> Bool {
        path.hasPrefix("/.well-known/")
            || ["/register", "/authorize", "/pair", "/token"].contains(path)
    }

    func response(for request: HTTPRequest) -> HTTPServerResponse? {
        let base = request.publicBaseURL
        switch (request.method, request.path) {
        case ("GET", "/.well-known/oauth-protected-resource"),
             ("GET", "/.well-known/oauth-protected-resource/mcp"):
            return protectedResourceMetadata(base: base)
        case ("GET", "/.well-known/oauth-authorization-server"):
            return authorizationServerMetadata(base: base)
        case ("POST", "/register"):
            return register(request)
        case ("GET", "/authorize"):
            return authorize(request, base: base)
        case ("POST", "/pair"):
            return pair(request)
        case ("POST", "/token"):
            return token(request)
        default:
            return handles(request.path)
                ? .json(405, ["error": "method_not_allowed"])
                : nil
        }
    }

    func authorizes(_ header: String?, resource: String) -> Bool {
        guard let token = Self.bearerToken(header) else { return false }
        return store.authorizes(token: token, resource: resource)
    }

    func challenge(base: String) -> String {
        let metadata = "\(base)/.well-known/oauth-protected-resource/mcp"
        return "Bearer resource_metadata=\"\(metadata)\", scope=\"mcp\""
    }

    private func protectedResourceMetadata(base: String) -> HTTPServerResponse {
        .json([
            "resource": "\(base)/mcp",
            "authorization_servers": [base],
            "scopes_supported": ["mcp"],
            "bearer_methods_supported": ["header"]
        ])
    }

    private func authorizationServerMetadata(base: String) -> HTTPServerResponse {
        .json([
            "issuer": base,
            "authorization_endpoint": "\(base)/authorize",
            "token_endpoint": "\(base)/token",
            "registration_endpoint": "\(base)/register",
            "response_types_supported": ["code"],
            "grant_types_supported": ["authorization_code"],
            "code_challenge_methods_supported": ["S256"],
            "token_endpoint_auth_methods_supported": ["none"],
            "scopes_supported": ["mcp"]
        ])
    }

    private func register(_ request: HTTPRequest) -> HTTPServerResponse {
        do {
            guard let object = try JSONSerialization.jsonObject(with: request.body)
                    as? [String: Any],
                  let redirects = object["redirect_uris"] as? [String] else {
                throw MCPOAuthError.invalidClient
            }
            let client = try store.register(
                name: object["client_name"] as? String ?? "MCP client",
                redirectURIs: redirects
            )
            return .json(201, [
                "client_id": client.id,
                "client_name": client.name,
                "redirect_uris": client.redirectURIs,
                "grant_types": ["authorization_code"],
                "response_types": ["code"],
                "token_endpoint_auth_method": "none"
            ])
        } catch {
            return oauthError("invalid_client_metadata", error.localizedDescription)
        }
    }

    private func authorize(_ request: HTTPRequest, base: String) -> HTTPServerResponse {
        do {
            let query = request.query
            guard query["response_type"] == "code",
                  query["code_challenge_method"] == "S256",
                  let clientID = query["client_id"],
                  let redirectURI = query["redirect_uri"],
                  let challenge = query["code_challenge"],
                  challenge.count >= 43,
                  let resource = query["resource"],
                  resource == "\(base)/mcp" else {
                throw MCPOAuthError.invalidGrant
            }
            let client = try store.client(id: clientID, redirectURI: redirectURI)
            let pending = try store.createPending(OAuthGrant(
                clientID: clientID,
                redirectURI: redirectURI,
                state: query["state"],
                challenge: challenge,
                resource: resource,
                scope: query["scope"] ?? "mcp",
                createdAt: Int64(Date().timeIntervalSince1970)
            ))
            return .html(pairingPage(client: client, pending: pending))
        } catch {
            return oauthError("invalid_request", error.localizedDescription)
        }
    }

    private func pair(_ request: HTTPRequest) -> HTTPServerResponse {
        do {
            let form = request.form
            guard let pending = form["pending"], let code = form["pairing_code"] else {
                throw MCPOAuthError.invalidGrant
            }
            let approved = try store.approve(pendingID: pending, pairingCode: code)
            guard var components = URLComponents(string: approved.1.redirectURI) else {
                throw MCPOAuthError.invalidGrant
            }
            var items = components.queryItems ?? []
            items.append(URLQueryItem(name: "code", value: approved.0))
            if let state = approved.1.state {
                items.append(URLQueryItem(name: "state", value: state))
            }
            components.queryItems = items
            guard let location = components.url?.absoluteString else {
                throw MCPOAuthError.invalidGrant
            }
            return .redirect(location)
        } catch {
            return .html(403, failurePage(error.localizedDescription))
        }
    }

    private func token(_ request: HTTPRequest) -> HTTPServerResponse {
        do {
            let form = request.form
            guard form["grant_type"] == "authorization_code",
                  let code = form["code"],
                  let clientID = form["client_id"],
                  let redirectURI = form["redirect_uri"],
                  let verifier = form["code_verifier"],
                  verifier.count >= 43,
                  let resource = form["resource"] else {
                throw MCPOAuthError.invalidGrant
            }
            _ = try store.client(id: clientID, redirectURI: redirectURI)
            let accessToken = try store.exchange(
                code: code,
                clientID: clientID,
                redirectURI: redirectURI,
                verifier: verifier,
                resource: resource
            )
            return .json([
                "access_token": accessToken,
                "token_type": "Bearer",
                "scope": "mcp"
            ], headers: ["Cache-Control": "no-store", "Pragma": "no-cache"])
        } catch {
            return oauthError("invalid_grant", error.localizedDescription)
        }
    }

    private func pairingPage(client: OAuthClient, pending: String) -> String {
        """
        <!doctype html><meta name="viewport" content="width=device-width">
        <title>Pair FIPS Wind Tunnel</title>
        <style>\(Self.style)</style>
        <main><p class="eyebrow">FIPS Protocol Wind Tunnel</p>
        <h1>Authorize \(Self.escape(client.name))</h1>
        <p>This client will be able to inspect and control the visible simulator.
        Enter the four-digit code shown by the Wind Tunnel owner.</p>
        <form method="post" action="/pair">
        <input type="hidden" name="pending" value="\(Self.escape(pending))">
        <input name="pairing_code" inputmode="numeric" pattern="[0-9]{4}"
          maxlength="4" autocomplete="one-time-code" autofocus required>
        <button type="submit">Authorize permanently</button></form></main>
        """
    }

    private func failurePage(_ message: String) -> String {
        """
        <!doctype html><meta name="viewport" content="width=device-width">
        <title>Pairing failed</title><style>\(Self.style)</style>
        <main><p class="eyebrow">FIPS Protocol Wind Tunnel</p>
        <h1>Pairing failed</h1><p>\(Self.escape(message))</p></main>
        """
    }

    private func oauthError(_ code: String, _ description: String) -> HTTPServerResponse {
        .json(400, ["error": code, "error_description": description])
    }

    private static func bearerToken(_ header: String?) -> String? {
        guard let header, header.hasPrefix("Bearer ") else { return nil }
        return String(header.dropFirst(7))
    }

    private static func escape(_ value: String) -> String {
        value.replacingOccurrences(of: "&", with: "&amp;")
            .replacingOccurrences(of: "<", with: "&lt;")
            .replacingOccurrences(of: ">", with: "&gt;")
            .replacingOccurrences(of: "\"", with: "&quot;")
    }

    private static let style = """
    *{box-sizing:border-box}body{margin:0;background:#f4f1ea;color:#171713;
    font:17px system-ui}main{max-width:560px;margin:12vh auto;padding:40px}
    .eyebrow{font-size:12px;text-transform:uppercase;letter-spacing:.14em}
    h1{font:42px Georgia,serif;margin:12px 0}p{line-height:1.55}
    input{width:100%;font-size:36px;letter-spacing:.4em;padding:14px;margin:18px 0}
    button{width:100%;padding:14px;background:#171713;color:white;border:0;font-weight:700}
    """
}
