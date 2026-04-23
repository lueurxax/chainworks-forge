import Foundation
import SwiftData

struct WorkflowMapCurrentStageSummary: Sendable {
    let stageID: String
    let label: String
    let iteration: Int?
    let attemptNumber: Int?
    let status: StageStatus?
}

@MainActor
final class WorkflowMapProjectionService {
    private struct PlanCacheKey: Hashable {
        let runID: UUID
        let workflowSnapshotHash: String
        let catalogSnapshotHash: String
        let planCompilerVersion: Int
    }

    private struct PlanCacheEntry {
        let plan: RunPlan
        let topology: WorkflowMapTopologyBuilder
    }

    private static var planCache: [PlanCacheKey: PlanCacheEntry] = [:]
    private static var planCacheMissCount = 0

    private let modelContext: ModelContext
    private let executionService: ExecutionService

    init(modelContext: ModelContext, executionService: ExecutionService) {
        self.modelContext = modelContext
        self.executionService = executionService
    }

    func runStatus(for run: Run) -> RunStatus {
        let latestPersistedStage = RunLatestStageStatusLoader.load(for: run, modelContext: modelContext)
        let liveTimeline = executionService.peekOrchestrator(for: run.id)?.liveTimeline ?? []
        return deriveRunStatus(
            for: run,
            latestPersistedStage: latestPersistedStage,
            liveTimeline: liveTimeline
        )
    }

    func currentStageSummary(for run: Run) -> WorkflowMapCurrentStageSummary? {
        let latestPersistedStage = RunLatestStageStatusLoader.load(for: run, modelContext: modelContext)
        let liveTimeline = executionService.peekOrchestrator(for: run.id)?.liveTimeline ?? []
        guard let stageID = deriveCurrentStageID(
            for: run,
            latestPersistedStage: latestPersistedStage,
            liveTimeline: liveTimeline
        ) else {
            return nil
        }

        let cachedPlan = cachedPlanEntry(for: run)
        let label = cachedPlan?.plan.states[stageID]?.label
            ?? latestPersistedStage?.label
            ?? run.cursorDerivedStageLabel

        let cursor = run.transitionCursor
        let cursorOwnsCurrentStage = cursor?.nextScheduledStateID == stageID

        return WorkflowMapCurrentStageSummary(
            stageID: stageID,
            label: label.isEmpty ? stageID : label,
            iteration: cursorOwnsCurrentStage
                ? cursor?.nextScheduledIteration ?? latestPersistedStage?.iteration
                : latestPersistedStage?.iteration,
            attemptNumber: cursorOwnsCurrentStage
                ? cursor?.nextScheduledAttemptNumber ?? latestPersistedStage?.attemptNumber
                : latestPersistedStage?.attemptNumber,
            status: latestPersistedStage?.stageID == stageID ? latestPersistedStage?.status : nil
        )
    }

    func projection(for run: Run) -> WorkflowMapProjection? {
        guard let cachedPlan = cachedPlanEntry(for: run) else {
            return nil
        }
        let persistedStages = RunStageSnapshotLoader.load(for: run, modelContext: modelContext)
        let persistedApprovals = PersistedRunGraph.approvals(for: run)
        let plan = cachedPlan.plan
        let topology = cachedPlan.topology
        let orderedStateIDs = topology.orderedStateIDs()
        let liveTimeline = executionService.peekOrchestrator(for: run.id)?.liveTimeline ?? []
        let persistedTimeline = buildPersistedTimeline(stages: persistedStages, approvals: persistedApprovals, plan: plan)
        let providerBindings = decodeProviderBindings(from: run.providerBindingSnapshotJSON)
        let currentStageID = deriveCurrentStageID(for: run, from: persistedStages, liveTimeline: liveTimeline)
        let projectionStages = suppressStaleFutureStageExecutions(
            in: persistedStages,
            currentStageID: currentStageID,
            orderedStateIDs: orderedStateIDs
        )
        let runStatus = deriveRunStatus(for: run, persistedStages: persistedStages, liveTimeline: liveTimeline)

        let stages: [WorkflowMapStageProjection] = orderedStateIDs.enumerated().compactMap { index, stateID in
            guard let state = plan.states[stateID] else { return nil }
            return buildStageProjection(
                run: run,
                plan: plan,
                topology: topology,
                persistedStages: projectionStages,
                currentStageID: currentStageID,
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
            runStatus: runStatus,
            currentStageID: currentStageID,
            currentStageLabel: currentStageID.flatMap { plan.states[$0]?.label },
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
            liveTimeline: liveTimeline,
            persistedTimeline: persistedTimeline
        )
    }

    private func cachedPlanEntry(for run: Run) -> PlanCacheEntry? {
        let key = PlanCacheKey(
            runID: run.id,
            workflowSnapshotHash: run.workflowSnapshotHash,
            catalogSnapshotHash: run.catalogSnapshotHash,
            planCompilerVersion: run.planCompilerVersion
        )

        if let cached = Self.planCache[key] {
            return cached
        }

        let compiler = RunPlanCompiler(modelContext: modelContext)
        guard let (plan, _) = try? compiler.rebuildPlanFromSnapshot(run: run) else {
            return nil
        }

        let entry = PlanCacheEntry(
            plan: plan,
            topology: WorkflowMapTopologyBuilder(plan: plan)
        )
        Self.planCache[key] = entry
        Self.planCacheMissCount += 1
        return entry
    }

    /// Proposal 032: Prefer the durable transition cursor for current-stage derivation.
    /// Falls back to persisted stage row heuristics for pre-P032 runs.
    private func deriveCurrentStageID(
        for run: Run,
        from persistedStages: [RunStageSnapshot],
        liveTimeline: [LiveExecutionTimelineEntry]
    ) -> String? {
        if let liveRetryStageID = latestLiveRetryStageIDIfNewerThanPersistedTerminalState(
            run: run,
            persistedStages: persistedStages,
            liveTimeline: liveTimeline
        ) {
            return liveRetryStageID
        }

        if let cursor = run.transitionCursor {
            switch cursor.settlementPhase {
            case .transitionSettled, .transitionStarted:
                return cursor.nextScheduledStateID
            case .terminal, .awaitingConflictResolution:
                return cursor.lastCompletedStateID
            case .awaitingFirstState:
                break // Fall through to heuristic
            }

            return nil
        }

        let sorted = persistedStages.sorted { $0.startedAt < $1.startedAt }
        return sorted.last(where: {
            $0.status == .running
                || $0.status == .waitingApproval
                || $0.status == .blocked
                || $0.status == .failed
                || $0.status == .ready
        })?.stageID
            ?? sorted.last(where: { $0.status == .completed })?.stageID
    }

    private func deriveCurrentStageID(
        for run: Run,
        latestPersistedStage: RunLatestStageStatusSnapshot?,
        liveTimeline: [LiveExecutionTimelineEntry]
    ) -> String? {
        if let liveRetryStageID = latestLiveRetryStageIDIfNewerThanPersistedTerminalState(
            run: run,
            latestPersistedStage: latestPersistedStage,
            liveTimeline: liveTimeline
        ) {
            return liveRetryStageID
        }

        if let cursor = run.transitionCursor {
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

        if let latestPersistedStage, latestPersistedStage.status != .completed, latestPersistedStage.status != .skipped {
            return latestPersistedStage.stageID
        }

        return latestPersistedStage?.stageID
    }

    private func deriveRunStatus(
        for run: Run,
        persistedStages: [RunStageSnapshot],
        liveTimeline: [LiveExecutionTimelineEntry]
    ) -> RunStatus {
        if run.cancellationRequestedAt != nil && run.cancellationSettledAt == nil {
            return .cancelling
        }

        if hasLiveRetryActivityNewerThanPersistedTerminalState(
            persistedStageStartedAt: persistedStages
                .sorted(by: compareStageSnapshots)
                .last(where: { $0.status == .blocked || $0.status == .failed })
                .map { $0.completedAt ?? $0.startedAt },
            liveTimeline: liveTimeline
        ) {
            return .running
        }

        if latestLiveRetryStageIDIfNewerThanPersistedTerminalState(
            run: run,
            persistedStages: persistedStages,
            liveTimeline: liveTimeline
        ) != nil {
            return .running
        }

        switch run.status {
        case .waitingApproval, .blocked, .completed, .failed, .cancelled, .cancelling:
            return run.status
        case .pending, .ready, .running:
            break
        }

        if let latestStage = persistedStages.sorted(by: { $0.startedAt < $1.startedAt }).last {
            switch latestStage.status {
            case .pending:
                return .pending
            case .ready:
                return run.status == .pending ? .pending : .ready
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

        return run.status
    }

    private func deriveRunStatus(
        for run: Run,
        latestPersistedStage: RunLatestStageStatusSnapshot?,
        liveTimeline: [LiveExecutionTimelineEntry]
    ) -> RunStatus {
        if run.cancellationRequestedAt != nil && run.cancellationSettledAt == nil {
            return .cancelling
        }

        if hasLiveRetryActivityNewerThanPersistedTerminalState(
            persistedStageStartedAt: latestPersistedStage.map { $0.completedAt ?? $0.startedAt },
            liveTimeline: liveTimeline
        ) {
            return .running
        }

        if latestLiveRetryStageIDIfNewerThanPersistedTerminalState(
            run: run,
            latestPersistedStage: latestPersistedStage,
            liveTimeline: liveTimeline
        ) != nil {
            return .running
        }

        switch run.status {
        case .waitingApproval, .blocked, .completed, .failed, .cancelled, .cancelling:
            return run.status
        case .pending, .ready, .running:
            break
        }

        if let latestStage = latestPersistedStage {
            switch latestStage.status {
            case .pending:
                return .pending
            case .ready:
                return run.status == .pending ? .pending : .ready
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

        return run.status
    }

    private func hasLiveRetryActivityNewerThanPersistedTerminalState(
        persistedStageStartedAt: Date?,
        liveTimeline: [LiveExecutionTimelineEntry]
    ) -> Bool {
        guard
            let persistedStageStartedAt,
            let latestLiveEntry = liveTimeline.max(by: { lhs, rhs in
                if lhs.event.timestamp == rhs.event.timestamp {
                    return lhs.id.uuidString < rhs.id.uuidString
                }
                return lhs.event.timestamp < rhs.event.timestamp
            })
        else {
            return false
        }

        return latestLiveEntry.event.timestamp > persistedStageStartedAt
    }

    private func latestLiveRetryStageIDIfNewerThanPersistedTerminalState(
        run: Run,
        persistedStages: [RunStageSnapshot],
        liveTimeline: [LiveExecutionTimelineEntry]
    ) -> String? {
        guard !liveTimeline.isEmpty else { return nil }
        guard
            let latestPersistedStage = persistedStages.sorted(by: compareStageSnapshots).last,
            latestPersistedStage.status == .blocked || latestPersistedStage.status == .failed,
            let latestLiveEntry = liveTimeline.max(by: { lhs, rhs in
                if lhs.event.timestamp == rhs.event.timestamp {
                    return lhs.id.uuidString < rhs.id.uuidString
                }
                return lhs.event.timestamp < rhs.event.timestamp
            }),
            latestLiveEntry.event.timestamp > (latestPersistedStage.completedAt ?? latestPersistedStage.startedAt)
        else {
            return nil
        }

        return latestLiveEntry.stageID
    }

    private func latestLiveRetryStageIDIfNewerThanPersistedTerminalState(
        run: Run,
        latestPersistedStage: RunLatestStageStatusSnapshot?,
        liveTimeline: [LiveExecutionTimelineEntry]
    ) -> String? {
        guard !liveTimeline.isEmpty else { return nil }
        guard
            let latestPersistedStage,
            latestPersistedStage.status == .blocked || latestPersistedStage.status == .failed,
            let latestLiveEntry = liveTimeline.max(by: { lhs, rhs in
                if lhs.event.timestamp == rhs.event.timestamp {
                    return lhs.stageID < rhs.stageID
                }
                return lhs.event.timestamp < rhs.event.timestamp
            }),
            latestLiveEntry.event.timestamp > (latestPersistedStage.completedAt ?? latestPersistedStage.startedAt)
        else {
            return nil
        }

        return latestLiveEntry.stageID
    }

    private func suppressStaleFutureStageExecutions(
        in persistedStages: [RunStageSnapshot],
        currentStageID: String?,
        orderedStateIDs: [String]
    ) -> [RunStageSnapshot] {
        guard
            let currentStageID,
            let currentStageOrder = orderedStateIDs.firstIndex(of: currentStageID),
            let currentStageExecution = persistedStages
                .filter({ $0.stageID == currentStageID })
                .sorted(by: compareStageSnapshots)
                .last
        else {
            return persistedStages
        }

        let currentStageIsNonTerminal: Bool = {
            switch currentStageExecution.status {
            case .pending, .ready, .running, .waitingApproval, .blocked, .failed:
                return true
            case .completed, .skipped:
                return false
            }
        }()

        guard currentStageIsNonTerminal else { return persistedStages }

        return persistedStages.filter { stage in
            guard
                let stageOrder = orderedStateIDs.firstIndex(of: stage.stageID),
                stageOrder > currentStageOrder,
                stage.startedAt < currentStageExecution.startedAt
            else {
                return true
            }

            switch stage.status {
            case .pending, .ready, .running, .waitingApproval, .blocked, .failed:
                return false
            case .completed, .skipped:
                return true
            }
        }
    }

#if DEBUG
    static func resetPlanCacheForTesting() {
        planCache = [:]
        planCacheMissCount = 0
    }

    static var planCacheMissCountForTesting: Int {
        planCacheMissCount
    }

    static var cachedPlanCountForTesting: Int {
        planCache.count
    }
#endif

    private func buildPersistedTimeline(
        stages: [RunStageSnapshot],
        approvals: [Approval],
        plan: RunPlan
    ) -> [WorkflowMapPersistedTimelineEntry] {
        var entries: [WorkflowMapPersistedTimelineEntry] = []

        for stage in stages {
            let timestamp = stage.completedAt ?? stage.startedAt
            let stageLabel = plan.states[stage.stageID]?.label ?? stage.label
            entries.append(
                WorkflowMapPersistedTimelineEntry(
                    id: "stage::\(stage.id.uuidString)",
                    title: stageLabel,
                    detail: "Persisted stage status: \(stage.status.rawValue.replacingOccurrences(of: "_", with: " "))",
                    timestamp: timestamp,
                    sessionID: nil
                )
            )

            for agent in stage.agentExecutions {
                let agentTimestamp = agent.completedAt ?? agent.startedAt
                if let sessionID = agent.runtimeSessionID?.trimmingCharacters(in: .whitespacesAndNewlines),
                   !sessionID.isEmpty {
                    let statusLabel = agent.status.rawValue.replacingOccurrences(of: "_", with: " ")
                    entries.append(
                        WorkflowMapPersistedTimelineEntry(
                            id: "agent-session::\(agent.id.uuidString)",
                            title: agent.agentTitle,
                            detail: "Persisted agent \(statusLabel) in session \(sessionID)",
                            timestamp: agentTimestamp,
                            sessionID: sessionID
                        )
                    )
                }

                if let supervision = agent.supervisionClassification {
                    entries.append(
                        WorkflowMapPersistedTimelineEntry(
                            id: "agent-supervision::\(agent.id.uuidString)",
                            title: agent.agentTitle,
                            detail: "Persisted supervision: \(supervision.defaultSummary)",
                            timestamp: agentTimestamp,
                            sessionID: agent.runtimeSessionID
                        )
                    )
                }

                if agent.retryReason == "automatic_watchdog_retry" {
                    let attemptLabel = agent.agentAttemptNumber.map { "attempt \($0)" } ?? "retry"
                    let retryDetail: String = {
                        switch agent.status {
                        case .completed:
                            return "Persisted automatic watchdog retry succeeded (\(attemptLabel)) for \(agent.taskName)"
                        case .failed:
                            return "Persisted automatic watchdog retry exhausted (\(attemptLabel)) for \(agent.taskName)"
                        default:
                            return "Persisted automatic watchdog retry (\(attemptLabel)) for \(agent.taskName)"
                        }
                    }()
                    entries.append(
                        WorkflowMapPersistedTimelineEntry(
                            id: "agent-retry::\(agent.id.uuidString)",
                            title: agent.agentTitle,
                            detail: retryDetail,
                            timestamp: agent.startedAt,
                            sessionID: agent.runtimeSessionID
                        )
                    )
                }
            }

            if let snapshot = decodeRecoverySnapshot(from: stage),
               let recommended = snapshot.recommendedAction,
               stage.agentExecutions.contains(where: { $0.retryReason == "automatic_watchdog_retry" }) {
                entries.append(
                    WorkflowMapPersistedTimelineEntry(
                        id: "stage-recovery::\(stage.id.uuidString)",
                        title: stageLabel,
                        detail: recommended.explanation,
                        timestamp: snapshot.timestamp,
                        sessionID: nil
                    )
                )
            }
        }

        for approval in approvals {
            let timestamp = approval.decidedAt ?? approval.requestedAt
            let stageLabel = plan.states[approval.stageID]?.label ?? approval.stageID
            entries.append(
                WorkflowMapPersistedTimelineEntry(
                    id: "approval::\(approval.id.uuidString)",
                    title: stageLabel,
                    detail: "Persisted approval \(approval.decision.rawValue.replacingOccurrences(of: "_", with: " "))",
                    timestamp: timestamp,
                    sessionID: nil
                )
            )
        }

        return entries.sorted { $0.timestamp > $1.timestamp }
    }

    private func buildStageProjection(
        run: Run,
        plan: RunPlan,
        topology: WorkflowMapTopologyBuilder,
        persistedStages: [RunStageSnapshot],
        currentStageID: String?,
        stateID: String,
        state: ExecutableState,
        order: Int,
        providerBindings: [String: ResolvedProviderBinding]
    ) -> WorkflowMapStageProjection? {
        let stageExecutions = persistedStages
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
            plan: plan,
            persistedStages: persistedStages,
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
            isCurrent: currentStageID == stateID,
            occurrences: occurrences,
            handoffs: handoffs,
            transitions: transitions,
            loopTelemetry: loopTelemetry
        )
    }

    private func decodeRecoverySnapshot(from stage: RunStageSnapshot) -> RecoveryActionSnapshot? {
        guard let data = stage.recoverySnapshotJSON else { return nil }
        return try? JSONDecoder().decode(RecoveryActionSnapshot.self, from: data)
    }

    private func buildOccurrences(
        plan: RunPlan,
        persistedStages: [RunStageSnapshot],
        stateID: String,
        state: ExecutableState,
        providerBindings: [String: ResolvedProviderBinding]
    ) -> [WorkflowMapOccurrenceProjection] {
        let descriptors = makeDescriptors(for: state)
        let stageExecutions = persistedStages.filter { $0.stageID == stateID }

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
                sessionID: nil,
                requestID: nil,
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
        in stageExecutions: [RunStageSnapshot]
    ) -> RunStageAgentSnapshot? {
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

    private func compareStageSnapshots(_ lhs: RunStageSnapshot, _ rhs: RunStageSnapshot) -> Bool {
        if lhs.startedAt == rhs.startedAt {
            if lhs.iteration == rhs.iteration {
                return lhs.attemptNumber < rhs.attemptNumber
            }
            return lhs.iteration < rhs.iteration
        }
        return lhs.startedAt < rhs.startedAt
    }
}
