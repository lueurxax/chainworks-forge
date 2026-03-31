import Foundation
import SwiftData

// MARK: - Proposal 013 Layer N: Stage Retry Coordinator

/// Owns retry-in-place versus clone-run semantics and prevents
/// stage/attempt identity drift.
///
/// Three distinct recovery actions (§5.2):
/// 1. Retry Failed Agent — same run, same stage, same stage attempt, new agent attempt
/// 2. Retry Failed Stage — same run, same stage lineage, new stage attempt
/// 3. Clone Run — new run, old run becomes terminal history
@MainActor
final class StageRetryCoordinator {

    private let modelContext: ModelContext

    init(modelContext: ModelContext) {
        self.modelContext = modelContext
    }

    // MARK: - Retry Failed Agent (§5.2, Rule 1)

    /// Retry a single failed agent within the current stage attempt.
    /// Does NOT increment the stage attempt number.
    /// Creates a new agent execution with proper lineage.
    func retryFailedAgent(
        run: Run,
        stage: StageExecution,
        failedAgent: AgentExecution
    ) throws -> AgentExecution {
        guard failedAgent.status == .failed else {
            throw StageRetryError.agentNotFailed(failedAgent.agentID)
        }

        // Compute next agent attempt number
        let existingAttempts = stage.agentExecutions
            .filter { $0.agentID == failedAgent.agentID }
            .count
        let nextAgentAttempt = existingAttempts + 1

        // Create new agent execution with lineage
        let retryExec = AgentExecution(
            agentID: failedAgent.agentID,
            agentTitle: failedAgent.agentTitle,
            taskName: failedAgent.taskName,
            status: .pending,
            provider: failedAgent.provider,
            effort: failedAgent.effort
        )
        retryExec.retryReason = "agent_retry_via_recovery"
        retryExec.agentAttemptNumber = nextAgentAttempt
        retryExec.supersedesAgentExecutionID = failedAgent.id
        retryExec.resolvedBackendProfileID = failedAgent.resolvedBackendProfileID

        // Collect reused sibling execution IDs
        let siblingIDs = stage.agentExecutions
            .filter { $0.agentID != failedAgent.agentID && $0.status == .completed }
            .map { $0.id }
        if !siblingIDs.isEmpty {
            retryExec.reusedSiblingExecutionIDsJSON = try? JSONEncoder().encode(siblingIDs)
        }

        retryExec.stageExecution = stage
        modelContext.insert(retryExec)

        // Update stage status — do NOT increment attemptNumber (§5.3 Rule 1)
        stage.status = .running
        stage.completedAt = nil
        stage.retryMode = RetryMode.agentRetry.rawValue

        // Update run status
        run.status = .running

        return retryExec
    }

    // MARK: - Retry Failed Aggregate Step (§5.2, Rule 3)

    /// Retry the failed aggregate step within the current stage attempt.
    /// Equivalent to an agent-level retry with aggregate-specific guardrails.
    func retryFailedAggregateStep(
        run: Run,
        stage: StageExecution,
        failedAggregateAgent: AgentExecution
    ) throws -> AgentExecution {
        guard isAggregateStep(failedAggregateAgent) else {
            throw StageRetryError.invalidRetryState(
                "Attempted aggregate retry on non-aggregate agent '\(failedAggregateAgent.agentID)'"
            )
        }

        return try retryFailedAgent(run: run, stage: stage, failedAgent: failedAggregateAgent)
    }

    // MARK: - Retry Failed Stage (§5.2, Rule 2)

    /// Retry the entire failed stage with a new stage attempt.
    /// Increments the stage attempt number and supersedes the whole stage attempt.
    func retryFailedStage(
        run: Run,
        stage: StageExecution
    ) throws -> StageExecution {
        guard stage.status == .failed || stage.status == .blocked else {
            throw StageRetryError.stageNotFailed(stage.stageID)
        }

        // Mark old stage as terminal
        stage.completedAt = stage.completedAt ?? Date()

        // Create new stage execution with incremented attempt
        let newStage = StageExecution(
            stageID: stage.stageID,
            label: stage.label,
            status: .ready,
            iteration: stage.iteration,
            attemptNumber: stage.attemptNumber + 1
        )
        newStage.retryMode = RetryMode.stageRetry.rawValue
        newStage.triggerReason = "stage_retry_via_recovery"
        newStage.supersedesAttemptNumber = stage.attemptNumber
        newStage.run = run

        modelContext.insert(newStage)

        // Update run status
        run.status = .ready

        return newStage
    }

    // MARK: - Recovery Policy (§5.4)

    /// Compute the narrowest valid recovery action.
    func narrowestRecoveryAction(
        for run: Run,
        failedStage: StageExecution?,
        failedAgent: AgentExecution?,
        validationFailure: ValidationFailureRecord?
    ) -> RecoveryActionSnapshot {
        var availableActions: [RecoveryActionDetail] = []
        var recommendedAction: RecoveryActionDetail?

        if let failedAgent, let failedStage {
            let isAggregate = isAggregateStep(failedAgent)
            let retryActionType: RecoveryActionType = isAggregate
                ? .retryFailedAggregateStep
                : .retryFailedAgent
            let retryActionDescription = isAggregate
                ? "Retry only the failed aggregate step '\(failedAgent.agentTitle)' in the same run. Reviewer outputs are reused."
                : "Retry only the failed agent '\(failedAgent.agentTitle)'. Successful sibling agents will be reused."
            let retryAgentAction = RecoveryActionDetail(
                action: retryActionType,
                stageID: failedStage.stageID,
                agentID: failedAgent.agentID,
                explanation: retryActionDescription,
                staysInSameRun: true,
                reusesSiblingOutputs: true,
                reExecutesWholeStage: false
            )
            availableActions.append(retryAgentAction)

            // For output-contract mismatch: recommend operator inspection first
            if let vf = validationFailure, vf.failureClass == .outputContractMismatch {
                recommendedAction = RecoveryActionDetail(
                    action: .operatorInspection,
                    stageID: failedStage.stageID,
                    agentID: failedAgent.agentID,
                    explanation: "Output contract mismatch detected. Inspect raw output before retrying.",
                    staysInSameRun: true,
                    reusesSiblingOutputs: false,
                    reExecutesWholeStage: false
                )
            } else {
                recommendedAction = retryAgentAction
            }
        }

        if let failedStage {
            availableActions.append(RecoveryActionDetail(
                action: .retryFailedStage,
                stageID: failedStage.stageID,
                agentID: nil,
                explanation: "Retry the entire '\(failedStage.label)' stage with all agents.",
                staysInSameRun: true,
                reusesSiblingOutputs: false,
                reExecutesWholeStage: true
            ))
        }

        availableActions.append(RecoveryActionDetail(
            action: .cloneRunFrozenSnapshot,
            stageID: nil,
            agentID: nil,
            explanation: "Create a new run using the frozen configuration snapshot from this run.",
            staysInSameRun: false,
            reusesSiblingOutputs: false,
            reExecutesWholeStage: false
        ))

        availableActions.append(RecoveryActionDetail(
            action: .cloneRunCurrentConfig,
            stageID: nil,
            agentID: nil,
            explanation: "Create a new run using the current (possibly updated) configuration.",
            staysInSameRun: false,
            reusesSiblingOutputs: false,
            reExecutesWholeStage: false
        ))

        return RecoveryActionSnapshot(
            id: UUID(),
            timestamp: Date(),
            runID: run.id,
            recommendedAction: recommendedAction ?? availableActions.first,
            availableActions: availableActions,
            validationFailureID: validationFailure?.id,
            source: .runtimePolicy
        )
    }
}

// MARK: - Retry Mode (§5.3)

enum RetryMode: String, Codable, Sendable {
    case agentRetry = "agent_retry"
    case stageRetry = "stage_retry"
    case freshExecution = "fresh_execution"
}

// MARK: - Recovery Action Snapshot (persisted)

/// Persisted summary of recovery actions shown to the operator.
struct RecoveryActionSnapshot: Codable, Sendable, Identifiable {
    let id: UUID
    let timestamp: Date
    let runID: UUID
    let recommendedAction: RecoveryActionDetail?
    let availableActions: [RecoveryActionDetail]
    let validationFailureID: UUID?
    let source: RecommendationSource
}

/// Detail for a single recovery action.
struct RecoveryActionDetail: Codable, Sendable, Equatable {
    let action: RecoveryActionType
    let stageID: String?
    let agentID: String?
    let explanation: String
    let staysInSameRun: Bool
    let reusesSiblingOutputs: Bool
    let reExecutesWholeStage: Bool
}

enum RecoveryActionType: String, Codable, Sendable, Equatable {
    case retryFailedAgent = "retry_failed_agent"
    case retryFailedAggregateStep = "retry_failed_aggregate_step"
    case retryFailedStage = "retry_failed_stage"
    case cloneRunFrozenSnapshot = "clone_run_frozen_snapshot"
    case cloneRunCurrentConfig = "clone_run_current_config"
    case operatorInspection = "operator_inspection"
}

private func isAggregateStep(_ agent: AgentExecution) -> Bool {
    let normalizedTaskName = agent.taskName
        .trimmingCharacters(in: .whitespacesAndNewlines)
        .lowercased()
    return normalizedTaskName == "aggregate_proposal_reviews"
        || normalizedTaskName == "aggregate_proposal_review"
        || normalizedTaskName.contains("aggregate_proposal_reviews")
}

// MARK: - Errors

enum StageRetryError: Error, LocalizedError {
    case agentNotFailed(String)
    case stageNotFailed(String)
    case invalidRetryState(String)

    var errorDescription: String? {
        switch self {
        case .agentNotFailed(let id): return "Agent '\(id)' is not in failed state"
        case .stageNotFailed(let id): return "Stage '\(id)' is not in failed/blocked state"
        case .invalidRetryState(let msg): return "Invalid retry state: \(msg)"
        }
    }
}
