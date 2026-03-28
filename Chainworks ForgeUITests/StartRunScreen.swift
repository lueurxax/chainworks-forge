import XCTest

/// Page Object for the Start Run sheet — mode selection and run launch.
struct StartRunScreen {
    let app: XCUIApplication

    private var notificationCenterApp: XCUIApplication {
        XCUIApplication(bundleIdentifier: "com.apple.UserNotificationCenter")
    }

    private var startRunBlockingValue: String? {
        let button = startRunButton
        guard button.exists else { return nil }
        return button.value as? String
    }

    private func workflowSelectionConfirmed(for title: String, timeout: TimeInterval = 3) -> Bool {
        let selectionMap = [
            "Full MVP (Live)": "fullMVPLive",
            "Proposal Loop (Live)": "proposalLoopLive",
            "Canonical Workflow": "canonicalRelease"
        ]
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if let expectedID = selectionMap[title] {
                let list = app.otherElements["workflow-preset-list"].firstMatch
                let selectedValue = (list.value as? String) ?? ""
                if selectedValue == expectedID {
                    return true
                }
            }
            switch title {
            case "Full MVP (Live)":
                if app.otherElements["delivery-configuration-section"].firstMatch.exists
                    || app.buttons["delivery-preflight-button"].firstMatch.exists
                    || app.buttons["Review"].firstMatch.exists {
                    return true
                }
            case "Proposal Loop (Live)":
                if app.otherElements["delivery-configuration-section"].firstMatch.exists == false,
                   (app.buttons["workflow-start-run-confirm-button"].firstMatch.exists
                    || app.buttons["Start Run"].firstMatch.exists) {
                    return true
                }
            case "Canonical Workflow":
                if app.buttons["workflow-start-run-confirm-button"].firstMatch.exists
                    || app.buttons["Start Run"].firstMatch.exists {
                    return true
                }
            default:
                break
            }
            RunLoop.current.run(until: Date().addingTimeInterval(0.2))
        }
        return false
    }

    private func waitForLiveWorkflowChoices(timeout: TimeInterval = 5) -> Bool {
        let signals = [
            app.buttons["workflow-preset-button-proposalLoopLive"].firstMatch,
            app.buttons["workflow-preset-button-fullMVPLive"].firstMatch,
            app.descendants(matching: .any)
                .matching(NSPredicate(format: "identifier == %@ AND label == %@", "workflow-preset-list", "Proposal Loop (Live)"))
                .firstMatch,
            app.descendants(matching: .any)
                .matching(NSPredicate(format: "identifier == %@ AND label == %@", "workflow-preset-list", "Full MVP (Live)"))
                .firstMatch,
            app.otherElements["live-runtime-missing-block"].firstMatch,
            app.staticTexts["Full MVP (Live)"].firstMatch,
            app.staticTexts["Proposal Loop (Live)"].firstMatch
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

    /// Selects Live execution mode. Returns true if Live mode was found and selected.
    func selectLiveMode() -> Bool {
        if waitForLiveWorkflowChoices(timeout: 1) {
            return true
        }
        let candidates = [
            app.buttons["execution-mode-live-button"].firstMatch,
            app.descendants(matching: .any)
                .matching(NSPredicate(format: "identifier == %@ AND label == %@", "execution-mode-list", "Live"))
                .firstMatch,
            app.radioButtons["execution-mode-live"].firstMatch,
            app.buttons["execution-mode-live"].firstMatch,
            app.buttons["Live"].firstMatch,
            app.radioButtons["Live"].firstMatch,
            app.segmentedControls.buttons["Live"].firstMatch
        ]
        for candidate in candidates {
            if candidate.waitForExistence(timeout: 2) {
                candidate.click()
                return waitForLiveWorkflowChoices()
            }
        }
        let predicate = NSPredicate(format: "label == %@ AND isEnabled == true", "Live")
        let fallback = app.descendants(matching: .any).matching(predicate).firstMatch
        guard fallback.waitForExistence(timeout: 2) else { return false }
        fallback.click()
        return waitForLiveWorkflowChoices()
    }

    @discardableResult
    func selectWorkflow(_ title: String) -> Bool {
        if workflowSelectionConfirmed(for: title, timeout: 3) {
            return true
        }
        let buttonMap = [
            "Full MVP (Live)": "workflow-preset-button-fullMVPLive",
            "Proposal Loop (Live)": "workflow-preset-button-proposalLoopLive",
            "Canonical Workflow": "workflow-preset-single"
        ]
        if let identifier = buttonMap[title] {
            let presetList = app.otherElements["workflow-preset-list"].firstMatch
            let directButton = app.buttons[identifier].firstMatch
            let directLabelButton = app.buttons[title].firstMatch

            if directButton.waitForExistence(timeout: 3) {
                clickWorkflowButton(directButton)
                if workflowSelectionConfirmed(for: title, timeout: 2) {
                    return true
                }
            }

            if presetList.waitForExistence(timeout: 2) {
                let scopedButton = presetList.buttons[identifier].firstMatch
                if scopedButton.exists {
                    clickWorkflowButton(scopedButton)
                    if workflowSelectionConfirmed(for: title, timeout: 2) {
                        return true
                    }
                }

                let labelButton = presetList.buttons[title].firstMatch
                if labelButton.exists {
                    clickWorkflowButton(labelButton)
                    if workflowSelectionConfirmed(for: title, timeout: 2) {
                        return true
                    }
                }

                if clickWorkflowRow(in: presetList, title: title),
                   workflowSelectionConfirmed(for: title, timeout: 2) {
                    return true
                }
            }

            if directLabelButton.exists {
                clickWorkflowButton(directLabelButton)
                if workflowSelectionConfirmed(for: title, timeout: 2) {
                    return true
                }
            }

            let directCandidates = [
                app.otherElements[identifier].firstMatch,
                app.staticTexts[title].firstMatch,
                app.descendants(matching: .any)
                    .matching(NSPredicate(format: "label == %@", title))
                    .firstMatch
            ]
            let directDeadline = Date().addingTimeInterval(1.5)
            while Date() < directDeadline {
                for candidate in directCandidates where candidate.exists {
                    clickWorkflowButton(candidate)
                    return workflowSelectionConfirmed(for: title)
                }
                RunLoop.current.run(until: Date().addingTimeInterval(0.15))
            }
        }

        let popupCandidates = [
            app.popUpButtons["workflow-preset-picker"].firstMatch,
            app.buttons["workflow-preset-picker"].firstMatch
        ]

        for popup in popupCandidates where popup.waitForExistence(timeout: 2) {
            if popup.label.contains(title) {
                return workflowSelectionConfirmed(for: title)
            }
            if let value = popup.value as? String, value.contains(title) {
                return workflowSelectionConfirmed(for: title)
            }
            popup.click()
            let menuItem = app.menuItems.matching(NSPredicate(format: "label CONTAINS %@", title)).firstMatch
            if menuItem.waitForExistence(timeout: 3) {
                menuItem.click()
                return workflowSelectionConfirmed(for: title)
            }
            if title == "Full MVP (Live)" {
                app.typeKey(.downArrow, modifierFlags: [])
                app.typeKey(.return, modifierFlags: [])
                RunLoop.current.run(until: Date().addingTimeInterval(0.3))
                if popup.label.contains(title) {
                    return workflowSelectionConfirmed(for: title)
                }
                if let value = popup.value as? String, value.contains(title) {
                    return workflowSelectionConfirmed(for: title)
                }
            }
            app.typeKey(.escape, modifierFlags: [])
        }

        let direct = app.staticTexts[title].firstMatch
        if direct.waitForExistence(timeout: 2) {
            direct.click()
            return workflowSelectionConfirmed(for: title)
        }
        return false
    }

    @discardableResult
    private func clickWorkflowRow(in list: XCUIElement, title: String) -> Bool {
        guard list.exists else { return false }

        let yOffset: CGFloat
        switch title {
        case "Proposal Loop (Live)":
            yOffset = 0.28
        case "Full MVP (Live)":
            yOffset = 0.72
        default:
            yOffset = 0.5
        }

        let coordinate = list.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: yOffset))
        coordinate.click()
        return true
    }

    private func clickWorkflowButton(_ button: XCUIElement) {
        if button.isHittable {
            button.click()
            return
        }

        button.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).click()
    }

    @discardableResult
    func runDeliveryPreflightIfNeeded() -> Bool {
        let reviewButton = app.buttons["Review"].firstMatch
        if reviewButton.exists {
            return true
        }

        let deadline = Date().addingTimeInterval(8)
        while Date() < deadline {
            if reviewButton.exists {
                return true
            }
            let preflightButton = app.buttons["delivery-preflight-button"].firstMatch
            if preflightButton.exists {
                break
            }
            _ = dismissSystemNotificationDialogIfNeeded()
            app.typeKey(.pageDown, modifierFlags: [])
            RunLoop.current.run(until: Date().addingTimeInterval(0.4))
        }

        _ = dismissSystemNotificationDialogIfNeeded()
        let preflightButton = app.buttons["delivery-preflight-button"].firstMatch
        guard preflightButton.waitForExistence(timeout: 2) else { return false }
        if !preflightButton.isHittable {
            app.typeKey(.pageDown, modifierFlags: [])
            RunLoop.current.run(until: Date().addingTimeInterval(0.3))
            _ = dismissSystemNotificationDialogIfNeeded()
        }
        preflightButton.click()
        return reviewButton.waitForExistence(timeout: 15)
    }

    @discardableResult
    private func dismissSystemNotificationDialogIfNeeded(timeout: TimeInterval = 3) -> Bool {
        let dialog = notificationCenterApp.dialogs.firstMatch
        let alert = notificationCenterApp.alerts.firstMatch
        let target = dialog.exists ? dialog : alert
        guard target.exists || target.waitForExistence(timeout: 0.5) else { return false }

        let candidates = [
            notificationCenterApp.buttons["action-button-2"].firstMatch,
            notificationCenterApp.buttons["Don't Allow"].firstMatch,
            notificationCenterApp.buttons["Don’t Allow"].firstMatch,
            notificationCenterApp.buttons["Close"].firstMatch,
            notificationCenterApp.buttons["Not Now"].firstMatch,
            notificationCenterApp.buttons["action-button-1"].firstMatch
        ]

        for button in candidates where button.exists {
            button.click()
            let disappeared = XCTNSPredicateExpectation(
                predicate: NSPredicate(format: "exists == false"),
                object: target
            )
            return XCTWaiter().wait(for: [disappeared], timeout: timeout) == .completed
        }

        app.typeKey(.escape, modifierFlags: [])
        let disappeared = XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "exists == false"),
            object: target
        )
        return XCTWaiter().wait(for: [disappeared], timeout: timeout) == .completed
    }

    @discardableResult
    func waitForStartRunReady(timeout: TimeInterval = 30) -> Bool {
        let startButton = startRunButton
        let compileButton = self.compileButton
        let allowWarningsToggle = app.descendants(matching: .any)
            .matching(NSPredicate(format: "identifier == %@", "allow-start-with-warnings-toggle"))
            .firstMatch
        let allowWarningsCheckbox = app.checkBoxes["Allow start with warnings"].firstMatch

        let deadline = Date().addingTimeInterval(timeout)
        let compileGraceDeadline = Date().addingTimeInterval(min(12, timeout / 2))
        var compileTriggered = false

        while Date() < deadline {
            if startButton.waitForExistence(timeout: 1), startButton.isEnabled {
                return true
            }

            if let blockingValue = startRunBlockingValue,
               blockingValue.contains("warning_confirmation_required"),
               !allowWarningsToggle.exists,
               !allowWarningsCheckbox.exists {
                app.typeKey(.pageDown, modifierFlags: [])
                RunLoop.current.run(until: Date().addingTimeInterval(0.3))
            }

            for checkbox in [allowWarningsToggle, allowWarningsCheckbox]
            where checkbox.exists && checkbox.isEnabled {
                if !checkbox.isHittable {
                    app.typeKey(.pageDown, modifierFlags: [])
                    RunLoop.current.run(until: Date().addingTimeInterval(0.3))
                }
                if checkbox.isHittable {
                    checkbox.click()
                    RunLoop.current.run(until: Date().addingTimeInterval(0.3))
                }
            }

            if !compileTriggered,
               Date() >= compileGraceDeadline,
               compileButton.waitForExistence(timeout: 1),
               compileButton.isEnabled {
                compileButton.click()
                compileTriggered = true
            }

            RunLoop.current.run(until: Date().addingTimeInterval(0.5))
        }

        return startButton.exists && startButton.isEnabled
    }

    /// The Start Run confirmation button.
    var startRunButton: XCUIElement {
        let identified = app.buttons["workflow-start-run-confirm-button"].firstMatch
        if identified.exists {
            return identified
        }
        return app.buttons["Start Run"].firstMatch
    }

    var startRunButtonStateDescription: String {
        let enabledState = startRunButton.exists ? (startRunButton.isEnabled ? "enabled" : "disabled") : "missing"
        let blockingValue = startRunBlockingValue ?? "unknown"
        return "\(enabledState) [\(blockingValue)]"
    }

    /// The Cancel button.
    var cancelButton: XCUIElement { app.buttons["Cancel"].firstMatch }

    /// The Compile button.
    var compileButton: XCUIElement {
        let identified = app.buttons["workflow-compile-button"].firstMatch
        if identified.exists {
            return identified
        }
        return app.buttons["Compile"].firstMatch
    }

    /// Dismisses the sheet via Cancel or Escape.
    func dismiss() {
        if cancelButton.exists { cancelButton.click() }
        else { app.typeKey(.escape, modifierFlags: []) }
    }
}
