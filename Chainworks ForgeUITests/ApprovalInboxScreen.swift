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
        let inbox = inboxView
        let empty = emptyState
        let title = emptyTitle
        let approve = approveButton
        let reject = rejectButton
        let predicate = NSPredicate { _, _ in
            inbox.exists || empty.exists || title.exists || approve.exists || reject.exists
        }
        let expectation = XCTNSPredicateExpectation(predicate: predicate, object: nil)
        return XCTWaiter().wait(for: [expectation], timeout: timeout) == .completed
    }

    func isRendered() -> Bool {
        inboxView.exists || emptyState.exists || emptyTitle.exists || approveButton.exists || rejectButton.exists
    }
}
