import XCTest

/// Page Object for the Start Run sheet — mode selection and run launch.
struct StartRunScreen {
    let app: XCUIApplication

    /// Selects Live execution mode. Returns true if Live mode was found and selected.
    func selectLiveMode() -> Bool {
        let candidates = [
            app.radioButtons["execution-mode-live"].firstMatch,
            app.buttons["execution-mode-live"].firstMatch,
            app.buttons["Live"].firstMatch,
            app.radioButtons["Live"].firstMatch,
            app.segmentedControls.buttons["Live"].firstMatch
        ]
        for candidate in candidates {
            if candidate.waitForExistence(timeout: 2) {
                candidate.click()
                return true
            }
        }
        let predicate = NSPredicate(format: "label == %@ AND isEnabled == true", "Live")
        let fallback = app.descendants(matching: .any).matching(predicate).firstMatch
        guard fallback.waitForExistence(timeout: 2) else { return false }
        fallback.click()
        return true
    }

    /// The Start Run confirmation button.
    var startRunButton: XCUIElement { app.buttons["Start Run"].firstMatch }

    /// The Cancel button.
    var cancelButton: XCUIElement { app.buttons["Cancel"].firstMatch }

    /// The Compile button.
    var compileButton: XCUIElement { app.buttons["Compile"].firstMatch }

    /// Dismisses the sheet via Cancel or Escape.
    func dismiss() {
        if cancelButton.exists { cancelButton.click() }
        else { app.typeKey(.escape, modifierFlags: []) }
    }
}
