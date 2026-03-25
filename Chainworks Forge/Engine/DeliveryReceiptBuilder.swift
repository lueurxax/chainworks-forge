import Foundation

// MARK: - DeliveryReceiptBuilder (Proposal 007 — §12.2)

/// Produces structured receipts, diff summaries, and release manifests.
struct DeliveryReceiptBuilder: Sendable {

    struct DeliveryReceipt: Codable, Sendable {
        let runID: String
        let workflowID: String
        let ideaTitle: String
        let deliveryConfig: DeliveryConfiguration
        let worktreeRoot: String
        let baseRevision: String?
        let releaseResult: ReleaseResultSummary?
        let implementationReviewStatus: String?
        let timestamp: Date
    }

    struct ReleaseResultSummary: Codable, Sendable {
        let commitSHA: String?
        let branch: String?
        let remote: String?
        let filesChanged: Int?
        let succeeded: Bool
        let failureStage: String?
        let failureReason: String?
    }

    /// Build a delivery receipt from run data.
    static func buildReceipt(
        runID: UUID,
        workflowID: String,
        ideaTitle: String,
        deliveryConfig: DeliveryConfiguration,
        worktreeRoot: String,
        baseRevision: String?,
        releaseResult: ReleaseOpsCoordinator.ReleaseResult?,
        implementationReviewStatus: String?
    ) -> DeliveryReceipt {
        let releaseSummary: ReleaseResultSummary?
        if let result = releaseResult {
            releaseSummary = ReleaseResultSummary(
                commitSHA: result.gitManifest?.commitSHA,
                branch: result.gitManifest?.branch,
                remote: result.gitManifest?.remote,
                filesChanged: result.gitManifest?.filesChanged,
                succeeded: result.succeeded,
                failureStage: result.failureStage,
                failureReason: result.failureReason
            )
        } else {
            releaseSummary = nil
        }

        return DeliveryReceipt(
            runID: runID.uuidString,
            workflowID: workflowID,
            ideaTitle: ideaTitle,
            deliveryConfig: deliveryConfig,
            worktreeRoot: worktreeRoot,
            baseRevision: baseRevision,
            releaseResult: releaseSummary,
            implementationReviewStatus: implementationReviewStatus,
            timestamp: Date()
        )
    }
}
