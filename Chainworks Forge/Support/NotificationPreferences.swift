import Foundation

// MARK: - P005-OPS §10: Notification Preferences

/// Operator notification preferences.
/// Intentionally conservative defaults — avoid spamming on every stage completion.
struct NotificationPreferences: Codable, Sendable {
    var approvalRequired: Bool
    var runBlocked: Bool
    var runFailed: Bool
    var runCompleted: Bool
    var menuBarEnabled: Bool

    nonisolated static let defaultPreferences = NotificationPreferences(
        approvalRequired: true,
        runBlocked: true,
        runFailed: true,
        runCompleted: true,
        menuBarEnabled: false
    )

    // MARK: - Persistence

    private static let storageKey = "chainworks_notification_preferences"

    func save() {
        if let data = try? JSONEncoder().encode(self) {
            UserDefaults.standard.set(data, forKey: Self.storageKey)
        }
    }

    static func load() -> NotificationPreferences {
        guard let data = UserDefaults.standard.data(forKey: storageKey),
              let prefs = try? JSONDecoder().decode(NotificationPreferences.self, from: data) else {
            return .defaultPreferences
        }
        return prefs
    }
}
