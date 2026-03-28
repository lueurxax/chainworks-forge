import Foundation

// MARK: - WorktreeProvisioner (Proposal 007 — ARCH-067)

/// Creates and manages dedicated writable implementation worktrees.
/// One run = one dedicated writable implementation worktree.
///
/// Naming convention: {worktreeBasePath}/cw-{ideaSlug}-{runShortID}/
/// Example: .chainworks/worktrees/cw-auth-flow-a1b2c3/
struct WorktreeProvisioner: Sendable {

    enum ProvisioningError: Error, LocalizedError {
        case repoRootNotFound(path: String)
        case baseBranchNotFound(branch: String, repoRoot: String)
        case worktreeBasePathNotWritable(path: String)
        case worktreePathOutsideAllowedRoot(path: String, allowedRoot: String)
        case worktreeAlreadyExists(path: String)
        case gitCommandFailed(command: String, output: String)
        case repoIdentityMismatch(expected: String, actual: String)
        case concurrentWorktreeConflict(runID: String)

        var errorDescription: String? {
            switch self {
            case .repoRootNotFound(let path):
                return "Repository root not found: \(path)"
            case .baseBranchNotFound(let branch, let repoRoot):
                return "Base branch '\(branch)' not found in \(repoRoot)"
            case .worktreeBasePathNotWritable(let path):
                return "Worktree base path is not writable: \(path)"
            case .worktreePathOutsideAllowedRoot(let path, let allowedRoot):
                return "Worktree path \(path) is outside allowed root \(allowedRoot)"
            case .worktreeAlreadyExists(let path):
                return "Worktree already exists at \(path)"
            case .gitCommandFailed(let command, let output):
                return "Git command failed: \(command)\n\(output)"
            case .repoIdentityMismatch(let expected, let actual):
                return "Repository identity mismatch: expected '\(expected)', found '\(actual)'"
            case .concurrentWorktreeConflict(let runID):
                return "Concurrent worktree conflict for run \(runID)"
            }
        }
    }

    struct ProvisioningResult: Sendable {
        let worktreeRoot: URL
        let baseRevision: String
        let branchName: String
    }

    private struct RepositoryIdentity: Sendable {
        let value: String
        let source: String
    }

    /// Provision a dedicated writable worktree for a repo-backed run.
    ///
    /// Steps per §7.4:
    /// 1. Verify source repository identity
    /// 2. Verify base branch exists
    /// 3. Record base revision
    /// 4. Create the worktree in the configured base path
    /// 5. Ensure path is inside the allowed worktree root
    /// 6. Return frozen worktreeRoot
    func provision(
        repoIdentifier: String,
        repoRoot: String,
        baseBranch: String,
        targetBranch: String,
        worktreeBasePath: String,
        ideaSlug: String,
        runShortID: String
    ) async throws -> ProvisioningResult {
        RuntimeDiagnostics.log("worktreeProvisioner begin repoRoot=\(repoRoot) baseBranch=\(baseBranch) targetBranch=\(targetBranch) worktreeBase=\(worktreeBasePath)")
        let fm = FileManager.default
        let repoURL = URL(fileURLWithPath: repoRoot)

        // Step 1: Verify repo root exists and is a git repo
        guard fm.fileExists(atPath: repoRoot) else {
            throw ProvisioningError.repoRootNotFound(path: repoRoot)
        }

        let repositoryIdentity = try await resolveRepositoryIdentity(
            repoIdentifier: repoIdentifier,
            repoURL: repoURL
        )
        let expectedIdentifier = RepositoryIdentityNormalizer.canonicalIdentifier(
            configuredIdentifier: repoIdentifier,
            repoRoot: repoRoot
        )
        let actualIdentifier = RepositoryIdentityNormalizer.canonicalIdentifier(
            configuredIdentifier: repositoryIdentity.value,
            repoRoot: repoRoot
        )
        guard expectedIdentifier == actualIdentifier else {
            throw ProvisioningError.repoIdentityMismatch(expected: expectedIdentifier, actual: actualIdentifier)
        }

        // Step 2: Verify base branch exists
        let branchCheck = try await runGit(["rev-parse", "--verify", "refs/heads/\(baseBranch)"], in: repoURL)
        guard !branchCheck.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw ProvisioningError.baseBranchNotFound(branch: baseBranch, repoRoot: repoRoot)
        }

        // Step 3: Record base revision
        let baseRevision = try await runGit(["rev-parse", "HEAD"], in: repoURL)
            .trimmingCharacters(in: .whitespacesAndNewlines)

        // Step 4: Create worktree
        let sanitizedSlug = ideaSlug
            .lowercased()
            .replacingOccurrences(of: " ", with: "-")
            .filter { $0.isLetter || $0.isNumber || $0 == "-" }
            .prefix(30)
        let worktreeDirName = "cw-\(sanitizedSlug)-\(runShortID)"
        let worktreeBase = URL(fileURLWithPath: worktreeBasePath)
        let worktreeRoot = worktreeBase.appendingPathComponent(worktreeDirName, isDirectory: true)

        // Step 5: Ensure path is inside allowed root
        let resolvedWorktree = worktreeRoot.standardizedFileURL.path
        let resolvedAllowed = worktreeBase.standardizedFileURL.path
        guard resolvedWorktree.hasPrefix(resolvedAllowed + "/") || resolvedWorktree == resolvedAllowed else {
            throw ProvisioningError.worktreePathOutsideAllowedRoot(
                path: resolvedWorktree, allowedRoot: resolvedAllowed
            )
        }

        guard !fm.fileExists(atPath: worktreeRoot.path) else {
            throw ProvisioningError.worktreeAlreadyExists(path: worktreeRoot.path)
        }

        // Create worktree base directory if needed
        try fm.createDirectory(at: worktreeBase, withIntermediateDirectories: true)

        // The frozen target branch is the release branch truth for the run.
        // Provision the worktree directly on that branch instead of inventing
        // a second runtime-only branch name that later release services reject.
        let branchName = targetBranch.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            ? "chainworks/\(worktreeDirName)"
            : targetBranch
        _ = try await runGit(
            ["worktree", "add", "-b", branchName, worktreeRoot.path, baseBranch],
            in: repoURL
        )

        RuntimeDiagnostics.log("worktreeProvisioner success branch=\(branchName) worktreeRoot=\(worktreeRoot.path)")

        return ProvisioningResult(
            worktreeRoot: worktreeRoot,
            baseRevision: baseRevision,
            branchName: branchName
        )
    }

    /// Remove a worktree (cleanup after run completes or is cancelled).
    func cleanup(worktreeRoot: URL, repoRoot: String) async throws {
        let repoURL = URL(fileURLWithPath: repoRoot)
        _ = try await runGit(["worktree", "remove", "--force", worktreeRoot.path], in: repoURL)
    }

    // MARK: - Private: Git Command Runner

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
            throw ProvisioningError.gitCommandFailed(
                command: "git \(arguments.joined(separator: " "))",
                output: errorOutput.isEmpty ? output : errorOutput
            )
        }

        return output
    }

    private func resolveRepositoryIdentity(
        repoIdentifier: String,
        repoURL: URL
    ) async throws -> RepositoryIdentity {
        if let remoteOutput = try? await runGit(["remote", "get-url", "origin"], in: repoURL) {
            let value = remoteOutput.trimmingCharacters(in: .whitespacesAndNewlines)
            if !value.isEmpty {
                return RepositoryIdentity(value: value, source: "origin")
            }
        }

        let repoBasename = repoURL.lastPathComponent.trimmingCharacters(in: .whitespacesAndNewlines)
        if !repoBasename.isEmpty {
            return RepositoryIdentity(value: repoBasename, source: "repo_root")
        }

        throw ProvisioningError.repoIdentityMismatch(expected: repoIdentifier, actual: "")
    }
}
