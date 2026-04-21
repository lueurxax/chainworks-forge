// P042 §6.2 / REQ-006 / UI-001 crash-budget reset coordinator tests.

import XCTest
@testable import Chainworks_Forge

final class CrashBudgetResetTests: XCTestCase {

    // MARK: - File primitives

    func test_delete_crash_budget_file_removes_file_when_present() throws {
        let tmp = try makeTempAppSupportDir()
        defer { try? FileManager.default.removeItem(at: tmp) }
        let url = CrashBudgetFiles.crashBudgetURL(appSupportDir: tmp)
        try Data("{}".utf8).write(to: url)
        XCTAssertTrue(FileManager.default.fileExists(atPath: url.path))

        let outcome = CrashBudgetFiles.deleteCrashBudgetFile(at: url)

        switch outcome {
        case .removed: break
        default: XCTFail("expected .removed, got \(outcome)")
        }
        XCTAssertFalse(FileManager.default.fileExists(atPath: url.path))
    }

    func test_delete_crash_budget_file_reports_already_absent_when_missing() throws {
        let tmp = try makeTempAppSupportDir()
        defer { try? FileManager.default.removeItem(at: tmp) }
        let url = CrashBudgetFiles.crashBudgetURL(appSupportDir: tmp)
        XCTAssertFalse(FileManager.default.fileExists(atPath: url.path))

        let outcome = CrashBudgetFiles.deleteCrashBudgetFile(at: url)

        switch outcome {
        case .alreadyAbsent: break
        default: XCTFail("expected .alreadyAbsent, got \(outcome)")
        }
    }

    // MARK: - Coordinator

    @MainActor
    func test_perform_reset_deletes_file_and_requests_single_restart() throws {
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

        let result = coordinator.performReset()

        XCTAssertTrue(result.isFullySuccessful)
        XCTAssertEqual(restarter.restartCount, 1,
                       "reset must request exactly one daemon restart")
        XCTAssertFalse(FileManager.default.fileExists(atPath: url.path),
                       "crash-budget file must be deleted")
    }

    @MainActor
    func test_perform_reset_noop_when_already_absent_still_requests_restart() throws {
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

        let result = coordinator.performReset()

        XCTAssertTrue(result.isFullySuccessful)
        XCTAssertEqual(restarter.restartCount, 1)
        if case .alreadyAbsent = result.fileOutcome {
            // Good.
        } else {
            XCTFail("expected .alreadyAbsent, got \(result.fileOutcome)")
        }
    }

    @MainActor
    func test_perform_reset_reports_restart_error_while_still_deleting_file() throws {
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

        let result = coordinator.performReset()

        XCTAssertNotNil(result.restartError,
                        "coordinator must surface restart failures to the UI")
        XCTAssertFalse(result.isFullySuccessful,
                       "restart failure blocks `isFullySuccessful`")
        // File still removed — deletion runs before the restart request.
        XCTAssertFalse(FileManager.default.fileExists(atPath: url.path))
        XCTAssertTrue(result.summary.contains("restart request failed"))
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
    func requestRestart() throws { restartCount += 1 }
}

@MainActor
private final class FailingRestarter: DaemonRestarter {
    let error: Error
    init(error: Error) { self.error = error }
    func requestRestart() throws { throw error }
}
