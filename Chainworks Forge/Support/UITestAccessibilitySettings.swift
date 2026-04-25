import SwiftUI

struct UITestAccessibilitySettings: Equatable {
    let differentiateWithoutColor: Bool
    let increaseContrast: Bool
    let reduceTransparency: Bool

    static let none = UITestAccessibilitySettings(
        differentiateWithoutColor: false,
        increaseContrast: false,
        reduceTransparency: false
    )

    static var requested: UITestAccessibilitySettings? {
        let environment = ProcessInfo.processInfo.environment
        let settings = UITestAccessibilitySettings(
            differentiateWithoutColor: environment["CHAINWORKS_UI_TEST_DIFFERENTIATE_WITHOUT_COLOR"] == "1",
            increaseContrast: environment["CHAINWORKS_UI_TEST_INCREASE_CONTRAST"] == "1",
            reduceTransparency: environment["CHAINWORKS_UI_TEST_REDUCE_TRANSPARENCY"] == "1"
        )
        return settings.hasOverrides ? settings : nil
    }

    var hasOverrides: Bool {
        differentiateWithoutColor || increaseContrast || reduceTransparency
    }

    var activeIdentifiers: [String] {
        var identifiers: [String] = []
        if differentiateWithoutColor {
            identifiers.append("ui-test-accessibility-differentiate-without-color")
        }
        if increaseContrast {
            identifiers.append("ui-test-accessibility-increase-contrast")
        }
        if reduceTransparency {
            identifiers.append("ui-test-accessibility-reduce-transparency")
        }
        return identifiers
    }
}

private struct UITestAccessibilitySettingsKey: EnvironmentKey {
    static let defaultValue: UITestAccessibilitySettings = .none
}

extension EnvironmentValues {
    var uiTestAccessibilitySettings: UITestAccessibilitySettings {
        get { self[UITestAccessibilitySettingsKey.self] }
        set { self[UITestAccessibilitySettingsKey.self] = newValue }
    }
}
