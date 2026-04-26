import Foundation
import SwiftData

@Model final class Run {
    @Attribute(.unique) var id: UUID
    var startedAt: Date
    var completedAt: Date?
    var status: RunStatus
    var loopCounters: [String: Int]
    var totalCostCents: Int64?

    // RunPlanSnapshot (immutable after creation — private(set) enforces contract)
    private(set) var workflowID: String
    private(set) var workflowTitle: String
    private(set) var workflowSnapshotHash: String
    private(set) var catalogSnapshotHash: String
    private(set) var workflowSourcePath: String
    private(set) var catalogSourcePath: String
    private(set) var workflowSnapshotJSON: Data
    private(set) var catalogSnapshotJSON: Data

    // Workspace paths (Proposal 002 — ARCH-025, ARCH-026)
    private(set) var workspaceRoot: String
    private(set) var artifactRoot: String
    private(set) var planCompilerVersion: Int

    // Drift detection
    var driftDetectedAt: Date?
    var driftDetails: String?
    var driftDecision: DriftDecision?

    // Steward cohorting metadata (Proposal 003 — optional, lightweight migration)
    var workflowFamily: String?
    var projectKey: String?
    var riskClass: RiskClass?
    var stack: String?
    var experimentCohortID: UUID?

    // P005-OPS §6.5 / Proposal 033 §6: runtime trust additions
    var latestSummaryArtifactID: UUID?
    var latestImmutableReportArtifactID: UUID?
    var latestReportVersion: Int = 0
    var runtimeTrustLevel: String?    // "fixture_verified" | legacy "server_*" | ACP-era "runtime_*"
    var providerBindingSnapshotJSON: Data?
    var startOptionsJSON: Data?

    // Proposal 011 (REQ-009): Frozen binding provenance per agent, keyed by agent ID.
    var bindingProvenanceJSON: Data?
    // Proposal 015: Frozen skill truth per run, keyed by skill ID.
    var resolvedSkillsJSON: Data?
    var skillContentHashesJSON: Data?
    var skillInjectedContentHashesJSON: Data?
    var resolvedMCPPoliciesJSON: Data?

    // Proposal 011 (REQ-007): Frozen idea workspace root path at run creation time.
    // Set once during startRun, not mutated afterward.
    var frozenWorkspaceRootPath: String?

    // Proposal 011: Cancellation settlement (REQ-002 — truthful run control)
    var cancellationRequestedAt: Date?       // when operator pressed stop
    var cancellationSettledAt: Date?         // when coordinator confirmed propagation
    var cancellationSettlementLog: Data?     // JSON array of per-agent settlement entries

    // Proposal 007: Delivery configuration (frozen pre-run contract — ARCH-067 through ARCH-075)
    var deliveryConfigurationJSON: Data?
    var deliveryPreflightJSON: Data?
    var worktreeRoot: String?
    var repoIdentifier: String?
    var repoRoot: String?
    var baseBranch: String?
    var baseRevision: String?
    var targetBranch: String?
    var releaseTargetID: String?
    var releaseMode: String?

    // Proposal 018: Derived session event audit trail
    var sessionEventAuditDerivedJSON: Data?
    // Proposal 018 §8: Session reuse KPI export (REQ-013)
    var sessionKPIExportJSON: Data?
    // Proposal 018: Structured lineage report for run reports (PROD-001)
    var sessionLineageReportJSON: Data?

    // Proposal 019: Context strategy experiment metadata (immutable assignment + snapshot)
    var contextStrategyProfileID: String = ""
    var strategyAssignmentMode: String = "default"
    var strategyRecommendationState: String = "not_evaluated"
    var contextStrategySnapshotJSON: Data?
    var promotedHandoffArtifactsJSON: Data?

    // Proposal 032: Atomic transition settlement and durable resume cursor.
    // Serialized TransitionCursor — the single canonical continuation truth for resume,
    // recovery, and report surfaces. Nil for pre-P032 runs (fallback to heuristic path).
    var transitionCursorJSON: Data?
    // Proposal 017 Phase A bridge: temporary JSON storage for workflow conflict
    // truth until Swift moves to first-class conflict persistence.
    var workflowConflictRecordsJSONV1: Data?
    // Proposal 054: Canonical implementation_self_assessment_summary projection.
    var implementationSelfAssessmentSummaryJSON: Data?
    // Proposal 017 Phase A: engine-owned implementation-entry handoff readback.
    var implementationHandoffStatusJSON: Data?

    @Relationship(inverse: \Idea.runs)
    var idea: Idea?

    @Relationship(deleteRule: .cascade)
    var stageExecutions: [StageExecution] = []

    @Relationship(deleteRule: .cascade)
    var approvals: [Approval] = []

    // Derived current stage (ARCH-PA-002), cursor-first (Proposal 032).
    @MainActor var currentStageID: String? {
        // Proposal 032: Prefer the durable cursor for current-stage derivation.
        if let cursor = transitionCursor {
            switch cursor.settlementPhase {
            case .transitionSettled, .transitionStarted:
                return cursor.nextScheduledStateID
            case .terminal, .awaitingConflictResolution:
                return cursor.lastCompletedStateID
            case .awaitingFirstState:
                break
            }

            return nil
        }
        // Fallback for pre-P032 runs or initial state.
        let sorted = RunStageSnapshotLoader.load(for: self).sorted { $0.startedAt < $1.startedAt }
        return sorted.last(where: {
            $0.status == .running
                || $0.status == .waitingApproval
                || $0.status == .blocked
                || $0.status == .failed
                || $0.status == .ready
        })?.stageID
            ?? sorted.last(where: { $0.status == .completed })?.stageID
    }

    /// Proposal 032: Cursor-derived stage label for shell surfaces.
    /// Returns a label that distinguishes interrupted-transition states
    /// (e.g. "Scheduled: review" vs. just "review").
    @MainActor var cursorDerivedStageLabel: String {
        guard let cursor = transitionCursor else {
            return currentStageID ?? "None"
        }
        switch cursor.settlementPhase {
        case .transitionSettled:
            if let next = cursor.nextScheduledStateID {
                return "Scheduled: \(next)"
            }
            return cursor.lastCompletedStateID ?? "None"
        case .transitionStarted:
            return cursor.nextScheduledStateID ?? "None"
        case .terminal, .awaitingConflictResolution:
            return cursor.lastCompletedStateID ?? "None"
        case .awaitingFirstState:
            return currentStageID ?? "Not started"
        }
    }

    init(
        id: UUID = UUID(),
        startedAt: Date = Date(),
        status: RunStatus = .pending,
        loopCounters: [String: Int] = [:],
        workflowID: String,
        workflowTitle: String,
        workflowSnapshotHash: String,
        catalogSnapshotHash: String,
        workflowSourcePath: String,
        catalogSourcePath: String,
        workflowSnapshotJSON: Data,
        catalogSnapshotJSON: Data,
        workspaceRoot: String = "",
        artifactRoot: String = "",
        planCompilerVersion: Int = 0,
        contextStrategyProfileID: String = "",
        strategyAssignmentMode: String = "default",
        strategyRecommendationState: String = "not_evaluated",
        contextStrategySnapshotJSON: Data? = nil,
        promotedHandoffArtifactsJSON: Data? = nil
    ) {
        self.id = id
        self.startedAt = startedAt
        self.status = status
        self.loopCounters = loopCounters
        self.workflowID = workflowID
        self.workflowTitle = workflowTitle
        self.workflowSnapshotHash = workflowSnapshotHash
        self.catalogSnapshotHash = catalogSnapshotHash
        self.workflowSourcePath = workflowSourcePath
        self.catalogSourcePath = catalogSourcePath
        self.workflowSnapshotJSON = workflowSnapshotJSON
        self.catalogSnapshotJSON = catalogSnapshotJSON
        self.workspaceRoot = workspaceRoot
        self.artifactRoot = artifactRoot
        self.planCompilerVersion = planCompilerVersion
        self.contextStrategyProfileID = contextStrategyProfileID
        self.strategyAssignmentMode = strategyAssignmentMode
        self.strategyRecommendationState = strategyRecommendationState
        self.contextStrategySnapshotJSON = contextStrategySnapshotJSON
        self.promotedHandoffArtifactsJSON = promotedHandoffArtifactsJSON
    }
}

struct RuntimeTrustPresentation: Equatable, Sendable {
    let rawValue: String?
    let semanticValue: String?

    init(trustLevel: String?) {
        self.rawValue = trustLevel
        switch trustLevel {
        case "fixture_verified":
            self.semanticValue = "fixture_verified"
        case "runtime_verified", "server_verified":
            self.semanticValue = "runtime_verified"
        case "runtime_unverified", "server_unverified":
            self.semanticValue = "runtime_unverified"
        default:
            self.semanticValue = nil
        }
    }

    var badgeLabel: String {
        switch rawValue {
        case "fixture_verified": return "Fixture / verified"
        case "server_unverified": return "Legacy (unverified)"
        case "server_verified": return "Legacy (verified)"
        case "runtime_verified": return "Verified"
        case "runtime_unverified": return "Unverified"
        default: return "Unknown"
        }
    }

    var badgeIcon: String {
        switch semanticValue {
        case "fixture_verified", "runtime_verified":
            return "checkmark.shield.fill"
        case "runtime_unverified":
            return "shield.lefthalf.filled"
        default:
            return "questionmark.circle"
        }
    }

    var badgeColorName: String {
        switch semanticValue {
        case "fixture_verified", "runtime_verified":
            return "success"
        case "runtime_unverified":
            return "warning"
        default:
            return "neutral"
        }
    }
}

extension Run {
    private var persistedOperatorStatusDominatesTransientStageTruth: Bool {
        switch status {
        case .waitingApproval, .blocked, .completed, .failed, .cancelled, .cancelling:
            return true
        case .pending, .ready, .running:
            return false
        }
    }

    @MainActor var runtimeTrustPresentation: RuntimeTrustPresentation {
        RuntimeTrustPresentation(trustLevel: runtimeTrustLevel)
    }

    @MainActor var normalizedRuntimeTrustLevel: String? {
        runtimeTrustPresentation.semanticValue
    }

    @MainActor var runtimeTrustDisplayLabel: String {
        runtimeTrustPresentation.badgeLabel
    }
}

enum RunStatus: String, Codable {
    case pending
    case ready
    case running
    case waitingApproval
    case blocked
    case completed
    case failed
    case cancelled
    case cancelling
}

// MARK: - Run Presentation Status (Proposal 011 — REQ-002)

@MainActor
extension Run {
    /// Lightweight list/sidebar truth that never loads stage snapshots.
    /// Use this on hot SwiftUI list paths where stored run status is enough.
    var listPresentationStatus: RunStatus {
        if cancellationRequestedAt != nil && cancellationSettledAt == nil {
            return .cancelling
        }
        return status
    }

    /// Whether operator surfaces should expose a stop/cancel action for this run.
    var canBeCancelledByOperator: Bool {
        switch presentationStatus {
        case .pending, .ready, .running, .waitingApproval, .blocked, .cancelling:
            return true
        case .completed, .failed, .cancelled:
            return false
        }
    }

    /// Truthful presentation status: shows `.cancelling` when a cancellation has been
    /// requested but not yet settled, and `.cancelled` only after full settlement.
    var presentationStatus: RunStatus {
        if cancellationRequestedAt != nil && cancellationSettledAt == nil {
            return .cancelling
        }
        let sorted = RunStageSnapshotLoader.load(for: self).sorted { $0.startedAt < $1.startedAt }
        if let latestStage = sorted.last {
            if persistedOperatorStatusDominatesTransientStageTruth {
                switch latestStage.status {
                case .pending, .ready, .running:
                    return status
                case .waitingApproval, .blocked, .failed:
                    break
                case .completed, .skipped:
                    return status
                }
            }
            switch latestStage.status {
            case .pending:
                return .pending
            case .ready:
                return status == .pending ? .pending : .ready
            case .running:
                return .running
            case .waitingApproval:
                return .waitingApproval
            case .blocked:
                return .blocked
            case .failed:
                return .failed
            case .completed, .skipped:
                break
            }
        }
        return status
    }

    /// Human-readable label for the current presentation status.
    var presentationStatusLabel: String {
        presentationStatus.rawValue.replacingOccurrences(of: "_", with: " ")
    }

    /// Human-readable label for list/sidebar status without loading stage snapshots.
    var listPresentationStatusLabel: String {
        listPresentationStatus.rawValue.replacingOccurrences(of: "_", with: " ")
    }

    /// Lightweight continuation state for resume/start paths.
    ///
    /// Proposal 032: Prefers the durable transition cursor when available.
    /// Falls back to heuristic reconstruction for pre-P032 runs (cursor == nil).
    var resumeContinuationStateID: String? {
        // Proposal 032 §5.4: Read the durable cursor first.
        if let cursor = transitionCursor {
            switch cursor.settlementPhase {
            case .transitionSettled:
                // Next state was scheduled but not started — resume from there.
                if let nextState = cursor.nextScheduledStateID {
                    return nextState
                }
            case .transitionStarted:
                // Next state had started execution — resume from it.
                if let nextState = cursor.nextScheduledStateID {
                    return nextState
                }
            case .awaitingFirstState:
                // No state ever completed — start from the beginning (nil = initial).
                return nil
            case .terminal, .awaitingConflictResolution:
                // Terminal runs shouldn't be resumed, but if called, return last completed.
                return cursor.lastCompletedStateID
            }
        }

        // Fallback: heuristic reconstruction for pre-P032 runs without a cursor.
        return heuristicResumeContinuationStateID
    }

    /// Pre-P032 heuristic resume targeting. Retained as fallback for runs that
    /// were created before the durable transition cursor existed.
    private var heuristicResumeContinuationStateID: String? {
        let persistedStages = PersistedRunGraph.stageExecutions(for: self)
        let sourceStages = persistedStages.isEmpty ? stageExecutions : persistedStages
        let sorted = sourceStages.sorted { lhs, rhs in
            if lhs.startedAt == rhs.startedAt {
                if lhs.iteration == rhs.iteration {
                    return lhs.attemptNumber < rhs.attemptNumber
                }
                return lhs.iteration < rhs.iteration
            }
            return lhs.startedAt < rhs.startedAt
        }

        if let interruptedTransitionStateID = interruptedTransitionResumeStateID(from: sorted) {
            return interruptedTransitionStateID
        }

        return sorted.last(where: {
            $0.status == .running
                || $0.status == .waitingApproval
                || $0.status == .blocked
                || $0.status == .failed
                || $0.status == .ready
        })?.stageID
            ?? sorted.last(where: { $0.status == .completed })?.stageID
    }

    private func interruptedTransitionResumeStateID(from stages: [StageExecution]) -> String? {
        guard let latestCompletedIndex = stages.lastIndex(where: { $0.status == .completed }) else {
            return nil
        }

        let trailingStages = Array(stages.suffix(from: stages.index(after: latestCompletedIndex)))

        if let materializedReadyStage = trailingStages.first(where: {
            $0.status == .ready && !$0.agentExecutions.contains(where: { !$0.artifacts.isEmpty })
        }) {
            return materializedReadyStage.stageID
        }

        guard let latestInterruptibleStage = trailingStages.last(where: {
            $0.status == .running
                || $0.status == .waitingApproval
                || $0.status == .blocked
                || $0.status == .failed
                || $0.status == .ready
        }) else {
            return nil
        }

        let hasPersistedArtifacts = latestInterruptibleStage.agentExecutions.contains { !$0.artifacts.isEmpty }
        guard !hasPersistedArtifacts else { return nil }

        return latestInterruptibleStage.stageID
    }
}

extension Run {
    var implementationHandoffStatus: ImplementationHandoffStatus? {
        get {
            guard let data = implementationHandoffStatusJSON else { return nil }
            return try? JSONDecoder().decode(ImplementationHandoffStatus.self, from: data)
        }
        set {
            implementationHandoffStatusJSON = try? JSONEncoder().encode(newValue)
        }
    }

    var workflowConflictBridgeV1: WorkflowConflictBridgeV1 {
        get {
            guard let data = workflowConflictRecordsJSONV1 else {
                return WorkflowConflictBridgeV1()
            }
            return (try? JSONDecoder().decode(WorkflowConflictBridgeV1.self, from: data))
                ?? WorkflowConflictBridgeV1()
        }
        set {
            workflowConflictRecordsJSONV1 = try? JSONEncoder().encode(newValue)
        }
    }

    var currentWorkflowConflictRecord: WorkflowConflictRecord? {
        workflowConflictBridgeV1.conflicts.last { $0.status.isCurrentBlocking }
    }

    func upsertWorkflowConflictRecord(_ record: WorkflowConflictRecord) {
        var bridge = workflowConflictBridgeV1
        let now = record.updatedAt
        if record.status.isCurrentBlocking {
            bridge.conflicts = bridge.conflicts.map { existing in
                guard existing.status.isCurrentBlocking,
                    existing.currentStateID == record.currentStateID,
                    existing.conflictFingerprint != record.conflictFingerprint,
                    existing.conflictID != record.conflictID
                else {
                    return existing
                }
                return existing.updatingStatus(
                    .superseded,
                    updatedAt: now,
                    supersededByConflictID: record.conflictID
                )
            }
        }
        if let index = bridge.conflicts.firstIndex(where: {
            $0.conflictFingerprint == record.conflictFingerprint
                || $0.conflictID == record.conflictID
        }) {
            bridge.conflicts[index] = record
        } else {
            bridge.conflicts.append(record)
        }
        workflowConflictBridgeV1 = bridge
    }

    @discardableResult
    func resolveCurrentWorkflowConflicts(
        currentStateID: String,
        selectedTransitionID: String,
        selectedNextStateID: String,
        stageExecutionID: UUID?,
        resolvedAt: String
    ) -> Int {
        var bridge = workflowConflictBridgeV1
        var resolvedCount = 0
        let resolution = WorkflowConflictRecord.graphResolutionRecord(
            selectedTransitionID: selectedTransitionID,
            selectedNextStateID: selectedNextStateID,
            stageExecutionID: stageExecutionID
        )
        bridge.conflicts = bridge.conflicts.map { conflict in
            guard conflict.status.isCurrentBlocking,
                conflict.currentStateID == currentStateID
            else {
                return conflict
            }
            resolvedCount += 1
            return conflict.updatingStatus(
                .resolved,
                updatedAt: resolvedAt,
                resolvedAt: resolvedAt,
                resolutionRecordJSON: resolution
            )
        }
        workflowConflictBridgeV1 = bridge
        return resolvedCount
    }

    func appendWorkflowAdvisoryRejectionRecord(_ record: WorkflowAdvisoryRejectionRecord) {
        var bridge = workflowConflictBridgeV1
        if let index = bridge.advisoryRejections.firstIndex(where: {
            $0.rejectionID == record.rejectionID
                || (
                    $0.currentStateID == record.currentStateID
                        && $0.selectedTransitionID == record.selectedTransitionID
                        && $0.advisoryHintHash == record.advisoryHintHash
                )
        }) {
            bridge.advisoryRejections[index] = record
        } else {
            bridge.advisoryRejections.append(record)
        }
        workflowConflictBridgeV1 = bridge
    }
}

// MARK: - CancellationSettlementEntry (Proposal 011 — REQ-002)

/// Records per-agent settlement details during cancellation propagation.
struct CancellationSettlementEntry: Codable, Sendable {
    let agentExecutionID: UUID
    let agentID: String
    let priorStatus: String              // status at cancellation-request time
    let terminalStatus: String           // status after propagation
    let sessionCloseAttempted: Bool
    let sessionCloseSucceeded: Bool?     // nil if no session was open
    let settledAt: Date
}

// MARK: - FrozenBindingProvenance (Proposal 011 — REQ-009)

/// How the resolved model was determined for a single agent binding.
/// Frozen at run start, never reconstructed from mutable current settings.
enum BindingProvenanceSource: String, Codable, Sendable {
    case backendProfileDefault = "backend_profile"
    case configuredProviderDefault = "configured_provider"
    case runOverride = "run_override"
    case unverifiable = "unverifiable"
}

/// Per-agent frozen provenance snapshot persisted in `Run.bindingProvenanceJSON`.
struct FrozenBindingProvenance: Codable, Sendable {
    /// The source that determined the resolved model.
    let source: BindingProvenanceSource
    /// The backend profile ID from the agent catalog.
    let backendProfileID: String
    /// The backend profile's declared model (may be "default" or explicit).
    let backendProfileModel: String
    /// The configured provider ID selected at run start (if any).
    let configuredProviderID: UUID?
    /// The configured provider's default model at the time of run start.
    let configuredProviderDefaultModel: String?
    /// The explicit run-start override model (if any).
    let runOverrideModel: String?
    /// The final resolved model actually sent to the runtime.
    let resolvedModel: String
    /// The final resolved provider family.
    let resolvedProviderFamily: String
}

enum DriftDecision: String, Codable {
    case continueWithOriginal
    case restartWithCurrent
    case cancelled
}

enum RiskClass: String, Codable {
    case standard
    case elevated
    case critical
}
