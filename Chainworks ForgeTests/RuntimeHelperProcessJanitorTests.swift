import Testing
import Foundation
@testable import Chainworks_Forge

@MainActor
struct RuntimeHelperProcessJanitorTests {
    private final class JanitorSpy: RuntimeHelperProcessJanitorProtocol, @unchecked Sendable {
        private(set) var sweepCallCount = 0

        func sweepStaleHelpers() {
            sweepCallCount += 1
        }
    }

    @Test("Runtime helper janitor only targets stale orphaned ACP helpers")
    func helperJanitorTargetsOnlyStaleOrphanedHelpers() {
        let snapshots: [RuntimeHelperProcessJanitor.ProcessSnapshot] = [
            .init(pid: 101, ppid: 1, elapsedTime: "10:01", command: "node /opt/homebrew/bin/claude-agent-acp"),
            .init(pid: 102, ppid: 1, elapsedTime: "09:59", command: "node /opt/homebrew/bin/codex-acp"),
            .init(pid: 103, ppid: 1, elapsedTime: "00:20", command: "node /opt/homebrew/bin/claude-agent-acp"),
            .init(pid: 104, ppid: 500, elapsedTime: "30:00", command: "/Applications/Xcode.app/Contents/Developer/usr/bin/mcpbridge"),
            .init(pid: 105, ppid: 1, elapsedTime: "15:00", command: "/Applications/Xcode.app/Contents/Developer/usr/bin/mcpbridge"),
            .init(pid: 106, ppid: 1, elapsedTime: "15:00", command: "/usr/bin/python3 something-else.py")
        ]
        var terminated: [Int32] = []
        let janitor = RuntimeHelperProcessJanitor(
            listProcesses: { snapshots },
            terminateProcess: { terminated.append($0) }
        )

        janitor.sweepStaleHelpers()

        #expect(terminated == [101, 102, 105])
    }

    @Test("Transport factory sweeps stale helpers before creating ACP transport")
    func transportFactorySweepsStaleHelpersBeforeCreatingACPTransport() throws {
        let janitor = JanitorSpy()
        let factory = DefaultRuntimeTransportFactory(
            fixtureTransport: nil,
            helperProcessJanitor: janitor
        )
        let agent = makeTestAgent()
        let binding = ResolvedProviderBinding(
            agentID: "test_agent",
            backendProfileID: nil,
            configuredProviderID: UUID(),
            providerFamily: "codex_acp",
            providerIdentifier: "codex",
            model: "gpt-5",
            effort: "medium",
            transport: "acp_stdio",
            adapterVersion: "v1",
            adapterFamily: "codex_acp",
            capabilityClass: .controlCapable
        )

        _ = try factory.transport(for: agent, binding: binding)

        #expect(janitor.sweepCallCount == 1)
    }

    @Test("Runtime helper janitor drains large stdout before waiting for process exit")
    func janitorDrainsLargeStdoutBeforeWait() throws {
        let tempDirectory = FileManager.default.temporaryDirectory
            .appendingPathComponent("janitor-large-stdout-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: tempDirectory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let scriptURL = tempDirectory.appendingPathComponent("large-output.py")
        try """
import sys
sys.stdout.write("x" * 200000)
sys.stdout.flush()
""".write(to: scriptURL, atomically: true, encoding: .utf8)

        let result = try RuntimeHelperProcessJanitor.runProcessAndCaptureStdout(
            executableURL: URL(fileURLWithPath: "/usr/bin/python3"),
            arguments: [scriptURL.path]
        )

        #expect(result.terminationStatus == 0)
        #expect(result.stdout.count == 200000)
    }

    @Test("Runtime helper janitor reports process enumeration failures")
    func helperJanitorReportsEnumerationFailures() {
        enum TestError: LocalizedError {
            case unavailable

            var errorDescription: String? {
                "fixture ps failed"
            }
        }

        var recordedFailure: String?
        let janitor = RuntimeHelperProcessJanitor(
            listProcesses: { throw TestError.unavailable },
            terminateProcess: { _ in },
            recordFailure: { recordedFailure = $0 }
        )

        janitor.sweepStaleHelpers()

        #expect(recordedFailure?.contains("fixture ps failed") == true)
    }
}
