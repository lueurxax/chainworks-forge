import XCTest

/// Page Object for the Ideas tab — idea creation, navigation, and Start Run sheet.
struct IdeasScreen {
    let app: XCUIApplication

    /// Creates a test idea and returns true on success. Assumes tabs are already visible.
    /// In headless xcodebuild, NavigationSplitView toolbar rendering is unreliable,
    /// so this function tries multiple strategies to find the "New Idea" button.
    @discardableResult
    func createIdea(title: String) -> Bool {
        let screen = AppScreen(app: app)
        guard screen.selectTab("Ideas") else { return false }

        var newIdeaButton = app.toolbars.buttons["New Idea"].firstMatch
        if !newIdeaButton.waitForExistence(timeout: 15) {
            newIdeaButton = app.buttons["New Idea"].firstMatch
            if !newIdeaButton.waitForExistence(timeout: 5) {
                let predicate = NSPredicate(format: "label == %@ AND isEnabled == true", "New Idea")
                newIdeaButton = app.descendants(matching: .any).matching(predicate).firstMatch
                guard newIdeaButton.waitForExistence(timeout: 5) else { return false }
            }
        }
        newIdeaButton.click()

        let titleField = app.textFields["Title"]
        guard titleField.waitForExistence(timeout: 10) else { return false }
        titleField.click()
        titleField.typeText(title)

        let saveBtn = app.buttons["Save Idea"].firstMatch
        guard saveBtn.waitForExistence(timeout: 5) else { return false }
        saveBtn.click()

        let ideaCell = app.staticTexts[title]
        return ideaCell.waitForExistence(timeout: 10)
    }

    /// Finds an idea row by title using multiple fallback strategies.
    func findRow(_ title: String) -> XCUIElement {
        let identifiedRow = app.buttons["idea-row-\(title)"].firstMatch
        if identifiedRow.exists { return identifiedRow }
        let staticText = app.staticTexts[title].firstMatch
        if staticText.exists { return staticText }
        let exactButton = app.buttons[title].firstMatch
        if exactButton.exists { return exactButton }
        let predicate = NSPredicate(format: "label CONTAINS %@ AND isEnabled == true", title)
        return app.buttons.matching(predicate).firstMatch
    }

    /// Navigates to idea detail and opens Start Run sheet. Returns true if sheet opened.
    func openStartRunSheet(for ideaTitle: String) -> Bool {
        let screen = AppScreen(app: app)
        guard screen.selectTab("Ideas") else { return false }

        let ideaRow = findRow(ideaTitle)
        guard ideaRow.waitForExistence(timeout: 15) else { return false }
        ideaRow.click()

        var startButton = app.buttons["start-new-run-button"].firstMatch
        if !startButton.waitForExistence(timeout: 10) {
            startButton = app.buttons["Start New Run"].firstMatch
            _ = startButton.waitForExistence(timeout: 5)
        }
        guard startButton.exists else { return false }
        startButton.click()
        return true
    }
}
