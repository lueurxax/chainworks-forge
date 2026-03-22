import Foundation
import SwiftData
import Observation

// MARK: - ApprovalRequest

/// Published when a stage requires human approval (ARCH-028).
struct ApprovalRequest: Identifiable, Sendable {
    let id: UUID
    let runID: UUID
    let stageID: String
    let stageLabel: String
    let requestedAt: Date
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
    }

    // MARK: - Start Execution

    /// Start executing the workflow from the initial state (or a resumed state).
    func start(from stateID: String? = nil) async {
        guard !isRunning else { return }
        isRunning = true
        isCancelled = false

        if let resumeState = stateID {
            currentStateID = resumeState
        }

        run.status = .running

        // Load any already-produced artifacts
        if let existingNames = try? artifactManager.producedArtifactNames(forRunID: workspace.runID) {
            producedArtifactNames = existingNames
        }

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

            // Publish approval request
            let request = ApprovalRequest(
                id: approval.id,
                runID: workspace.runID,
                stageID: state.id,
                stageLabel: state.label,
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
        agentExec.stageExecution = stageExec
        modelContext.insert(agentExec)

        // Gather input artifacts
        let inputData = gatherInputs(for: task)

        // Build execution context
        let execContext = ExecutionContext(
            workspace: workspace,
            stageID: state.id,
            iteration: currentIteration(for: state.id),
            attemptNumber: 1,
            inputArtifacts: inputData,
            variables: runtimeVariables,
            ideaBody: run.idea?.body ?? ""
        )

        // Execute off-MainActor, marshal results back
        let result: AgentResult
        do {
            result = try await executor.execute(task: task, agent: agent, context: execContext)
        } catch {
            agentExec.status = .failed
            agentExec.completedAt = Date()
            agentExec.logSnippet = "Error: \(error.localizedDescription)"
            return false
        }

        // Marshal results back to MainActor
        agentExec.completedAt = Date()
        agentExec.costCents = result.costCents
        agentExec.logSnippet = result.logSnippet

        if result.succeeded {
            agentExec.status = .completed

            // Persist outputs via ArtifactManager (ARCH-030: sole disk writer)
            do {
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

                    // Extract fields from JSON artifacts for expression evaluation
                    if artifact.format == .json {
                        if let data = result.outputs[artifact.name],
                           let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
                            var fields: [String: AnyCodableValue] = [:]
                            for (key, value) in json {
                                if let intVal = value as? Int {
                                    fields[key] = .int(intVal)
                                } else if let doubleVal = value as? Double {
                                    fields[key] = .double(doubleVal)
                                } else if let stringVal = value as? String {
                                    fields[key] = .string(stringVal)
                                } else if let boolVal = value as? Bool {
                                    fields[key] = .bool(boolVal)
                                }
                            }
                            artifactFields[artifact.name] = fields
                        }
                    }
                }

                // Aggregate cost
                if let cost = result.costCents {
                    run.totalCostCents = (run.totalCostCents ?? 0) + cost
                }
            } catch {
                agentExec.status = .failed
                agentExec.logSnippet = "Artifact persistence error: \(error.localizedDescription)"
                return false
            }

            return true
        } else {
            agentExec.status = .failed
            agentExec.logSnippet = result.errorMessage
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
            agentExec.stageExecution = stageExec
            modelContext.insert(agentExec)

            taskAgentPairs.append((task, agent, agentExec))
        }

        // Execute all in parallel
        let results = await withTaskGroup(of: (Int, AgentResult?).self) { group in
            for (index, pair) in taskAgentPairs.enumerated() {
                let execContext = ExecutionContext(
                    workspace: workspace,
                    stageID: state.id,
                    iteration: currentIteration(for: state.id),
                    attemptNumber: 1,
                    inputArtifacts: gatherInputs(for: pair.task),
                    variables: runtimeVariables,
                    ideaBody: run.idea?.body ?? ""
                )
                let task = pair.task
                let agent = pair.agent
                let executor = self.executor

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

            agentExec.completedAt = Date()

            guard let result = optResult, result.succeeded else {
                agentExec.status = .failed
                agentExec.logSnippet = optResult?.errorMessage ?? "Execution failed"
                allSucceeded = false
                continue
            }

            agentExec.status = .completed
            agentExec.costCents = result.costCents
            agentExec.logSnippet = result.logSnippet

            do {
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
                }

                if let cost = result.costCents {
                    run.totalCostCents = (run.totalCostCents ?? 0) + cost
                }
            } catch {
                agentExec.status = .failed
                agentExec.logSnippet = "Artifact persistence error: \(error.localizedDescription)"
                allSucceeded = false
            }
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

    private func gatherInputs(for task: AgentTask) -> [String: Data] {
        var inputs: [String: Data] = [:]
        guard let inputNames = task.inputs else { return inputs }

        for name in inputNames {
            // Look up from already-produced artifacts
            if let artifacts = try? artifactManager.artifacts(forRunID: workspace.runID),
               let artifact = artifacts.last(where: { $0.name == name }) {
                if let data = try? artifactManager.readArtifact(artifact, workspace: workspace) {
                    inputs[name] = data
                }
            }
        }
        return inputs
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
}
