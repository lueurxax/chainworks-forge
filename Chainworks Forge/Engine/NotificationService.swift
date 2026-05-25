import Foundation
import UserNotifications
import AppKit

struct P081OperatorAlertNativeDeliveryMetricEvent: Equatable, Sendable {
    static let metricName = "operator_alert_native_delivery_total"

    let severity: String
    let surface: String
    let result: String
}

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
    private var runAttentionCount: Int = 0
    private var operatorAlertAttentionCount: Int = 0
    private var userMenuBarEnabled: Bool = false
    private var operatorAlertForcesMenuBar: Bool = false
    private var deliveredOperatorAlertKeys: Set<String> = []
    private(set) var p081NativeDeliveryMetricEvents: [P081OperatorAlertNativeDeliveryMetricEvent] = []

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
        runAttentionCount = waitingApprovalCount + blockedCount
        refreshDockBadgeLabel()
    }

    func clearDockBadge() {
        runAttentionCount = 0
        operatorAlertAttentionCount = 0
        deliveredOperatorAlertKeys.removeAll()
        refreshDockBadgeLabel()
    }

    // MARK: - P081 Operator Alerts

    func applyP081OperatorAlerts(_ alerts: [P081OperatorAlert], now: Date = Date()) {
        let activeAlerts = alerts.filter(\.active)
        operatorAlertForcesMenuBar = activeAlerts.contains { alert in
            guard let delivery = alert.nativeDelivery else { return false }
            return delivery.dockBadgeContribution > 0
                || delivery.requestUserAttention.lowercased() == "critical"
        }
        refreshMenuBarState()
        operatorAlertAttentionCount = activeAlerts.reduce(0) { total, alert in
            total + max(0, alert.nativeDelivery?.dockBadgeContribution ?? 0)
        }
        refreshDockBadgeLabel()

        let activeDeliveryKeys = Set(activeAlerts.compactMap(\.nativeDelivery?.deliveryKey))
        deliveredOperatorAlertKeys.formIntersection(activeDeliveryKeys)

        for alert in activeAlerts {
            guard let nativeDelivery = alert.nativeDelivery else { continue }
            if isP081OperatorAlertSilenced(alert, now: now) {
                recordP081NativeDeliveryMetric(alert: alert, result: "silenced")
                continue
            }
            guard deliveredOperatorAlertKeys.insert(nativeDelivery.deliveryKey).inserted else {
                recordP081NativeDeliveryMetric(alert: alert, result: "deduped")
                continue
            }
            requestP081OperatorAttention(nativeDelivery.requestUserAttention)
            scheduleP081OperatorAlertNotification(alert, nativeDelivery: nativeDelivery)
            recordP081NativeDeliveryMetric(alert: alert, result: "delivered")
        }
    }

    // MARK: - Menu Bar (§10)

    func setMenuBarEnabled(_ enabled: Bool) {
        userMenuBarEnabled = enabled
        refreshMenuBarState()
    }

    // MARK: - Preferences

    func updatePreferences(_ newPreferences: NotificationPreferences) {
        preferences = newPreferences
        _ = newPreferences.save()
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
        runAttentionCount += 1
        refreshDockBadgeLabel()
    }

    private func refreshDockBadgeLabel() {
        pendingAttentionCount = max(0, runAttentionCount + operatorAlertAttentionCount)
        if let app = NSApp {
            app.dockTile.badgeLabel = pendingAttentionCount > 0 ? "\(pendingAttentionCount)" : nil
        }
    }

    private func refreshMenuBarState() {
        isMenuBarEnabled = userMenuBarEnabled || operatorAlertForcesMenuBar
    }

    private func isP081OperatorAlertSilenced(_ alert: P081OperatorAlert, now: Date) -> Bool {
        guard let silencedUntilMs = alert.silencedUntilMs else { return false }
        let nowMs = Int(now.timeIntervalSince1970 * 1_000)
        return silencedUntilMs > nowMs
    }

    private func requestP081OperatorAttention(_ mode: String) {
        guard let app = NSApp else { return }
        switch mode.lowercased() {
        case "critical":
            app.requestUserAttention(.criticalRequest)
        case "informational", "info":
            app.requestUserAttention(.informationalRequest)
        default:
            return
        }
    }

    private func scheduleP081OperatorAlertNotification(
        _ alert: P081OperatorAlert,
        nativeDelivery: P081OperatorAlertNativeDelivery
    ) {
        guard !Self.notificationsSuppressedForCurrentProcess else { return }
        let content = UNMutableNotificationContent()
        content.title = alert.title
        content.body = alert.message
        content.sound = .default
        content.categoryIdentifier = nativeDelivery.notificationCategory
        scheduleNotification(id: "p081_operator_alert_\(nativeDelivery.deliveryKey)", content: content)
    }

    private func recordP081NativeDeliveryMetric(alert: P081OperatorAlert, result: String) {
        p081NativeDeliveryMetricEvents.append(
            P081OperatorAlertNativeDeliveryMetricEvent(
                severity: alert.severity,
                surface: "macos_notification_service",
                result: result
            )
        )
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
