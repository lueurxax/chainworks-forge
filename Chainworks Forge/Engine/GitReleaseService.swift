import Foundation

// MARK: - GitReleaseService (Proposal 007 — §9.3)

/// Deterministic service for commit/push. No free-form agent shelling for release mechanics.
/// Rules:
/// - no source edits
/// - no staging arbitrary extra files outside approved worktree state
/// - no implicit branch guessing
/// - no push if gate not approved
struct GitReleaseService: Sendable {
    private enum DeliveryProofMode: String {
        case happyPath = "happy_path"
        case nonHappyPath = "non_happy_path"
    }

    enum GitReleaseError: Error, LocalizedError {
        case worktreeNotFound(path: String)
        case nothingToCommit
        case pushFailed(output: String)
        case commitFailed(output: String)
        case notOnExpectedBranch(expected: String, actual: String)

        var errorDescription: String? {
            switch self {
            case .worktreeNotFound(let path):
                return "Worktree not found at: \(path)"
            case .nothingToCommit:
                return "Nothing to commit — worktree has no changes"
            case .pushFailed(let output):
                return "Push failed: \(output)"
            case .commitFailed(let output):
                return "Commit failed: \(output)"
            case .notOnExpectedBranch(let expected, let actual):
                return "Expected branch '\(expected)', but currently on '\(actual)'"
            }
        }
    }

    struct ReleaseManifest: Codable, Sendable {
        let commitSHA: String
        let branch: String
        let remote: String
        let commitMessage: String
        let filesChanged: Int
        let insertions: Int
        let deletions: Int
        let timestamp: Date
    }

    struct GitPushReceipt: Codable, Sendable {
        let commitSHA: String
        let remote: String
        let branch: String
        let status: String // "success" | "failed"
        let failureReason: String?
        let timestamp: Date
    }

    /// Execute deterministic commit and push.
    ///
    /// Inputs:
    /// - worktreeRoot: path to the dedicated worktree
    /// - targetBranch: the branch to push to
    /// - commitMessage: structured commit message
    ///
    /// Outputs:
    /// - ReleaseManifest + GitPushReceipt
    func commitAndPush(
        worktreeRoot: URL,
        targetBranch: String,
        commitMessage: String
    ) async throws -> (manifest: ReleaseManifest, receipt: GitPushReceipt) {
        RuntimeDiagnostics.log("gitReleaseService begin worktree=\(worktreeRoot.path) branch=\(targetBranch)")
        if let proofMode = ProcessInfo.processInfo.environment["CHAINWORKS_DELIVERY_PROOF_MODE"]
            .flatMap(DeliveryProofMode.init(rawValue:)) {
            return try await commitAndPushForDeliveryProof(
                worktreeRoot: worktreeRoot,
                targetBranch: targetBranch,
                commitMessage: commitMessage,
                proofMode: proofMode
            )
        }

        let fm = FileManager.default
        guard fm.fileExists(atPath: worktreeRoot.path) else {
            throw GitReleaseError.worktreeNotFound(path: worktreeRoot.path)
        }

        // Verify we're on the expected branch
        let currentBranch = try await runGit(["rev-parse", "--abbrev-ref", "HEAD"], in: worktreeRoot)
            .trimmingCharacters(in: .whitespacesAndNewlines)
        guard currentBranch == targetBranch || currentBranch.hasSuffix(targetBranch) else {
            throw GitReleaseError.notOnExpectedBranch(expected: targetBranch, actual: currentBranch)
        }

        // Check for changes
        let status = try await runGit(["status", "--porcelain"], in: worktreeRoot)
        let statusLineCount = status.split(separator: "\n").count
        let flattenedStatus = status.replacingOccurrences(of: "\n", with: " | ")
        RuntimeDiagnostics.log("gitReleaseService statusLines=\(statusLineCount) raw=\(flattenedStatus)")
        guard !status.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw GitReleaseError.nothingToCommit
        }

        // Stage all changes in the worktree
        _ = try await runGit(["add", "-A"], in: worktreeRoot)

        // Commit
        let commitOutput = try await runGit(["commit", "-m", commitMessage], in: worktreeRoot)
        guard !commitOutput.contains("nothing to commit") else {
            throw GitReleaseError.nothingToCommit
        }

        // Get commit SHA
        let commitSHA = try await runGit(["rev-parse", "HEAD"], in: worktreeRoot)
            .trimmingCharacters(in: .whitespacesAndNewlines)

        // Get diff stat
        let diffStat = try await runGit(["diff", "--stat", "HEAD~1..HEAD"], in: worktreeRoot)
        let (files, ins, dels) = parseDiffStat(diffStat)

        // Push
        let remote = "origin"
        var pushReceipt: GitPushReceipt
        do {
            _ = try await runGit(["push", "-u", remote, currentBranch], in: worktreeRoot)
            pushReceipt = GitPushReceipt(
                commitSHA: commitSHA,
                remote: remote,
                branch: currentBranch,
                status: "success",
                failureReason: nil,
                timestamp: Date()
            )
        } catch let error as GitReleaseError {
            pushReceipt = GitPushReceipt(
                commitSHA: commitSHA,
                remote: remote,
                branch: currentBranch,
                status: "failed",
                failureReason: error.localizedDescription,
                timestamp: Date()
            )
            throw error
        }

        let manifest = ReleaseManifest(
            commitSHA: commitSHA,
            branch: currentBranch,
            remote: remote,
            commitMessage: commitMessage,
            filesChanged: files,
            insertions: ins,
            deletions: dels,
            timestamp: Date()
        )

        return (manifest: manifest, receipt: pushReceipt)
    }

    private func commitAndPushForDeliveryProof(
        worktreeRoot: URL,
        targetBranch: String,
        commitMessage: String,
        proofMode: DeliveryProofMode
    ) async throws -> (manifest: ReleaseManifest, receipt: GitPushReceipt) {
        let fm = FileManager.default
        guard fm.fileExists(atPath: worktreeRoot.path) else {
            throw GitReleaseError.worktreeNotFound(path: worktreeRoot.path)
        }

        let currentBranch = try await runGit(["rev-parse", "--abbrev-ref", "HEAD"], in: worktreeRoot)
            .trimmingCharacters(in: .whitespacesAndNewlines)
        guard currentBranch == targetBranch || currentBranch.hasSuffix(targetBranch) else {
            throw GitReleaseError.notOnExpectedBranch(expected: targetBranch, actual: currentBranch)
        }

        let status = try await runGit(["status", "--porcelain"], in: worktreeRoot)
        let proofStatusLineCount = status.split(separator: "\n").count
        let flattenedProofStatus = status.replacingOccurrences(of: "\n", with: " | ")
        RuntimeDiagnostics.log("gitReleaseService proofStatusLines=\(proofStatusLineCount) raw=\(flattenedProofStatus)")
        guard !status.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw GitReleaseError.nothingToCommit
        }

        _ = try await runGit(["add", "-A"], in: worktreeRoot)
        _ = try await runGit(
            ["-c", "user.name=Chainworks Forge", "-c", "user.email=chainworks-forge@local", "commit", "-m", commitMessage],
            in: worktreeRoot
        )

        let commitSHA = try await runGit(["rev-parse", "HEAD"], in: worktreeRoot)
            .trimmingCharacters(in: .whitespacesAndNewlines)
        let diffStat = try await runGit(["diff", "--stat", "HEAD~1..HEAD"], in: worktreeRoot)
        let (files, ins, dels) = parseDiffStat(diffStat)

        let manifest = ReleaseManifest(
            commitSHA: commitSHA,
            branch: currentBranch,
            remote: "origin",
            commitMessage: commitMessage,
            filesChanged: files,
            insertions: ins,
            deletions: dels,
            timestamp: Date()
        )
        let receipt = GitPushReceipt(
            commitSHA: commitSHA,
            remote: "origin",
            branch: currentBranch,
            status: "success",
            failureReason: proofMode == .nonHappyPath ? "Proof mode will fail during publish stage." : nil,
            timestamp: Date()
        )
        return (manifest, receipt)
    }

    // MARK: - Private

    private func parseDiffStat(_ stat: String) -> (files: Int, insertions: Int, deletions: Int) {
        // Parse "N files changed, M insertions(+), K deletions(-)" from last line
        let lines = stat.components(separatedBy: "\n").filter { !$0.isEmpty }
        guard let summary = lines.last else { return (0, 0, 0) }

        var files = 0, ins = 0, dels = 0
        let parts = summary.components(separatedBy: ",")
        for part in parts {
            let trimmed = part.trimmingCharacters(in: .whitespaces)
            if trimmed.contains("file") {
                files = Int(trimmed.components(separatedBy: " ").first ?? "0") ?? 0
            } else if trimmed.contains("insertion") {
                ins = Int(trimmed.components(separatedBy: " ").first ?? "0") ?? 0
            } else if trimmed.contains("deletion") {
                dels = Int(trimmed.components(separatedBy: " ").first ?? "0") ?? 0
            }
        }
        return (files, ins, dels)
    }

    private func runGit(_ arguments: [String], in directory: URL) async throws -> String {
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
        let errorOutput = String(data: errorPipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""

        guard process.terminationStatus == 0 else {
            if arguments.first == "push" {
                throw GitReleaseError.pushFailed(output: errorOutput.isEmpty ? output : errorOutput)
            } else if arguments.first == "commit" {
                throw GitReleaseError.commitFailed(output: errorOutput.isEmpty ? output : errorOutput)
            }
            throw GitReleaseError.commitFailed(output: errorOutput.isEmpty ? output : errorOutput)
        }

        return output
    }
}
