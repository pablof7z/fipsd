import Foundation

struct MCPKnowledgeResource {
    let uri: String
    let name: String
    let title: String
    let description: String
    let mimeType: String
    let file: URL
    let collection: String

    var definition: [String: Any] {
        let size = (try? file.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0
        return [
            "uri": uri,
            "name": name,
            "title": title,
            "description": description,
            "mimeType": mimeType,
            "size": size,
            "annotations": ["audience": ["assistant"], "priority": priority]
        ]
    }

    private var priority: Double {
        if uri == "fips-wind-tunnel://skill" { return 1 }
        if collection == "skill" || name.contains("schema") { return 0.9 }
        return 0.6
    }
}

final class MCPKnowledge {
    private let resources: [MCPKnowledgeResource]

    init() {
        guard let root = Self.workspaceRoot() else {
            resources = []
            return
        }
        var catalog: [MCPKnowledgeResource] = []
        catalog += Self.skillResources(root)
        catalog += Self.treeResources(
            root: root,
            subdirectory: "docs",
            prefix: "fips-wind-tunnel://product/docs/",
            collection: "product",
            extensions: ["md"]
        )
        catalog += Self.treeResources(
            root: root,
            subdirectory: "schemas",
            prefix: "fips-wind-tunnel://product/schemas/",
            collection: "schema",
            extensions: ["json"]
        )
        catalog += Self.treeResources(
            root: root,
            subdirectory: "examples",
            prefix: "fips-wind-tunnel://product/examples/",
            collection: "example",
            extensions: ["json", "yaml", "yml"]
        )
        let protocolRoot = root.deletingLastPathComponent().appendingPathComponent("fips")
        catalog += Self.protocolResources(protocolRoot)
        resources = catalog.sorted { $0.uri < $1.uri }
    }

    func list() -> [[String: Any]] {
        resources.map(\.definition)
    }

    func search(_ arguments: [String: Any]) -> [String: Any] {
        let query = (arguments["query"] as? String ?? "").lowercased()
        let collection = (arguments["collection"] as? String ?? "").lowercased()
        let requested = (arguments["limit"] as? NSNumber)?.intValue ?? 50
        let limit = min(max(requested, 1), 500)
        let matches = resources.filter { resource in
            let matchesCollection = collection.isEmpty || resource.collection == collection
            let haystack = [
                resource.uri, resource.name, resource.title, resource.description
            ].joined(separator: " ").lowercased()
            return matchesCollection && (query.isEmpty || haystack.contains(query))
        }
        return [
            "resources": matches.prefix(limit).map(\.definition),
            "matched": matches.count,
            "returned": min(matches.count, limit),
            "collections": ["skill", "product", "schema", "example", "protocol"]
        ]
    }

    func read(uri: String) throws -> [String: Any] {
        guard let resource = resources.first(where: { $0.uri == uri }) else {
            throw MCPKnowledgeError.unknownResource(uri)
        }
        let text = try String(contentsOf: resource.file, encoding: .utf8)
        return [
            "uri": resource.uri,
            "mimeType": resource.mimeType,
            "text": text
        ]
    }

    func skill() throws -> [String: Any] {
        try read(uri: "fips-wind-tunnel://skill")
    }

    private static func skillResources(_ root: URL) -> [MCPKnowledgeResource] {
        let skillRoot = root.appendingPathComponent("skills/fips-wind-tunnel")
        var result = [resource(
            uri: "fips-wind-tunnel://skill",
            file: skillRoot.appendingPathComponent("SKILL.md"),
            root: skillRoot,
            collection: "skill",
            description: "Core agent instructions for FIPS and Wind Tunnel work."
        )]
        let references = [
            "fips-protocol-map", "wind-tunnel-model", "campaigns-and-events"
        ]
        result += references.map { name in
            resource(
                uri: "fips-wind-tunnel://skill/reference/\(name)",
                file: skillRoot.appendingPathComponent("references/\(name).md"),
                root: skillRoot,
                collection: "skill",
                description: "Skill reference: \(name.replacingOccurrences(of: "-", with: " "))."
            )
        }
        return result.filter { FileManager.default.fileExists(atPath: $0.file.path) }
    }

    private static func protocolResources(_ root: URL) -> [MCPKnowledgeResource] {
        var result = treeResources(
            root: root,
            subdirectory: "docs",
            prefix: "fips-wind-tunnel://protocol/docs/",
            collection: "protocol",
            extensions: ["md"]
        )
        let readme = root.appendingPathComponent("README.md")
        if FileManager.default.fileExists(atPath: readme.path) {
            result.append(resource(
                uri: "fips-wind-tunnel://protocol/README.md",
                file: readme,
                root: root,
                collection: "protocol",
                description: "Checked-in FIPS project overview."
            ))
        }
        return result
    }

    private static func treeResources(
        root: URL,
        subdirectory: String,
        prefix: String,
        collection: String,
        extensions: Set<String>
    ) -> [MCPKnowledgeResource] {
        let directory = root.appendingPathComponent(subdirectory, isDirectory: true)
        guard let enumerator = FileManager.default.enumerator(
            at: directory,
            includingPropertiesForKeys: [.isRegularFileKey],
            options: [.skipsHiddenFiles]
        ) else { return [] }
        return enumerator.compactMap { item in
            guard let file = item as? URL,
                  extensions.contains(file.pathExtension.lowercased()),
                  (try? file.resourceValues(forKeys: [.isRegularFileKey]).isRegularFile) == true
            else { return nil }
            let relative = String(file.path.dropFirst(directory.path.count + 1))
            return resource(
                uri: prefix + relative,
                file: file,
                root: directory,
                collection: collection,
                description: "\(collection.capitalized) knowledge: \(relative)."
            )
        }
    }

    private static func resource(
        uri: String,
        file: URL,
        root: URL,
        collection: String,
        description: String
    ) -> MCPKnowledgeResource {
        let relative = file.path.hasPrefix(root.path)
            ? String(file.path.dropFirst(root.path.count)).trimmingCharacters(
                in: CharacterSet(charactersIn: "/")
            )
            : file.lastPathComponent
        let title = file.deletingPathExtension().lastPathComponent
            .replacingOccurrences(of: "-", with: " ").capitalized
        return MCPKnowledgeResource(
            uri: uri,
            name: relative.isEmpty ? file.lastPathComponent : relative,
            title: title,
            description: description,
            mimeType: file.pathExtension == "json" ? "application/json" : "text/markdown",
            file: file,
            collection: collection
        )
    }

    private static func workspaceRoot() -> URL? {
        let environment = ProcessInfo.processInfo.environment
        if let path = environment["FIPS_WIND_TUNNEL_WORKSPACE"], !path.isEmpty {
            return URL(fileURLWithPath: path, isDirectory: true)
        }
        let configured = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".config/fips-wind-tunnel/workspace-path")
        if let path = try? String(contentsOf: configured, encoding: .utf8)
            .trimmingCharacters(in: .whitespacesAndNewlines),
           !path.isEmpty {
            return URL(fileURLWithPath: path, isDirectory: true)
        }
        let cwd = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
        return FileManager.default.fileExists(
            atPath: cwd.appendingPathComponent("skills/fips-wind-tunnel/SKILL.md").path
        ) ? cwd : nil
    }
}

enum MCPKnowledgeError: LocalizedError {
    case unknownResource(String)

    var errorDescription: String? {
        switch self {
        case let .unknownResource(uri): "Unknown FIPS knowledge resource \(uri)."
        }
    }
}
