import Combine
import Foundation

// MARK: - P083 identity hold sessions model

/// P083 manual_process_identity_check_ui_v1: Fetches identity-ambiguous provider sessions
/// for the currently-selected run and publishes them for the ManualProcessIdentityCheckBanner.
///
/// Surfaces: run detail overview, stage detail provider-session row, recovery inbox.
/// Data source: p083IdentityHoldSessions GraphQL query (backend truth only; no local caching).
/// Read-only: lifecycle mutations (markProcessAbsent) route through the external MCP command
/// path, not through governed SwiftUI, per docs/reference/ui-action-boundary.md.
@MainActor
final class P083IdentityHoldSessionsModel: ObservableObject {
    @Published var sessions: [P083IdentityAmbiguousInboxView.IdentityAmbiguousSession] = []

    private let readClient: P031GraphQLReadClient<P031URLSessionGraphQLReadTransport>
    private var fetchTask: Task<Void, Never>?
    private var currentRunID: String?

    init(endpoint: DaemonClientEndpoint) {
        let transport = P031URLSessionGraphQLReadTransport(endpoint: endpoint)
        self.readClient = P031GraphQLReadClient(transport: transport)
    }

    func updateSelectedRun(_ runID: String?) {
        fetchTask?.cancel()
        currentRunID = runID
        sessions = []
        guard let runID else { return }
        fetchTask = Task { await load(runID: runID) }
    }

    /// Re-fetches identity hold sessions for the current run without clearing the displayed list
    /// first. Used by the Retry Identity Check action in ManualProcessIdentityCheckBanner per
    /// manual_process_identity_check_ui_v1: a read-only probe that refreshes backend readback.
    func refreshCurrentRun() {
        guard let runID = currentRunID else { return }
        fetchTask?.cancel()
        fetchTask = Task { await load(runID: runID) }
    }

    private func load(runID: String) async {
        guard !Task.isCancelled else { return }
        do {
            let result = try await readClient.execute(
                SessionsPayload.self,
                operationName: "P083IdentityHoldSessions",
                document: Self.document,
                variables: ["runId": .string(runID)]
            )
            guard !Task.isCancelled else { return }
            sessions = result.p083IdentityHoldSessions.map { gql in
                P083IdentityAmbiguousInboxView.IdentityAmbiguousSession(
                    id: gql.providerSessionId,
                    providerName: gql.providerName,
                    cancellationEpoch: gql.cancellationEpoch,
                    lastSeenPid: gql.lastSeenPid.map { Int($0) },
                    processStartIdentityHash: gql.processStartIdentityHash,
                    latestReceiptId: gql.latestReceiptId,
                    reasonDetail: gql.reasonDetail
                )
            }
        } catch {
            sessions = []
        }
    }

    static func noOp() -> P083IdentityHoldSessionsModel {
        P083IdentityHoldSessionsModel(endpoint: DaemonClientEndpoint.operatorDefault())
    }

    private static let document = """
        query P083IdentityHoldSessions($runId: ID!) {
            p083IdentityHoldSessions(runId: $runId) {
                providerSessionId
                providerName
                cancellationEpoch
                lastSeenPid
                processStartIdentityHash
                latestReceiptId
                reasonDetail
            }
        }
        """

    private struct SessionsPayload: Decodable {
        let p083IdentityHoldSessions: [GqlSession]

        struct GqlSession: Decodable {
            let providerSessionId: String
            let providerName: String
            let cancellationEpoch: Int?
            let lastSeenPid: Int?
            let processStartIdentityHash: String?
            let latestReceiptId: String?
            let reasonDetail: String?
        }
    }
}
