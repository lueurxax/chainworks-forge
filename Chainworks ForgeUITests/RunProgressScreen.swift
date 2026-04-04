import XCTest

/// Page Object for the Run Progress view — monitoring, approval, artifacts.
struct RunProgressScreen {
    let app: XCUIApplication

    private func identifiedAny(_ identifier: String) -> XCUIElement {
        app.descendants(matching: .any)
            .matching(NSPredicate(format: "identifier == %@", identifier))
            .firstMatch
    }

    /// Navigates to run progress view if not already visible.
    @discardableResult
    func openIfNeeded(workflowTitle: String, timeout: TimeInterval = 15) -> Bool {
        if hasProgressSurface(timeout: 3) { return true }

        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            let candidates = [
                app.buttons["run-row-\(workflowTitle)"].firstMatch,
                app.links["run-row-\(workflowTitle)"].firstMatch,
                app.otherElements["run-row-\(workflowTitle)"].firstMatch,
                app.buttons[workflowTitle].firstMatch,
                app.links[workflowTitle].firstMatch,
                app.staticTexts[workflowTitle].firstMatch
            ]

            for candidate in candidates {
                if candidate.waitForExistence(timeout: 1) {
                    candidate.click()
                    if hasProgressSurface(timeout: 3) { return true }
                }
            }

            if hasProgressSurface(timeout: 1) {
                return true
            }

            app.typeKey(.pageDown, modifierFlags: [])
            RunLoop.current.run(until: Date().addingTimeInterval(0.3))
        }

        return hasProgressSurface(timeout: 2)
    }

    /// The Approve button in the run progress view.
    var approveButton: XCUIElement {
        let identified = app.buttons["approval-approve-button"].firstMatch
        return identified.exists ? identified : app.buttons["Approve"].firstMatch
    }

    @discardableResult
    func revealApprovalButton(timeout: TimeInterval = 6) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        let button = approveButton
        let window = app.windows.firstMatch
        while Date() < deadline {
            if button.exists && button.isHittable {
                return true
            }

            let scrollView = app.scrollViews.firstMatch
            let windowFrame = window.exists ? window.frame : .zero
            let buttonFrame = button.exists ? button.frame : .zero

            let shouldSwipeDown: Bool
            if !windowFrame.isEmpty, !buttonFrame.isEmpty {
                shouldSwipeDown = buttonFrame.maxY < windowFrame.minY + 80
            } else {
                shouldSwipeDown = false
            }

            if scrollView.exists {
                if shouldSwipeDown {
                    scrollView.swipeDown()
                } else {
                    scrollView.swipeUp()
                }
            } else if shouldSwipeDown {
                app.swipeDown()
            } else {
                app.swipeUp()
            }
            RunLoop.current.run(until: Date().addingTimeInterval(0.4))
        }

        return button.exists && button.isHittable
    }

    /// Whether the run progress surface is visible in the current UI hierarchy.
    @discardableResult
    func isVisible(timeout: TimeInterval = 3) -> Bool {
        hasProgressSurface(timeout: timeout)
    }

    @discardableResult
    func selectPane(_ title: String, timeout: TimeInterval = 5) -> Bool {
        let segmentedCandidates = [
            app.segmentedControls["run-progress-pane-picker"].firstMatch,
            app.otherElements["run-progress-pane-picker"].firstMatch,
            app.descendants(matching: .any)
                .matching(NSPredicate(format: "identifier == %@", "run-progress-pane-picker"))
                .firstMatch
        ]

        let buttonCandidates = [
            app.segmentedControls.buttons[title].firstMatch,
            app.buttons[title].firstMatch,
            app.radioButtons[title].firstMatch,
            app.descendants(matching: .any)
                .matching(NSPredicate(format: "label == %@ OR value == %@", title, title))
                .firstMatch
        ]

        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if buttonCandidates.contains(where: { $0.exists && $0.isHittable }) {
                buttonCandidates.first(where: { $0.exists && $0.isHittable })?.click()
                RunLoop.current.run(until: Date().addingTimeInterval(0.3))
                return true
            }

            if segmentedCandidates.contains(where: \.exists) {
                app.typeKey(.tab, modifierFlags: [])
                RunLoop.current.run(until: Date().addingTimeInterval(0.2))
            }
            RunLoop.current.run(until: Date().addingTimeInterval(0.2))
        }

        return buttonCandidates.contains(where: { $0.exists && $0.isHittable })
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
        let statusElement = app.descendants(matching: .staticText)
            .matching(NSPredicate(format: "identifier BEGINSWITH %@", "run-status-"))
            .firstMatch
        let predicate = NSPredicate { _, _ in
            guard statusElement.exists else { return false }
            let id = statusElement.identifier
            guard id.hasPrefix("run-status-") else { return false }
            return statuses.contains(String(id.dropFirst("run-status-".count)))
        }
        let expectation = XCTNSPredicateExpectation(predicate: predicate, object: nil)
        guard XCTWaiter().wait(for: [expectation], timeout: timeout) == .completed else {
            return nil
        }
        return currentRunStatus()
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
        let progressOutline = app.outlines["run-progress-view"].firstMatch
        let progressOther = app.otherElements["run-progress-view"].firstMatch
        let progressAny = identifiedAny("run-progress-view")
        let statusLabel = app.descendants(matching: .staticText)
            .matching(NSPredicate(format: "identifier BEGINSWITH %@", "run-status-"))
            .firstMatch
        let sectionTitles = ["Summary", "Progress", "Artifacts", "Approvals", "Workflow Map", "Approval Gate"]
        let sections = sectionTitles.map { sectionLabel($0) }

        let predicate = NSPredicate { _, _ in
            if progressOutline.exists || progressOther.exists || progressAny.exists || statusLabel.exists { return true }
            return sections.contains { $0.exists }
        }
        let expectation = XCTNSPredicateExpectation(predicate: predicate, object: nil)
        return XCTWaiter().wait(for: [expectation], timeout: timeout) == .completed
    }

    private func sectionLabel(_ name: String) -> XCUIElement {
        let predicate = NSPredicate(format: "label == %@ OR value == %@", name, name)
        return app.descendants(matching: .staticText).matching(predicate).firstMatch
    }
}
