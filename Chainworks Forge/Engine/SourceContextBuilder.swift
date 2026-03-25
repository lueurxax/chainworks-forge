import Foundation

// MARK: - SourceContextBuilder (Proposal 007 — Layer I)

/// Materializes the code context that writing/review agents need
/// without relying on hidden cwd state.
struct SourceContextBuilder: Sendable {

    struct SourceContext: Sendable {
        let worktreeRoot: String
        let repoRoot: String
        let baseBranch: String
        let baseRevision: String?
        let targetBranch: String
        let changedFilesManifest: [String]
        let diffSummary: String
    }

    /// Build source context from the current worktree state.
    static func build(
        worktreeRoot: URL,
        repoRoot: String,
        baseBranch: String,
        baseRevision: String?,
        targetBranch: String
    ) async throws -> SourceContext {
        // Get list of changed files
        let changedFiles = try await getChangedFiles(in: worktreeRoot, baseBranch: baseBranch)

        // Get diff summary
        let diffSummary = try await getDiffSummary(in: worktreeRoot, baseBranch: baseBranch)

        return SourceContext(
            worktreeRoot: worktreeRoot.path,
            repoRoot: repoRoot,
            baseBranch: baseBranch,
            baseRevision: baseRevision,
            targetBranch: targetBranch,
            changedFilesManifest: changedFiles,
            diffSummary: diffSummary
        )
    }

    // MARK: - Private

    private static func getChangedFiles(in worktreeRoot: URL, baseBranch: String) async throws -> [String] {
        let output = try await runGit(["diff", "--name-only", baseBranch], in: worktreeRoot)
        return output.components(separatedBy: "\n").filter { !$0.isEmpty }
    }

    private static func getDiffSummary(in worktreeRoot: URL, baseBranch: String) async throws -> String {
        try await runGit(["diff", "--stat", baseBranch], in: worktreeRoot)
    }

    private static func runGit(_ arguments: [String], in directory: URL) async throws -> String {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/git")
        process.arguments = arguments
        process.currentDirectoryURL = directory

        let pipe = Pipe()
        let errorPipe = Pipe()
        process.standardOutput = pipe
        process.standardError = errorPipe

        try process.run()
        process.waitUntilExit()

        let output = String(data: pipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""

        return output
    }
}
