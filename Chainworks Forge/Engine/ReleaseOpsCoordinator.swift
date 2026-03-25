import Foundation

// MARK: - ReleaseOpsCoordinator (Proposal 007 — §9)

/// Drives commit → push → archive → distribute after approval.
/// Release side effects execute only through deterministic services (ARCH-069).
/// Agents may decide THAT release should happen, but they do not free-form
/// the commit/push/archive/upload mechanics.
struct ReleaseOpsCoordinator: Sendable {

    enum ReleaseError: Error, LocalizedError {
        case missingDeliveryConfiguration
        case missingWorktreeRoot
        case releaseNotApproved
        case partialFailure(stage: String, reason: String)

        var errorDescription: String? {
            switch self {
            case .missingDeliveryConfiguration:
                return "No delivery configuration found for this run"
            case .missingWorktreeRoot:
                return "No worktree root found for this run"
            case .releaseNotApproved:
                return "Release has not been approved"
            case .partialFailure(let stage, let reason):
                return "Partial release failure at \(stage): \(reason)"
            }
        }
    }

    struct ReleaseResult: Sendable {
        let gitManifest: GitReleaseService.ReleaseManifest?
        let gitReceipt: GitReleaseService.GitPushReceipt?
        let bundleManifest: ConnectPublishService.ReleaseBundleManifest?
        let uploadReceipt: ConnectPublishService.ConnectUploadReceipt?
        let succeeded: Bool
        let failureStage: String?
        let failureReason: String?
    }

    private let gitService = GitReleaseService()
    private let publishService = ConnectPublishService()

    /// Execute the full release sequence:
    /// 1. commit_and_push_to_github via GitReleaseService
    /// 2. build_archive_and_push_connect via ConnectPublishService
    ///
    /// Partial failure semantics (§9.4):
    /// - If commit/push succeeds but archive/upload fails,
    ///   receipts remain persisted and run becomes blocked.
    /// - No hidden rollback.
    func executeRelease(
        deliveryConfig: DeliveryConfiguration,
        worktreeRoot: URL,
        commitMessage: String
    ) async -> ReleaseResult {
        // Step 1: Commit and push
        let gitResult: (manifest: GitReleaseService.ReleaseManifest, receipt: GitReleaseService.GitPushReceipt)
        do {
            gitResult = try await gitService.commitAndPush(
                worktreeRoot: worktreeRoot,
                targetBranch: deliveryConfig.targetBranch,
                commitMessage: commitMessage
            )
        } catch {
            return ReleaseResult(
                gitManifest: nil,
                gitReceipt: nil,
                bundleManifest: nil,
                uploadReceipt: nil,
                succeeded: false,
                failureStage: "commit_and_push",
                failureReason: error.localizedDescription
            )
        }

        // Step 2: Build, archive, and upload
        do {
            let publishResult = try await publishService.buildArchiveAndUpload(
                worktreeRoot: worktreeRoot,
                gitPushReceipt: gitResult.receipt,
                releaseManifest: gitResult.manifest,
                releaseTargetID: deliveryConfig.releaseTargetID,
                releaseMode: deliveryConfig.releaseMode
            )

            return ReleaseResult(
                gitManifest: gitResult.manifest,
                gitReceipt: gitResult.receipt,
                bundleManifest: publishResult.bundle,
                uploadReceipt: publishResult.receipt,
                succeeded: true,
                failureStage: nil,
                failureReason: nil
            )
        } catch {
            // Partial failure: commit/push succeeded but archive/upload failed
            return ReleaseResult(
                gitManifest: gitResult.manifest,
                gitReceipt: gitResult.receipt,
                bundleManifest: nil,
                uploadReceipt: nil,
                succeeded: false,
                failureStage: "build_archive_and_push",
                failureReason: error.localizedDescription
            )
        }
    }
}
