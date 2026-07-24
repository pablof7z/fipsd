import Foundation

extension ClaudeACPClient {
    nonisolated static func npxExecutable(
        environment: [String: String] = ProcessInfo.processInfo.environment,
        fileManager: FileManager = .default
    ) throws -> URL {
        if let explicit = environment["FIPS_WIND_TUNNEL_NPX"] {
            let url = URL(fileURLWithPath: explicit)
            if fileManager.isExecutableFile(atPath: url.path) { return url }
        }
        let pathCandidates = (environment["PATH"] ?? "")
            .split(separator: ":")
            .map { URL(fileURLWithPath: String($0)).appendingPathComponent("npx") }
        let fixed = ["/opt/homebrew/bin/npx", "/usr/local/bin/npx"]
            .map(URL.init(fileURLWithPath:))
        let nvmRoot = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".nvm/versions/node", isDirectory: true)
        let nvm = (try? fileManager.contentsOfDirectory(
            at: nvmRoot,
            includingPropertiesForKeys: nil
        ))?
        .sorted {
            $0.lastPathComponent.compare(
                $1.lastPathComponent,
                options: .numeric
            ) == .orderedDescending
        }
        .map { $0.appendingPathComponent("bin/npx") } ?? []
        if let found = (pathCandidates + fixed + nvm).first(where: {
            fileManager.isExecutableFile(atPath: $0.path)
        }) {
            return found
        }
        throw ClaudeACPError.missingNPX
    }

    nonisolated static func mcpExecutable(
        environment: [String: String] = ProcessInfo.processInfo.environment,
        fileManager: FileManager = .default
    ) throws -> URL {
        let fallback = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".local/bin/fips-wind-tunnel-mcp")
        let url = environment["FIPS_WIND_TUNNEL_MCP"]
            .map(URL.init(fileURLWithPath:)) ?? fallback
        guard fileManager.isExecutableFile(atPath: url.path) else {
            throw ClaudeACPError.missingMCP(url.path)
        }
        return url
    }

    nonisolated static func sessionWorkingDirectory(
        environment: [String: String] = ProcessInfo.processInfo.environment
    ) -> URL {
        if let path = environment["FIPS_WIND_TUNNEL_WORKSPACE"], !path.isEmpty {
            return URL(fileURLWithPath: path, isDirectory: true)
        }
        return FileManager.default.homeDirectoryForCurrentUser
    }

    nonisolated static func windTunnelSkill(
        environment: [String: String] = ProcessInfo.processInfo.environment,
        fileManager: FileManager = .default
    ) throws -> String {
        let root: URL
        if let path = environment["FIPS_WIND_TUNNEL_WORKSPACE"], !path.isEmpty {
            root = URL(fileURLWithPath: path, isDirectory: true)
        } else {
            let configured = FileManager.default.homeDirectoryForCurrentUser
                .appendingPathComponent(".config/fips-wind-tunnel/workspace-path")
            guard let path = try? String(
                contentsOf: configured,
                encoding: .utf8
            ).trimmingCharacters(in: .whitespacesAndNewlines),
                  !path.isEmpty else {
                throw ClaudeACPError.missingSkill(configured.path)
            }
            root = URL(fileURLWithPath: path, isDirectory: true)
        }
        let skill = root.appendingPathComponent(
            "skills/fips-wind-tunnel/SKILL.md"
        )
        guard fileManager.fileExists(atPath: skill.path),
              let text = try? String(contentsOf: skill, encoding: .utf8),
              !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        else {
            throw ClaudeACPError.missingSkill(skill.path)
        }
        return text
    }

    nonisolated static func executablePath(
        npx: URL,
        environment: [String: String]
    ) -> String {
        let local = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".local/bin").path
        return [
            npx.deletingLastPathComponent().path,
            local,
            environment["PATH"] ?? "/usr/bin:/bin:/usr/sbin:/sbin"
        ].joined(separator: ":")
    }
}
