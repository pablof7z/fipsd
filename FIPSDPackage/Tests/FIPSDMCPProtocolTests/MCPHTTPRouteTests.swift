import Testing
@testable import FIPSDMCPProtocol

@Test func chatGPTRootEndpointAliasesCanonicalMCPPath() {
    #expect(MCPHTTPRoute.accepts("/"))
    #expect(MCPHTTPRoute.accepts("/mcp"))
    #expect(!MCPHTTPRoute.accepts("/health"))
    #expect(!MCPHTTPRoute.accepts("/other"))
}
