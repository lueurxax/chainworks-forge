import Combine
import Foundation

// MARK: - P046 read models

struct P046SessionLineageReadModel: Decodable, Identifiable, Sendable {
    let id: String
    let agentId: String
    let lineageKey: String?
    let sessionReuseScope: String?
    let activeGenerationId: String?
    let generationCount: Int?
    let latestEventAt: String?
    let healthState: String?
    let createdAt: String?
}

struct P046SessionLineageConnectionReadModel: Decodable, Sendable {
    struct PageInfo: Decodable, Sendable {
        let hasNextPage: Bool
        let endCursor: String?
    }
    let nodes: [P046SessionLineageReadModel]
    let pageInfo: PageInfo
}

struct P046SessionKpiSummaryReadModel: Decodable, Sendable {
    let runId: String
    let lineageCount: Int
    let generationCount: Int
    let activeGenerationCount: Int
    let closedGenerationCount: Int?
    let totalTurnCount: Int?
    let totalCostCents: Int?
    let latestActivityAt: String?
}

struct P046SessionHealthWarningReadModel: Decodable, Sendable {
    let reasonCode: String
    let severity: String
    let message: String?
}

struct P046SessionHealthReadModel: Decodable, Sendable {
    let runId: String
    let state: String
    let thresholdsVersion: String?
    let warnings: [P046SessionHealthWarningReadModel]
    let checkedAt: String?
}

struct P046SessionStatusChangedReadModel: Decodable, Sendable {
    let runId: String
    let lineageId: String?
    let generationId: String?
    let eventId: String?
    let status: String
    let recordedAt: String?
    let resyncRequired: Bool
}

// MARK: - P046 store protocol

// Separate from P031WorkflowReadStore so it can be conditionally adopted
// by stores that back a P046-capable daemon.
protocol P046SessionStore: Sendable {
    // Capability probe: returns true when P046 fields are present in the server schema.
    // Must be called before any P046 query/subscription document is constructed or sent.
    func checkP046SchemaAvailability() async throws -> Bool
    func fetchP046SessionLineages(runID: String) async throws -> P046SessionLineageConnectionReadModel
    func fetchP046SessionKpiSummary(runID: String) async throws -> P046SessionKpiSummaryReadModel
    func fetchP046SessionHealth(runID: String) async throws -> P046SessionHealthReadModel
    nonisolated func subscribeP046SessionStatusChanged(
        runID: String
    ) throws -> AsyncThrowingStream<P046SessionStatusChangedReadModel, Error>
}

// MARK: - GraphQL documents

nonisolated enum P046GraphQLDocuments {
    // Lightweight capability probe: queries sessionObservabilityAvailable which is
    // absent from the schema when P046 is disabled. A "Cannot query field" error
    // means no P046 documents should be issued to this daemon.
    static let capabilityCheck = """
        query P046CapabilityCheck {
          sessionObservabilityAvailable
        }
        """

    static let sessionLineages = """
        query P046SessionLineages($runId: ID!, $first: Int) {
          sessionLineages(runId: $runId, first: $first) {
            nodes {
              id
              agentId
              lineageKey
              sessionReuseScope
              activeGenerationId
              generationCount
              latestEventAt
              healthState
              createdAt
            }
            pageInfo {
              hasNextPage
              endCursor
            }
          }
        }
        """

    static let sessionKpiSummary = """
        query P046SessionKpiSummary($runId: ID!) {
          sessionKpiSummary(runId: $runId) {
            runId
            lineageCount
            generationCount
            activeGenerationCount
            closedGenerationCount
            totalTurnCount
            totalCostCents
            latestActivityAt
          }
        }
        """

    static let sessionHealth = """
        query P046SessionHealth($runId: ID!) {
          sessionHealth(runId: $runId) {
            runId
            state
            thresholdsVersion
            warnings {
              reasonCode
              severity
              message
            }
            checkedAt
          }
        }
        """

    static let sessionStatusChanged = """
        subscription P046SessionStatusChanged($runId: ID!) {
          sessionStatusChanged(runId: $runId) {
            runId
            lineageId
            generationId
            eventId
            status
            recordedAt
            resyncRequired
          }
        }
        """
}

// MARK: - P046SessionObservabilityModel

// MainActor-scoped session observability model owned by the selected-run detail coordinator.
// P046 state is transient UI state — never persisted to SwiftData.
// Capability discovery runs before issuing any P046 GraphQL documents.
@MainActor
final class P046SessionObservabilityModel: ObservableObject {

    enum Availability: Equatable {
        case unknown
        case available
        case unavailable  // P046 fields absent from schema or disabled-schema rollback mode
    }

    @Published private(set) var availability: Availability = .unknown
    @Published private(set) var lineages: [P046SessionLineageReadModel] = []
    @Published private(set) var kpiSummary: P046SessionKpiSummaryReadModel?
    @Published private(set) var health: P046SessionHealthReadModel?
    @Published private(set) var isLoading = false
    @Published private(set) var isStale = false
    @Published private(set) var loadError: String?

    private let checkCapability: @Sendable () async throws -> Bool
    private let fetchLineages: @Sendable (String) async throws -> P046SessionLineageConnectionReadModel
    private let fetchKpi: @Sendable (String) async throws -> P046SessionKpiSummaryReadModel
    private let fetchHealth: @Sendable (String) async throws -> P046SessionHealthReadModel
    private let subscribeStatus: @Sendable (String) throws -> AsyncThrowingStream<P046SessionStatusChangedReadModel, Error>

    private var observationTask: Task<Void, Never>?
    private var observedRunID: String?
    private var seenSubscriptionEventKeys: Set<String> = []

    init(
        checkCapability: @escaping @Sendable () async throws -> Bool,
        fetchLineages: @escaping @Sendable (String) async throws -> P046SessionLineageConnectionReadModel,
        fetchKpi: @escaping @Sendable (String) async throws -> P046SessionKpiSummaryReadModel,
        fetchHealth: @escaping @Sendable (String) async throws -> P046SessionHealthReadModel,
        subscribeStatus: @escaping @Sendable (String) throws -> AsyncThrowingStream<P046SessionStatusChangedReadModel, Error>
    ) {
        self.checkCapability = checkCapability
        self.fetchLineages = fetchLineages
        self.fetchKpi = fetchKpi
        self.fetchHealth = fetchHealth
        self.subscribeStatus = subscribeStatus
    }

    static func make<Store: P046SessionStore>(store: Store) -> P046SessionObservabilityModel {
        P046SessionObservabilityModel(
            checkCapability: { try await store.checkP046SchemaAvailability() },
            fetchLineages: { runID in try await store.fetchP046SessionLineages(runID: runID) },
            fetchKpi: { runID in try await store.fetchP046SessionKpiSummary(runID: runID) },
            fetchHealth: { runID in try await store.fetchP046SessionHealth(runID: runID) },
            subscribeStatus: { runID in try store.subscribeP046SessionStatusChanged(runID: runID) }
        )
    }

    // No-op model for preview/test contexts where no P046 store is available.
    // Reports availability=.unavailable immediately and performs no network calls.
    static func noOp() -> P046SessionObservabilityModel {
        P046SessionObservabilityModel(
            checkCapability: { false },
            fetchLineages: { _ in
                P046SessionLineageConnectionReadModel(
                    nodes: [],
                    pageInfo: P046SessionLineageConnectionReadModel.PageInfo(
                        hasNextPage: false, endCursor: nil
                    )
                )
            },
            fetchKpi: { runID in
                P046SessionKpiSummaryReadModel(
                    runId: runID, lineageCount: 0, generationCount: 0,
                    activeGenerationCount: 0, closedGenerationCount: nil,
                    totalTurnCount: nil, totalCostCents: nil, latestActivityAt: nil
                )
            },
            fetchHealth: { runID in
                P046SessionHealthReadModel(
                    runId: runID, state: "UNKNOWN", thresholdsVersion: nil,
                    warnings: [], checkedAt: nil
                )
            },
            subscribeStatus: { _ in
                AsyncThrowingStream { _ in }
            }
        )
    }

    deinit {
        observationTask?.cancel()
    }

    // Call when the selected run changes. Cancels prior task and starts fresh.
    // Use .task(id: runID) in SwiftUI to drive this.
    func updateSelectedRun(_ runID: String?) {
        guard observedRunID != runID else { return }
        observationTask?.cancel()
        observationTask = nil
        observedRunID = runID
        seenSubscriptionEventKeys.removeAll()
        lineages = []
        kpiSummary = nil
        health = nil
        isStale = false
        loadError = nil

        guard let runID else {
            availability = .unknown
            isLoading = false
            return
        }

        observationTask = Task { [weak self] in
            guard let self else { return }
            await self.runObservation(runID: runID)
        }
    }

    private func runObservation(runID: String) async {
        isLoading = true
        loadError = nil

        // Capability discovery precedes all P046 document construction.
        // checkCapability issues only the lightweight sessionObservabilityAvailable probe —
        // a "Cannot query field" response means the schema has P046 disabled.
        do {
            try Task.checkCancellation()
            let available = try await checkCapability()
            if !available {
                availability = .unavailable
                isLoading = false
                return
            }
        } catch is CancellationError {
            isLoading = false
            return
        } catch {
            isLoading = false
            if isP046DisabledSchemaError(error) {
                availability = .unavailable
                return
            }
            loadError = Self.describeError(error)
            return
        }

        // Schema confirmed available: now issue full P046 documents.
        do {
            try Task.checkCancellation()
            async let lineagesResult = fetchLineages(runID)
            async let kpiResult = fetchKpi(runID)
            async let healthResult = fetchHealth(runID)
            let (conn, kpi, hlth) = try await (lineagesResult, kpiResult, healthResult)
            try Task.checkCancellation()
            lineages = conn.nodes
            kpiSummary = kpi
            health = hlth
            availability = .available
            isLoading = false
            isStale = false
        } catch is CancellationError {
            isLoading = false
            return
        } catch {
            isLoading = false
            if isP046DisabledSchemaError(error) {
                availability = .unavailable
                return
            }
            loadError = Self.describeError(error)
            return
        }

        // Subscribe for live notifications. On resync, re-query before resuming rendering.
        do {
            let stream = try subscribeStatus(runID)
            for try await event in stream {
                try Task.checkCancellation()
                guard event.runId == runID else { continue }
                guard markSubscriptionEventIfNew(event) else { continue }
                if event.resyncRequired {
                    isStale = true
                    if await requery(runID: runID) {
                        isStale = false
                    }
                } else {
                    _ = await requery(runID: runID)
                }
            }
            isStale = true
            if await requery(runID: runID) {
                isStale = false
            }
        } catch is CancellationError {
            return
        } catch {
            isStale = true
            if await requery(runID: runID) {
                isStale = false
            }
        }
    }

    private func requery(runID: String) async -> Bool {
        do {
            async let lineagesResult = fetchLineages(runID)
            async let kpiResult = fetchKpi(runID)
            async let healthResult = fetchHealth(runID)
            let (conn, kpi, hlth) = try await (lineagesResult, kpiResult, healthResult)
            try Task.checkCancellation()
            lineages = conn.nodes
            kpiSummary = kpi
            health = hlth
            loadError = nil
            return true
        } catch is CancellationError {
            return false
        } catch {
            loadError = Self.describeError(error)
            return false
        }
    }

    private func markSubscriptionEventIfNew(_ event: P046SessionStatusChangedReadModel) -> Bool {
        let key: String
        if let eventId = event.eventId, !eventId.isEmpty {
            key = "event:\(eventId)"
        } else {
            key = [
                "fallback",
                event.lineageId ?? "_",
                event.generationId ?? "_",
                event.recordedAt ?? "_",
                event.status,
                event.resyncRequired ? "resync" : "status",
            ].joined(separator: "\u{1f}")
        }
        return seenSubscriptionEventKeys.insert(key).inserted
    }

    // Returns true when the error indicates P046 fields are absent from the schema.
    // In that case no further P046 documents should be sent to this daemon.
    private func isP046DisabledSchemaError(_ error: Error) -> Bool {
        guard case P031GraphQLReadBoundaryError.graphqlErrors(let messages) = error else {
            return false
        }
        let p046Fields = [
            "sessionObservabilityAvailable",
            "sessionLineages", "sessionKpiSummary", "sessionHealth",
            "sessionStatusChanged", "sessionLineage", "sessionGenerations", "sessionEvents",
        ]
        return messages.contains { msg in
            let lower = msg.lowercased()
            let isSchemaError = lower.contains("cannot query field")
                || lower.contains("unknown field")
                || lower.contains("p046")
                || lower.contains("disabled")
            return isSchemaError && p046Fields.contains { lower.contains($0.lowercased()) }
        }
    }

    private static func describeError(_ error: Error) -> String {
        switch error {
        case P031GraphQLReadBoundaryError.graphqlErrors(let msgs):
            return msgs.first ?? "session observability query failed"
        default:
            return error.localizedDescription
        }
    }
}
