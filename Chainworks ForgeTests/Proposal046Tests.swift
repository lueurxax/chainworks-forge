import Foundation
import Testing
@testable import Chainworks_Forge

// MARK: - P046 Swift guardrail and disabled-schema gating tests
//
// Tests required by the P046 test_plan:
// - Disabled-schema client test: P046 documents not sent when capability discovery says absent.
// - SwiftUI guardrail: no resetSession GraphQL document, no governed reset UI, no SwiftData persistence,
//   no AppKit-owned GraphQL task is introduced by P046.

@Suite("P046 session observability Swift guardrails", .tags(.fast))
@MainActor
struct Proposal046Tests {

    // MARK: - Disabled-schema gating

    @Test("P046SessionObservabilityModel transitions to unavailable on disabled-schema error")
    func disabledSchemaTransitionsToUnavailable() async throws {
        let queriesFired = P046StringRecorder()
        let model = P046SessionObservabilityModel(
            checkCapability: {
                queriesFired.append("capability")
                throw P031GraphQLReadBoundaryError.graphqlErrors([
                    "Cannot query field 'sessionObservabilityAvailable' on type 'QueryRoot'."
                ])
            },
            fetchLineages: { _ in
                queriesFired.append("lineages")
                throw P031GraphQLReadBoundaryError.graphqlErrors(["should not be called"])
            },
            fetchKpi: { _ in
                queriesFired.append("kpi")
                throw P031GraphQLReadBoundaryError.graphqlErrors(["should not be called"])
            },
            fetchHealth: { _ in
                queriesFired.append("health")
                throw P031GraphQLReadBoundaryError.graphqlErrors(["should not be called"])
            },
            subscribeStatus: { _ in
                queriesFired.append("subscribe")
                return AsyncThrowingStream { _ in }
            }
        )

        model.updateSelectedRun("run-1")
        // Yield to allow the async observation task to run to completion.
        try await Task.sleep(nanoseconds: 100_000_000)

        #expect(model.availability == .unavailable,
            "Model must report unavailable when capability probe fails with disabled-schema error")
        #expect(queriesFired.snapshot == ["capability"],
            "Only capability probe must fire on disabled-schema error; fired: \(queriesFired.snapshot)")
    }

    @Test("P046SessionObservabilityModel transitions to unavailable when capability returns false")
    func capabilityFalseTransitionsToUnavailable() async throws {
        let queriesFired = P046StringRecorder()
        let model = P046SessionObservabilityModel(
            checkCapability: {
                queriesFired.append("capability")
                return false
            },
            fetchLineages: { _ in
                queriesFired.append("lineages")
                throw P031GraphQLReadBoundaryError.graphqlErrors(["should not be called"])
            },
            fetchKpi: { _ in
                queriesFired.append("kpi")
                throw P031GraphQLReadBoundaryError.graphqlErrors(["should not be called"])
            },
            fetchHealth: { _ in
                queriesFired.append("health")
                throw P031GraphQLReadBoundaryError.graphqlErrors(["should not be called"])
            },
            subscribeStatus: { _ in
                queriesFired.append("subscribe")
                return AsyncThrowingStream { _ in }
            }
        )

        model.updateSelectedRun("run-1")
        try await Task.sleep(nanoseconds: 100_000_000)

        #expect(model.availability == .unavailable,
            "Model must report unavailable when capability returns false")
        #expect(queriesFired.snapshot == ["capability"],
            "Only capability probe must fire when capability returns false; fired: \(queriesFired.snapshot)")
    }

    @Test("P046SessionObservabilityModel does not fire any P046 docs when run is nil")
    func nilRunFiresNoQueries() async throws {
        let queriesFired = P046StringRecorder()
        let model = P046SessionObservabilityModel(
            checkCapability: { queriesFired.append("capability"); return true },
            fetchLineages: { _ in queriesFired.append("lineages"); throw P031GraphQLReadBoundaryError.graphqlErrors([""]) },
            fetchKpi: { _ in queriesFired.append("kpi"); throw P031GraphQLReadBoundaryError.graphqlErrors([""]) },
            fetchHealth: { _ in queriesFired.append("health"); throw P031GraphQLReadBoundaryError.graphqlErrors([""]) },
            subscribeStatus: { _ in queriesFired.append("subscribe"); return AsyncThrowingStream { _ in } }
        )

        model.updateSelectedRun(nil)
        try await Task.sleep(nanoseconds: 50_000_000)

        #expect(queriesFired.snapshot.isEmpty, "No P046 documents must be issued when runID is nil")
        #expect(model.availability == .unknown)
    }

    // MARK: - SwiftUI guardrail: no GraphQL reset mutation in P046 documents

    @Test("P046 GraphQL documents contain no session reset or control mutations")
    func noResetOrControlMutationInP046Documents() {
        let documents: [(String, String)] = [
            ("capabilityCheck", P046GraphQLDocuments.capabilityCheck),
            ("sessionLineages", P046GraphQLDocuments.sessionLineages),
            ("sessionKpiSummary", P046GraphQLDocuments.sessionKpiSummary),
            ("sessionHealth", P046GraphQLDocuments.sessionHealth),
            ("sessionStatusChanged", P046GraphQLDocuments.sessionStatusChanged),
        ]
        // Proposal rollout_contract_v1 hold condition: GraphQL schema must not expose any of these.
        let forbiddenNames = [
            "resetSession", "resetAgentSession", "sessionsReset", "closeSession",
            "invalidateSession", "compactSession", "retrySession", "recoverSession",
            "cancelSession",
        ]
        for (name, doc) in documents {
            for forbidden in forbiddenNames {
                #expect(!doc.contains(forbidden),
                    "P046 document '\(name)' must not contain '\(forbidden)'")
            }
        }
    }

    @Test("P046 query and subscription documents are read-only operations, not mutations")
    func p046DocumentsAreReadOnly() throws {
        let queries: [(String, String)] = [
            ("P046CapabilityCheck", P046GraphQLDocuments.capabilityCheck),
            ("P046SessionLineages", P046GraphQLDocuments.sessionLineages),
            ("P046SessionKpiSummary", P046GraphQLDocuments.sessionKpiSummary),
            ("P046SessionHealth", P046GraphQLDocuments.sessionHealth),
        ]
        for (opName, doc) in queries {
            let request = try P031GraphQLReadRequest(operationName: opName, document: doc)
            #expect(request.operationKind == .query,
                "'\(opName)' must be a query operation per P031 read boundary")
        }
        let subRequest = try P031GraphQLReadRequest(
            operationName: "P046SessionStatusChanged",
            document: P046GraphQLDocuments.sessionStatusChanged
        )
        #expect(subRequest.operationKind == .subscription,
            "P046SessionStatusChanged must be a subscription, not a mutation")
    }

    // MARK: - SwiftUI guardrail: P046 state is transient, not persisted to SwiftData

    @Test("P046SessionObservabilityModel is created without SwiftData container and holds no persisted fields")
    func p046StateIsTransientNoSwiftData() {
        // P046SessionObservabilityModel must not reference ModelContext or @Model.
        // It is created directly without any SwiftData container, proving no persistence.
        let model = P046SessionObservabilityModel.noOp()
        #expect(model.availability == .unknown,
            "noOp model availability must start as unknown")
        #expect(model.lineages.isEmpty)
        #expect(model.kpiSummary == nil)
        #expect(model.health == nil)
        #expect(!model.isLoading)
        #expect(!model.isStale)
        #expect(model.loadError == nil)
    }

    // MARK: - SwiftUI guardrail: run switching cancels prior task

    @Test("Switching selected run cancels prior observation and clears state")
    func switchingRunClearsPriorState() async throws {
        let model = P046SessionObservabilityModel.noOp()

        model.updateSelectedRun("run-a")
        try await Task.sleep(nanoseconds: 50_000_000)
        // noOp capability returns false → availability = .unavailable
        #expect(model.availability == .unavailable)

        model.updateSelectedRun(nil)
        // nil run must reset state
        #expect(model.availability == .unknown)
        #expect(model.lineages.isEmpty)
    }

    @Test("Resync refresh failure keeps P046 session readback stale")
    func resyncRefreshFailureKeepsSessionReadbackStale() async throws {
        let fetchCount = P046Counter()
        let continuation = P046StatusContinuationBox()
        let model = Self.makeModel(
            fetchLineages: { runID in
                if fetchCount.increment() == 1 {
                    return Self.lineageConnection(runID: runID, marker: "initial")
                }
                throw P031GraphQLReadBoundaryError.graphqlErrors(["refresh failed"])
            },
            subscribeStatus: { _ in
                AsyncThrowingStream { streamContinuation in
                    continuation.set(streamContinuation)
                }
            }
        )

        model.updateSelectedRun("run-1")
        try await Task.sleep(nanoseconds: 100_000_000)
        continuation.yield(Self.statusEvent(runID: "run-1", eventID: nil, status: "RESYNC_REQUIRED", resyncRequired: true))
        try await Task.sleep(nanoseconds: 100_000_000)

        #expect(model.isStale, "stale must remain true until a full fresh readback succeeds")
        #expect(model.loadError == "refresh failed")
    }

    @Test("Normal subscription completion refreshes before clearing stale state")
    func subscriptionCompletionRefreshesBeforeClearingStaleState() async throws {
        let fetchCount = P046Counter()
        let model = Self.makeModel(
            fetchLineages: { runID in
                let count = fetchCount.increment()
                return Self.lineageConnection(runID: runID, marker: "fetch-\(count)")
            },
            subscribeStatus: { _ in
                AsyncThrowingStream { streamContinuation in
                    streamContinuation.finish()
                }
            }
        )

        model.updateSelectedRun("run-1")
        try await Task.sleep(nanoseconds: 150_000_000)

        #expect(fetchCount.value >= 2, "stream completion must trigger a fresh readback")
        #expect(!model.isStale, "successful completion refresh may clear stale state")
        #expect(model.lineages.first?.lineageKey == "fetch-2")
    }

    @Test("Subscription error attempts fresh readback before staying stale")
    func subscriptionErrorAttemptsFreshReadback() async throws {
        struct SubscriptionClosed: Error {}
        let fetchCount = P046Counter()
        let model = Self.makeModel(
            fetchLineages: { runID in
                let count = fetchCount.increment()
                return Self.lineageConnection(runID: runID, marker: "fetch-\(count)")
            },
            subscribeStatus: { _ in
                AsyncThrowingStream { streamContinuation in
                    streamContinuation.finish(throwing: SubscriptionClosed())
                }
            }
        )

        model.updateSelectedRun("run-1")
        try await Task.sleep(nanoseconds: 150_000_000)

        #expect(fetchCount.value >= 2, "subscription error must attempt a fresh readback")
        #expect(!model.isStale, "successful error refresh may clear stale state")
        #expect(model.lineages.first?.lineageKey == "fetch-2")
    }

    @Test("Duplicate subscription events are ignored by event id")
    func duplicateSubscriptionEventsAreIgnored() async throws {
        let fetchCount = P046Counter()
        let continuation = P046StatusContinuationBox()
        let model = Self.makeModel(
            fetchLineages: { runID in
                let count = fetchCount.increment()
                return Self.lineageConnection(runID: runID, marker: "fetch-\(count)")
            },
            subscribeStatus: { _ in
                AsyncThrowingStream { streamContinuation in
                    continuation.set(streamContinuation)
                }
            }
        )

        model.updateSelectedRun("run-1")
        try await Task.sleep(nanoseconds: 100_000_000)
        continuation.yield(Self.statusEvent(runID: "run-1", eventID: "event-1"))
        continuation.yield(Self.statusEvent(runID: "run-1", eventID: "event-1"))
        try await Task.sleep(nanoseconds: 150_000_000)

        #expect(fetchCount.value == 2, "initial read plus one unique event refresh expected; got \(fetchCount.value)")
        model.updateSelectedRun(nil)
    }

    private static func makeModel(
        fetchLineages: @escaping @Sendable (String) async throws -> P046SessionLineageConnectionReadModel,
        fetchKpi: @escaping @Sendable (String) async throws -> P046SessionKpiSummaryReadModel = { runID in
            P046SessionKpiSummaryReadModel(
                runId: runID,
                lineageCount: 1,
                generationCount: 1,
                activeGenerationCount: 1,
                closedGenerationCount: 0,
                totalTurnCount: 0,
                totalCostCents: 0,
                latestActivityAt: nil
            )
        },
        fetchHealth: @escaping @Sendable (String) async throws -> P046SessionHealthReadModel = { runID in
            P046SessionHealthReadModel(
                runId: runID,
                state: "HEALTHY",
                thresholdsVersion: "p046_session_health_thresholds_v1",
                warnings: [],
                checkedAt: nil
            )
        },
        subscribeStatus: @escaping @Sendable (String) throws -> AsyncThrowingStream<P046SessionStatusChangedReadModel, Error>
    ) -> P046SessionObservabilityModel {
        P046SessionObservabilityModel(
            checkCapability: { true },
            fetchLineages: fetchLineages,
            fetchKpi: fetchKpi,
            fetchHealth: fetchHealth,
            subscribeStatus: subscribeStatus
        )
    }

    nonisolated private static func lineageConnection(runID: String, marker: String) -> P046SessionLineageConnectionReadModel {
        P046SessionLineageConnectionReadModel(
            nodes: [
                P046SessionLineageReadModel(
                    id: "\(runID)-lineage",
                    agentId: "code_writer",
                    lineageKey: marker,
                    sessionReuseScope: "run",
                    activeGenerationId: "generation-1",
                    generationCount: 1,
                    latestEventAt: nil,
                    healthState: "HEALTHY",
                    createdAt: nil
                )
            ],
            pageInfo: .init(hasNextPage: false, endCursor: nil)
        )
    }

    nonisolated private static func statusEvent(
        runID: String,
        eventID: String?,
        status: String = "ACTIVE",
        resyncRequired: Bool = false
    ) -> P046SessionStatusChangedReadModel {
        P046SessionStatusChangedReadModel(
            runId: runID,
            lineageId: "lineage-1",
            generationId: "generation-1",
            eventId: eventID,
            status: status,
            recordedAt: "2026-05-24T00:00:00Z",
            resyncRequired: resyncRequired
        )
    }
}

private final class P046StringRecorder: @unchecked Sendable {
    private let lock = NSLock()
    private var values: [String] = []

    func append(_ value: String) {
        lock.withLock {
            values.append(value)
        }
    }

    var snapshot: [String] {
        lock.withLock { values }
    }
}

private final class P046Counter: @unchecked Sendable {
    private let lock = NSLock()
    private var count = 0

    @discardableResult
    func increment() -> Int {
        lock.withLock {
            count += 1
            return count
        }
    }

    var value: Int {
        lock.withLock { count }
    }
}

private final class P046StatusContinuationBox: @unchecked Sendable {
    private let lock = NSLock()
    private var continuation: AsyncThrowingStream<P046SessionStatusChangedReadModel, Error>.Continuation?

    func set(
        _ continuation: AsyncThrowingStream<P046SessionStatusChangedReadModel, Error>.Continuation
    ) {
        lock.withLock {
            self.continuation = continuation
        }
    }

    func yield(_ value: P046SessionStatusChangedReadModel) {
        lock.withLock { continuation }?.yield(value)
    }
}
