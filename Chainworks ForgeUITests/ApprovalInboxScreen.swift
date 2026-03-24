import XCTest

/// Page Object for the Approvals tab — empty state and inline approval actions.
struct ApprovalInboxScreen {
    let app: XCUIApplication

    private func identifiedElement(_ identifier: String) -> XCUIElement {
        let predicate = NSPredicate(format: "identifier == %@", identifier)
        return app.descendants(matching: .any).matching(predicate).firstMatch
    }

    var inboxView: XCUIElement { identifiedElement("approval-inbox-view") }
    var emptyState: XCUIElement { identifiedElement("approval-inbox-empty-state") }
    var emptyTitle: XCUIElement { app.staticTexts["No Pending Approvals"].firstMatch }
    var approveButton: XCUIElement { app.buttons["approval-approve-button"].firstMatch }
    var rejectButton: XCUIElement { app.buttons["approval-reject-button"].firstMatch }

    @discardableResult
    func waitForRendered(timeout: TimeInterval = 10) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if isRendered() {
                return true
            }
            RunLoop.current.run(until: Date().addingTimeInterval(0.25))
        }
        return isRendered()
    }

    func isRendered() -> Bool {
        inboxView.exists || emptyState.exists || emptyTitle.exists || approveButton.exists || rejectButton.exists
    }
}
