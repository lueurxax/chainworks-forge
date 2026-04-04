import Foundation
import SwiftData

// MARK: - P005-OPS §7: Recovery Coordinator

/// Safe recovery actions for proposal-loop runs.
/// Supports: Retry Agent, Retry Aggregate Step, Retry Stage, Resume from Approval Gate,
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

        if case .failed = run.status,
           let failedStage = lastFailedStage(in: run) ?? lastBlockedStage(in: run),
           let snapshotActions = recoveryActions(from: failedStage),
           !snapshotActions.isEmpty {
            let validated = validateSnapshotActions(snapshotActions, stage: failedStage)
            if !validated.isEmpty { return validated }
        }

        if case .blocked = run.status,
           let blockedStage = lastBlockedStage(in: run) ?? lastFailedStage(in: run),
           let snapshotActions = recoveryActions(from: blockedStage),
           !snapshotActions.isEmpty {
            let validated = validateSnapshotActions(snapshotActions, stage: blockedStage)
            if !validated.isEmpty { return validated }
        }

        var actions: [RecoveryAction] = []

        switch run.status {
        case .failed:
            if let failedStage = lastFailedStage(in: run) {
                if let failedAgent = lastFailedAgent(in: failedStage) {
                    if isAggregateStep(failedAgent) {
                        actions.append(.retryAggregateStep(stageID: failedStage.stageID, agentID: failedAgent.agentID))
                    } else {
                        actions.append(.retryAgent(stageID: failedStage.stageID, agentID: failedAgent.agentID))
                        // Proposal 018: Allow operator reset when an agent fails
                        actions.append(.resetAgentSession(stageID: failedStage.stageID, agentID: failedAgent.agentID))
                    }
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
                    if isAggregateStep(failedAgent) {
                        actions.append(.retryAggregateStep(stageID: failedStage.stageID, agentID: failedAgent.agentID))
                    } else {
                        actions.append(.retryAgent(stageID: failedStage.stageID, agentID: failedAgent.agentID))
                        // Proposal 018: Allow operator reset when an agent fails
                        actions.append(.resetAgentSession(stageID: failedStage.stageID, agentID: failedAgent.agentID))
                    }
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
        guard let stage = retryTargetStage(in: run, stageID: stageID) else {
            throw RecoveryError.stageNotFound(stageID)
        }
        guard let agent = retryTargetAgent(in: stage, agentID: agentID) else {
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

    /// Execute a retry aggregate step action. Returns the updated run and emits a new report version.
    /// Proposal 013: Uses StageRetryCoordinator for proper attempt lineage (§5.2 Rule 3).
    func retryAggregateStep(run: Run, stageID: String, agentID: String) throws -> Run {
        guard run.status == .failed || run.status == .blocked else {
            throw RecoveryError.invalidStateForAction(current: run.status.rawValue, action: "retryAggregateStep")
        }
        guard isProposalLoopReadOnly(run) else {
            throw RecoveryError.notProposalLoopRun
        }

        // Find the stage and aggregate agent
        guard let stage = retryTargetStage(in: run, stageID: stageID) else {
            throw RecoveryError.stageNotFound(stageID)
        }
        guard let agent = retryTargetAgent(in: stage, agentID: agentID) else {
            throw RecoveryError.agentNotFound(agentID)
        }
        guard isAggregateStep(agent) else {
            throw RecoveryError.invalidStateForAction(
                current: "agent '\(agent.agentID)'",
                action: "retryAggregateStep"
            )
        }

        // Proposal 013 §5.2: Use StageRetryCoordinator for proper lineage
        let retryCoordinator = StageRetryCoordinator(modelContext: modelContext)
        _ = try retryCoordinator.retryFailedAggregateStep(
            run: run,
            stage: stage,
            failedAggregateAgent: agent
        )

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

    /// Reset an agent's reusable session lineage (Proposal 018).
    func resetAgentSession(run: Run, stageID: String, agentID: String) async throws -> Run {
        guard isProposalLoopReadOnly(run) else {
            throw RecoveryError.notProposalLoopRun
        }

        let resetCoordinator = SessionResetCoordinator(modelContext: modelContext)
        try await resetCoordinator.resetAgentSession(runID: run.id, agentID: agentID)

        // Emit recovery report noting the reset
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
        let (plan, _) = try compiler.rebuildPlanFromSnapshot(run: original)
        settleSourceRunForCloneIfNeeded(original)

        let (clone, _) = try compiler.createRun(
            for: idea,
            plan: plan,
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
        catalogSourcePath: String,
        startSnapshot: RunStartSnapshot
    ) throws -> Run {
        guard isProposalLoopReadOnly(original) else {
            throw RecoveryError.notProposalLoopRun
        }

        let plan = try compiler.previewCompile(
            workflow: workflow,
            catalog: catalog,
            catalogSourcePath: catalogSourcePath
        )
        settleSourceRunForCloneIfNeeded(original)
        let (clone, _) = try compiler.createRun(
            for: idea,
            plan: plan,
            workflowSourcePath: workflowSourcePath,
            catalogSourcePath: catalogSourcePath,
            startSnapshot: startSnapshot
        )
        clone.runtimeTrustLevel = original.runtimeTrustLevel

        // Emit report on original run noting the clone
        _ = try reportBuilder.emitReport(for: original)

        try modelContext.save()
        return clone
    }

    func cloneRunCurrentConfig(
        original: Run,
        idea: Idea,
        workflow: WorkflowDefinition,
        catalog: AgentCatalog,
        compiler: RunPlanCompiler,
        workflowSourcePath: String,
        catalogSourcePath: String
    ) throws -> Run {
        try cloneRunCurrentConfig(
            original: original,
            idea: idea,
            workflow: workflow,
            catalog: catalog,
            compiler: compiler,
            workflowSourcePath: workflowSourcePath,
            catalogSourcePath: catalogSourcePath,
            startSnapshot: RunStartSnapshot()
        )
    }

    // MARK: - Recovery Context (§7.4) — Proposal 013: Evidence-aware

    /// Build a recovery context for the recovery sheet UI.
    /// Proposal 013: Now includes evidence summary and failure class information.
    func recoveryContext(for run: Run) -> RecoveryContext {
        let reason: String
        let mostRecentStage: String
        let trustSummary: String

        let failedStage = lastFailedStage(in: run) ?? lastBlockedStage(in: run)
        let evidencePacket = failedStage.flatMap { decodeEvidencePacket(from: $0) }
        let validationFailure = failedStage.flatMap { canonicalValidationFailure(for: $0, packet: evidencePacket) }

        if let vf = validationFailure {
            reason = "\(vf.failureClass.rawValue.replacingOccurrences(of: "_", with: " ").capitalized): \(vf.failureSummary)"
        } else if let packet = evidencePacket {
            reason = "\(packet.failureClass.rawValue.replacingOccurrences(of: "_", with: " ").capitalized): \(packet.failureSummary)"
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
        let persistedSnapshot = failedStage.flatMap { canonicalRecoverySnapshot(for: $0) }

        // Proposal 013 UX-001: For contract mismatch, suggest operator inspection first
        // instead of blind retry. If a persisted recovery snapshot exists, it owns
        // the suggested action relation and must be read before recomputing locally.
        let suggestedAction: RecoveryAction?
        if let snapshotAction = persistedSnapshot?.recommendedAction.flatMap(recoveryAction(from:)) {
            suggestedAction = snapshotAction
        } else if let vf = validationFailure, vf.failureClass == .outputContractMismatch {
            // Operator-mediated: don't suggest retry first for contract mismatch
            // Instead, the evidence panel should be inspected before retrying
            suggestedAction = nil
        } else {
            suggestedAction = actions.first
        }

        // Proposal 013: Build evidence summary
        let evidenceSummary: String?
        if let packet = evidencePacket {
            var parts: [String] = []
            if packet.rawOutputsExist { parts.append("raw output present") }
            if packet.receiptExists { parts.append("receipt present") }
            if packet.transcriptExists { parts.append("transcript present") }
            evidenceSummary = parts.isEmpty ? nil : parts.joined(separator: ", ")
        } else if let vf = validationFailure {
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
            failureClass: validationFailure?.failureClass.rawValue ?? evidencePacket?.failureClass.rawValue
        )
    }

    // MARK: - Proposal 013: Evidence Packet Building

    /// Build a failed-stage evidence packet for the evidence panel.
    func buildEvidencePacket(for run: Run) -> FailedStageEvidencePacket? {
        guard let stage = lastFailedStage(in: run) ?? lastBlockedStage(in: run) else { return nil }
        if let packet = decodeEvidencePacket(from: stage) {
            return packet
        }

        let failedAgent = lastFailedAgent(in: stage)
        let validationFailure = canonicalValidationFailure(for: stage, packet: nil)
        let envelopes = stage.agentExecutions
            .compactMap { decodeOutputEnvelopes(from: $0) }
            .flatMap(\.self)
        let recoverySnapshot = canonicalRecoverySnapshot(for: stage)

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

    private func retryTargetStage(in run: Run, stageID: String) -> StageExecution? {
        let matching = run.stageExecutions
            .filter { $0.stageID == stageID }
            .sorted { $0.startedAt < $1.startedAt }

        return matching.last(where: { $0.status == .failed || $0.status == .blocked })
            ?? matching.last
    }

    private func retryTargetAgent(in stage: StageExecution, agentID: String) -> AgentExecution? {
        let matching = stage.agentExecutions
            .filter { $0.agentID == agentID }
            .sorted { $0.startedAt < $1.startedAt }

        // Only return a failed agent execution for retry.
        // Falling back to a non-failed execution would cause StageRetryCoordinator
        // to reject it with agentNotFailed, making the button appear non-responsive.
        return matching.last(where: { $0.status == .failed })
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

    private func decodeEvidencePacket(from stage: StageExecution) -> FailedStageEvidencePacket? {
        guard let data = stage.evidencePacketJSON else { return nil }
        return try? JSONDecoder().decode(FailedStageEvidencePacket.self, from: data)
    }

    private func canonicalRecoverySnapshot(for stage: StageExecution) -> RecoveryActionSnapshot? {
        if let snapshot = decodeRecoverySnapshot(from: stage) {
            return snapshot
        }
        return decodeEvidencePacket(from: stage)?.recoverySnapshot
    }

    private func canonicalValidationFailure(
        for stage: StageExecution,
        packet: FailedStageEvidencePacket?
    ) -> ValidationFailureRecord? {
        if let data = stage.validationFailureJSON {
            return try? JSONDecoder().decode(ValidationFailureRecord.self, from: data)
        }
        if let packetValidation = packet?.validationFailure {
            return packetValidation
        }
        return lastFailedAgent(in: stage).flatMap { decodeValidationFailure(from: $0) }
    }

    /// Validate snapshot-based recovery actions against current state.
    /// Filters out stale agent-retry actions where the agent is no longer failed.
    private func validateSnapshotActions(_ actions: [RecoveryAction], stage: StageExecution) -> [RecoveryAction] {
        actions.filter { action in
            switch action {
            case .retryAgent(_, let agentID), .retryAggregateStep(_, let agentID), .resetAgentSession(_, let agentID):
                // Only keep agent-level actions if there's actually a failed execution for that agent
                return stage.agentExecutions.contains { $0.agentID == agentID && $0.status == .failed }
            case .retryStage, .resumeFromApprovalGate, .cloneRunFrozenSnapshot, .cloneRunCurrentConfig:
                return true
            }
        }
    }

    private func recoveryActions(from stage: StageExecution) -> [RecoveryAction]? {
        let snapshot = canonicalRecoverySnapshot(for: stage)
        let actions = snapshot?.availableActions.compactMap(recoveryAction(from:))
        return actions
    }

    private func recoveryAction(from detail: RecoveryActionDetail) -> RecoveryAction? {
        switch detail.action {
        case .retryFailedAgent:
            guard let stageID = detail.stageID, let agentID = detail.agentID else { return nil }
            return .retryAgent(stageID: stageID, agentID: agentID)
        case .retryFailedAggregateStep:
            guard let stageID = detail.stageID, let agentID = detail.agentID else { return nil }
            return .retryAggregateStep(stageID: stageID, agentID: agentID)
        case .retryFailedStage:
            guard let stageID = detail.stageID else { return nil }
            return .retryStage(stageID: stageID)
        case .cloneRunFrozenSnapshot:
            return .cloneRunFrozenSnapshot
        case .cloneRunCurrentConfig:
            return .cloneRunCurrentConfig
        case .operatorInspection:
            return nil
        }
    }

    private func isAggregateStep(_ agent: AgentExecution) -> Bool {
        let normalizedTaskName = agent.taskName
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()
        return normalizedTaskName == "aggregate_proposal_reviews"
            || normalizedTaskName == "aggregate_proposal_review"
            || normalizedTaskName.contains("aggregate_proposal_reviews")
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
    case retryAggregateStep(stageID: String, agentID: String)
    case retryStage(stageID: String)
    case resumeFromApprovalGate(stageID: String)
    case resetAgentSession(stageID: String, agentID: String)
    case cloneRunFrozenSnapshot
    case cloneRunCurrentConfig

    var id: String {
        switch self {
        case .retryAgent(let stageID, let agentID): return "retryAgent_\(stageID)_\(agentID)"
        case .retryAggregateStep(let stageID, let agentID): return "retryAggregateStep_\(stageID)_\(agentID)"
        case .retryStage(let stageID): return "retryStage_\(stageID)"
        case .resumeFromApprovalGate(let stageID): return "resumeGate_\(stageID)"
        case .resetAgentSession(let stageID, let agentID): return "resetSession_\(stageID)_\(agentID)"
        case .cloneRunFrozenSnapshot: return "cloneFrozen"
        case .cloneRunCurrentConfig: return "cloneCurrent"
        }
    }

    var label: String {
        switch self {
        case .retryAgent: return "Retry Agent"
        case .retryAggregateStep: return "Retry Aggregate Step"
        case .retryStage: return "Retry Stage"
        case .resumeFromApprovalGate: return "Resume from Approval Gate"
        case .resetAgentSession: return "Reset Agent Session"
        case .cloneRunFrozenSnapshot: return "Clone Run (Frozen Snapshot)"
        case .cloneRunCurrentConfig: return "Clone Run (Current Config)"
        }
    }

    var systemImage: String {
        switch self {
        case .retryAgent: return "arrow.clockwise"
        case .retryAggregateStep: return "arrow.counterclockwise"
        case .retryStage: return "arrow.counterclockwise"
        case .resumeFromApprovalGate: return "play.fill"
        case .resetAgentSession: return "trash.circle"
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
