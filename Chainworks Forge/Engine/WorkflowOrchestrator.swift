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
    /// Frozen provider bindings captured at run start.
    private let providerBindingsByAgentID: [String: ResolvedProviderBinding]

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
        configureLiveEventBridge()
    }

    // MARK: - Start Execution

    /// Start executing the workflow from the initial state (or a resumed state).
    func start(from stateID: String? = nil) async {
        guard !isRunning, !isCancelled else { return }

        if let resumeState = stateID {
            currentStateID = resumeState
        }

        loadPersistedArtifacts()

        if restorePendingApprovalIfNeeded(for: currentStateID) {
            return
        }

        isRunning = true
        run.status = .running

        // Main state machine loop
        await executeStateMachine()
    }

    /// Cancel the current execution.
    func cancel() {
        isCancelled = true
        run.status = .cancelled
        isRunning = false
    }

    /// Resolve an approval (called from UI or ExecutionService).
    func resolveApproval(stageID: String, granted: Bool, comment: String? = nil) {
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
        guard let state = plan.states[stageID] else { return }

        // Execute run_after_approval block if present
        if let runAfterApproval = state.runAfterApproval {
            if let stageExec = run.stageExecutions.first(where: { $0.stageID == stageID }) {
                let success = await executeRunBlock(runAfterApproval, state: state, stageExec: stageExec)
                if !success {
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
            run.status = .blocked
            isRunning = false
            onComplete?(false)
            return
        }

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
            if state.type == .end {
                run.status = .completed
                run.completedAt = Date()
                persistFinalFeatureReportIfNeeded(finalStateID: state.id)
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
                    run.status = .completed
                    run.completedAt = Date()
                    isRunning = false
                    onComplete?(true)
                    return
                }
                // Otherwise, stalled
                run.status = .blocked
                isRunning = false
                onComplete?(false)
                return
            }

            // Move to next state
            currentStateID = transition.to
        }
    }

    // MARK: - State Execution Result

    private enum StateResult {
        case succeeded
        case failed
        case paused // waiting for approval
    }

    // MARK: - Execute State

    /// Execute a single state: run block, then check approval.
    private func executeState(_ state: ExecutableState) async -> StateResult {
        // Create StageExecution lazily (ARCH-027)
        let iteration = currentIteration(for: state.id)
        let stageExec = StageExecution(
            stageID: state.id,
            label: state.label,
            status: .running,
            iteration: iteration,
            attemptNumber: 1
        )
        stageExec.run = run
        modelContext.insert(stageExec)

        // Execute run block
        if let runBlock = state.runBlock {
            let blockSuccess = await executeRunBlock(runBlock, state: state, stageExec: stageExec)
            if !blockSuccess {
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
                requestedAt: Date()
            )
            onApprovalRequest?(request)
            return .paused // Will resume when approval is resolved
        }

        // Execute run_after_approval block (if approval was already granted on resume)
        if let runAfterApproval = state.runAfterApproval {
            let afterSuccess = await executeRunBlock(runAfterApproval, state: state, stageExec: stageExec)
            if !afterSuccess {
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
        guard let agent = plan.agentBindings[task.agent] else {
            return false
        }

        // Create AgentExecution lazily (ARCH-027)
        let agentExec = AgentExecution(
            agentID: agent.id,
            agentTitle: agent.title,
            taskName: task.task,
            status: .running,
            provider: agent.provider,
            effort: agent.effort
        )
        // Proposal 003 — REQ-002: Populate Steward metadata on AgentExecution.
        agentExec.agentConfigHash = Self.computeAgentConfigHash(agent: agent)
        agentExec.skillSnapshotHash = DefinitionHasher.hashString(agent.skillRef)

        agentExec.stageExecution = stageExec
        modelContext.insert(agentExec)
        registerLiveExecution(agentExec, for: agent.id)

        // Gather input artifacts
        let inputData = gatherInputs(for: task)
        agentExec.consumedInputArtifactNamesJSON = encodeArtifactNameList(Array(inputData.keys).sorted())
        agentExec.inputBindingsJSON = buildInputBindings(for: task)
        agentExec.resolvedBackendProfileID = agent.backendProfileID

        // Build execution context
        let execContext = ExecutionContext(
            workspace: workspace,
            stageID: state.id,
            iteration: currentIteration(for: state.id),
            attemptNumber: 1,
            inputArtifacts: inputData,
            variables: runtimeVariables,
            ideaBody: run.idea?.body ?? "",
            providerBinding: providerBindingsByAgentID[agent.id]
        )

        // Execute off-MainActor, marshal results back
        let result: AgentResult
        do {
            result = try await executor.execute(task: task, agent: agent, context: execContext)
        } catch {
            agentExec.status = .failed
            agentExec.completedAt = Date()
            agentExec.logSnippet = "Error: \(error.localizedDescription)"
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
        agentExec.logSnippet = mergedLogSnippet(
            existing: agentExec.logSnippet,
            result: result.logSnippet
        )
        // Record sessionID from the executor (§6.1)
        if let sessionID = result.sessionID {
            agentExec.providerSessionID = sessionID
            agentExec.gooseSessionID = sessionID
        }

        if result.succeeded {
            // Persist outputs via ArtifactManager (ARCH-030: sole disk writer)
            do {
                let validatedFields = try validateStructuredOutputs(
                    result.outputs,
                    for: task,
                    agent: agent
                )
                let artifacts = try artifactManager.persistOutputs(
                    outputs: result.outputs,
                    agent: agent,
                    agentExecution: agentExec,
                    workspace: workspace,
                    stageID: state.id,
                    iteration: currentIteration(for: state.id),
                    attemptNumber: 1,
                    catalog: catalog
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
            } catch {
                agentExec.status = .failed
                agentExec.logSnippet = "Output validation or persistence error: \(error.localizedDescription)"
                unregisterLiveExecution(agentExec, for: agent.id)
                return false
            }

            unregisterLiveExecution(agentExec, for: agent.id)
            return true
        } else {
            agentExec.status = .failed
            agentExec.logSnippet = result.errorMessage
            unregisterLiveExecution(agentExec, for: agent.id)
            return false
        }
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
            guard let agent = plan.agentBindings[task.agent] else { continue }

            let agentExec = AgentExecution(
                agentID: agent.id,
                agentTitle: agent.title,
                taskName: task.task,
                status: .running,
                provider: agent.provider,
                effort: agent.effort
            )
            // Proposal 003 — REQ-002: Populate Steward metadata on AgentExecution.
            agentExec.agentConfigHash = Self.computeAgentConfigHash(agent: agent)
            agentExec.skillSnapshotHash = DefinitionHasher.hashString(agent.skillRef)

            agentExec.stageExecution = stageExec
            modelContext.insert(agentExec)
            registerLiveExecution(agentExec, for: agent.id)

            taskAgentPairs.append((task, agent, agentExec))
        }

        // Execute all in parallel
        let results = await withTaskGroup(of: (Int, AgentResult?).self) { group in
            for (index, pair) in taskAgentPairs.enumerated() {
                let gatheredInputs = gatherInputs(for: pair.task)
                let task = pair.task
                let agent = pair.agent
                let execContext = ExecutionContext(
                    workspace: workspace,
                    stageID: state.id,
                    iteration: currentIteration(for: state.id),
                    attemptNumber: 1,
                    inputArtifacts: gatheredInputs,
                    variables: runtimeVariables,
                    ideaBody: run.idea?.body ?? "",
                    providerBinding: providerBindingsByAgentID[agent.id]
                )
                let executor = self.executor
                pair.agentExec.consumedInputArtifactNamesJSON = encodeArtifactNameList(Array(gatheredInputs.keys).sorted())
                pair.agentExec.inputBindingsJSON = buildInputBindings(for: pair.task)

                group.addTask {
                    do {
                        let result = try await executor.execute(
                            task: task,
                            agent: agent,
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

            guard let result = optResult, result.succeeded else {
                agentExec.status = .failed
                agentExec.logSnippet = optResult?.errorMessage ?? "Execution failed"
                unregisterLiveExecution(agentExec, for: agent.id)
                allSucceeded = false
                continue
            }

            agentExec.costCents = result.costCents
            agentExec.resolvedModel = result.resolvedModel
            agentExec.configuredProviderID = result.configuredProviderID
            agentExec.adapterVersion = result.adapterVersion
            agentExec.providerReceiptJSON = encodeProviderReceipt(result.providerReceipt)
            if let sessionID = result.sessionID {
                agentExec.providerSessionID = sessionID
                agentExec.gooseSessionID = sessionID
            }
            agentExec.logSnippet = mergedLogSnippet(
                existing: agentExec.logSnippet,
                result: result.logSnippet
            )

            do {
                let validatedFields = try validateStructuredOutputs(
                    result.outputs,
                    for: pair.task,
                    agent: agent
                )
                let artifacts = try artifactManager.persistOutputs(
                    outputs: result.outputs,
                    agent: agent,
                    agentExecution: agentExec,
                    workspace: workspace,
                    stageID: state.id,
                    iteration: currentIteration(for: state.id),
                    attemptNumber: 1,
                    catalog: catalog
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
            } catch {
                agentExec.status = .failed
                agentExec.logSnippet = "Output validation or persistence error: \(error.localizedDescription)"
                unregisterLiveExecution(agentExec, for: agent.id)
                allSucceeded = false
                continue
            }

            unregisterLiveExecution(agentExec, for: agent.id)
        }

        return allSucceeded
    }

    // MARK: - Helpers

    private func currentIteration(for stageID: String) -> Int {
        let existing = run.stageExecutions.filter { $0.stageID == stageID }
        return existing.count + 1
    }

    private func isApprovalGranted(for stageID: String) -> Bool {
        run.approvals.contains { $0.stageID == stageID && $0.decision == .granted }
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
            requestedAt: approval.requestedAt
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
        run.stageExecutions
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

        for outputName in OutputContractResolver.expectedOutputs(for: task, agent: agent) {
            guard let data = outputs[outputName] else { continue }
            guard let contractID = OutputContractResolver.contractID(
                for: outputName,
                agent: agent,
                catalog: catalog
            ),
            let catalog,
            let contract = catalog.contracts[contractID],
            contract.format == "json" else {
                continue
            }
            let json = try parseJSONObject(
                data,
                agentID: agent.id,
                contractID: contractID,
                outputName: outputName
            )

            for field in contract.requiredFields where json[field] == nil {
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
            try requireArray(json["issues"], field: "issues", agentID: agentID, contractID: contractID, outputName: outputName)
            try requireArray(json["blocking_issues"], field: "blocking_issues", agentID: agentID, contractID: contractID, outputName: outputName)
            try requireArray(json["non_blocking_issues"], field: "non_blocking_issues", agentID: agentID, contractID: contractID, outputName: outputName)
            try requireArray(json["suggestions"], field: "suggestions", agentID: agentID, contractID: contractID, outputName: outputName)
            try requireArray(json["assumptions"], field: "assumptions", agentID: agentID, contractID: contractID, outputName: outputName)
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
            if let boolVal = value as? Bool {
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

    // MARK: - Failure Handling

    private func handleFailure(state: ExecutableState) {
        let policy = plan.failurePolicy
        let action = policy?.onError ?? "pause_and_require_human"

        switch action {
        case "pause_and_require_human":
            run.status = .blocked
            isRunning = false
            isPaused = true
        case "fail_run":
            run.status = .failed
            run.completedAt = Date()
            isRunning = false
        default:
            run.status = .blocked
            isRunning = false
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
            agent.outputContract ?? ""
        ].joined(separator: "|")
        return DefinitionHasher.hashString(canonical)
    }

    // MARK: - Live Event Routing

    private func configureLiveEventBridge() {
        guard let gooseExecutor = executor as? GooseAgentExecutor else { return }

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
    }

    private func recordLiveExecutionEvent(agentID: String, event: ExecutionEvent) {
        let agentExecution = resolvedAgentExecution(for: agentID)

        if let sessionID = event.sessionID {
            agentExecution?.providerSessionID = sessionID
            agentExecution?.gooseSessionID = sessionID
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
            liveTimeline[lastIndex] = entry
        } else {
            liveTimeline.append(entry)
            if liveTimeline.count > 40 {
                liveTimeline.removeFirst(liveTimeline.count - 40)
            }
        }
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

    private static func decodeProviderBindings(from data: Data?) -> [String: ResolvedProviderBinding] {
        guard let data else { return [:] }
        return (try? JSONDecoder().decode([String: ResolvedProviderBinding].self, from: data)) ?? [:]
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
}
