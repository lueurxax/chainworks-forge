// P042 §6.2 / REQ-006 / UI-001 crash-budget reset coordinator tests.

import Foundation
import Testing
@testable import Chainworks_Forge

struct CrashBudgetResetTests {

    // MARK: - File primitives

    @Test func `Delete crash budget file removes file when present`() throws {
        let tmp = try makeTempAppSupportDir()
        defer { try? FileManager.default.removeItem(at: tmp) }
        let url = CrashBudgetFiles.crashBudgetURL(appSupportDir: tmp)
        try Data("{}".utf8).write(to: url)
        #expect(FileManager.default.fileExists(atPath: url.path))

        let outcome = CrashBudgetFiles.deleteCrashBudgetFile(at: url)

        if case .removed = outcome {} else {
            Issue.record("expected .removed, got \(outcome)")
        }
        #expect(!FileManager.default.fileExists(atPath: url.path))
    }

    @Test func `Delete crash budget file reports already absent when missing`() throws {
        let tmp = try makeTempAppSupportDir()
        defer { try? FileManager.default.removeItem(at: tmp) }
        let url = CrashBudgetFiles.crashBudgetURL(appSupportDir: tmp)
        #expect(!FileManager.default.fileExists(atPath: url.path))

        let outcome = CrashBudgetFiles.deleteCrashBudgetFile(at: url)

        if case .alreadyAbsent = outcome {} else {
            Issue.record("expected .alreadyAbsent, got \(outcome)")
        }
    }

    // MARK: - Coordinator

    @MainActor
    @Test func `Perform reset deletes file and requests single restart`() async throws {
        let tmp = try makeTempAppSupportDir()
        defer { try? FileManager.default.removeItem(at: tmp) }
        let url = CrashBudgetFiles.crashBudgetURL(appSupportDir: tmp)
        try Data("{\"crash_count\":5}".utf8).write(to: url)

        let restarter = CountingRestarter()
        let coordinator = CrashBudgetResetCoordinator(
            appSupportDir: tmp,
            fileManager: .default,
            restarter: restarter
        )

        let result = await coordinator.performReset()

        #expect(result.isFullySuccessful)
        #expect(restarter.restartCount == 1,
                "reset must request exactly one daemon restart")
        #expect(!FileManager.default.fileExists(atPath: url.path),
                "crash-budget file must be deleted")
    }

    @MainActor
    @Test func `Perform reset noop when already absent still requests restart`() async throws {
        // §6.2: the operator may click Reset preemptively before the
        // daemon records a crash. In that case there is no file to
        // delete but we still want to kick the daemon so UI stays in
        // sync with operator intent. (Contract: file outcome =
        // .alreadyAbsent → still restart-request.)
        let tmp = try makeTempAppSupportDir()
        defer { try? FileManager.default.removeItem(at: tmp) }

        let restarter = CountingRestarter()
        let coordinator = CrashBudgetResetCoordinator(
            appSupportDir: tmp,
            fileManager: .default,
            restarter: restarter
        )

        let result = await coordinator.performReset()

        #expect(result.isFullySuccessful)
        #expect(restarter.restartCount == 1)
        if case .alreadyAbsent = result.fileOutcome {
            // Good.
        } else {
            Issue.record("expected .alreadyAbsent, got \(result.fileOutcome)")
        }
    }

    @MainActor
    @Test func `Perform reset reports restart error while still deleting file`() async throws {
        let tmp = try makeTempAppSupportDir()
        defer { try? FileManager.default.removeItem(at: tmp) }
        let url = CrashBudgetFiles.crashBudgetURL(appSupportDir: tmp)
        try Data("{}".utf8).write(to: url)

        struct RestartFailure: Error {}
        let restarter = FailingRestarter(error: RestartFailure())
        let coordinator = CrashBudgetResetCoordinator(
            appSupportDir: tmp,
            fileManager: .default,
            restarter: restarter
        )

        let result = await coordinator.performReset()

        #expect(result.restartError != nil,
                "coordinator must surface restart failures to the UI")
        #expect(!result.isFullySuccessful,
                "restart failure blocks `isFullySuccessful`")
        // File still removed — deletion runs before the restart request.
        #expect(!FileManager.default.fileExists(atPath: url.path))
        #expect(result.summary.contains("restart request failed"))
    }

    // MARK: - Helpers

    private func makeTempAppSupportDir() throws -> URL {
        let tmp = FileManager.default.temporaryDirectory
            .appendingPathComponent("crashBudgetResetTests-\(UUID().uuidString)",
                                    isDirectory: true)
        try FileManager.default.createDirectory(
            at: tmp, withIntermediateDirectories: true
        )
        return tmp
    }
}

// MARK: - Test doubles

@MainActor
private final class CountingRestarter: DaemonRestarter {
    var restartCount: Int = 0
    func requestRestart() async throws { restartCount += 1 }
}

@MainActor
private final class FailingRestarter: DaemonRestarter {
    let error: Error
    init(error: Error) { self.error = error }
    func requestRestart() async throws { throw error }
}
