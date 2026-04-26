// P042 §6.1 supervisor-owned anomalous PID-lock UX tests.

import Combine
import XCTest
@testable import Chainworks_Forge

final class SupervisorTests: XCTestCase {

    // MARK: - Classifier

    func test_classify_exit_75_is_anomalous_pid_lock() {
        let exit = DaemonProcessExit.classify(status: 75, reason: .exit)
        XCTAssertEqual(exit, .anomalousPidLock)
    }

    func test_classify_exit_0_is_clean_or_duplicate_healthy() {
        // §6.1 DuplicateHealthy exits 0 intentionally. Clean shutdown
        // also exits 0. Classifier reports `.cleanExit` in both cases;
        // the distinction is drawn by the caller (it knows whether
        // the lock was contended before the spawn).
        let exit = DaemonProcessExit.classify(status: 0, reason: .exit)
        XCTAssertEqual(exit, .cleanExit(code: 0))
    }

    func test_classify_non_zero_exit_is_unknown_failure() {
        let exit = DaemonProcessExit.classify(status: 1, reason: .exit)
        XCTAssertEqual(exit, .unknownFailure(code: 1))
    }

    func test_classify_uncaught_signal_is_signalled() {
        let exit = DaemonProcessExit.classify(status: 9, reason: .uncaughtSignal)
        XCTAssertEqual(exit, .signalled(signal: 9))
    }

    // MARK: - Supervisor publisher

    @MainActor
    func test_supervisor_records_exit_and_publishes_anomalous_pid_lock() async {
        let supervisor = DaemonProcessSupervisor()
        var seenAnomalous = 0
        let cancellable = supervisor.anomalousPidLockPublisher.sink { _ in
            seenAnomalous += 1
        }
        defer { cancellable.cancel() }

        supervisor.record(.cleanExit(code: 0))
        XCTAssertEqual(supervisor.lastExit, .cleanExit(code: 0))
        XCTAssertEqual(seenAnomalous, 0)

        supervisor.record(.anomalousPidLock)
        XCTAssertEqual(supervisor.lastExit, .anomalousPidLock)
        XCTAssertEqual(seenAnomalous, 1)

        // A second anomalous exit publishes again — the banner UI
        // treats each event as a new dialog opportunity so an
        // operator who dismissed the first alert still sees a
        // recurrence.
        supervisor.record(.anomalousPidLock)
        XCTAssertEqual(seenAnomalous, 2)
    }

    @MainActor
    func test_supervisor_records_raw_status_through_classifier() {
        let supervisor = DaemonProcessSupervisor()
        supervisor.record(status: 75, reason: .exit)
        XCTAssertEqual(supervisor.lastExit, .anomalousPidLock)
    }

    // MARK: - App-surfaces contract (AC-3 / AC-12)

    @MainActor
    func test_app_surfaces_pid_lock_dialog_on_exit_75_before_ready() async {
        // This is the P042 §10.2 canonical test name. It validates
        // that an exit-75 event from the supervised daemon causes the
        // supervisor to publish an anomalous-pid-lock notification —
        // which the UI layer (`DaemonLifecycleBanner` or a dedicated
        // alert presenter) binds to and renders as a dialog BEFORE
        // any `DaemonStatus` Ready frame could arrive. The daemon has
        // not reached HTTP bind in this scenario, so
        // `DaemonStatusViewModel.status` stays `nil`.
        let supervisor = DaemonProcessSupervisor()
        var receivedDialogSignals = 0
        let cancellable = supervisor.anomalousPidLockPublisher.sink { _ in
            receivedDialogSignals += 1
        }
        defer { cancellable.cancel() }

        // Synthetic exit-75 from a pre-bind supervised daemon.
        supervisor.record(status: 75, reason: .exit)

        XCTAssertEqual(
            receivedDialogSignals,
            1,
            "UI must receive exactly one dialog signal per exit-75 event"
        )
        XCTAssertEqual(
            supervisor.lastExit,
            .anomalousPidLock,
            "lastExit must expose the typed anomalous-pid-lock classification"
        )
    }

    // MARK: - R13 REL-001 crash-budget writer contract

    /// §6.2 classification rule: clean and duplicate-healthy exits are
    /// the "does not count" half of the budget; every other exit
    /// counts. Pins the `isAbnormal` vocabulary so later classifier
    /// additions don't silently grow the non-counting set.
    func test_is_abnormal_classifies_per_6_2_rule() {
        XCTAssertFalse(DaemonProcessExit.cleanExit(code: 0).isAbnormal)
        XCTAssertFalse(DaemonProcessExit.duplicateHealthy.isAbnormal)
        XCTAssertTrue(DaemonProcessExit.anomalousPidLock.isAbnormal)
        XCTAssertTrue(DaemonProcessExit.unknownFailure(code: 1).isAbnormal)
        XCTAssertTrue(DaemonProcessExit.signalled(signal: 9).isAbnormal)
    }

    @MainActor
    func test_abnormal_exit_increments_crash_budget_file() throws {
        let dir = try makeTempAppSupportDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let budget = CrashBudgetFiles.crashBudgetURL(appSupportDir: dir)
        var fakeNow: UInt64 = 1_000_000
        let supervisor = DaemonProcessSupervisor(
            crashBudgetURL: budget,
            nowSecondsProvider: { fakeNow }
        )

        // First abnormal exit: empty file → (first_crash_at = now, count = 1).
        supervisor.record(.unknownFailure(code: 1))
        var decoded = try readBudget(budget)
        XCTAssertEqual(decoded.first_crash_at, 1_000_000)
        XCTAssertEqual(decoded.crash_count, 1)

        // Second abnormal exit within 60 s: count = 2, window unchanged.
        fakeNow = 1_000_030 // +30 s
        supervisor.record(.anomalousPidLock)
        decoded = try readBudget(budget)
        XCTAssertEqual(decoded.first_crash_at, 1_000_000)
        XCTAssertEqual(decoded.crash_count, 2)
    }

    @MainActor
    func test_abnormal_exit_after_window_opens_new_window() throws {
        let dir = try makeTempAppSupportDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let budget = CrashBudgetFiles.crashBudgetURL(appSupportDir: dir)
        var fakeNow: UInt64 = 1_000_000
        let supervisor = DaemonProcessSupervisor(
            crashBudgetURL: budget,
            nowSecondsProvider: { fakeNow }
        )

        supervisor.record(.unknownFailure(code: 1))
        // 61 seconds later — past the 60 s window. A new window opens
        // at the new `now`, count resets to 1.
        fakeNow += 61
        supervisor.record(.signalled(signal: 9))
        let decoded = try readBudget(budget)
        XCTAssertEqual(decoded.first_crash_at, 1_000_061)
        XCTAssertEqual(decoded.crash_count, 1,
                       "window expiry must reset the counter to 1 per §6.2")
    }

    @MainActor
    func test_clean_exit_does_not_touch_crash_budget_file() throws {
        let dir = try makeTempAppSupportDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let budget = CrashBudgetFiles.crashBudgetURL(appSupportDir: dir)
        let supervisor = DaemonProcessSupervisor(
            crashBudgetURL: budget,
            nowSecondsProvider: { 1_000_000 }
        )

        // Clean exit must not create the file.
        supervisor.record(.cleanExit(code: 0))
        XCTAssertFalse(FileManager.default.fileExists(atPath: budget.path))

        // Duplicate-healthy exit also does not count.
        supervisor.record(.duplicateHealthy)
        XCTAssertFalse(FileManager.default.fileExists(atPath: budget.path))
    }

    // MARK: - Helpers

    private func makeTempAppSupportDir() throws -> URL {
        let tmp = FileManager.default.temporaryDirectory
            .appendingPathComponent("supervisorCrashBudgetTests-\(UUID().uuidString)",
                                    isDirectory: true)
        try FileManager.default.createDirectory(
            at: tmp, withIntermediateDirectories: true
        )
        return tmp
    }

    private func readBudget(_ url: URL) throws -> CrashBudgetFile {
        let data = try Data(contentsOf: url)
        return try JSONDecoder().decode(CrashBudgetFile.self, from: data)
    }
}
