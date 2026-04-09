import Foundation
import SwiftData
import Observation
import CryptoKit

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
        self.providerBindingsByAgentID = Self.decodeProviderBindings(from: run.providerBindingSnapshotJSON)
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
                // Legacy run being resumed — seed cursor reflecting that the
                // orchestrator is about to start executing `resumeState`.
                run.persistTransitionCursor(TransitionCursor.seededForResume(
                    nextScheduledStateID: resumeState
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

        // Resume if we were waiting
        if isPaused && granted {
            isPaused = false
            isRunning = true
            run.status = .running

            // Mark the stage execution as completed
            if let stageExec = run.stageExecutions.first(where: { $0.stageID == stageID && $0.status == .waitingApproval }) {
                stageExec.status = .completed
                stageExec.completedAt = Date()
            }

            // Evaluate transitions from the approved state and resume
            Task { @MainActor in
                await resumeAfterApproval(stageID: stageID)
            }
        } else if !granted {
            // Rejection: cancel the run (proposal contract — rejection cancels, not fails)
            run.status = .cancelled
            isCancelled = true
            isRunning = false
            onComplete?(false)
        }
    }

    /// Resume execution after an approval is granted.
    private func resumeAfterApproval(stageID: String) async {
        guard let state = plan.states[stageID] else {
            RuntimeDiagnostics.log("resumeAfterApproval missingState stageID=\(stageID)")
            return
        }
        RuntimeDiagnostics.log("resumeAfterApproval begin stageID=\(stageID)")

        // Execute run_after_approval block if present
        if let runAfterApproval = state.runAfterApproval {
            if let stageExec = run.stageExecutions.first(where: { $0.stageID == stageID }) {
                let success = await executeRunBlock(runAfterApproval, state: state, stageExec: stageExec)
                if !success {
                    RuntimeDiagnostics.log("resumeAfterApproval runAfterApprovalFailed stageID=\(stageID)")
                    handleFailure(state: state)
                    return
                }
            }
        }

        // Evaluate transitions from the approved state
        let context = TransitionEvaluator.EvaluationContext(
            producedArtifactNames: producedArtifactNames,
            approvalGranted: true,
            variables: runtimeVariables,
            artifactFields: artifactFields
        )

        guard let transition = TransitionEvaluator.evaluateFirst(
            transitions: state.transitions,
            context: context
        ) else {
            RuntimeDiagnostics.log("resumeAfterApproval noTransition stageID=\(stageID)")
            run.status = .blocked
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
            RuntimeDiagnostics.log("resumeAfterApproval settlementFailed stageID=\(stageID) to=\(transition.to)")
            run.status = .blocked
            run.driftDetails = "Transition settlement failed after approval: could not durably persist continuation from '\(stageID)' to '\(transition.to)'. Manual resume required."
            isRunning = false
            onComplete?(false)
            return
        }

        RuntimeDiagnostics.log("resumeAfterApproval transition stageID=\(stageID) to=\(transition.to)")
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
            case .failed:
                handleFailure(state: state)
                return
            case .succeeded:
                if state.type == .end {
                    RuntimeDiagnostics.log("executeStateMachine completedTerminalEndState stateID=\(state.id)")
                    run.status = .completed
                    run.completedAt = Date()
                    settleTerminal(lastCompletedStateID: state.id, lastCompletedStageExec: run.stageExecutions.last { $0.stageID == state.id && $0.status == .completed })
                    persistDeliveryReceiptIfNeeded(finalStateID: state.id)
                    persistFinalFeatureReportIfNeeded(finalStateID: state.id)
                    persistDeclarativeCoverageIfNeeded(finalStateID: state.id)
                    isRunning = false
                    onComplete?(true)
                    return
                }
                break // Continue to transition evaluation
            }

            // Evaluate transitions
            let context = TransitionEvaluator.EvaluationContext(
                producedArtifactNames: producedArtifactNames,
                approvalGranted: isApprovalGranted(for: state.id),
                variables: runtimeVariables,
                artifactFields: artifactFields
            )

            guard let transition = TransitionEvaluator.evaluateFirst(
                transitions: state.transitions,
                context: context
            ) else {
                // No transition matches — check if we should wait (approval) or fail
                if state.approvalRequired && !isApprovalGranted(for: state.id) {
                    // Already handled in executeState — should be paused
                    return
                }
                // Dead end — mark complete if no transitions defined
                if state.transitions.isEmpty {
                    RuntimeDiagnostics.log("executeStateMachine deadEndComplete stateID=\(state.id)")
                    run.status = .completed
                    run.completedAt = Date()
                    settleTerminal(lastCompletedStateID: state.id, lastCompletedStageExec: run.stageExecutions.last { $0.stageID == state.id && $0.status == .completed })
                    persistDeliveryReceiptIfNeeded(finalStateID: state.id)
                    isRunning = false
                    onComplete?(true)
                    return
                }
                // Otherwise, stalled
                RuntimeDiagnostics.log("executeStateMachine blockedNoTransition stateID=\(state.id) artifacts=\(producedArtifactNames.sorted())")
                run.status = .blocked
                settleTerminal()
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
                RuntimeDiagnostics.log("executeStateMachine settlementFailed from=\(state.id) to=\(transition.to)")
                run.status = .blocked
                run.driftDetails = "Transition settlement failed: could not durably persist continuation from '\(state.id)' to '\(transition.to)'. Manual resume required."
                isRunning = false
                onComplete?(false)
                return
            }

            RuntimeDiagnostics.log("executeStateMachine transition from=\(state.id) to=\(transition.to)")
            currentStateID = transition.to
        }
    }

    // MARK: - State Execution Result

    private enum StateResult {
        case succeeded
        case failed
        case paused // waiting for approval
    }

    private func stateHasExecutableWork(_ state: ExecutableState) -> Bool {
        state.runBlock != nil || state.runAfterApproval != nil || state.approvalRequired
    }

    // MARK: - Execute State

    /// Execute a single state: run block, then check approval.
    private func executeState(_ state: ExecutableState) async -> StateResult {
        RuntimeDiagnostics.log("executeState begin stateID=\(state.id)")

        // Proposal 032: Mark the transition as started once agent work begins.
        if let cursor = run.transitionCursor,
           cursor.settlementPhase == .transitionSettled,
           cursor.nextScheduledStateID == state.id {
            run.persistTransitionCursor(cursor.markingTransitionStarted())
        }

        let stageExec: StageExecution
        if let resumableStage = resumableStageExecution(for: state.id) {
            stageExec = resumableStage
            stageExec.status = .running
            stageExec.completedAt = nil
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
        }

        // Proposal 007: Provision worktree before executing implementation states
        do {
            try await provisionWorktreeIfNeeded(for: state)
        } catch {
            RuntimeDiagnostics.log("executeState worktreeProvisioningFailed stateID=\(state.id) error=\(error.localizedDescription)")
            stageExec.status = .failed
            stageExec.completedAt = Date()
            stageExec.label = "\(state.label) — worktree provisioning failed: \(error.localizedDescription)"
            return .failed
        }

        // Execute run block
        if let runBlock = state.runBlock {
            let blockSuccess = await executeRunBlock(runBlock, state: state, stageExec: stageExec)
            if !blockSuccess {
                RuntimeDiagnostics.log("executeState runBlockFailed stateID=\(state.id)")
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
            RuntimeDiagnostics.log("executeState waitingApproval stateID=\(state.id) policy=\(state.approvalPolicy ?? "nil")")
            return .paused // Will resume when approval is resolved
        }

        // Execute run_after_approval block (if approval was already granted on resume)
        if let runAfterApproval = state.runAfterApproval {
            let afterSuccess = await executeRunBlock(runAfterApproval, state: state, stageExec: stageExec)
            if !afterSuccess {
                RuntimeDiagnostics.log("executeState runAfterApprovalFailed stateID=\(state.id)")
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
                return .succeeded // Let transition evaluator decide next state
            }

            // Update runtime variable for expression evaluation
            runtimeVariables[loop.counter] = .int(newCount)
        }

        stageExec.status = .completed
        stageExec.completedAt = Date()
        RuntimeDiagnostics.log("executeState completed stateID=\(state.id)")
        return .succeeded
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

        // Gather input artifacts
        let inputData: [String: Data]
        do {
            inputData = try await gatherExecutionInputs(for: task, agent: agent)
        } catch {
            agentExec.status = .failed
            agentExec.completedAt = Date()
            agentExec.logSnippet = "Source context preparation failed: \(error.localizedDescription)"
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
        agentExec.consumedInputArtifactNamesJSON = encodeArtifactNameList(Array(inputData.keys).sorted())
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
        let primaryExecutionBinding = strategyAdjustedBinding(
            for: agent,
            baseBinding: providerBindingsByAgentID[agent.id],
            modelTier: contextStrategyProfile?.defaultModelTier
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
                requestedTier: contextStrategyProfile?.defaultModelTier,
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
            handoffPacket: handoffPacket
        )

        // Proposal 007 REQ-008 / REQ-011: Route release agents through ReleaseOpsCoordinator
        // for delivery-configured runs instead of the generic executor path.
        if let config = deliveryConfig,
           (agent.id == "commit_and_push_to_github" || agent.id == "build_archive_and_push_connect") {
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
            requestedTier: contextStrategyProfile?.defaultModelTier,
            effectiveAgent: primaryExecutionAgent
        )
        var escalationCount = 0
        var retryableEscalationCount = 0
        do {
            result = try await executor.execute(task: task, agent: primaryExecutionAgent, context: execContext)
            if shouldEscalateStrategyExecution(
                result: result,
                profile: contextStrategyProfile
            ) {
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
                    handoffPacket: handoffPacket
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
        agentExec.actualAdapterFamily = settlementBinding?.adapterFamily ?? "goose"
        agentExec.actualCapabilityClass = settlementBinding?.capabilityClass?.rawValue ?? "legacy_operator_grade"
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
                let (artifacts, rawEnvelopes) = try ArtifactPersistenceOrderingPolicy.persistRawOutputs(
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
                        stageID: state.id, iteration: stageExec.iteration, attemptNumber: stageExec.attemptNumber
                    )
                }

                // Step 3: Check for validation failures
                let failedResults = validationResults.values.filter { $0.status == OutputValidationStatus.failed }
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
                        rawErrorMessage: failedResults.compactMap(\.validationError).joined(separator: "; "),
                        rawFinishEvent: nil
                    )
                    let validationMessages = failedResults.compactMap { $0.validationError }
                    agentExec.logSnippet = "Output contract validation failed: \(validationMessages.joined(separator: "; "))"

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

                // Update tracking for transition evaluation
                for artifact in artifacts {
                    producedArtifactNames.insert(artifact.name)

                    if let fields = validatedFields[artifact.name] {
                        artifactFields[artifact.name] = fields
                    } else if artifact.format == .json,
                              let data = result.outputs[artifact.name],
                              let fields = tryExtractScalarFields(from: data) {
                        artifactFields[artifact.name] = fields
                    }

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
                agentExec.logSnippet = "Output validation or persistence error: \(error.localizedDescription)"
                applyTerminalExecutionTruth(
                    to: agentExec,
                    canonicalOutcome: result.outputPresence == .durableOutput ? .failedAfterOutputValidation : .failedBeforeOutput,
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
            if !result.outputs.isEmpty {
                do {
                    let (artifacts, envelopes) = try ArtifactPersistenceOrderingPolicy.persistRawOutputs(
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
                            stageID: state.id, iteration: stageExec.iteration, attemptNumber: stageExec.attemptNumber
                        )
                    }

                    if shouldTreatAfterOutputFailureAsRecoveredSuccess(result) {
                        var recoveredEnvelopes = envelopes
                        let contractOutputs = filteredContractOutputs(
                            from: result.outputs,
                            task: task,
                            agent: agent
                        )
                        let validationResults = ArtifactPersistenceOrderingPolicy.validatePersistedOutputs(
                            outputs: contractOutputs,
                            agent: agent,
                            catalog: catalog,
                            envelopes: &recoveredEnvelopes
                        )
                        let failedResults = validationResults.values.filter { $0.status == OutputValidationStatus.failed }

                        if failedResults.isEmpty {
                            agentExec.outputEnvelopesJSON = try? JSONEncoder().encode(recoveredEnvelopes)
                            let validatedFields = try validateStructuredOutputs(
                                result.outputs,
                                for: task,
                                agent: agent
                            )

                            for artifact in artifacts {
                                producedArtifactNames.insert(artifact.name)

                                if let fields = validatedFields[artifact.name] {
                                    artifactFields[artifact.name] = fields
                                } else if artifact.format == .json,
                                          let data = result.outputs[artifact.name],
                                          let fields = tryExtractScalarFields(from: data) {
                                    artifactFields[artifact.name] = fields
                                }

                                if artifact.name.hasSuffix("_transcript.md") {
                                    agentExec.transcriptArtifactPath = artifact.filePath
                                    agentExec.transcriptPath = artifact.filePath
                                }
                            }

                            if let cost = result.costCents {
                                run.totalCostCents = (run.totalCostCents ?? 0) + cost
                            }

                            agentExec.status = .completed
                            agentExec.retryReason = nil
                            agentExec.logSnippet = mergedLogSnippet(
                                existing: agentExec.logSnippet,
                                result: "Recovered after transport failure because durable outputs validated"
                            )
                            updateCompactionOutcome(agentExec: agentExec, succeeded: true)
                            unregisterLiveExecution(agentExec, for: agent.id)
                            return true
                        }
                    }

                    persistStageFailureEvidence(
                        stageExec: stageExec,
                        failedAgentExec: agentExec,
                        validationFailure: nil,
                        additionalEnvelopes: envelopes
                    )
                } catch {
                    agentExec.logSnippet = mergedLogSnippet(
                        existing: agentExec.logSnippet,
                        result: "Raw failure outputs could not be persisted: \(error.localizedDescription)"
                    )
                }
            } else {
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
            return false
        }
    }

    private func capturePersistedExecutionEvidence(from artifacts: [Artifact], for agentExec: AgentExecution) {
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
        let canonicalOutcome = result.canonicalOutcome ?? (result.succeeded ? .completed : .failedBeforeOutput)
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
        let validatedState: Data? = agentExec.outputEnvelopesJSON ?? checkpoint.lastValidatedAggregateStateJSON

        let enrichedCheckpoint = AgentSessionCheckpoint(
            machineSummary: checkpoint.machineSummary,
            nextSteps: checkpoint.nextSteps,
            durableLearnings: checkpoint.durableLearnings,
            unresolvedBlockers: checkpoint.unresolvedBlockers,
            openDecisions: checkpoint.openDecisions,
            openQuestions: checkpoint.openQuestions,
            unresolvedConstraints: checkpoint.unresolvedConstraints,
            selectedArtifactReferences: artifactIDs.isEmpty ? checkpoint.selectedArtifactReferences : artifactIDs,
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

    private func decodeOutputEnvelopes(from agentExec: AgentExecution) -> [StructuredOutputEnvelope] {
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
                envelope.sessionID ?? "no-session"
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
            RuntimeDiagnostics.log("executeReleaseAgentTask missingWorktree agentID=\(agent.id) stateID=\(state.id)")
            agentExec.status = .failed
            agentExec.completedAt = Date()
            agentExec.logSnippet = "Release agent requires a provisioned worktree but none is available."
            unregisterLiveExecution(agentExec, for: agent.id)
            stageExec.status = .failed
            return false
        }

        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]

        // Build commit message from approved_proposal artifact name
        let proposalName = producedArtifactNames.first(where: { $0.contains("proposal") }) ?? "approved_proposal"
        let commitMessage = "[\(deliveryConfig.repoIdentifier)] Apply \(proposalName) via Chainworks Forge"

        if agent.id == "commit_and_push_to_github" {
            let gitService = GitReleaseService()
            do {
                RuntimeDiagnostics.log("executeReleaseAgentTask begin agentID=\(agent.id) branch=\(deliveryConfig.targetBranch) worktree=\(worktreeRoot.path)")
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
                agentExec.logSnippet = "GitReleaseService: commit \(manifest.commitSHA.prefix(8)) pushed to \(manifest.branch)"
                RuntimeDiagnostics.log("executeReleaseAgentTask success agentID=\(agent.id) branch=\(manifest.branch)")
                unregisterLiveExecution(agentExec, for: agent.id)
                return true
            } catch {
                RuntimeDiagnostics.log("executeReleaseAgentTask failure agentID=\(agent.id) error=\(error.localizedDescription)")
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
                  let manifestData = inputData["release_manifest"] else {
                agentExec.status = .failed
                agentExec.completedAt = Date()
                agentExec.logSnippet = "ConnectPublishService requires git_push_receipt and release_manifest inputs."
                stageExec.status = .failed
                unregisterLiveExecution(agentExec, for: agent.id)
                return false
            }

            let decoder = JSONDecoder()
            decoder.dateDecodingStrategy = .iso8601
            guard let gitReceipt = try? decoder.decode(GitReleaseService.GitPushReceipt.self, from: receiptData),
                  let releaseManifest = try? decoder.decode(GitReleaseService.ReleaseManifest.self, from: manifestData) else {
                agentExec.status = .failed
                agentExec.completedAt = Date()
                agentExec.logSnippet = "ConnectPublishService received invalid release inputs."
                stageExec.status = .failed
                unregisterLiveExecution(agentExec, for: agent.id)
                return false
            }
            do {
                RuntimeDiagnostics.log("executeReleaseAgentTask begin agentID=\(agent.id) target=\(deliveryConfig.releaseTargetID) mode=\(deliveryConfig.releaseMode.rawValue)")
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
                agentExec.logSnippet = "ConnectPublishService: bundle \(bundle.bundleVersion) uploaded to \(uploadReceipt.destination)"
                RuntimeDiagnostics.log("executeReleaseAgentTask success agentID=\(agent.id) destination=\(uploadReceipt.destination)")
                unregisterLiveExecution(agentExec, for: agent.id)
                return true
            } catch {
                RuntimeDiagnostics.log("executeReleaseAgentTask failure agentID=\(agent.id) error=\(error.localizedDescription)")
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
        var taskAgentPairs: [(task: AgentTask, agent: ResolvedAgent, agentExec: AgentExecution)] = []

        for task in tasks {
            guard let resolvedAgent = plan.agentBindings[task.agent] else { continue }
            let agent = effectiveAgent(from: resolvedAgent)

            let agentExec: AgentExecution
            if let resumableAgent = resumableAgentExecution(for: task, agent: agent, in: stageExec) {
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

        // Execute all in parallel
        let results = await withTaskGroup(of: (Int, AgentResult?).self) { group in
            for (index, pair) in taskAgentPairs.enumerated() {
                let gatheredInputs: [String: Data]
                do {
                    gatheredInputs = try await gatherExecutionInputs(for: pair.task, agent: pair.agent)
                } catch {
                    pair.agentExec.consumedInputArtifactNamesJSON = encodeArtifactNameList([])
                    pair.agentExec.inputBindingsJSON = buildInputBindings(for: pair.task)
                    group.addTask {
                        (
                            index,
                            AgentResult(
                                outputs: [:],
                                logSnippet: nil,
                                costCents: nil,
                                succeeded: false,
                                errorMessage: "Source context preparation failed: \(error.localizedDescription)",
                                sessionID: nil,
                                durationSeconds: 0,
                                providerReceipt: nil,
                                resolvedModel: nil,
                                configuredProviderID: nil,
                                adapterVersion: nil
                            )
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
                let primaryExecutionBinding = strategyAdjustedBinding(
                    for: agent,
                    baseBinding: providerBindingsByAgentID[agent.id],
                    modelTier: contextStrategyProfile?.defaultModelTier
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
                        requestedTier: contextStrategyProfile?.defaultModelTier,
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
                    handoffPacket: handoffPacket
                )
                let executor = self.executor
                pair.agentExec.consumedInputArtifactNamesJSON = encodeArtifactNameList(Array(gatheredInputs.keys).sorted())
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

        for (index, optResult) in results {
            let pair = taskAgentPairs[index]
            let agentExec = pair.agentExec
            let agent = pair.agent
            agentExec.resolvedBackendProfileID = agent.backendProfileID

            agentExec.completedAt = Date()

            if let result = optResult {
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
                agentExec.actualAdapterFamily = settlementBinding?.adapterFamily ?? "goose"
                agentExec.actualCapabilityClass = settlementBinding?.capabilityClass?.rawValue ?? "legacy_operator_grade"

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
                    modelTierUsed: agentExec.modelTierUsed ?? self.contextStrategyProfile?.defaultModelTier ?? "bound_runtime",
                    escalationCount: 0,
                    retryableEscalationCount: 0,
                    lazyEvidenceHitCount: result.lazyEvidenceArtifactHits.count
                )
            }

            guard let result = optResult else {
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
                if !result.outputs.isEmpty {
                    do {
                        let (artifacts, envelopes) = try ArtifactPersistenceOrderingPolicy.persistRawOutputs(
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
                            let validationResults = ArtifactPersistenceOrderingPolicy.validatePersistedOutputs(
                                outputs: contractOutputs,
                                agent: agent,
                                catalog: catalog,
                                envelopes: &recoveredEnvelopes
                            )
                            let failedResults = validationResults.values.filter { $0.status == OutputValidationStatus.failed }

                            if failedResults.isEmpty {
                                agentExec.outputEnvelopesJSON = try? JSONEncoder().encode(recoveredEnvelopes)
                                let validatedFields = try validateStructuredOutputs(
                                    result.outputs,
                                    for: pair.task,
                                    agent: agent
                                )

                                for artifact in artifacts {
                                    producedArtifactNames.insert(artifact.name)

                                    if let fields = validatedFields[artifact.name] {
                                        artifactFields[artifact.name] = fields
                                    } else if artifact.format == .json,
                                              let data = result.outputs[artifact.name],
                                              let fields = tryExtractScalarFields(from: data) {
                                        artifactFields[artifact.name] = fields
                                    }

                                    if artifact.name.hasSuffix("_transcript.md") {
                                        agentExec.transcriptArtifactPath = artifact.filePath
                                        agentExec.transcriptPath = artifact.filePath
                                    }
                                }

                                if let cost = result.costCents {
                                    run.totalCostCents = (run.totalCostCents ?? 0) + cost
                                }

                                agentExec.status = .completed
                                agentExec.retryReason = nil
                                agentExec.logSnippet = mergedLogSnippet(
                                    existing: agentExec.logSnippet,
                                    result: "Recovered after transport failure because durable outputs validated"
                                )
                                updateCompactionOutcome(agentExec: agentExec, succeeded: true)
                                unregisterLiveExecution(agentExec, for: agent.id)
                                continue
                            }
                        }

                        persistStageFailureEvidence(
                            stageExec: stageExec,
                            failedAgentExec: agentExec,
                            validationFailure: nil,
                            additionalEnvelopes: envelopes
                        )
                    } catch {
                        agentExec.logSnippet = mergedLogSnippet(
                            existing: agentExec.logSnippet,
                            result: "Raw failure outputs could not be persisted: \(error.localizedDescription)"
                        )
                    }
                } else {
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
                allSucceeded = false
                continue
            }

            do {
                let (artifacts, rawEnvelopes) = try ArtifactPersistenceOrderingPolicy.persistRawOutputs(
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

                let failedResults = validationResults.values.filter { $0.status == OutputValidationStatus.failed }
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
                        rawErrorMessage: failedResults.compactMap(\.validationError).joined(separator: "; "),
                        rawFinishEvent: nil
                    )
                    let validationMessages = failedResults.compactMap { $0.validationError }
                    agentExec.logSnippet = "Output contract validation failed: \(validationMessages.joined(separator: "; "))"

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
                    producedArtifactNames.insert(artifact.name)
                    if let fields = validatedFields[artifact.name] {
                        artifactFields[artifact.name] = fields
                    } else if artifact.format == .json,
                              let data = result.outputs[artifact.name],
                              let fields = tryExtractScalarFields(from: data) {
                        artifactFields[artifact.name] = fields
                    }

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
                agentExec.logSnippet = "Output validation or persistence error: \(error.localizedDescription)"
                applyTerminalExecutionTruth(
                    to: agentExec,
                    canonicalOutcome: result.outputPresence == .durableOutput ? .failedAfterOutputValidation : .failedBeforeOutput,
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

        return allSucceeded
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
        RuntimeDiagnostics.log("worktreeProvisioned stateID=\(state.id) branch=\(result.branchName) root=\(result.worktreeRoot.path)")
    }

    // MARK: - Helpers

    private func currentIteration(for stageID: String) -> Int {
        guard !run.isDeleted, run.modelContext != nil else { return 1 }
        let existing = run.stageExecutions.filter { $0.stageID == stageID }
        return existing.count + 1
    }

    private func resumableStageExecution(for stageID: String) -> StageExecution? {
        run.stageExecutions
            .filter { $0.stageID == stageID && ($0.status == .running || $0.status == .ready) }
            .sorted {
                if $0.iteration != $1.iteration { return $0.iteration < $1.iteration }
                if $0.attemptNumber != $1.attemptNumber { return $0.attemptNumber < $1.attemptNumber }
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
              let profile else {
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
        let effectiveScope = Self.effectiveSessionReuseScope(for: agent, profile: contextStrategyProfile)
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
           !frozenPath.isEmpty {
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

    private func loadPersistedArtifacts() {
        let artifacts = persistedArtifacts()

        producedArtifactNames = Set(artifacts.map(\.name))

        for artifact in artifacts where artifact.format == .json {
            guard let data = try? artifactManager.readArtifact(artifact, workspace: workspace),
                  let fields = tryExtractScalarFields(from: data) else {
                continue
            }
            artifactFields[artifact.name] = fields
        }
    }

    private func reconcileLateMaterializedOutputsIfNeeded() {
        guard let state = plan.states[currentStateID],
              let stageExec = resumableStageExecution(for: currentStateID) else {
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
                  latestExec.status != .completed else {
                continue
            }

            for candidateExec in matchingExecutions.reversed() {
                guard let recovered = recoverLatePersistedContractOutputs(
                    for: candidateExec,
                    latestExecution: latestExec,
                    task: task,
                    agent: agent,
                    stageExec: stageExec,
                    state: state
                ), recovered else {
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
            if let existingArtifact = candidateExec.artifacts.last(where: { $0.name == outputName }),
               ArtifactStorage.exists(filePath: existingArtifact.filePath),
               let data = try? artifactManager.readArtifact(existingArtifact, workspace: currentWorkspace) {
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
                  ) else {
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
            guard let importedArtifacts = try? artifactManager.persistOutputs(
                outputs: missingPersistedOutputs,
                agent: agent,
                agentExecution: candidateExec,
                workspace: currentWorkspace,
                stageID: state.id,
                iteration: stageExec.iteration,
                attemptNumber: stageExec.attemptNumber,
                catalog: catalog
            ) else {
                return false
            }
            capturePersistedExecutionEvidence(from: importedArtifacts, for: candidateExec)
        }

        candidateExec.outputEnvelopesJSON = try? JSONEncoder().encode(envelopes)
        candidateExec.validationFailureJSON = nil
        candidateExec.status = .completed
        candidateExec.completedAt = candidateExec.completedAt ?? Date()
        candidateExec.retryReason = nil
        candidateExec.logSnippet = mergedLogSnippet(
            existing: candidateExec.logSnippet,
            result: "Recovered from late materialized contract outputs on disk"
        )
        applyTerminalExecutionTruth(
            to: candidateExec,
            canonicalOutcome: .completedWithTransportError,
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
            latestExecution.retryReason = nil
            latestExecution.logSnippet = mergedLogSnippet(
                existing: latestExecution.logSnippet,
                result: "Skipped because a prior attempt recovered valid contract outputs"
            )
            applyTerminalExecutionTruth(
                to: latestExecution,
                canonicalOutcome: .completedWithTransportError,
                transportErrorKind: latestExecution.transportErrorKind,
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
            producedArtifactNames.insert(artifact.name)
            if let fields = validatedFields[artifact.name] {
                artifactFields[artifact.name] = fields
            } else if artifact.format == .json,
                      let data = try? artifactManager.readArtifact(artifact, workspace: currentWorkspace),
                      let fields = tryExtractScalarFields(from: data) {
                artifactFields[artifact.name] = fields
            }
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
            directory = directory.appendingPathComponent("agent-retry-\(agentAttemptNumber)", isDirectory: true)
        }

        return directory.appendingPathComponent(name).path
    }

    private func restorePendingApprovalIfNeeded(for stateID: String) -> Bool {
        guard run.status == .waitingApproval,
              let state = plan.states[stateID],
              state.approvalRequired else {
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
        if let existing = run.approvals.first(where: { $0.stageID == state.id && $0.decision == .requested }) {
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
              let worktreeRoot = currentWorkspace.worktreeRoot else {
            return inputs
        }

        let sourceContext = try await SourceContextBuilder.build(
            worktreeRoot: worktreeRoot,
            repoRoot: config.repoRoot,
            baseBranch: config.baseBranch,
            baseRevision: run.baseRevision,
            targetBranch: config.targetBranch
        )

        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]

        if let data = try? encoder.encode(sourceContext) {
            inputs["source_context"] = data
        }

        if !sourceContext.diffSummary.isEmpty,
           let data = sourceContext.diffSummary.data(using: .utf8) {
            inputs["source_diff_summary"] = data
        }

        if (agent.worktreeWriteEnabled || plan.requiresProjectAccess),
           !sourceContext.changedFilesManifest.isEmpty,
           let data = sourceContext.changedFilesManifest.joined(separator: "\n").data(using: .utf8) {
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
            bindings.append(InputBinding(
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
            guard let contractID = OutputContractResolverV2.resolveContractID(
                for: outputName,
                agent: agent,
                catalog: catalog
            ),
            let schema = OutputContractResolverV2.resolveSchema(for: outputName, agent: agent, catalog: catalog),
            schema.machineFormat == .json else {
                continue
            }

            // Proposal 013 §4.3: Skip strict field validation for structured_with_human_companion
            // contracts — the V2 validation has already accepted the output.
            if schema.validationMode == .structuredWithHumanCompanion {
                // Try JSON extraction for transition evaluation, but don't throw on failure
                if let fields = tryExtractScalarFields(from: data) {
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

            validated[outputName] = scalarFields(from: json)
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
            try requireString(json["agent_id"], field: "agent_id", agentID: agentID, contractID: contractID, outputName: outputName)
            try requireString(json["role"], field: "role", agentID: agentID, contractID: contractID, outputName: outputName)
            try requireNumber(json["score"], field: "score", agentID: agentID, contractID: contractID, outputName: outputName)
            try requireString(json["decision"], field: "decision", agentID: agentID, contractID: contractID, outputName: outputName)
            try requireString(json["verdict"], field: "verdict", agentID: agentID, contractID: contractID, outputName: outputName)
            try requireString(json["summary"], field: "summary", agentID: agentID, contractID: contractID, outputName: outputName)
            // Allow empty arrays or nulls for these fields to avoid failures with codex-like models
            if json["issues"] != nil { try requireArray(json["issues"], field: "issues", agentID: agentID, contractID: contractID, outputName: outputName) }
            if json["blocking_issues"] != nil { try requireArray(json["blocking_issues"], field: "blocking_issues", agentID: agentID, contractID: contractID, outputName: outputName) }
            if json["non_blocking_issues"] != nil { try requireArray(json["non_blocking_issues"], field: "non_blocking_issues", agentID: agentID, contractID: contractID, outputName: outputName) }
            if json["suggestions"] != nil { try requireArray(json["suggestions"], field: "suggestions", agentID: agentID, contractID: contractID, outputName: outputName) }
            if json["assumptions"] != nil { try requireArray(json["assumptions"], field: "assumptions", agentID: agentID, contractID: contractID, outputName: outputName) }
        case "proposal_review_summary_v1":
            try requireBool(json["pass"], field: "pass", agentID: agentID, contractID: contractID, outputName: outputName)
            try requireNumber(json["average_score"], field: "average_score", agentID: agentID, contractID: contractID, outputName: outputName)
            try requireNumber(json["aggregate_score"], field: "aggregate_score", agentID: agentID, contractID: contractID, outputName: outputName)
            try requireNumber(json["min_individual_score"], field: "min_individual_score", agentID: agentID, contractID: contractID, outputName: outputName)
            try requireInt(json["blocker_count"], field: "blocker_count", agentID: agentID, contractID: contractID, outputName: outputName)
            try requireString(json["summary"], field: "summary", agentID: agentID, contractID: contractID, outputName: outputName)
            try requireArray(json["required_changes"], field: "required_changes", agentID: agentID, contractID: contractID, outputName: outputName)
            try requireArray(json["recurring_themes"], field: "recurring_themes", agentID: agentID, contractID: contractID, outputName: outputName)
            try requireString(json["decision"], field: "decision", agentID: agentID, contractID: contractID, outputName: outputName)
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
           floor(number.doubleValue) == number.doubleValue {
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

    private func tryExtractScalarFields(from data: Data) -> [String: AnyCodableValue]? {
        guard let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return nil
        }
        return scalarFields(from: json)
    }

    private func scalarFields(from json: [String: Any]) -> [String: AnyCodableValue] {
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
        return fields
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
            RuntimeDiagnostics.log("settleTransition seq=\(newCursor.sequenceNumber) completed=\(completedStateID) next=\(nextStateID)")
            return true
        } catch {
            RuntimeDiagnostics.log("settleTransition FAILED seq=\(newCursor.sequenceNumber) completed=\(completedStateID) next=\(nextStateID) error=\(error.localizedDescription)")
            return false
        }
    }

    /// Mark the cursor as terminal (completed, failed, blocked, or cancelled).
    private func settleTerminal(
        lastCompletedStateID: String? = nil,
        lastCompletedStageExec: StageExecution? = nil
    ) {
        let currentCursor = run.transitionCursor ?? .initial()
        let newCursor = currentCursor.markingTerminal(
            lastCompletedStateID: lastCompletedStateID,
            lastCompletedStageExecutionID: lastCompletedStageExec?.id
        )
        run.persistTransitionCursor(newCursor)
    }

    // MARK: - Failure Handling

    private func handleFailure(state: ExecutableState) {
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
            agent.outputContract ?? ""
        ].joined(separator: "|")
        return DefinitionHasher.hashString(canonical)
    }

    private static func applySkillMetadata(to agentExecution: AgentExecution, from agent: ResolvedAgent) {
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
        guard let gooseExecutor = executor as? RuntimeAgentExecutor else { return }

        gooseExecutor.onExecutionEvent = { [weak self] agentID, event in
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
               now.timeIntervalSince(lastRoutedAt) < Self.liveTextChunkCoalescingWindow {
                return false
            }
            lastRoutedLiveTextChunkAtByAgentID[agentID] = now
            return true
        default:
            lastRoutedLiveTextChunkAtByAgentID.removeValue(forKey: agentID)
            return true
        }
    }

    func injectTestingLiveExecutionEvent(agentID: String, event: ExecutionEvent, now: Date = Date()) {
        recordLiveExecutionEvent(agentID: agentID, event: event, now: now)
    }

    private func recordLiveExecutionEvent(agentID: String, event: ExecutionEvent, now: Date = Date()) {
        if event.type == .textChunk {
            guard let visibleChunk = visibleLiveTextChunk(agentID: agentID, chunk: event.detail) else { return }
            bufferLiveTextChunk(agentID: agentID, chunk: visibleChunk)
            guard shouldRecordLiveExecutionEvent(agentID: agentID, event: event, now: now) else { return }
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
        guard shouldRecordLiveExecutionEvent(agentID: agentID, event: event, now: now) else { return }
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
           let lastIndex = liveTimeline.lastIndex(where: { $0.agentID == agentID && $0.event.type == .textChunk }) {
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
        if let reportJSON = SessionLineageReportBridge.generateReportJSON(for: run.id, context: modelContext) {
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
                let gen = lineage.generations.first(where: { $0.id == event.generationID })?.generation ?? 0
                allEntries.append(AuditEntry(
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
        let canonicalOutcome = result.canonicalOutcome ?? (result.succeeded ? .completed : .failedBeforeOutput)
        let runtimeProvider = result.runtimeProvider ?? result.providerReceipt?.providerFamily
        let runtimeModel = result.runtimeModel ?? result.providerReceipt?.model ?? result.resolvedModel
        let envelope = result.outcomeEnvelope ?? OutcomeEnvelope(
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
            transportErrorKind: result.transportErrorKind,
            providerStopReason: result.providerStopReason,
            outputPresence: result.outputPresence,
            runtimeProvider: runtimeProvider,
            runtimeModel: runtimeModel,
            envelope: envelope
        )
        agentExec.mcpProfileID = result.mcpProfileID
        agentExec.requestedMCPExtensionsJSON = encodeStringArray(result.requestedMCPExtensions)
        agentExec.effectiveMCPRuntimeExtensionIDsJSON = encodeStringArray(result.effectiveMCPRuntimeExtensionIDs)
        agentExec.deniedMCPExtensionsJSON = encodeStringArray(result.deniedMCPExtensions)
        agentExec.mcpSessionStartupLatencyMilliseconds = result.mcpSessionStartupLatencyMilliseconds
        agentExec.mcpServerTelemetryJSON = encodeMCPServerMetrics(result.mcpServerMetrics)
        agentExec.retryReason = suggestedRetryReason(from: result)
    }

    private func applyTerminalExecutionTruth(
        to agentExec: AgentExecution,
        canonicalOutcome: AgentCanonicalOutcome,
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
        agentExec.transportErrorKind = transportErrorKind
        agentExec.providerStopReason = providerStopReason
        agentExec.outputPresence = outputPresence
        agentExec.runtimeProvider = runtimeProvider
        agentExec.runtimeModel = runtimeModel
        agentExec.settledAt = agentExec.completedAt ?? Date()

        let resolvedEnvelope = envelope ?? OutcomeEnvelope(
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
        let canonicalOutcome = result.canonicalOutcome ?? (result.succeeded ? .completed : .failedBeforeOutput)
        let lowercasedError = result.errorMessage?.lowercased() ?? ""

        switch canonicalOutcome {
        case .limitExhaustedBeforeOutput, .limitExhaustedAfterOutput:
            if lowercasedError.contains("capacity") || lowercasedError.contains("resource_exhausted") {
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
            || lowercasedError.contains("failed to read session") {
            return "Provider session became unavailable; retry this agent"
        }

        return nil
    }

    private static func decodeProviderBindings(from data: Data?) -> [String: ResolvedProviderBinding] {
        guard let data else { return [:] }
        return (try? JSONDecoder().decode([String: ResolvedProviderBinding].self, from: data)) ?? [:]
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
        if let stewardProfile = try? JSONDecoder().decode(StewardContextStrategyProfile.self, from: json) {
            let profileID = decodeContextStrategyProfileID(from: run) ?? "current_mixed_baseline"
            return stewardProfile.runtimeProfile(profileID: profileID)
        }
        return nil
    }

    private static func decodePromotedHandoffArtifacts(from run: Run) -> [String] {
        guard let data = run.promotedHandoffArtifactsJSON,
              let names = try? JSONDecoder().decode([String].self, from: data) else {
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
        guard let resolvedModel = resolveStrategyModel(for: agent, baseBinding: baseBinding, modelTier: modelTier) else {
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

    private func strategyAdjustedAgent(
        from agent: ResolvedAgent,
        binding: ResolvedProviderBinding?
    ) -> ResolvedAgent {
        let resolvedModel = binding?.model ?? agent.model
        guard resolvedModel != agent.model else { return agent }

        return ResolvedAgent(
            id: agent.id,
            title: agent.title,
            mode: agent.mode,
            backendProfileID: agent.backendProfileID,
            provider: agent.provider,
            model: resolvedModel,
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
            sessionReuseScope: agent.sessionReuseScope,
            sessionFamilyID: agent.sessionFamilyID,
            runtimeProfileID: agent.runtimeProfileID
        )
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

        let family = baseBinding.flatMap { ProviderFamily(rawValue: $0.providerFamily) }
            ?? ProviderFamily.from(runtimeIdentifier: agent.provider)

        switch (family, normalizedTier) {
        case (.codex?, "fast"):
            return "gpt-5.3-codex-spark"
        case (.codex?, "frontier"):
            return "GPT-5.4"
        case (.claude?, "fast"):
            return "sonnet"
        case (.claude?, "frontier"):
            return "opus"
        case (.gemini?, "fast"):
            return "gemini-2.5-flash"
        case (.gemini?, "frontier"):
            return "gemini-2.5-pro"
        default:
            return baseBinding?.model ?? agent.model
        }
    }

    private func resolvedModelTierUsed(
        requestedTier: String?,
        effectiveAgent: ResolvedAgent
    ) -> String {
        let normalizedTier = requestedTier?.trimmingCharacters(in: .whitespacesAndNewlines)
        if let normalizedTier, !normalizedTier.isEmpty {
            return normalizedTier
        }
        return effectiveAgent.model.isEmpty ? "bound_runtime" : "bound_runtime"
    }

    private func shouldEscalateStrategyExecution(
        result: AgentResult,
        profile: ContextStrategyProfile?
    ) -> Bool {
        guard !result.succeeded else { return false }
        guard let profile else { return false }

        let defaultTier = profile.defaultModelTier?.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let escalationTier = profile.escalationModelTier?.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard let escalationTier, !escalationTier.isEmpty, escalationTier != defaultTier else {
            return false
        }

        if result.canonicalOutcome == .failedAfterOutputValidation {
            return false
        }
        if let error = result.errorMessage?.lowercased() {
            if error.contains("required outputs missing")
                || error.contains("output contract")
                || error.contains("not valid json")
                || error.contains("missing required field") {
                return false
            }
            if error.contains("timed out")
                || error.contains("timeout")
                || error.contains("session not found")
                || error.contains("-1001") {
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
        case .timedOutBeforeOutput, .timedOutAfterOutput, .completedWithTransportError:
            return true
        default:
            return false
        }
    }

    private func mergedLogSnippet(existing: String?, result: String?) -> String? {
        let existing = existing?.trimmingCharacters(in: .whitespacesAndNewlines)
        let result = result?.trimmingCharacters(in: .whitespacesAndNewlines)

        if let existing, !existing.isEmpty,
           existing != "Session started",
           existing != "Prompt submitted" {
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
        let cacheEffectiveness = payloadBytesBeforeStrategy > 0
            ? Double(payloadReductionBytes) / Double(payloadBytesBeforeStrategy)
            : nil
        agentExec.inputPayloadBytes = summary?.payloadBytesAfterStrategy ?? totalInputBytes
        agentExec.handoffMode = handoffPacket?.mode.rawValue ?? profile?.defaultHandoffMode.rawValue
        agentExec.modelTierUsed = modelTierUsed ?? profile?.defaultModelTier ?? "bound_runtime"
        agentExec.promotedArtifactNamesJSON = encodeArtifactNameList(handoffPacket?.promotedArtifacts ?? [])

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
        guard let data = try? JSONSerialization.data(withJSONObject: report, options: [.prettyPrinted, .sortedKeys]) else {
            return
        }

        let reportProvider = run.stageExecutions
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
            producedArtifactNames.insert(artifact.name)
            if let fields = tryExtractScalarFields(from: data) {
                artifactFields[artifact.name] = fields
            }
        }
    }

    // Proposal 013 §8.2: Update compaction outcome truth on agent execution
    private func updateCompactionOutcome(agentExec: AgentExecution, succeeded: Bool) {
        guard let data = agentExec.compactionMetadataJSON,
              var metadata = try? JSONDecoder().decode(CompactionMetadata.self, from: data) else {
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
        let artifactCount = run.stageExecutions.flatMap(\.agentExecutions).flatMap(\.artifacts).count
        let approvalCount = run.approvals.count
        let totalCostUSD = Double(run.totalCostCents ?? 0) / 100.0

        return [
            "final_status": run.status.rawValue,
            "summary": "\(run.workflowTitle) completed with \(stageCount) stages, \(agentCount) agent executions, \(artifactCount) artifacts, and \(approvalCount) approval checkpoints.",
            "started_at": ISO8601DateFormatter().string(from: run.startedAt),
            "completed_at": ISO8601DateFormatter().string(from: completedAt),
            "duration_seconds": durationSeconds,
            "total_cost": totalCostUSD,
            "cost_currency": "USD"
        ]
    }

    private func persistDeliveryReceiptIfNeeded(finalStateID: String) {
        guard producedArtifactNames.contains("delivery_receipt") == false else { return }
        guard let releaseResult = currentReleaseResultSummary() else { return }

        let provider = run.stageExecutions
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
              let deliveryConfig = try? JSONDecoder().decode(DeliveryConfiguration.self, from: deliveryConfigurationJSON),
              let worktreeRoot = run.worktreeRoot else {
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
            producedArtifactNames.insert(artifact.name)
            if let fields = tryExtractScalarFields(from: data) {
                artifactFields[artifact.name] = fields
            }
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
            .filter { $0.agentID == "commit_and_push_to_github" || $0.agentID == "build_archive_and_push_connect" }
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
           let fields = tryExtractScalarFields(from: data) {
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

    private func decodeArtifact<T: Decodable>(named name: String, from artifacts: [Artifact]) -> T? {
        guard let artifact = artifacts.last(where: { $0.name == name }),
              let data = try? artifactManager.readArtifact(artifact, workspace: workspace) else {
            return nil
        }
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return try? decoder.decode(T.self, from: data)
    }
}
