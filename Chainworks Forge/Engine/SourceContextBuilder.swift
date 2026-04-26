import Foundation

// MARK: - SourceContextBuilder (Proposal 007 — Layer I)

/// Materializes the code context that writing/review agents need
/// without relying on hidden cwd state.
struct SourceContextBuilder: Sendable {

    enum SourceContextError: LocalizedError, Equatable {
        case gitCommandTimedOut(arguments: [String], timeoutSeconds: TimeInterval)
        case gitCommandFailed(arguments: [String], terminationStatus: Int32, stderr: String)

        var errorDescription: String? {
            switch self {
            case .gitCommandTimedOut(let arguments, let timeoutSeconds):
                return "git \(arguments.joined(separator: " ")) timed out after \(Int(timeoutSeconds))s"
            case .gitCommandFailed(let arguments, let terminationStatus, let stderr):
                let normalizedStderr = stderr.trimmingCharacters(in: .whitespacesAndNewlines)
                if normalizedStderr.isEmpty {
                    return "git \(arguments.joined(separator: " ")) failed with exit code \(terminationStatus)"
                }
                return "git \(arguments.joined(separator: " ")) failed with exit code \(terminationStatus): \(normalizedStderr)"
            }
        }
    }

    struct SourceContext: Codable, Sendable {
        let worktreeRoot: String
        let repoRoot: String
        let baseBranch: String
        let baseRevision: String?
        let targetBranch: String
        let changedFilesManifest: [String]
        let diffSummary: String
    }

    static var gitCommandTimeoutSeconds: TimeInterval = 15
    static var gitRunner: @Sendable ([String], URL, TimeInterval) async throws -> String = defaultRunGit

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
        try await gitRunner(arguments, directory, gitCommandTimeoutSeconds)
    }

    private static func defaultRunGit(
        _ arguments: [String],
        _ directory: URL,
        _ timeoutSeconds: TimeInterval
    ) async throws -> String {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/git")
        process.arguments = arguments
        process.currentDirectoryURL = directory

        let outputPipe = Pipe()
        let errorPipe = Pipe()
        process.standardOutput = outputPipe
        process.standardError = errorPipe

        try process.run()

        let stdoutReader = Task.detached(priority: .utility) {
            outputPipe.fileHandleForReading.readDataToEndOfFile()
        }
        let stderrReader = Task.detached(priority: .utility) {
            errorPipe.fileHandleForReading.readDataToEndOfFile()
        }

        do {
            let terminationStatus = try await waitForTermination(
                of: process,
                arguments: arguments,
                timeoutSeconds: timeoutSeconds
            )
            let stdoutData = await stdoutReader.value
            let stderrData = await stderrReader.value
            let stdout = String(data: stdoutData, encoding: .utf8) ?? ""
            let stderr = String(data: stderrData, encoding: .utf8) ?? ""

            guard terminationStatus == 0 else {
                throw SourceContextError.gitCommandFailed(
                    arguments: arguments,
                    terminationStatus: terminationStatus,
                    stderr: stderr
                )
            }

            return stdout
        } catch {
            if process.isRunning {
                process.terminate()
            }
            _ = await stdoutReader.value
            _ = await stderrReader.value
            throw error
        }
    }

    private static func waitForTermination(
        of process: Process,
        arguments: [String],
        timeoutSeconds: TimeInterval
    ) async throws -> Int32 {
        try await withThrowingTaskGroup(of: Int32.self) { group in
            group.addTask {
                await withCheckedContinuation { continuation in
                    process.terminationHandler = { terminatedProcess in
                        continuation.resume(returning: terminatedProcess.terminationStatus)
                    }
                }
            }
            group.addTask {
                let durationNanoseconds = UInt64(max(timeoutSeconds, 0.1) * 1_000_000_000)
                try await Task.sleep(nanoseconds: durationNanoseconds)
                if process.isRunning {
                    process.terminate()
                }
                throw SourceContextError.gitCommandTimedOut(
                    arguments: arguments,
                    timeoutSeconds: timeoutSeconds
                )
            }

            let status = try await group.next()!
            group.cancelAll()
            process.terminationHandler = nil
            return status
        }
    }
}
