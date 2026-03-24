import XCTest

/// Page Object for the main app shell — tab navigation and bootstrap waiting.
struct AppScreen {
    let app: XCUIApplication

    private static let knownTabLabels = ["Ideas", "Approvals", "Agent Catalog", "Workflow Inspector"]

    /// Waits for the ContentView TabView to render by looking for a known tab label.
    /// macOS SwiftUI TabView renders tabs as radio buttons; in some environments
    /// they may appear as `.tabs` instead.
    @discardableResult
    func waitForTabs(timeout: TimeInterval = 30) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            for label in Self.knownTabLabels {
                if app.radioButtons[label].exists { return true }
                if app.tabs[label].exists { return true }
            }
            RunLoop.current.run(until: Date().addingTimeInterval(0.5))
        }
        return false
    }

    /// Finds a tab by label. Tries radioButtons, tabs, buttons, then predicate fallback.
    func tab(_ label: String) -> XCUIElement {
        let radio = app.radioButtons[label]
        if radio.exists { return radio }
        let t = app.tabs[label]
        if t.exists { return t }
        let btn = app.buttons[label]
        if btn.exists { return btn }
        return app.buttons[label].firstMatch
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
