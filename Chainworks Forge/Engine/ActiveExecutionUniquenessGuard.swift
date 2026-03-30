import Foundation
import SwiftData

@MainActor
final class ActiveExecutionUniquenessGuard {
    private let modelContext: ModelContext

    init(modelContext: ModelContext) {
        self.modelContext = modelContext
    }

    func claimOrCreateStageExecution(
        run: Run,
        stageID: String,
        label: String,
        iteration: Int,
        desiredStatus: StageStatus = .running
    ) -> StageExecution {
        let lineageID = Self.stageLineageID(stageID: stageID, iteration: iteration)
        let activeSiblings = run.stageExecutions
            .filter {
                $0.stageID == stageID
                    && Self.activeStageStatuses.contains($0.status)
                    && ($0.lineageID ?? lineageID) == lineageID
            }
            .sorted(by: Self.canonicalStageOrdering)

        if let canonical = activeSiblings.last {
            activateStageExecution(canonical, run: run, desiredStatus: desiredStatus)
            expireDuplicateActiveStages(activeSiblings.dropLast(), protectedID: canonical.id)
            return canonical
        }

        let stage = StageExecution(
            stageID: stageID,
            label: label,
            status: desiredStatus,
            iteration: iteration,
            attemptNumber: 1
        )
        stage.lineageID = lineageID
        stage.activeOwnerToken = UUID().uuidString
        stage.run = run
        modelContext.insert(stage)
        return stage
    }

    func activateStageExecution(
        _ stage: StageExecution,
        run: Run,
        desiredStatus: StageStatus,
        resetSettlement: Bool = true
    ) {
        let lineageID = stage.lineageID ?? Self.stageLineageID(stageID: stage.stageID, iteration: stage.iteration)
        stage.lineageID = lineageID
        stage.status = desiredStatus
        stage.activeOwnerToken = UUID().uuidString
        if resetSettlement {
            stage.completedAt = nil
            stage.settlementKind = nil
            stage.settledAt = nil
        }

        let activeSiblings = run.stageExecutions
            .filter {
                $0.id != stage.id
                    && Self.activeStageStatuses.contains($0.status)
                    && ($0.lineageID ?? Self.stageLineageID(stageID: $0.stageID, iteration: $0.iteration)) == lineageID
            }
        expireDuplicateActiveStages(activeSiblings, protectedID: stage.id)
    }

    func claimOrCreateRequestedApproval(
        run: Run,
        stageID: String,
        lineageID: String
    ) -> Approval {
        let requested = run.approvals
            .filter {
                $0.stageID == stageID
                    && $0.decision == .requested
                    && ($0.lineageID ?? lineageID) == lineageID
            }
            .sorted { $0.requestedAt < $1.requestedAt }

        if let canonical = requested.last {
            canonical.lineageID = canonical.lineageID ?? lineageID
            expireDuplicateRequestedApprovals(requested.dropLast(), protectedID: canonical.id, lineageID: lineageID)
            return canonical
        }

        let approval = Approval(stageID: stageID)
        approval.decision = .requested
        approval.lineageID = lineageID
        approval.run = run
        modelContext.insert(approval)
        return approval
    }

    private func expireDuplicateActiveStages<S: Sequence>(_ duplicates: S, protectedID: UUID) where S.Element == StageExecution {
        let now = Date()
        for stale in duplicates where stale.id != protectedID {
            stale.status = .blocked
            stale.settlementKind = .repaired
            stale.settledAt = stale.settledAt ?? now
            stale.completedAt = stale.completedAt ?? now
            stale.activeOwnerToken = nil
        }
    }

    private func expireDuplicateRequestedApprovals<S: Sequence>(
        _ duplicates: S,
        protectedID: UUID,
        lineageID: String
    ) where S.Element == Approval {
        let now = Date()
        for stale in duplicates where stale.id != protectedID {
            stale.lineageID = stale.lineageID ?? lineageID
            stale.decision = .expired
            stale.repairedAt = stale.repairedAt ?? now
        }
    }

    private static func stageLineageID(stageID: String, iteration: Int) -> String {
        "\(stageID)::iteration:\(iteration)"
    }

    private static func canonicalStageOrdering(_ lhs: StageExecution, _ rhs: StageExecution) -> Bool {
        if lhs.iteration != rhs.iteration {
            return lhs.iteration < rhs.iteration
        }
        if lhs.attemptNumber != rhs.attemptNumber {
            return lhs.attemptNumber < rhs.attemptNumber
        }
        return lhs.startedAt < rhs.startedAt
    }

    private static let activeStageStatuses: Set<StageStatus> = [.running, .ready, .waitingApproval]
}
