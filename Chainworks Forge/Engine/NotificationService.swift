import Foundation
import UserNotifications
import AppKit

// MARK: - P005-OPS §10: Notification Service

/// Local notifications, dock badge, optional menu bar presence.
/// Fires for: approval required, run blocked, run failed, run completed.
/// Intentionally conservative — no spam on every stage completion.
@MainActor
@Observable
final class NotificationService {
    private static let processEnvironment = ProcessInfo.processInfo.environment

    private(set) var pendingAttentionCount: Int = 0
    private(set) var isMenuBarEnabled: Bool = false
    private var preferences: NotificationPreferences
    private var authorizationStatus: UNAuthorizationStatus?

    init(preferences: NotificationPreferences = .defaultPreferences) {
        self.preferences = preferences
    }

    // MARK: - Setup

    func requestAuthorization() async {
        guard !Self.notificationsSuppressedForCurrentProcess else { return }
        let center = UNUserNotificationCenter.current()
        do {
            let granted = try await center.requestAuthorization(options: [.alert, .badge, .sound])
            authorizationStatus = granted ? .authorized : .denied
            if !granted {
                ForgeLogger.notification.info("User denied notification authorization")
            }
        } catch {
            ForgeLogger.notification.error("Authorization request failed: \(error.localizedDescription)")
        }
    }

    // MARK: - Notification Events (§10)

    func notifyApprovalRequired(run: Run, stageLabel: String) {
        guard preferences.approvalRequired else { return }
        let content = UNMutableNotificationContent()
        content.title = "Approval Required"
        content.body = "\(run.idea?.title ?? "Run") — \(stageLabel)"
        content.sound = .default
        content.categoryIdentifier = "APPROVAL_REQUIRED"
        scheduleNotification(id: "approval_\(run.id.uuidString)", content: content)
        incrementAttention()
    }

    func notifyRunBlocked(run: Run, reason: String) {
        guard preferences.runBlocked else { return }
        let content = UNMutableNotificationContent()
        content.title = "Run Blocked"
        content.body = "\(run.idea?.title ?? "Run") — \(reason)"
        content.sound = .default
        content.categoryIdentifier = "RUN_BLOCKED"
        scheduleNotification(id: "blocked_\(run.id.uuidString)", content: content)
        incrementAttention()
    }

    func notifyRunFailed(run: Run) {
        guard preferences.runFailed else { return }
        let content = UNMutableNotificationContent()
        content.title = "Run Failed"
        content.body = "\(run.idea?.title ?? "Run") — \(run.workflowTitle)"
        content.sound = .default
        content.categoryIdentifier = "RUN_FAILED"
        scheduleNotification(id: "failed_\(run.id.uuidString)", content: content)
        incrementAttention()
    }

    func notifyRunCompleted(run: Run) {
        guard preferences.runCompleted else { return }
        let content = UNMutableNotificationContent()
        content.title = "Run Completed"
        content.body = "\(run.idea?.title ?? "Run") — \(run.workflowTitle)"
        content.sound = .default
        content.categoryIdentifier = "RUN_COMPLETED"
        scheduleNotification(id: "completed_\(run.id.uuidString)", content: content)
    }

    /// Proposal 011 — REQ-002: Notify when a run has been fully cancelled after settlement.
    func notifyRunCancelled(run: Run) {
        let content = UNMutableNotificationContent()
        content.title = "Run Cancelled"
        content.body = "\(run.idea?.title ?? "Run") — \(run.workflowTitle)"
        content.sound = .default
        content.categoryIdentifier = "RUN_CANCELLED"
        scheduleNotification(id: "cancelled_\(run.id.uuidString)", content: content)
    }

    // MARK: - Dock Badge (§10)

    /// Update dock badge with count of runs requiring attention.
    func updateDockBadge(waitingApprovalCount: Int, blockedCount: Int) {
        pendingAttentionCount = waitingApprovalCount + blockedCount
        // Guard against unit-test contexts where NSApp may not be initialized
        if let app = NSApp {
            app.dockTile.badgeLabel = pendingAttentionCount > 0 ? "\(pendingAttentionCount)" : nil
        }
    }

    func clearDockBadge() {
        pendingAttentionCount = 0
        if let app = NSApp {
            app.dockTile.badgeLabel = nil
        }
    }

    // MARK: - Menu Bar (§10)

    func setMenuBarEnabled(_ enabled: Bool) {
        isMenuBarEnabled = enabled
    }

    // MARK: - Preferences

    func updatePreferences(_ newPreferences: NotificationPreferences) {
        preferences = newPreferences
    }

    // MARK: - Helpers

    private func scheduleNotification(id: String, content: UNNotificationContent) {
        guard !Self.notificationsSuppressedForCurrentProcess else { return }
        // Guard: skip scheduling in unit-test contexts where notification center may not be available
        guard NSApp != nil else { return }
        if let authorizationStatus, !Self.isDeliveryAllowed(for: authorizationStatus) {
            return
        }
        let request = UNNotificationRequest(
            identifier: id,
            content: content,
            trigger: nil // Immediate delivery
        )
        Task { @MainActor [weak self] in
            let center = UNUserNotificationCenter.current()
            let settings = await center.notificationSettings()
            self?.authorizationStatus = settings.authorizationStatus
            guard Self.isDeliveryAllowed(for: settings.authorizationStatus) else {
                return
            }
            do {
                try await center.add(request)
            } catch {
                ForgeLogger.notification.error("Failed to schedule: \(error.localizedDescription)")
            }
        }
    }

    private func incrementAttention() {
        pendingAttentionCount += 1
        if let app = NSApp {
            app.dockTile.badgeLabel = "\(pendingAttentionCount)"
        }
    }

    private static var notificationsSuppressedForCurrentProcess: Bool {
        processEnvironment["CHAINWORKS_IN_MEMORY_STORE"] == "1"
            || processEnvironment.keys.contains(where: { $0.hasPrefix("CHAINWORKS_UI_TEST") })
    }

    private static func isDeliveryAllowed(for status: UNAuthorizationStatus) -> Bool {
        switch status {
        case .authorized, .provisional, .ephemeral:
            return true
        default:
            return false
        }
    }
}
