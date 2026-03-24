import XCTest

/// Page Object for the Run Progress view — monitoring, approval, artifacts.
struct RunProgressScreen {
    let app: XCUIApplication

    /// Navigates to run progress view if not already visible.
    @discardableResult
    func openIfNeeded(workflowTitle: String, timeout: TimeInterval = 15) -> Bool {
        if hasProgressSurface(timeout: 3) { return true }

        let candidates = [
            app.windows[workflowTitle].firstMatch,
            app.buttons["run-row-\(workflowTitle)"].firstMatch,
            app.links["run-row-\(workflowTitle)"].firstMatch,
            app.otherElements["run-row-\(workflowTitle)"].firstMatch,
            app.buttons[workflowTitle].firstMatch,
            app.links[workflowTitle].firstMatch,
            app.staticTexts[workflowTitle].firstMatch
        ]

        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if hasProgressSurface(timeout: 1) { return true }

            for candidate in candidates {
                if candidate.exists || candidate.waitForExistence(timeout: 1) {
                    candidate.click()
                    if hasProgressSurface(timeout: 5) {
                        return true
                    }
                }
            }

            RunLoop.current.run(until: Date().addingTimeInterval(0.25))
        }

        return hasProgressSurface(timeout: 1)
    }

    /// The Approve button in the run progress view.
    var approveButton: XCUIElement { app.buttons["Approve"].firstMatch }

    /// Whether the run progress surface is visible in the current UI hierarchy.
    @discardableResult
    func isVisible(timeout: TimeInterval = 3) -> Bool {
        hasProgressSurface(timeout: timeout)
    }

    /// Whether any run status label is visible in the current progress surface.
    func hasRunStatus(timeout: TimeInterval = 2) -> Bool {
        app.descendants(matching: .staticText)
            .matching(NSPredicate(format: "identifier BEGINSWITH %@", "run-status-"))
            .firstMatch
            .waitForExistence(timeout: timeout)
    }

    /// Returns the current run status string if the progress surface exposes one.
    func currentRunStatus() -> String? {
        let statusLabel = app.descendants(matching: .staticText)
            .matching(NSPredicate(format: "identifier BEGINSWITH %@", "run-status-"))
            .firstMatch
        guard statusLabel.exists || statusLabel.waitForExistence(timeout: 1) else { return nil }
        let identifier = statusLabel.identifier
        if identifier.hasPrefix("run-status-") {
            return String(identifier.dropFirst("run-status-".count))
        }
        if let value = statusLabel.value as? String, !value.isEmpty {
            return value
        }
        return nil
    }

    /// Waits for the run to enter one of the expected statuses.
    func waitForRunStatus(_ statuses: Set<String>, timeout: TimeInterval = 20) -> String? {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if let status = currentRunStatus(), statuses.contains(status) {
                return status
            }
            RunLoop.current.run(until: Date().addingTimeInterval(0.25))
        }
        if let status = currentRunStatus(), statuses.contains(status) {
            return status
        }
        return nil
    }

    /// Checks if a named section label exists.
    func hasSection(_ name: String) -> Bool {
        sectionLabel(name).exists
    }

    /// Waits for a named section to appear.
    func waitForSection(_ name: String, timeout: TimeInterval = 5) -> Bool {
        sectionLabel(name).waitForExistence(timeout: timeout)
    }

    func hasArtifactNamed(_ name: String) -> Bool {
        let predicate = NSPredicate(format: "label CONTAINS %@", name)
        return app.buttons.matching(predicate).firstMatch.exists
    }

    private func hasProgressSurface(timeout: TimeInterval) -> Bool {
        let progressCandidates = [
            app.outlines["run-progress-view"].firstMatch,
            app.otherElements["run-progress-view"].firstMatch
        ]
        for candidate in progressCandidates where candidate.waitForExistence(timeout: timeout) {
            return true
        }

        let statusLabel = app.descendants(matching: .staticText)
            .matching(NSPredicate(format: "identifier BEGINSWITH %@", "run-status-"))
            .firstMatch
        if statusLabel.waitForExistence(timeout: timeout) {
            return true
        }

        let sectionTitles = ["Overview", "Current Phase", "Stages", "Live Timeline", "Active Agents", "Artifacts", "Approval Gate"]
        return sectionTitles.contains { title in
            sectionLabel(title).waitForExistence(timeout: 1)
        }
    }

    private func sectionLabel(_ name: String) -> XCUIElement {
        let predicate = NSPredicate(format: "label == %@ OR value == %@", name, name)
        return app.descendants(matching: .staticText).matching(predicate).firstMatch
    }
}
