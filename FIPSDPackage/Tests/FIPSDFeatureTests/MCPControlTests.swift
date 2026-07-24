import Foundation
import Testing
@testable import FIPSDFeature

@MainActor
@Test func mcpControlUpdatesValidatedParametersAndReportsState() throws {
    let model = WorkbenchModel()
    let request = AppControlRequest(
        id: "set",
        token: "ignored-after-server-auth",
        command: "set_parameters",
        arguments: [
            "parameters": .object([
                "nodes": .integer(5),
                "arrivals": .integer(2),
                "interval_seconds": .number(5),
                "mixed_transports": .bool(false)
            ])
        ]
    )
    let response = model.handleControl(request)
    #expect(response.ok)
    #expect(model.configuration.nodes == 5)
    #expect(model.configuration.arrivals == 2)
    #expect(model.configuration.intervalSeconds == 5)
    #expect(!model.configuration.mixedTransports)
    let state = try #require(response.result?.object)
    let configuration = try #require(state["configuration"]?.object)
    #expect(configuration["nodes"]?.int == 5)
    #expect(configuration["arrivals"]?.int == 2)
}

@MainActor
@Test func mcpPlaybackAndExplanationReflectTheVisibleModel() throws {
    let model = WorkbenchModel()
    model.speed = 1
    let speed = model.handleControl(AppControlRequest(
        id: "speed",
        token: "",
        command: "playback",
        arguments: [
            "action": .string("set_speed"),
            "speed": .number(20)
        ]
    ))
    #expect(speed.ok)
    #expect(model.speed == 20)
    let explanation = model.handleControl(AppControlRequest(
        id: "explain",
        token: "",
        command: "explain",
        arguments: ["focus": .string("root convergence")]
    ))
    #expect(explanation.ok)
    let object = try #require(explanation.result?.object)
    #expect(object["explanation"]?.string?.contains("Requested focus") == true)
    #expect(object["state"]?.object?["speed"]?.double == 20)
}

@MainActor
@Test func mcpControlRejectsUnknownParametersAndCommands() {
    let model = WorkbenchModel()
    let parameter = model.handleControl(AppControlRequest(
        id: "parameter",
        token: "",
        command: "set_parameters",
        arguments: ["parameters": .object(["made_up": .integer(1)])]
    ))
    #expect(!parameter.ok)
    #expect(parameter.error?.contains("parameters.made_up") == true)
    let command = model.handleControl(AppControlRequest(
        id: "command",
        token: "",
        command: "erase_everything",
        arguments: [:]
    ))
    #expect(!command.ok)
    #expect(command.error?.contains("Unsupported control command") == true)
}
