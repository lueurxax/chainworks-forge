import Foundation
import SwiftData

// MARK: - P005-OPS §7: Recovery Coordinator

/// Safe recovery actions for proposal-loop runs.
/// Supports: Retry Agent, Retry Stage, Resume from Approval Gate,
/// Clone Run (Frozen Snapshot), Clone Run (Current Config).
/// Does NOT own repo-backed or release-side recovery (Proposal 007).
@MainActor
final class RecoveryCoordinator {

    private let modelContext: ModelContext
    private let reportBuilder: RunReportBuilder

    init(modelContext: ModelContext) {
        self.modelContext = modelContext
        self.reportBuilder = RunReportBuilder(modelContext: modelContext)
    }

    // MARK: - Recovery Actions (§7.1)

    /// Available recovery actions for the given run.
    func availableActions(for run: Run) -> [RecoveryAction] {
        guard isProposalLoopReadOnly(run) else { return [] }

        var actions: [RecoveryAction] = []

        switch run.status {
        case .failed:
            if let failedStage = lastFailedStage(in: run) {
                if let failedAgent = lastFailedAgent(in: failedStage) {
                    actions.append(.retryAgent(stageID: failedStage.stageID, agentID: failedAgent.agentID))
                }
                actions.append(.retryStage(stageID: failedStage.stageID))
            }
            actions.append(.cloneRunFrozenSnapshot)
            actions.append(.cloneRunCurrentConfig)

        case .blocked:
            if let blockedStage = lastBlockedStage(in: run) {
                actions.append(.retryStage(stageID: blockedStage.stageID))
            } else if let failedStage = lastFailedStage(in: run) {
                if let failedAgent = lastFailedAgent(in: failedStage) {
                    actions.append(.retryAgent(stageID: failedStage.stageID, agentID: failedAgent.agentID))
                }
                actions.append(.retryStage(stageID: failedStage.stageID))
            }
            actions.append(.cloneRunFrozenSnapshot)
            actions.append(.cloneRunCurrentConfig)

        case .waitingApproval:
            if let gateStage = lastApprovalGateStage(in: run) {
                actions.append(.resumeFromApprovalGate(stageID: gateStage.stageID))
            }

        default:
            break
        }

        return actions
    }

    // MARK: - Execute Recovery

    /// Execute a retry agent action. Returns the updated run and emits a new report version.
    /// Proposal 013: Uses StageRetryCoordinator for proper attempt lineage (§5.2 Rule 1).
    func retryAgent(run: Run, stageID: String, agentID: String) throws -> Run {
        guard run.status == .failed || run.status == .blocked else {
            throw RecoveryError.invalidStateForAction(current: run.status.rawValue, action: "retryAgent")
        }
        guard isProposalLoopReadOnly(run) else {
            throw RecoveryError.notProposalLoopRun
        }

        // Find the stage and agent
        guard let stage = run.stageExecutions.first(where: { $0.stageID == stageID }) else {
            throw RecoveryError.stageNotFound(stageID)
        }
        guard let agent = stage.agentExecutions.first(where: { $0.agentID == agentID }) else {
            throw RecoveryError.agentNotFound(agentID)
        }

        // Proposal 013 §5.2: Use StageRetryCoordinator for proper lineage
        let retryCoordinator = StageRetryCoordinator(modelContext: modelContext)
        _ = try retryCoordinator.retryFailedAgent(run: run, stage: stage, failedAgent: agent)

        // Build and persist recovery snapshot
        let validationFailure = decodeValidationFailure(from: agent)
        let snapshot = retryCoordinator.narrowestRecoveryAction(
            for: run,
            failedStage: stage,
            failedAgent: agent,
            validationFailure: validationFailure
        )
        stage.recoverySnapshotJSON = try? JSONEncoder().encode(snapshot)

        // Emit recovery report
        _ = try reportBuilder.emitReport(for: run)

        try modelContext.save()
        return run
    }

    /// Execute a retry stage action.
    /// Proposal 013: Uses StageRetryCoordinator for proper attempt lineage (§5.2 Rule 2).
    func retryStage(run: Run, stageID: String) throws -> Run {
        guard run.status == .failed || run.status == .blocked else {
            throw RecoveryError.invalidStateForAction(current: run.status.rawValue, action: "retryStage")
        }
        guard isProposalLoopReadOnly(run) else {
            throw RecoveryError.notProposalLoopRun
        }

        guard let stage = run.stageExecutions.first(where: { $0.stageID == stageID }) else {
            throw RecoveryError.stageNotFound(stageID)
        }

        // Proposal 013 §5.2: Use StageRetryCoordinator for proper lineage
        let retryCoordinator = StageRetryCoordinator(modelContext: modelContext)
        _ = try retryCoordinator.retryFailedStage(run: run, stage: stage)

        // Emit recovery report
        _ = try reportBuilder.emitReport(for: run)

        try modelContext.save()
        return run
    }

    /// Resume from an approval gate.
    func resumeFromApprovalGate(run: Run, stageID: String) throws -> Run {
        guard run.status == .waitingApproval else {
            throw RecoveryError.invalidStateForAction(current: run.status.rawValue, action: "resumeGate")
        }
        guard isProposalLoopReadOnly(run) else {
            throw RecoveryError.notProposalLoopRun
        }

        guard let stage = run.stageExecutions.first(where: { $0.stageID == stageID }) else {
            throw RecoveryError.stageNotFound(stageID)
        }
        guard stage.status == .waitingApproval else {
            throw RecoveryError.invalidStateForAction(current: stage.status.rawValue, action: "resumeGate")
        }

        // Re-arm the approval gate
        stage.status = .ready
        run.status = .ready

        // Emit recovery report
        _ = try reportBuilder.emitReport(for: run)

        try modelContext.save()
        return run
    }

    /// Clone a run using the frozen snapshot.
    func cloneRunFrozenSnapshot(
        original: Run,
        idea: Idea,
        compiler: RunPlanCompiler
    ) throws -> Run {
        guard isProposalLoopReadOnly(original) else {
            throw RecoveryError.notProposalLoopRun
        }

        // Rebuild plan from frozen snapshot
        let (plan, workspace) = try compiler.rebuildPlanFromSnapshot(run: original)
        settleSourceRunForCloneIfNeeded(original)

        let clone = try RunRepository(context: modelContext).createRunFromPlan(
            for: idea,
            plan: plan,
            workspace: workspace,
            workflowSourcePath: original.workflowSourcePath,
            catalogSourcePath: original.catalogSourcePath,
            startSnapshot: RunStartSnapshot.from(run: original)
        )
        clone.runtimeTrustLevel = original.runtimeTrustLevel

        // Emit report on original run noting the clone
        _ = try reportBuilder.emitReport(for: original)

        try modelContext.save()
        return clone
    }

    /// Clone a run using the current (possibly updated) config.
    func cloneRunCurrentConfig(
        original: Run,
        idea: Idea,
        workflow: WorkflowDefinition,
        catalog: AgentCatalog,
        compiler: RunPlanCompiler,
        workflowSourcePath: String,
        catalogSourcePath: String
    ) throws -> Run {
        guard isProposalLoopReadOnly(original) else {
            throw RecoveryError.notProposalLoopRun
        }

        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)
        settleSourceRunForCloneIfNeeded(original)
        let (clone, _) = try compiler.createRun(
            for: idea,
            plan: plan,
            workflowSourcePath: workflowSourcePath,
            catalogSourcePath: catalogSourcePath
        )
        clone.runtimeTrustLevel = original.runtimeTrustLevel

        // Emit report on original run noting the clone
        _ = try reportBuilder.emitReport(for: original)

        try modelContext.save()
        return clone
    }

    // MARK: - Recovery Context (§7.4) — Proposal 013: Evidence-aware

    /// Build a recovery context for the recovery sheet UI.
    /// Proposal 013: Now includes evidence summary and failure class information.
    func recoveryContext(for run: Run) -> RecoveryContext {
        let reason: String
        let mostRecentStage: String
        let trustSummary: String

        // Proposal 013: Try to get failure reason from validation evidence first
        let failedStage = lastFailedStage(in: run) ?? lastBlockedStage(in: run)
        let failedAgent = failedStage.flatMap { lastFailedAgent(in: $0) }
        let validationFailure = failedAgent.flatMap { decodeValidationFailure(from: $0) }

        if let vf = validationFailure {
            reason = "\(vf.failureClass.rawValue.replacingOccurrences(of: "_", with: " ").capitalized): \(vf.failureSummary)"
        } else if let details = run.driftDetails {
            reason = details
        } else if run.status == .failed {
            reason = "Run failed during execution"
        } else if run.status == .blocked {
            reason = "Run blocked and cannot proceed"
        } else {
            reason = "Unknown"
        }

        let sorted = run.stageExecutions.sorted { $0.startedAt < $1.startedAt }
        mostRecentStage = sorted.last?.label ?? "None"

        trustSummary = run.runtimeTrustLevel ?? "unknown"

        let actions = availableActions(for: run)

        // Proposal 013 UX-001: For contract mismatch, suggest operator inspection first
        // instead of blind retry. Use narrowestRecoveryAction for evidence-backed suggestion.
        let suggestedAction: RecoveryAction?
        if let vf = validationFailure, vf.failureClass == .outputContractMismatch {
            // Operator-mediated: don't suggest retry first for contract mismatch
            // Instead, the evidence panel should be inspected before retrying
            suggestedAction = nil
        } else {
            suggestedAction = actions.first
        }

        // Proposal 013: Build evidence summary
        let evidenceSummary: String?
        if let vf = validationFailure {
            var parts: [String] = []
            if vf.rawOutputExists { parts.append("raw output present") }
            if vf.receiptExists { parts.append("receipt present") }
            if vf.transcriptExists { parts.append("transcript present") }
            evidenceSummary = parts.isEmpty ? nil : parts.joined(separator: ", ")
        } else {
            evidenceSummary = nil
        }

        return RecoveryContext(
            reason: reason,
            mostRecentStage: mostRecentStage,
            trustSummary: trustSummary,
            suggestedAction: suggestedAction,
            allowedActions: actions,
            evidenceSummary: evidenceSummary,
            failureClass: validationFailure?.failureClass.rawValue
        )
    }

    // MARK: - Proposal 013: Evidence Packet Building

    /// Build a failed-stage evidence packet for the evidence panel.
    func buildEvidencePacket(for run: Run) -> FailedStageEvidencePacket? {
        guard let stage = lastFailedStage(in: run) ?? lastBlockedStage(in: run) else { return nil }
        let failedAgent = lastFailedAgent(in: stage)
        let validationFailure = failedAgent.flatMap { decodeValidationFailure(from: $0) }
        let envelopes = failedAgent.flatMap { decodeOutputEnvelopes(from: $0) } ?? []
        let recoverySnapshot = decodeRecoverySnapshot(from: stage)

        return FailedStageEvidenceBuilder.buildEvidencePacket(
            stageExecution: stage,
            failedAgent: failedAgent,
            validationFailure: validationFailure,
            outputEnvelopes: envelopes,
            recoverySnapshot: recoverySnapshot
        )
    }

    // MARK: - Helpers

    private func isProposalLoopReadOnly(_ run: Run) -> Bool {
        // Current baseline: proposal-loop runs only (§7.2)
        // No writable worktree stages, no git stages, no publish stages.
        // Future run types with repo-backed stages are deferred to Proposal 007.
        return true
    }

    private func lastFailedStage(in run: Run) -> StageExecution? {
        run.stageExecutions
            .filter { $0.status == .failed }
            .sorted { $0.startedAt < $1.startedAt }
            .last
    }

    private func lastBlockedStage(in run: Run) -> StageExecution? {
        run.stageExecutions
            .filter { $0.status == .blocked }
            .sorted { $0.startedAt < $1.startedAt }
            .last
    }

    private func lastFailedAgent(in stage: StageExecution) -> AgentExecution? {
        stage.agentExecutions
            .filter { $0.status == .failed }
            .sorted { $0.startedAt < $1.startedAt }
            .last
    }

    private func lastApprovalGateStage(in run: Run) -> StageExecution? {
        run.stageExecutions
            .filter { $0.status == .waitingApproval }
            .sorted { $0.startedAt < $1.startedAt }
            .last
    }

    // MARK: - Proposal 013: Decode Helpers

    private func decodeValidationFailure(from agent: AgentExecution) -> ValidationFailureRecord? {
        guard let data = agent.validationFailureJSON else { return nil }
        return try? JSONDecoder().decode(ValidationFailureRecord.self, from: data)
    }

    private func decodeOutputEnvelopes(from agent: AgentExecution) -> [StructuredOutputEnvelope]? {
        guard let data = agent.outputEnvelopesJSON else { return nil }
        return try? JSONDecoder().decode([StructuredOutputEnvelope].self, from: data)
    }

    private func decodeRecoverySnapshot(from stage: StageExecution) -> RecoveryActionSnapshot? {
        guard let data = stage.recoverySnapshotJSON else { return nil }
        return try? JSONDecoder().decode(RecoveryActionSnapshot.self, from: data)
    }

    /// A recovery clone replaces the blocked source run as the active run for the idea.
    /// The source run stays in durable history, but must no longer occupy the single-active-run slot.
    private func settleSourceRunForCloneIfNeeded(_ run: Run) {
        guard run.status == .blocked else { return }
        run.status = .cancelled
        run.completedAt = run.completedAt ?? Date()
        if let details = run.driftDetails, !details.isEmpty {
            if !details.localizedCaseInsensitiveContains("superseded by recovery clone") {
                run.driftDetails = "\(details) Superseded by recovery clone."
            }
        } else {
            run.driftDetails = "Superseded by recovery clone."
        }
    }
}

// MARK: - Recovery Types

enum RecoveryAction: Identifiable, Equatable {
    case retryAgent(stageID: String, agentID: String)
    case retryStage(stageID: String)
    case resumeFromApprovalGate(stageID: String)
    case cloneRunFrozenSnapshot
    case cloneRunCurrentConfig

    var id: String {
        switch self {
        case .retryAgent(let stageID, let agentID): return "retryAgent_\(stageID)_\(agentID)"
        case .retryStage(let stageID): return "retryStage_\(stageID)"
        case .resumeFromApprovalGate(let stageID): return "resumeGate_\(stageID)"
        case .cloneRunFrozenSnapshot: return "cloneFrozen"
        case .cloneRunCurrentConfig: return "cloneCurrent"
        }
    }

    var label: String {
        switch self {
        case .retryAgent: return "Retry Agent"
        case .retryStage: return "Retry Stage"
        case .resumeFromApprovalGate: return "Resume from Approval Gate"
        case .cloneRunFrozenSnapshot: return "Clone Run (Frozen Snapshot)"
        case .cloneRunCurrentConfig: return "Clone Run (Current Config)"
        }
    }

    var systemImage: String {
        switch self {
        case .retryAgent: return "arrow.clockwise"
        case .retryStage: return "arrow.counterclockwise"
        case .resumeFromApprovalGate: return "play.fill"
        case .cloneRunFrozenSnapshot: return "doc.on.doc"
        case .cloneRunCurrentConfig: return "doc.on.doc.fill"
        }
    }
}

struct RecoveryContext {
    let reason: String
    let mostRecentStage: String
    let trustSummary: String
    let suggestedAction: RecoveryAction?
    let allowedActions: [RecoveryAction]

    // Proposal 013: Evidence-aware recovery context
    let evidenceSummary: String?
    let failureClass: String?

    init(
        reason: String,
        mostRecentStage: String,
        trustSummary: String,
        suggestedAction: RecoveryAction?,
        allowedActions: [RecoveryAction],
        evidenceSummary: String? = nil,
        failureClass: String? = nil
    ) {
        self.reason = reason
        self.mostRecentStage = mostRecentStage
        self.trustSummary = trustSummary
        self.suggestedAction = suggestedAction
        self.allowedActions = allowedActions
        self.evidenceSummary = evidenceSummary
        self.failureClass = failureClass
    }
}

enum RecoveryError: Error, LocalizedError {
    case invalidStateForAction(current: String, action: String)
    case notProposalLoopRun
    case stageNotFound(String)
    case agentNotFound(String)

    var errorDescription: String? {
        switch self {
        case .invalidStateForAction(let current, let action):
            return "Cannot \(action) from state '\(current)'"
        case .notProposalLoopRun:
            return "Recovery is only supported for proposal-loop read-only runs in P005-OPS"
        case .stageNotFound(let id):
            return "Stage '\(id)' not found"
        case .agentNotFound(let id):
            return "Agent '\(id)' not found"
        }
    }
}
