import Foundation

#if os(macOS)
import AppKit
#endif

@MainActor
protocol ExecutionTerminationControlling: AnyObject {
    func prepareForTermination()
}

/// P083 shutdown_contract_v1: Coordinates graceful AppKit shutdown.
///
/// AppKit termination paths:
/// - graceful_appkit (normal Quit, logout/system shutdown): AppKit invokes
///   applicationShouldTerminate. We return .terminateLater, drain the bounded daemon
///   wave up to host_total_ms (30 000 ms), then call reply(toApplicationShouldTerminate:true).
///   The host_total_ms budget is only valid for this graceful path.
/// - abrupt_external (Force Quit, SIGKILL, crash): AppKit does NOT invoke
///   applicationShouldTerminate. Recovery relies on durable shutdown_signal_side_effects
///   and provider_cancellation_intents rows written before the signal. Do NOT claim
///   host_total_ms coverage for abrupt_external paths.
@MainActor
class AppTerminationCoordinator: NSObject {
    weak var executionTerminationController: ExecutionTerminationControlling?

    /// Maximum ms to wait for in-progress provider shutdown waves before replying to AppKit.
    /// Per shutdown_contract_v1.reliability_deadline_overflow_contract_v1.shutdown_deadline_defaults.
    static let hostTotalMs: Int = 30_000

    /// Fixed tail (ms) after host_total_ms for queued_no_signal receipt flush on graceful_appkit paths.
    /// Per shutdown_contract_v1.recovery_rules.graceful_appkit.
    static let receiptFlushTailMs: Int = 1_000

    #if os(macOS)
    override init() {
        super.init()
    }
    #else
    override init() {
        super.init()
    }
    #endif

    func prepareForTermination() {
        executionTerminationController?.prepareForTermination()
    }
}

#if os(macOS)
extension AppTerminationCoordinator: NSApplicationDelegate {
    /// P083 shutdown_contract_v1 graceful_appkit path.
    ///
    /// Returns .terminateLater and schedules a background task that:
    /// 1. Calls prepareForTermination() to drain in-progress provider shutdown waves.
    /// 2. Waits up to host_total_ms for the bounded daemon wave.
    /// 3. Replies to AppKit with reply(toApplicationShouldTerminate:true).
    ///
    /// This is ONLY called for graceful Quit and logout/system-shutdown paths where
    /// AppKit invokes the delegate. Force Quit, SIGKILL, and crashes do NOT invoke this.
    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        Task { @MainActor in
            // Drain in-progress work before waiting.
            prepareForTermination()
            // Wait up to host_total_ms + receiptFlushTailMs for shutdown wave completion and
            // queued_no_signal receipt flush. The total deadline is only honored on this
            // graceful_appkit path; abrupt_external paths rely on durable intent row recovery.
            let totalMs = AppTerminationCoordinator.hostTotalMs + AppTerminationCoordinator.receiptFlushTailMs
            let deadlineNs = UInt64(totalMs) * 1_000_000
            try? await Task.sleep(nanoseconds: deadlineNs)
            sender.reply(toApplicationShouldTerminate: true)
        }
        return .terminateLater
    }

    /// Called by AppKit immediately before the process exits, after the terminateLater reply
    /// from applicationShouldTerminate. This is a graceful_appkit path — abrupt_external paths
    /// (Force Quit, SIGKILL, crash, power loss) do NOT invoke this callback. Recovery for
    /// abrupt_external paths relies solely on durable shutdown_signal_side_effects and
    /// provider_cancellation_intents rows written before the signal.
    func applicationWillTerminate(_ notification: Notification) {
        prepareForTermination()
    }
}
#endif
