import XCTest

/// Page Object for the main app shell — tab navigation and bootstrap waiting.
/// Uses `.windows.firstMatch` to scope queries and avoid multiple-window
/// ambiguity caused by macOS scene restoration across test runs.
struct AppScreen {
    let app: XCUIApplication

    private static let knownTabLabels = ["Runs Home", "Ideas", "Approvals", "Agent Catalog", "Workflow Inspector", "Pilot Readiness", "Settings"]

    /// The primary app window. macOS may restore previous windows from scene state,
    /// so we always scope element queries to a single window to prevent ambiguity.
    private var primaryWindow: XCUIElement {
        app.windows.firstMatch
    }

    /// Waits for the ContentView TabView to render by looking for a known tab label.
    /// macOS SwiftUI TabView renders tabs as radio buttons; in some environments
    /// they may appear as `.tabs` instead.
    @discardableResult
    func waitForTabs(timeout: TimeInterval = 30) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            let win = primaryWindow
            for label in Self.knownTabLabels {
                if win.radioButtons[label].exists { return true }
                if win.tabs[label].exists { return true }
            }
            RunLoop.current.run(until: Date().addingTimeInterval(0.5))
        }
        return false
    }

    /// Finds a tab by label scoped to the primary window.
    /// Tries exact match on radioButtons, tabs, buttons first.
    /// Falls back to CONTAINS-based predicate matching to handle macOS SwiftUI
    /// badge-modified accessibility labels (e.g. "Approvals" → "Approvals, 1 item").
    func tab(_ label: String) -> XCUIElement {
        let win = primaryWindow
        // Exact match
        let radio = win.radioButtons[label]
        if radio.exists { return radio }
        let t = win.tabs[label]
        if t.exists { return t }
        let btn = win.buttons[label]
        if btn.exists { return btn }
        // CONTAINS fallback for badge-modified labels
        let predicate = NSPredicate(format: "label BEGINSWITH %@", label)
        let radioMatch = win.radioButtons.matching(predicate).firstMatch
        if radioMatch.exists { return radioMatch }
        let tabMatch = win.tabs.matching(predicate).firstMatch
        if tabMatch.exists { return tabMatch }
        let btnMatch = win.buttons.matching(predicate).firstMatch
        if btnMatch.exists { return btnMatch }
        return win.buttons[label].firstMatch
    }

    @discardableResult
    func selectTab(_ label: String, timeout: TimeInterval = 10) -> Bool {
        let target = tab(label)
        guard target.waitForExistence(timeout: timeout) else { return false }

        if isTabSelected(label) {
            return true
        }

        if target.isEnabled {
            target.click()
        }

        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if isTabSelected(label) {
                return true
            }
            if target.isEnabled {
                target.click()
            }
            RunLoop.current.run(until: Date().addingTimeInterval(0.25))
        }
        return isTabSelected(label)
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
