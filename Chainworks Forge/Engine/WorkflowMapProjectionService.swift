import Foundation
import SwiftData

@MainActor
final class WorkflowMapProjectionService {
    private let modelContext: ModelContext
    private let executionService: ExecutionService

    init(modelContext: ModelContext, executionService: ExecutionService) {
        self.modelContext = modelContext
        self.executionService = executionService
    }

    func projection(for run: Run) -> WorkflowMapProjection? {
        let compiler = RunPlanCompiler(modelContext: modelContext)
        guard let (plan, _) = try? compiler.rebuildPlanFromSnapshot(run: run) else {
            return nil
        }

        let topology = WorkflowMapTopologyBuilder(plan: plan)
        let orderedStateIDs = topology.orderedStateIDs()
        let liveTimeline = Array(executionService.orchestrator(for: run.id)?.liveTimeline.reversed() ?? [])
        let providerBindings = decodeProviderBindings(from: run.providerBindingSnapshotJSON)

        let stages: [WorkflowMapStageProjection] = orderedStateIDs.enumerated().compactMap { index, stateID in
            guard let state = plan.states[stateID] else { return nil }
            return buildStageProjection(
                run: run,
                plan: plan,
                topology: topology,
                stateID: stateID,
                state: state,
                order: index,
                providerBindings: providerBindings
            )
        }

        let occurrences = stages.flatMap(\.occurrences)
        let edges = stages.flatMap { $0.handoffs + $0.transitions }
        let loops = stages.compactMap(\.loopTelemetry)

        let activeOccurrences = occurrences.filter { $0.state == .thinking }.count
        let completedOccurrences = occurrences.filter { $0.state == .completed }.count
        let pendingOccurrences = occurrences.filter { [.notStarted, .ready, .waitingInput].contains($0.state) }.count
        let failedOccurrences = occurrences.filter { $0.state == .failed }.count
        let communicationCount = edges.count + occurrences.count

        return WorkflowMapProjection(
            runID: run.id,
            workflowID: plan.workflowID,
            workflowTitle: plan.workflowTitle,
            runStatus: run.presentationStatus,
            currentStageID: run.currentStageID,
            currentStageLabel: run.currentStageID.flatMap { plan.states[$0]?.label },
            generatedAt: Date(),
            stageCount: stages.count,
            occurrenceCount: occurrences.count,
            activeOccurrenceCount: activeOccurrences,
            completedOccurrenceCount: completedOccurrences,
            pendingOccurrenceCount: pendingOccurrences,
            failedOccurrenceCount: failedOccurrences,
            communicationCount: communicationCount,
            liveEventCount: liveTimeline.count,
            stages: stages,
            occurrences: occurrences,
            edges: edges,
            loops: loops,
            liveTimeline: liveTimeline
        )
    }

    private func buildStageProjection(
        run: Run,
        plan: RunPlan,
        topology: WorkflowMapTopologyBuilder,
        stateID: String,
        state: ExecutableState,
        order: Int,
        providerBindings: [String: ResolvedProviderBinding]
    ) -> WorkflowMapStageProjection? {
        let stageExecutions = run.stageExecutions
            .filter { $0.stageID == stateID }
            .sorted {
                if $0.iteration == $1.iteration {
                    return $0.startedAt < $1.startedAt
                }
                return $0.iteration < $1.iteration
            }

        let latestStageExecution = stageExecutions.last
        let stageStatus = latestStageExecution.map { mapStageState($0.status) } ?? .notStarted
        let latestIteration = latestStageExecution?.iteration ?? 0
        let latestAttempt = latestStageExecution?.attemptNumber ?? 0

        let occurrences = buildOccurrences(
            run: run,
            plan: plan,
            stateID: stateID,
            state: state,
            providerBindings: providerBindings
        )
        let handoffs = buildHandoffEdges(
            plan: plan,
            stateID: stateID,
            state: state,
            occurrences: occurrences
        )

        let transitions = topology.transitionEdges(for: stateID, state: state)
        let loopTelemetry = topology.loopTelemetry(for: stateID, state: state, run: run)
        let ownerLabel = plan.agentBindings[state.ownerAgentID]?.title ?? state.ownerAgentID

        return WorkflowMapStageProjection(
            id: stateID,
            label: state.label,
            order: order,
            type: state.type,
            ownerAgentID: state.ownerAgentID,
            ownerAgentTitle: ownerLabel,
            status: stageStatus,
            iteration: latestIteration,
            attemptNumber: latestAttempt,
            approvalRequired: state.approvalRequired,
            isCurrent: run.currentStageID == stateID,
            occurrences: occurrences,
            handoffs: handoffs,
            transitions: transitions,
            loopTelemetry: loopTelemetry
        )
    }

    private func buildOccurrences(
        run: Run,
        plan: RunPlan,
        stateID: String,
        state: ExecutableState,
        providerBindings: [String: ResolvedProviderBinding]
    ) -> [WorkflowMapOccurrenceProjection] {
        let descriptors = makeDescriptors(for: state)
        let stageExecutions = run.stageExecutions.filter { $0.stageID == stateID }

        return descriptors.enumerated().map { index, descriptor in
            let execution = findExecution(
                for: descriptor,
                in: stageExecutions
            )
            let providerData = providerBinding(for: descriptor.agentID, plan: plan, providerBindings: providerBindings)
            let occurrenceState = mapOccurrenceState(execution?.status)

            return WorkflowMapOccurrenceProjection(
                id: "\(stateID)::\(descriptor.agentID)::\(descriptor.taskName)::\(index)",
                agentID: descriptor.agentID,
                agentTitle: plan.agentBindings[descriptor.agentID]?.title ?? descriptor.agentID,
                taskName: descriptor.taskName,
                stageID: stateID,
                stageLabel: state.label,
                state: occurrenceState,
                provider: providerData.provider,
                model: providerData.model,
                effort: providerData.effort,
                executionCount: stageExecutions.flatMap(\.agentExecutions).filter { $0.agentID == descriptor.agentID && $0.taskName == descriptor.taskName }.count,
                startedAt: execution?.startedAt,
                completedAt: execution?.completedAt,
                sessionID: execution?.providerSessionID ?? execution?.gooseSessionID,
                requestID: execution?.providerRequestID,
                logSnippet: execution?.logSnippet,
                ordinal: index
            )
        }
    }

    private func buildHandoffEdges(
        plan: RunPlan,
        stateID: String,
        state: ExecutableState,
        occurrences: [WorkflowMapOccurrenceProjection]
    ) -> [WorkflowMapEdge] {
        let descriptors = makeDescriptors(for: state)
        guard !descriptors.isEmpty else { return [] }

        var occurrenceByTask: [String: WorkflowMapOccurrenceProjection] = [:]
        for occurrence in occurrences {
            occurrenceByTask[occurrence.taskName] = occurrence
        }
        var edges: [WorkflowMapEdge] = []

        let sequential = descriptors.filter { $0.lane == .sequence }
        if sequential.count > 1 {
            for pair in zip(sequential, sequential.dropFirst()) {
                edges.append(edge(
                    kind: .sequence,
                    from: pair.0,
                    to: pair.1,
                    stageID: stateID,
                    stateLabel: state.label,
                    occurrenceByTask: occurrenceByTask,
                    plan: plan
                ))
            }
        }

        let parallel = descriptors.filter { $0.lane == .parallel }
        if !parallel.isEmpty {
            for descriptor in parallel {
                edges.append(edge(
                    kind: .fanout,
                    from: stageOwnerDescriptor(for: state),
                    to: descriptor,
                    stageID: stateID,
                    stateLabel: state.label,
                    occurrenceByTask: occurrenceByTask,
                    plan: plan
                ))
            }
            if let thenFirst = descriptors.first(where: { $0.lane == .then }) {
                for descriptor in parallel {
                    edges.append(edge(
                        kind: .join,
                        from: descriptor,
                        to: thenFirst,
                        stageID: stateID,
                        stateLabel: state.label,
                        occurrenceByTask: occurrenceByTask,
                        plan: plan
                    ))
                }
            }
        }

        let thenDescriptors = descriptors.filter { $0.lane == .then }
        if thenDescriptors.count > 1 {
            for pair in zip(thenDescriptors, thenDescriptors.dropFirst()) {
                edges.append(edge(
                    kind: .join,
                    from: pair.0,
                    to: pair.1,
                    stageID: stateID,
                    stateLabel: state.label,
                    occurrenceByTask: occurrenceByTask,
                    plan: plan
                ))
            }
        }

        if let lastSequential = sequential.last, let firstThen = thenDescriptors.first {
            edges.append(edge(
                kind: .join,
                from: lastSequential,
                to: firstThen,
                stageID: stateID,
                stateLabel: state.label,
                occurrenceByTask: occurrenceByTask,
                plan: plan
            ))
        }

        if let firstSequential = sequential.first {
            let source = stageOwnerDescriptor(for: state)
            edges.insert(edge(
                kind: .sequence,
                from: source,
                to: firstSequential,
                stageID: stateID,
                stateLabel: state.label,
                occurrenceByTask: occurrenceByTask,
                plan: plan
            ), at: 0)
        } else if let firstThen = thenDescriptors.first {
            let source = stageOwnerDescriptor(for: state)
            edges.insert(edge(
                kind: .join,
                from: source,
                to: firstThen,
                stageID: stateID,
                stateLabel: state.label,
                occurrenceByTask: occurrenceByTask,
                plan: plan
            ), at: 0)
        }

        return edges
    }

    private func stageOwnerDescriptor(for state: ExecutableState) -> TaskDescriptor {
        TaskDescriptor(agentID: state.ownerAgentID, taskName: state.label, lane: .sequence)
    }

    private func edge(
        kind: WorkflowMapEdgeKind,
        from: TaskDescriptor,
        to: TaskDescriptor,
        stageID: String,
        stateLabel: String,
        occurrenceByTask: [String: WorkflowMapOccurrenceProjection],
        plan: RunPlan
    ) -> WorkflowMapEdge {
        let fromOccurrence = occurrenceByTask[from.taskName]
        let toOccurrence = occurrenceByTask[to.taskName]
        let fromLabel = fromOccurrence?.agentTitle ?? plan.agentBindings[from.agentID]?.title ?? from.agentID
        let toLabel = toOccurrence?.agentTitle ?? plan.agentBindings[to.agentID]?.title ?? to.agentID
        return WorkflowMapEdge(
            id: "\(stageID)::\(kind.rawValue)::\(from.taskName)::\(to.taskName)",
            kind: kind,
            fromLabel: fromLabel,
            toLabel: toLabel,
            fromStageID: stageID,
            toStageID: stageID,
            count: 1,
            detail: stateLabel
        )
    }

    private struct TaskDescriptor {
        let agentID: String
        let taskName: String
        let lane: Lane

        init(agentID: String, taskName: String, lane: Lane = .sequence) {
            self.agentID = agentID
            self.taskName = taskName
            self.lane = lane
        }
    }

    private enum Lane {
        case sequence
        case parallel
        case then
    }

    private func makeDescriptors(for state: ExecutableState) -> [TaskDescriptor] {
        var result: [TaskDescriptor] = []

        if let runBlock = state.runBlock {
            for phase in runBlock.phases {
                switch phase {
                case .sequential(let tasks):
                    for task in tasks {
                        result.append(TaskDescriptor(agentID: task.agent, taskName: task.task, lane: .sequence))
                    }
                case .parallel(let tasks):
                    for task in tasks {
                        result.append(TaskDescriptor(agentID: task.agent, taskName: task.task, lane: .parallel))
                    }
                }
            }
        }

        if let runAfterApproval = state.runAfterApproval {
            for phase in runAfterApproval.phases {
                switch phase {
                case .sequential(let tasks):
                    for task in tasks {
                        result.append(TaskDescriptor(agentID: task.agent, taskName: task.task, lane: .then))
                    }
                case .parallel(let tasks):
                    for task in tasks {
                        result.append(TaskDescriptor(agentID: task.agent, taskName: task.task, lane: .then))
                    }
                }
            }
        }

        return result
    }

    private func findExecution(
        for descriptor: TaskDescriptor,
        in stageExecutions: [StageExecution]
    ) -> AgentExecution? {
        let candidates = stageExecutions.flatMap { $0.agentExecutions }
            .filter { $0.agentID == descriptor.agentID && $0.taskName == descriptor.taskName }
            .sorted {
                if $0.startedAt == $1.startedAt {
                    return $0.id.uuidString < $1.id.uuidString
                }
                return $0.startedAt < $1.startedAt
            }
        return candidates.last
    }

    private func providerBinding(
        for agentID: String,
        plan: RunPlan,
        providerBindings: [String: ResolvedProviderBinding]
    ) -> (provider: String, model: String, effort: String) {
        if let binding = providerBindings[agentID] {
            return (binding.providerIdentifier, binding.model, binding.effort)
        }

        if let resolved = plan.agentBindings[agentID] {
            return (resolved.provider, resolved.model, resolved.effort)
        }

        return ("unknown", "unknown", "unknown")
    }

    private func mapOccurrenceState(_ status: AgentStatus?) -> WorkflowMapOccurrenceState {
        switch status {
        case .running:
            return .thinking
        case .completed:
            return .completed
        case .failed:
            return .failed
        case .cancelled, .skipped:
            return .skipped
        case .ready:
            return .ready
        case .pending, nil:
            return .notStarted
        }
    }

    private func mapStageState(_ status: StageStatus) -> WorkflowMapStageState {
        switch status {
        case .pending:
            return .pending
        case .ready:
            return .ready
        case .running:
            return .running
        case .waitingApproval:
            return .waitingApproval
        case .blocked:
            return .blocked
        case .completed:
            return .completed
        case .failed:
            return .failed
        case .skipped:
            return .skipped
        }
    }

    private func decodeProviderBindings(from data: Data?) -> [String: ResolvedProviderBinding] {
        guard let data else { return [:] }
        return (try? JSONDecoder().decode([String: ResolvedProviderBinding].self, from: data)) ?? [:]
    }
}
