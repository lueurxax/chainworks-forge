import Foundation
import SwiftData

struct RunStageAgentSnapshot: Identifiable, Sendable {
    let id: UUID
    let agentID: String
    let agentTitle: String
    let taskName: String
    let agentAttemptNumber: Int?
    let supersedesAgentExecutionID: UUID?
    let startedAt: Date
    let completedAt: Date?
    let status: AgentStatus
    let provider: String
    let effort: String
    let runtimeSessionID: String?
    let costCents: Int64?
    let logSnippet: String?
    let resolvedModel: String?
    let providerReceiptPresent: Bool
    let sessionLineageID: UUID?
    let retryReason: String?
    let canonicalOutcome: AgentCanonicalOutcome?
    let supervisionClassification: SupervisionClassification?
    let transportErrorKind: TransportErrorKind?
    let outputPresence: OutputPresence?
    let providerStopReason: String?
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
    let recoverySnapshotJSON: Data?
    let agentExecutions: [RunStageAgentSnapshot]
}

struct RunLatestStageStatusSnapshot: Sendable {
    let stageID: String
    let startedAt: Date
    let status: StageStatus
}

@MainActor
enum RunStageSnapshotLoader {
    private struct CacheEntry {
        let snapshots: [RunStageSnapshot]
        let cachedAt: TimeInterval
    }

    private static let cacheTTL: TimeInterval = 0.25
    private static var cache: [UUID: CacheEntry] = [:]

    #if DEBUG
    private static var loadInvocationCount = 0
    #endif

    static func load(for runID: UUID, modelContext: ModelContext) -> [RunStageSnapshot] {
        if let cached = cachedSnapshots(for: runID) {
            return cached
        }

        #if DEBUG
        loadInvocationCount += 1
        #endif
        let descriptor = FetchDescriptor<StageExecution>(
            predicate: #Predicate<StageExecution> { stage in
                stage.run?.id == runID
            },
            sortBy: [
                SortDescriptor(\.startedAt, order: .forward),
                SortDescriptor(\.iteration, order: .forward),
                SortDescriptor(\.attemptNumber, order: .forward)
            ]
        )

        let snapshots = ((try? modelContext.fetch(descriptor)) ?? []).map(makeSnapshot)
        storeSnapshots(snapshots, for: runID)
        return snapshots
    }

    static func load(for run: Run, modelContext: ModelContext) -> [RunStageSnapshot] {
        load(for: run.id, modelContext: modelContext)
    }

    static func load(for run: Run) -> [RunStageSnapshot] {
        if let cached = cachedSnapshots(for: run.id) {
            return cached
        }

        if let modelContext = run.modelContext {
            let freshContext = ModelContext(modelContext.container)
            let persisted = load(for: run.id, modelContext: freshContext)
            if !persisted.isEmpty || run.stageExecutions.isEmpty {
                return persisted
            }
        }

        let snapshots = run.stageExecutions
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
        storeSnapshots(snapshots, for: run.id)
        return snapshots
    }

    #if DEBUG
    static func resetLoadInvocationCountForTesting() {
        loadInvocationCount = 0
    }

    static var loadInvocationCountForTesting: Int {
        loadInvocationCount
    }

    static func resetCacheForTesting() {
        cache.removeAll()
    }

    static var cacheEntryCountForTesting: Int {
        cache.count
    }
    #endif

    private static func cachedSnapshots(for runID: UUID) -> [RunStageSnapshot]? {
        guard let entry = cache[runID] else { return nil }
        let age = Date().timeIntervalSinceReferenceDate - entry.cachedAt
        guard age <= cacheTTL else {
            cache.removeValue(forKey: runID)
            return nil
        }
        return entry.snapshots
    }

    private static func storeSnapshots(_ snapshots: [RunStageSnapshot], for runID: UUID) {
        cache[runID] = CacheEntry(
            snapshots: snapshots,
            cachedAt: Date().timeIntervalSinceReferenceDate
        )
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
            recoverySnapshotJSON: stage.recoverySnapshotJSON,
            agentExecutions: sortedAgents.map { agent in
                RunStageAgentSnapshot(
                    id: agent.id,
                    agentID: agent.agentID,
                    agentTitle: agent.agentTitle,
                    taskName: agent.taskName,
                    agentAttemptNumber: agent.agentAttemptNumber,
                    supersedesAgentExecutionID: agent.supersedesAgentExecutionID,
                    startedAt: agent.startedAt,
                    completedAt: agent.completedAt,
                    status: agent.status,
                    provider: agent.provider,
                    effort: agent.effort,
                    runtimeSessionID: agent.runtimeSessionID,
                    costCents: agent.costCents,
                    logSnippet: agent.logSnippet,
                    resolvedModel: agent.resolvedModel,
                    providerReceiptPresent: agent.providerReceiptJSON != nil,
                    sessionLineageID: agent.sessionLineageID,
                    retryReason: agent.retryReason,
                    canonicalOutcome: agent.canonicalOutcome,
                    supervisionClassification: agent.supervisionClassification,
                    transportErrorKind: agent.transportErrorKind,
                    outputPresence: agent.outputPresence,
                    providerStopReason: agent.providerStopReason
                )
            }
        )
    }
}

@MainActor
enum RunLatestStageStatusLoader {
    private struct CacheEntry {
        let snapshot: RunLatestStageStatusSnapshot?
        let cachedAt: TimeInterval
    }

    private static let cacheTTL: TimeInterval = 0.25
    private static var cache: [UUID: CacheEntry] = [:]

    #if DEBUG
    private static var loadInvocationCount = 0
    #endif

    static func load(for runID: UUID, modelContext: ModelContext) -> RunLatestStageStatusSnapshot? {
        if let cached = cachedSnapshot(for: runID) {
            return cached
        }

        #if DEBUG
        loadInvocationCount += 1
        #endif

        var descriptor = FetchDescriptor<StageExecution>(
            predicate: #Predicate<StageExecution> { stage in
                stage.run?.id == runID
            },
            sortBy: [
                SortDescriptor(\.startedAt, order: .reverse),
                SortDescriptor(\.iteration, order: .reverse),
                SortDescriptor(\.attemptNumber, order: .reverse)
            ]
        )
        descriptor.fetchLimit = 1

        let snapshot = ((try? modelContext.fetch(descriptor)) ?? []).first.map(makeSnapshot)
        storeSnapshot(snapshot, for: runID)
        return snapshot
    }

    static func load(for run: Run, modelContext: ModelContext) -> RunLatestStageStatusSnapshot? {
        load(for: run.id, modelContext: modelContext)
    }

    #if DEBUG
    static func resetLoadInvocationCountForTesting() {
        loadInvocationCount = 0
    }

    static var loadInvocationCountForTesting: Int {
        loadInvocationCount
    }

    static func resetCacheForTesting() {
        cache.removeAll()
    }
    #endif

    private static func cachedSnapshot(for runID: UUID) -> RunLatestStageStatusSnapshot?? {
        guard let entry = cache[runID] else { return nil }
        let age = Date().timeIntervalSinceReferenceDate - entry.cachedAt
        guard age <= cacheTTL else {
            cache.removeValue(forKey: runID)
            return nil
        }
        return entry.snapshot
    }

    private static func storeSnapshot(_ snapshot: RunLatestStageStatusSnapshot?, for runID: UUID) {
        cache[runID] = CacheEntry(
            snapshot: snapshot,
            cachedAt: Date().timeIntervalSinceReferenceDate
        )
    }

    private static func makeSnapshot(_ stage: StageExecution) -> RunLatestStageStatusSnapshot {
        RunLatestStageStatusSnapshot(
            stageID: stage.stageID,
            startedAt: stage.startedAt,
            status: stage.status
        )
    }
}
