import Foundation

enum WorkflowMapStageState: String, Sendable {
    case notStarted = "not_started"
    case pending
    case ready
    case running
    case waitingApproval = "waiting_approval"
    case blocked
    case completed
    case failed
    case skipped
}

enum WorkflowMapOccurrenceState: String, Sendable, CaseIterable {
    case notStarted = "not_started"
    case ready
    case thinking
    case waitingInput = "waiting_input"
    case completed
    case failed
    case skipped
}

enum WorkflowMapEdgeKind: String, Sendable {
    case sequence
    case fanout
    case join
    case transition
    case loop
}

struct WorkflowMapProjection: Sendable {
    let runID: UUID
    let workflowID: String
    let workflowTitle: String
    let runStatus: RunStatus
    let currentStageID: String?
    let currentStageLabel: String?
    let generatedAt: Date
    let stageCount: Int
    let occurrenceCount: Int
    let activeOccurrenceCount: Int
    let completedOccurrenceCount: Int
    let pendingOccurrenceCount: Int
    let failedOccurrenceCount: Int
    let communicationCount: Int
    let liveEventCount: Int
    let stages: [WorkflowMapStageProjection]
    let occurrences: [WorkflowMapOccurrenceProjection]
    let edges: [WorkflowMapEdge]
    let loops: [WorkflowMapLoopTelemetry]
    let liveTimeline: [LiveExecutionTimelineEntry]
    let persistedTimeline: [WorkflowMapPersistedTimelineEntry]
    let xcodeRuntimeObservations: [WorkflowMapXcodeRuntimeObservation]

    var activeOccurrences: [WorkflowMapOccurrenceProjection] {
        occurrences.filter { $0.state == .thinking }
    }

    var completedOccurrences: [WorkflowMapOccurrenceProjection] {
        occurrences.filter { $0.state == .completed }
    }

    var pendingOccurrences: [WorkflowMapOccurrenceProjection] {
        occurrences.filter { [.notStarted, .ready, .waitingInput].contains($0.state) }
    }

    var failedOccurrences: [WorkflowMapOccurrenceProjection] {
        occurrences.filter { $0.state == .failed }
    }

    var readyOccurrences: [WorkflowMapOccurrenceProjection] {
        occurrences.filter { $0.state == .ready }
    }

    var notStartedOccurrences: [WorkflowMapOccurrenceProjection] {
        occurrences.filter { $0.state == .notStarted }
    }

    var waitingInputOccurrences: [WorkflowMapOccurrenceProjection] {
        occurrences.filter { $0.state == .waitingInput }
    }

    var skippedOccurrences: [WorkflowMapOccurrenceProjection] {
        occurrences.filter { $0.state == .skipped }
    }
}

struct WorkflowMapPersistedTimelineEntry: Identifiable, Sendable {
    let id: String
    let title: String
    let detail: String
    let timestamp: Date
    let sessionID: String?
}

struct WorkflowMapXcodeRuntimeObservation: Identifiable, Sendable, Equatable {
    let id: String
    let stageID: String
    let stageLabel: String
    let agentExecutionID: UUID
    let agentTitle: String
    let brokerObservations: [WorkflowMapXcodeBrokerObservation]
    let shimInvocations: [WorkflowMapXcodeShimInvocation]
    let shimWarnings: [WorkflowMapXcodeShimWarning]
    let hostExecutorEvents: [WorkflowMapXcodeHostExecutorEvent]
    let storage: WorkflowMapXcodeRuntimeStorageStatus

    var latestBrokerObservation: WorkflowMapXcodeBrokerObservation? {
        brokerObservations.last
    }

    var selectedSimulatorID: String? {
        brokerObservations.reversed().compactMap(\.simulatorID).first
            ?? hostExecutorEvents.reversed().compactMap(\.selectedSimulatorID).first
    }

    var coalescedShimWarnings: [WorkflowMapXcodeShimWarning] {
        var seenKeys = Set<String>()
        return shimWarnings.filter { warning in
            seenKeys.insert(warning.coalescingKey).inserted
        }
    }

    var bridgeProgressStatus: WorkflowMapXcodeBridgeProgressStatus? {
        brokerObservations.reversed().compactMap {
            WorkflowMapXcodeBridgeProgressStatus(observation: $0)
        }.first
    }

    var hasRenderableEvidence: Bool {
        !brokerObservations.isEmpty
            || !shimInvocations.isEmpty
            || !shimWarnings.isEmpty
            || !hostExecutorEvents.isEmpty
            || storage.truncated
    }
}

struct WorkflowMapXcodeBrokerObservation: Sendable, Equatable {
    let source: String
    let backendStartDisposition: String
    let poolID: String?
    let leaseID: String?
    let xcodePID: String?
    let backendProcessID: Int?
    let xcodeHomeDisposition: String?
    let xcodeTmpdirDisposition: String?
    let siblingLeasesAtSpawn: Int?
    let backendInitializeWaitMilliseconds: Int?
    let backendStartupLatencyMilliseconds: Int?
    let backendFailureClass: String?
    let statusUpdate: String?
    let simulatorSelectionMode: String?
    let simulatorID: String?
}

struct WorkflowMapXcodeShimInvocation: Sendable, Equatable {
    let tool: String
    let policyDecision: String
    let policyReason: String
    let exitStatus: Int
}

struct WorkflowMapXcodeShimWarning: Sendable, Equatable {
    let policyReason: String
    let sourceField: String
    let matchedSubstring: String
    let excerpt: String

    var coalescingKey: String {
        [
            policyReason.trimmingCharacters(in: .whitespacesAndNewlines).lowercased(),
            sourceField.trimmingCharacters(in: .whitespacesAndNewlines).lowercased(),
            matchedSubstring.trimmingCharacters(in: .whitespacesAndNewlines),
            excerpt.trimmingCharacters(in: .whitespacesAndNewlines)
        ].joined(separator: "\u{1F}")
    }
}

struct WorkflowMapXcodeHostExecutorEvent: Sendable, Equatable {
    let tool: String
    let hostEnvDisposition: String
    let selectedSimulatorID: String?
    let exitStatus: Int
    let durationMilliseconds: Int
}

struct WorkflowMapXcodeRuntimeStorageStatus: Sendable, Equatable {
    let truncated: Bool
    let totalEventsDropped: Int
    let corruptJSONRecoveryCount: Int
}

struct WorkflowMapXcodeBridgeProgressStatus: Sendable, Equatable {
    enum Kind: Sendable, Equatable {
        case waitingForLock
        case starting
        case actionRequired
    }

    let kind: Kind
    let label: String
    let detail: String

    fileprivate init?(observation: WorkflowMapXcodeBrokerObservation) {
        let detail = [
            observation.backendFailureClass,
            observation.statusUpdate,
            observation.backendStartDisposition
        ]
        .compactMap { $0?.trimmingCharacters(in: .whitespacesAndNewlines) }
        .filter { !$0.isEmpty }
        .joined(separator: " ")
        let normalized = detail.lowercased()

        if normalized.contains("xcode_mcp_action_required")
            || normalized.contains("action required")
            || normalized.contains("check xcode")
            || normalized.contains("consent")
        {
            self.kind = .actionRequired
            self.label = "Action Required: Check Xcode"
            self.detail = detail
            return
        }

        if normalized.contains("queue_waiting")
            || normalized.contains("waiting for xcode mcp bridge")
            || normalized.contains("waiting for xcode bridge")
            || (normalized.contains("waiting") && normalized.contains("lock"))
        {
            self.kind = .waitingForLock
            self.label = "Waiting for Xcode Bridge lock"
            self.detail = detail
            return
        }

        if normalized.contains("lease_reserved")
            || normalized.contains("reserved brokered xcode mcp lease")
            || normalized.contains("initialize_lock_acquired")
            || normalized.contains("forwarding brokered xcode mcp initialize")
            || normalized.contains("starting")
        {
            self.kind = .starting
            self.label = "Starting Xcode Bridge"
            self.detail = detail
            return
        }

        return nil
    }
}

func latestXcodeBridgeProgressStatus(
    in observations: [WorkflowMapXcodeRuntimeObservation]
) -> WorkflowMapXcodeBridgeProgressStatus? {
    observations.reversed().compactMap(\.bridgeProgressStatus).first
}

func buildXcodeRuntimeObservations(
    from persistedStages: [RunStageSnapshot]
) -> [WorkflowMapXcodeRuntimeObservation] {
    persistedStages.flatMap { stage in
        stage.agentExecutions.compactMap { agent in
            guard let data = agent.actualXcodeRuntimeObservationJSON,
                  let observation = WorkflowMapXcodeRuntimePayload.decode(
                    data: data,
                    stage: stage,
                    agent: agent
                  ),
                  observation.hasRenderableEvidence
            else {
                return nil
            }
            return observation
        }
    }
}

private struct WorkflowMapXcodeRuntimePayload: Decodable {
    let mcpBrokerObservations: [BrokerObservation]
    let xcodeShimEvents: [ShimEvent]
    let xcodeHostExecutorEvents: [HostExecutorEvent]
    let storage: Storage?

    enum CodingKeys: String, CodingKey {
        case mcpBrokerObservations = "mcp_broker_observations"
        case xcodeShimEvents = "xcode_shim_events"
        case xcodeHostExecutorEvents = "xcode_host_executor_events"
        case storage
    }

    static func decode(
        data: Data,
        stage: RunStageSnapshot,
        agent: RunStageAgentSnapshot
    ) -> WorkflowMapXcodeRuntimeObservation? {
        guard let payload = try? JSONDecoder().decode(Self.self, from: data) else {
            return nil
        }

        let shimInvocations = payload.xcodeShimEvents.compactMap { event -> WorkflowMapXcodeShimInvocation? in
            guard case .shimInvocation(let invocation) = event else { return nil }
            return WorkflowMapXcodeShimInvocation(
                tool: invocation.tool,
                policyDecision: invocation.policyDecision,
                policyReason: invocation.policyReason,
                exitStatus: invocation.exitStatus
            )
        }

        let shimWarnings = payload.xcodeShimEvents.compactMap { event -> WorkflowMapXcodeShimWarning? in
            guard case .warning(let warning) = event else { return nil }
            return WorkflowMapXcodeShimWarning(
                policyReason: warning.policyReason,
                sourceField: warning.sourceField,
                matchedSubstring: warning.matchedSubstring,
                excerpt: warning.excerpt
            )
        }

        return WorkflowMapXcodeRuntimeObservation(
            id: "\(stage.stageID)::\(agent.id.uuidString)::xcode-runtime",
            stageID: stage.stageID,
            stageLabel: stage.label,
            agentExecutionID: agent.id,
            agentTitle: agent.agentTitle,
            brokerObservations: payload.mcpBrokerObservations.map(\.projection),
            shimInvocations: shimInvocations,
            shimWarnings: shimWarnings,
            hostExecutorEvents: payload.xcodeHostExecutorEvents.map(\.projection),
            storage: payload.storage?.projection ?? WorkflowMapXcodeRuntimeStorageStatus(
                truncated: false,
                totalEventsDropped: 0,
                corruptJSONRecoveryCount: 0
            )
        )
    }

    struct BrokerObservation: Decodable {
        let source: String?
        let backendStartDisposition: String?
        let poolID: String?
        let leaseID: String?
        let xcodePID: String?
        let backendProcessID: Int?
        let xcodeHomeDisposition: String?
        let xcodeTmpdirDisposition: String?
        let simulatorSelection: SimulatorSelection?
        let siblingLeasesAtSpawn: Int?
        let backendInitializeWaitMilliseconds: Int?
        let backendStartupLatencyMilliseconds: Int?
        let backendFailureClass: String?
        let statusUpdate: String?

        enum CodingKeys: String, CodingKey {
            case source
            case backendStartDisposition = "backend_start_disposition"
            case poolID = "pool_id"
            case leaseID = "lease_id"
            case xcodePID = "xcode_pid"
            case backendProcessID = "backend_process_id"
            case xcodeHomeDisposition = "xcode_home_disposition"
            case xcodeTmpdirDisposition = "xcode_tmpdir_disposition"
            case simulatorSelection = "simulator_selection"
            case siblingLeasesAtSpawn = "sibling_leases_at_spawn"
            case backendInitializeWaitMilliseconds = "backend_initialize_wait_ms"
            case backendStartupLatencyMilliseconds = "backend_startup_latency_ms"
            case backendFailureClass = "backend_failure_class"
            case statusUpdate = "status_update"
        }

        var projection: WorkflowMapXcodeBrokerObservation {
            WorkflowMapXcodeBrokerObservation(
                source: source ?? "xcode_mcp_broker",
                backendStartDisposition: backendStartDisposition ?? "unknown",
                poolID: poolID,
                leaseID: leaseID,
                xcodePID: xcodePID,
                backendProcessID: backendProcessID,
                xcodeHomeDisposition: xcodeHomeDisposition,
                xcodeTmpdirDisposition: xcodeTmpdirDisposition,
                siblingLeasesAtSpawn: siblingLeasesAtSpawn,
                backendInitializeWaitMilliseconds: backendInitializeWaitMilliseconds,
                backendStartupLatencyMilliseconds: backendStartupLatencyMilliseconds,
                backendFailureClass: backendFailureClass,
                statusUpdate: statusUpdate,
                simulatorSelectionMode: simulatorSelection?.mode,
                simulatorID: simulatorSelection?.simulatorID
            )
        }
    }

    struct SimulatorSelection: Decodable {
        let mode: String
        let simulatorID: String?

        enum CodingKeys: String, CodingKey {
            case mode
            case simulatorID = "simulator_id"
        }
    }

    enum ShimEvent: Decodable {
        case shimInvocation(ShimInvocation)
        case warning(ShimWarning)

        enum CodingKeys: String, CodingKey {
            case kind
        }

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            let kind = try container.decode(String.self, forKey: .kind)
            switch kind {
            case "shim_invocation":
                self = .shimInvocation(try ShimInvocation(from: decoder))
            case "warning":
                self = .warning(try ShimWarning(from: decoder))
            default:
                self = .warning(ShimWarning(
                    policyReason: "Unknown shim event kind: \(kind)",
                    sourceField: "kind",
                    matchedSubstring: kind,
                    excerpt: kind
                ))
            }
        }
    }

    struct ShimInvocation: Decodable {
        let tool: String
        let policyDecision: String
        let policyReason: String
        let exitStatus: Int

        enum CodingKeys: String, CodingKey {
            case tool
            case policyDecision = "policy_decision"
            case policyReason = "policy_reason"
            case exitStatus = "exit_status"
        }
    }

    struct ShimWarning: Decodable {
        let policyReason: String
        let sourceField: String
        let matchedSubstring: String
        let excerpt: String

        enum CodingKeys: String, CodingKey {
            case policyReason = "policy_reason"
            case sourceField = "source_field"
            case matchedSubstring = "matched_substring"
            case excerpt
        }
    }

    struct HostExecutorEvent: Decodable {
        let tool: String
        let hostEnvDisposition: String
        let selectedSimulatorID: String?
        let exitStatus: Int
        let durationMilliseconds: Int

        enum CodingKeys: String, CodingKey {
            case tool
            case hostEnvDisposition = "host_env_disposition"
            case selectedSimulatorID = "selected_simulator_id"
            case exitStatus = "exit_status"
            case durationMilliseconds = "duration_ms"
        }

        var projection: WorkflowMapXcodeHostExecutorEvent {
            WorkflowMapXcodeHostExecutorEvent(
                tool: tool,
                hostEnvDisposition: hostEnvDisposition,
                selectedSimulatorID: selectedSimulatorID,
                exitStatus: exitStatus,
                durationMilliseconds: durationMilliseconds
            )
        }
    }

    struct Storage: Decodable {
        let truncated: Bool?
        let totalEventsDropped: Int?
        let corruptJSONRecoveryCount: Int?

        enum CodingKeys: String, CodingKey {
            case truncated
            case totalEventsDropped = "total_events_dropped"
            case corruptJSONRecoveryCount = "corrupt_json_recovery_count"
        }

        var projection: WorkflowMapXcodeRuntimeStorageStatus {
            WorkflowMapXcodeRuntimeStorageStatus(
                truncated: truncated ?? false,
                totalEventsDropped: totalEventsDropped ?? 0,
                corruptJSONRecoveryCount: corruptJSONRecoveryCount ?? 0
            )
        }
    }
}

struct WorkflowMapStageProjection: Identifiable, Sendable {
    let id: String
    let label: String
    let order: Int
    let type: StateType?
    let ownerAgentID: String
    let ownerAgentTitle: String
    let status: WorkflowMapStageState
    let iteration: Int
    let attemptNumber: Int
    let approvalRequired: Bool
    let isCurrent: Bool
    let occurrences: [WorkflowMapOccurrenceProjection]
    let handoffs: [WorkflowMapEdge]
    let transitions: [WorkflowMapEdge]
    let loopTelemetry: WorkflowMapLoopTelemetry?

    var communicationCount: Int {
        handoffs.count + transitions.count + occurrences.count
    }
}

struct WorkflowMapOccurrenceProjection: Identifiable, Sendable {
    let id: String
    let agentID: String
    let agentTitle: String
    let taskName: String
    let stageID: String
    let stageLabel: String
    let state: WorkflowMapOccurrenceState
    let provider: String
    let model: String
    let effort: String
    let executionCount: Int
    let startedAt: Date?
    let completedAt: Date?
    let sessionID: String?
    let requestID: String?
    let logSnippet: String?
    let ordinal: Int
}

struct WorkflowMapEdge: Identifiable, Sendable {
    let id: String
    let kind: WorkflowMapEdgeKind
    let fromLabel: String
    let toLabel: String
    let fromStageID: String
    let toStageID: String
    let count: Int
    let detail: String?
}

struct WorkflowMapLoopTelemetry: Identifiable, Sendable {
    let id: String
    let counter: String
    let current: Int
    let max: Int
    let stageID: String
    let stageLabel: String
    let exhausted: Bool
    let progress: Double
}

struct WorkflowMapTopologyBuilder {
    let plan: RunPlan

    func orderedStateIDs() -> [String] {
        var ordered: [String] = []
        var seen = Set<String>()
        var queue: [String] = [plan.initialStateID]

        while let stateID = queue.first {
            queue.removeFirst()
            guard seen.insert(stateID).inserted else { continue }
            ordered.append(stateID)

            guard let state = plan.states[stateID] else { continue }
            for transition in state.transitions {
                if !seen.contains(transition.to) {
                    queue.append(transition.to)
                }
            }
        }

        let remaining = plan.states.keys.sorted().filter { !seen.contains($0) }
        ordered.append(contentsOf: remaining)
        return ordered
    }

    func stageIndexMap() -> [String: Int] {
        Dictionary(uniqueKeysWithValues: orderedStateIDs().enumerated().map { ($1, $0) })
    }

    func transitionEdges(for stateID: String, state: ExecutableState) -> [WorkflowMapEdge] {
        let sourceLabel = state.label
        return state.transitions.enumerated().map { index, transition in
            WorkflowMapEdge(
                id: "\(stateID)::transition::\(index)::\(transition.to)",
                kind: .transition,
                fromLabel: sourceLabel,
                toLabel: plan.states[transition.to]?.label ?? transition.to,
                fromStageID: stateID,
                toStageID: transition.to,
                count: 1,
                detail: conditionSummary(for: transition.condition)
            )
        }
    }

    func loopTelemetry(for stateID: String, state: ExecutableState, run: Run) -> WorkflowMapLoopTelemetry? {
        guard let loop = state.loop else { return nil }
        let current = run.loopCounters[loop.counter] ?? 0
        let max = max(1, loop.resolvedMax)
        let progress = min(1.0, Double(current) / Double(max))
        return WorkflowMapLoopTelemetry(
            id: "\(stateID)::loop::\(loop.counter)",
            counter: loop.counter,
            current: current,
            max: max,
            stageID: stateID,
            stageLabel: state.label,
            exhausted: current >= max,
            progress: progress
        )
    }

    private func conditionSummary(for condition: TransitionCondition) -> String? {
        switch condition {
        case .always:
            return "always"
        case .artifactExists(let name):
            return "exists(\(name))"
        case .approvalGranted:
            return "approval granted"
        case .approvalRejected:
            return "approval rejected"
        case .expression(let expr):
            return expr
        }
    }
}
