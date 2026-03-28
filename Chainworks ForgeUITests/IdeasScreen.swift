import XCTest
#if os(macOS)
import AppKit
#endif

/// Page Object for the Ideas tab — idea creation, navigation, and Start Run sheet.
struct IdeasScreen {
    let app: XCUIApplication

    private func dismissTransientDialogIfNeeded() {
        let dialog = app.dialogs.firstMatch
        guard dialog.exists else { return }
        app.typeKey(.escape, modifierFlags: [])
        let dismissed = XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "exists == false"),
            object: dialog
        )
        _ = XCTWaiter.wait(for: [dismissed], timeout: 2)
        RunLoop.current.run(until: Date().addingTimeInterval(0.2))
    }

    private func newIdeaTitleField() -> XCUIElement {
        let candidates = [
            app.textFields["new-idea-title-field"].firstMatch,
            app.textFields["Title"].firstMatch,
            app.descendants(matching: .any)
                .matching(NSPredicate(format: "identifier == %@", "new-idea-title-field"))
                .firstMatch
        ]
        for candidate in candidates where candidate.exists {
            return candidate
        }
        return candidates[0]
    }

    private func newIdeaSaveButton() -> XCUIElement {
        let candidates = [
            app.descendants(matching: .button)
                .matching(NSPredicate(format: "identifier == %@", "new-idea-save-button"))
                .firstMatch,
            app.buttons["Save Idea"].firstMatch,
            app.descendants(matching: .button)
                .matching(NSPredicate(format: "label == %@", "Save Idea"))
                .firstMatch
        ]
        for candidate in candidates where candidate.exists {
            return candidate
        }
        return candidates[0]
    }

    private func newIdeaSheet() -> XCUIElement {
        app.descendants(matching: .any)
            .matching(NSPredicate(format: "identifier == %@", "new-idea-sheet"))
            .firstMatch
    }

    private func commitNewIdeaSheet(title: String) -> Bool {
        let titleField = newIdeaTitleField()
        guard titleField.waitForExistence(timeout: 10) else { return false }

        var saveBtn = newIdeaSaveButton()
        guard saveBtn.waitForExistence(timeout: 5) else { return false }

        let maybePrefilled = XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "isEnabled == true"),
            object: saveBtn
        )
        let prefilledReady = XCTWaiter.wait(for: [maybePrefilled], timeout: 2) == .completed
        if !prefilledReady {
            let currentValue = ((titleField.value as? String) ?? "")
                .trimmingCharacters(in: .whitespacesAndNewlines)
            if currentValue != title {
                replaceText(in: titleField, with: title)
            }

            let bodyField = app.descendants(matching: .any)
                .matching(NSPredicate(format: "identifier == %@", "new-idea-body-field"))
                .firstMatch
            if bodyField.waitForExistence(timeout: 2) {
                bodyField.click()
            }

            saveBtn = newIdeaSaveButton()
            guard saveBtn.waitForExistence(timeout: 5) else { return false }
            let saveEnabled = XCTNSPredicateExpectation(
                predicate: NSPredicate(format: "isEnabled == true"),
                object: saveBtn
            )
            guard XCTWaiter.wait(for: [saveEnabled], timeout: 5) == .completed else { return false }
        }

        saveBtn.click()
        return waitForIdeaCreationToSettle(named: title)
    }

    private func replaceText(in element: XCUIElement, with value: String) {
        dismissTransientDialogIfNeeded()
        if !app.dialogs.firstMatch.exists {
            element.click()
        }
        element.typeKey("a", modifierFlags: .command)
        element.typeKey(.delete, modifierFlags: [])
#if os(macOS)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(value, forType: .string)
        element.typeKey("v", modifierFlags: .command)
        RunLoop.current.run(until: Date().addingTimeInterval(0.3))
        var afterPaste = (element.value as? String) ?? ""
        if !afterPaste.contains(value) {
            // Fallback 1: Edit menu paste
            let editMenu = app.menuBars.menuBarItems["Edit"].firstMatch
            if editMenu.waitForExistence(timeout: 1) {
                editMenu.click()
                let pasteItem = app.menuItems["Paste"].firstMatch
                if pasteItem.waitForExistence(timeout: 1), pasteItem.isEnabled {
                    pasteItem.click()
                } else {
                    app.typeKey(.escape, modifierFlags: [])
                }
            }
            RunLoop.current.run(until: Date().addingTimeInterval(0.3))
            afterPaste = (element.value as? String) ?? ""
        }
        if !afterPaste.contains(value) {
            // Fallback 2: Direct typeText — slower but works in headless xcodebuild
            element.typeKey("a", modifierFlags: .command)
            element.typeKey(.delete, modifierFlags: [])
            element.typeText(value)
            RunLoop.current.run(until: Date().addingTimeInterval(0.3))
        }
        element.typeKey(.tab, modifierFlags: [])
        RunLoop.current.run(until: Date().addingTimeInterval(0.3))
#else
        element.typeText(value)
#endif
    }

    private func onIdeasRoot() -> Bool {
        let signals = [
            app.descendants(matching: .any).matching(NSPredicate(format: "identifier == %@", "ideas-root-view")).firstMatch,
            app.descendants(matching: .any).matching(NSPredicate(format: "identifier == %@", "idea-list")).firstMatch,
            app.descendants(matching: .any).matching(NSPredicate(format: "identifier == %@", "ideas-new-idea")).firstMatch,
            app.descendants(matching: .any).matching(NSPredicate(format: "identifier == %@", "ideas-new-idea-inline")).firstMatch,
            app.descendants(matching: .any).matching(NSPredicate(format: "identifier == %@", "ideas-open-archive")).firstMatch
        ]
        return signals.contains(where: \.exists)
    }

    private func waitForIdeaCreationToSettle(named title: String) -> Bool {
        RunLoop.current.run(until: Date().addingTimeInterval(0.5))

        if detailSurfaceReady(timeout: 10) {
            return true
        }

        let row = findRow(title)
        if row.waitForExistence(timeout: 10) {
            return openIdea(named: title)
        }

        return false
    }

    private func detailSurfaceReady(timeout: TimeInterval = 10) -> Bool {
        let signals = [
            app.textFields["idea-workspace-root-path-field"].firstMatch,
            app.buttons["start-new-run-button"].firstMatch,
            app.buttons["archive-idea-button"].firstMatch,
            app.buttons["stop-run-button"].firstMatch
        ]
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if signals.contains(where: \.exists) {
                return true
            }
            RunLoop.current.run(until: Date().addingTimeInterval(0.2))
        }
        return signals.contains(where: \.exists)
    }

    @discardableResult
    private func revealSidebarIfNeeded() -> Bool {
        let toggleLabels = ["Show Sidebar", "Hide Sidebar", "Toggle Sidebar", "Show Navigation", "Hide Navigation"]

        for label in toggleLabels {
            let button = app.buttons[label].firstMatch
            if button.waitForExistence(timeout: 1), button.isHittable {
                button.click()
                RunLoop.current.run(until: Date().addingTimeInterval(0.5))
                return true
            }
        }

        return false
    }

    /// Creates a test idea and returns true on success. Assumes tabs are already visible.
    /// In headless xcodebuild, NavigationSplitView toolbar rendering is unreliable,
    /// so this function tries multiple strategies to find the "New Idea" button.
    @discardableResult
    func createIdea(title: String) -> Bool {
        let screen = AppScreen(app: app)
        if !onIdeasRoot() && !screen.selectTab("Ideas") {
            return false
        }

        var newIdeaButton = app.descendants(matching: .any)
            .matching(NSPredicate(format: "identifier == %@", "ideas-new-idea-inline"))
            .firstMatch
        if !newIdeaButton.waitForExistence(timeout: 5) {
            newIdeaButton = app.descendants(matching: .any)
                .matching(NSPredicate(format: "identifier == %@", "ideas-new-idea"))
                .firstMatch
        }
        if !newIdeaButton.waitForExistence(timeout: 5) {
            newIdeaButton = app.toolbars.buttons["New Idea"].firstMatch
        }
        if !newIdeaButton.waitForExistence(timeout: 15) {
            newIdeaButton = app.buttons["New Idea"].firstMatch
            if !newIdeaButton.waitForExistence(timeout: 5) {
                let predicate = NSPredicate(format: "label == %@ AND isEnabled == true", "New Idea")
                newIdeaButton = app.descendants(matching: .any).matching(predicate).firstMatch
                if !newIdeaButton.waitForExistence(timeout: 5) {
                    app.typeKey("n", modifierFlags: .command)
                    return commitNewIdeaSheet(title: title)
                }
            }
        }
        newIdeaButton.click()
        return commitNewIdeaSheet(title: title)
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

    @discardableResult
    func openIdea(named ideaTitle: String) -> Bool {
        let ideaRow = findRow(ideaTitle)
        if ideaRow.waitForExistence(timeout: 10) {
            ideaRow.click()
            if detailSurfaceReady(timeout: 5) {
                return true
            }
        }

        _ = revealSidebarIfNeeded()
        if ideaRow.waitForExistence(timeout: 10) {
            ideaRow.click()
            if detailSurfaceReady(timeout: 5) {
                return true
            }
        }

        let screen = AppScreen(app: app)
        if !onIdeasRoot() && !screen.selectTab("Ideas") {
            return false
        }
        _ = revealSidebarIfNeeded()
        guard ideaRow.waitForExistence(timeout: 10) else { return false }
        ideaRow.click()
        return detailSurfaceReady()
    }

    /// Navigates to idea detail and opens Start Run sheet. Returns true if sheet opened.
    func openStartRunSheet(for ideaTitle: String) -> Bool {
        guard openIdea(named: ideaTitle) else { return false }

        var startButton = app.buttons["start-new-run-button"].firstMatch
        if !startButton.waitForExistence(timeout: 10) {
            startButton = app.buttons["Start New Run"].firstMatch
            _ = startButton.waitForExistence(timeout: 5)
        }
        guard startButton.exists else { return false }
        startButton.click()
        return true
    }

    /// Scrolls a SwiftUI Form element into the hittable viewport.
    /// Uses coordinate-based tap which forces XCUITest to scroll the element on screen.
    private func scrollToElement(_ element: XCUIElement) {
        guard element.exists else { return }
        // coordinate(withNormalizedOffset:) triggers auto-scroll to the element
        // even when the element is off-screen in a ScrollView or Form.
        let coordinate = element.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
        coordinate.tap()
        RunLoop.current.run(until: Date().addingTimeInterval(0.2))
    }

    @discardableResult
    func setProjectDirectory(_ path: String, for ideaTitle: String) -> Bool {
        let pathField = app.textFields["idea-workspace-root-path-field"].firstMatch
        if !pathField.waitForExistence(timeout: 10) {
            guard openIdea(named: ideaTitle) else { return false }
            _ = revealSidebarIfNeeded()
            guard openIdea(named: ideaTitle), pathField.waitForExistence(timeout: 10) else { return false }
        }

        let currentValue = (pathField.value as? String)?
            .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        if currentValue == path {
            return true
        }

        // Scroll the field into the hittable area of the form by clicking on it
        // only when a real edit is needed. Re-focusing an already-correct path field
        // has been enough to destabilize macOS UI automation in the canonical 007 flow.
        scrollToElement(pathField)

        // Attempt text entry with retry — clipboard operations are unreliable in headless xcodebuild.
        for attempt in 1...3 {
            replaceText(in: pathField, with: path)
            RunLoop.current.run(until: Date().addingTimeInterval(0.3))
            let afterReplace = (pathField.value as? String)?
                .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            if afterReplace.contains(path) { break }
            if attempt < 3 {
                // Re-focus and retry
                scrollToElement(pathField)
            }
        }

        let saveButton = app.buttons["idea-workspace-root-save"].firstMatch
        guard saveButton.waitForExistence(timeout: 5) else { return false }

        // Wait up to 3s for SwiftUI binding propagation to enable the button.
        if !saveButton.isEnabled {
            let enabled = XCTNSPredicateExpectation(
                predicate: NSPredicate(format: "isEnabled == true"),
                object: saveButton
            )
            _ = XCTWaiter.wait(for: [enabled], timeout: 3)
        }

        if !saveButton.isEnabled {
            // Save still disabled — check if the value silently matched (already saved).
            let updatedValue = (pathField.value as? String)?
                .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            return updatedValue == path
        }
        saveButton.click()

        let status = app.staticTexts["idea-workspace-root-status"].firstMatch
        if status.waitForExistence(timeout: 10) {
            return true
        }

        let fieldHasPath = XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "value CONTAINS %@", path),
            object: pathField
        )
        if XCTWaiter.wait(for: [fieldHasPath], timeout: 3) == .completed, !saveButton.isEnabled {
            return true
        }

        return false
    }
}
