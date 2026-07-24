import Foundation

let arguments = CommandLine.arguments
if let index = arguments.firstIndex(of: "--http") {
    guard arguments.indices.contains(index + 1),
          let port = UInt16(arguments[index + 1]),
          let token = ProcessInfo.processInfo.environment["FIPS_WIND_TUNNEL_HTTP_TOKEN"],
          let pairingCode = ProcessInfo.processInfo.environment[
            "FIPS_WIND_TUNNEL_PAIRING_CODE"
          ]
    else {
        fatalError("""
        usage: fips-wind-tunnel-mcp --http PORT with \
        FIPS_WIND_TUNNEL_HTTP_TOKEN and FIPS_WIND_TUNNEL_PAIRING_CODE
        """)
    }
    try MCPHTTPServer(
        port: port,
        bearerToken: token,
        pairingCode: pairingCode
    ).run()
} else {
    MCPServer().run()
}
