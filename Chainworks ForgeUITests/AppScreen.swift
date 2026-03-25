import XCTest

/// Page Object for the main app shell — tab navigation and bootstrap waiting.
/// Uses `.windows.firstMatch` to scope queries and avoid multiple-window
/// ambiguity caused by macOS scene restoration across test runs.
struct AppScreen {
    let app: XCUIApplication

    private static let knownTabLabels = ["Runs Home", "Ideas", "Approvals", "Agent Catalog", "Workflow Inspector", "Pilot Readiness", "Settings"]
    private static let compactNavigationLabels = [
        "Show Sidebar",
        "Hide Sidebar",
        "Toggle Sidebar",
        "Show Navigation",
        "Hide Navigation",
        "Sidebar"
    ]

    /// The primary app window. macOS may restore previous windows from scene state,
    /// so we always scope element queries to a single window to prevent ambiguity.
    private var primaryWindow: XCUIElement {
        app.windows.firstMatch
    }

    private func tabCandidates(_ label: String) -> [XCUIElement] {
        let win = primaryWindow
        let beginsWith = NSPredicate(format: "label BEGINSWITH %@", label)

        return [
            win.radioButtons[label].firstMatch,
            win.tabs[label].firstMatch,
            win.buttons[label].firstMatch,
            win.staticTexts[label].firstMatch,
            win.outlines.staticTexts[label].firstMatch,
            win.radioButtons.matching(beginsWith).firstMatch,
            win.tabs.matching(beginsWith).firstMatch,
            win.buttons.matching(beginsWith).firstMatch,
            win.staticTexts.matching(beginsWith).firstMatch,
            win.outlines.staticTexts.matching(beginsWith).firstMatch,
            win.descendants(matching: .any).matching(beginsWith).firstMatch
        ]
    }

    private func tabsVisible() -> Bool {
        Self.knownTabLabels.contains { label in
            tabCandidates(label).contains(where: \.exists)
        }
    }

    private func compactNavigationToggleCandidates() -> [XCUIElement] {
        let win = primaryWindow
        let navPredicate = NSPredicate(
            format: "label CONTAINS[c] 'sidebar' OR label CONTAINS[c] 'navigation' OR identifier CONTAINS[c] 'sidebar' OR identifier CONTAINS[c] 'navigation'"
        )

        var candidates: [XCUIElement] = []

        for label in Self.compactNavigationLabels {
            candidates.append(win.buttons[label].firstMatch)
            candidates.append(win.toolbars.buttons[label].firstMatch)
        }

        candidates.append(win.buttons.matching(navPredicate).firstMatch)
        candidates.append(win.toolbars.buttons.matching(navPredicate).firstMatch)

        let windowFrame = win.frame
        if !windowFrame.isEmpty {
            let heuristicButtons = win.buttons.allElementsBoundByIndex.filter { button in
                guard button.exists, button.isHittable else { return false }
                let frame = button.frame
                guard !frame.isEmpty else { return false }
                guard frame.width <= 72, frame.height <= 72 else { return false }
                guard frame.minY < windowFrame.minY + 140 else { return false }
                return frame.midX > windowFrame.midX
            }
            candidates.append(contentsOf: heuristicButtons)
        }

        return candidates
    }

    @discardableResult
    private func revealCompactNavigationIfNeeded() -> Bool {
        guard !tabsVisible() else { return true }

        for candidate in compactNavigationToggleCandidates() {
            guard candidate.exists, candidate.isHittable else { continue }
            candidate.click()
            RunLoop.current.run(until: Date().addingTimeInterval(0.5))
            if tabsVisible() {
                return true
            }
        }

        return tabsVisible()
    }

    private func expectedRootVisible(for label: String) -> Bool {
        switch label {
        case "Runs Home":
            return app.otherElements["runs-home-list"].exists
        case "Ideas":
            return app.otherElements["ideas-root-view"].exists || app.otherElements["idea-list"].exists
        case "Approvals":
            return app.otherElements["approval-inbox-view"].exists
                || app.otherElements["approval-inbox-empty-state"].exists
                || app.buttons["approval-approve-button"].exists
        case "Agent Catalog":
            return app.otherElements["agent-catalog-count"].exists
        case "Workflow Inspector":
            return app.otherElements["workflow-state-count"].exists
        case "Pilot Readiness":
            return app.otherElements["pilot-readiness-view"].exists
                || app.otherElements["pilot-readiness-title"].exists
        case "Settings":
            return app.otherElements["provider-settings-view"].exists
                || app.otherElements["provider-settings-title"].exists
        default:
            return false
        }
    }

    /// Waits for the ContentView TabView to render by looking for a known tab label.
    /// macOS SwiftUI TabView renders tabs as radio buttons; in some environments
    /// they may appear as `.tabs` instead.
    @discardableResult
    func waitForTabs(timeout: TimeInterval = 30) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if tabsVisible() {
                return true
            }
            _ = revealCompactNavigationIfNeeded()
            if tabsVisible() {
                return true
            }
            RunLoop.current.run(until: Date().addingTimeInterval(0.25))
        }
        return tabsVisible()
    }

    /// Finds a tab by label scoped to the primary window.
    /// Tries exact match on radioButtons, tabs, buttons first.
    /// Falls back to CONTAINS-based predicate matching to handle macOS SwiftUI
    /// badge-modified accessibility labels (e.g. "Approvals" → "Approvals, 1 item").
    func tab(_ label: String) -> XCUIElement {
        _ = revealCompactNavigationIfNeeded()
        return tabCandidates(label).first(where: \.exists) ?? primaryWindow.buttons[label].firstMatch
    }

    @discardableResult
    func selectTab(_ label: String, timeout: TimeInterval = 10) -> Bool {
        _ = revealCompactNavigationIfNeeded()
        let target = tab(label)
        guard target.waitForExistence(timeout: timeout) else { return false }

        if isTabSelected(label) { return true }
        if target.isEnabled || target.isHittable { target.click() }

        if expectedRootVisible(for: label) {
            return true
        }

        let predicate = NSPredicate { _, _ in
            if let number = target.value as? NSNumber { return number.intValue == 1 }
            if let string = target.value as? String { return string == "1" || string.lowercased() == "selected" }
            return self.expectedRootVisible(for: label)
        }
        let expectation = XCTNSPredicateExpectation(predicate: predicate, object: nil)
        if XCTWaiter().wait(for: [expectation], timeout: timeout) == .completed {
            return true
        }

        // Single retry — badge-modified accessibility labels may need a second click
        _ = revealCompactNavigationIfNeeded()
        if target.isEnabled || target.isHittable { target.click() }
        return isTabSelected(label) || expectedRootVisible(for: label)
    }

    private func isTabSelected(_ label: String) -> Bool {
        let target = tab(label)
        if let number = target.value as? NSNumber {
            return number.intValue == 1
        }
        if let string = target.value as? String {
            return string == "1" || string.lowercased() == "selected"
        }
        return false
    }
}
