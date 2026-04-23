// P042 Swift daemon lifecycle client (§5.2 / §5.3 / §7.3).
//
// Consolidated into one file so a single Xcode target-membership toggle
// picks up the whole module. All public types stay `internal`; nothing
// here is exported outside the app target.
//
// Surfaces exposed:
//
// - `DaemonStatus`, `DaemonLifecycleState`, `DegradedReason`,
//   `FailureReason`, etc. — wire-compatible Swift mirrors of the Rust
//   `domain::lifecycle` types. They decode the same snake_case JSON
//   that `/health`, `/ready`, and `daemonStatus.json` emit.
//
// - `DaemonPortFile` — reads
//   `~/Library/Application Support/Chainworks Forge/daemon.port`,
//   falling back to port 4000 on absent files per §7.3.
//
// - `DaemonLifecycleClient` — issues the `daemonStatus` GraphQL query
//   over HTTP with bearer auth. Returns a decoded `DaemonStatus` or a
//   typed error envelope. Never derives state from transport errors;
//   the view model renders "unavailable" when the HTTP call fails.
//
// - `DaemonStatusViewModel` — `@MainActor ObservableObject` that the UI
//   binds to. Exposes `status` + `lastError`; `isUnavailable` is the
//   UI's signal to render the unavailable panel (without synthesizing
//   a lifecycle state — §5.3 forbids that).

import Combine
import Foundation

// MARK: - Wire types (mirror domain::lifecycle)

/// Daemon lifecycle phase. Mirrors `DaemonLifecycleState` in the Rust
/// daemon's `domain::lifecycle` module.
enum DaemonLifecycleState: String, Codable, Sendable, CaseIterable {
    case notStarted = "not_started"
    case starting
    case ready
    case degraded
    case restarting
    case failed
    case shutdown

    /// Matches `DaemonLifecycleState::is_live` on the daemon side: a
    /// liveness probe should accept this state (§5.2).
    var isLive: Bool {
        switch self {
        case .ready, .degraded: return true
        default: return false
        }
    }

    /// Matches `is_terminal`. Recovery requires a restart.
    var isTerminal: Bool {
        switch self {
        case .failed, .shutdown: return true
        default: return false
        }
    }
}

/// Non-terminal reason the daemon is currently degraded.
enum DegradedKind: String, Codable, Sendable {
    case backgroundExecutorStalled = "background_executor_stalled"
    case acpRuntimeUnavailable = "acp_runtime_unavailable"
    case staleProjection = "stale_projection"
    case authPrincipalTableUnreadable = "auth_principal_table_unreadable"
    case diskSpaceLow = "disk_space_low"
}

/// Terminal failure reason. Every variant requires a daemon restart.
enum FailureKind: String, Codable, Sendable {
    case migrationFailed = "migration_failed"
    case schemaNewerThanBinary = "schema_newer_than_binary"
    case backupFailed = "backup_failed"
    case crashLoopBudgetExhausted = "crash_loop_budget_exhausted"
}

struct DegradedReason: Codable, Sendable, Equatable {
    let kind: DegradedKind
    let detail: String
    let since: Date
}

struct FailureReason: Codable, Sendable, Equatable {
    let kind: FailureKind
    let detail: String
    let since: Date
    let backupPath: String?

    enum CodingKeys: String, CodingKey {
        case kind
        case detail
        case since
        case backupPath = "backup_path"
    }
}

enum XcodeBrokerHealthState: String, Codable, Sendable {
    case disabled
    case healthy
    case degraded
    case failed
}

struct XcodeBrokerHealthSnapshot: Codable, Sendable, Equatable {
    let state: XcodeBrokerHealthState
    let poolID: String
    let activeLeases: Int
    let queuedLeases: Int
    let maxActiveLeases: Int
    let maxQueuedLeases: Int
    let brokerDisabled: Bool

    enum CodingKeys: String, CodingKey {
        case state
        case poolID = "pool_id"
        case activeLeases = "active_leases"
        case queuedLeases = "queued_leases"
        case maxActiveLeases = "max_active_leases"
        case maxQueuedLeases = "max_queued_leases"
        case brokerDisabled = "broker_disabled"
    }
}

/// Wire shape for `/health`, `/ready`, and `daemonStatus.json`.
struct DaemonStatus: Codable, Sendable, Equatable {
    let state: DaemonLifecycleState
    let schemaVersion: Int
    let binarySchemaVersion: Int
    let buildSha: String
    let startedAt: Date?
    let lastStateChangeAt: Date
    let degraded: [DegradedReason]
    let failure: FailureReason?
    let restartCountSinceBoot: Int
    let pid: Int
    let xcodeBrokerHealth: XcodeBrokerHealthSnapshot?

    enum CodingKeys: String, CodingKey {
        case state
        case schemaVersion = "schema_version"
        case binarySchemaVersion = "binary_schema_version"
        case buildSha = "build_sha"
        case startedAt = "started_at"
        case lastStateChangeAt = "last_state_change_at"
        case degraded
        case failure
        case restartCountSinceBoot = "restart_count_since_boot"
        case pid
        case xcodeBrokerHealth = "xcode_broker_health"
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        state = try c.decode(DaemonLifecycleState.self, forKey: .state)
        schemaVersion = try c.decode(Int.self, forKey: .schemaVersion)
        binarySchemaVersion = try c.decode(Int.self, forKey: .binarySchemaVersion)
        buildSha = try c.decode(String.self, forKey: .buildSha)
        startedAt = try c.decodeIfPresent(Date.self, forKey: .startedAt)
        lastStateChangeAt = try c.decode(Date.self, forKey: .lastStateChangeAt)
        degraded = (try? c.decode([DegradedReason].self, forKey: .degraded)) ?? []
        failure = try c.decodeIfPresent(FailureReason.self, forKey: .failure)
        restartCountSinceBoot = try c.decode(Int.self, forKey: .restartCountSinceBoot)
        pid = try c.decode(Int.self, forKey: .pid)
        xcodeBrokerHealth = try c.decodeIfPresent(
            XcodeBrokerHealthSnapshot.self,
            forKey: .xcodeBrokerHealth
        )
    }

    init(
        state: DaemonLifecycleState,
        schemaVersion: Int,
        binarySchemaVersion: Int,
        buildSha: String,
        startedAt: Date?,
        lastStateChangeAt: Date,
        degraded: [DegradedReason],
        failure: FailureReason?,
        restartCountSinceBoot: Int,
        pid: Int,
        xcodeBrokerHealth: XcodeBrokerHealthSnapshot? = nil
    ) {
        self.state = state
        self.schemaVersion = schemaVersion
        self.binarySchemaVersion = binarySchemaVersion
        self.buildSha = buildSha
        self.startedAt = startedAt
        self.lastStateChangeAt = lastStateChangeAt
        self.degraded = degraded
        self.failure = failure
        self.restartCountSinceBoot = restartCountSinceBoot
        self.pid = pid
        self.xcodeBrokerHealth = xcodeBrokerHealth
    }
}

func daemonJSONDecoder() -> JSONDecoder {
    let decoder = JSONDecoder()
    let formatter = ISO8601DateFormatter()
    formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    decoder.dateDecodingStrategy = .custom { d in
        let container = try d.singleValueContainer()
        let string = try container.decode(String.self)
        if let date = formatter.date(from: string) {
            return date
        }
        let plain = ISO8601DateFormatter()
        plain.formatOptions = [.withInternetDateTime]
        if let date = plain.date(from: string) {
            return date
        }
        throw DecodingError.dataCorruptedError(
            in: container,
            debugDescription: "expected ISO-8601 date; got \(string)"
        )
    }
    return decoder
}

// MARK: - Port file (§7.3)

enum DaemonPortFileError: Error, Equatable, CustomStringConvertible {
    case empty(url: URL)
    case nonUtf8(url: URL)
    case invalid(url: URL, text: String)

    var description: String {
        switch self {
        case .empty(let url):
            return "daemon.port is empty at \(url.path)"
        case .nonUtf8(let url):
            return "daemon.port is not valid UTF-8 at \(url.path)"
        case .invalid(let url, let text):
            return "daemon.port contents are not a valid port number (\(text)) at \(url.path)"
        }
    }
}

struct DaemonPortFile {
    static let defaultPort: Int = 4000

    static func defaultURL() -> URL {
        appSupportDirectory().appendingPathComponent("daemon.port", isDirectory: false)
    }

    static func appSupportDirectory() -> URL {
        let home = FileManager.default.homeDirectoryForCurrentUser
        return home
            .appendingPathComponent("Library", isDirectory: true)
            .appendingPathComponent("Application Support", isDirectory: true)
            .appendingPathComponent("Chainworks Forge", isDirectory: true)
    }

    /// Read the port from `daemon.port`. Returns `defaultPort` when the
    /// file is absent; throws for parse errors so the caller can surface
    /// a diagnostic (§5.3 — the client must not silently fall back on a
    /// corrupt file).
    static func read(at url: URL = defaultURL()) throws -> Int {
        guard let data = try? Data(contentsOf: url) else {
            return defaultPort
        }
        guard let text = String(data: data, encoding: .utf8)?
            .trimmingCharacters(in: .whitespacesAndNewlines)
        else {
            throw DaemonPortFileError.nonUtf8(url: url)
        }
        guard !text.isEmpty else {
            throw DaemonPortFileError.empty(url: url)
        }
        guard let port = Int(text), (1...65_535).contains(port) else {
            throw DaemonPortFileError.invalid(url: url, text: text)
        }
        return port
    }

    static func baseURL(at url: URL = defaultURL()) throws -> URL {
        let port = try read(at: url)
        return URL(string: "http://127.0.0.1:\(port)")!
    }
}

// MARK: - HTTP client

enum DaemonClientError: Error, CustomStringConvertible {
    case httpFailure(status: Int, body: String)
    case graphqlErrors([String])
    case decoding(Error)
    case malformedURL(String)
    case transport(Error)

    var description: String {
        switch self {
        case .httpFailure(let status, let body):
            return "daemon HTTP \(status): \(body)"
        case .graphqlErrors(let errors):
            return "daemon GraphQL errors: \(errors.joined(separator: "; "))"
        case .decoding(let error):
            return "daemon response decode: \(error)"
        case .malformedURL(let s):
            return "malformed daemon URL: \(s)"
        case .transport(let error):
            return "daemon transport: \(error)"
        }
    }
}

struct DaemonClientEndpoint: Sendable, Equatable {
    var baseURL: URL
    var bearerToken: String

    var graphqlURL: URL {
        baseURL.appendingPathComponent("graphql", isDirectory: false)
    }

    var graphqlWSURL: URL {
        var components = URLComponents(url: baseURL, resolvingAgainstBaseURL: false)!
        components.scheme = baseURL.scheme == "https" ? "wss" : "ws"
        components.path = "/graphql/ws"
        return components.url!
    }

    static func operatorDefault() -> DaemonClientEndpoint {
        let bearer = DaemonOperatorTokenStore.resolveOperatorToken() ?? "unset"
        let base: URL
        do {
            base = try DaemonPortFile.baseURL()
        } catch {
            base = URL(string: "http://127.0.0.1:\(DaemonPortFile.defaultPort)")!
        }
        return DaemonClientEndpoint(baseURL: base, bearerToken: bearer)
    }
}

struct DaemonLifecycleClient {
    var endpoint: DaemonClientEndpoint
    var urlSession: URLSession

    init(
        endpoint: DaemonClientEndpoint,
        urlSession: URLSession = .shared
    ) {
        self.endpoint = endpoint
        self.urlSession = urlSession
    }

    func snapshot() async throws -> DaemonStatus {
        let body: [String: Any] = ["query": Self.snapshotQuery]
        let data = try JSONSerialization.data(withJSONObject: body)
        var request = URLRequest(url: endpoint.graphqlURL)
        request.httpMethod = "POST"
        request.httpBody = data
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("Bearer \(endpoint.bearerToken)", forHTTPHeaderField: "Authorization")

        let (respData, response): (Data, URLResponse)
        do {
            (respData, response) = try await urlSession.data(for: request)
        } catch {
            throw DaemonClientError.transport(error)
        }
        guard let http = response as? HTTPURLResponse else {
            throw DaemonClientError.httpFailure(status: -1, body: "no HTTP response")
        }
        guard (200..<300).contains(http.statusCode) else {
            let body = String(data: respData, encoding: .utf8) ?? "<binary>"
            throw DaemonClientError.httpFailure(status: http.statusCode, body: body)
        }
        return try Self.decodeSnapshot(respData)
    }

    func schedulerReadback() async throws -> SchedulerHealthReadback {
        let body: [String: Any] = ["query": Self.schedulerReadbackQuery]
        let data = try JSONSerialization.data(withJSONObject: body)
        var request = URLRequest(url: endpoint.graphqlURL)
        request.httpMethod = "POST"
        request.httpBody = data
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("Bearer \(endpoint.bearerToken)", forHTTPHeaderField: "Authorization")

        let (respData, response): (Data, URLResponse)
        do {
            (respData, response) = try await urlSession.data(for: request)
        } catch {
            throw DaemonClientError.transport(error)
        }
        guard let http = response as? HTTPURLResponse else {
            throw DaemonClientError.httpFailure(status: -1, body: "no HTTP response")
        }
        guard (200..<300).contains(http.statusCode) else {
            let body = String(data: respData, encoding: .utf8) ?? "<binary>"
            throw DaemonClientError.httpFailure(status: http.statusCode, body: body)
        }
        return try Self.decodeSchedulerReadback(respData)
    }

    static func decodeSnapshot(_ data: Data) throws -> DaemonStatus {
        let envelope: SnapshotEnvelope
        do {
            envelope = try JSONDecoder().decode(SnapshotEnvelope.self, from: data)
        } catch {
            throw DaemonClientError.decoding(error)
        }
        if let errors = envelope.errors, !errors.isEmpty {
            throw DaemonClientError.graphqlErrors(errors.map { $0.message })
        }
        guard let snapshot = envelope.data?.daemonStatus else {
            throw DaemonClientError.decoding(
                NSError(domain: "DaemonClient", code: -1, userInfo: [
                    NSLocalizedDescriptionKey: "data.daemonStatus missing"
                ])
            )
        }
        guard let jsonData = snapshot.json.data(using: .utf8) else {
            throw DaemonClientError.decoding(
                NSError(domain: "DaemonClient", code: -1, userInfo: [
                    NSLocalizedDescriptionKey: "snapshot.json not UTF-8"
                ])
            )
        }
        do {
            return try daemonJSONDecoder().decode(DaemonStatus.self, from: jsonData)
        } catch {
            throw DaemonClientError.decoding(error)
        }
    }

    static let snapshotQuery: String = "{ daemonStatus { json } }"

    static let schedulerReadbackQuery: String = """
    {
      schedulerHealthSummary {
        queuedCount
        oldestQueuedAgeMs
        globalQueueDepth
        activeAgentExecutions
        dbWriterWaitP95Ms
        commandLatencyP95MsJson
        lastHostInterruptionEpochId
        sustainedBackpressureState
        staleAfterMs
        updatedAt
        isStale
      }
      activeExecutionCountsByProvider {
        providerFamily
        activeCount
      }
      queuedBackpressuredCountsByProviderAndReason {
        providerFamily
        topReason
        queuedCount
        oldestQueuedAgeMs
        globalQueueDepth
        staleAfterMs
        updatedAt
        isStale
      }
    }
    """

    static func decodeSchedulerReadback(_ data: Data) throws -> SchedulerHealthReadback {
        let envelope: SchedulerReadbackEnvelope
        do {
            envelope = try JSONDecoder().decode(SchedulerReadbackEnvelope.self, from: data)
        } catch {
            throw DaemonClientError.decoding(error)
        }
        if let errors = envelope.errors, !errors.isEmpty {
            throw DaemonClientError.graphqlErrors(errors.map { $0.message })
        }
        guard let data = envelope.data else {
            throw DaemonClientError.decoding(
                NSError(domain: "DaemonClient", code: -1, userInfo: [
                    NSLocalizedDescriptionKey: "scheduler readback data missing"
                ])
            )
        }
        return SchedulerHealthReadback(
            health: data.schedulerHealthSummary,
            activeProviders: data.activeExecutionCountsByProvider,
            queueSummaries: data.queuedBackpressuredCountsByProviderAndReason
        )
    }

    // MARK: - Subscription (daemonStatusChanged)

    /// Open the GraphQL-over-WebSocket connection, authenticate via
    /// `connection_init`, and subscribe to `daemonStatusChanged`.
    /// Each incoming frame resolves as a new `DaemonStatus` on the
    /// returned `AsyncStream`. The stream terminates when the caller
    /// drops the returned subscription, when the server closes, or on
    /// transport error.
    ///
    /// The snapshot-plus-subscribe contract (§5.2 + P056 §5.5) is the
    /// caller's responsibility: call `snapshot()` once to seed, then
    /// call `subscribe()` and feed both into the view model.
    func subscribe() -> DaemonSubscription {
        let ws = urlSession.webSocketTask(with: Self.subscribeRequest(for: endpoint))
        return DaemonSubscription(socket: ws, bearerToken: endpoint.bearerToken)
    }

    /// GraphQL-WS (`graphql-transport-ws`) is what async-graphql's
    /// `GraphQLWebSocket` speaks; the URL scheme and sub-protocol are
    /// pinned here so the same client works in tests and production.
    static func subscribeRequest(for endpoint: DaemonClientEndpoint) -> URLRequest {
        var request = URLRequest(url: endpoint.graphqlWSURL)
        // The server accepts both `graphql-transport-ws` (spec-current)
        // and `graphql-ws` (legacy Apollo); we advertise the current
        // one.
        request.setValue(
            "graphql-transport-ws",
            forHTTPHeaderField: "Sec-WebSocket-Protocol"
        )
        return request
    }
}

/// Live-feed subscription owned by the caller. Drop it to close the
/// socket. Frames arrive on `stream`; errors terminate the stream after
/// at most one sentinel event has been delivered (so the UI can render
/// "subscription lost, fetching snapshot").
final class DaemonSubscription {
    /// Async stream of decoded frames. `nil` elements indicate a
    /// recoverable blip (e.g. broadcast lag) — the UI should call
    /// `DaemonLifecycleClient.snapshot()` to re-seed state.
    let stream: AsyncStream<Result<DaemonStatus, DaemonClientError>>

    private let socket: URLSessionWebSocketTask
    private let continuation: AsyncStream<Result<DaemonStatus, DaemonClientError>>.Continuation
    private let readerTask: Task<Void, Never>

    init(socket: URLSessionWebSocketTask, bearerToken: String) {
        self.socket = socket
        var cont: AsyncStream<Result<DaemonStatus, DaemonClientError>>.Continuation!
        self.stream = AsyncStream { c in cont = c }
        self.continuation = cont
        let task = Task {
            await Self.drive(
                socket: socket,
                bearerToken: bearerToken,
                continuation: cont
            )
        }
        self.readerTask = task
    }

    deinit {
        readerTask.cancel()
        socket.cancel(with: .goingAway, reason: nil)
    }

    /// graphql-transport-ws client loop. Spec:
    ///
    ///   1. → `connection_init` with `Authorization: Bearer …`.
    ///   2. ← `connection_ack`.
    ///   3. → `subscribe` with the operation + id.
    ///   4. ← `next` frames until `complete` or `error`.
    private static func drive(
        socket: URLSessionWebSocketTask,
        bearerToken: String,
        continuation: AsyncStream<Result<DaemonStatus, DaemonClientError>>.Continuation
    ) async {
        socket.resume()
        let initMsg: [String: Any] = [
            "type": "connection_init",
            "payload": [
                "Authorization": "Bearer \(bearerToken)",
            ],
        ]
        do {
            try await socket.send(.string(Self.encodeJSON(initMsg)))
        } catch {
            continuation.yield(.failure(.transport(error)))
            continuation.finish()
            return
        }

        while !Task.isCancelled {
            let msg: URLSessionWebSocketTask.Message
            do {
                msg = try await socket.receive()
            } catch {
                continuation.yield(.failure(.transport(error)))
                continuation.finish()
                return
            }
            guard case .string(let text) = msg,
                  let data = text.data(using: .utf8),
                  let obj = try? JSONSerialization.jsonObject(with: data)
                    as? [String: Any],
                  let type = obj["type"] as? String
            else {
                continue
            }

            switch type {
            case "connection_ack":
                // Now send the actual subscribe frame.
                let subscribe: [String: Any] = [
                    "id": "daemon-status-1",
                    "type": "subscribe",
                    "payload": [
                        "query": "subscription { daemonStatusChanged { json } }",
                    ],
                ]
                do {
                    try await socket.send(.string(Self.encodeJSON(subscribe)))
                } catch {
                    continuation.yield(.failure(.transport(error)))
                    continuation.finish()
                    return
                }
            case "next":
                guard let payload = obj["payload"] as? [String: Any],
                      let data = payload["data"] as? [String: Any],
                      let status = data["daemonStatusChanged"] as? [String: Any],
                      let json = status["json"] as? String,
                      let jsonData = json.data(using: .utf8)
                else {
                    continue
                }
                do {
                    let decoded = try daemonJSONDecoder().decode(
                        DaemonStatus.self,
                        from: jsonData
                    )
                    continuation.yield(.success(decoded))
                } catch {
                    continuation.yield(.failure(.decoding(error)))
                }
            case "complete":
                continuation.finish()
                return
            case "error":
                let messages = (obj["payload"] as? [[String: Any]])?
                    .compactMap { $0["message"] as? String } ?? []
                continuation.yield(.failure(.graphqlErrors(messages)))
                continuation.finish()
                return
            case "ping":
                // Reply with pong to keep the socket alive. Non-fatal on failure.
                _ = try? await socket.send(.string(Self.encodeJSON(["type": "pong"])))
            default:
                continue
            }
        }
    }

    private static func encodeJSON(_ value: [String: Any]) -> String {
        let data = (try? JSONSerialization.data(withJSONObject: value)) ?? Data()
        return String(data: data, encoding: .utf8) ?? "{}"
    }
}

private struct SnapshotEnvelope: Decodable {
    let data: SnapshotData?
    let errors: [GraphQLError]?
}

private struct SnapshotData: Decodable {
    let daemonStatus: SnapshotPayload
}

private struct SnapshotPayload: Decodable {
    let json: String
}

private struct GraphQLError: Decodable {
    let message: String
}

struct SchedulerHealthReadback: Sendable, Equatable {
    let health: SchedulerHealthSummaryPayload?
    let activeProviders: [SchedulerProviderActiveCountPayload]
    let queueSummaries: [SchedulerQueueSummaryPayload]
}

struct SchedulerHealthSummaryPayload: Decodable, Sendable, Equatable {
    let queuedCount: Int
    let oldestQueuedAgeMs: Int
    let globalQueueDepth: Int
    let activeAgentExecutions: Int
    let dbWriterWaitP95Ms: Int?
    let commandLatencyP95MsJson: String?
    let lastHostInterruptionEpochId: String?
    let sustainedBackpressureState: String
    let staleAfterMs: Int
    let updatedAt: String
    let isStale: Bool
}

struct SchedulerProviderActiveCountPayload: Decodable, Sendable, Equatable, Identifiable {
    var id: String { providerFamily }
    let providerFamily: String
    let activeCount: Int
}

struct SchedulerQueueSummaryPayload: Decodable, Sendable, Equatable, Identifiable {
    var id: String { "\(providerFamily ?? "all")-\(topReason)" }
    let providerFamily: String?
    let topReason: String
    let queuedCount: Int
    let oldestQueuedAgeMs: Int
    let globalQueueDepth: Int
    let staleAfterMs: Int
    let updatedAt: String
    let isStale: Bool
}

struct SchedulerHealthBannerIssue: Sendable, Equatable {
    enum Kind: String, Sendable {
        case sustainedBackpressure
        case staleProjection
        case dbWriterPressure
    }

    let kind: Kind
    let title: String
    let detail: String
    let systemImage: String
}

private struct SchedulerReadbackEnvelope: Decodable {
    let data: SchedulerReadbackData?
    let errors: [GraphQLError]?
}

private struct SchedulerReadbackData: Decodable {
    let schedulerHealthSummary: SchedulerHealthSummaryPayload?
    let activeExecutionCountsByProvider: [SchedulerProviderActiveCountPayload]
    let queuedBackpressuredCountsByProviderAndReason: [SchedulerQueueSummaryPayload]
}

@MainActor
final class SchedulerHealthViewModel: ObservableObject {
    @Published private(set) var readback: SchedulerHealthReadback?
    @Published private(set) var lastError: DaemonClientError?

    private let client: DaemonLifecycleClient

    init(client: DaemonLifecycleClient) {
        self.client = client
    }

    func refresh() async {
        do {
            readback = try await client.schedulerReadback()
            lastError = nil
        } catch let error as DaemonClientError {
            lastError = error
        } catch {
            lastError = .transport(error)
        }
    }

    static func bootstrap() -> SchedulerHealthViewModel {
        SchedulerHealthViewModel(
            client: DaemonLifecycleClient(endpoint: .operatorDefault())
        )
    }

    var bannerIssue: SchedulerHealthBannerIssue? {
        SchedulerHealthPresentation.bannerIssue(for: readback)
    }
}

enum SchedulerHealthPresentation {
    static func bannerIssue(for readback: SchedulerHealthReadback?) -> SchedulerHealthBannerIssue? {
        guard let health = readback?.health else { return nil }
        if health.isStale {
            return SchedulerHealthBannerIssue(
                kind: .staleProjection,
                title: "Scheduler projection stale",
                detail: "Open Scheduler Health",
                systemImage: "clock.badge.exclamationmark"
            )
        }
        if isSustainedBackpressure(health) {
            return SchedulerHealthBannerIssue(
                kind: .sustainedBackpressure,
                title: "System Busy - queued agents",
                detail: "\(health.queuedCount) queued, oldest \(durationLabel(milliseconds: health.oldestQueuedAgeMs))",
                systemImage: "hourglass.circle"
            )
        }
        if let wait = health.dbWriterWaitP95Ms, wait > 0 {
            return SchedulerHealthBannerIssue(
                kind: .dbWriterPressure,
                title: "Database writer busy",
                detail: "p95 wait \(wait) ms",
                systemImage: "externaldrive.badge.exclamationmark"
            )
        }
        return nil
    }

    private static func isSustainedBackpressure(_ health: SchedulerHealthSummaryPayload) -> Bool {
        guard health.queuedCount > 0 else { return false }
        let state = health.sustainedBackpressureState
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()
        if !["", "none", "clear", "healthy", "idle"].contains(state) {
            return true
        }
        return health.oldestQueuedAgeMs >= 5 * 60 * 1000
    }

    static func durationLabel(milliseconds: Int) -> String {
        if milliseconds <= 0 {
            return "0s"
        }
        let seconds = milliseconds / 1000
        if seconds < 60 {
            return "\(seconds)s"
        }
        let minutes = seconds / 60
        let remainder = seconds % 60
        return remainder == 0 ? "\(minutes)m" : "\(minutes)m \(remainder)s"
    }

    static func reasonLabel(_ reason: String) -> String {
        switch reason {
        case "run_capacity": return "Run at agent limit"
        case "provider_capacity": return "Waiting for provider slot"
        case "global_capacity": return "System agent limit reached"
        case "startup_recovery_backpressure": return "Recovering queued work"
        case "db_writer_capacity": return "Database writer busy"
        default: return reason.replacingOccurrences(of: "_", with: " ").capitalized
        }
    }

    static func hostInterruptionLabel(_ kind: String) -> String {
        switch kind {
        case "system_sleep", "wall_clock_gap": return "Recovering from system sleep"
        case "network_migration": return "Resuming after network change"
        default: return "Recovering interrupted work"
        }
    }

    static func hostInterruptionSymbol(_ kind: String) -> String {
        switch kind {
        case "system_sleep", "wall_clock_gap": return "moon.zzz"
        case "network_migration": return "wifi.exclamationmark"
        default: return "arrow.clockwise.circle"
        }
    }
}

enum DaemonOperatorTokenStore {
    /// Minimal reader for `principals.json` — the app's principals file
    /// is a JSON list; the operator token is the first entry whose
    /// `class == "operator"`. Returns `nil` when the file is absent,
    /// malformed, or contains no operator.
    static func resolveOperatorToken() -> String? {
        let home = FileManager.default.homeDirectoryForCurrentUser
        let path = home
            .appendingPathComponent(".chainworks", isDirectory: true)
            .appendingPathComponent("auth", isDirectory: true)
            .appendingPathComponent("principals.json", isDirectory: false)
        guard let data = try? Data(contentsOf: path),
              let root = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
              let list = root["principals"] as? [[String: Any]]
        else {
            return nil
        }
        for entry in list {
            if let cls = entry["class"] as? String,
               cls == "operator",
               let token = entry["token"] as? String,
               !token.isEmpty
            {
                return token
            }
        }
        return nil
    }
}

// MARK: - View model

@MainActor
final class DaemonStatusViewModel: ObservableObject {
    @Published private(set) var status: DaemonStatus?
    @Published private(set) var lastError: DaemonClientError?

    /// True iff no successful snapshot has been observed. The UI uses
    /// this to render the Unavailable panel (restart, Export Diagnostics)
    /// instead of a lifecycle banner. We deliberately do NOT synthesize
    /// a lifecycle state from transport errors (§5.3).
    var isUnavailable: Bool { status == nil }

    private let client: DaemonLifecycleClient
    private var subscriptionTask: Task<Void, Never>?

    init(client: DaemonLifecycleClient) {
        self.client = client
    }

    deinit {
        subscriptionTask?.cancel()
    }

    /// Snapshot fetch. Safe to call repeatedly.
    func refresh() async {
        do {
            self.status = try await client.snapshot()
            self.lastError = nil
        } catch let error as DaemonClientError {
            self.lastError = error
        } catch {
            self.lastError = .transport(error)
        }
    }

    /// Snapshot-plus-subscribe bootstrap (P042 §5.2 / P056 §5.5).
    /// Issues a single `refresh()` to seed state, then opens a
    /// `daemonStatusChanged` subscription so live transitions update
    /// `status` without polling. Safe to call multiple times — any
    /// existing subscription task is cancelled first.
    func startSnapshotPlusSubscribe() async {
        subscriptionTask?.cancel()
        await refresh()
        let subscription = client.subscribe()
        subscriptionTask = Task { [weak self] in
            for await frame in subscription.stream {
                guard !Task.isCancelled, let self else { break }
                switch frame {
                case .success(let status):
                    await MainActor.run { self.apply(frame: status) }
                case .failure(let error):
                    // P042 §5.3: subscription drops do NOT synthesize a
                    // state. We park `lastError` so the UI can surface a
                    // "live feed lost — re-snapshotting" hint but keep
                    // the previous known-good snapshot rendered.
                    await MainActor.run { self.lastError = error }
                    // Pull a fresh snapshot so we don't stay out of sync
                    // while the caller decides whether to reconnect.
                    await self.refresh()
                    return
                }
            }
        }
    }

    /// Test-only helper; also used by SwiftUI previews.
    func apply(frame: DaemonStatus) {
        self.status = frame
        self.lastError = nil
    }

    /// `true` when the banner is worth rendering. `Ready` without a
    /// prior error → quiet; any other state (Degraded, Failed,
    /// Starting, Unavailable) → banner. This keeps the operator UI
    /// distraction-free on a healthy daemon while making failures and
    /// transitions visible without a click.
    var shouldDisplayBanner: Bool {
        if let status {
            return status.state != .ready
        }
        return true
    }

    /// Default factory: resolve the daemon's port from
    /// `~/Library/Application Support/Chainworks Forge/daemon.port`,
    /// pair it with a bearer token from the app's principals file, and
    /// return a view model ready for `.task { await startSnapshotPlusSubscribe() }`.
    ///
    /// The bearer token comes from `$HOME/.chainworks/auth/principals.json` —
    /// we look for an operator entry. If the file is absent or the
    /// port resolution throws, we fall back to a no-op client
    /// (Unavailable forever). The banner renders the Retry / Export
    /// affordances in that case.
    @MainActor
    static func bootstrap() -> DaemonStatusViewModel {
        let client = DaemonLifecycleClient(endpoint: .operatorDefault())
        return DaemonStatusViewModel(client: client)
    }
}
