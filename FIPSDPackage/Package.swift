// swift-tools-version: 6.1
// The swift-tools-version declares the minimum version of Swift required to build this package.

import PackageDescription

let package = Package(
    name: "FIPSDFeature",
    platforms: [.macOS(.v15)],
    products: [
        // Products define the executables and libraries a package produces, making them visible to other packages.
        .library(
            name: "FIPSDFeature",
            targets: ["FIPSDFeature"]
        ),
        .executable(
            name: "fips-wind-tunnel-mcp",
            targets: ["FIPSDMCP"]
        ),
    ],
    dependencies: [
        .package(
            url: "https://github.com/gonzalezreal/swift-markdown-ui",
            from: "2.4.1"
        ),
    ],
    targets: [
        // Targets are the basic building blocks of a package, defining a module or a test suite.
        // Targets can depend on other targets in this package and products from dependencies.
        .target(
            name: "FIPSDFeature",
            dependencies: [
                .product(name: "MarkdownUI", package: "swift-markdown-ui")
            ],
            resources: [.process("Resources")]
        ),
        .target(name: "FIPSDMCPProtocol"),
        .executableTarget(
            name: "FIPSDMCP",
            dependencies: ["FIPSDMCPProtocol"]
        ),
        .testTarget(
            name: "FIPSDFeatureTests",
            dependencies: [
                "FIPSDFeature"
            ],
            resources: [.copy("Resources")]
        ),
        .testTarget(
            name: "FIPSDMCPProtocolTests",
            dependencies: ["FIPSDMCPProtocol"]
        ),
    ]
)
