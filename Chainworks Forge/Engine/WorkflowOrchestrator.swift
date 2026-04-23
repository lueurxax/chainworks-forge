import CryptoKit
import Foundation
import Observation
import SwiftData

// MARK: - ApprovalRequest

/// Published when a stage requires human approval (ARCH-028).
struct ApprovalRequest: Identifiable, Sendable {
    let id: UUID
    let runID: UUID
    let stageID: String
    let stageLabel: String
    /// Artifact names available for review (§5.2, §8.2).
    let precedingArtifacts: [String]
    let requestedAt: Date
    /// Proposal 007 §11.1: approval policy for tailored gate rendering (e.g. "manual_release").
    let approvalPolicy: String?
}

struct LiveExecutionTimelineEntry: Identifiable, Sendable {
    let id: UUID
    let agentID: String
    let agentTitle: String
    let stageID: String
    let event: ExecutionEvent

    init(
        id: UUID = UUID(),
        agentID: String,
        agentTitle: String,
        stageID: String,
        event: ExecutionEvent
    ) {
        self.id = id
        self.agentID = agentID
        self.agentTitle = agentTitle
        self.stageID = stageID
        self.event = event
    }
}

// MARK: - WorkflowOrchestrator (@MainActor @Observable, per-run state machine driver)

/// Per-run state machine driver. Executes a RunPlan through its states,
/// managing sequential/parallel agent execution, approval gates, loops, and failure handling.
///
/// Design invariants:
/// - StageExecution/AgentExecution created LAZILY when entering a state (ARCH-027)
/// - Run.currentStageID stays derived (Proposal 001 invariant)
/// - Agent execution runs off-MainActor, results marshalled back
/// - Approval gates publish ApprovalRequest to callback
@MainActor
@Observable
final class WorkflowOrchestrator {
    private static let automaticWatchdogRetryReason = "automatic_watchdog_retry"

    // MARK: - Configuration

    let run: Run
    let plan: RunPlan
    let workspace: RunWorkspace
    let executor: AgentExecutor
    let modelContext: ModelContext
    let artifactManager: ArtifactManager
    let catalog: AgentCatalog?

    // MARK: - State

    private(set) var currentStateID: String
    private(set) var isRunning: Bool = false
    private(set) var isPaused: Bool = false
    private(set) var isCancelled: Bool = false
    private(set) var liveTimeline: [LiveExecutionTimelineEntry] = []
    private var pendingLiveTextChunksByAgentID: [String: String] = [:]
    private var suppressingStructuredOutputByAgentID: [String: Bool] = [:]

    /// Callback for approval requests (ARCH-028: published to a collection, not singleton)
    var onApprovalRequest: ((ApprovalRequest) -> Void)?

    /// Callback for orchestrator completion
    var onComplete: ((Bool) -> Void)?

    // MARK: - Internal tracking

    /// Tracks produced artifact names for transition evaluation
    private var producedArtifactNames: Set<String> = []
    /// Tracks artifact field data for expression evaluation
    private var artifactFields: [String: [String: AnyCodableValue]] = [:]
    private var transitionAdvisories: [TransitionAdvisoryHint] = []
    /// Runtime variables (clone of plan.variables, mutable for loop counters)
    private var runtimeVariables: [String: AnyCodableValue]
    /// Tracks currently active agent executions for live event routing.
    private var liveAgentExecutionsByAgentID: [String: [AgentExecution]] = [:]
    /// Coalesces high-frequency live text streaming so UI surfaces do not rebuild on every token.
    private var lastRoutedLiveTextChunkAtByAgentID: [String: Date] = [:]
    /// Frozen provider bindings captured at run start.
    private let providerBindingsByAgentID: [String: ResolvedProviderBinding]
    private let contextStrategyProfileID: String?
    private let strategyAssignmentMode: String?
    private let contextStrategyProfile: ContextStrategyProfile?
    private let promotedHandoffArtifacts: [String]
    private let handoffCompiler: HandoffCompiler = .init()

    // MARK: - Delivery Integration (Proposal 007)

    /// Decoded delivery configuration from the Run's frozen JSON, if this is a repo-backed run.
    private var deliveryConfig: DeliveryConfiguration? {
        guard let data = run.deliveryConfigurationJSON else { return nil }
        return try? JSONDecoder().decode(DeliveryConfiguration.self, from: data)
    }

    // MARK: - Init

    init(
        run: Run,
        plan: RunPlan,
        workspace: RunWorkspace,
        executor: AgentExecutor,
        modelContext: ModelContext,
        catalog: AgentCatalog? = nil
    ) {
        self.run = run
        self.plan = plan
        self.workspace = workspace
        self.executor = executor
        self.modelContext = modelContext
        self.artifactManager = ArtifactManager(modelContext: modelContext)
        self.catalog = catalog
        self.currentStateID = plan.initialStateID
        self.runtimeVariables = plan.variables
        self.providerBindingsByAgentID = Self.decodeProviderBindings(
            from: run.providerBindingSnapshotJSON)
        self.contextStrategyProfileID = Self.decodeContextStrategyProfileID(from: run)
        self.strategyAssignmentMode = Self.decodeStrategyAssignmentMode(from: run)
        self.contextStrategyProfile = Self.decodeContextStrategyProfile(from: run)
        self.promotedHandoffArtifacts = Self.decodePromotedHandoffArtifacts(from: run)
        configureLiveEventBridge()
    }

    // MARK: - Start Execution

    /// Start executing the workflow from the initial state (or a resumed state).
    func start(from stateID: String? = nil) async {
        guard !isRunning, !isCancelled else { return }

        if let resumeState = stateID {
            currentStateID = resumeState
        }

        // Proposal 032: Initialize durable transition cursor if absent.
        // For resumed runs that already have a cursor, preserve it.
        // For pre-P032 legacy runs being resumed with a heuristic continuation,
        // seed the cursor from the resume state so we don't lose the computed
        // continuation by overwriting with `.initial()`.
        if run.transitionCursor == nil {
            if let resumeState = stateID {
                let seededStage = resumableStageExecution(for: resumeState)
                // Legacy run being resumed — seed cursor reflecting that the
                // orchestrator is about to start executing `resumeState`.
                run.persistTransitionCursor(
                    TransitionCursor.seededForResume(
                        nextScheduledStateID: resumeState,
                        nextScheduledIteration: seededStage?.iteration,
                        nextScheduledAttemptNumber: seededStage?.attemptNumber,
                        scheduledStageExecutionID: seededStage?.id
                    ))
            } else {
                run.persistTransitionCursor(.initial())
            }
        }

        reconcileLateMaterializedOutputsIfNeeded()
        loadPersistedArtifacts()

        if restorePendingApprovalIfNeeded(for: currentStateID) {
            return
        }

        isRunning = true
        run.status = .running
        run.completedAt = nil
        healPrematureBlockedStateIfNeeded()
        do {
            try modelContext.save()
        } catch {
            RuntimeDiagnostics.log(
                "start durableRunStatusSaveFailed runID=\(run.id) error=\(error.localizedDescription)"
            )
        }

        // Main state machine loop
        await executeStateMachine()
    }

    /// Cancel the current execution (legacy immediate path — still available for rejection flows).
    func cancel() {
        isCancelled = true
        run.status = .cancelled
        run.completedAt = run.completedAt ?? Date()
        isRunning = false
    }

    /// Proposal 011 — REQ-002: Signal the orchestrator to stop advancing stages
    /// without directly setting the run to `.cancelled`. The cancellation coordinator
    /// handles the terminal transition after settlement.
    func signalCancellation() {
        isCancelled = true
        isRunning = false
    }

    /// Resolve an approval (called from UI or ExecutionService).
    func resolveApproval(stageID: String, granted: Bool, comment: String? = nil) {
        RuntimeDiagnostics.log("resolveApproval stageID=\(stageID) granted=\(granted)")
        // Find or create the Approval record
        let approval = run.approvals.first { $0.stageID == stageID && $0.decision == .requested }
        if let approval {
            approval.decision = granted ? .granted : .rejected
            approval.decidedAt = Date()
            approval.comment = comment
        }

        // Resume if we were waiting. Grants may execute run_after_approval work;
        // rejections skip that work and evaluate approval.rejected transitions.
        if isPaused {
            isPaused = false
            isRunning = true
            run.status = .running

            // Mark the stage execution as completed
            if let stageExec = run.stageExecutions.first(where: {
                $0.stageID == stageID && $0.status == .waitingApproval
            }) {
                stageExec.status = .completed
                stageExec.completedAt = Date()
            }

            // Evaluate transitions from the resolved approval state and resume.
            Task { @MainActor in
                await resumeAfterApproval(stageID: stageID, approvalGranted: granted)
            }
        }
    }

    /// Resume execution after an approval is resolved.
    private func resumeAfterApproval(stageID: String, approvalGranted: Bool) async {
        guard let state = plan.states[stageID] else {
            RuntimeDiagnostics.log("resumeAfterApproval missingState stageID=\(stageID)")
            return
        }
        RuntimeDiagnostics.log("resumeAfterApproval begin stageID=\(stageID)")

        // Execute run_after_approval only for granted approvals.
        if approvalGranted, let runAfterApproval = state.runAfterApproval {
            if let stageExec = run.stageExecutions.first(where: { $0.stageID == stageID }) {
                let success = await executeRunBlock(
                    runAfterApproval, state: state, stageExec: stageExec)
                if !success {
                    RuntimeDiagnostics.log(
                        "resumeAfterApproval runAfterApprovalFailed stageID=\(stageID)")
                    if run.status == .blocked,
                        run.currentWorkflowConflictRecord?.reason == .implementationHandoffUnavailable
                    {
                        isRunning = false
                        onComplete?(false)
                        return
                    }
                    handleFailure(state: state)
                    return
                }
            }
        }

        // Evaluate transitions from the resolved approval state
        let context = makeTransitionEvaluationContext(
            approvalGranted: approvalGranted,
            approvalRejected: !approvalGranted
        )

        let resolution = TransitionEvaluator.resolveAuthority(
            transitions: state.transitions,
            fromStateID: state.id,
            context: context
        )

        guard let transition = resolution.selectedTransition else {
            RuntimeDiagnostics.log("resumeAfterApproval noTransition stageID=\(stageID)")
            let completedStageExecutionID = run.stageExecutions.first(where: {
                $0.stageID == stageID && $0.status == .completed
            })?.id
            run.status = .blocked
            let conflict = persistBlockingWorkflowConflict(
                resolution: resolution,
                currentStateID: state.id,
                stageExecutionID: completedStageExecutionID
            )
            settleWorkflowConflict(
                conflict,
                currentStateID: state.id,
                currentStageExecutionID: completedStageExecutionID
            )
            isRunning = false
            onComplete?(false)
            return
        }

        // Proposal 032: Atomic settlement for approval-resume transition (fail-closed).
        let completedStageExec = run.stageExecutions
            .filter { $0.stageID == stageID && $0.status == .completed }
            .sorted { $0.startedAt < $1.startedAt }
            .last
        let settled = settleTransition(
            completedStateID: stageID,
            completedStageExecutionID: completedStageExec?.id,
            nextStateID: transition.to
        )

        guard settled else {
            RuntimeDiagnostics.log(
                "resumeAfterApproval settlementFailed stageID=\(stageID) to=\(transition.to)")
            run.status = .blocked
            run.driftDetails =
                "Transition settlement failed after approval: could not durably persist continuation from '\(stageID)' to '\(transition.to)'. Manual resume required."
            isRunning = false
            onComplete?(false)
            return
        }
        resolveWorkflowConflictAfterSelectedTransition(
            resolution: resolution,
            currentStateID: stageID,
            stageExecutionID: completedStageExec?.id
        )
        recordAdvisoryRejectionsAfterSelectedTransition(
            resolution: resolution,
            currentStateID: stageID,
            stageExecutionID: completedStageExec?.id
        )

        RuntimeDiagnostics.log(
            "resumeAfterApproval transition stageID=\(stageID) to=\(transition.to)")
        currentStateID = transition.to
        await executeStateMachine()
    }

    // MARK: - State Machine Core

    private func executeStateMachine() async {
        while isRunning && !isCancelled {
            guard let state = plan.states[currentStateID] else {
                run.status = .failed
                isRunning = false
                onComplete?(false)
                return
            }

            // Check for end state
            if state.type == .end && !stateHasExecutableWork(state) {
                RuntimeDiagnostics.log("executeStateMachine reachedEnd stateID=\(state.id)")
                run.status = .completed
                run.completedAt = Date()
                settleTerminal()
                persistDeliveryReceiptIfNeeded(finalStateID: state.id)
                persistFinalFeatureReportIfNeeded(finalStateID: state.id)
                persistDeclarativeCoverageIfNeeded(finalStateID: state.id)
                isRunning = false
                onComplete?(true)
                return
            }

            // Execute the state
            let result = await executeState(state)

            if isCancelled { return }

            switch result {
            case .paused:
                // Waiting for approval — exit loop, will resume later
                return
            case .blocked:
                isRunning = false
                onComplete?(false)
                return
            case .failed:
                handleFailure(state: state)
                return
            case .succeeded:
                if state.type == .end {
                    RuntimeDiagnostics.log(
                        "executeStateMachine completedTerminalEndState stateID=\(state.id)")
                    run.status = .completed
                    run.completedAt = Date()
                    settleTerminal(
                        lastCompletedStateID: state.id,
                        lastCompletedStageExec: run.stageExecutions.last {
                            $0.stageID == state.id && $0.status == .completed
                        })
                    persistDeliveryReceiptIfNeeded(finalStateID: state.id)
                    persistFinalFeatureReportIfNeeded(finalStateID: state.id)
                    persistDeclarativeCoverageIfNeeded(finalStateID: state.id)
                    isRunning = false
                    onComplete?(true)
                    return
                }
                break  // Continue to transition evaluation
            }

            // Evaluate transitions
            let context = makeTransitionEvaluationContext(
                approvalGranted: isApprovalGranted(for: state.id),
                approvalRejected: isApprovalRejected(for: state.id)
            )

            if state.transitions.isEmpty {
                // No transition matches — check if we should wait (approval) or fail
                if state.approvalRequired && !isApprovalGranted(for: state.id) {
                    // Already handled in executeState — should be paused
                    return
                }
                // Dead end — mark complete if no transitions defined
                RuntimeDiagnostics.log(
                    "executeStateMachine deadEndComplete stateID=\(state.id)")
                run.status = .completed
                run.completedAt = Date()
                settleTerminal(
                    lastCompletedStateID: state.id,
                    lastCompletedStageExec: run.stageExecutions.last {
                        $0.stageID == state.id && $0.status == .completed
                    })
                persistDeliveryReceiptIfNeeded(finalStateID: state.id)
                isRunning = false
                onComplete?(true)
                return
            }

            let resolution = TransitionEvaluator.resolveAuthority(
                transitions: state.transitions,
                fromStateID: state.id,
                context: context
            )

            guard let transition = resolution.selectedTransition else {
                // Otherwise, stalled
                RuntimeDiagnostics.log(
                    "executeStateMachine blockedNoTransition stateID=\(state.id) artifacts=\(producedArtifactNames.sorted())"
                )
                let completedStageExecutionID = run.stageExecutions.last {
                    $0.stageID == state.id && $0.status == .completed
                }?.id
                run.status = .blocked
                let conflict = persistBlockingWorkflowConflict(
                    resolution: resolution,
                    currentStateID: state.id,
                    stageExecutionID: completedStageExecutionID
                )
                settleWorkflowConflict(
                    conflict,
                    currentStateID: state.id,
                    currentStageExecutionID: completedStageExecutionID
                )
                isRunning = false
                onComplete?(false)
                return
            }

            // Proposal 032: Atomic transition settlement (fail-closed).
            // Persist the completed state and scheduled next state in one save boundary
            // before advancing the state machine. If the save fails, the state machine
            // must NOT advance — block instead to prevent half-settled state.
            let completedStageExec = run.stageExecutions
                .filter { $0.stageID == state.id && $0.status == .completed }
                .sorted { $0.startedAt < $1.startedAt }
                .last
            let settled = settleTransition(
                completedStateID: state.id,
                completedStageExecutionID: completedStageExec?.id,
                nextStateID: transition.to
            )

            guard settled else {
                RuntimeDiagnostics.log(
                    "executeStateMachine settlementFailed from=\(state.id) to=\(transition.to)")
                run.status = .blocked
                run.driftDetails =
                    "Transition settlement failed: could not durably persist continuation from '\(state.id)' to '\(transition.to)'. Manual resume required."
                isRunning = false
                onComplete?(false)
                return
            }
            resolveWorkflowConflictAfterSelectedTransition(
                resolution: resolution,
                currentStateID: state.id,
                stageExecutionID: completedStageExec?.id
            )
            recordAdvisoryRejectionsAfterSelectedTransition(
                resolution: resolution,
                currentStateID: state.id,
                stageExecutionID: completedStageExec?.id
            )

            RuntimeDiagnostics.log(
                "executeStateMachine transition from=\(state.id) to=\(transition.to)")
            healPrematureBlockedStateIfNeeded()
            currentStateID = transition.to
        }
    }

    private func makeTransitionEvaluationContext(
        approvalGranted: Bool,
        approvalRejected: Bool
    ) -> TransitionEvaluator.EvaluationContext {
        TransitionEvaluator.EvaluationContext(
            producedArtifactNames: producedArtifactNames,
            declaredArtifactNames: declaredWorkflowArtifactNames,
            approvalGranted: approvalGranted,
            approvalRejected: approvalRejected,
            variables: runtimeVariables,
            artifactFields: artifactFields
        )
    }

    private var declaredWorkflowArtifactNames: Set<String> {
        var names = Set(plan.agentBindings.values.flatMap(\.outputs))
        for state in plan.states.values {
            collectDeclaredOutputs(from: state.runBlock, into: &names)
            collectDeclaredOutputs(from: state.runAfterApproval, into: &names)
        }
        return names
    }

    private func collectDeclaredOutputs(
        from block: ExecutableRunBlock?,
        into names: inout Set<String>
    ) {
        guard let block else { return }
        for phase in block.phases {
            let tasks: [AgentTask]
            switch phase {
            case .sequential(let phaseTasks), .parallel(let phaseTasks):
                tasks = phaseTasks
            }
            for task in tasks {
                for output in task.outputs ?? [] {
                    names.insert(output)
                }
            }
        }
    }

    private func persistBlockingWorkflowConflict(
        resolution: TransitionEvaluator.AuthorityResolution,
        currentStateID: String,
        stageExecutionID: UUID?
    ) -> WorkflowConflictRecord {
        let reason = resolution.conflictReason ?? .workflowConflictUnverifiable
        let candidateHash = workflowConflictHash(for: resolution.candidateEvaluations)
        let fingerprint = workflowConflictFingerprint(
            currentStateID: currentStateID,
            reason: reason,
            candidateHash: candidateHash
        )
        let now = ISO8601DateFormatter().string(from: Date())
        let existing = run.workflowConflictBridgeV1.conflicts.first {
            $0.conflictFingerprint == fingerprint
        }
        let status = workflowConflictInitialStatus(for: reason)
        let terminalFailureReason = workflowConflictTerminalFailureReason(for: reason)
        let record = WorkflowConflictRecord(
            conflictID: existing?.conflictID ?? "conflict-\(UUID().uuidString)",
            conflictFingerprint: fingerprint,
            runID: run.id.uuidString,
            stageExecutionID: stageExecutionID?.uuidString,
            lineageID: nil,
            currentStateID: currentStateID,
            reason: reason,
            operatorLabel: resolution.operatorLabel ?? "Workflow transition requires resolution",
            status: status,
            candidateTransitions: resolution.candidateEvaluations,
            candidateTransitionHash: candidateHash,
            advisoryEvidenceRefs: [],
            createdAt: existing?.createdAt ?? now,
            updatedAt: now,
            terminalFailureReason: terminalFailureReason
        )
        run.upsertWorkflowConflictRecord(record)
        RuntimeDiagnostics.log(
            "workflowConflict persisted stateID=\(currentStateID) reason=\(reason.rawValue) fingerprint=\(fingerprint)"
        )
        do {
            try modelContext.save()
        } catch {
            RuntimeDiagnostics.log(
                "workflowConflict saveFailed stateID=\(currentStateID) error=\(error.localizedDescription)"
            )
            run.driftDetails =
                "Workflow conflict persisted in memory but could not be saved: \(error.localizedDescription)"
        }
        return record
    }

    private func workflowConflictInitialStatus(for reason: WorkflowConflictReason) -> WorkflowConflictStatus {
        switch reason {
        case .aggregateTransitionTruthConflicted:
            return .operatorConfirmationRequired
        case .workflowConflictUnverifiable:
            return .terminalUnverifiable
        default:
            return .unresolved
        }
    }

    private func workflowConflictTerminalFailureReason(for reason: WorkflowConflictReason) -> String? {
        guard reason == .workflowConflictUnverifiable else { return nil }
        return "Workflow transition outcome could not be verified"
    }

    private func resolveWorkflowConflictAfterSelectedTransition(
        resolution: TransitionEvaluator.AuthorityResolution,
        currentStateID: String,
        stageExecutionID: UUID?
    ) {
        guard let selectedEvaluation = resolution.selectedEvaluation,
            let selectedNextStateID = resolution.selectedNextStateID
        else {
            return
        }
        let now = ISO8601DateFormatter().string(from: Date())
        let resolvedCount = run.resolveCurrentWorkflowConflicts(
            currentStateID: currentStateID,
            selectedTransitionID: selectedEvaluation.transitionID,
            selectedNextStateID: selectedNextStateID,
            stageExecutionID: stageExecutionID,
            resolvedAt: now
        )
        guard resolvedCount > 0 else { return }
        do {
            try modelContext.save()
            RuntimeDiagnostics.log(
                "workflowConflict resolved stateID=\(currentStateID) selectedTransitionID=\(selectedEvaluation.transitionID) count=\(resolvedCount)"
            )
        } catch {
            RuntimeDiagnostics.log(
                "workflowConflict resolveSaveFailed stateID=\(currentStateID) error=\(error.localizedDescription)"
            )
            run.driftDetails =
                "Workflow conflict resolved in memory but could not be saved: \(error.localizedDescription)"
        }
    }

    private func recordAdvisoryRejectionsAfterSelectedTransition(
        resolution: TransitionEvaluator.AuthorityResolution,
        currentStateID: String,
        stageExecutionID: UUID?
    ) {
        guard let selectedEvaluation = resolution.selectedEvaluation,
            let selectedNextStateID = resolution.selectedNextStateID
        else {
            return
        }

        let rejectedAdvisories = transitionAdvisories.filter { advisory in
            guard let nextStage = advisory.nextStageHint else { return false }
            return nextStage != selectedNextStateID
        }
        guard !rejectedAdvisories.isEmpty else { return }

        let now = ISO8601DateFormatter().string(from: Date())
        for advisory in rejectedAdvisories {
            let graphMembershipResult = advisoryGraphMembershipResult(
                advisory: advisory,
                selectedNextStateID: selectedNextStateID
            )
            let advisoryHintHash = workflowConflictHash(for: [
                "schema": AnyCodableValue.string("workflow_advisory_hint_v1"),
                "source_artifact_id": .string(advisory.sourceArtifactID),
                "next_stage": advisory.nextStageHint.map(AnyCodableValue.string) ?? .null,
                "next_action": advisory.nextAction.map(AnyCodableValue.string) ?? .null,
                "graph_membership_result": .string(graphMembershipResult)
            ])
            let record = WorkflowAdvisoryRejectionRecord(
                rejectionID: "advisory-rejection-\(UUID().uuidString)",
                runID: run.id.uuidString,
                stageExecutionID: stageExecutionID?.uuidString,
                lineageID: stageExecutionID?.uuidString,
                currentStateID: currentStateID,
                selectedTransitionID: selectedEvaluation.transitionID,
                selectedNextStateID: selectedNextStateID,
                advisoryNextStageHint: advisory.nextStageHint,
                advisoryNextAction: advisory.nextAction,
                advisoryHintHash: advisoryHintHash,
                advisoryHintProvenance: advisoryProvenance(
                    for: advisory,
                    graphMembershipResult: graphMembershipResult
                ),
                graphMembershipResult: graphMembershipResult,
                createdAt: now
            )
            run.appendWorkflowAdvisoryRejectionRecord(record)
        }

        do {
            try modelContext.save()
            RuntimeDiagnostics.log(
                "workflowAdvisoryRejection persisted stateID=\(currentStateID) selectedNextStateID=\(selectedNextStateID) count=\(rejectedAdvisories.count)"
            )
        } catch {
            RuntimeDiagnostics.log(
                "workflowAdvisoryRejection saveFailed stateID=\(currentStateID) error=\(error.localizedDescription)"
            )
            run.driftDetails =
                "Workflow advisory rejection persisted in memory but could not be saved: \(error.localizedDescription)"
        }
    }

    private func advisoryGraphMembershipResult(
        advisory: TransitionAdvisoryHint,
        selectedNextStateID: String
    ) -> String {
        guard let nextStage = advisory.nextStageHint else {
            return "no_next_stage_hint"
        }
        guard plan.states[nextStage] != nil else {
            return "absent_from_graph"
        }
        return nextStage == selectedNextStateID ? "graph_state_selected" : "graph_state_not_selected"
    }

    private func advisoryProvenance(
        for advisory: TransitionAdvisoryHint,
        graphMembershipResult: String
    ) -> [AdvisoryHintExtraction] {
        var provenance: [AdvisoryHintExtraction] = []
        if let nextStage = advisory.nextStageHint {
            provenance.append(
                AdvisoryHintExtraction(
                    sourceArtifactID: advisory.sourceArtifactID,
                    sourceAgentExecutionID: advisory.sourceAgentExecutionID,
                    advisoryPath: "$.next_stage",
                    rawValueHash: workflowConflictHash(for: nextStage),
                    redactedValue: nextStage,
                    graphMembershipResult: graphMembershipResult,
                    supersededByProjection: advisory.supersededByProjection,
                    includedInCandidateTransitionHash: true
                )
            )
        }
        if let nextAction = advisory.nextAction {
            provenance.append(
                AdvisoryHintExtraction(
                    sourceArtifactID: advisory.sourceArtifactID,
                    sourceAgentExecutionID: advisory.sourceAgentExecutionID,
                    advisoryPath: "$.next_action",
                    rawValueHash: workflowConflictHash(for: nextAction),
                    redactedValue: nextAction,
                    graphMembershipResult: graphMembershipResult,
                    supersededByProjection: advisory.supersededByProjection,
                    includedInCandidateTransitionHash: true
                )
            )
        }
        return provenance
    }

    private func workflowConflictHash(
        for evaluations: [CandidateTransitionEvaluation]
    ) -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        let data = (try? encoder.encode(evaluations)) ?? Data()
        return "sha256:\(sha256Hex(data))"
    }

    private func workflowConflictHash<T: Encodable>(for value: T) -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        let data = (try? encoder.encode(value)) ?? Data()
        return "sha256:\(sha256Hex(data))"
    }

    private func workflowConflictFingerprint(
        currentStateID: String,
        reason: WorkflowConflictReason,
        candidateHash: String
    ) -> String {
        let source = [
            run.id.uuidString,
            currentStateID,
            reason.rawValue,
            candidateHash
        ].joined(separator: "|")
        return "sha256:\(sha256Hex(Data(source.utf8)))"
    }

    private func sha256Hex(_ data: Data) -> String {
        SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    }

    // MARK: - State Execution Result

    private enum StateResult {
        case succeeded
        case failed
        case blocked
        case paused  // waiting for approval
    }

    private func stateHasExecutableWork(_ state: ExecutableState) -> Bool {
        state.runBlock != nil || state.runAfterApproval != nil || state.approvalRequired
    }

    // MARK: - Execute State

    /// Execute a single state: run block, then check approval.
    private func executeState(_ state: ExecutableState) async -> StateResult {
        RuntimeDiagnostics.log("executeState begin stateID=\(state.id)")

        let stageExec: StageExecution
        var requiresDurableStageStartSave = false
        if let scheduledSelection = scheduledStageSelection(for: state.id) {
            if let scheduledStage = scheduledSelection.execution,
                scheduledStage.status == .running || scheduledStage.status == .ready
            {
                stageExec = scheduledStage
                stageExec.status = .running
                stageExec.completedAt = nil
                requiresDurableStageStartSave = true
            } else {
                let freshStage = StageExecution(
                    stageID: state.id,
                    label: state.label,
                    status: .running,
                    iteration: scheduledSelection.iteration,
                    attemptNumber: scheduledSelection.attemptNumber
                )
                freshStage.run = run
                modelContext.insert(freshStage)
                stageExec = freshStage
                requiresDurableStageStartSave = true
            }
        } else if let resumableStage = resumableStageExecution(for: state.id) {
            stageExec = resumableStage
            stageExec.status = .running
            stageExec.completedAt = nil
            requiresDurableStageStartSave = true
        } else {
            // Create StageExecution lazily (ARCH-027)
            let iteration = currentIteration(for: state.id)
            let freshStage = StageExecution(
                stageID: state.id,
                label: state.label,
                status: .running,
                iteration: iteration,
                attemptNumber: 1
            )
            freshStage.run = run
            modelContext.insert(freshStage)
            stageExec = freshStage
            requiresDurableStageStartSave = true
        }

        // Proposal 032: Mark the transition as started once the downstream stage
        // is materialized, not earlier at state entry.
        if let cursor = run.transitionCursor,
            cursor.settlementPhase == .transitionSettled,
            cursor.nextScheduledStateID == state.id
        {
            run.persistTransitionCursor(cursor.markingTransitionStarted())
            requiresDurableStageStartSave = true
        }

        if requiresDurableStageStartSave {
            do {
                try modelContext.save()
            } catch {
                RuntimeDiagnostics.log(
                    "executeState durableStageStartSaveFailed stateID=\(state.id) error=\(error.localizedDescription)"
                )
                run.driftDetails =
                    "Failed to durably persist stage start for '\(state.id)': \(error.localizedDescription)"
                return .failed
            }
        }

        // Proposal 007: Provision worktree before executing implementation states
        do {
            try await provisionWorktreeIfNeeded(for: state)
        } catch {
            RuntimeDiagnostics.log(
                "executeState worktreeProvisioningFailed stateID=\(state.id) error=\(error.localizedDescription)"
            )
            stageExec.status = .failed
            stageExec.completedAt = Date()
            stageExec.label =
                "\(state.label) — worktree provisioning failed: \(error.localizedDescription)"
            return .failed
        }

        // Execute run block
        if let runBlock = state.runBlock {
            let blockSuccess = await executeRunBlock(runBlock, state: state, stageExec: stageExec)
            if !blockSuccess {
                RuntimeDiagnostics.log("executeState runBlockFailed stateID=\(state.id)")
                if run.status == .blocked,
                    run.currentWorkflowConflictRecord?.reason == .implementationHandoffUnavailable
                {
                    return .blocked
                }
                stageExec.status = .failed
                stageExec.completedAt = Date()
                return .failed
            }
        }

        // Check if approval is required
        if state.approvalRequired {
            stageExec.status = .waitingApproval
            run.status = .waitingApproval
            isPaused = true

            // Create approval record
            let approval = Approval(stageID: state.id)
            approval.decision = .requested
            approval.run = run
            modelContext.insert(approval)

            // Publish approval request with preceding artifact names (§5.2, §8.2)
            let request = ApprovalRequest(
                id: approval.id,
                runID: workspace.runID,
                stageID: state.id,
                stageLabel: state.label,
                precedingArtifacts: Array(producedArtifactNames.sorted()),
                requestedAt: Date(),
                approvalPolicy: state.approvalPolicy
            )
            onApprovalRequest?(request)
            RuntimeDiagnostics.log(
                "executeState waitingApproval stateID=\(state.id) policy=\(state.approvalPolicy ?? "nil")"
            )
            return .paused  // Will resume when approval is resolved
        }

        // Execute run_after_approval block (if approval was already granted on resume)
        if let runAfterApproval = state.runAfterApproval {
            let afterSuccess = await executeRunBlock(
                runAfterApproval, state: state, stageExec: stageExec)
            if !afterSuccess {
                RuntimeDiagnostics.log("executeState runAfterApprovalFailed stateID=\(state.id)")
                if run.status == .blocked,
                    run.currentWorkflowConflictRecord?.reason == .implementationHandoffUnavailable
                {
                    return .blocked
                }
                stageExec.status = .failed
                stageExec.completedAt = Date()
                return .failed
            }
        }

        // Handle loops
        if let loop = state.loop {
            let counter = run.loopCounters[loop.counter] ?? 0
            let newCount = counter + 1
            run.loopCounters[loop.counter] = newCount

            if newCount >= loop.resolvedMax {
                // Budget exhausted
                handleLoopBudgetExhausted(state: state, counter: loop.counter, count: newCount)
                stageExec.status = .completed
                stageExec.completedAt = Date()
                healPrematureBlockedStateIfNeeded()
                return .succeeded  // Let transition evaluator decide next state
            }

            // Update runtime variable for expression evaluation
            runtimeVariables[loop.counter] = .int(newCount)
        }

        stageExec.status = .completed
        stageExec.completedAt = Date()
        healPrematureBlockedStateIfNeeded()
        RuntimeDiagnostics.log("executeState completed stateID=\(state.id)")
        return .succeeded
    }

    private func implementationHandoffUnavailable(
        for task: AgentTask,
        agent: ResolvedAgent,
        state: ExecutableState,
        stageExec: StageExecution
    ) -> Bool {
        guard isCodeWriterImplementationTask(task, agent: agent) else {
            return false
        }

        let requiredArtifacts = (task.inputs ?? []).sorted()
        guard !requiredArtifacts.isEmpty else {
            persistImplementationHandoffStatus(
                stateID: state.id,
                taskName: task.task,
                requiredArtifacts: [],
                availableArtifacts: [],
                missingArtifacts: [],
                codeWriterStartStatus: "not_queued",
                status: "not_required"
            )
            return false
        }

        ensureApprovedProposalHandoffArtifactIfPossible(state: state, stageExec: stageExec)
        let persisted = persistedArtifacts()
        let availableArtifacts = Set(persisted.map(\.name))
        let missingArtifacts = requiredArtifacts.filter { !availableArtifacts.contains($0) }
        if missingArtifacts.isEmpty {
            persistImplementationHandoffStatus(
                stateID: state.id,
                taskName: task.task,
                requiredArtifacts: requiredArtifacts,
                availableArtifacts: requiredArtifacts,
                missingArtifacts: [],
                codeWriterStartStatus: "not_queued",
                status: "ready"
            )
            return false
        }

        RuntimeDiagnostics.log(
            "implementationHandoffUnavailable stateID=\(state.id) task=\(task.task) missing=\(missingArtifacts.joined(separator: ","))"
        )
        stageExec.status = .blocked
        stageExec.completedAt = Date()
        run.status = .blocked
        persistImplementationHandoffStatus(
            stateID: state.id,
            taskName: task.task,
            requiredArtifacts: requiredArtifacts,
            availableArtifacts: requiredArtifacts.filter { availableArtifacts.contains($0) },
            missingArtifacts: missingArtifacts,
            codeWriterStartStatus: "blocked_before_code",
            status: "blocked_before_code"
        )
        persistImplementationHandoffUnavailableConflict(
            currentStateID: state.id,
            stageExecutionID: stageExec.id,
            requiredArtifacts: requiredArtifacts,
            missingArtifacts: missingArtifacts
        )
        settleWorkflowConflictBlocked(
            currentStateID: state.id,
            currentStageExecutionID: stageExec.id
        )
        return true
    }

    private func isCodeWriterImplementationTask(
        _ task: AgentTask,
        agent: ResolvedAgent
    ) -> Bool {
        guard agent.id == "code_writer" else { return false }
        return task.task == "start_implementation"
            || task.task == "initial_implementation"
            || task.task == "continue_implementation"
    }

    private func persistImplementationHandoffStatus(
        stateID: String,
        taskName: String,
        requiredArtifacts: [String],
        availableArtifacts: [String],
        missingArtifacts: [String],
        codeWriterStartStatus: String,
        status: String
    ) {
        let approvedProposalArtifact = persistedArtifacts()
            .last { $0.name == "approved_proposal" }
        run.implementationHandoffStatus = ImplementationHandoffStatus(
            runID: run.id.uuidString,
            currentStateID: stateID,
            taskName: taskName,
            requiredInputArtifacts: requiredArtifacts,
            availableInputArtifacts: availableArtifacts,
            missingInputArtifacts: missingArtifacts,
            approvedProposalPresent: availableArtifacts.contains("approved_proposal"),
            approvedProposalArtifactID: approvedProposalArtifact?.id.uuidString,
            approvedProposalDigest: approvedProposalArtifact?.checksumSHA256,
            worktreeRoot: run.worktreeRoot,
            workspaceRoot: run.workspaceRoot,
            artifactRoot: run.artifactRoot,
            codeWriterStartStatus: codeWriterStartStatus,
            status: status,
            missingHandoffOutputs: missingArtifacts,
            retryableFrom: "implementation_handoff:\(stateID)",
            blockedBeforeCodeReason: status == "blocked_before_code"
                ? "implementation_handoff_unavailable"
                : nil,
            updatedAt: ISO8601DateFormatter().string(from: Date())
        )
    }

    private func persistImplementationHandoffCodeWriterStartStatus(_ status: String) {
        guard let current = run.implementationHandoffStatus,
            current.status != "blocked_before_code"
        else {
            return
        }

        run.implementationHandoffStatus = ImplementationHandoffStatus(
            schemaVersion: current.schemaVersion,
            runID: current.runID,
            currentStateID: current.currentStateID,
            taskName: current.taskName,
            requiredInputArtifacts: current.requiredInputArtifacts,
            availableInputArtifacts: current.availableInputArtifacts,
            missingInputArtifacts: current.missingInputArtifacts,
            approvedProposalPresent: current.approvedProposalPresent,
            approvedProposalArtifactID: current.approvedProposalArtifactID,
            approvedProposalDigest: current.approvedProposalDigest,
            worktreeRoot: current.worktreeRoot,
            workspaceRoot: current.workspaceRoot,
            artifactRoot: current.artifactRoot,
            codeWriterStartStatus: status,
            status: current.status,
            missingHandoffOutputs: current.missingHandoffOutputs,
            lastHandoffAgentExecutionID: current.lastHandoffAgentExecutionID,
            retryableFrom: current.retryableFrom,
            blockedBeforeCodeReason: current.blockedBeforeCodeReason,
            updatedAt: ISO8601DateFormatter().string(from: Date())
        )

        do {
            try modelContext.save()
        } catch {
            RuntimeDiagnostics.log(
                "implementationHandoffStartStatusSaveFailed status=\(status) error=\(error.localizedDescription)"
            )
        }
    }

    private func ensureApprovedProposalHandoffArtifactIfPossible(
        state: ExecutableState,
        stageExec: StageExecution
    ) {
        let artifacts = persistedArtifacts()
        guard !artifacts.contains(where: { $0.name == "approved_proposal" }),
            let proposalCurrent = artifacts.last(where: { $0.name == "proposal_current" }),
            let data = try? artifactManager.readArtifact(proposalCurrent, workspace: workspace)
        else {
            return
        }

        do {
            _ = try artifactManager.persistSystemArtifact(
                name: "approved_proposal",
                data: data,
                contractID: "approved_proposal",
                format: proposalCurrent.format,
                workspace: workspace,
                stageID: state.id,
                agentID: "engine",
                provider: "engine",
                model: nil,
                effort: nil,
                attemptNumber: stageExec.attemptNumber
            )
            RuntimeDiagnostics.log(
                "implementationHandoffSnapshottedApprovedProposal stateID=\(state.id) sourceArtifactID=\(proposalCurrent.id.uuidString)"
            )
        } catch {
            RuntimeDiagnostics.log(
                "implementationHandoffApprovedProposalSnapshotFailed stateID=\(state.id) error=\(error.localizedDescription)"
            )
        }
    }

    private func persistImplementationHandoffUnavailableConflict(
        currentStateID: String,
        stageExecutionID: UUID?,
        requiredArtifacts: [String],
        missingArtifacts: [String]
    ) {
        let candidate = CandidateTransitionEvaluation(
            transitionID: "\(currentStateID)__implementation_handoff",
            fromStateID: currentStateID,
            toStateID: currentStateID,
            conditionExpressionID: "implementation_handoff.required_inputs",
            result: .missingInput,
            requiredArtifacts: requiredArtifacts,
            missingArtifacts: missingArtifacts,
            missingFields: [],
            sourceArtifactIDs: [],
            sourceAgentExecutionID: nil,
            sanitizedDiagnostic:
                "Implementation handoff is missing required input artifact(s): \(missingArtifacts.joined(separator: ", "))"
        )
        let candidateHash = workflowConflictHash(for: [candidate])
        let fingerprint = workflowConflictFingerprint(
            currentStateID: currentStateID,
            reason: .implementationHandoffUnavailable,
            candidateHash: candidateHash
        )
        let now = ISO8601DateFormatter().string(from: Date())
        let existing = run.workflowConflictBridgeV1.conflicts.first {
            $0.conflictFingerprint == fingerprint
        }
        let record = WorkflowConflictRecord(
            conflictID: existing?.conflictID ?? "conflict-\(UUID().uuidString)",
            conflictFingerprint: fingerprint,
            runID: run.id.uuidString,
            stageExecutionID: stageExecutionID?.uuidString,
            lineageID: nil,
            currentStateID: currentStateID,
            reason: .implementationHandoffUnavailable,
            operatorLabel: "Implementation handoff is unavailable",
            status: .unresolved,
            candidateTransitions: [candidate],
            candidateTransitionHash: candidateHash,
            advisoryEvidenceRefs: [],
            createdAt: existing?.createdAt ?? now,
            updatedAt: now
        )
        run.upsertWorkflowConflictRecord(record)
    }

    // MARK: - Execute Run Blocks

    private func executeRunBlock(
        _ block: ExecutableRunBlock,
        state: ExecutableState,
        stageExec: StageExecution
    ) async -> Bool {
        for phase in block.phases {
            switch phase {
            case .sequential(let tasks):
                for task in tasks {
                    if isCancelled { return false }
                    let success = await executeAgentTask(task, state: state, stageExec: stageExec)
                    if !success { return false }
                }

            case .parallel(let tasks):
                let success = await executeParallelTasks(tasks, state: state, stageExec: stageExec)
                if !success { return false }
            }
        }
        return true
    }

    private func executeAgentTask(
        _ task: AgentTask,
        state: ExecutableState,
        stageExec: StageExecution
    ) async -> Bool {
        guard let resolvedAgent = plan.agentBindings[task.agent] else {
            return false
        }
        let agent = effectiveAgent(from: resolvedAgent)

        if implementationHandoffUnavailable(for: task, agent: agent, state: state, stageExec: stageExec) {
            return false
        }

        let agentExec: AgentExecution
        if let resumableAgent = resumableAgentExecution(for: task, agent: agent, in: stageExec) {
            switch resumableAgent.status {
            case .completed:
                return true
            case .pending, .ready, .running:
                agentExec = resumableAgent
                agentExec.status = .running
                agentExec.completedAt = nil
            case .failed, .cancelled, .skipped:
                let freshExecution = AgentExecution(
                    agentID: agent.id,
                    agentTitle: agent.title,
                    taskName: task.task,
                    status: .running,
                    provider: agent.provider,
                    effort: agent.effort
                )
                freshExecution.stageExecution = stageExec
                modelContext.insert(freshExecution)
                agentExec = freshExecution
            }
        } else {
            // Create AgentExecution lazily (ARCH-027)
            let freshExecution = AgentExecution(
                agentID: agent.id,
                agentTitle: agent.title,
                taskName: task.task,
                status: .running,
                provider: agent.provider,
                effort: agent.effort
            )
            freshExecution.stageExecution = stageExec
            modelContext.insert(freshExecution)
            agentExec = freshExecution
        }
        // Proposal 003 — REQ-002: Populate Steward metadata on AgentExecution.
        agentExec.agentConfigHash = Self.computeAgentConfigHash(agent: agent)
        Self.applySkillMetadata(to: agentExec, from: agent)
        registerLiveExecution(agentExec, for: agent.id)

        do {
            try modelContext.save()
        } catch {
            RuntimeDiagnostics.log(
                "executeAgentTask durableAgentStartSaveFailed agentID=\(agent.id) stateID=\(state.id) error=\(error.localizedDescription)"
            )
            agentExec.status = .failed
            agentExec.completedAt = Date()
            agentExec.logSnippet =
                "Failed to durably persist agent start: \(error.localizedDescription)"
            applyTerminalExecutionTruth(
                to: agentExec,
                canonicalOutcome: .failedBeforeOutput,
                transportErrorKind: .unknown,
                providerStopReason: nil,
                outputPresence: .none,
                runtimeProvider: nil,
                runtimeModel: nil,
                rawErrorMessage: error.localizedDescription,
                rawFinishEvent: nil
            )
            unregisterLiveExecution(agentExec, for: agent.id)
            return false
        }
        if isCodeWriterImplementationTask(task, agent: agent) {
            persistImplementationHandoffCodeWriterStartStatus("running")
        }

        // Gather input artifacts
        let inputData: [String: Data]
        do {
            inputData = try await gatherExecutionInputs(for: task, agent: agent)
        } catch {
            agentExec.status = .failed
            agentExec.completedAt = Date()
            agentExec.logSnippet =
                "Source context preparation failed: \(error.localizedDescription)"
            applyTerminalExecutionTruth(
                to: agentExec,
                canonicalOutcome: .failedBeforeOutput,
                transportErrorKind: .unknown,
                providerStopReason: nil,
                outputPresence: .none,
                runtimeProvider: nil,
                runtimeModel: nil,
                rawErrorMessage: error.localizedDescription,
                rawFinishEvent: nil
            )
            unregisterLiveExecution(agentExec, for: agent.id)
            return false
        }
        agentExec.consumedInputArtifactNamesJSON = encodeArtifactNameList(
            Array(inputData.keys).sorted())
        agentExec.inputBindingsJSON = buildInputBindings(for: task)
        agentExec.resolvedBackendProfileID = agent.backendProfileID
        let inputArtifactPaths = gatherInputArtifactPaths(for: task)

        // Proposal 018: Record the exact owner tuple
        let ownerKey = InvocationOwnerKeyBuilder.build(
            runID: run.id,
            agentID: agent.id,
            stageLineageID: stageExec.lineageID ?? stageExec.stageID,
            taskName: task.task,
            ownerExecutionLineageID: agentExec.id
        )
        agentExec.invocationOwnerKey = ownerKey

        // Build execution context — Proposal 018: add lineage and owner IDs
        let handoffPacket = buildHandoffPacket(
            profileID: contextStrategyProfileID,
            profile: contextStrategyProfile,
            agent: agent,
            task: task,
            inputArtifacts: inputData,
            inputArtifactPaths: inputArtifactPaths
        )
        let primaryModelTier = preferredPrimaryModelTier(
            for: agent,
            task: task,
            stageExecution: stageExec,
            profile: contextStrategyProfile
        )
        let primaryExecutionBinding = strategyAdjustedBinding(
            for: agent,
            baseBinding: providerBindingsByAgentID[agent.id],
            modelTier: primaryModelTier
        )
        let primaryExecutionAgent = strategyAdjustedAgent(
            from: agent,
            binding: primaryExecutionBinding
        )
        applyStrategyExecutionMetadata(
            to: agentExec,
            handoffPacket: handoffPacket,
            inputArtifacts: inputData,
            profile: contextStrategyProfile,
            modelTierUsed: resolvedModelTierUsed(
                requestedTier: primaryModelTier,
                effectiveAgent: primaryExecutionAgent
            )
        )
        let execContext = ExecutionContext(
            workspace: currentWorkspace,
            projectRoot: preferredProjectRoot,
            stageID: state.id,
            stageLineageID: stageExec.lineageID,
            ownerExecutionLineageID: agentExec.id,
            iteration: stageExec.iteration,
            attemptNumber: stageExec.attemptNumber,
            inputArtifacts: inputData,
            inputArtifactPaths: inputArtifactPaths,
            variables: runtimeVariables,
            ideaBody: run.idea?.body ?? "",
            ideaAttachmentPath: run.idea?.attachmentPath,
            providerBinding: primaryExecutionBinding,
            catalog: catalog,
            contextStrategyProfileID: contextStrategyProfileID,
            strategyAssignmentMode: strategyAssignmentMode,
            contextStrategyProfile: contextStrategyProfile,
            handoffPacket: handoffPacket,
            agentAttemptNumber: agentExec.agentAttemptNumber,
            retryReason: agentExec.retryReason,
            supersedesAgentExecutionID: agentExec.supersedesAgentExecutionID
        )

        // Proposal 007 REQ-008 / REQ-011: Route release agents through ReleaseOpsCoordinator
        // for delivery-configured runs instead of the generic executor path.
        if let config = deliveryConfig,
            agent.id == "commit_and_push_to_github" || agent.id == "build_archive_and_push_connect"
        {
            return await executeReleaseAgentTask(
                agent: agent,
                agentExec: agentExec,
                stageExec: stageExec,
                state: state,
                deliveryConfig: config,
                inputData: inputData
            )
        }

        // Execute off-MainActor, marshal results back
        var result: AgentResult
        var finalModelTierUsed = resolvedModelTierUsed(
            requestedTier: primaryModelTier,
            effectiveAgent: primaryExecutionAgent
        )
        var escalationCount = 0
        var retryableEscalationCount = 0
        do {
            result = try await executor.execute(
                task: task, agent: primaryExecutionAgent, context: execContext)
            var attemptedCapacityFallbackModels = Set([primaryExecutionAgent.model.lowercased()])
            var currentFallbackBinding = primaryExecutionBinding

            while let fallbackBinding = capacityFallbackBinding(
                for: agent,
                attemptedBinding: currentFallbackBinding,
                result: result
            ) {
                let normalizedFallbackModel = fallbackBinding.model.lowercased()
                guard attemptedCapacityFallbackModels.insert(normalizedFallbackModel).inserted
                else {
                    break
                }

                let fallbackAgent = strategyAdjustedAgent(
                    from: agent,
                    binding: fallbackBinding
                )
                ForgeLogger.execution.info(
                    "Capacity fallback for agent '\(agent.id)': \(currentFallbackBinding?.model ?? primaryExecutionAgent.model) -> \(fallbackBinding.model)"
                )
                let fallbackContext = ExecutionContext(
                    workspace: currentWorkspace,
                    projectRoot: preferredProjectRoot,
                    stageID: state.id,
                    stageLineageID: stageExec.lineageID,
                    ownerExecutionLineageID: agentExec.id,
                    iteration: stageExec.iteration,
                    attemptNumber: stageExec.attemptNumber,
                    inputArtifacts: inputData,
                    inputArtifactPaths: inputArtifactPaths,
                    variables: runtimeVariables,
                    ideaBody: run.idea?.body ?? "",
                    ideaAttachmentPath: run.idea?.attachmentPath,
                    providerBinding: fallbackBinding,
                    catalog: catalog,
                    contextStrategyProfileID: contextStrategyProfileID,
                    strategyAssignmentMode: strategyAssignmentMode,
                    contextStrategyProfile: contextStrategyProfile,
                    handoffPacket: handoffPacket,
                    agentAttemptNumber: agentExec.agentAttemptNumber,
                    retryReason: agentExec.retryReason,
                    supersedesAgentExecutionID: agentExec.supersedesAgentExecutionID
                )
                result = try await executor.execute(
                    task: task,
                    agent: fallbackAgent,
                    context: fallbackContext
                )
                currentFallbackBinding = fallbackBinding
                finalModelTierUsed = resolvedModelTierUsed(
                    requestedTier: nil,
                    effectiveAgent: fallbackAgent
                )
            }

            if shouldEscalateStrategyExecution(
                result: result,
                profile: contextStrategyProfile
            )
                && shouldAttemptEscalatedExecution(
                    primaryModelTier: primaryModelTier,
                    profile: contextStrategyProfile
                )
            {
                let escalatedBinding = strategyAdjustedBinding(
                    for: agent,
                    baseBinding: providerBindingsByAgentID[agent.id],
                    modelTier: contextStrategyProfile?.escalationModelTier
                )
                let escalatedAgent = strategyAdjustedAgent(
                    from: agent,
                    binding: escalatedBinding
                )
                let escalatedContext = ExecutionContext(
                    workspace: currentWorkspace,
                    projectRoot: preferredProjectRoot,
                    stageID: state.id,
                    stageLineageID: stageExec.lineageID,
                    ownerExecutionLineageID: agentExec.id,
                    iteration: stageExec.iteration,
                    attemptNumber: stageExec.attemptNumber,
                    inputArtifacts: inputData,
                    inputArtifactPaths: inputArtifactPaths,
                    variables: runtimeVariables,
                    ideaBody: run.idea?.body ?? "",
                    ideaAttachmentPath: run.idea?.attachmentPath,
                    providerBinding: escalatedBinding,
                    catalog: catalog,
                    contextStrategyProfileID: contextStrategyProfileID,
                    strategyAssignmentMode: strategyAssignmentMode,
                    contextStrategyProfile: contextStrategyProfile,
                    handoffPacket: handoffPacket,
                    agentAttemptNumber: agentExec.agentAttemptNumber,
                    retryReason: agentExec.retryReason,
                    supersedesAgentExecutionID: agentExec.supersedesAgentExecutionID
                )
                result = try await executor.execute(
                    task: task,
                    agent: escalatedAgent,
                    context: escalatedContext
                )
                finalModelTierUsed = resolvedModelTierUsed(
                    requestedTier: contextStrategyProfile?.escalationModelTier,
                    effectiveAgent: escalatedAgent
                )
                escalationCount = 1
                retryableEscalationCount = 1
            }
        } catch {
            agentExec.status = .failed
            agentExec.completedAt = Date()
            agentExec.logSnippet = "Error: \(error.localizedDescription)"
            applyTerminalExecutionTruth(
                to: agentExec,
                canonicalOutcome: .failedBeforeOutput,
                transportErrorKind: .unknown,
                providerStopReason: nil,
                outputPresence: .none,
                runtimeProvider: nil,
                runtimeModel: nil,
                rawErrorMessage: error.localizedDescription,
                rawFinishEvent: nil
            )
            persistStageFailureEvidence(
                stageExec: stageExec,
                failedAgentExec: agentExec,
                validationFailure: nil,
                additionalEnvelopes: []
            )
            unregisterLiveExecution(agentExec, for: agent.id)
            return false
        }

        let automaticWatchdogRetryConsumed = hasConsumedAutomaticWatchdogRetry(
            for: task,
            agent: agent,
            currentExecution: agentExec,
            in: stageExec
        )
        if automaticWatchdogRetryConsumed, result.supervisionClassification != nil {
            result = result.markedAutomaticRetryConsumed()
        }

        // Marshal results back to MainActor
        agentExec.completedAt = Date()
        agentExec.costCents = result.costCents
        agentExec.resolvedModel = result.resolvedModel
        agentExec.configuredProviderID = result.configuredProviderID
        agentExec.adapterVersion = result.adapterVersion
        agentExec.providerReceiptJSON = encodeProviderReceipt(result.providerReceipt)

        // Proposal 026 ARCH-001: Persist actual runtime settlement fields.
        let settlementBinding = providerBindingsByAgentID[agent.id]
        agentExec.runtimeProfileID = settlementBinding?.runtimeProfileID
        agentExec.actualAdapterFamily = settlementBinding?.adapterFamily ?? "claude_agent_acp"
        agentExec.actualCapabilityClass =
            settlementBinding?.capabilityClass?.rawValue ?? "legacy_operator_grade"
        agentExec.logSnippet = mergedLogSnippet(
            existing: agentExec.logSnippet,
            result: result.logSnippet
        )
        finalizeStrategyExecutionMetadata(
            on: agentExec,
            modelTierUsed: finalModelTierUsed,
            escalationCount: escalationCount,
            retryableEscalationCount: retryableEscalationCount,
            lazyEvidenceHitCount: result.lazyEvidenceArtifactHits.count
        )
        // Proposal 018: Record session lineage and disposition (REQ-012 §5.3)
        agentExec.sessionLineageID = result.sessionLineageID
        agentExec.sessionGenerationID = result.sessionGenerationID
        agentExec.sessionReuseDisposition = result.sessionReuseDisposition
        agentExec.sessionReuseScope = agent.sessionReuseScope
        agentExec.sessionFamilyID = agent.sessionFamilyID
        // sessionResetReason is set during reset flow, not here
        if result.sessionReuseDisposition == .fresh_after_reset {
            agentExec.sessionResetReason = "Session was reset before this invocation"
        }
        // PROD-001: Persist structured session receipt fields for receipt/report consumption.
        // SessionReuseReceiptFields is the canonical receipt extension for session provenance.
        if let receiptFieldsData = try? JSONEncoder().encode(
            SessionReuseReceiptFields.from(execution: agentExec)
        ) {
            agentExec.compactionMetadataJSON = agentExec.compactionMetadataJSON ?? receiptFieldsData
        }

        applyExecutionTruth(from: result, to: agentExec)
        // Record sessionID from the executor (§6.1)
        if let sessionID = result.sessionID {
            agentExec.providerSessionID = sessionID
            agentExec.runtimeSessionID = sessionID
        }

        if result.succeeded {
            // Proposal 013 §6.2: Ordered persistence — raw outputs first, then validation, then settlement.
            do {
                // Step 1: Persist raw outputs BEFORE validation (§6.2 Rule 2)
                let (artifacts, rawEnvelopes) =
                    try ArtifactPersistenceOrderingPolicy.persistRawOutputs(
                        result: result,
                        agent: agent,
                        agentExecution: agentExec,
                        workspace: currentWorkspace,
                        stageID: state.id,
                        iteration: stageExec.iteration,
                        attemptNumber: stageExec.attemptNumber,
                        artifactManager: artifactManager,
                        catalog: catalog
                    )
                capturePersistedExecutionEvidence(from: artifacts, for: agentExec)

                // Step 2: Validate structured outputs AFTER raw persistence (§6.2 Rule 3)
                var envelopes = rawEnvelopes
                let contractOutputs = filteredContractOutputs(
                    from: result.outputs,
                    task: task,
                    agent: agent
                )
                let validationResults = ArtifactPersistenceOrderingPolicy.validatePersistedOutputs(
                    outputs: contractOutputs,
                    agent: agent,
                    catalog: catalog,
                    envelopes: &envelopes
                )

                // Persist output envelopes as evidence
                agentExec.outputEnvelopesJSON = try? JSONEncoder().encode(envelopes)

                // Proposal 018 REQ-010: Persist enriched session checkpoint AFTER validation
                // so that persistEnrichedCheckpoint reads real outputEnvelopesJSON as
                // the canonical lastValidatedAggregateStateJSON (§6.4).
                if let checkpoint = result.sessionCheckpoint {
                    try persistEnrichedCheckpoint(
                        checkpoint: checkpoint, artifacts: artifacts, agentExec: agentExec,
                        stageID: state.id, iteration: stageExec.iteration,
                        attemptNumber: stageExec.attemptNumber
                    )
                }

                // Step 3: Check for validation failures
                let failedResults = validationResults.values.filter {
                    $0.status == OutputValidationStatus.failed
                }
                if !failedResults.isEmpty {
                    // Build and persist validation failure record (§6.2 Rule 4)
                    let failureRecord = ArtifactPersistenceOrderingPolicy.buildFailureRecord(
                        validationResults: validationResults,
                        agent: agent,
                        stageID: state.id,
                        runID: run.id,
                        rawOutputExists: true,
                        receiptExists: agentExec.providerReceiptJSON != nil,
                        transcriptExists: hasPersistedTranscriptEvidence(for: agentExec),
                        catalog: catalog
                    )

                    if let failureRecord {
                        agentExec.validationFailureJSON = try? JSONEncoder().encode(failureRecord)
                        incrementContractFailureCount(on: agentExec)
                        _ = try ArtifactPersistenceOrderingPolicy.persistFailureEvidence(
                            failureRecord: failureRecord,
                            workspace: currentWorkspace,
                            stageID: state.id,
                            agentID: agent.id,
                            attemptNumber: stageExec.attemptNumber,
                            artifactManager: artifactManager
                        )
                    }

                    // Raw outputs are preserved even though validation failed
                    agentExec.status = .failed
                    applyTerminalExecutionTruth(
                        to: agentExec,
                        canonicalOutcome: .failedAfterOutputValidation,
                        transportErrorKind: agentExec.transportErrorKind,
                        providerStopReason: agentExec.providerStopReason,
                        outputPresence: .durableOutput,
                        runtimeProvider: agentExec.runtimeProvider,
                        runtimeModel: agentExec.runtimeModel,
                        rawErrorMessage: failedResults.compactMap(\.validationError).joined(
                            separator: "; "),
                        rawFinishEvent: nil
                    )
                    let validationMessages = failedResults.compactMap { $0.validationError }
                    agentExec.logSnippet =
                        "Output contract validation failed: \(validationMessages.joined(separator: "; "))"

                    persistStageFailureEvidence(
                        stageExec: stageExec,
                        failedAgentExec: agentExec,
                        validationFailure: failureRecord,
                        additionalEnvelopes: envelopes
                    )

                    unregisterLiveExecution(agentExec, for: agent.id)
                    return false
                }

                // Validation passed — extract fields for transition evaluation
                let validatedFields = try validateStructuredOutputs(
                    result.outputs,
                    for: task,
                    agent: agent
                )

                for artifact in artifacts {
                    recordArtifactForTransition(
                        artifact,
                        data: result.outputs[artifact.name],
                        validatedFields: validatedFields
                    )

                    if artifact.name.hasSuffix("_transcript.md") {
                        agentExec.transcriptArtifactPath = artifact.filePath
                        agentExec.transcriptPath = artifact.filePath
                    }
                }

                // Aggregate cost
                if let cost = result.costCents {
                    run.totalCostCents = (run.totalCostCents ?? 0) + cost
                }
                agentExec.status = .completed
                // Proposal 013 §8.2: Record compaction outcome truth
                updateCompactionOutcome(agentExec: agentExec, succeeded: true)
            } catch {
                agentExec.status = .failed
                agentExec.logSnippet =
                    "Output validation or persistence error: \(error.localizedDescription)"
                applyTerminalExecutionTruth(
                    to: agentExec,
                    canonicalOutcome: result.outputPresence == .durableOutput
                        ? .failedAfterOutputValidation : .failedBeforeOutput,
                    transportErrorKind: result.transportErrorKind,
                    providerStopReason: result.providerStopReason,
                    outputPresence: result.outputPresence,
                    runtimeProvider: agentExec.runtimeProvider,
                    runtimeModel: agentExec.runtimeModel,
                    rawErrorMessage: error.localizedDescription,
                    rawFinishEvent: result.outcomeEnvelope?.rawFinishEvent
                )
                persistStageFailureEvidence(
                    stageExec: stageExec,
                    failedAgentExec: agentExec,
                    validationFailure: nil,
                    additionalEnvelopes: []
                )
                // Proposal 013 §8.2: Record compaction outcome truth
                updateCompactionOutcome(agentExec: agentExec, succeeded: false)
                unregisterLiveExecution(agentExec, for: agent.id)
                return false
            }

            unregisterLiveExecution(agentExec, for: agent.id)
            return true
        } else {
            let shouldScheduleAutomaticRetry =
                result.supervisionClassification != nil && !automaticWatchdogRetryConsumed
            if !result.outputs.isEmpty {
                do {
                    let (artifacts, envelopes) =
                        try ArtifactPersistenceOrderingPolicy.persistRawOutputs(
                            result: result,
                            agent: agent,
                            agentExecution: agentExec,
                            workspace: currentWorkspace,
                            stageID: state.id,
                            iteration: stageExec.iteration,
                            attemptNumber: stageExec.attemptNumber,
                            artifactManager: artifactManager,
                            catalog: catalog
                        )
                    capturePersistedExecutionEvidence(from: artifacts, for: agentExec)
                    agentExec.outputEnvelopesJSON = try? JSONEncoder().encode(envelopes)

                    // Proposal 018 REQ-010: Persist enriched session checkpoint (non-success path).
                    if let checkpoint = result.sessionCheckpoint {
                        try? persistEnrichedCheckpoint(
                            checkpoint: checkpoint, artifacts: artifacts, agentExec: agentExec,
                            stageID: state.id, iteration: stageExec.iteration,
                            attemptNumber: stageExec.attemptNumber
                        )
                    }

                    if shouldTreatAfterOutputFailureAsRecoveredSuccess(result) {
                        var recoveredEnvelopes = envelopes
                        let contractOutputs = filteredContractOutputs(
                            from: result.outputs,
                            task: task,
                            agent: agent
                        )
                        let validationResults =
                            ArtifactPersistenceOrderingPolicy.validatePersistedOutputs(
                                outputs: contractOutputs,
                                agent: agent,
                                catalog: catalog,
                                envelopes: &recoveredEnvelopes
                            )
                        let failedResults = validationResults.values.filter {
                            $0.status == OutputValidationStatus.failed
                        }

                        if failedResults.isEmpty {
                            agentExec.outputEnvelopesJSON = try? JSONEncoder().encode(
                                recoveredEnvelopes)
                            let validatedFields = try validateStructuredOutputs(
                                result.outputs,
                                for: task,
                                agent: agent
                            )

                            for artifact in artifacts {
                                recordArtifactForTransition(
                                    artifact,
                                    data: result.outputs[artifact.name],
                                    validatedFields: validatedFields
                                )

                                if artifact.name.hasSuffix("_transcript.md") {
                                    agentExec.transcriptArtifactPath = artifact.filePath
                                    agentExec.transcriptPath = artifact.filePath
                                }
                            }

                            if let cost = result.costCents {
                                run.totalCostCents = (run.totalCostCents ?? 0) + cost
                            }

                            agentExec.status = .completed
                            agentExec.logSnippet = mergedLogSnippet(
                                existing: agentExec.logSnippet,
                                result:
                                    "Recovered after transport failure because durable outputs validated"
                            )
                            applyTerminalExecutionTruth(
                                to: agentExec,
                                canonicalOutcome: .completedWithTransportError,
                                supervisionClassification: agentExec.supervisionClassification
                                    ?? result.supervisionClassification,
                                transportErrorKind: agentExec.transportErrorKind
                                    ?? result.transportErrorKind,
                                providerStopReason: agentExec.providerStopReason
                                    ?? result.providerStopReason,
                                outputPresence: .durableOutput,
                                runtimeProvider: agentExec.runtimeProvider,
                                runtimeModel: agentExec.runtimeModel,
                                rawErrorMessage: agentExec.logSnippet,
                                rawFinishEvent: result.outcomeEnvelope?.rawFinishEvent
                            )
                            updateCompactionOutcome(agentExec: agentExec, succeeded: true)
                            unregisterLiveExecution(agentExec, for: agent.id)
                            return true
                        }
                    }

                    if !shouldScheduleAutomaticRetry {
                        persistStageFailureEvidence(
                            stageExec: stageExec,
                            failedAgentExec: agentExec,
                            validationFailure: nil,
                            additionalEnvelopes: envelopes
                        )
                    }
                } catch {
                    agentExec.logSnippet = mergedLogSnippet(
                        existing: agentExec.logSnippet,
                        result:
                            "Raw failure outputs could not be persisted: \(error.localizedDescription)"
                    )
                }
            } else if !shouldScheduleAutomaticRetry {
                persistStageFailureEvidence(
                    stageExec: stageExec,
                    failedAgentExec: agentExec,
                    validationFailure: nil,
                    additionalEnvelopes: []
                )
            }
            agentExec.status = .failed
            agentExec.logSnippet = result.errorMessage
            // Proposal 013 §8.2: Record compaction outcome truth
            updateCompactionOutcome(agentExec: agentExec, succeeded: false)
            unregisterLiveExecution(agentExec, for: agent.id)
            if shouldScheduleAutomaticRetry {
                do {
                    _ = try scheduleAutomaticWatchdogRetry(
                        run: run,
                        stageExec: stageExec,
                        failedAgentExec: agentExec
                    )
                    return await executeAgentTask(task, state: state, stageExec: stageExec)
                } catch {
                    agentExec.logSnippet = mergedLogSnippet(
                        existing: agentExec.logSnippet,
                        result:
                            "Automatic watchdog retry scheduling failed: \(error.localizedDescription)"
                    )
                    persistStageFailureEvidence(
                        stageExec: stageExec,
                        failedAgentExec: agentExec,
                        validationFailure: nil,
                        additionalEnvelopes: []
                    )
                }
            }
            return false
        }
    }

    private func capturePersistedExecutionEvidence(
        from artifacts: [Artifact], for agentExec: AgentExecution
    ) {
        for artifact in artifacts where artifact.name.hasSuffix("_transcript.md") {
            agentExec.transcriptArtifactPath = artifact.filePath
            agentExec.transcriptPath = artifact.filePath
        }
    }

    private func shouldTreatAfterOutputFailureAsRecoveredSuccess(_ result: AgentResult) -> Bool {
        guard result.outputPresence == .durableOutput else { return false }
        if ImplementationFailureArtifactSynthesizer.containsRecoverableArtifactSet(result.outputs) {
            return true
        }
        let canonicalOutcome =
            result.canonicalOutcome ?? (result.succeeded ? .completed : .failedBeforeOutput)
        switch canonicalOutcome {
        case .timedOutAfterOutput, .completedWithTransportError, .limitExhaustedAfterOutput:
            return true
        default:
            return false
        }
    }

    /// REQ-010: Enrich a checkpoint with real artifact UUIDs and validated aggregate state,
    /// then persist it. This is the single canonical path for checkpoint persistence,
    /// ensuring consistency across success and non-success flows.
    ///
    /// - `artifacts`: persisted Artifact objects from the current execution (may be empty on failure paths)
    /// - `agentExec`: the execution record; `outputEnvelopesJSON` is read as validated aggregate state
    /// - `result`: the agent result containing the raw checkpoint from the executor
    @discardableResult
    private func persistEnrichedCheckpoint(
        checkpoint: AgentSessionCheckpoint,
        artifacts: [Artifact],
        agentExec: AgentExecution,
        stageID: String,
        iteration: Int,
        attemptNumber: Int
    ) throws -> Artifact {
        let artifactIDs = artifacts.map(\.id)

        // Build validated aggregate state from output envelopes if available.
        // OutputEnvelopesJSON is the real validation truth persisted by the contract layer.
        let validatedState: Data? =
            agentExec.outputEnvelopesJSON ?? checkpoint.lastValidatedAggregateStateJSON

        let enrichedCheckpoint = AgentSessionCheckpoint(
            machineSummary: checkpoint.machineSummary,
            nextSteps: checkpoint.nextSteps,
            durableLearnings: checkpoint.durableLearnings,
            unresolvedBlockers: checkpoint.unresolvedBlockers,
            openDecisions: checkpoint.openDecisions,
            openQuestions: checkpoint.openQuestions,
            unresolvedConstraints: checkpoint.unresolvedConstraints,
            selectedArtifactReferences: artifactIDs.isEmpty
                ? checkpoint.selectedArtifactReferences : artifactIDs,
            lastValidatedAggregateStateJSON: validatedState,
            ownerAndBindingContextJSON: checkpoint.ownerAndBindingContextJSON,
            scopeContextJSON: checkpoint.scopeContextJSON,
            compactedConversationStateJSON: checkpoint.compactedConversationStateJSON
        )
        let checkpointArtifact = try artifactManager.persistSessionCheckpoint(
            checkpoint: enrichedCheckpoint,
            agentExecution: agentExec,
            workspace: currentWorkspace,
            stageID: stageID,
            iteration: iteration,
            attemptNumber: attemptNumber
        )
        agentExec.rehydratedFromCheckpointArtifactID = checkpointArtifact.id
        return checkpointArtifact
    }

    private func hasPersistedTranscriptEvidence(for agentExec: AgentExecution) -> Bool {
        agentExec.transcriptPath != nil
            || agentExec.transcriptArtifactPath != nil
            || agentExec.artifacts.contains(where: { $0.name.hasSuffix("_transcript.md") })
    }

    private func decodeOutputEnvelopes(from agentExec: AgentExecution) -> [StructuredOutputEnvelope]
    {
        guard let data = agentExec.outputEnvelopesJSON else { return [] }
        return (try? JSONDecoder().decode([StructuredOutputEnvelope].self, from: data)) ?? []
    }

    private func mergedOutputEnvelopes(
        for stageExec: StageExecution,
        additionalEnvelopes: [StructuredOutputEnvelope]
    ) -> [StructuredOutputEnvelope] {
        let stageEnvelopes = stageExec.agentExecutions.flatMap { decodeOutputEnvelopes(from: $0) }
        let all = stageEnvelopes + additionalEnvelopes
        var seen: Set<String> = []
        var merged: [StructuredOutputEnvelope] = []

        for envelope in all {
            let key = [
                envelope.outputName,
                envelope.agentID,
                envelope.stageID,
                envelope.rawPayloadChecksum ?? "no-checksum",
                envelope.sessionID ?? "no-session",
            ].joined(separator: "|")
            if seen.insert(key).inserted {
                merged.append(envelope)
            }
        }

        return merged
    }

    private func persistStageFailureEvidence(
        stageExec: StageExecution,
        failedAgentExec: AgentExecution,
        validationFailure: ValidationFailureRecord?,
        additionalEnvelopes: [StructuredOutputEnvelope]
    ) {
        let retryCoordinator = StageRetryCoordinator(modelContext: modelContext)
        let recoverySnapshot = retryCoordinator.narrowestRecoveryAction(
            for: run,
            failedStage: stageExec,
            failedAgent: failedAgentExec,
            validationFailure: validationFailure
        )
        stageExec.recoverySnapshotJSON = try? JSONEncoder().encode(recoverySnapshot)

        if let validationFailure {
            if failedAgentExec.validationFailureJSON == nil {
                failedAgentExec.validationFailureJSON = try? JSONEncoder().encode(validationFailure)
            }
            stageExec.validationFailureJSON = failedAgentExec.validationFailureJSON
        }

        let evidencePacket = FailedStageEvidenceBuilder.buildEvidencePacket(
            stageExecution: stageExec,
            failedAgent: failedAgentExec,
            validationFailure: validationFailure,
            outputEnvelopes: mergedOutputEnvelopes(
                for: stageExec,
                additionalEnvelopes: additionalEnvelopes
            ),
            recoverySnapshot: recoverySnapshot
        )
        stageExec.evidencePacketJSON = try? JSONEncoder().encode(evidencePacket)
    }

    // MARK: - Release Agent Execution (Proposal 007 REQ-008 / REQ-011)

    /// Route release agents through deterministic ReleaseOpsCoordinator services
    /// instead of the generic executor path. Persists receipts as artifacts and
    /// handles partial failure per §9.4.
    private func executeReleaseAgentTask(
        agent: ResolvedAgent,
        agentExec: AgentExecution,
        stageExec: StageExecution,
        state: ExecutableState,
        deliveryConfig: DeliveryConfiguration,
        inputData: [String: Data]
    ) async -> Bool {
        guard let worktreeRoot = currentWorkspace.worktreeRoot else {
            RuntimeDiagnostics.log(
                "executeReleaseAgentTask missingWorktree agentID=\(agent.id) stateID=\(state.id)")
            agentExec.status = .failed
            agentExec.completedAt = Date()
            agentExec.logSnippet =
                "Release agent requires a provisioned worktree but none is available."
            unregisterLiveExecution(agentExec, for: agent.id)
            stageExec.status = .failed
            return false
        }

        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]

        // Build commit message from approved_proposal artifact name
        let proposalName =
            producedArtifactNames.first(where: { $0.contains("proposal") }) ?? "approved_proposal"
        let commitMessage =
            "[\(deliveryConfig.repoIdentifier)] Apply \(proposalName) via Chainworks Forge"

        if agent.id == "commit_and_push_to_github" {
            let gitService = GitReleaseService()
            do {
                RuntimeDiagnostics.log(
                    "executeReleaseAgentTask begin agentID=\(agent.id) branch=\(deliveryConfig.targetBranch) worktree=\(worktreeRoot.path)"
                )
                let (manifest, receipt) = try await gitService.commitAndPush(
                    worktreeRoot: worktreeRoot,
                    targetBranch: deliveryConfig.targetBranch,
                    commitMessage: commitMessage
                )

                // Persist release_manifest and git_push_receipt as artifacts
                var outputs: [String: Data] = [:]
                if let manifestData = try? encoder.encode(manifest) {
                    outputs["release_manifest"] = manifestData
                }
                if let receiptData = try? encoder.encode(receipt) {
                    outputs["git_push_receipt"] = receiptData
                }

                let artifacts = try artifactManager.persistOutputs(
                    outputs: outputs,
                    agent: agent,
                    agentExecution: agentExec,
                    workspace: currentWorkspace,
                    stageID: state.id,
                    iteration: stageExec.iteration,
                    attemptNumber: stageExec.attemptNumber,
                    catalog: catalog
                )
                for artifact in artifacts {
                    producedArtifactNames.insert(artifact.name)
                }

                agentExec.status = .completed
                agentExec.completedAt = Date()
                agentExec.logSnippet =
                    "GitReleaseService: commit \(manifest.commitSHA.prefix(8)) pushed to \(manifest.branch)"
                RuntimeDiagnostics.log(
                    "executeReleaseAgentTask success agentID=\(agent.id) branch=\(manifest.branch)")
                unregisterLiveExecution(agentExec, for: agent.id)
                return true
            } catch {
                RuntimeDiagnostics.log(
                    "executeReleaseAgentTask failure agentID=\(agent.id) error=\(error.localizedDescription)"
                )
                // REQ-011: Persist any partial receipts and propagate failure
                let result = ReleaseOpsCoordinator.ReleaseResult(
                    gitManifest: nil,
                    gitReceipt: nil,
                    bundleManifest: nil,
                    uploadReceipt: nil,
                    succeeded: false,
                    failureStage: "commit_and_push",
                    failureReason: error.localizedDescription
                )
                persistDeliveryReceipt(
                    finalStateID: state.id,
                    provider: agent.provider,
                    model: agent.model,
                    effort: agent.effort,
                    releaseResult: result
                )
                agentExec.status = .failed
                agentExec.completedAt = Date()
                agentExec.logSnippet = "GitReleaseService failed: \(error.localizedDescription)"
                stageExec.status = .failed
                unregisterLiveExecution(agentExec, for: agent.id)
                return false
            }
        } else if agent.id == "build_archive_and_push_connect" {
            // Requires git_push_receipt and release_manifest from prior agent
            guard let receiptData = inputData["git_push_receipt"],
                let manifestData = inputData["release_manifest"]
            else {
                agentExec.status = .failed
                agentExec.completedAt = Date()
                agentExec.logSnippet =
                    "ConnectPublishService requires git_push_receipt and release_manifest inputs."
                stageExec.status = .failed
                unregisterLiveExecution(agentExec, for: agent.id)
                return false
            }

            let decoder = JSONDecoder()
            decoder.dateDecodingStrategy = .iso8601
            guard
                let gitReceipt = try? decoder.decode(
                    GitReleaseService.GitPushReceipt.self, from: receiptData),
                let releaseManifest = try? decoder.decode(
                    GitReleaseService.ReleaseManifest.self, from: manifestData)
            else {
                agentExec.status = .failed
                agentExec.completedAt = Date()
                agentExec.logSnippet = "ConnectPublishService received invalid release inputs."
                stageExec.status = .failed
                unregisterLiveExecution(agentExec, for: agent.id)
                return false
            }
            do {
                RuntimeDiagnostics.log(
                    "executeReleaseAgentTask begin agentID=\(agent.id) target=\(deliveryConfig.releaseTargetID) mode=\(deliveryConfig.releaseMode.rawValue)"
                )
                let publishService = ConnectPublishService()
                let (bundle, uploadReceipt) = try await publishService.buildArchiveAndUpload(
                    worktreeRoot: worktreeRoot,
                    gitPushReceipt: gitReceipt,
                    releaseManifest: releaseManifest,
                    releaseTargetID: deliveryConfig.releaseTargetID,
                    releaseMode: deliveryConfig.releaseMode
                )

                // Persist release_bundle_manifest and connect_upload_receipt as artifacts
                var outputs: [String: Data] = [:]
                if let bundleData = try? encoder.encode(bundle) {
                    outputs["release_bundle_manifest"] = bundleData
                }
                if let uploadData = try? encoder.encode(uploadReceipt) {
                    outputs["connect_upload_receipt"] = uploadData
                }

                let artifacts = try artifactManager.persistOutputs(
                    outputs: outputs,
                    agent: agent,
                    agentExecution: agentExec,
                    workspace: currentWorkspace,
                    stageID: state.id,
                    iteration: stageExec.iteration,
                    attemptNumber: 1,
                    catalog: catalog
                )
                for artifact in artifacts {
                    producedArtifactNames.insert(artifact.name)
                }

                let result = ReleaseOpsCoordinator.ReleaseResult(
                    gitManifest: releaseManifest,
                    gitReceipt: gitReceipt,
                    bundleManifest: bundle,
                    uploadReceipt: uploadReceipt,
                    succeeded: true,
                    failureStage: nil,
                    failureReason: nil
                )
                persistDeliveryReceipt(
                    finalStateID: state.id,
                    provider: agent.provider,
                    model: agent.model,
                    effort: agent.effort,
                    releaseResult: result
                )

                agentExec.status = .completed
                agentExec.completedAt = Date()
                agentExec.logSnippet =
                    "ConnectPublishService: bundle \(bundle.bundleVersion) uploaded to \(uploadReceipt.destination)"
                RuntimeDiagnostics.log(
                    "executeReleaseAgentTask success agentID=\(agent.id) destination=\(uploadReceipt.destination)"
                )
                unregisterLiveExecution(agentExec, for: agent.id)
                return true
            } catch {
                RuntimeDiagnostics.log(
                    "executeReleaseAgentTask failure agentID=\(agent.id) error=\(error.localizedDescription)"
                )
                // REQ-011: Persist partial receipts (git_push_receipt already persisted by prior agent)
                // and propagate failure so run becomes .blocked via existing failure handling
                let result = ReleaseOpsCoordinator.ReleaseResult(
                    gitManifest: releaseManifest,
                    gitReceipt: gitReceipt,
                    bundleManifest: nil,
                    uploadReceipt: nil,
                    succeeded: false,
                    failureStage: "build_archive_and_push",
                    failureReason: error.localizedDescription
                )
                persistDeliveryReceipt(
                    finalStateID: state.id,
                    provider: agent.provider,
                    model: agent.model,
                    effort: agent.effort,
                    releaseResult: result
                )
                agentExec.status = .failed
                agentExec.completedAt = Date()
                agentExec.logSnippet = "ConnectPublishService failed: \(error.localizedDescription)"
                stageExec.status = .failed
                unregisterLiveExecution(agentExec, for: agent.id)
                return false
            }
        }

        // Fallback: should not reach here for known release agents
        agentExec.status = .failed
        agentExec.completedAt = Date()
        agentExec.logSnippet = "Unknown release agent: \(agent.id)"
        unregisterLiveExecution(agentExec, for: agent.id)
        return false
    }

    /// Execute tasks in parallel with proper actor isolation.
    private func executeParallelTasks(
        _ tasks: [AgentTask],
        state: ExecutableState,
        stageExec: StageExecution
    ) async -> Bool {
        // Prepare agent executions for all tasks first
        var taskAgentPairs: [(task: AgentTask, agent: ResolvedAgent, agentExec: AgentExecution)] =
            []

        for task in tasks {
            guard let resolvedAgent = plan.agentBindings[task.agent] else { continue }
            let agent = effectiveAgent(from: resolvedAgent)

            if implementationHandoffUnavailable(
                for: task,
                agent: agent,
                state: state,
                stageExec: stageExec
            ) {
                return false
            }

            let agentExec: AgentExecution
            if let resumableAgent = resumableAgentExecution(for: task, agent: agent, in: stageExec)
            {
                switch resumableAgent.status {
                case .completed:
                    continue
                case .pending, .ready, .running:
                    agentExec = resumableAgent
                    agentExec.status = .running
                    agentExec.completedAt = nil
                case .failed, .cancelled, .skipped:
                    let freshExecution = AgentExecution(
                        agentID: agent.id,
                        agentTitle: agent.title,
                        taskName: task.task,
                        status: .running,
                        provider: agent.provider,
                        effort: agent.effort
                    )
                    freshExecution.stageExecution = stageExec
                    modelContext.insert(freshExecution)
                    agentExec = freshExecution
                }
            } else {
                let freshExecution = AgentExecution(
                    agentID: agent.id,
                    agentTitle: agent.title,
                    taskName: task.task,
                    status: .running,
                    provider: agent.provider,
                    effort: agent.effort
                )
                freshExecution.stageExecution = stageExec
                modelContext.insert(freshExecution)
                agentExec = freshExecution
            }
            // Proposal 003 — REQ-002: Populate Steward metadata on AgentExecution.
            agentExec.agentConfigHash = Self.computeAgentConfigHash(agent: agent)
            Self.applySkillMetadata(to: agentExec, from: agent)
            registerLiveExecution(agentExec, for: agent.id)

            taskAgentPairs.append((task, agent, agentExec))
        }

        do {
            try modelContext.save()
        } catch {
            RuntimeDiagnostics.log(
                "executeParallelTasks durableAgentStartSaveFailed stateID=\(state.id) error=\(error.localizedDescription)"
            )
            let now = Date()
            for pair in taskAgentPairs {
                pair.agentExec.status = .failed
                pair.agentExec.completedAt = pair.agentExec.completedAt ?? now
                pair.agentExec.logSnippet =
                    "Failed to durably persist agent start: \(error.localizedDescription)"
                applyTerminalExecutionTruth(
                    to: pair.agentExec,
                    canonicalOutcome: .failedBeforeOutput,
                    transportErrorKind: .unknown,
                    providerStopReason: nil,
                    outputPresence: .none,
                    runtimeProvider: nil,
                    runtimeModel: nil,
                    rawErrorMessage: error.localizedDescription,
                    rawFinishEvent: nil
                )
                unregisterLiveExecution(pair.agentExec, for: pair.agent.id)
            }
            return false
        }

        // Execute all in parallel
        let results = await withTaskGroup(of: (Int, AgentResult?).self) { group in
            for (index, pair) in taskAgentPairs.enumerated() {
                let gatheredInputs: [String: Data]
                do {
                    gatheredInputs = try await gatherExecutionInputs(
                        for: pair.task, agent: pair.agent)
                } catch {
                    pair.agentExec.consumedInputArtifactNamesJSON = encodeArtifactNameList([])
                    pair.agentExec.inputBindingsJSON = buildInputBindings(for: pair.task)
                    let preparationFailure = AgentResult(
                        outputs: [:],
                        logSnippet: nil,
                        costCents: nil,
                        succeeded: false,
                        errorMessage:
                            "Source context preparation failed: \(error.localizedDescription)",
                        sessionID: nil,
                        durationSeconds: 0,
                        providerReceipt: nil,
                        resolvedModel: nil,
                        configuredProviderID: nil,
                        adapterVersion: nil
                    )
                    group.addTask {
                        (
                            index,
                            preparationFailure
                        )
                    }
                    continue
                }
                let task = pair.task
                let agent = pair.agent
                let inputArtifactPaths = gatherInputArtifactPaths(for: task)

                // Proposal 018: Record the exact owner tuple
                let ownerKey = InvocationOwnerKeyBuilder.build(
                    runID: run.id,
                    agentID: agent.id,
                    stageLineageID: stageExec.lineageID ?? stageExec.stageID,
                    taskName: task.task,
                    ownerExecutionLineageID: pair.agentExec.id
                )
                pair.agentExec.invocationOwnerKey = ownerKey

                // Build execution context — Proposal 018: add lineage and owner IDs
                let handoffPacket = buildHandoffPacket(
                    profileID: contextStrategyProfileID,
                    profile: contextStrategyProfile,
                    agent: agent,
                    task: task,
                    inputArtifacts: gatheredInputs,
                    inputArtifactPaths: inputArtifactPaths
                )
                let primaryModelTier = self.preferredPrimaryModelTier(
                    for: agent,
                    task: task,
                    stageExecution: stageExec,
                    profile: contextStrategyProfile
                )
                let primaryExecutionBinding = strategyAdjustedBinding(
                    for: agent,
                    baseBinding: providerBindingsByAgentID[agent.id],
                    modelTier: primaryModelTier
                )
                let primaryExecutionAgent = strategyAdjustedAgent(
                    from: agent,
                    binding: primaryExecutionBinding
                )
                self.applyStrategyExecutionMetadata(
                    to: pair.agentExec,
                    handoffPacket: handoffPacket,
                    inputArtifacts: gatheredInputs,
                    profile: self.contextStrategyProfile,
                    modelTierUsed: resolvedModelTierUsed(
                        requestedTier: primaryModelTier,
                        effectiveAgent: primaryExecutionAgent
                    )
                )
                let execContext = ExecutionContext(
                    workspace: currentWorkspace,
                    projectRoot: preferredProjectRoot,
                    stageID: state.id,
                    stageLineageID: stageExec.lineageID,
                    ownerExecutionLineageID: pair.agentExec.id,
                    iteration: stageExec.iteration,
                    attemptNumber: stageExec.attemptNumber,
                    inputArtifacts: gatheredInputs,
                    inputArtifactPaths: inputArtifactPaths,
                    variables: runtimeVariables,
                    ideaBody: run.idea?.body ?? "",
                    ideaAttachmentPath: run.idea?.attachmentPath,
                    providerBinding: primaryExecutionBinding,
                    catalog: catalog,
                    contextStrategyProfileID: contextStrategyProfileID,
                    strategyAssignmentMode: strategyAssignmentMode,
                    contextStrategyProfile: contextStrategyProfile,
                    handoffPacket: handoffPacket,
                    agentAttemptNumber: pair.agentExec.agentAttemptNumber,
                    retryReason: pair.agentExec.retryReason,
                    supersedesAgentExecutionID: pair.agentExec.supersedesAgentExecutionID
                )
                let executor = self.executor
                pair.agentExec.consumedInputArtifactNamesJSON = encodeArtifactNameList(
                    Array(gatheredInputs.keys).sorted())
                pair.agentExec.inputBindingsJSON = buildInputBindings(for: pair.task)

                group.addTask {
                    do {
                        let result = try await executor.execute(
                            task: task,
                            agent: primaryExecutionAgent,
                            context: execContext
                        )
                        return (index, result)
                    } catch {
                        return (index, nil)
                    }
                }
            }

            var collected: [(Int, AgentResult?)] = []
            for await result in group {
                collected.append(result)
            }
            return collected.sorted { $0.0 < $1.0 }
        }

        // Marshal all results back on MainActor
        var allSucceeded = true
        var scheduledAutomaticRetry = false

        for (index, optResult) in results {
            let pair = taskAgentPairs[index]
            let agentExec = pair.agentExec
            let agent = pair.agent
            var normalizedResult = optResult
            agentExec.resolvedBackendProfileID = agent.backendProfileID
            let automaticWatchdogRetryConsumed = hasConsumedAutomaticWatchdogRetry(
                for: pair.task,
                agent: agent,
                currentExecution: agentExec,
                in: stageExec
            )

            agentExec.completedAt = Date()

            if var result = optResult {
                let primaryModelTier = preferredPrimaryModelTier(
                    for: agent,
                    task: pair.task,
                    stageExecution: stageExec,
                    profile: contextStrategyProfile
                )
                var currentFallbackBinding = strategyAdjustedBinding(
                    for: agent,
                    baseBinding: providerBindingsByAgentID[agent.id],
                    modelTier: primaryModelTier
                )
                var attemptedCapacityFallbackModels = Set([
                    (currentFallbackBinding?.model ?? agent.model).lowercased()
                ])

                while let fallbackBinding = capacityFallbackBinding(
                    for: agent,
                    attemptedBinding: currentFallbackBinding,
                    result: result
                ) {
                    let normalizedFallbackModel = fallbackBinding.model.lowercased()
                    guard attemptedCapacityFallbackModels.insert(normalizedFallbackModel).inserted
                    else {
                        break
                    }

                    let gatheredInputs: [String: Data]
                    do {
                        gatheredInputs = try await gatherExecutionInputs(
                            for: pair.task, agent: agent)
                    } catch {
                        result = AgentResult(
                            outputs: [:],
                            logSnippet: nil,
                            costCents: nil,
                            succeeded: false,
                            errorMessage:
                                "Source context preparation failed: \(error.localizedDescription)",
                            sessionID: nil,
                            durationSeconds: 0,
                            providerReceipt: nil,
                            resolvedModel: nil,
                            configuredProviderID: nil,
                            adapterVersion: nil
                        )
                        break
                    }

                    let inputArtifactPaths = gatherInputArtifactPaths(for: pair.task)
                    let handoffPacket = buildHandoffPacket(
                        profileID: contextStrategyProfileID,
                        profile: contextStrategyProfile,
                        agent: agent,
                        task: pair.task,
                        inputArtifacts: gatheredInputs,
                        inputArtifactPaths: inputArtifactPaths
                    )
                    let fallbackAgent = strategyAdjustedAgent(
                        from: agent,
                        binding: fallbackBinding
                    )
                    ForgeLogger.execution.info(
                        "Capacity fallback for agent '\(agent.id)': \(currentFallbackBinding?.model ?? agent.model) -> \(fallbackBinding.model)"
                    )
                    let fallbackContext = ExecutionContext(
                        workspace: currentWorkspace,
                        projectRoot: preferredProjectRoot,
                        stageID: state.id,
                        stageLineageID: stageExec.lineageID,
                        ownerExecutionLineageID: agentExec.id,
                        iteration: stageExec.iteration,
                        attemptNumber: stageExec.attemptNumber,
                        inputArtifacts: gatheredInputs,
                        inputArtifactPaths: inputArtifactPaths,
                        variables: runtimeVariables,
                        ideaBody: run.idea?.body ?? "",
                        ideaAttachmentPath: run.idea?.attachmentPath,
                        providerBinding: fallbackBinding,
                        catalog: catalog,
                        contextStrategyProfileID: contextStrategyProfileID,
                        strategyAssignmentMode: strategyAssignmentMode,
                        contextStrategyProfile: contextStrategyProfile,
                        handoffPacket: handoffPacket,
                        agentAttemptNumber: agentExec.agentAttemptNumber,
                        retryReason: agentExec.retryReason,
                        supersedesAgentExecutionID: agentExec.supersedesAgentExecutionID
                    )
                    do {
                        result = try await executor.execute(
                            task: pair.task,
                            agent: fallbackAgent,
                            context: fallbackContext
                        )
                    } catch {
                        result = AgentResult(
                            outputs: [:],
                            logSnippet: nil,
                            costCents: nil,
                            succeeded: false,
                            errorMessage: error.localizedDescription,
                            sessionID: nil,
                            durationSeconds: 0,
                            providerReceipt: nil,
                            resolvedModel: fallbackAgent.model,
                            configuredProviderID: fallbackBinding.configuredProviderID,
                            adapterVersion: fallbackBinding.adapterVersion,
                            canonicalOutcome: .failedBeforeOutput,
                            sessionReuseDisposition: .fresh,
                            transportErrorKind: .unknown,
                            providerStopReason: nil,
                            outputPresence: .none,
                            runtimeProvider: fallbackBinding.providerIdentifier,
                            runtimeModel: fallbackAgent.model
                        )
                    }
                    currentFallbackBinding = fallbackBinding
                }

                if automaticWatchdogRetryConsumed, result.supervisionClassification != nil {
                    result = result.markedAutomaticRetryConsumed()
                }
                normalizedResult = result
                agentExec.costCents = result.costCents
                agentExec.resolvedModel = result.resolvedModel
                agentExec.configuredProviderID = result.configuredProviderID
                agentExec.adapterVersion = result.adapterVersion
                agentExec.providerReceiptJSON = encodeProviderReceipt(result.providerReceipt)
                agentExec.sessionLineageID = result.sessionLineageID
                agentExec.sessionGenerationID = result.sessionGenerationID
                agentExec.sessionReuseDisposition = result.sessionReuseDisposition

                // Proposal 026 ARCH-001: Persist actual runtime settlement fields.
                let settlementBinding = providerBindingsByAgentID[agent.id]
                agentExec.runtimeProfileID = settlementBinding?.runtimeProfileID
                agentExec.actualAdapterFamily =
                    settlementBinding?.adapterFamily ?? "claude_agent_acp"
                agentExec.actualCapabilityClass =
                    settlementBinding?.capabilityClass?.rawValue ?? "legacy_operator_grade"

                applyExecutionTruth(from: result, to: agentExec)
                if let sessionID = result.sessionID {
                    agentExec.providerSessionID = sessionID
                    agentExec.runtimeSessionID = sessionID
                }
                agentExec.logSnippet = mergedLogSnippet(
                    existing: agentExec.logSnippet,
                    result: result.logSnippet
                )
                finalizeStrategyExecutionMetadata(
                    on: agentExec,
                    modelTierUsed: agentExec.modelTierUsed ?? self.contextStrategyProfile?
                        .defaultModelTier
                        ?? "bound_runtime",
                    escalationCount: 0,
                    retryableEscalationCount: 0,
                    lazyEvidenceHitCount: result.lazyEvidenceArtifactHits.count
                )
            }

            guard let result = normalizedResult else {
                agentExec.status = .failed
                agentExec.logSnippet = "Execution failed"
                applyTerminalExecutionTruth(
                    to: agentExec,
                    canonicalOutcome: .failedBeforeOutput,
                    transportErrorKind: .unknown,
                    providerStopReason: nil,
                    outputPresence: .none,
                    runtimeProvider: nil,
                    runtimeModel: nil,
                    rawErrorMessage: agentExec.logSnippet,
                    rawFinishEvent: nil
                )
                persistStageFailureEvidence(
                    stageExec: stageExec,
                    failedAgentExec: agentExec,
                    validationFailure: nil,
                    additionalEnvelopes: []
                )
                updateCompactionOutcome(agentExec: agentExec, succeeded: false)
                unregisterLiveExecution(agentExec, for: agent.id)
                allSucceeded = false
                continue
            }

            guard result.succeeded else {
                let shouldScheduleAutomaticRetry =
                    result.supervisionClassification != nil && !automaticWatchdogRetryConsumed
                if !result.outputs.isEmpty {
                    do {
                        let (artifacts, envelopes) =
                            try ArtifactPersistenceOrderingPolicy.persistRawOutputs(
                                result: result,
                                agent: agent,
                                agentExecution: agentExec,
                                workspace: currentWorkspace,
                                stageID: state.id,
                                iteration: stageExec.iteration,
                                attemptNumber: stageExec.attemptNumber,
                                artifactManager: artifactManager,
                                catalog: catalog
                            )
                        capturePersistedExecutionEvidence(from: artifacts, for: agentExec)
                        agentExec.outputEnvelopesJSON = try? JSONEncoder().encode(envelopes)

                        if let checkpoint = result.sessionCheckpoint {
                            try? persistEnrichedCheckpoint(
                                checkpoint: checkpoint,
                                artifacts: artifacts,
                                agentExec: agentExec,
                                stageID: state.id,
                                iteration: stageExec.iteration,
                                attemptNumber: stageExec.attemptNumber
                            )
                        }

                        if shouldTreatAfterOutputFailureAsRecoveredSuccess(result) {
                            var recoveredEnvelopes = envelopes
                            let contractOutputs = filteredContractOutputs(
                                from: result.outputs,
                                task: pair.task,
                                agent: agent
                            )
                            let validationResults =
                                ArtifactPersistenceOrderingPolicy.validatePersistedOutputs(
                                    outputs: contractOutputs,
                                    agent: agent,
                                    catalog: catalog,
                                    envelopes: &recoveredEnvelopes
                                )
                            let failedResults = validationResults.values.filter {
                                $0.status == OutputValidationStatus.failed
                            }

                            if failedResults.isEmpty {
                                agentExec.outputEnvelopesJSON = try? JSONEncoder().encode(
                                    recoveredEnvelopes)
                                let validatedFields = try validateStructuredOutputs(
                                    result.outputs,
                                    for: pair.task,
                                    agent: agent
                                )

                                for artifact in artifacts {
                                    recordArtifactForTransition(
                                        artifact,
                                        data: result.outputs[artifact.name],
                                        validatedFields: validatedFields
                                    )

                                    if artifact.name.hasSuffix("_transcript.md") {
                                        agentExec.transcriptArtifactPath = artifact.filePath
                                        agentExec.transcriptPath = artifact.filePath
                                    }
                                }

                                if let cost = result.costCents {
                                    run.totalCostCents = (run.totalCostCents ?? 0) + cost
                                }

                                agentExec.status = .completed
                                agentExec.logSnippet = mergedLogSnippet(
                                    existing: agentExec.logSnippet,
                                    result:
                                        "Recovered after transport failure because durable outputs validated"
                                )
                                applyTerminalExecutionTruth(
                                    to: agentExec,
                                    canonicalOutcome: .completedWithTransportError,
                                    supervisionClassification: agentExec.supervisionClassification
                                        ?? result.supervisionClassification,
                                    transportErrorKind: agentExec.transportErrorKind
                                        ?? result.transportErrorKind,
                                    providerStopReason: agentExec.providerStopReason
                                        ?? result.providerStopReason,
                                    outputPresence: .durableOutput,
                                    runtimeProvider: agentExec.runtimeProvider,
                                    runtimeModel: agentExec.runtimeModel,
                                    rawErrorMessage: agentExec.logSnippet,
                                    rawFinishEvent: result.outcomeEnvelope?.rawFinishEvent
                                )
                                updateCompactionOutcome(agentExec: agentExec, succeeded: true)
                                unregisterLiveExecution(agentExec, for: agent.id)
                                continue
                            }
                        }

                        if !shouldScheduleAutomaticRetry {
                            persistStageFailureEvidence(
                                stageExec: stageExec,
                                failedAgentExec: agentExec,
                                validationFailure: nil,
                                additionalEnvelopes: envelopes
                            )
                        }
                    } catch {
                        agentExec.logSnippet = mergedLogSnippet(
                            existing: agentExec.logSnippet,
                            result:
                                "Raw failure outputs could not be persisted: \(error.localizedDescription)"
                        )
                    }
                } else if !shouldScheduleAutomaticRetry {
                    persistStageFailureEvidence(
                        stageExec: stageExec,
                        failedAgentExec: agentExec,
                        validationFailure: nil,
                        additionalEnvelopes: []
                    )
                }

                agentExec.status = .failed
                agentExec.logSnippet = result.errorMessage
                updateCompactionOutcome(agentExec: agentExec, succeeded: false)
                unregisterLiveExecution(agentExec, for: agent.id)
                if shouldScheduleAutomaticRetry {
                    do {
                        _ = try scheduleAutomaticWatchdogRetry(
                            run: run,
                            stageExec: stageExec,
                            failedAgentExec: agentExec
                        )
                        scheduledAutomaticRetry = true
                        continue
                    } catch {
                        agentExec.logSnippet = mergedLogSnippet(
                            existing: agentExec.logSnippet,
                            result:
                                "Automatic watchdog retry scheduling failed: \(error.localizedDescription)"
                        )
                        persistStageFailureEvidence(
                            stageExec: stageExec,
                            failedAgentExec: agentExec,
                            validationFailure: nil,
                            additionalEnvelopes: []
                        )
                    }
                }
                allSucceeded = false
                continue
            }

            do {
                let (artifacts, rawEnvelopes) =
                    try ArtifactPersistenceOrderingPolicy.persistRawOutputs(
                        result: result,
                        agent: agent,
                        agentExecution: agentExec,
                        workspace: currentWorkspace,
                        stageID: state.id,
                        iteration: stageExec.iteration,
                        attemptNumber: stageExec.attemptNumber,
                        artifactManager: artifactManager,
                        catalog: catalog
                    )
                capturePersistedExecutionEvidence(from: artifacts, for: agentExec)

                var envelopes = rawEnvelopes
                let contractOutputs = filteredContractOutputs(
                    from: result.outputs,
                    task: pair.task,
                    agent: agent
                )
                let validationResults = ArtifactPersistenceOrderingPolicy.validatePersistedOutputs(
                    outputs: contractOutputs,
                    agent: agent,
                    catalog: catalog,
                    envelopes: &envelopes
                )
                agentExec.outputEnvelopesJSON = try? JSONEncoder().encode(envelopes)

                if let checkpoint = result.sessionCheckpoint {
                    try persistEnrichedCheckpoint(
                        checkpoint: checkpoint,
                        artifacts: artifacts,
                        agentExec: agentExec,
                        stageID: state.id,
                        iteration: stageExec.iteration,
                        attemptNumber: stageExec.attemptNumber
                    )
                }

                let failedResults = validationResults.values.filter {
                    $0.status == OutputValidationStatus.failed
                }
                if !failedResults.isEmpty {
                    let failureRecord = ArtifactPersistenceOrderingPolicy.buildFailureRecord(
                        validationResults: validationResults,
                        agent: agent,
                        stageID: state.id,
                        runID: run.id,
                        rawOutputExists: true,
                        receiptExists: agentExec.providerReceiptJSON != nil,
                        transcriptExists: hasPersistedTranscriptEvidence(for: agentExec),
                        catalog: catalog
                    )

                    if let failureRecord {
                        agentExec.validationFailureJSON = try? JSONEncoder().encode(failureRecord)
                        incrementContractFailureCount(on: agentExec)
                        _ = try ArtifactPersistenceOrderingPolicy.persistFailureEvidence(
                            failureRecord: failureRecord,
                            workspace: currentWorkspace,
                            stageID: state.id,
                            agentID: agent.id,
                            attemptNumber: stageExec.attemptNumber,
                            artifactManager: artifactManager
                        )
                    }

                    agentExec.status = .failed
                    applyTerminalExecutionTruth(
                        to: agentExec,
                        canonicalOutcome: .failedAfterOutputValidation,
                        transportErrorKind: agentExec.transportErrorKind,
                        providerStopReason: agentExec.providerStopReason,
                        outputPresence: .durableOutput,
                        runtimeProvider: agentExec.runtimeProvider,
                        runtimeModel: agentExec.runtimeModel,
                        rawErrorMessage: failedResults.compactMap(\.validationError).joined(
                            separator: "; "),
                        rawFinishEvent: nil
                    )
                    let validationMessages = failedResults.compactMap { $0.validationError }
                    agentExec.logSnippet =
                        "Output contract validation failed: \(validationMessages.joined(separator: "; "))"

                    persistStageFailureEvidence(
                        stageExec: stageExec,
                        failedAgentExec: agentExec,
                        validationFailure: failureRecord,
                        additionalEnvelopes: envelopes
                    )

                    updateCompactionOutcome(agentExec: agentExec, succeeded: false)
                    unregisterLiveExecution(agentExec, for: agent.id)
                    allSucceeded = false
                    continue
                }

                let validatedFields = try validateStructuredOutputs(
                    result.outputs,
                    for: pair.task,
                    agent: agent
                )

                for artifact in artifacts {
                    recordArtifactForTransition(
                        artifact,
                        data: result.outputs[artifact.name],
                        validatedFields: validatedFields
                    )

                    if artifact.name.hasSuffix("_transcript.md") {
                        agentExec.transcriptArtifactPath = artifact.filePath
                        agentExec.transcriptPath = artifact.filePath
                    }
                }

                if let cost = result.costCents {
                    run.totalCostCents = (run.totalCostCents ?? 0) + cost
                }
                agentExec.status = .completed
                updateCompactionOutcome(agentExec: agentExec, succeeded: true)
            } catch {
                agentExec.status = .failed
                agentExec.logSnippet =
                    "Output validation or persistence error: \(error.localizedDescription)"
                applyTerminalExecutionTruth(
                    to: agentExec,
                    canonicalOutcome: result.outputPresence == .durableOutput
                        ? .failedAfterOutputValidation : .failedBeforeOutput,
                    transportErrorKind: result.transportErrorKind,
                    providerStopReason: result.providerStopReason,
                    outputPresence: result.outputPresence,
                    runtimeProvider: agentExec.runtimeProvider,
                    runtimeModel: agentExec.runtimeModel,
                    rawErrorMessage: error.localizedDescription,
                    rawFinishEvent: result.outcomeEnvelope?.rawFinishEvent
                )
                persistStageFailureEvidence(
                    stageExec: stageExec,
                    failedAgentExec: agentExec,
                    validationFailure: nil,
                    additionalEnvelopes: []
                )
                updateCompactionOutcome(agentExec: agentExec, succeeded: false)
                unregisterLiveExecution(agentExec, for: agent.id)
                allSucceeded = false
                continue
            }

            unregisterLiveExecution(agentExec, for: agent.id)
        }

        if scheduledAutomaticRetry {
            return await executeParallelTasks(tasks, state: state, stageExec: stageExec)
        }

        return allSucceeded
    }

    private func hasConsumedAutomaticWatchdogRetry(
        for task: AgentTask,
        agent: ResolvedAgent,
        currentExecution: AgentExecution,
        in stageExec: StageExecution
    ) -> Bool {
        if currentExecution.retryReason == Self.automaticWatchdogRetryReason {
            return true
        }

        return stageExec.agentExecutions.contains {
            $0.id != currentExecution.id && $0.agentID == agent.id && $0.taskName == task.task
                && $0.retryReason == Self.automaticWatchdogRetryReason
        }
    }

    @discardableResult
    private func scheduleAutomaticWatchdogRetry(
        run: Run,
        stageExec: StageExecution,
        failedAgentExec: AgentExecution
    ) throws -> AgentExecution {
        let retryCoordinator = StageRetryCoordinator(modelContext: modelContext)
        let retryExec = try retryCoordinator.retryFailedAgent(
            run: run,
            stage: stageExec,
            failedAgent: failedAgentExec,
            retryReason: Self.automaticWatchdogRetryReason
        )
        try modelContext.save()
        return retryExec
    }

    // MARK: - Delivery Worktree Provisioning (Proposal 007 — ARCH-067)

    /// Provisions a dedicated worktree when entering the implementation_started state
    /// for a repo-backed delivery run, if not already provisioned.
    private func provisionWorktreeIfNeeded(for state: ExecutableState) async throws {
        // Only provision for implementation-start states
        guard state.id.contains("implementation_started") || state.id.contains("state_7") else {
            return
        }

        // Only provision for delivery-configured runs
        guard let config = deliveryConfig else { return }

        // Skip if already provisioned
        guard run.worktreeRoot == nil else { return }

        let ideaSlug = run.idea?.title ?? "untitled"
        let runShortID = String(run.id.uuidString.prefix(6)).lowercased()

        let provisioner = WorktreeProvisioner()
        let result = try await provisioner.provision(
            repoIdentifier: config.repoIdentifier,
            repoRoot: config.repoRoot,
            baseBranch: config.baseBranch,
            targetBranch: config.targetBranch,
            worktreeBasePath: config.worktreeBasePath,
            ideaSlug: ideaSlug,
            runShortID: runShortID
        )

        // Persist provisioning result on the Run
        run.worktreeRoot = result.worktreeRoot.path
        run.baseRevision = result.baseRevision
        RuntimeDiagnostics.log(
            "worktreeProvisioned stateID=\(state.id) branch=\(result.branchName) root=\(result.worktreeRoot.path)"
        )
    }

    // MARK: - Helpers

    private func currentIteration(for stageID: String) -> Int {
        guard !run.isDeleted, run.modelContext != nil else { return 1 }
        let existing = run.stageExecutions.filter { $0.stageID == stageID }
        return (existing.map(\.iteration).max() ?? 0) + 1
    }

    private func scheduledStageSelection(for stageID: String) -> (
        iteration: Int, attemptNumber: Int, execution: StageExecution?
    )? {
        guard
            let cursor = run.transitionCursor,
            cursor.nextScheduledStateID == stageID
        else {
            return nil
        }

        let iteration = cursor.nextScheduledIteration ?? currentIteration(for: stageID)
        let attemptNumber = cursor.nextScheduledAttemptNumber ?? 1
        let candidates = run.stageExecutions.filter {
            $0.stageID == stageID && $0.iteration == iteration && $0.attemptNumber == attemptNumber
        }

        if let scheduledID = cursor.scheduledStageExecutionID,
            let exact = candidates.first(where: { $0.id == scheduledID })
        {
            return (iteration, attemptNumber, exact)
        }

        let execution = candidates.sorted {
            if $0.startedAt != $1.startedAt { return $0.startedAt < $1.startedAt }
            return $0.id.uuidString < $1.id.uuidString
        }.last
        return (iteration, attemptNumber, execution)
    }

    private func resumableStageExecution(for stageID: String) -> StageExecution? {
        run.stageExecutions
            .filter { $0.stageID == stageID && ($0.status == .running || $0.status == .ready) }
            .sorted {
                if $0.iteration != $1.iteration { return $0.iteration < $1.iteration }
                if $0.attemptNumber != $1.attemptNumber {
                    return $0.attemptNumber < $1.attemptNumber
                }
                return $0.startedAt < $1.startedAt
            }
            .last
    }

    private func resumableAgentExecution(
        for task: AgentTask,
        agent: ResolvedAgent,
        in stageExec: StageExecution
    ) -> AgentExecution? {
        stageExec.agentExecutions
            .filter { $0.agentID == agent.id && $0.taskName == task.task }
            .sorted {
                let lhsAttempt = $0.agentAttemptNumber ?? 1
                let rhsAttempt = $1.agentAttemptNumber ?? 1
                if lhsAttempt != rhsAttempt { return lhsAttempt < rhsAttempt }
                return $0.startedAt < $1.startedAt
            }
            .last
    }

    private var currentWorkspace: RunWorkspace {
        RunWorkspace(
            runID: workspace.runID,
            workspaceRoot: workspace.workspaceRoot,
            artifactRoot: workspace.artifactRoot,
            worktreeRoot: run.worktreeRoot.flatMap { URL(fileURLWithPath: $0) }
        )
    }

    private func buildHandoffPacket(
        profileID: String?,
        profile: ContextStrategyProfile?,
        agent: ResolvedAgent,
        task: AgentTask,
        inputArtifacts: [String: Data],
        inputArtifactPaths: [String: String]
    ) -> HandoffPacket? {
        guard let profileID,
            let profile
        else {
            return nil
        }

        let previewContext = ExecutionContext(
            workspace: currentWorkspace,
            projectRoot: preferredProjectRoot,
            stageID: currentStateID,
            ownerExecutionLineageID: UUID(),
            iteration: currentIteration(for: currentStateID),
            attemptNumber: 1,
            inputArtifacts: inputArtifacts,
            inputArtifactPaths: inputArtifactPaths,
            variables: runtimeVariables,
            ideaBody: run.idea?.body ?? "",
            ideaAttachmentPath: run.idea?.attachmentPath,
            providerBinding: providerBindingsByAgentID[agent.id],
            catalog: catalog,
            contextStrategyProfileID: profileID,
            strategyAssignmentMode: strategyAssignmentMode,
            contextStrategyProfile: profile,
            handoffPacket: nil
        )

        return handoffCompiler.compile(
            profileID: profileID,
            profile: profile,
            agent: agent,
            task: task,
            context: previewContext,
            promotedArtifacts: promotedHandoffArtifacts
        )
    }

    private func effectiveAgent(from agent: ResolvedAgent) -> ResolvedAgent {
        let effectiveScope = Self.effectiveSessionReuseScope(
            for: agent, profile: contextStrategyProfile)
        guard effectiveScope != agent.sessionReuseScope else {
            return agent
        }

        return ResolvedAgent(
            id: agent.id,
            title: agent.title,
            mode: agent.mode,
            backendProfileID: agent.backendProfileID,
            provider: agent.provider,
            model: agent.model,
            effort: agent.effort,
            maxTurns: agent.maxTurns,
            temperature: agent.temperature,
            permissionProfile: agent.permissionProfile,
            mcpProfileID: agent.mcpProfileID,
            skillRef: agent.skillRef,
            skillRole: agent.skillRole,
            resolvedSkill: agent.resolvedSkill,
            prompt: agent.prompt,
            outputContract: agent.outputContract,
            requiresHumanApproval: agent.requiresHumanApproval,
            inputs: agent.inputs,
            outputs: agent.outputs,
            worktreeWriteEnabled: agent.worktreeWriteEnabled,
            sessionReuseScope: effectiveScope,
            sessionFamilyID: agent.sessionFamilyID,
            runtimeProfileID: agent.runtimeProfileID
        )
    }

    static func effectiveSessionReuseScope(
        for agent: ResolvedAgent,
        profile: ContextStrategyProfile?
    ) -> SessionReuseScope {
        guard
            let rule = profile?.agents[agent.id] ?? profile?.agents["*"],
            let continuityMode = rule.continuityMode
        else {
            return agent.sessionReuseScope
        }

        switch continuityMode {
        case .familyWithinRun:
            return .same_agent_family_within_run
        case .none:
            return .none
        }
    }

    private var preferredProjectRoot: URL? {
        if let frozenPath = run.frozenWorkspaceRootPath?
            .trimmingCharacters(in: .whitespacesAndNewlines),
            !frozenPath.isEmpty
        {
            return URL(fileURLWithPath: frozenPath, isDirectory: true)
        }

        if let config = deliveryConfig {
            return URL(fileURLWithPath: config.repoRoot, isDirectory: true)
        }

        return nil
    }

    private func isApprovalGranted(for stageID: String) -> Bool {
        guard !run.isDeleted, run.modelContext != nil else { return false }
        return run.approvals.contains { $0.stageID == stageID && $0.decision == .granted }
    }

    private func isApprovalRejected(for stageID: String) -> Bool {
        guard !run.isDeleted, run.modelContext != nil else { return false }
        return run.approvals.contains { $0.stageID == stageID && $0.decision == .rejected }
    }

    private func loadPersistedArtifacts() {
        let artifacts = persistedArtifacts()

        producedArtifactNames = Set(artifacts.map(\.name))

        for artifact in artifacts where artifact.format == .json {
            recordArtifactForTransition(
                artifact,
                data: try? artifactManager.readArtifact(artifact, workspace: workspace),
                validatedFields: [:]
            )
        }
    }

    private func reconcileLateMaterializedOutputsIfNeeded() {
        guard let state = plan.states[currentStateID],
            let stageExec = resumableStageExecution(for: currentStateID)
        else {
            return
        }

        for task in tasks(in: state) {
            guard let resolvedAgent = plan.agentBindings[task.agent] else { continue }
            let agent = effectiveAgent(from: resolvedAgent)
            let matchingExecutions = stageExec.agentExecutions
                .filter { $0.agentID == agent.id && $0.taskName == task.task }
                .sorted {
                    let lhsAttempt = $0.agentAttemptNumber ?? 1
                    let rhsAttempt = $1.agentAttemptNumber ?? 1
                    if lhsAttempt != rhsAttempt { return lhsAttempt < rhsAttempt }
                    return $0.startedAt < $1.startedAt
                }

            guard let latestExec = matchingExecutions.last,
                latestExec.status != .completed
            else {
                continue
            }

            for candidateExec in matchingExecutions.reversed() {
                guard
                    let recovered = recoverLatePersistedContractOutputs(
                        for: candidateExec,
                        latestExecution: latestExec,
                        task: task,
                        agent: agent,
                        stageExec: stageExec,
                        state: state
                    ), recovered
                else {
                    continue
                }
                break
            }
        }
    }

    private func tasks(in state: ExecutableState) -> [AgentTask] {
        var collected: [AgentTask] = []
        if let runBlock = state.runBlock {
            for phase in runBlock.phases {
                switch phase {
                case .sequential(let tasks):
                    collected.append(contentsOf: tasks)
                case .parallel(let tasks):
                    collected.append(contentsOf: tasks)
                }
            }
        }
        if let runAfterApproval = state.runAfterApproval {
            for phase in runAfterApproval.phases {
                switch phase {
                case .sequential(let tasks):
                    collected.append(contentsOf: tasks)
                case .parallel(let tasks):
                    collected.append(contentsOf: tasks)
                }
            }
        }
        return collected
    }

    private func recoverLatePersistedContractOutputs(
        for candidateExec: AgentExecution,
        latestExecution: AgentExecution,
        task: AgentTask,
        agent: ResolvedAgent,
        stageExec: StageExecution,
        state: ExecutableState
    ) -> Bool? {
        let expectedOutputNames = OutputContractResolverV2.expectedOutputs(for: task, agent: agent)
        guard !expectedOutputNames.isEmpty else { return false }

        var recoveredOutputs: [String: Data] = [:]
        for outputName in expectedOutputNames {
            if let existingArtifact = candidateExec.artifacts.last(where: { $0.name == outputName }
            ),
                ArtifactStorage.exists(filePath: existingArtifact.filePath),
                let data = try? artifactManager.readArtifact(
                    existingArtifact, workspace: currentWorkspace)
            {
                recoveredOutputs[outputName] = data
                continue
            }

            let expectedPath = expectedArtifactPath(
                for: outputName,
                agentID: agent.id,
                stageID: state.id,
                iteration: stageExec.iteration,
                attemptNumber: stageExec.attemptNumber,
                agentAttemptNumber: candidateExec.agentAttemptNumber
            )

            guard ArtifactStorage.exists(filePath: expectedPath),
                let data = try? ArtifactStorage.read(
                    filePath: expectedPath,
                    workspaceRoot: currentWorkspace.workspaceRoot
                )
            else {
                return false
            }

            recoveredOutputs[outputName] = data
        }

        guard recoveredOutputs.count == expectedOutputNames.count else { return false }

        var envelopes: [StructuredOutputEnvelope] = []
        let validationResults = ArtifactPersistenceOrderingPolicy.validatePersistedOutputs(
            outputs: recoveredOutputs,
            agent: agent,
            catalog: catalog,
            envelopes: &envelopes
        )
        let failedResults = validationResults.values.filter { $0.status == .failed }
        guard failedResults.isEmpty else { return false }

        let validatedFields: [String: [String: AnyCodableValue]]
        do {
            validatedFields = try validateStructuredOutputs(
                recoveredOutputs,
                for: task,
                agent: agent
            )
        } catch {
            return false
        }

        let missingPersistedOutputs = recoveredOutputs.filter { outputName, _ in
            !candidateExec.artifacts.contains(where: { $0.name == outputName })
        }
        if !missingPersistedOutputs.isEmpty {
            guard
                let importedArtifacts = try? artifactManager.persistOutputs(
                    outputs: missingPersistedOutputs,
                    agent: agent,
                    agentExecution: candidateExec,
                    workspace: currentWorkspace,
                    stageID: state.id,
                    iteration: stageExec.iteration,
                    attemptNumber: stageExec.attemptNumber,
                    catalog: catalog
                )
            else {
                return false
            }
            capturePersistedExecutionEvidence(from: importedArtifacts, for: candidateExec)
        }

        candidateExec.outputEnvelopesJSON = try? JSONEncoder().encode(envelopes)
        candidateExec.validationFailureJSON = nil
        candidateExec.status = .completed
        candidateExec.completedAt = candidateExec.completedAt ?? Date()
        candidateExec.logSnippet = mergedLogSnippet(
            existing: candidateExec.logSnippet,
            result: "Recovered from late materialized contract outputs on disk"
        )
        applyTerminalExecutionTruth(
            to: candidateExec,
            canonicalOutcome: .completedWithTransportError,
            supervisionClassification: candidateExec.supervisionClassification,
            transportErrorKind: candidateExec.transportErrorKind,
            providerStopReason: candidateExec.providerStopReason,
            outputPresence: .durableOutput,
            runtimeProvider: candidateExec.runtimeProvider,
            runtimeModel: candidateExec.runtimeModel,
            rawErrorMessage: candidateExec.logSnippet,
            rawFinishEvent: nil
        )
        updateCompactionOutcome(agentExec: candidateExec, succeeded: true)

        if latestExecution.id != candidateExec.id {
            latestExecution.status = .completed
            latestExecution.completedAt = latestExecution.completedAt ?? Date()
            latestExecution.validationFailureJSON = nil
            latestExecution.logSnippet = mergedLogSnippet(
                existing: latestExecution.logSnippet,
                result: "Skipped because a prior attempt recovered valid contract outputs"
            )
            applyTerminalExecutionTruth(
                to: latestExecution,
                canonicalOutcome: .completedWithTransportError,
                supervisionClassification: latestExecution.supervisionClassification,
                transportErrorKind: latestExecution.transportErrorKind
                    ?? candidateExec.transportErrorKind,
                providerStopReason: latestExecution.providerStopReason,
                outputPresence: .durableOutput,
                runtimeProvider: latestExecution.runtimeProvider,
                runtimeModel: latestExecution.runtimeModel,
                rawErrorMessage: latestExecution.logSnippet,
                rawFinishEvent: nil
            )
            updateCompactionOutcome(agentExec: latestExecution, succeeded: true)
        }

        stageExec.validationFailureJSON = nil
        stageExec.evidencePacketJSON = nil
        stageExec.recoverySnapshotJSON = nil

        for artifact in candidateExec.artifacts {
            recordArtifactForTransition(
                artifact,
                data: try? artifactManager.readArtifact(artifact, workspace: currentWorkspace),
                validatedFields: validatedFields
            )
        }

        return true
    }

    private func expectedArtifactPath(
        for name: String,
        agentID: String,
        stageID: String,
        iteration: Int,
        attemptNumber: Int,
        agentAttemptNumber: Int?
    ) -> String {
        var directory = currentWorkspace.artifactRoot
            .appendingPathComponent("\(stageID).\(iteration)", isDirectory: true)
            .appendingPathComponent(agentID, isDirectory: true)
            .appendingPathComponent("\(attemptNumber)", isDirectory: true)

        if let agentAttemptNumber, agentAttemptNumber > 1 {
            directory = directory.appendingPathComponent(
                "agent-retry-\(agentAttemptNumber)", isDirectory: true)
        }

        return directory.appendingPathComponent(name).path
    }

    private func restorePendingApprovalIfNeeded(for stateID: String) -> Bool {
        guard run.status == .waitingApproval,
            let state = plan.states[stateID],
            state.approvalRequired
        else {
            return false
        }

        let approval = existingOrRestoredApproval(for: state)
        isRunning = false
        isPaused = true
        currentStateID = stateID
        run.status = .waitingApproval

        let request = ApprovalRequest(
            id: approval.id,
            runID: workspace.runID,
            stageID: state.id,
            stageLabel: state.label,
            precedingArtifacts: Array(producedArtifactNames.sorted()),
            requestedAt: approval.requestedAt,
            approvalPolicy: state.approvalPolicy
        )
        onApprovalRequest?(request)
        return true
    }

    private func existingOrRestoredApproval(for state: ExecutableState) -> Approval {
        if let existing = run.approvals.first(where: {
            $0.stageID == state.id && $0.decision == .requested
        }) {
            return existing
        }

        let approval = Approval(stageID: state.id)
        approval.decision = .requested
        approval.run = run
        modelContext.insert(approval)
        return approval
    }

    private func gatherInputs(for task: AgentTask) -> [String: Data] {
        var inputs: [String: Data] = [:]
        guard let inputNames = task.inputs else { return inputs }
        let artifacts = persistedArtifacts()

        for name in inputNames {
            // Look up from already-produced artifacts
            if let artifact = artifacts.last(where: { $0.name == name }) {
                if let data = try? artifactManager.readArtifact(artifact, workspace: workspace) {
                    inputs[name] = data
                }
            }
        }
        return inputs
    }

    private func gatherExecutionInputs(
        for task: AgentTask,
        agent: ResolvedAgent
    ) async throws -> [String: Data] {
        var inputs = gatherInputs(for: task)

        guard let config = deliveryConfig,
            let worktreeRoot = currentWorkspace.worktreeRoot
        else {
            return inputs
        }

        let sourceContext: SourceContextBuilder.SourceContext
        do {
            sourceContext = try await SourceContextBuilder.build(
                worktreeRoot: worktreeRoot,
                repoRoot: config.repoRoot,
                baseBranch: config.baseBranch,
                baseRevision: run.baseRevision,
                targetBranch: config.targetBranch
            )
        } catch {
            let message =
                "sourceContextBuildFailed stateID=\(currentStateID) task=\(task.task) agentID=\(agent.id) error=\(error.localizedDescription)"
            RuntimeDiagnostics.log(message)
            print(message)
            return inputs
        }

        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]

        if let data = try? encoder.encode(sourceContext) {
            inputs["source_context"] = data
        }

        if !sourceContext.diffSummary.isEmpty,
            let data = sourceContext.diffSummary.data(using: .utf8)
        {
            inputs["source_diff_summary"] = data
        }

        if agent.worktreeWriteEnabled || plan.requiresProjectAccess,
            !sourceContext.changedFilesManifest.isEmpty,
            let data = sourceContext.changedFilesManifest.joined(separator: "\n").data(using: .utf8)
        {
            inputs["source_changed_files_manifest"] = data
        }

        return inputs
    }

    private func gatherInputArtifactPaths(for task: AgentTask) -> [String: String] {
        guard let inputNames = task.inputs, !inputNames.isEmpty else { return [:] }
        let artifacts = persistedArtifacts()
        var paths: [String: String] = [:]

        for name in inputNames {
            if let artifact = artifacts.last(where: { $0.name == name }) {
                paths[name] = artifact.filePath
            }
        }

        return paths
    }

    /// P005-OPS §9.3: Build structured input bindings with producing agent traceability.
    private func buildInputBindings(for task: AgentTask) -> Data? {
        guard let inputNames = task.inputs, !inputNames.isEmpty else { return nil }
        var bindings: [InputBinding] = []
        let artifacts = persistedArtifacts()
        for name in inputNames {
            var producingAgentID: String?
            if let artifact = artifacts.last(where: { $0.name == name }) {
                producingAgentID = artifact.agentID
            }
            bindings.append(
                InputBinding(
                    inputName: name,
                    artifactName: name,
                    producingAgentID: producingAgentID
                ))
        }
        return try? JSONEncoder().encode(bindings)
    }

    private func persistedArtifacts() -> [Artifact] {
        guard !run.isDeleted, run.modelContext != nil else { return [] }
        return run.stageExecutions
            .sorted { $0.startedAt < $1.startedAt }
            .flatMap { stage in
                stage.agentExecutions
                    .sorted { $0.startedAt < $1.startedAt }
                    .flatMap(\.artifacts)
            }
            .sorted { $0.createdAt < $1.createdAt }
    }

    private func validateStructuredOutputs(
        _ outputs: [String: Data],
        for task: AgentTask,
        agent: ResolvedAgent
    ) throws -> [String: [String: AnyCodableValue]] {
        var validated: [String: [String: AnyCodableValue]] = [:]

        // Proposal 013: V2 resolver — catalog-driven, no hardcoded fallbacks
        for outputName in OutputContractResolverV2.expectedOutputs(for: task, agent: agent) {
            guard let data = outputs[outputName] else { continue }
            guard
                let contractID = OutputContractResolverV2.resolveContractID(
                    for: outputName,
                    agent: agent,
                    catalog: catalog
                ),
                let schema = OutputContractResolverV2.resolveSchema(
                    for: outputName, agent: agent, catalog: catalog),
                schema.machineFormat == .json
            else {
                continue
            }

            // Proposal 013 §4.3: Skip strict field validation for structured_with_human_companion
            // contracts — the V2 validation has already accepted the output.
            if schema.validationMode == .structuredWithHumanCompanion {
                // Try JSON extraction for transition evaluation, but don't throw on failure
                if let fields = tryExtractScalarFields(from: data, artifactName: outputName) {
                    validated[outputName] = fields
                }
                continue
            }

            let json = try parseJSONObject(
                data,
                agentID: agent.id,
                contractID: contractID,
                outputName: outputName
            )

            for field in schema.requiredFields where json[field] == nil {
                throw ExecutionError.outputContractViolation(
                    agentID: agent.id,
                    contractID: contractID,
                    details: "Missing required field '\(field)' in '\(outputName)'"
                )
            }

            try validateTypedFields(
                json,
                contractID: contractID,
                agentID: agent.id,
                outputName: outputName
            )

            validated[outputName] = scalarFields(from: json, artifactName: outputName)
        }

        return validated
    }

    private func filteredContractOutputs(
        from outputs: [String: Data],
        task: AgentTask,
        agent: ResolvedAgent
    ) -> [String: Data] {
        let expected = Set(OutputContractResolverV2.expectedOutputs(for: task, agent: agent))
        return outputs.filter { expected.contains($0.key) }
    }

    private func parseJSONObject(
        _ data: Data,
        agentID: String,
        contractID: String,
        outputName: String
    ) throws -> [String: Any] {
        let rawObject: Any
        do {
            rawObject = try JSONSerialization.jsonObject(with: data)
        } catch {
            throw ExecutionError.outputContractViolation(
                agentID: agentID,
                contractID: contractID,
                details: "'\(outputName)' is not valid JSON"
            )
        }

        guard let json = rawObject as? [String: Any] else {
            throw ExecutionError.outputContractViolation(
                agentID: agentID,
                contractID: contractID,
                details: "'\(outputName)' must be a top-level JSON object"
            )
        }

        return json
    }

    private func validateTypedFields(
        _ json: [String: Any],
        contractID: String,
        agentID: String,
        outputName: String
    ) throws {
        switch contractID {
        case "proposal_review_v1":
            try requireString(
                json["agent_id"], field: "agent_id", agentID: agentID, contractID: contractID,
                outputName: outputName)
            try requireString(
                json["role"], field: "role", agentID: agentID, contractID: contractID,
                outputName: outputName)
            try requireNumber(
                json["score"], field: "score", agentID: agentID, contractID: contractID,
                outputName: outputName)
            try requireString(
                json["decision"], field: "decision", agentID: agentID, contractID: contractID,
                outputName: outputName)
            try requireString(
                json["verdict"], field: "verdict", agentID: agentID, contractID: contractID,
                outputName: outputName)
            try requireString(
                json["summary"], field: "summary", agentID: agentID, contractID: contractID,
                outputName: outputName)
            // Allow empty arrays or nulls for these fields to avoid failures with codex-like models
            if json["issues"] != nil {
                try requireArray(
                    json["issues"], field: "issues", agentID: agentID, contractID: contractID,
                    outputName: outputName)
            }
            if json["blocking_issues"] != nil {
                try requireArray(
                    json["blocking_issues"], field: "blocking_issues", agentID: agentID,
                    contractID: contractID, outputName: outputName)
            }
            if json["non_blocking_issues"] != nil {
                try requireArray(
                    json["non_blocking_issues"], field: "non_blocking_issues", agentID: agentID,
                    contractID: contractID, outputName: outputName)
            }
            if json["suggestions"] != nil {
                try requireArray(
                    json["suggestions"], field: "suggestions", agentID: agentID,
                    contractID: contractID,
                    outputName: outputName)
            }
            if json["assumptions"] != nil {
                try requireArray(
                    json["assumptions"], field: "assumptions", agentID: agentID,
                    contractID: contractID,
                    outputName: outputName)
            }
        case "proposal_review_summary_v1":
            try requireBool(
                json["pass"], field: "pass", agentID: agentID, contractID: contractID,
                outputName: outputName)
            try requireNumber(
                json["average_score"], field: "average_score", agentID: agentID,
                contractID: contractID,
                outputName: outputName)
            try requireNumber(
                json["aggregate_score"], field: "aggregate_score", agentID: agentID,
                contractID: contractID,
                outputName: outputName)
            try requireNumber(
                json["min_individual_score"], field: "min_individual_score", agentID: agentID,
                contractID: contractID, outputName: outputName)
            try requireInt(
                json["blocker_count"], field: "blocker_count", agentID: agentID,
                contractID: contractID,
                outputName: outputName)
            try requireString(
                json["summary"], field: "summary", agentID: agentID, contractID: contractID,
                outputName: outputName)
            try requireArray(
                json["required_changes"], field: "required_changes", agentID: agentID,
                contractID: contractID, outputName: outputName)
            try requireArray(
                json["recurring_themes"], field: "recurring_themes", agentID: agentID,
                contractID: contractID, outputName: outputName)
            try requireString(
                json["decision"], field: "decision", agentID: agentID, contractID: contractID,
                outputName: outputName)
        case "implementation_self_assessment_v2":
            if let validationError =
                OutputContractResolverV2.implementationSelfAssessmentV2ValidationError(in: json)
            {
                throw ExecutionError.outputContractViolation(
                    agentID: agentID,
                    contractID: contractID,
                    details: "'\(outputName)' \(validationError)"
                )
            }
        default:
            break
        }
    }

    private func requireString(
        _ value: Any?,
        field: String,
        agentID: String,
        contractID: String,
        outputName: String
    ) throws {
        guard value is String else {
            throw ExecutionError.outputContractViolation(
                agentID: agentID,
                contractID: contractID,
                details: "'\(outputName)' field '\(field)' must be a string"
            )
        }
    }

    private func requireNumber(
        _ value: Any?,
        field: String,
        agentID: String,
        contractID: String,
        outputName: String
    ) throws {
        guard value is NSNumber || value is Int || value is Double else {
            throw ExecutionError.outputContractViolation(
                agentID: agentID,
                contractID: contractID,
                details: "'\(outputName)' field '\(field)' must be numeric"
            )
        }
    }

    private func requireInt(
        _ value: Any?,
        field: String,
        agentID: String,
        contractID: String,
        outputName: String
    ) throws {
        if let number = value as? NSNumber,
            CFGetTypeID(number) != CFBooleanGetTypeID(),
            floor(number.doubleValue) == number.doubleValue
        {
            return
        }
        guard value is Int else {
            throw ExecutionError.outputContractViolation(
                agentID: agentID,
                contractID: contractID,
                details: "'\(outputName)' field '\(field)' must be an integer"
            )
        }
    }

    private func requireBool(
        _ value: Any?,
        field: String,
        agentID: String,
        contractID: String,
        outputName: String
    ) throws {
        if let boolValue = value as? Bool {
            _ = boolValue
            return
        }
        if let number = value as? NSNumber, CFGetTypeID(number) == CFBooleanGetTypeID() {
            return
        }
        throw ExecutionError.outputContractViolation(
            agentID: agentID,
            contractID: contractID,
            details: "'\(outputName)' field '\(field)' must be a boolean"
        )
    }

    private func requireArray(
        _ value: Any?,
        field: String,
        agentID: String,
        contractID: String,
        outputName: String
    ) throws {
        guard value is [Any] else {
            throw ExecutionError.outputContractViolation(
                agentID: agentID,
                contractID: contractID,
                details: "'\(outputName)' field '\(field)' must be an array"
            )
        }
    }

    private func recordArtifactForTransition(
        _ artifact: Artifact,
        data: Data?,
        validatedFields: [String: [String: AnyCodableValue]]
    ) {
        producedArtifactNames.insert(artifact.name)

        if let data {
            refreshImplementationSelfAssessmentProjection(from: data, artifactName: artifact.name)
        }

        if let fields = validatedFields[artifact.name] {
            artifactFields[artifact.name] = fields
        } else if artifact.format == .json,
            let data,
            let fields = tryExtractScalarFields(from: data, artifactName: artifact.name)
        {
            artifactFields[artifact.name] = fields
        }

        if let data {
            for advisory in transitionAdvisoryHints(from: artifact, data: data) {
                upsertTransitionAdvisory(advisory)
            }
        }

        applyImplementationSelfAssessmentProjectionToTransitionFields()
    }

    private struct TransitionAdvisoryHint: Sendable {
        let sourceArtifactID: String
        let sourceAgentExecutionID: String?
        let nextStageHint: String?
        let nextAction: String?
        let supersededByProjection: Bool
    }

    private func transitionAdvisoryHints(
        from artifact: Artifact,
        data: Data
    ) -> [TransitionAdvisoryHint] {
        guard let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return []
        }

        var advisories: [TransitionAdvisoryHint] = []
        let nextStage = advisoryStringValue(
            in: json,
            snakeCaseKey: "next_stage",
            camelCaseKey: "nextStage"
        )
        let nextAction = advisoryStringValue(
            in: json,
            snakeCaseKey: "next_action",
            camelCaseKey: "nextAction"
        )
        if nextStage != nil || nextAction != nil {
            advisories.append(
                TransitionAdvisoryHint(
                    sourceArtifactID: artifact.id.uuidString,
                    sourceAgentExecutionID: artifact.agentExecution?.id.uuidString,
                    nextStageHint: nextStage,
                    nextAction: nextAction,
                    supersededByProjection: false
                )
            )
        }

        advisories.append(contentsOf: projectedTransitionAdvisoryHints(from: artifact, json: json))
        return advisories
    }

    private func projectedTransitionAdvisoryHints(
        from artifact: Artifact,
        json: [String: Any]
    ) -> [TransitionAdvisoryHint] {
        guard isRunStateProjectionArtifact(artifact, json: json) else { return [] }

        var items = advisoryArtifactItems(in: json)
        if let activeIndex = json["active_index"] as? [String: Any] {
            items.append(contentsOf: advisoryArtifactItems(in: activeIndex))
        }
        if let activeIndex = json["activeIndex"] as? [String: Any] {
            items.append(contentsOf: advisoryArtifactItems(in: activeIndex))
        }
        if let activeIndexJSON = json["active_index_json"] as? [String: Any] {
            items.append(contentsOf: advisoryArtifactItems(in: activeIndexJSON))
        }

        var advisories: [TransitionAdvisoryHint] = []
        var seenPaths = Set<String>()
        for item in items {
            guard let path = advisoryStringValue(
                in: item,
                snakeCaseKey: "advisory_path",
                camelCaseKey: "advisoryPath"
            ) else {
                continue
            }
            guard seenPaths.insert(path).inserted else { continue }

            let url = URL(fileURLWithPath: path).standardizedFileURL
            guard isWorkspaceOwnedProjectionAdvisoryURL(url) else { continue }
            guard let data = try? Data(contentsOf: url),
                let advisoryJSON = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
            else {
                continue
            }

            let sourceArtifactID = advisoryStringValue(
                in: item,
                snakeCaseKey: "advisory_id",
                camelCaseKey: "advisoryID"
            ) ?? advisoryStringValue(
                in: item,
                snakeCaseKey: "artifact_id",
                camelCaseKey: "artifactID"
            ) ?? path
            let sourceAgentExecutionID = advisoryStringValue(
                in: item,
                snakeCaseKey: "source_agent_execution_id",
                camelCaseKey: "sourceAgentExecutionID"
            )
            let nextStage = advisoryStringValue(
                in: advisoryJSON,
                snakeCaseKey: "next_stage",
                camelCaseKey: "nextStage"
            )
            let nextAction = advisoryStringValue(
                in: advisoryJSON,
                snakeCaseKey: "next_action",
                camelCaseKey: "nextAction"
            )
            guard nextStage != nil || nextAction != nil else { continue }
            advisories.append(
                TransitionAdvisoryHint(
                    sourceArtifactID: sourceArtifactID,
                    sourceAgentExecutionID: sourceAgentExecutionID,
                    nextStageHint: nextStage,
                    nextAction: nextAction,
                    supersededByProjection: true
                )
            )
        }

        return advisories
    }

    private func isWorkspaceOwnedProjectionAdvisoryURL(_ url: URL) -> Bool {
        let advisoryPath = url.path
        let allowedRoots = [
            workspace.workspaceRoot.standardizedFileURL.path,
            workspace.artifactRoot.standardizedFileURL.path
        ]
        return allowedRoots.contains { root in
            advisoryPath == root || advisoryPath.hasPrefix(root + "/")
        }
    }

    private func isRunStateProjectionArtifact(_ artifact: Artifact, json: [String: Any]) -> Bool {
        artifact.contractID == "run_state_projection_v1"
            || artifact.name == "run_state_projection"
            || artifact.name == "run_state_projection_v1"
            || advisoryStringValue(
                in: json,
                snakeCaseKey: "contract_id",
                camelCaseKey: "contractID"
            ) == "run_state_projection_v1"
    }

    private func advisoryArtifactItems(in json: [String: Any]) -> [[String: Any]] {
        var items: [[String: Any]] = []
        if let advisoryArtifacts = json["advisory_artifacts"] as? [[String: Any]] {
            items.append(contentsOf: advisoryArtifacts)
        }
        if let advisoryArtifacts = json["advisoryArtifacts"] as? [[String: Any]] {
            items.append(contentsOf: advisoryArtifacts)
        }
        if let contractAdvisories = json["artifact_contract_advisories"] as? [[String: Any]] {
            items.append(contentsOf: contractAdvisories)
        }
        if let contractAdvisories = json["artifactContractAdvisories"] as? [[String: Any]] {
            items.append(contentsOf: contractAdvisories)
        }
        return items
    }

    private func advisoryStringValue(
        in json: [String: Any],
        snakeCaseKey: String,
        camelCaseKey: String
    ) -> String? {
        let rawValue = json[snakeCaseKey] ?? json[camelCaseKey]
        guard let value = rawValue as? String else { return nil }
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }

    private func upsertTransitionAdvisory(_ advisory: TransitionAdvisoryHint) {
        if let index = transitionAdvisories.firstIndex(where: {
            $0.sourceArtifactID == advisory.sourceArtifactID
        }) {
            transitionAdvisories[index] = advisory
        } else {
            transitionAdvisories.append(advisory)
        }
    }

    private func refreshImplementationSelfAssessmentProjection(
        from data: Data, artifactName: String?
    ) {
        guard
            let summaryData = ImplementationSelfAssessmentSummaryProjection.canonicalSummaryData(
                from: data,
                artifactName: artifactName
            )
        else {
            return
        }

        run.implementationSelfAssessmentSummaryJSON = summaryData
    }

    private func applyImplementationSelfAssessmentProjectionToTransitionFields() {
        guard let summaryData = run.implementationSelfAssessmentSummaryJSON,
            let fields = ImplementationSelfAssessmentSummaryProjection.scalarFields(
                fromCanonicalSummaryData: summaryData
            )
        else {
            return
        }

        artifactFields["implementation_self_assessment_v2"] = fields
    }

    private func tryExtractScalarFields(
        from data: Data,
        artifactName: String? = nil
    ) -> [String: AnyCodableValue]? {
        guard let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return nil
        }
        return scalarFields(from: json, artifactName: artifactName)
    }

    private func scalarFields(
        from json: [String: Any],
        artifactName: String? = nil
    ) -> [String: AnyCodableValue] {
        var fields: [String: AnyCodableValue] = [:]
        for (key, value) in json {
            if let number = value as? NSNumber {
                if CFGetTypeID(number) == CFBooleanGetTypeID() {
                    fields[key] = .bool(number.boolValue)
                } else if floor(number.doubleValue) == number.doubleValue {
                    fields[key] = .int(number.intValue)
                } else {
                    fields[key] = .double(number.doubleValue)
                }
            } else if let boolVal = value as? Bool {
                fields[key] = .bool(boolVal)
            } else if let intVal = value as? Int {
                fields[key] = .int(intVal)
            } else if let doubleVal = value as? Double {
                fields[key] = .double(doubleVal)
            } else if let stringVal = value as? String {
                fields[key] = .string(stringVal)
            }
        }

        if let derivedStatus = canonicalImplementationSelfAssessmentStatus(from: json) {
            fields["status"] = .string(derivedStatus)
        }

        return fields
    }

    private func canonicalImplementationSelfAssessmentStatus(from json: [String: Any]) -> String? {
        guard ImplementationSelfAssessmentSummaryProjection.isCanonicalSummaryObject(json),
            let status = json["status"] as? String
        else {
            return nil
        }
        return status
    }

    // MARK: - Proposal 032: Atomic Transition Settlement

    /// Atomically settle a transition from a completed state to the next scheduled state.
    /// Persists the cursor and saves the model context in one boundary, ensuring that
    /// resume/recovery surfaces always see a durable continuation point.
    ///
    /// Returns `true` if the save succeeded. Callers must NOT advance the state machine
    /// when this returns `false` — the transition is not durably committed.
    @discardableResult
    private func settleTransition(
        completedStateID: String,
        completedStageExecutionID: UUID?,
        nextStateID: String
    ) -> Bool {
        let currentCursor = run.transitionCursor ?? .initial()
        let iteration = currentIteration(for: nextStateID)
        let newCursor = currentCursor.settlingTransition(
            completedStateID: completedStateID,
            completedStageExecutionID: completedStageExecutionID,
            nextStateID: nextStateID,
            nextIteration: iteration
        )
        run.persistTransitionCursor(newCursor)
        do {
            try modelContext.save()
            RuntimeDiagnostics.log(
                "settleTransition seq=\(newCursor.sequenceNumber) completed=\(completedStateID) next=\(nextStateID)"
            )
            return true
        } catch {
            RuntimeDiagnostics.log(
                "settleTransition FAILED seq=\(newCursor.sequenceNumber) completed=\(completedStateID) next=\(nextStateID) error=\(error.localizedDescription)"
            )
            return false
        }
    }

    /// Mark the cursor as terminal (completed, failed, blocked, or cancelled).
    private func settleTerminal(
        lastCompletedStateID: String? = nil,
        lastCompletedStageExec: StageExecution? = nil,
        terminalFailureReason: String? = nil
    ) {
        let currentCursor = run.transitionCursor ?? .initial()
        let newCursor = currentCursor.markingTerminal(
            lastCompletedStateID: lastCompletedStateID,
            lastCompletedStageExecutionID: lastCompletedStageExec?.id,
            terminalFailureReason: terminalFailureReason
        )
        run.persistTransitionCursor(newCursor)
    }

    private func settleWorkflowConflict(
        _ conflict: WorkflowConflictRecord,
        currentStateID: String,
        currentStageExecutionID: UUID?
    ) {
        guard conflict.status != .terminalUnverifiable else {
            settleTerminal(
                lastCompletedStateID: currentStateID,
                terminalFailureReason: conflict.terminalFailureReason
            )
            do {
                try modelContext.save()
                RuntimeDiagnostics.log(
                    "settleWorkflowConflictTerminal stateID=\(currentStateID) reason=\(conflict.reason.rawValue)"
                )
            } catch {
                RuntimeDiagnostics.log(
                    "settleWorkflowConflictTerminal FAILED stateID=\(currentStateID) error=\(error.localizedDescription)"
                )
                run.driftDetails =
                    "Terminal workflow conflict cursor could not be saved for '\(currentStateID)': \(error.localizedDescription)"
            }
            return
        }
        settleWorkflowConflictBlocked(
            currentStateID: currentStateID,
            currentStageExecutionID: currentStageExecutionID
        )
    }

    private func settleWorkflowConflictBlocked(
        currentStateID: String,
        currentStageExecutionID: UUID?
    ) {
        let currentCursor = run.transitionCursor ?? .initial()
        let newCursor = currentCursor.markingWorkflowConflictBlocked(
            currentStateID: currentStateID,
            currentStageExecutionID: currentStageExecutionID
        )
        run.persistTransitionCursor(newCursor)
        do {
            try modelContext.save()
            RuntimeDiagnostics.log(
                "settleWorkflowConflictBlocked seq=\(newCursor.sequenceNumber) stateID=\(currentStateID)"
            )
        } catch {
            RuntimeDiagnostics.log(
                "settleWorkflowConflictBlocked FAILED seq=\(newCursor.sequenceNumber) stateID=\(currentStateID) error=\(error.localizedDescription)"
            )
            run.driftDetails =
                "Workflow conflict cursor could not be saved for '\(currentStateID)': \(error.localizedDescription)"
        }
    }

    // MARK: - Failure Handling

    private func handleFailure(state: ExecutableState) {
        if stateHasApplyPatchVerificationFailure(state) {
            run.status = .failed
            run.completedAt = Date()
            settleTerminal()
            persistDeliveryReceiptIfNeeded(finalStateID: state.id)
            isRunning = false
            RuntimeDiagnostics.log("handleFailure failedApplyPatchVerification stateID=\(state.id)")
            onComplete?(false)
            return
        }

        let policy = plan.failurePolicy
        let action = policy?.onError ?? "pause_and_require_human"
        RuntimeDiagnostics.log("handleFailure stateID=\(state.id) action=\(action)")

        switch action {
        case "pause_and_require_human":
            run.status = .blocked
            settleTerminal()
            persistDeliveryReceiptIfNeeded(finalStateID: state.id)
            isRunning = false
            isPaused = true
            RuntimeDiagnostics.log("handleFailure blocked stateID=\(state.id)")
        case "fail_run":
            run.status = .failed
            run.completedAt = Date()
            settleTerminal()
            persistDeliveryReceiptIfNeeded(finalStateID: state.id)
            isRunning = false
            RuntimeDiagnostics.log("handleFailure failed stateID=\(state.id)")
        default:
            run.status = .blocked
            settleTerminal()
            persistDeliveryReceiptIfNeeded(finalStateID: state.id)
            isRunning = false
            RuntimeDiagnostics.log("handleFailure blockedDefault stateID=\(state.id)")
        }

        onComplete?(false)
    }

    private func handleLoopBudgetExhausted(state: ExecutableState, counter: String, count: Int) {
        let policy = plan.failurePolicy
        let action = policy?.onLoopBudgetExhausted ?? "pause_and_require_human"

        switch action {
        case "pause_and_require_human":
            run.status = .blocked
            isPaused = true
        case "fail_run":
            run.status = .failed
            run.completedAt = Date()
        default:
            break
        }
    }

    // MARK: - Steward Metadata Helpers (Proposal 003 — REQ-002)

    /// Compute a deterministic config hash for a resolved agent.
    private static func computeAgentConfigHash(agent: ResolvedAgent) -> String {
        let canonical = [
            agent.id, agent.provider, agent.model, agent.effort,
            String(agent.maxTurns), String(agent.temperature),
            agent.permissionProfile, agent.skillRef,
            agent.skillRole ?? "",
            agent.resolvedSkill?.injectedContentHash ?? "",
            agent.outputContract ?? "",
        ].joined(separator: "|")
        return DefinitionHasher.hashString(canonical)
    }

    private static func applySkillMetadata(
        to agentExecution: AgentExecution, from agent: ResolvedAgent
    ) {
        guard let resolvedSkill = agent.resolvedSkill else {
            agentExecution.skillRef = agent.skillRef
            agentExecution.skillSnapshotHash = nil
            agentExecution.skillType = nil
            agentExecution.skillRole = agent.skillRole
            agentExecution.skillContentSummary = nil
            return
        }

        agentExecution.skillRef = agent.skillRef
        agentExecution.skillSnapshotHash = resolvedSkill.injectedContentHash
        agentExecution.skillType = resolvedSkill.type.rawValue
        agentExecution.skillRole = resolvedSkill.role ?? agent.skillRole
        agentExecution.skillContentSummary = resolvedSkill.contentSummary
    }

    static let liveTextChunkCoalescingWindow: TimeInterval = 0.35

    // MARK: - Live Event Routing

    private func configureLiveEventBridge() {
        guard let runtimeExecutor = executor as? RuntimeAgentExecutor else { return }

        runtimeExecutor.onExecutionEvent = { [weak self] agentID, event in
            Task { @MainActor [weak self] in
                self?.recordLiveExecutionEvent(agentID: agentID, event: event)
            }
        }
    }

    private func registerLiveExecution(_ agentExecution: AgentExecution, for agentID: String) {
        liveAgentExecutionsByAgentID[agentID, default: []].append(agentExecution)
    }

    private func unregisterLiveExecution(_ agentExecution: AgentExecution, for agentID: String) {
        guard var executions = liveAgentExecutionsByAgentID[agentID] else { return }
        executions.removeAll { $0.id == agentExecution.id }
        if executions.isEmpty {
            liveAgentExecutionsByAgentID.removeValue(forKey: agentID)
        } else {
            liveAgentExecutionsByAgentID[agentID] = executions
        }
        lastRoutedLiveTextChunkAtByAgentID.removeValue(forKey: agentID)
    }

    func shouldRecordLiveExecutionEvent(
        agentID: String,
        event: ExecutionEvent,
        now: Date = Date()
    ) -> Bool {
        switch event.type {
        case .textChunk:
            let trimmed = event.detail.trimmingCharacters(in: .whitespacesAndNewlines)
            guard trimmed.isEmpty == false else { return false }
            if let lastRoutedAt = lastRoutedLiveTextChunkAtByAgentID[agentID],
                now.timeIntervalSince(lastRoutedAt) < Self.liveTextChunkCoalescingWindow
            {
                return false
            }
            lastRoutedLiveTextChunkAtByAgentID[agentID] = now
            return true
        default:
            lastRoutedLiveTextChunkAtByAgentID.removeValue(forKey: agentID)
            return true
        }
    }

    func injectTestingLiveExecutionEvent(agentID: String, event: ExecutionEvent, now: Date = Date())
    {
        recordLiveExecutionEvent(agentID: agentID, event: event, now: now)
    }

    private func recordLiveExecutionEvent(
        agentID: String, event: ExecutionEvent, now: Date = Date()
    ) {
        if event.type == .textChunk {
            guard let visibleChunk = visibleLiveTextChunk(agentID: agentID, chunk: event.detail)
            else {
                return
            }
            bufferLiveTextChunk(agentID: agentID, chunk: visibleChunk)
            guard shouldRecordLiveExecutionEvent(agentID: agentID, event: event, now: now) else {
                return
            }
            let mergedEvent = ExecutionEvent(
                type: .textChunk,
                timestamp: event.timestamp,
                detail: consumeBufferedLiveTextChunk(for: agentID),
                sessionID: event.sessionID,
                requestID: event.requestID,
                toolName: event.toolName
            )
            commitLiveExecutionEvent(agentID: agentID, event: mergedEvent)
            return
        }

        flushBufferedLiveTextChunkIfNeeded(agentID: agentID, timestamp: event.timestamp)
        guard shouldRecordLiveExecutionEvent(agentID: agentID, event: event, now: now) else {
            return
        }
        commitLiveExecutionEvent(agentID: agentID, event: event)
    }

    private func commitLiveExecutionEvent(agentID: String, event: ExecutionEvent) {
        let agentExecution = resolvedAgentExecution(for: agentID)

        if let sessionID = event.sessionID {
            agentExecution?.providerSessionID = sessionID
            agentExecution?.runtimeSessionID = sessionID
        }
        if let requestID = event.requestID {
            agentExecution?.providerRequestID = requestID
        }

        let existingSnippet = agentExecution?.logSnippet
        let eventSnippet = logSnippet(for: event, existing: existingSnippet)
        agentExecution?.logSnippet = eventSnippet

        let entry = LiveExecutionTimelineEntry(
            agentID: agentID,
            agentTitle: agentExecution?.agentTitle ?? plan.agentBindings[agentID]?.title ?? agentID,
            stageID: agentExecution?.stageExecution?.stageID ?? currentStateID,
            event: event
        )

        if event.type == .textChunk,
            let lastIndex = liveTimeline.lastIndex(where: {
                $0.agentID == agentID && $0.event.type == .textChunk
            })
        {
            // Accumulate text chunks into a single rolling entry so the timeline card
            // shows the full streamed text, not just the latest word.
            let previousDetail = liveTimeline[lastIndex].event.detail
            let newChunk = event.detail
            let accumulated: String
            if newChunk.isEmpty {
                accumulated = previousDetail
            } else {
                let joined = previousDetail + newChunk
                accumulated = joined.count > 2_000 ? String(joined.suffix(2_000)) : joined
            }
            let accumulatedEvent = ExecutionEvent(
                type: .textChunk,
                timestamp: event.timestamp,
                detail: accumulated,
                sessionID: event.sessionID
            )
            let stableEntry = LiveExecutionTimelineEntry(
                id: liveTimeline[lastIndex].id,
                agentID: entry.agentID,
                agentTitle: entry.agentTitle,
                stageID: entry.stageID,
                event: accumulatedEvent
            )
            liveTimeline[lastIndex] = stableEntry
        } else {
            liveTimeline.append(entry)
            if liveTimeline.count > 40 {
                liveTimeline.removeFirst(liveTimeline.count - 40)
            }
        }

        // Proposal 018: Update session audit trail on major events
        if event.type == .sessionStarted || event.type == .sessionClosed || event.type == .error {
            updateSessionAuditTrail()
        }
    }

    private func bufferLiveTextChunk(agentID: String, chunk: String) {
        guard chunk.isEmpty == false else { return }
        pendingLiveTextChunksByAgentID[agentID, default: ""].append(chunk)
    }

    private func consumeBufferedLiveTextChunk(for agentID: String) -> String {
        pendingLiveTextChunksByAgentID.removeValue(forKey: agentID) ?? ""
    }

    private func visibleLiveTextChunk(agentID: String, chunk: String) -> String? {
        guard chunk.isEmpty == false else { return nil }

        let startMarker = "<<<CHAINWORKS_OUTPUT:"
        let endMarker = "<<<END_CHAINWORKS_OUTPUT>>>"
        var suppressing = suppressingStructuredOutputByAgentID[agentID] ?? false
        var remainder = chunk[...]
        var visible = ""

        while !remainder.isEmpty {
            if suppressing {
                if let endRange = remainder.range(of: endMarker) {
                    remainder = remainder[endRange.upperBound...]
                    suppressing = false
                } else {
                    suppressingStructuredOutputByAgentID[agentID] = true
                    return nil
                }
            } else if let startRange = remainder.range(of: startMarker) {
                visible += remainder[..<startRange.lowerBound]
                remainder = remainder[startRange.lowerBound...]
                suppressing = true
            } else if let genericMarkerRange = remainder.range(of: "<<<") {
                visible += remainder[..<genericMarkerRange.lowerBound]
                suppressing = true
                remainder = remainder[genericMarkerRange.lowerBound...]
            } else {
                visible += remainder
                remainder = "".prefix(0)
            }
        }

        suppressingStructuredOutputByAgentID[agentID] = suppressing
        return visible.isEmpty ? nil : visible
    }

    private func flushBufferedLiveTextChunkIfNeeded(agentID: String, timestamp: Date) {
        guard let buffered = pendingLiveTextChunksByAgentID[agentID] else { return }
        guard buffered.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty == false else {
            pendingLiveTextChunksByAgentID.removeValue(forKey: agentID)
            return
        }

        let event = ExecutionEvent(
            type: .textChunk,
            timestamp: timestamp,
            detail: consumeBufferedLiveTextChunk(for: agentID)
        )
        commitLiveExecutionEvent(agentID: agentID, event: event)
    }

    /// Proposal 018: Final audit trail update on run completion.
    /// Also generates KPI export and report bridge data (§8, REQ-013, PROD-001).
    func updateSessionAuditTrailOnCompletion() {
        updateSessionAuditTrail()
        // REQ-013: Export KPIs on completion
        if let kpiJSON = SessionReuseKPIExporter.exportJSON(for: run.id, context: modelContext) {
            run.sessionKPIExportJSON = kpiJSON
        }
        // PROD-001: Generate structured lineage report for run-level reporting.
        // SessionLineageReportBridge is the production consumer of lineage data
        // for run reports and export surfaces.
        if let reportJSON = SessionLineageReportBridge.generateReportJSON(
            for: run.id, context: modelContext)
        {
            run.sessionLineageReportJSON = reportJSON
        }
    }

    /// Proposal 018: Aggregates session events into a derived audit trail on the Run.
    private func updateSessionAuditTrail() {
        // Fetch all lineage/events for this run
        let runID = run.id
        let descriptor = FetchDescriptor<AgentSessionLineage>(
            predicate: #Predicate<AgentSessionLineage> { $0.runID == runID }
        )

        guard let lineages = try? modelContext.fetch(descriptor) else { return }

        struct AuditEntry: Codable {
            let agentID: String
            let eventType: String
            let timestamp: Date
            let generation: Int
        }

        var allEntries: [AuditEntry] = []
        for lineage in lineages {
            for event in lineage.events {
                let gen =
                    lineage.generations.first(where: { $0.id == event.generationID })?.generation
                    ?? 0
                allEntries.append(
                    AuditEntry(
                        agentID: lineage.agentID,
                        eventType: event.eventType.rawValue,
                        timestamp: event.recordedAt,
                        generation: gen
                    ))
            }
        }

        allEntries.sort { $0.timestamp < $1.timestamp }
        run.sessionEventAuditDerivedJSON = try? JSONEncoder().encode(allEntries)
    }

    private func logSnippet(for event: ExecutionEvent, existing: String?) -> String {
        switch event.type {
        case .sessionStarted:
            return event.detail
        case .promptSubmitted:
            return existing ?? "Prompt submitted"
        case .toolCallStarted, .toolCallFinished:
            return event.detail
        case .textChunk:
            let trimmed = event.detail.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else { return existing ?? "Streaming output..." }
            return String(trimmed.prefix(220))
        case .finalOutput:
            return "Final output received"
        case .finish:
            return event.detail
        case .error:
            return "Provider error: \(event.detail)"
        case .sessionClosed:
            return existing ?? "Session closed"
        case .unknown:
            return event.detail
        }
    }

    private func resolvedAgentExecution(for agentID: String) -> AgentExecution? {
        if let activeExecution = liveAgentExecutionsByAgentID[agentID]?.last {
            return activeExecution
        }

        return run.stageExecutions
            .flatMap(\.agentExecutions)
            .filter { $0.agentID == agentID }
            .sorted { $0.startedAt < $1.startedAt }
            .last
    }

    private func encodeProviderReceipt(_ receipt: ProviderExecutionReceipt?) -> Data? {
        guard let receipt else { return nil }
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        return try? encoder.encode(receipt)
    }

    private func encodeOutcomeEnvelope(_ envelope: OutcomeEnvelope?) -> Data? {
        guard let envelope else { return nil }
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        return try? encoder.encode(envelope)
    }

    private func encodeStringArray(_ values: [String]) -> Data? {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        return try? encoder.encode(values)
    }

    private func encodeMCPServerMetrics(_ metrics: [MCPServerExecutionMetric]) -> Data? {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        return try? encoder.encode(metrics)
    }

    private func applyExecutionTruth(from result: AgentResult, to agentExec: AgentExecution) {
        let canonicalOutcome =
            result.canonicalOutcome ?? (result.succeeded ? .completed : .failedBeforeOutput)
        let runtimeProvider = result.runtimeProvider ?? result.providerReceipt?.providerFamily
        let runtimeModel =
            result.runtimeModel ?? result.providerReceipt?.model ?? result.resolvedModel
        let envelope =
            result.outcomeEnvelope
            ?? OutcomeEnvelope(
                canonicalOutcome: canonicalOutcome,
                transportErrorKind: result.transportErrorKind,
                providerStopReason: result.providerStopReason,
                outputPresence: result.outputPresence,
                rawErrorMessage: result.errorMessage,
                rawFinishEvent: nil
            )

        applyTerminalExecutionTruth(
            to: agentExec,
            canonicalOutcome: canonicalOutcome,
            supervisionClassification: result.supervisionClassification,
            transportErrorKind: result.transportErrorKind,
            providerStopReason: result.providerStopReason,
            outputPresence: result.outputPresence,
            runtimeProvider: runtimeProvider,
            runtimeModel: runtimeModel,
            envelope: envelope
        )
        agentExec.mcpProfileID = result.mcpProfileID
        agentExec.requestedMCPExtensionsJSON = encodeStringArray(result.requestedMCPExtensions)
        agentExec.effectiveMCPRuntimeExtensionIDsJSON = encodeStringArray(
            result.effectiveMCPRuntimeExtensionIDs)
        agentExec.deniedMCPExtensionsJSON = encodeStringArray(result.deniedMCPExtensions)
        agentExec.mcpSessionStartupLatencyMilliseconds = result.mcpSessionStartupLatencyMilliseconds
        agentExec.mcpServerTelemetryJSON = encodeMCPServerMetrics(result.mcpServerMetrics)
        if let retryReason = suggestedRetryReason(from: result) {
            agentExec.retryReason = retryReason
        }
    }

    private func applyTerminalExecutionTruth(
        to agentExec: AgentExecution,
        canonicalOutcome: AgentCanonicalOutcome,
        supervisionClassification: SupervisionClassification? = nil,
        transportErrorKind: TransportErrorKind?,
        providerStopReason: String?,
        outputPresence: OutputPresence,
        runtimeProvider: String?,
        runtimeModel: String?,
        rawErrorMessage: String? = nil,
        rawFinishEvent: String? = nil,
        envelope: OutcomeEnvelope? = nil
    ) {
        agentExec.canonicalOutcome = canonicalOutcome
        agentExec.supervisionClassification = supervisionClassification
        agentExec.transportErrorKind = transportErrorKind
        agentExec.providerStopReason = providerStopReason
        agentExec.outputPresence = outputPresence
        agentExec.runtimeProvider = runtimeProvider
        agentExec.runtimeModel = runtimeModel
        agentExec.settledAt = agentExec.completedAt ?? Date()

        let resolvedEnvelope =
            envelope
            ?? OutcomeEnvelope(
                canonicalOutcome: canonicalOutcome,
                transportErrorKind: transportErrorKind,
                providerStopReason: providerStopReason,
                outputPresence: outputPresence,
                rawErrorMessage: rawErrorMessage,
                rawFinishEvent: rawFinishEvent
            )
        agentExec.outcomeEnvelopeJSON = encodeOutcomeEnvelope(resolvedEnvelope)
    }

    private func suggestedRetryReason(from result: AgentResult) -> String? {
        let canonicalOutcome =
            result.canonicalOutcome ?? (result.succeeded ? .completed : .failedBeforeOutput)
        let lowercasedError = result.errorMessage?.lowercased() ?? ""

        switch canonicalOutcome {
        case .limitExhaustedBeforeOutput, .limitExhaustedAfterOutput:
            if lowercasedError.contains("capacity")
                || lowercasedError.contains("resource_exhausted")
            {
                return "Provider capacity exhausted; retry this agent"
            }
            return "Provider limit exhausted; retry this agent"
        case .timedOutBeforeOutput, .timedOutAfterOutput:
            return "Execution timed out; retry this agent"
        case .completedWithTransportError:
            return "Transport failed after output; retry this agent"
        default:
            break
        }

        if lowercasedError.contains("session not found")
            || lowercasedError.contains("failed to read session")
        {
            return "Provider session became unavailable; retry this agent"
        }

        return nil
    }

    private static func decodeProviderBindings(from data: Data?) -> [String:
        ResolvedProviderBinding]
    {
        guard let data else { return [:] }
        return (try? JSONDecoder().decode([String: ResolvedProviderBinding].self, from: data))
            ?? [:]
    }

    private static func decodeContextStrategyProfileID(from run: Run) -> String? {
        let profileID = run.contextStrategyProfileID.trimmingCharacters(in: .whitespacesAndNewlines)
        return profileID.isEmpty ? nil : profileID
    }

    private static func decodeStrategyAssignmentMode(from run: Run) -> String? {
        let mode = run.strategyAssignmentMode.trimmingCharacters(in: .whitespacesAndNewlines)
        return mode.isEmpty ? nil : mode
    }

    private static func decodeContextStrategyProfile(from run: Run) -> ContextStrategyProfile? {
        guard let json = run.contextStrategySnapshotJSON else { return nil }
        if let profile = try? JSONDecoder().decode(ContextStrategyProfile.self, from: json) {
            return profile
        }
        if let stewardProfile = try? JSONDecoder().decode(
            StewardContextStrategyProfile.self, from: json)
        {
            let profileID = decodeContextStrategyProfileID(from: run) ?? "current_mixed_baseline"
            return stewardProfile.runtimeProfile(profileID: profileID)
        }
        return nil
    }

    private static func decodePromotedHandoffArtifacts(from run: Run) -> [String] {
        guard let data = run.promotedHandoffArtifactsJSON,
            let names = try? JSONDecoder().decode([String].self, from: data)
        else {
            return []
        }
        return names
    }

    private func strategyAdjustedBinding(
        for agent: ResolvedAgent,
        baseBinding: ResolvedProviderBinding?,
        modelTier: String?
    ) -> ResolvedProviderBinding? {
        guard let baseBinding else { return nil }
        guard
            let resolvedModel = resolveStrategyModel(
                for: agent, baseBinding: baseBinding, modelTier: modelTier)
        else {
            return baseBinding
        }

        return ResolvedProviderBinding(
            agentID: baseBinding.agentID,
            backendProfileID: baseBinding.backendProfileID,
            configuredProviderID: baseBinding.configuredProviderID,
            providerFamily: baseBinding.providerFamily,
            providerIdentifier: baseBinding.providerIdentifier,
            model: resolvedModel,
            effort: baseBinding.effort,
            transport: baseBinding.transport,
            adapterVersion: baseBinding.adapterVersion,
            runtimeProfileID: baseBinding.runtimeProfileID,
            adapterFamily: baseBinding.adapterFamily,
            capabilityClass: baseBinding.capabilityClass
        )
    }

    private func preferredPrimaryModelTier(
        for agent: ResolvedAgent,
        task: AgentTask,
        stageExecution: StageExecution,
        profile: ContextStrategyProfile?
    ) -> String? {
        let requestedTier = profile?.defaultModelTier
        guard
            shouldHoldBackFastTierForNoProgress(
                requestedTier: requestedTier,
                agent: agent,
                task: task,
                stageExecution: stageExecution
            )
        else {
            return requestedTier
        }

        let escalationTier = profile?.escalationModelTier?.trimmingCharacters(
            in: .whitespacesAndNewlines)
        if let escalationTier, !escalationTier.isEmpty {
            return escalationTier
        }
        return nil
    }

    private func shouldHoldBackFastTierForNoProgress(
        requestedTier: String?,
        agent: ResolvedAgent,
        task: AgentTask,
        stageExecution: StageExecution
    ) -> Bool {
        let normalizedTier = requestedTier?.trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()
        guard normalizedTier == "fast" else { return false }
        guard agent.id == "code_writer" else { return false }
        guard agent.mode == "implementation" || agent.worktreeWriteEnabled else { return false }
        guard
            task.task == "implement" || task.task == "continue_implementation"
                || task.task == "initial_implementation"
        else {
            return false
        }
        guard
            let priorExecution = latestComparableExecution(
                for: agent, task: task, stageExecution: stageExecution)
        else {
            return false
        }

        return executionShowsNoMeaningfulImplementationProgress(priorExecution)
            || executionExhaustedFastTierLimits(priorExecution)
    }

    private func latestComparableExecution(
        for agent: ResolvedAgent,
        task: AgentTask,
        stageExecution: StageExecution
    ) -> AgentExecution? {
        let currentStageComparableExecutions = stageExecution.agentExecutions
            .filter { candidate in
                candidate.agentID == agent.id
                    && candidate.taskName == task.task
                    && candidate.completedAt != nil
            }
            .sorted { lhs, rhs in
                let lhsCompleted = lhs.completedAt ?? lhs.startedAt
                let rhsCompleted = rhs.completedAt ?? rhs.startedAt
                if lhsCompleted == rhsCompleted {
                    return lhs.startedAt < rhs.startedAt
                }
                return lhsCompleted < rhsCompleted
            }
        if let latestCurrentStageExecution = currentStageComparableExecutions.last {
            return latestCurrentStageExecution
        }

        let comparableExecutions = run.stageExecutions
            .filter { candidate in
                candidate.id != stageExecution.id
                    && candidate.stageID == stageExecution.stageID
            }
            .flatMap(\.agentExecutions)
            .filter { candidate in
                candidate.agentID == agent.id
                    && candidate.taskName == task.task
                    && candidate.startedAt <= stageExecution.startedAt
            }
            .sorted { lhs, rhs in
                let lhsCompleted = lhs.completedAt ?? lhs.startedAt
                let rhsCompleted = rhs.completedAt ?? rhs.startedAt
                if lhsCompleted == rhsCompleted {
                    return lhs.startedAt < rhs.startedAt
                }
                return lhsCompleted < rhsCompleted
            }

        return comparableExecutions.last
    }

    private func executionShowsNoMeaningfulImplementationProgress(_ execution: AgentExecution)
        -> Bool
    {
        if let changedFilesArtifact = artifact(
            named: ImplementationFailureArtifactSynthesizer.changedFilesArtifactName,
            for: execution
        ) {
            let changedFiles = changedFilesList(from: changedFilesArtifact)
            if let changedFiles, !changedFiles.isEmpty {
                return false
            }
            if changedFiles != nil {
                return true
            }
        }

        if let progressArtifact = artifact(
            named: ImplementationFailureArtifactSynthesizer.progressArtifactName,
            for: execution
        ) {
            let completedItems = completedImplementationItems(from: progressArtifact)
            if let completedItems, !completedItems.isEmpty {
                return false
            }
            if completedItems != nil {
                return true
            }
        }

        return false
    }

    private func executionExhaustedFastTierLimits(_ execution: AgentExecution) -> Bool {
        if let providerStopReason = execution.providerStopReason?.lowercased(),
            isUsageLimitStopReason(providerStopReason)
        {
            return true
        }

        switch execution.canonicalOutcome {
        case .limitExhaustedBeforeOutput, .limitExhaustedAfterOutput:
            return true
        default:
            break
        }

        if let snippet = execution.logSnippet?.lowercased(),
            isUsageLimitStopReason(snippet)
        {
            return true
        }

        return false
    }

    private func stateHasApplyPatchVerificationFailure(_ state: ExecutableState) -> Bool {
        run.stageExecutions
            .filter { $0.stageID == state.id }
            .flatMap(\.agentExecutions)
            .contains { isApplyPatchVerificationFailure($0) }
    }

    private func isApplyPatchVerificationFailure(_ execution: AgentExecution) -> Bool {
        if let providerStopReason = execution.providerStopReason?.lowercased(),
            providerStopReason == "apply_patch_verification_failed"
        {
            return true
        }
        if let snippet = execution.logSnippet?.lowercased(),
            snippet.contains("apply_patch verification failed")
        {
            return true
        }
        if let envelopeData = execution.outcomeEnvelopeJSON,
            let envelope = try? JSONDecoder().decode(OutcomeEnvelope.self, from: envelopeData),
            envelope.rawErrorMessage?.lowercased().contains("apply_patch verification failed")
                == true
        {
            return true
        }
        return false
    }

    private func artifact(named name: String, for execution: AgentExecution) -> Artifact? {
        execution.artifacts.first { artifact in
            artifact.name == name || artifact.contractID == name
        }
    }

    private func changedFilesList(from artifact: Artifact) -> [String]? {
        guard let object = artifactJSONObject(for: artifact) else { return nil }
        return object["files"] as? [String]
    }

    private func completedImplementationItems(from artifact: Artifact) -> [String]? {
        guard let object = artifactJSONObject(for: artifact) else { return nil }
        return object["completed_items"] as? [String]
    }

    private func artifactJSONObject(for artifact: Artifact) -> [String: Any]? {
        guard FileManager.default.fileExists(atPath: artifact.filePath) else { return nil }
        guard let data = try? Data(contentsOf: URL(fileURLWithPath: artifact.filePath)) else {
            return nil
        }
        return (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
    }

    private func strategyAdjustedAgent(
        from agent: ResolvedAgent,
        binding: ResolvedProviderBinding?
    ) -> ResolvedAgent {
        let resolvedProvider = binding?.providerIdentifier ?? agent.provider
        let resolvedModel = binding?.model ?? agent.model
        let resolvedEffort = binding?.effort ?? agent.effort
        let resolvedRuntimeProfileID = binding?.runtimeProfileID ?? agent.runtimeProfileID
        guard
            resolvedProvider != agent.provider
                || resolvedModel != agent.model
                || resolvedEffort != agent.effort
                || resolvedRuntimeProfileID != agent.runtimeProfileID
        else {
            return agent
        }

        return ResolvedAgent(
            id: agent.id,
            title: agent.title,
            mode: agent.mode,
            backendProfileID: agent.backendProfileID,
            provider: resolvedProvider,
            model: resolvedModel,
            effort: resolvedEffort,
            maxTurns: agent.maxTurns,
            temperature: agent.temperature,
            permissionProfile: agent.permissionProfile,
            mcpProfileID: agent.mcpProfileID,
            skillRef: agent.skillRef,
            skillRole: agent.skillRole,
            resolvedSkill: agent.resolvedSkill,
            prompt: agent.prompt,
            outputContract: agent.outputContract,
            requiresHumanApproval: agent.requiresHumanApproval,
            inputs: agent.inputs,
            outputs: agent.outputs,
            worktreeWriteEnabled: agent.worktreeWriteEnabled,
            sessionReuseScope: agent.sessionReuseScope,
            sessionFamilyID: agent.sessionFamilyID,
            runtimeProfileID: resolvedRuntimeProfileID
        )
    }

    private func capacityFallbackBinding(
        for agent: ResolvedAgent,
        attemptedBinding: ResolvedProviderBinding?,
        result: AgentResult
    ) -> ResolvedProviderBinding? {
        guard shouldAttemptCapacityFallback(for: result) else { return nil }
        guard let attemptedBinding else { return nil }

        let family =
            ProviderFamily(rawValue: attemptedBinding.providerFamily)
            ?? ProviderFamily.from(runtimeIdentifier: agent.provider)
        guard
            let fallbackModel = fallbackModelForCapacityExhaustion(
                family: family,
                currentModel: attemptedBinding.model
            )
        else {
            return nil
        }
        guard fallbackModel.caseInsensitiveCompare(attemptedBinding.model) != .orderedSame else {
            return nil
        }

        return ResolvedProviderBinding(
            agentID: attemptedBinding.agentID,
            backendProfileID: attemptedBinding.backendProfileID,
            configuredProviderID: attemptedBinding.configuredProviderID,
            providerFamily: attemptedBinding.providerFamily,
            providerIdentifier: attemptedBinding.providerIdentifier,
            model: fallbackModel,
            effort: attemptedBinding.effort,
            transport: attemptedBinding.transport,
            adapterVersion: attemptedBinding.adapterVersion,
            runtimeProfileID: attemptedBinding.runtimeProfileID,
            adapterFamily: attemptedBinding.adapterFamily,
            capabilityClass: attemptedBinding.capabilityClass
        )
    }

    private func shouldAttemptCapacityFallback(for result: AgentResult) -> Bool {
        if result.providerStopReason == "model_capacity_exhausted" {
            return true
        }

        let lowercasedError = result.errorMessage?.lowercased() ?? ""
        return lowercasedError.contains("model_capacity_exhausted")
            || lowercasedError.contains("capacity exhausted")
            || lowercasedError.contains("no capacity available for model")
    }

    private func fallbackModelForCapacityExhaustion(
        family: ProviderFamily?,
        currentModel: String
    ) -> String? {
        let lowercasedModel = currentModel.trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()

        switch family {
        case .geminiACP?:
            if lowercasedModel == "gemini-2.5-flash" {
                return nil
            }
            if lowercasedModel == "gemini-2.5-pro" {
                return "gemini-2.5-flash"
            }
            if lowercasedModel.contains("preview") || lowercasedModel.hasPrefix("gemini-3") {
                return "gemini-2.5-pro"
            }
            return "gemini-2.5-pro"
        default:
            return nil
        }
    }

    private func resolveStrategyModel(
        for agent: ResolvedAgent,
        baseBinding: ResolvedProviderBinding?,
        modelTier: String?
    ) -> String? {
        let normalizedTier = modelTier?.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard let normalizedTier, !normalizedTier.isEmpty else {
            return baseBinding?.model ?? agent.model
        }

        let family =
            baseBinding.flatMap { ProviderFamily(rawValue: $0.providerFamily) }
            ?? ProviderFamily.from(runtimeIdentifier: agent.provider)

        if shouldForceFrontierCodexTier(for: agent, family: family), normalizedTier == "fast" {
            return "GPT-5.4"
        }

        switch (family, normalizedTier) {
        case (.codexACP?, "fast"):
            return "gpt-5.3-codex-spark"
        case (.codexACP?, "frontier"):
            return "GPT-5.4"
        case (.claudeACP?, "fast"):
            return "sonnet"
        case (.claudeACP?, "frontier"):
            return "opus"
        case (.geminiACP?, "fast"):
            return "gemini-2.5-flash"
        case (.geminiACP?, "frontier"):
            return "gemini-2.5-pro"
        default:
            return baseBinding?.model ?? agent.model
        }
    }

    private func resolvedModelTierUsed(
        requestedTier: String?,
        effectiveAgent: ResolvedAgent
    ) -> String {
        let normalizedTier = requestedTier?
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()
        let family = ProviderFamily.from(runtimeIdentifier: effectiveAgent.provider)
        if normalizedTier == "fast",
            shouldForceFrontierCodexTier(for: effectiveAgent, family: family)
        {
            return "frontier"
        }
        if let normalizedTier, !normalizedTier.isEmpty {
            return normalizedTier
        }
        return effectiveAgent.model.isEmpty ? "bound_runtime" : "bound_runtime"
    }

    private func shouldForceFrontierCodexTier(
        for agent: ResolvedAgent,
        family: ProviderFamily?
    ) -> Bool {
        guard family == .codexACP else { return false }

        let skillRole = agent.skillRole?.trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()
        let backendProfileID = agent.backendProfileID?.trimmingCharacters(
            in: .whitespacesAndNewlines
        )
        .lowercased()
        let agentID = agent.id.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let mode = agent.mode.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()

        return skillRole == "architect"
            || backendProfileID == "codex_architect_high"
            || agentID == "proposal_reviewer_architect"
            || mode == "proposal_review.architect"
    }

    private func shouldEscalateStrategyExecution(
        result: AgentResult,
        profile: ContextStrategyProfile?
    ) -> Bool {
        guard !result.succeeded else { return false }
        guard let profile else { return false }

        let defaultTier = profile.defaultModelTier?.trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()
        let escalationTier = profile.escalationModelTier?.trimmingCharacters(
            in: .whitespacesAndNewlines
        ).lowercased()
        guard let escalationTier, !escalationTier.isEmpty, escalationTier != defaultTier else {
            return false
        }
        if result.providerStopReason?.lowercased() == "apply_patch_verification_failed" {
            return false
        }
        if isUsageLimitResult(result) {
            return true
        }

        if result.canonicalOutcome == .failedAfterOutputValidation {
            return false
        }
        if let error = result.errorMessage?.lowercased() {
            if error.contains("required outputs missing")
                || error.contains("output contract")
                || error.contains("not valid json")
                || error.contains("missing required field")
            {
                return false
            }
            if error.contains("apply_patch verification failed") {
                return false
            }
            if error.contains("timed out")
                || error.contains("timeout")
                || error.contains("session not found")
                || error.contains("-1001")
            {
                return true
            }
        }

        switch result.transportErrorKind {
        case .timeout, .stream, .provider:
            return true
        default:
            break
        }

        switch result.canonicalOutcome {
        case .timedOutBeforeOutput, .timedOutAfterOutput, .completedWithTransportError,
            .limitExhaustedBeforeOutput, .limitExhaustedAfterOutput:
            return true
        default:
            return false
        }
    }

    private func isUsageLimitResult(_ result: AgentResult) -> Bool {
        if let providerStopReason = result.providerStopReason?.lowercased(),
            isUsageLimitStopReason(providerStopReason)
        {
            return true
        }
        if let error = result.errorMessage?.lowercased(),
            isUsageLimitStopReason(error)
        {
            return true
        }
        switch result.canonicalOutcome {
        case .limitExhaustedBeforeOutput, .limitExhaustedAfterOutput:
            return true
        default:
            return false
        }
    }

    private func isUsageLimitStopReason(_ value: String) -> Bool {
        value.contains("usage_limit_exceeded")
            || value.contains("usage limit")
            || value.contains("rate limit")
            || value.contains("quota")
            || value.contains("limit exceeded")
            || value.contains("budget exhausted")
    }

    private func shouldAttemptEscalatedExecution(
        primaryModelTier: String?,
        profile: ContextStrategyProfile?
    ) -> Bool {
        let primaryTier = primaryModelTier?.trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()
        let escalationTier = profile?.escalationModelTier?.trimmingCharacters(
            in: .whitespacesAndNewlines
        ).lowercased()
        guard let escalationTier, !escalationTier.isEmpty else { return false }
        return primaryTier != escalationTier
    }

    private func mergedLogSnippet(existing: String?, result: String?) -> String? {
        let existing = existing?.trimmingCharacters(in: .whitespacesAndNewlines)
        let result = result?.trimmingCharacters(in: .whitespacesAndNewlines)

        if let existing, !existing.isEmpty,
            existing != "Session started",
            existing != "Prompt submitted"
        {
            return existing
        }

        guard let result, !result.isEmpty else { return existing }
        return result
    }

    private func applyStrategyExecutionMetadata(
        to agentExec: AgentExecution,
        handoffPacket: HandoffPacket?,
        inputArtifacts: [String: Data],
        profile: ContextStrategyProfile?,
        modelTierUsed: String? = nil
    ) {
        let totalInputBytes = inputArtifacts.values.reduce(0) { $0 + $1.count }
        let summary = handoffPacket?.summaryMetrics
        let payloadBytesBeforeStrategy = summary?.payloadBytesBeforeStrategy ?? totalInputBytes
        let payloadBytesAfterStrategy = summary?.payloadBytesAfterStrategy ?? totalInputBytes
        let payloadReductionBytes = summary?.payloadReductionBytes ?? 0
        let cacheEffectiveness =
            payloadBytesBeforeStrategy > 0
            ? Double(payloadReductionBytes) / Double(payloadBytesBeforeStrategy)
            : nil
        agentExec.inputPayloadBytes = summary?.payloadBytesAfterStrategy ?? totalInputBytes
        agentExec.handoffMode = handoffPacket?.mode.rawValue ?? profile?.defaultHandoffMode.rawValue
        agentExec.modelTierUsed = modelTierUsed ?? profile?.defaultModelTier ?? "bound_runtime"
        agentExec.promotedArtifactNamesJSON = encodeArtifactNameList(
            handoffPacket?.promotedArtifacts ?? [])

        let signals = StrategyLimitPressureSignals(
            inputPayloadBytes: summary?.payloadBytesAfterStrategy ?? totalInputBytes,
            payloadBytesBeforeStrategy: payloadBytesBeforeStrategy,
            payloadBytesAfterStrategy: payloadBytesAfterStrategy,
            payloadReductionBytes: payloadReductionBytes,
            mandatoryArtifactCount: summary?.mandatoryArtifactCount ?? inputArtifacts.count,
            summarizedArtifactCount: summary?.summarizedArtifactCount ?? 0,
            lazyArtifactCount: summary?.lazyArtifactCount ?? 0,
            lazyEvidenceHitCount: 0,
            lazyEvidenceHitRate: summary?.lazyArtifactCount == 0 ? 0.0 : 0.0,
            compactionCount: summary?.compactionCount ?? 0,
            cacheEffectiveness: cacheEffectiveness,
            compactionChurnCount: summary?.compactionCount ?? 0,
            escalationCount: 0,
            retryableEscalationCount: 0,
            contractFailureCount: 0,
            operatorPromotedArtifactCount: handoffPacket?.promotedArtifacts.count ?? 0
        )
        agentExec.limitPressureSignalsJSON = try? JSONEncoder().encode(signals)
    }

    private func finalizeStrategyExecutionMetadata(
        on agentExec: AgentExecution,
        modelTierUsed: String,
        escalationCount: Int,
        retryableEscalationCount: Int,
        lazyEvidenceHitCount: Int
    ) {
        agentExec.modelTierUsed = modelTierUsed
        guard
            let data = agentExec.limitPressureSignalsJSON,
            let signals = try? JSONDecoder().decode(StrategyLimitPressureSignals.self, from: data)
        else {
            return
        }

        let updated = StrategyLimitPressureSignals(
            inputPayloadBytes: signals.inputPayloadBytes,
            payloadBytesBeforeStrategy: signals.payloadBytesBeforeStrategy,
            payloadBytesAfterStrategy: signals.payloadBytesAfterStrategy,
            payloadReductionBytes: signals.payloadReductionBytes,
            mandatoryArtifactCount: signals.mandatoryArtifactCount,
            summarizedArtifactCount: signals.summarizedArtifactCount,
            lazyArtifactCount: signals.lazyArtifactCount,
            lazyEvidenceHitCount: lazyEvidenceHitCount,
            lazyEvidenceHitRate: signals.lazyArtifactCount > 0
                ? Double(lazyEvidenceHitCount) / Double(signals.lazyArtifactCount)
                : 0.0,
            compactionCount: signals.compactionCount,
            cacheEffectiveness: signals.cacheEffectiveness,
            compactionChurnCount: signals.compactionChurnCount,
            escalationCount: escalationCount,
            retryableEscalationCount: retryableEscalationCount,
            contractFailureCount: signals.contractFailureCount,
            operatorPromotedArtifactCount: signals.operatorPromotedArtifactCount
        )
        agentExec.limitPressureSignalsJSON = try? JSONEncoder().encode(updated)
    }

    private func incrementContractFailureCount(on agentExec: AgentExecution) {
        guard
            let data = agentExec.limitPressureSignalsJSON,
            var signals = try? JSONDecoder().decode(StrategyLimitPressureSignals.self, from: data)
        else {
            return
        }
        signals = StrategyLimitPressureSignals(
            inputPayloadBytes: signals.inputPayloadBytes,
            payloadBytesBeforeStrategy: signals.payloadBytesBeforeStrategy,
            payloadBytesAfterStrategy: signals.payloadBytesAfterStrategy,
            payloadReductionBytes: signals.payloadReductionBytes,
            mandatoryArtifactCount: signals.mandatoryArtifactCount,
            summarizedArtifactCount: signals.summarizedArtifactCount,
            lazyArtifactCount: signals.lazyArtifactCount,
            lazyEvidenceHitCount: signals.lazyEvidenceHitCount,
            lazyEvidenceHitRate: signals.lazyEvidenceHitRate,
            compactionCount: signals.compactionCount,
            cacheEffectiveness: signals.cacheEffectiveness,
            compactionChurnCount: signals.compactionChurnCount,
            escalationCount: signals.escalationCount,
            retryableEscalationCount: signals.retryableEscalationCount,
            contractFailureCount: signals.contractFailureCount + 1,
            operatorPromotedArtifactCount: signals.operatorPromotedArtifactCount
        )
        agentExec.limitPressureSignalsJSON = try? JSONEncoder().encode(signals)
    }

    private func encodeArtifactNameList(_ names: [String]) -> Data? {
        try? JSONEncoder().encode(names)
    }

    private func persistFinalFeatureReportIfNeeded(finalStateID: String) {
        guard producedArtifactNames.contains("final_feature_report") == false else { return }

        let report = buildFinalFeatureReport()
        guard
            let data = try? JSONSerialization.data(
                withJSONObject: report, options: [.prettyPrinted, .sortedKeys])
        else {
            return
        }

        let reportProvider =
            run.stageExecutions
            .flatMap(\.agentExecutions)
            .last?.provider ?? "chainworks"
        let reportModel = run.stageExecutions
            .flatMap(\.agentExecutions)
            .last?.artifacts
            .last?.model
        let reportEffort = run.stageExecutions
            .flatMap(\.agentExecutions)
            .last?.effort

        if let artifact = try? artifactManager.persistSystemArtifact(
            name: "final_feature_report",
            data: data,
            contractID: "final_feature_report_v1",
            format: .json,
            workspace: workspace,
            stageID: finalStateID,
            agentID: "system_reporter",
            provider: reportProvider,
            model: reportModel,
            effort: reportEffort,
            attemptNumber: 1
        ) {
            recordArtifactForTransition(artifact, data: data, validatedFields: [:])
        }
    }

    // Proposal 013 §8.2: Update compaction outcome truth on agent execution
    private func updateCompactionOutcome(agentExec: AgentExecution, succeeded: Bool) {
        guard let data = agentExec.compactionMetadataJSON,
            var metadata = try? JSONDecoder().decode(CompactionMetadata.self, from: data)
        else {
            return
        }
        metadata.stageOutcome = succeeded ? .succeededWithCompaction : .failedDespiteCompaction
        agentExec.compactionMetadataJSON = try? JSONEncoder().encode(metadata)
    }

    // Proposal 013 Layer Q: Emit declarative coverage snapshot at run terminal state
    private func persistDeclarativeCoverageIfNeeded(finalStateID: String) {
        guard producedArtifactNames.contains("declarative_coverage_report") == false else { return }
        let report = DeclarativeCoverageReport()
        guard let data = try? JSONEncoder().encode(report) else { return }
        if let artifact = try? artifactManager.persistSystemArtifact(
            name: "declarative_coverage_report",
            data: data,
            contractID: "declarative_coverage_v1",
            format: .json,
            workspace: workspace,
            stageID: finalStateID,
            agentID: "system_reporter",
            provider: "system",
            model: nil,
            effort: nil,
            attemptNumber: 1
        ) {
            producedArtifactNames.insert(artifact.name)
        }
    }

    private func buildFinalFeatureReport() -> [String: Any] {
        let completedAt = run.completedAt ?? Date()
        let durationSeconds = max(0, completedAt.timeIntervalSince(run.startedAt))
        let stageCount = run.stageExecutions.count
        let agentCount = run.stageExecutions.flatMap(\.agentExecutions).count
        let artifactCount = run.stageExecutions.flatMap(\.agentExecutions).flatMap(\.artifacts)
            .count
        let approvalCount = run.approvals.count
        let totalCostUSD = Double(run.totalCostCents ?? 0) / 100.0

        return [
            "final_status": run.status.rawValue,
            "summary":
                "\(run.workflowTitle) completed with \(stageCount) stages, \(agentCount) agent executions, \(artifactCount) artifacts, and \(approvalCount) approval checkpoints.",
            "started_at": ISO8601DateFormatter().string(from: run.startedAt),
            "completed_at": ISO8601DateFormatter().string(from: completedAt),
            "duration_seconds": durationSeconds,
            "total_cost": totalCostUSD,
            "cost_currency": "USD",
        ]
    }

    private func persistDeliveryReceiptIfNeeded(finalStateID: String) {
        guard producedArtifactNames.contains("delivery_receipt") == false else { return }
        guard let releaseResult = currentReleaseResultSummary() else { return }

        let provider =
            run.stageExecutions
            .flatMap(\.agentExecutions)
            .last?.provider ?? "system"
        let model = run.stageExecutions
            .flatMap(\.agentExecutions)
            .last?.resolvedModel
        let effort = run.stageExecutions
            .flatMap(\.agentExecutions)
            .last?.effort

        persistDeliveryReceipt(
            finalStateID: finalStateID,
            provider: provider,
            model: model,
            effort: effort,
            releaseResult: releaseResult
        )
    }

    private func persistDeliveryReceipt(
        finalStateID: String,
        provider: String,
        model: String?,
        effort: String?,
        releaseResult: ReleaseOpsCoordinator.ReleaseResult
    ) {
        guard producedArtifactNames.contains("delivery_receipt") == false else { return }
        guard let deliveryConfigurationJSON = run.deliveryConfigurationJSON,
            let deliveryConfig = try? JSONDecoder().decode(
                DeliveryConfiguration.self, from: deliveryConfigurationJSON),
            let worktreeRoot = run.worktreeRoot
        else {
            return
        }

        let receipt = DeliveryReceiptBuilder.buildReceipt(
            runID: run.id,
            workflowID: run.workflowID,
            ideaTitle: run.idea?.title ?? "Unknown",
            deliveryConfig: deliveryConfig,
            worktreeRoot: worktreeRoot,
            baseRevision: run.baseRevision,
            releaseResult: releaseResult,
            implementationReviewStatus: currentImplementationReviewStatus()
        )

        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        guard let data = try? encoder.encode(receipt) else { return }

        if let artifact = try? artifactManager.persistSystemArtifact(
            name: "delivery_receipt",
            data: data,
            contractID: "delivery_receipt",
            format: .json,
            workspace: workspace,
            stageID: finalStateID,
            agentID: "system_delivery",
            provider: provider,
            model: model,
            effort: effort,
            attemptNumber: 1
        ) {
            recordArtifactForTransition(artifact, data: data, validatedFields: [:])
        }
    }

    private func currentReleaseResultSummary() -> ReleaseOpsCoordinator.ReleaseResult? {
        let artifacts = (try? artifactManager.artifacts(forRunID: run.id)) ?? []

        let gitManifest: GitReleaseService.ReleaseManifest? = decodeArtifact(
            named: "release_manifest",
            from: artifacts
        )
        let gitReceipt: GitReleaseService.GitPushReceipt? = decodeArtifact(
            named: "git_push_receipt",
            from: artifacts
        )
        let bundleManifest: ConnectPublishService.ReleaseBundleManifest? = decodeArtifact(
            named: "release_bundle_manifest",
            from: artifacts
        )
        let uploadReceipt: ConnectPublishService.ConnectUploadReceipt? = decodeArtifact(
            named: "connect_upload_receipt",
            from: artifacts
        )

        let releaseAgents = run.stageExecutions
            .flatMap(\.agentExecutions)
            .filter {
                $0.agentID == "commit_and_push_to_github"
                    || $0.agentID == "build_archive_and_push_connect"
            }
        guard !releaseAgents.isEmpty else { return nil }

        let succeeded = uploadReceipt != nil
        let failureStage: String? = {
            if succeeded { return nil }
            if gitManifest == nil || gitReceipt == nil { return "commit_and_push" }
            return "build_archive_and_push"
        }()
        let failureReason: String? = {
            guard !succeeded else { return nil }
            return releaseAgents.last(where: { $0.status == .failed })?.logSnippet
        }()

        return ReleaseOpsCoordinator.ReleaseResult(
            gitManifest: gitManifest,
            gitReceipt: gitReceipt,
            bundleManifest: bundleManifest,
            uploadReceipt: uploadReceipt,
            succeeded: succeeded,
            failureStage: failureStage,
            failureReason: failureReason
        )
    }

    private func currentImplementationReviewStatus() -> String? {
        let artifacts = (try? artifactManager.artifacts(forRunID: run.id)) ?? []
        if let artifact = artifacts.last(where: { $0.name == "implementation_review_summary" }),
            let data = try? artifactManager.readArtifact(artifact, workspace: workspace),
            let fields = tryExtractScalarFields(from: data)
        {
            if case .string(let decision)? = fields["decision"] {
                return decision
            }
            if case .string(let status)? = fields["status"] {
                return status
            }
            if case .bool(let pass)? = fields["pass"] {
                return pass ? "pass" : "fail"
            }
        }

        if let fields = artifactFields["implementation_review_summary"] {
            if case .string(let decision)? = fields["decision"] {
                return decision
            }
            if case .string(let status)? = fields["status"] {
                return status
            }
            if case .bool(let pass)? = fields["pass"] {
                return pass ? "pass" : "fail"
            }
        }
        return producedArtifactNames.contains("implementation_review_summary") ? "available" : nil
    }

    private func decodeArtifact<T: Decodable>(named name: String, from artifacts: [Artifact]) -> T?
    {
        guard let artifact = artifacts.last(where: { $0.name == name }),
            let data = try? artifactManager.readArtifact(artifact, workspace: workspace)
        else {
            return nil
        }
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return try? decoder.decode(T.self, from: data)
    }

    private func healPrematureBlockedStateIfNeeded() {
        guard run.status == .blocked, !isCancelled, !isPaused else { return }

        run.status = .running
        run.completedAt = nil

        if let details = run.driftDetails,
            details.localizedCaseInsensitiveContains("execution stalled after")
        {
            run.driftDetails = nil
        }
    }
}
