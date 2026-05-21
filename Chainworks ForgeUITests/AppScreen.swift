import XCTest

/// Page Object for the main app shell — tab navigation and bootstrap waiting.
/// Uses `.windows.firstMatch` to scope queries and avoid multiple-window
/// ambiguity caused by macOS scene restoration across test runs.
struct AppScreen {
    let app: XCUIApplication

    // P036 consolidated tabs are listed first so waitForTabs detects the new shell immediately.
    private static let knownTabLabels = ["Runs", "Ideas", "Definitions", "Settings", "Runs Home", "Approvals", "Agent Catalog", "Workflow Inspector", "Pilot Readiness"]
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

    private func identifiedAny(_ identifier: String) -> XCUIElement {
        app.descendants(matching: .any)
            .matching(NSPredicate(format: "identifier == %@", identifier))
            .firstMatch
    }

    private func tabIdentifier(for label: String) -> String? {
        switch label {
        // P036 consolidated tab identifiers
        case "Runs":
            return "p036-sidebar-runs"
        case "Ideas":
            return "p036-sidebar-ideas"
        case "Definitions":
            return "p036-sidebar-definitions"
        case "Settings":
            return "p036-sidebar-settings"
        // Legacy tab identifiers
        case "Runs Home":
            return "tab-runs-home"
        case "Approvals":
            return "tab-approvals"
        case "Agent Catalog":
            return "tab-agent-catalog"
        case "Workflow Inspector":
            return "tab-workflow-inspector"
        case "Pilot Readiness":
            return "tab-pilot-readiness"
        default:
            return nil
        }
    }

    private func tabCandidates(_ label: String) -> [XCUIElement] {
        let win = primaryWindow
        let beginsWith = NSPredicate(format: "label BEGINSWITH %@", label)
        let identifierPredicate = tabIdentifier(for: label).map { NSPredicate(format: "identifier == %@", $0) }
        let identifierMatches: [XCUIElement]
        if let identifierPredicate {
            identifierMatches = [
                win.descendants(matching: .radioButton).matching(identifierPredicate).firstMatch,
                win.descendants(matching: .tab).matching(identifierPredicate).firstMatch,
                win.descendants(matching: .button).matching(identifierPredicate).firstMatch,
                win.descendants(matching: .staticText).matching(identifierPredicate).firstMatch,
                app.descendants(matching: .any).matching(identifierPredicate).firstMatch
            ]
        } else {
            identifierMatches = []
        }

        return identifierMatches + [
            win.descendants(matching: .scrollView).matching(beginsWith).firstMatch,
            win.descendants(matching: .outline).matching(beginsWith).firstMatch,
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
        guard win.exists else {
            return [
                app.buttons.matching(
                    NSPredicate(
                        format: "label CONTAINS[c] 'sidebar' OR label CONTAINS[c] 'navigation' OR identifier CONTAINS[c] 'sidebar' OR identifier CONTAINS[c] 'navigation'"
                    )
                ).firstMatch
            ]
        }
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
        // P036 consolidated tabs
        case "Runs":
            return identifiedAny("runs-home-owner-view").exists
                || identifiedAny("runs-home-list").exists
                || identifiedAny("runs-home-section-waiting-approval").exists
                || identifiedAny("run-detail-panel").exists
                || primaryWindow.staticTexts["Waiting Approval"].exists
        case "Definitions":
            return identifiedAny("definitions-view").exists
                || identifiedAny("agent-catalog-view").exists
                || identifiedAny("workflow-state-list").exists
        // Legacy tabs
        case "Runs Home":
            return identifiedAny("runs-home-owner-view").exists
                || identifiedAny("runs-home-list").exists
                || identifiedAny("runs-home-section-waiting-approval").exists
                || identifiedAny("run-detail-panel").exists
                || primaryWindow.staticTexts["Waiting Approval"].exists
        case "Ideas":
            return identifiedAny("ideas-root-view").exists
                || identifiedAny("idea-list").exists
                || identifiedAny("ideas-open-archive").exists
                || identifiedAny("ideas-summary-open-archive").exists
                || identifiedAny("ideas-new-idea").exists
                || identifiedAny("ideas-new-idea-inline").exists
        case "Approvals":
            return identifiedAny("approval-inbox-view").exists
                || identifiedAny("approval-inbox-empty-state").exists
                || identifiedAny("approval-gate-view").exists
                || app.buttons["approval-approve-button"].exists
        case "Agent Catalog":
            return identifiedAny("agent-catalog-count").exists
        case "Workflow Inspector":
            return identifiedAny("workflow-state-count").exists
        case "Pilot Readiness":
            return identifiedAny("pilot-readiness-view").exists
                || identifiedAny("pilot-readiness-title").exists
        case "Settings":
            return identifiedAny("settings-view").exists
                || identifiedAny("system-readiness-view").exists
                || identifiedAny("provider-settings-view").exists
                || identifiedAny("provider-settings-title").exists
        default:
            return false
        }
    }

    private func clickRepresentative(for target: XCUIElement, label: String) {
        let identifier = tabIdentifier(for: label)
        let queries: [XCUIElement] = [
            target.descendants(matching: .radioButton).matching(NSPredicate(format: "label BEGINSWITH %@", label)).firstMatch,
            target.descendants(matching: .button).matching(NSPredicate(format: "label BEGINSWITH %@", label)).firstMatch,
            target.descendants(matching: .staticText).matching(NSPredicate(format: "label BEGINSWITH %@", label)).firstMatch,
            target.descendants(matching: .any).matching(NSPredicate(format: "label BEGINSWITH %@", label)).firstMatch
        ] + (identifier.map { id in
            [
                target.descendants(matching: .radioButton).matching(NSPredicate(format: "identifier == %@", id)).firstMatch,
                target.descendants(matching: .button).matching(NSPredicate(format: "identifier == %@", id)).firstMatch,
                target.descendants(matching: .staticText).matching(NSPredicate(format: "identifier == %@", id)).firstMatch,
                target.descendants(matching: .any).matching(NSPredicate(format: "identifier == %@", id)).firstMatch
            ]
        } ?? [])

        if let child = queries.first(where: { $0.exists && $0.isHittable }) {
            child.click()
            return
        }

        if target.isHittable || target.isEnabled {
            target.click()
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
        if expectedRootVisible(for: label) {
            return true
        }
        let target = tab(label)
        guard target.waitForExistence(timeout: timeout) else { return false }

        if isTabSelected(label) { return true }
        clickRepresentative(for: target, label: label)

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
        clickRepresentative(for: target, label: label)
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
