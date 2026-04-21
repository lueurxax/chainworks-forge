// P042 §6.1 / AC-3 / AC-12 Swift-side supervisor.
//
// SMAppService owns the happy-path launch. This supervisor owns the
// anomalous PID-lock path (exit 75 = EX_TEMPFAIL) and the generic
// pre-bind failure path: when the daemon process exits before HTTP
// bind, there's no `DaemonStatus` to read, so the operator needs a
// supervisor-owned surface instead of the lifecycle banner.
//
// Conceptually:
//   • In Debug / dev builds we spawn the daemon as a child `Process`
//     because SMAppService's LWCR rejects Apple Development signing.
//     That spawn is where we observe termination codes.
//   • In Release builds SMAppService owns the process, but if that
//     fails we can still run a one-shot probe launch to capture
//     exit code 75 and route it through the same notifier.
//
// The supervisor is intentionally UI-agnostic: it publishes a typed
// `DaemonProcessExit` event via a `@Published` on an ObservableObject.
// A SwiftUI view (the lifecycle banner's Unavailable panel, or a
// dedicated alert presenter) binds to it and shows the operator
// dialog. Unit tests drive the classifier directly with synthetic
// termination codes — no need for a live process.

import Combine
import Foundation

/// Terminal classification of a daemon process exit, specialised for
/// the supervisor-owned failure paths. P042 §6.1 carves out exit 75
/// from the `DaemonStatus.failure` vocabulary because the daemon
/// never got far enough to populate status — it failed before HTTP
/// bind.
enum DaemonProcessExit: Equatable, Sendable {
    /// Clean exit (the daemon finished graceful drain). No UI needed.
    case cleanExit(code: Int32)
    /// Duplicate PID-lock holder was healthy; current process exited
    /// 0 by design (§6.1 DuplicateHealthy). No UI needed.
    case duplicateHealthy
    /// Anomalous holder — `EX_TEMPFAIL` (75). Surface a dialog with
    /// the recommended recovery steps and an action button to
    /// re-register / reboot the agent.
    case anomalousPidLock
    /// Non-zero exit that doesn't match any known supervisor path.
    /// Caller should log the code and surface a generic "daemon
    /// failed to start" dialog.
    case unknownFailure(code: Int32)
    /// Daemon was killed by a signal (e.g. SIGKILL from the OS).
    case signalled(signal: Int32)
}

extension DaemonProcessExit {
    /// Classifier — the production code path. Matches the `Process`
    /// termination API: `.exit` with status, `.uncaughtSignal` with
    /// signal number.
    static func classify(
        status: Int32,
        reason: Process.TerminationReason
    ) -> DaemonProcessExit {
        switch reason {
        case .uncaughtSignal:
            return .signalled(signal: status)
        case .exit:
            fallthrough
        @unknown default:
            switch status {
            case 0:
                return .cleanExit(code: 0)
            case 75:
                return .anomalousPidLock
            default:
                return .unknownFailure(code: status)
            }
        }
    }
}

/// Publishes the latest `DaemonProcessExit` on the main actor so
/// SwiftUI views can bind and render the appropriate dialog.
///
/// P042 §6.2 / R13 REL-001: the supervisor is also the **writer** of
/// the crash-loop budget file. The Rust daemon only reads the file at
/// startup (and resets it after a stable `Ready` window). Without a
/// production writer on the Swift side, real abnormal exits never
/// accumulated and `FailureKind::CrashLoopBudgetExhausted` could never
/// fire naturally. Every non-clean exit classification now appends to
/// `crash-budget.json` via `CrashBudgetRecorder`.
@MainActor
final class DaemonProcessSupervisor: ObservableObject {
    /// Process-wide shared supervisor. The Debug daemon fallback in
    /// `Chainworks_ForgeApp` hooks `Process.terminationHandler` into
    /// this instance so exit-75 events reach the UI via
    /// `anomalousPidLockPublisher`. SwiftUI views bind to the same
    /// instance to render the P042 §6.1 operator dialog.
    static let shared = DaemonProcessSupervisor()

    /// The most recent supervised process exit, or `nil` when no
    /// supervised process has been observed yet (fresh app).
    @Published private(set) var lastExit: DaemonProcessExit?

    /// Delivered after `record(_:)` observes an anomalous PID-lock
    /// exit. UI subscribes to this instead of toggling on `lastExit`
    /// so a single exit event maps to one dialog regardless of
    /// `@Published` coalescing.
    let anomalousPidLockPublisher = PassthroughSubject<Void, Never>()

    /// Path of the crash-budget file to update on abnormal exits.
    /// Defaults to the packaged location; tests inject a tempdir.
    private let crashBudgetURL: URL
    /// Closure that returns "now" in unix seconds. Tests pin the
    /// clock so window-boundary logic is deterministic.
    private let nowSecondsProvider: () -> UInt64

    init(
        crashBudgetURL: URL = CrashBudgetFiles.crashBudgetURL(
            appSupportDir: CrashBudgetFiles.defaultAppSupportDir()
        ),
        nowSecondsProvider: @escaping () -> UInt64 = {
            UInt64(Date().timeIntervalSince1970)
        }
    ) {
        self.crashBudgetURL = crashBudgetURL
        self.nowSecondsProvider = nowSecondsProvider
    }

    func record(_ exit: DaemonProcessExit) {
        self.lastExit = exit
        if case .anomalousPidLock = exit {
            anomalousPidLockPublisher.send(())
        }
        if exit.isAbnormal {
            // Fire-and-forget: we never surface I/O errors from the
            // budget file to the operator — the worst case is a
            // single lost crash, and the next exit starts fresh.
            CrashBudgetRecorder.recordAbnormalExit(
                at: crashBudgetURL,
                now: nowSecondsProvider()
            )
        }
    }

    /// Convenience for call-sites that already have raw status values.
    func record(status: Int32, reason: Process.TerminationReason) {
        record(DaemonProcessExit.classify(status: status, reason: reason))
    }
}

extension DaemonProcessExit {
    /// §6.2 crash-budget rule: clean exits and duplicate-healthy
    /// exits don't count toward the budget. Every other classification
    /// — anomalous PID-lock, unknown non-zero exits, signalled
    /// termination — counts as an abnormal exit.
    var isAbnormal: Bool {
        switch self {
        case .cleanExit, .duplicateHealthy:
            return false
        case .anomalousPidLock, .unknownFailure, .signalled:
            return true
        }
    }
}

/// On-disk shape of `~/Library/Application Support/Chainworks Forge/crash-budget.json`.
/// Matches the Rust `CrashBudgetFile` struct in
/// `control-plane/crates/daemon/src/supervisor.rs`: the keys are
/// lowercase snake_case (`first_crash_at`, `crash_count`) so the Rust
/// daemon's startup read parses the same file this Swift writer emits.
struct CrashBudgetFile: Codable, Equatable {
    var first_crash_at: UInt64
    var crash_count: UInt32

    static let empty = CrashBudgetFile(first_crash_at: 0, crash_count: 0)
}

/// Implements P042 §6.2 crash-budget updates in Swift. The Rust side
/// has its own `supervisor::record_crash`, but that function is called
/// by "external supervision logic" — on the production path, that
/// supervisor IS this app. Mirror the Rust logic here so the file
/// format stays bit-identical.
enum CrashBudgetRecorder {
    /// §6.2: 60 s crash window.
    static let windowSeconds: UInt64 = 60

    /// Append one abnormal exit to the budget file at `url`. If the
    /// file is absent or the last crash is older than the window,
    /// open a new window with `crash_count = 1`. Otherwise increment
    /// the count. Never throws — we treat budget I/O as best-effort.
    static func recordAbnormalExit(at url: URL, now: UInt64) {
        let fileManager = FileManager.default
        // Create the parent dir on first write (packaged app support
        // dir exists in normal operation, but the Debug dev fallback
        // may run before anything else has touched it).
        let parent = url.deletingLastPathComponent()
        try? fileManager.createDirectory(
            at: parent, withIntermediateDirectories: true
        )

        var file = readOrDefault(at: url)
        if file.crash_count == 0 || now &- file.first_crash_at > windowSeconds {
            file.first_crash_at = now
            file.crash_count = 1
        } else {
            // Saturating add: a ~4-billion-crash budget file would be
            // pathological, but stay safe.
            file.crash_count = file.crash_count == UInt32.max
                ? UInt32.max
                : file.crash_count + 1
        }

        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        guard let data = try? encoder.encode(file) else { return }
        try? data.write(to: url, options: .atomic)
    }

    /// Read the current budget file, returning `.empty` on any
    /// failure (absent file, bad JSON, unreadable — all indicate a
    /// fresh window should start on the next write).
    static func readOrDefault(at url: URL) -> CrashBudgetFile {
        guard let data = try? Data(contentsOf: url),
              let decoded = try? JSONDecoder().decode(CrashBudgetFile.self, from: data)
        else {
            return .empty
        }
        return decoded
    }
}
