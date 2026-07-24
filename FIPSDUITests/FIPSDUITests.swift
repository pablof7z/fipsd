import XCTest

final class FIPSDUITests: XCTestCase {
    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    @MainActor
    func testWorkbenchSwitchesBetweenExperimentControlsAndClaude() throws {
        let app = XCUIApplication()
        app.terminate()
        app.launchArguments += ["-ApplePersistenceIgnoreState", "YES"]
        app.launch()
        XCTAssertTrue(app.radioButtons["Experiment"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.radioButtons["Agent"].exists)
        XCTAssertTrue(
            app.textFields["claude-agent-composer"].waitForExistence(timeout: 10)
        )

        app.radioButtons["Experiment"].click()
        XCTAssertFalse(app.buttons["generateRunButton"].exists)
        XCTAssertTrue(app.buttons["runConfiguredButton"].exists)
        XCTAssertTrue(app.buttons["playbackButton"].exists)
        XCTAssertTrue(app.staticTexts["liveAccountingHeading"].exists)

        let inspectorToggle = app.buttons["toggleRightInspectorButton"]
        XCTAssertTrue(inspectorToggle.exists)
        inspectorToggle.click()
        XCTAssertTrue(
            app.staticTexts["liveAccountingHeading"]
                .waitForNonExistence(timeout: 2)
        )
        inspectorToggle.click()
        XCTAssertTrue(
            app.staticTexts["liveAccountingHeading"]
                .waitForExistence(timeout: 2)
        )
    }
}
