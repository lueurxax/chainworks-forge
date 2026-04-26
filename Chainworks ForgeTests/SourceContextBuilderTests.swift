import Foundation
import Testing
@testable import Chainworks_Forge

@Suite("SourceContextBuilder", .tags(.fast))
struct SourceContextBuilderTests {
    @Test("Source context git runner times out instead of hanging forever")
    func sourceContextBuilderTimesOutStuckGitRunner() async throws {
        let originalRunner = SourceContextBuilder.gitRunner
        let originalTimeout = SourceContextBuilder.gitCommandTimeoutSeconds
        defer {
            SourceContextBuilder.gitRunner = originalRunner
            SourceContextBuilder.gitCommandTimeoutSeconds = originalTimeout
        }

        SourceContextBuilder.gitCommandTimeoutSeconds = 0.1
        SourceContextBuilder.gitRunner = { _, _, _ in
            try await Task.sleep(nanoseconds: 2_000_000_000)
            return ""
        }

        await #expect(throws: SourceContextBuilder.SourceContextError.self) {
            _ = try await SourceContextBuilder.build(
                worktreeRoot: URL(fileURLWithPath: "/tmp"),
                repoRoot: "/tmp/repo",
                baseBranch: "main",
                baseRevision: nil,
                targetBranch: "dogfood/test"
            )
        }
    }
}
