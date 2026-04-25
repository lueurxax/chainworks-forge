// P042 §6.2 operator reset flow (REQ-006 / UI-001).
//
// When the daemon enters `failed/crash_loop_budget_exhausted`, the
// banner offers a "Reset Crash Budget" button. This file owns the
// logic behind that button so the view stays declarative:
//
//   1. Delete the packaged crash-budget file at
//      `~/Library/Application Support/Chainworks Forge/crash-budget.json`.
//      The daemon reads that file on startup and treats its absence as
//      `CrashBudgetDecision::Clean` — so removing it clears the budget
//      without restarting the computer.
//
//   2. Ask `SMAppService.agent(plistName:)` to unregister and
//      re-register, which forces launchd to stop any stale supervised
//      process and start a fresh one. On Debug dev workstations where
//      SMAppService is unused (LWCR rejects Apple Development signing)
//      we skip the SMAppService step and rely on the app's own
//      `scheduleDaemonSupervisionProbe` to spawn the next daemon process.
//
// The file is deliberately testable: the delete primitive takes a
// `FileManager` and a base URL so tests can target a tempdir. The
// SMAppService side-effect is routed through a `Restarter` type so
// tests can substitute a fake. Production callers use `.shared` which
// binds both sides to live app state.

import Foundation
#if os(macOS)
import ServiceManagement
#endif

/// Pure file-system primitives for the reset flow. Isolated here so
/// tests can call them against a tempdir without spinning up
/// `SMAppService` or the lifecycle view model.
enum CrashBudgetFiles {
    /// Resolves `~/Library/Application Support/Chainworks Forge`.
    nonisolated static func defaultAppSupportDir(
        fileManager: FileManager = .default
    ) -> URL {
        fileManager.homeDirectoryForCurrentUser
            .appendingPathComponent(
                "Library/Application Support/Chainworks Forge",
                isDirectory: true
            )
    }

    /// Resolves the packaged daemon's crash-budget file path. Takes an
    /// explicit `appSupportDir` so tests can target a tempdir.
    nonisolated static func crashBudgetURL(appSupportDir: URL) -> URL {
        appSupportDir.appendingPathComponent("crash-budget.json", isDirectory: false)
    }

    /// Delete the crash-budget file at the given URL. Returns the
    /// typed outcome (`.removed` | `.alreadyAbsent` | `.failed`) so the
    /// UI can tell the operator whether there was anything to clear.
    /// Missing file is NOT an error: P042 §6.2 documents file-absence
    /// as "clean", and a user who clicks Reset on an already-clean
    /// state deserves a soft confirmation rather than an error dialog.
    nonisolated static func deleteCrashBudgetFile(
        at url: URL,
        fileManager: FileManager = .default
    ) -> CrashBudgetResetFileOutcome {
        guard fileManager.fileExists(atPath: url.path) else {
            return .alreadyAbsent
        }
        do {
            try fileManager.removeItem(at: url)
            return .removed
        } catch {
            return .failed(error: error)
        }
    }
}

/// Outcome of the file-delete step. Kept equatable-ish for tests via
/// the custom `description` and the matching `enum tag` helper below.
enum CrashBudgetResetFileOutcome: CustomStringConvertible {
    case removed
    case alreadyAbsent
    case failed(error: Error)

    var description: String {
        switch self {
        case .removed: return "removed"
        case .alreadyAbsent: return "alreadyAbsent"
        case .failed(let error): return "failed(\(error))"
        }
    }

    var isSuccess: Bool {
        switch self {
        case .removed, .alreadyAbsent: return true
        case .failed: return false
        }
    }
}

/// Restart coordinator abstraction. Production binding forwards to
/// `SMAppService`; tests substitute a counting fake so they can assert
/// "the reset flow requested one restart" without touching launchd.
protocol DaemonRestarter {
    /// Request the packaged daemon to be unregistered + re-registered.
    /// Best-effort: callers do not treat failures as a blocker (the
    /// lifecycle banner will re-surface state once the daemon comes
    /// back or stays down). Implementations should be idempotent.
    @MainActor
    func requestRestart() throws
}

#if os(macOS)
/// Production restarter. Calls `SMAppService.agent(plistName:).register()`
/// after unregistering the current registration. Unregister failures are
/// swallowed (the service may have been in an inconsistent state and a
/// fresh `register()` resolves both cases).
struct SMAppServiceDaemonRestarter: DaemonRestarter {
    let plistName: String

    init(plistName: String = "com.chainworks.forge.daemon.plist") {
        self.plistName = plistName
    }

    @MainActor
    func requestRestart() throws {
        let service = SMAppService.agent(plistName: plistName)
        // Unregister may throw if the service wasn't registered — that
        // is fine, the subsequent register() is what matters.
        try? service.unregister()
        try service.register()
    }
}
#endif

/// High-level façade used by the UI button. Combines the file delete
/// and the restart request and emits a typed `CrashBudgetResetResult`
/// so the UI can render the correct follow-up banner or alert.
@MainActor
struct CrashBudgetResetCoordinator {
    let appSupportDir: URL
    let fileManager: FileManager
    let restarter: DaemonRestarter?

    /// Default production binding. Uses live `FileManager` + a real
    /// `SMAppServiceDaemonRestarter` on macOS and a no-op restarter on
    /// platforms where SMAppService is unavailable.
    static var shared: CrashBudgetResetCoordinator {
        #if os(macOS)
        return CrashBudgetResetCoordinator(
            appSupportDir: CrashBudgetFiles.defaultAppSupportDir(),
            fileManager: .default,
            restarter: SMAppServiceDaemonRestarter()
        )
        #else
        return CrashBudgetResetCoordinator(
            appSupportDir: CrashBudgetFiles.defaultAppSupportDir(),
            fileManager: .default,
            restarter: nil
        )
        #endif
    }

    /// Perform the reset flow. Returns a typed result so the UI knows
    /// whether to celebrate or warn the operator. Does not throw —
    /// every failure mode is captured in the returned value.
    func performReset() -> CrashBudgetResetResult {
        let url = CrashBudgetFiles.crashBudgetURL(appSupportDir: appSupportDir)
        let fileOutcome = CrashBudgetFiles.deleteCrashBudgetFile(
            at: url,
            fileManager: fileManager
        )
        var restartError: Error?
        if fileOutcome.isSuccess, let restarter = restarter {
            do {
                try restarter.requestRestart()
            } catch {
                restartError = error
            }
        }
        return CrashBudgetResetResult(
            fileOutcome: fileOutcome,
            restartError: restartError
        )
    }
}

/// Typed outcome of the reset flow. The UI switches on
/// `isFullySuccessful` to render a success banner, or falls back to an
/// informational alert when the file was already gone or the restart
/// request failed.
struct CrashBudgetResetResult {
    let fileOutcome: CrashBudgetResetFileOutcome
    let restartError: Error?

    /// `true` when the file delete AND the restart request both
    /// succeeded (or the restarter was absent in a test build).
    var isFullySuccessful: Bool {
        fileOutcome.isSuccess && restartError == nil
    }

    /// Short human-readable summary suitable for an `NSAlert` body.
    var summary: String {
        switch (fileOutcome, restartError) {
        case (.removed, nil):
            return "Crash budget cleared and the daemon was asked to restart."
        case (.removed, .some(let err)):
            return "Crash budget cleared, but the daemon restart request failed: \(err)"
        case (.alreadyAbsent, nil):
            return "Crash budget was already clear; no action needed."
        case (.alreadyAbsent, .some(let err)):
            return "Crash budget was already clear; the restart request failed: \(err)"
        case (.failed(let err), _):
            return "Could not clear crash budget: \(err)"
        }
    }
}
