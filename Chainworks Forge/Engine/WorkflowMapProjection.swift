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
