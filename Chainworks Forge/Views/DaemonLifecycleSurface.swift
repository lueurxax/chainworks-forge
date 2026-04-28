// P042 §5.3 app-facing lifecycle surface.
//
// The main UI binds to `DaemonStatusViewModel`; this file provides:
//
//   • `DaemonDiagnosticsExportCommand.run()` — File → Export Diagnostics
//     menu action wired up in `Chainworks_ForgeApp.body.commands`.
//   • `DaemonLifecycleBanner` — SwiftUI view that renders one of the
//     four P042 §5.3 states (Starting / Ready / Degraded / Failed) or
//     an Unavailable panel. Uses only `DaemonStatus` truth — never
//     infers state from transport errors.
//
// Both helpers are pure view-layer code: no networking, no scheduling.
// The one exception is `DaemonDiagnosticsExportCommand.run()`, which
// opens an `NSSavePanel` and runs the zip export off the main actor.

import Foundation
import SwiftUI
#if os(macOS)
import AppKit
#endif
import UniformTypeIdentifiers

/// File → Export Diagnostics action. Opens a save panel, then kicks off
/// the export on a background task. Errors bubble up through an alert.
enum DaemonDiagnosticsExportCommand {
    static func run(status: DaemonStatus? = nil) {
        #if os(macOS)
        let panel = NSSavePanel()
        panel.nameFieldStringValue = defaultFilename()
        panel.allowedContentTypes = [.zip]
        panel.canCreateDirectories = true
        panel.title = "Export Diagnostics"
        panel.begin { response in
            guard response == .OK, let url = panel.url else { return }
            Task.detached {
                do {
                    _ = try DiagnosticsBundleBuilder.export(
                        inputs: .defaults(status: status),
                        to: url
                    )
                } catch {
                    await MainActor.run {
                        presentExportError(error)
                    }
                }
            }
        }
        #endif
    }

    private static func defaultFilename() -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withFullDate]
        let stamp = formatter.string(from: Date())
        return "chainworks-diagnostics-\(stamp).zip"
    }

    #if os(macOS)
    private static func presentExportError(_ error: Error) {
        let alert = NSAlert()
        alert.messageText = "Could not export diagnostics"
        alert.informativeText = "\(error)"
        alert.alertStyle = .warning
        alert.addButton(withTitle: "OK")
        alert.runModal()
    }
    #endif
}

/// SwiftUI banner that renders the daemon's current lifecycle state.
/// Consumers pass a `DaemonStatusViewModel`; the banner updates
/// automatically as the snapshot or subscription publishes new values.
///
/// P042 §6.1: the banner is also the UX surface for the supervisor-
/// owned anomalous PID-lock path. When the supervised daemon exits
/// with status 75 (`EX_TEMPFAIL`) before HTTP bind, there is no
/// `DaemonStatus` to drive the Unavailable panel's text — so the
/// banner subscribes to `DaemonProcessSupervisor.anomalousPidLockPublisher`
/// and renders an `.alert()` with actionable recovery copy.
struct DaemonLifecycleBanner: View {
    @ObservedObject var viewModel: DaemonStatusViewModel
    @ObservedObject var supervisor: DaemonProcessSupervisor = .shared
    var schedulerHealthIssue: SchedulerHealthBannerIssue?
    var onOpenSchedulerHealth: (() -> Void)?
    @State private var showAnomalousPidLockAlert: Bool = false
    @State private var showCrashBudgetResetResultAlert: Bool = false
    @State private var crashBudgetResetResultSummary: String = ""

    var body: some View {
        // Kept intentionally compact so the SwiftUI type-checker can
        // resolve the `some View` return type quickly. Alerts and
        // subscribers live in `bannerContent`, which returns a
        // pre-composed `some View` so the outer scope doesn't have to
        // type-check the whole modifier chain at once.
        bannerContent
    }

    private var bannerContent: some View {
        Group {
            if let status = viewModel.status {
                phaseView(for: status)
            } else {
                unavailableView()
            }
        }
        .onReceive(supervisor.anomalousPidLockPublisher) { _ in
            // Each anomalous-exit event reopens the dialog so an
            // operator who dismissed a previous alert still sees
            // recurrences (§6.1 operator-visibility contract).
            showAnomalousPidLockAlert = true
        }
        .alert(
            "Daemon PID-lock is held by another process",
            isPresented: $showAnomalousPidLockAlert,
            actions: { anomalousPidLockAlertActions() },
            message: { anomalousPidLockAlertMessage() }
        )
        .alert(
            "Crash budget reset",
            isPresented: $showCrashBudgetResetResultAlert,
            actions: { Button("OK", role: .cancel) { } },
            message: { Text(crashBudgetResetResultSummary) }
        )
    }

    @ViewBuilder
    private func anomalousPidLockAlertActions() -> some View {
        Button("Export Diagnostics") {
            DaemonDiagnosticsExportCommand.run(status: viewModel.status)
        }
        Button("Dismiss", role: .cancel) { }
    }

    @ViewBuilder
    private func anomalousPidLockAlertMessage() -> some View {
        Text(
            "The Chainworks Forge daemon exited with EX_TEMPFAIL (75) "
            + "because another process still holds the PID-lock. "
            + "Re-register the LaunchAgent or sign out / sign back in "
            + "to release the stale holder, then relaunch the app."
        )
    }

    @ViewBuilder
    private func phaseView(for status: DaemonStatus) -> some View {
        VStack(spacing: 8) {
            if let schedulerHealthIssue, status.state != .failed {
                schedulerHealthView(issue: schedulerHealthIssue)
            } else {
                lifecycleRow(for: status)
            }
            if let xcodeBrokerHealth = status.xcodeBrokerHealth {
                xcodeBrokerHealthRow(xcodeBrokerHealth)
            }
        }
    }

    @ViewBuilder
    private func lifecycleRow(for status: DaemonStatus) -> some View {
        switch status.state {
        case .starting, .restarting, .notStarted:
            row(symbol: "hourglass", tint: .gray, text: "Daemon starting…")
        case .ready:
            row(symbol: "checkmark.circle.fill", tint: .green, text: "Daemon ready")
        case .degraded:
            let kinds = status.degraded.map { $0.kind.rawValue }.joined(separator: ", ")
            row(
                symbol: "exclamationmark.triangle.fill",
                tint: .yellow,
                text: "Degraded — \(kinds.isEmpty ? "reason unknown" : kinds)"
            )
        case .failed:
            let reason = status.failure?.kind.rawValue ?? "unknown"
            row(
                symbol: "xmark.octagon.fill",
                tint: .red,
                text: "Daemon failed — \(reason)",
                action: { combinedFailedStateActions(failureKind: status.failure?.kind) }
            )
        case .shutdown:
            row(symbol: "power", tint: .gray, text: "Daemon shut down")
        }
    }

    private func xcodeBrokerHealthRow(_ health: XcodeBrokerHealthSnapshot) -> some View {
        let presentation = xcodeBrokerHealthPresentation(health)
        return row(
            symbol: presentation.symbol,
            tint: presentation.tint,
            text: presentation.text
        )
        .accessibilityIdentifier("xcode-broker-health")
    }

    private func xcodeBrokerHealthPresentation(
        _ health: XcodeBrokerHealthSnapshot
    ) -> (symbol: String, tint: Color, text: String) {
        let leases = "\(health.activeLeases)/\(health.maxActiveLeases) active"
        let queue = "\(health.queuedLeases)/\(health.maxQueuedLeases) queued"
        let backend = health.backendAvailable ? "backend ready" : "backend missing"
        let persistence = health.observationPersistenceFailures > 0
            ? ", \(health.observationPersistenceFailures) observation write failures"
            : ""
        let helperCleanup = health.staleLeaseCount > 0 || health.helperCleanupReapedLeasesTotal > 0
            ? ", \(health.staleLeaseCount) stale, \(health.backendSessionCount) backend sessions, \(health.helperCleanupReapedLeasesTotal) cleaned"
            : ""
        let acquisition = health.canAcquireNewXcodeLeases ? "leases available" : "leases blocked"
        let message = health.operatorMessage.isEmpty ? nil : health.operatorMessage
        let suffix = "\(leases), \(queue), \(backend), \(acquisition)\(persistence)\(helperCleanup)"
        switch health.state {
        case .disabled:
            return ("pause.circle.fill", .gray, "\(message ?? "Xcode Broker disabled") — \(suffix)")
        case .healthy:
            return ("checkmark.shield.fill", .green, "\(message ?? "Xcode Broker healthy") — \(suffix)")
        case .degraded:
            return ("exclamationmark.triangle.fill", .yellow, "\(message ?? "Xcode Broker degraded") — \(suffix)")
        case .failed:
            return ("xmark.octagon.fill", .red, "\(message ?? "Xcode Broker failed") — \(suffix)")
        }
    }

    @ViewBuilder
    private func unavailableView() -> some View {
        row(
            symbol: "bolt.horizontal.icloud",
            tint: .orange,
            text: "Daemon unavailable",
            action: { combinedUnavailableActions() }
        )
    }

    @ViewBuilder
    private func schedulerHealthView(issue: SchedulerHealthBannerIssue) -> some View {
        let tint = schedulerHealthTint(issue.kind)
        row(
            symbol: issue.systemImage,
            tint: tint,
            text: "\(issue.title) — \(issue.detail)",
            action: schedulerHealthAction
        )
        .accessibilityIdentifier("daemon-scheduler-health-banner")
    }

    @ViewBuilder
    private func row(
        symbol: String,
        tint: Color,
        text: String,
        @ViewBuilder action: () -> some View = { EmptyView() }
    ) -> some View {
        HStack(spacing: 12) {
            Image(systemName: symbol).foregroundStyle(tint)
            Text(text).font(.callout).foregroundStyle(.primary)
            Spacer(minLength: 0)
            action()
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(tint.opacity(0.12), in: RoundedRectangle(cornerRadius: 8))
    }

    @ViewBuilder
    private func failedStateActions(failureKind: FailureKind?) -> some View {
        HStack(spacing: 8) {
            if failureKind == .crashLoopBudgetExhausted {
                // P042 §6.2 + REQ-006: only the crash-loop exhaustion
                // state surfaces the Reset Crash Budget affordance.
                // Other terminal failure kinds (`migration_failed`,
                // `schema_newer_than_binary`, `backup_failed`) need
                // different recovery paths that don't involve deleting
                // `crash-budget.json`.
                Button("Reset Crash Budget") {
                    performCrashBudgetReset()
                }
                .controlSize(.small)
                .accessibilityIdentifier("daemon-reset-crash-budget-button")
            }
            Button("Export Diagnostics") {
                DaemonDiagnosticsExportCommand.run(status: viewModel.status)
            }
            .controlSize(.small)
        }
    }

    @ViewBuilder
    private func combinedFailedStateActions(failureKind: FailureKind?) -> some View {
        HStack(spacing: 8) {
            failedStateActions(failureKind: failureKind)
            schedulerHealthAction()
        }
    }

    private func performCrashBudgetReset() {
        Task { @MainActor in
            let result = await CrashBudgetResetCoordinator.shared.performReset()
            crashBudgetResetResultSummary = result.summary
            showCrashBudgetResetResultAlert = true
            // After a successful reset the daemon should come back up
            // on its own; refresh the view model so the banner picks
            // up the next heartbeat rather than staying pinned on the
            // old failed frame.
            if result.isFullySuccessful {
                await viewModel.refresh()
            }
        }
    }

    private func unavailableActions() -> some View {
        HStack(spacing: 8) {
            Button("Retry") {
                Task { await viewModel.refresh() }
            }
            .controlSize(.small)
            Button("Export Diagnostics") {
                DaemonDiagnosticsExportCommand.run()
            }
            .controlSize(.small)
        }
    }

    @ViewBuilder
    private func combinedUnavailableActions() -> some View {
        HStack(spacing: 8) {
            unavailableActions()
            schedulerHealthAction()
        }
    }

    @ViewBuilder
    private func schedulerHealthAction() -> some View {
        if schedulerHealthIssue != nil, let onOpenSchedulerHealth {
            Button("Scheduler Health") {
                onOpenSchedulerHealth()
            }
            .controlSize(.small)
            .accessibilityIdentifier("daemon-open-scheduler-health")
        }
    }

    private func schedulerHealthTint(_ kind: SchedulerHealthBannerIssue.Kind) -> Color {
        switch kind {
        case .sustainedBackpressure, .dbWriterPressure:
            return .orange
        case .staleProjection:
            return .yellow
        }
    }
}
