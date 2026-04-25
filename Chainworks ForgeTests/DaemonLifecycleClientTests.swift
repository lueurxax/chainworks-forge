// P042 §5.3 Swift lifecycle client tests.
//
// Focused on decoder fidelity + no-inference contract. Live HTTP round
// trips are covered by the Rust `request_id_propagation` integration
// tests (which drive the daemon router directly). Here we isolate the
// Swift decoding and view-model behaviour.

import XCTest
@testable import Chainworks_Forge

final class DaemonLifecycleClientTests: XCTestCase {

    // MARK: - Wire decoding

    func test_DaemonStatus_decodes_ready_snapshot_from_health_json() throws {
        let json = """
        {
          "state": "ready",
          "schema_version": 14,
          "binary_schema_version": 14,
          "build_sha": "abc123",
          "started_at": "2026-04-18T10:00:00Z",
          "last_state_change_at": "2026-04-18T10:00:00Z",
          "restart_count_since_boot": 0,
          "pid": 42831
        }
        """
        let status = try daemonJSONDecoder().decode(
            DaemonStatus.self,
            from: Data(json.utf8)
        )
        XCTAssertEqual(status.state, .ready)
        XCTAssertEqual(status.schemaVersion, 14)
        XCTAssertEqual(status.buildSha, "abc123")
        XCTAssertTrue(status.degraded.isEmpty)
        XCTAssertNil(status.failure)
    }

    func test_DaemonStatus_decodes_xcode_broker_health_snapshot_when_reported() throws {
        let json = """
        {
          "state": "ready",
          "schema_version": 14,
          "binary_schema_version": 14,
          "build_sha": "abc123",
          "started_at": "2026-04-18T10:00:00Z",
          "last_state_change_at": "2026-04-18T10:00:00Z",
          "restart_count_since_boot": 0,
          "pid": 42831,
          "xcode_broker_health": {
            "state": "degraded",
            "pool_id": "host-user-xcode",
            "active_leases": 8,
            "queued_leases": 2,
            "max_active_leases": 8,
            "max_queued_leases": 16,
            "broker_disabled": false,
            "backend_available": true,
            "observation_persistence_failures": 3
          }
        }
        """
        let status = try daemonJSONDecoder().decode(
            DaemonStatus.self,
            from: Data(json.utf8)
        )
        XCTAssertEqual(status.xcodeBrokerHealth?.state, .degraded)
        XCTAssertEqual(status.xcodeBrokerHealth?.poolID, "host-user-xcode")
        XCTAssertEqual(status.xcodeBrokerHealth?.activeLeases, 8)
        XCTAssertEqual(status.xcodeBrokerHealth?.queuedLeases, 2)
        XCTAssertEqual(status.xcodeBrokerHealth?.maxActiveLeases, 8)
        XCTAssertEqual(status.xcodeBrokerHealth?.maxQueuedLeases, 16)
        XCTAssertEqual(status.xcodeBrokerHealth?.brokerDisabled, false)
        XCTAssertEqual(status.xcodeBrokerHealth?.backendAvailable, true)
        XCTAssertEqual(status.xcodeBrokerHealth?.observationPersistenceFailures, 3)
    }

    func test_DaemonStatus_decodes_degraded_reasons_array() throws {
        let json = """
        {
          "state": "degraded",
          "schema_version": 14,
          "binary_schema_version": 14,
          "build_sha": "abc",
          "started_at": "2026-04-18T10:00:00Z",
          "last_state_change_at": "2026-04-18T10:02:11Z",
          "degraded": [{
            "kind": "stale_projection",
            "detail": "projection lag >5s",
            "since": "2026-04-18T10:02:11Z"
          }],
          "restart_count_since_boot": 0,
          "pid": 42831
        }
        """
        let status = try daemonJSONDecoder().decode(
            DaemonStatus.self,
            from: Data(json.utf8)
        )
        XCTAssertEqual(status.state, .degraded)
        XCTAssertEqual(status.degraded.count, 1)
        XCTAssertEqual(status.degraded.first?.kind, .staleProjection)
    }

    func test_DaemonStatus_decodes_failed_snapshot_with_backup_path() throws {
        let json = """
        {
          "state": "failed",
          "schema_version": 13,
          "binary_schema_version": 14,
          "build_sha": "abc",
          "started_at": null,
          "last_state_change_at": "2026-04-18T10:05:00Z",
          "failure": {
            "kind": "migration_failed",
            "detail": "CREATE TABLE foo: unique constraint violation",
            "since": "2026-04-18T10:05:00Z",
            "backup_path": "/Users/op/Library/Application Support/Chainworks Forge/control-plane.db.backup-…sqlite"
          },
          "restart_count_since_boot": 0,
          "pid": 42831
        }
        """
        let status = try daemonJSONDecoder().decode(
            DaemonStatus.self,
            from: Data(json.utf8)
        )
        XCTAssertEqual(status.state, .failed)
        XCTAssertEqual(status.failure?.kind, .migrationFailed)
        XCTAssertNotNil(status.failure?.backupPath)
    }

    // MARK: - GraphQL envelope decoding

    func test_decodeSnapshot_unwraps_graphql_json_field_into_DaemonStatus() throws {
        let inner = """
        {\"state\":\"ready\",\"schema_version\":14,\"binary_schema_version\":14,\"build_sha\":\"x\",\"last_state_change_at\":\"2026-04-18T10:00:00Z\",\"restart_count_since_boot\":0,\"pid\":1}
        """
        let envelope = [
            "data": [
                "daemonStatus": [
                    "json": inner
                ]
            ]
        ]
        let data = try JSONSerialization.data(withJSONObject: envelope)
        let decoded = try DaemonLifecycleClient.decodeSnapshot(data)
        XCTAssertEqual(decoded.state, .ready)
        XCTAssertEqual(decoded.binarySchemaVersion, 14)
    }

    func test_decodeSnapshot_reports_graphql_errors_as_typed_envelope() {
        let envelope: [String: Any] = [
            "errors": [["message": "forbidden"]]
        ]
        let data = try! JSONSerialization.data(withJSONObject: envelope)
        XCTAssertThrowsError(try DaemonLifecycleClient.decodeSnapshot(data)) { err in
            guard case DaemonClientError.graphqlErrors(let messages) = err else {
                return XCTFail("expected .graphqlErrors, got \(err)")
            }
            XCTAssertEqual(messages, ["forbidden"])
        }
    }

    // MARK: - Port file

    func test_DaemonPortFile_reads_fallback_when_file_absent() throws {
        let tmp = try XCTUnwrap(
            try? URL(fileURLWithPath: NSTemporaryDirectory())
                .appendingPathComponent(UUID().uuidString, isDirectory: true)
        )
        try FileManager.default.createDirectory(at: tmp, withIntermediateDirectories: true)
        let port = try DaemonPortFile.read(at: tmp.appendingPathComponent("absent.port"))
        XCTAssertEqual(port, DaemonPortFile.defaultPort)
    }

    func test_DaemonPortFile_reads_numeric_content() throws {
        let tmp = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: tmp, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tmp) }
        let file = tmp.appendingPathComponent("daemon.port")
        try Data("58743\n".utf8).write(to: file)
        XCTAssertEqual(try DaemonPortFile.read(at: file), 58743)
    }

    func test_DaemonPortFile_rejects_empty_file() throws {
        let tmp = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: tmp, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tmp) }
        let file = tmp.appendingPathComponent("daemon.port")
        try Data().write(to: file)
        XCTAssertThrowsError(try DaemonPortFile.read(at: file)) { err in
            guard case DaemonPortFileError.empty = err else {
                return XCTFail("expected .empty, got \(err)")
            }
        }
    }

    func test_DaemonPortFile_rejects_non_numeric_content() throws {
        let tmp = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: tmp, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tmp) }
        let file = tmp.appendingPathComponent("daemon.port")
        try Data("not-a-port".utf8).write(to: file)
        XCTAssertThrowsError(try DaemonPortFile.read(at: file)) { err in
            guard case DaemonPortFileError.invalid = err else {
                return XCTFail("expected .invalid, got \(err)")
            }
        }
    }

    func test_DaemonPortFile_baseURL_builds_loopback_http() throws {
        let tmp = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: tmp, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tmp) }
        let file = tmp.appendingPathComponent("daemon.port")
        try Data("4321".utf8).write(to: file)
        let url = try DaemonPortFile.baseURL(at: file)
        XCTAssertEqual(url.absoluteString, "http://127.0.0.1:4321")
    }

    // MARK: - Lifecycle state predicates

    func test_DaemonLifecycleState_is_live_matches_rust_contract() {
        XCTAssertTrue(DaemonLifecycleState.ready.isLive)
        XCTAssertTrue(DaemonLifecycleState.degraded.isLive)
        XCTAssertFalse(DaemonLifecycleState.starting.isLive)
        XCTAssertFalse(DaemonLifecycleState.failed.isLive)
    }

    func test_DaemonLifecycleState_is_terminal_matches_rust_contract() {
        XCTAssertTrue(DaemonLifecycleState.failed.isTerminal)
        XCTAssertTrue(DaemonLifecycleState.shutdown.isTerminal)
        XCTAssertFalse(DaemonLifecycleState.ready.isTerminal)
        XCTAssertFalse(DaemonLifecycleState.degraded.isTerminal)
    }

    // MARK: - No-inference contract

    @MainActor
    func test_client_never_infers_state_outside_contract() async {
        // Build a view model with a client pointing at a port that
        // refuses connections. `refresh()` should surface an error —
        // NOT synthesize a DaemonLifecycleState. `isUnavailable` stays
        // true; `status` stays nil. This test pins the §5.3 contract.
        let endpoint = DaemonClientEndpoint(
            baseURL: URL(string: "http://127.0.0.1:1")!, // reserved, refuses
            bearerToken: "test"
        )
        let config = URLSessionConfiguration.ephemeral
        config.timeoutIntervalForRequest = 0.5
        config.timeoutIntervalForResource = 0.5
        let client = DaemonLifecycleClient(
            endpoint: endpoint,
            urlSession: URLSession(configuration: config)
        )
        let vm = DaemonStatusViewModel(client: client)
        await vm.refresh()
        XCTAssertNil(vm.status, "no synthetic status on transport failure")
        XCTAssertTrue(vm.isUnavailable, "UI must render Unavailable, not Ready/Failed")
        XCTAssertNotNil(vm.lastError, "transport error is surfaced as-is")
    }

    // MARK: - WS URL derivation

    func test_endpoint_graphqlWSURL_rewrites_scheme_to_ws() {
        let endpoint = DaemonClientEndpoint(
            baseURL: URL(string: "http://127.0.0.1:4000")!,
            bearerToken: "x"
        )
        XCTAssertEqual(endpoint.graphqlWSURL.absoluteString, "ws://127.0.0.1:4000/graphql/ws")
    }
}
