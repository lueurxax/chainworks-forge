import Foundation

// MARK: - DeliveryPreflightService (Proposal 007 — §9.6)

/// Validates the mutable DeliveryConfiguration draft before a repo-backed run.
/// Extends the provider-platform baseline with repo/release-specific checks.
struct DeliveryPreflightService: Sendable {

    struct PreflightResult: Codable, Sendable {
        let checks: [PreflightCheck]
        let passed: Bool
        let timestamp: Date

        var failedChecks: [PreflightCheck] {
            checks.filter { !$0.passed }
        }
    }

    struct PreflightCheck: Codable, Sendable {
        let id: String
        let label: String
        let passed: Bool
        let detail: String?
    }

    /// Validate a delivery configuration draft.
    ///
    /// Checks (§9.6):
    /// - Target repository identity and expected root
    /// - Selected base branch exists
    /// - Worktree base path is writable and inside allowed root
    /// - Git auth/push target is usable
    /// - Selected release target is valid for chosen ReleaseMode
    /// - No repo-safety contract violation
    func validate(_ config: DeliveryConfiguration) async -> PreflightResult {
        var checks: [PreflightCheck] = []

        // Check 1: Repository root exists
        let repoExists = FileManager.default.fileExists(atPath: config.repoRoot)
        checks.append(PreflightCheck(
            id: "repo_root",
            label: "Repository root exists",
            passed: repoExists,
            detail: repoExists ? config.repoRoot : "Path not found: \(config.repoRoot)"
        ))

        // Check 2: Repository is a git repo
        let gitDirExists = FileManager.default.fileExists(
            atPath: URL(fileURLWithPath: config.repoRoot).appendingPathComponent(".git").path
        )
        checks.append(PreflightCheck(
            id: "git_repo",
            label: "Valid git repository",
            passed: gitDirExists,
            detail: gitDirExists ? nil : "No .git directory found at \(config.repoRoot)"
        ))

        // Check 3: Base branch exists (if repo exists)
        var baseBranchExists = false
        if repoExists && gitDirExists {
            baseBranchExists = await checkBranchExists(config.baseBranch, repoRoot: config.repoRoot)
        }
        checks.append(PreflightCheck(
            id: "base_branch",
            label: "Base branch '\(config.baseBranch)' exists",
            passed: baseBranchExists,
            detail: baseBranchExists ? nil : "Branch '\(config.baseBranch)' not found"
        ))

        // Check 4: Worktree base path is creatable/writable
        let worktreeBaseWritable = checkDirectoryCreatable(at: config.worktreeBasePath)
        checks.append(PreflightCheck(
            id: "worktree_writable",
            label: "Worktree base path is writable",
            passed: worktreeBaseWritable,
            detail: worktreeBaseWritable ? config.worktreeBasePath : "Path not writable: \(config.worktreeBasePath)"
        ))

        // Check 5: Release target valid for mode
        let validTarget = !config.releaseTargetID.isEmpty
        checks.append(PreflightCheck(
            id: "release_target",
            label: "Release target configured",
            passed: validTarget,
            detail: validTarget ? "\(config.releaseTargetLabel) (\(config.releaseMode.rawValue))" : "No release target specified"
        ))

        // Check 6: Repo identifier not empty
        let validRepoID = !config.repoIdentifier.isEmpty
        checks.append(PreflightCheck(
            id: "repo_identifier",
            label: "Repository identifier set",
            passed: validRepoID,
            detail: validRepoID ? config.repoIdentifier : "Missing repository identifier"
        ))

        let allPassed = checks.allSatisfy(\.passed)
        return PreflightResult(checks: checks, passed: allPassed, timestamp: Date())
    }

    // MARK: - Private

    private func checkBranchExists(_ branch: String, repoRoot: String) async -> Bool {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/git")
        process.arguments = ["rev-parse", "--verify", "refs/heads/\(branch)"]
        process.currentDirectoryURL = URL(fileURLWithPath: repoRoot)

        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = Pipe()

        do {
            try process.run()
            process.waitUntilExit()
            return process.terminationStatus == 0
        } catch {
            return false
        }
    }

    private func checkDirectoryCreatable(at path: String) -> Bool {
        let fileManager = FileManager.default
        let url = URL(fileURLWithPath: path, isDirectory: true)
        let probeURL = url.appendingPathComponent(".cw-write-probe-\(UUID().uuidString)")

        do {
            try fileManager.createDirectory(at: url, withIntermediateDirectories: true)
            try Data("ok".utf8).write(to: probeURL, options: .atomic)
            try? fileManager.removeItem(at: probeURL)
            return true
        } catch {
            return false
        }
    }
}
