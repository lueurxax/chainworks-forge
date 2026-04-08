import Foundation
import SwiftData

struct RunStageAgentSnapshot: Identifiable, Sendable {
    let id: UUID
    let agentID: String
    let agentTitle: String
    let taskName: String
    let startedAt: Date
    let completedAt: Date?
    let status: AgentStatus
    let provider: String
    let effort: String
    let costCents: Int64?
    let logSnippet: String?
    let resolvedModel: String?
    let providerReceiptPresent: Bool
    let sessionLineageID: UUID?
}

struct RunStageSnapshot: Identifiable, Sendable {
    let id: UUID
    let stageID: String
    let label: String
    let startedAt: Date
    let completedAt: Date?
    let status: StageStatus
    let iteration: Int
    let attemptNumber: Int
    let agentExecutions: [RunStageAgentSnapshot]
}

@MainActor
enum RunStageSnapshotLoader {
    static func load(for runID: UUID, modelContext: ModelContext) -> [RunStageSnapshot] {
        let descriptor = FetchDescriptor<StageExecution>(
            sortBy: [
                SortDescriptor(\.startedAt, order: .forward),
                SortDescriptor(\.iteration, order: .forward),
                SortDescriptor(\.attemptNumber, order: .forward)
            ]
        )

        let stages = ((try? modelContext.fetch(descriptor)) ?? []).filter { $0.run?.id == runID }
        return stages.map(makeSnapshot)
    }

    static func load(for run: Run, modelContext: ModelContext) -> [RunStageSnapshot] {
        load(for: run.id, modelContext: modelContext)
    }

    static func load(for run: Run) -> [RunStageSnapshot] {
        if let modelContext = run.modelContext {
            let freshContext = ModelContext(modelContext.container)
            return load(for: run.id, modelContext: freshContext)
        }

        return run.stageExecutions
            .sorted {
                if $0.startedAt == $1.startedAt {
                    if $0.iteration == $1.iteration {
                        return $0.attemptNumber < $1.attemptNumber
                    }
                    return $0.iteration < $1.iteration
                }
                return $0.startedAt < $1.startedAt
            }
            .map(makeSnapshot)
    }

    private static func makeSnapshot(_ stage: StageExecution) -> RunStageSnapshot {
        let sortedAgents = stage.agentExecutions.sorted { lhs, rhs in
            if lhs.startedAt == rhs.startedAt {
                return lhs.agentTitle.localizedStandardCompare(rhs.agentTitle) == .orderedAscending
            }
            return lhs.startedAt < rhs.startedAt
        }

        return RunStageSnapshot(
            id: stage.id,
            stageID: stage.stageID,
            label: stage.label,
            startedAt: stage.startedAt,
            completedAt: stage.completedAt,
            status: stage.status,
            iteration: stage.iteration,
            attemptNumber: stage.attemptNumber,
            agentExecutions: sortedAgents.map { agent in
                RunStageAgentSnapshot(
                    id: agent.id,
                    agentID: agent.agentID,
                    agentTitle: agent.agentTitle,
                    taskName: agent.taskName,
                    startedAt: agent.startedAt,
                    completedAt: agent.completedAt,
                    status: agent.status,
                    provider: agent.provider,
                    effort: agent.effort,
                    costCents: agent.costCents,
                    logSnippet: agent.logSnippet,
                    resolvedModel: agent.resolvedModel,
                    providerReceiptPresent: agent.providerReceiptJSON != nil,
                    sessionLineageID: agent.sessionLineageID
                )
            }
        )
    }
}
