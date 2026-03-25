import Foundation

// MARK: - RepoSafetyGuard (Proposal 007 — §7.7)

/// Enforces repo identity, base branch, path boundaries, and write scope.
/// Before any file operation or tool call:
/// - target path must be under workspaceRoot or worktreeRoot
/// - release services must refuse any repo root mismatch
/// - a violation blocks the run immediately
struct RepoSafetyGuard: Sendable {

    enum SafetyViolation: Error, LocalizedError {
        case pathOutsideBoundary(path: String, boundary: String)
        case repoRootMismatch(expected: String, actual: String)
        case worktreeNotProvisioned
        case concurrentWriteAttempt(runID: String)
        case missingDeliveryConfiguration

        var errorDescription: String? {
            switch self {
            case .pathOutsideBoundary(let path, let boundary):
                return "Path '\(path)' is outside the allowed boundary '\(boundary)'"
            case .repoRootMismatch(let expected, let actual):
                return "Repository root mismatch: expected '\(expected)', found '\(actual)'"
            case .worktreeNotProvisioned:
                return "Worktree has not been provisioned for this run"
            case .concurrentWriteAttempt(let runID):
                return "Concurrent write attempt detected for run \(runID)"
            case .missingDeliveryConfiguration:
                return "No delivery configuration found for this run"
            }
        }
    }

    /// Validate that a target path is within the allowed workspace or worktree boundary.
    static func validatePath(
        _ targetPath: String,
        workspaceRoot: String,
        worktreeRoot: String?
    ) throws {
        let resolvedTarget = URL(fileURLWithPath: targetPath).standardizedFileURL.path
        let resolvedWorkspace = URL(fileURLWithPath: workspaceRoot).standardizedFileURL.path

        // Check workspace boundary
        if resolvedTarget.hasPrefix(resolvedWorkspace + "/") || resolvedTarget == resolvedWorkspace {
            return
        }

        // Check worktree boundary if provisioned
        if let worktreeRoot {
            let resolvedWorktree = URL(fileURLWithPath: worktreeRoot).standardizedFileURL.path
            if resolvedTarget.hasPrefix(resolvedWorktree + "/") || resolvedTarget == resolvedWorktree {
                return
            }
        }

        throw SafetyViolation.pathOutsideBoundary(
            path: targetPath,
            boundary: worktreeRoot ?? workspaceRoot
        )
    }

    /// Validate that the repo root matches the delivery configuration.
    static func validateRepoIdentity(
        expectedRepoRoot: String,
        actualRepoRoot: String
    ) throws {
        let expected = URL(fileURLWithPath: expectedRepoRoot).standardizedFileURL.path
        let actual = URL(fileURLWithPath: actualRepoRoot).standardizedFileURL.path
        guard expected == actual else {
            throw SafetyViolation.repoRootMismatch(expected: expectedRepoRoot, actual: actualRepoRoot)
        }
    }

    /// Validate that a delivery configuration exists and has a provisioned worktree.
    static func validateWorktreeReady(worktreeRoot: String?) throws {
        guard let worktreeRoot, !worktreeRoot.isEmpty else {
            throw SafetyViolation.worktreeNotProvisioned
        }
        guard FileManager.default.fileExists(atPath: worktreeRoot) else {
            throw SafetyViolation.worktreeNotProvisioned
        }
    }
}
