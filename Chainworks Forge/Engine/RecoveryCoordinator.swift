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
                actions.append(contentsOf: narrowRecoveryActions(for: run, stage: failedStage))
            }
            actions.append(.cloneRunFrozenSnapshot)
            actions.append(.cloneRunCurrentConfig)

        case .blocked:
            if let resumableStage = resumableManualResumeStage(in: run) {
                actions.append(.resumeRun(stageID: resumableStage.stageID))
            }
            if let blockedStage = lastBlockedStage(in: run) {
                actions.append(contentsOf: narrowRecoveryActions(for: run, stage: blockedStage))
            } else if let failedStage = lastFailedStage(in: run) {
                actions.append(contentsOf: narrowRecoveryActions(for: run, stage: failedStage))
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

        return deduplicated(actions)
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
        guard let stage = canonicalStage(in: run, stageID: stageID, matching: [.failed, .blocked]) else {
            throw RecoveryError.stageNotFound(stageID)
        }
        guard let agent = stage.agentExecutions
            .filter({ $0.agentID == agentID })
            .sorted(by: { $0.startedAt < $1.startedAt })
            .last else {
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

        guard let stage = canonicalStage(in: run, stageID: stageID, matching: [.failed, .blocked]) else {
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

    /// Execute an aggregate-step retry action.
    /// Re-runs only the aggregate proposal-review step and reuses contract-valid reviewer outputs.
    func retryAggregateStep(run: Run, stageID: String) throws -> Run {
        guard run.status == .failed || run.status == .blocked else {
            throw RecoveryError.invalidStateForAction(current: run.status.rawValue, action: "retryAggregateStep")
        }
        guard isProposalLoopReadOnly(run) else {
            throw RecoveryError.notProposalLoopRun
        }

        guard let stage = canonicalStage(in: run, stageID: stageID, matching: [.failed, .blocked]) else {
            throw RecoveryError.stageNotFound(stageID)
        }
        let aggregateRecord = aggregateSettlementRecord(for: stage)
        let aggregateAgent = stage.agentExecutions
            .filter(\.isAggregateProposalReviewStep)
            .sorted { lhs, rhs in
                if lhs.blocksForwardProgress != rhs.blocksForwardProgress {
                    return !lhs.blocksForwardProgress && rhs.blocksForwardProgress
                }
                return lhs.startedAt < rhs.startedAt
            }
            .last
        guard let aggregateAgent else {
            throw RecoveryError.aggregateStepNotFound(stageID)
        }

        let retryCoordinator = StageRetryCoordinator(modelContext: modelContext)
        _ = try retryCoordinator.retryFailedAgent(
            run: run,
            stage: stage,
            failedAgent: aggregateAgent,
            allowNonBlockingRetry: aggregateRecord != nil && aggregateAgent.isAggregateProposalReviewStep
        )

        let validationFailure = decodeValidationFailure(from: aggregateAgent)
            ?? aggregateRecord.flatMap(decodeValidationFailure(from:))
        let snapshot = retryCoordinator.narrowestRecoveryAction(
            for: run,
            failedStage: stage,
            failedAgent: aggregateAgent,
            validationFailure: validationFailure,
            aggregateFailure: aggregateRecord != nil
        )
        stage.recoverySnapshotJSON = try? JSONEncoder().encode(snapshot)

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

        guard let stage = canonicalStage(in: run, stageID: stageID, matching: [.waitingApproval]) else {
            throw RecoveryError.stageNotFound(stageID)
        }
        guard stage.status == .waitingApproval else {
            throw RecoveryError.invalidStateForAction(current: stage.status.rawValue, action: "resumeGate")
        }

        // Re-arm the approval gate
        stage.status = .ready
        stage.activeOwnerToken = UUID().uuidString
        stage.settlementKind = nil
        stage.settledAt = nil
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
        let bindingSummary: String?

        // Proposal 013: Try to get failure reason from validation evidence first
        let failedStage = lastFailedStage(in: run) ?? lastBlockedStage(in: run)
        let failedAgent = failedStage.flatMap { lastFailedAgent(in: $0) }
        let aggregateRecord = failedStage.flatMap { aggregateSettlementRecord(for: $0) }
        let validationFailure = failedAgent.flatMap { decodeValidationFailure(from: $0) }
            ?? aggregateRecord.flatMap(decodeValidationFailure(from:))

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

        let sorted = canonicalStages(in: run)
        mostRecentStage = sorted.last?.label ?? "None"

        trustSummary = run.runtimeTrustLevel ?? "unknown"
        bindingSummary = RuntimeBindingTruthSummaryBuilder.summaryText(for: run)

        let actions = availableActions(for: run)

        // Proposal 013 UX-001: For contract mismatch, suggest operator inspection first
        // instead of blind retry. Use narrowestRecoveryAction for evidence-backed suggestion.
        let suggestedAction: RecoveryAction?
        let operatorInspectionRequired = failedStage.map { requiresOperatorInspection(run: run, stage: $0) } ?? false
        if let vf = validationFailure, vf.failureClass == .outputContractMismatch {
            // Operator-mediated: don't suggest retry first for contract mismatch
            // Instead, the evidence panel should be inspected before retrying
            suggestedAction = nil
        } else if operatorInspectionRequired {
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
            bindingSummary: bindingSummary,
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
        let aggregateRecord = aggregateSettlementRecord(for: stage)
        let validationFailure = failedAgent.flatMap { decodeValidationFailure(from: $0) }
            ?? aggregateRecord.flatMap(decodeValidationFailure(from:))
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
        canonicalStages(in: run)
            .filter { $0.status == .failed }
            .sorted { $0.startedAt < $1.startedAt }
            .last
    }

    private func lastBlockedStage(in run: Run) -> StageExecution? {
        canonicalStages(in: run)
            .filter { $0.status == .blocked }
            .sorted { $0.startedAt < $1.startedAt }
            .last
    }

    private func lastFailedAgent(in stage: StageExecution) -> AgentExecution? {
        stage.agentExecutions
            .filter { $0.blocksForwardProgress }
            .sorted { $0.startedAt < $1.startedAt }
            .last
    }

    private func aggregateSettlementRecord(for stage: StageExecution) -> AggregateSettlementRecord? {
        let descriptor = FetchDescriptor<AggregateSettlementRecord>()
        let records = (try? modelContext.fetch(descriptor)) ?? []
        return records
            .filter { $0.stageExecutionID == stage.id }
            .sorted { lhs, rhs in
                let lhsSettledAt = lhs.settledAt ?? .distantPast
                let rhsSettledAt = rhs.settledAt ?? .distantPast
                if lhsSettledAt != rhsSettledAt {
                    return lhsSettledAt < rhsSettledAt
                }
                return lhs.id.uuidString < rhs.id.uuidString
            }
            .last
    }

    private func requiresOperatorInspection(run: Run, stage: StageExecution) -> Bool {
        let aggregateRecord = aggregateSettlementRecord(for: stage)
        let failedAgent = lastFailedAgent(in: stage)
        let validationFailure = failedAgent.flatMap(decodeValidationFailure(from:))
            ?? aggregateRecord.flatMap(decodeValidationFailure(from:))
        if let snapshot = decodeRecoverySnapshot(from: stage) {
            return snapshot.recommendedAction?.action == .operatorInspection
        }

        let retryCoordinator = StageRetryCoordinator(modelContext: modelContext)
        let snapshot = retryCoordinator.narrowestRecoveryAction(
            for: run,
            failedStage: stage,
            failedAgent: failedAgent,
            validationFailure: validationFailure,
            aggregateFailure: aggregateRecord != nil
        )
        return snapshot.recommendedAction?.action == .operatorInspection
    }

    private func hasAggregateExecution(in stage: StageExecution) -> Bool {
        stage.agentExecutions.contains(where: \.isAggregateProposalReviewStep)
    }

    private func narrowRecoveryActions(for run: Run, stage: StageExecution) -> [RecoveryAction] {
        let failedAgent = lastFailedAgent(in: stage)
        let aggregateRecord = aggregateSettlementRecord(for: stage)
        let validationFailure = failedAgent.flatMap(decodeValidationFailure(from:))
            ?? aggregateRecord.flatMap(decodeValidationFailure(from:))

        let retryCoordinator = StageRetryCoordinator(modelContext: modelContext)
        let snapshot = retryCoordinator.narrowestRecoveryAction(
            for: run,
            failedStage: stage,
            failedAgent: failedAgent,
            validationFailure: validationFailure,
            aggregateFailure: aggregateRecord != nil
        )

        let requiresInspection = snapshot.recommendedAction?.action == .operatorInspection
        let allowSameRunRetryDuringInspection = validationFailure?.failureClass == .outputContractMismatch

        return snapshot.availableActions.compactMap { detail in
            if requiresInspection && !allowSameRunRetryDuringInspection {
                switch detail.action {
                case .cloneRunFrozenSnapshot, .cloneRunCurrentConfig:
                    break
                default:
                    return nil
                }
            }
            return recoveryAction(from: detail)
        }
    }

    private func recoveryAction(from detail: RecoveryActionDetail) -> RecoveryAction? {
        switch detail.action {
        case .retryFailedAgent:
            guard let stageID = detail.stageID, let agentID = detail.agentID else { return nil }
            return .retryAgent(stageID: stageID, agentID: agentID)
        case .retryAggregateStep:
            guard let stageID = detail.stageID else { return nil }
            return .retryAggregateStep(stageID: stageID)
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

    private func deduplicated(_ actions: [RecoveryAction]) -> [RecoveryAction] {
        var seen: Set<String> = []
        var result: [RecoveryAction] = []
        for action in actions {
            if seen.insert(action.id).inserted {
                result.append(action)
            }
        }
        return result
    }

    private func lastApprovalGateStage(in run: Run) -> StageExecution? {
        canonicalStages(in: run)
            .filter { $0.status == .waitingApproval }
            .sorted { $0.startedAt < $1.startedAt }
            .last
    }

    private func resumableManualResumeStage(in run: Run) -> StageExecution? {
        guard run.status == .blocked,
              let details = run.driftDetails?.lowercased(),
              (details.contains("explicit operator resume")
                || details.contains("explicit operator recovery")),
              let currentStageID = run.currentStageID else {
            return nil
        }

        return canonicalStages(in: run)
            .filter { $0.stageID == currentStageID && ($0.status == .running || $0.status == .ready) }
            .first(where: { stage in
                stage.agentExecutions.contains(where: { $0.status == .pending || $0.status == .running })
            })
    }

    private func canonicalStages(in run: Run) -> [StageExecution] {
        let grouped = Dictionary(grouping: run.stageExecutions) { stage in
            stage.lineageID ?? "\(stage.stageID)::\(stage.iteration)"
        }

        return grouped.values.compactMap { stages in
            stages.max { lhs, rhs in
                if lhs.attemptNumber != rhs.attemptNumber {
                    return lhs.attemptNumber < rhs.attemptNumber
                }
                if lhs.startedAt != rhs.startedAt {
                    return lhs.startedAt < rhs.startedAt
                }
                return lhs.id.uuidString < rhs.id.uuidString
            }
        }
        .sorted { $0.startedAt < $1.startedAt }
    }

    private func canonicalStage(in run: Run, stageID: String, matching statuses: Set<StageStatus>) -> StageExecution? {
        canonicalStages(in: run)
            .filter { $0.stageID == stageID && statuses.contains($0.status) }
            .sorted { lhs, rhs in
                if lhs.iteration != rhs.iteration { return lhs.iteration < rhs.iteration }
                if lhs.attemptNumber != rhs.attemptNumber { return lhs.attemptNumber < rhs.attemptNumber }
                return lhs.startedAt < rhs.startedAt
            }
            .last
    }

    // MARK: - Proposal 013: Decode Helpers

    private func decodeValidationFailure(from agent: AgentExecution) -> ValidationFailureRecord? {
        guard let data = agent.validationFailureJSON else { return nil }
        return try? JSONDecoder().decode(ValidationFailureRecord.self, from: data)
    }

    private func decodeValidationFailure(from aggregateRecord: AggregateSettlementRecord) -> ValidationFailureRecord? {
        guard let data = aggregateRecord.validationFailureJSON else { return nil }
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
    case resumeRun(stageID: String)
    case retryAgent(stageID: String, agentID: String)
    case retryAggregateStep(stageID: String)
    case retryStage(stageID: String)
    case resumeFromApprovalGate(stageID: String)
    case cloneRunFrozenSnapshot
    case cloneRunCurrentConfig

    var id: String {
        switch self {
        case .resumeRun(let stageID): return "resumeRun_\(stageID)"
        case .retryAgent(let stageID, let agentID): return "retryAgent_\(stageID)_\(agentID)"
        case .retryAggregateStep(let stageID): return "retryAggregate_\(stageID)"
        case .retryStage(let stageID): return "retryStage_\(stageID)"
        case .resumeFromApprovalGate(let stageID): return "resumeGate_\(stageID)"
        case .cloneRunFrozenSnapshot: return "cloneFrozen"
        case .cloneRunCurrentConfig: return "cloneCurrent"
        }
    }

    var label: String {
        switch self {
        case .resumeRun: return "Resume Run"
        case .retryAgent: return "Retry Agent"
        case .retryAggregateStep: return "Retry Aggregate Step"
        case .retryStage: return "Retry Stage"
        case .resumeFromApprovalGate: return "Resume from Approval Gate"
        case .cloneRunFrozenSnapshot: return "Clone Run (Frozen Snapshot)"
        case .cloneRunCurrentConfig: return "Clone Run (Current Config)"
        }
    }

    var systemImage: String {
        switch self {
        case .resumeRun: return "play.circle.fill"
        case .retryAgent: return "arrow.clockwise"
        case .retryAggregateStep: return "arrow.trianglehead.2.clockwise.rotate.90"
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
    let bindingSummary: String?
    let suggestedAction: RecoveryAction?
    let allowedActions: [RecoveryAction]

    // Proposal 013: Evidence-aware recovery context
    let evidenceSummary: String?
    let failureClass: String?

    init(
        reason: String,
        mostRecentStage: String,
        trustSummary: String,
        bindingSummary: String? = nil,
        suggestedAction: RecoveryAction?,
        allowedActions: [RecoveryAction],
        evidenceSummary: String? = nil,
        failureClass: String? = nil
    ) {
        self.reason = reason
        self.mostRecentStage = mostRecentStage
        self.trustSummary = trustSummary
        self.bindingSummary = bindingSummary
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
    case aggregateStepNotFound(String)

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
        case .aggregateStepNotFound(let stageID):
            return "Aggregate step not found in stage '\(stageID)'"
        }
    }
}
