public enum MCPHTTPRoute {
    public static let canonicalPath = "/mcp"

    public static func accepts(_ path: String) -> Bool {
        path == canonicalPath || path == "/"
    }
}
