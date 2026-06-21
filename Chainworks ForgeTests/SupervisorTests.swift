// P042 §6.1 supervisor-owned anomalous PID-lock UX tests.

import Combine
import Foundation
import Testing
@testable import Chainworks_Forge

@MainActor
struct SupervisorTests {

    // MARK: - Classifier

    @Test func `Classify exit 75 is anomalous pid lock`() {
        let exit = DaemonProcessExit.classify(status: 75, reason: .exit)
        #expect(exit == .anomalousPidLock)
    }

    @Test func `Classify exit 0 is clean or duplicate healthy`() {
        // §6.1 DuplicateHealthy exits 0 intentionally. Clean shutdown
        // also exits 0. Classifier reports `.cleanExit` in both cases;
        // the distinction is drawn by the caller (it knows whether
        // the lock was contended before the spawn).
        let exit = DaemonProcessExit.classify(status: 0, reason: .exit)
        #expect(exit == .cleanExit(code: 0))
    }

    @Test func `Classify non zero exit is unknown failure`() {
        let exit = DaemonProcessExit.classify(status: 1, reason: .exit)
        #expect(exit == .unknownFailure(code: 1))
    }

    @Test func `Classify uncaught signal is signalled`() {
        let exit = DaemonProcessExit.classify(status: 9, reason: .uncaughtSignal)
        #expect(exit == .signalled(signal: 9))
    }

    // MARK: - Supervisor publisher

    @Test func `Supervisor records exit and publishes anomalous pid lock`() async {
        let supervisor = DaemonProcessSupervisor()
        var seenAnomalous = 0
        let cancellable = supervisor.anomalousPidLockPublisher.sink { _ in
            seenAnomalous += 1
        }
        defer { cancellable.cancel() }

        supervisor.record(.cleanExit(code: 0))
        #expect(supervisor.lastExit == .cleanExit(code: 0))
        #expect(seenAnomalous == 0)

        supervisor.record(.anomalousPidLock)
        #expect(supervisor.lastExit == .anomalousPidLock)
        #expect(seenAnomalous == 1)

        // A second anomalous exit publishes again — the banner UI
        // treats each event as a new dialog opportunity so an
        // operator who dismissed the first alert still sees a
        // recurrence.
        supervisor.record(.anomalousPidLock)
        #expect(seenAnomalous == 2)
    }

    @Test func `Supervisor records raw status through classifier`() {
        let supervisor = DaemonProcessSupervisor()
        supervisor.record(status: 75, reason: .exit)
        #expect(supervisor.lastExit == .anomalousPidLock)
    }

    // MARK: - App-surfaces contract (AC-3 / AC-12)

    @Test func `App surfaces pid lock dialog on exit 75 before ready`() async {
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

        #expect(
            receivedDialogSignals == 1,
            "UI must receive exactly one dialog signal per exit-75 event"
        )
        #expect(
            supervisor.lastExit == .anomalousPidLock,
            "lastExit must expose the typed anomalous-pid-lock classification"
        )
    }

    // MARK: - R13 REL-001 crash-budget writer contract

    /// §6.2 classification rule: clean and duplicate-healthy exits are
    /// the "does not count" half of the budget; every other exit
    /// counts. Pins the `isAbnormal` vocabulary so later classifier
    /// additions don't silently grow the non-counting set.
    @Test func `Is abnormal classifies per 6 2 rule`() {
        #expect(!DaemonProcessExit.cleanExit(code: 0).isAbnormal)
        #expect(!DaemonProcessExit.duplicateHealthy.isAbnormal)
        #expect(DaemonProcessExit.anomalousPidLock.isAbnormal)
        #expect(DaemonProcessExit.unknownFailure(code: 1).isAbnormal)
        #expect(DaemonProcessExit.signalled(signal: 9).isAbnormal)
    }

    @Test func `Abnormal exit increments crash budget file`() throws {
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
        #expect(decoded.first_crash_at == 1_000_000)
        #expect(decoded.crash_count == 1)

        // Second abnormal exit within 60 s: count = 2, window unchanged.
        fakeNow = 1_000_030 // +30 s
        supervisor.record(.anomalousPidLock)
        decoded = try readBudget(budget)
        #expect(decoded.first_crash_at == 1_000_000)
        #expect(decoded.crash_count == 2)
    }

    @Test func `Abnormal exit after window opens new window`() throws {
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
        #expect(decoded.first_crash_at == 1_000_061)
        #expect(decoded.crash_count == 1,
                "window expiry must reset the counter to 1 per §6.2")
    }

    @Test func `Clean exit does not touch crash budget file`() throws {
        let dir = try makeTempAppSupportDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let budget = CrashBudgetFiles.crashBudgetURL(appSupportDir: dir)
        let supervisor = DaemonProcessSupervisor(
            crashBudgetURL: budget,
            nowSecondsProvider: { 1_000_000 }
        )

        // Clean exit must not create the file.
        supervisor.record(.cleanExit(code: 0))
        #expect(!FileManager.default.fileExists(atPath: budget.path))

        // Duplicate-healthy exit also does not count.
        supervisor.record(.duplicateHealthy)
        #expect(!FileManager.default.fileExists(atPath: budget.path))
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
