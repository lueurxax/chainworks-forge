// P042 §5.3 Swift lifecycle client tests.
//
// Focused on decoder fidelity + no-inference contract. Live HTTP round
// trips are covered by the Rust `request_id_propagation` integration
// tests (which drive the daemon router directly). Here we isolate the
// Swift decoding and view-model behaviour.

import Foundation
import Testing
@testable import Chainworks_Forge

@MainActor
struct DaemonLifecycleClientTests {

    // MARK: - Wire decoding

    @Test func `DaemonStatus decodes ready snapshot from health json`() throws {
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
        #expect(status.state == .ready)
        #expect(status.schemaVersion == 14)
        #expect(status.buildSha == "abc123")
        #expect(status.degraded.isEmpty)
        #expect(status.failure == nil)
    }

    @Test func `DaemonStatus decodes xcode broker health snapshot when reported`() throws {
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
            "observation_persistence_failures": 3,
            "stale_lease_count": 1,
            "backend_session_count": 2,
            "helper_cleanup_reaped_leases_total": 4,
            "reason_code": "xcode_observation_persist_failed",
            "can_acquire_new_xcode_leases": false,
            "active_lease_count": 8,
            "initialize_queue_depth": 2,
            "last_transition_at": "2026-04-18T10:01:00Z",
            "operator_message": "Xcode broker degraded: observation persistence failures"
          }
        }
        """
        let status = try daemonJSONDecoder().decode(
            DaemonStatus.self,
            from: Data(json.utf8)
        )
        #expect(status.xcodeBrokerHealth?.state == .degraded)
        #expect(status.xcodeBrokerHealth?.poolID == "host-user-xcode")
        #expect(status.xcodeBrokerHealth?.activeLeases == 8)
        #expect(status.xcodeBrokerHealth?.queuedLeases == 2)
        #expect(status.xcodeBrokerHealth?.maxActiveLeases == 8)
        #expect(status.xcodeBrokerHealth?.maxQueuedLeases == 16)
        #expect(status.xcodeBrokerHealth?.brokerDisabled == false)
        #expect(status.xcodeBrokerHealth?.backendAvailable == true)
        #expect(status.xcodeBrokerHealth?.observationPersistenceFailures == 3)
        #expect(status.xcodeBrokerHealth?.staleLeaseCount == 1)
        #expect(status.xcodeBrokerHealth?.backendSessionCount == 2)
        #expect(status.xcodeBrokerHealth?.helperCleanupReapedLeasesTotal == 4)
        #expect(status.xcodeBrokerHealth?.reasonCode == "xcode_observation_persist_failed")
        #expect(status.xcodeBrokerHealth?.canAcquireNewXcodeLeases == false)
        #expect(status.xcodeBrokerHealth?.activeLeaseCount == 8)
        #expect(status.xcodeBrokerHealth?.initializeQueueDepth == 2)
        #expect(status.xcodeBrokerHealth?.lastTransitionAt == "2026-04-18T10:01:00Z")
        #expect(
            status.xcodeBrokerHealth?.operatorMessage
                == "Xcode broker degraded: observation persistence failures"
        )
    }

    @Test func `DaemonStatus decodes degraded reasons array`() throws {
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
        #expect(status.state == .degraded)
        #expect(status.degraded.count == 1)
        #expect(status.degraded.first?.kind == .staleProjection)
    }

    @Test func `DaemonStatus decodes failed snapshot with backup path`() throws {
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
        #expect(status.state == .failed)
        #expect(status.failure?.kind == .migrationFailed)
        #expect(status.failure?.backupPath != nil)
    }

    // MARK: - GraphQL envelope decoding

    @Test func `decodeSnapshot unwraps graphql json field into DaemonStatus`() throws {
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
        #expect(decoded.state == .ready)
        #expect(decoded.binarySchemaVersion == 14)
    }

    @Test func `decodeSnapshot reports graphql errors as typed envelope`() throws {
        let envelope: [String: Any] = [
            "errors": [["message": "forbidden"]]
        ]
        let data = try JSONSerialization.data(withJSONObject: envelope)
        let error = #expect(throws: (any Error).self) {
            try DaemonLifecycleClient.decodeSnapshot(data)
        }
        if case DaemonClientError.graphqlErrors(let messages)? = error {
            #expect(messages == ["forbidden"])
        } else {
            Issue.record("expected .graphqlErrors, got \(String(describing: error))")
        }
    }

    @Test func `decodeStorageDiagnosticsSnapshots extracts p075 storage payloads`() throws {
        let envelope: [String: Any] = [
            "data": [
                "storageHealth": [
                    "updatedAt": "2026-05-08T19:00:00Z",
                    "staleAfterMs": 5000,
                    "isStale": false,
                    "dbState": "HEALTHY",
                    "evidenceSpool": [
                        "enabled": true,
                        "filesWrittenTotal": 2,
                        "bytesWrittenTotal": 128,
                        "metadataRowsTotal": 2,
                        "orphanFiles": 0,
                        "orphanBytes": 0,
                        "recoveredFiles": 0,
                        "checksumMismatchFiles": 0,
                        "pendingDeleteFiles": 0
                    ]
                ]
            ]
        ]
        let data = try JSONSerialization.data(withJSONObject: envelope)
        let snapshots = try DaemonLifecycleClient.decodeStorageDiagnosticsSnapshots(data)
        let storage = try JSONSerialization.jsonObject(with: snapshots.storageHealthData)
            as? [String: Any]
        let evidence = try #require(snapshots.evidenceSpoolSummaryData)
        let summary = try JSONSerialization.jsonObject(with: evidence) as? [String: Any]

        #expect(storage?["dbState"] as? String == "HEALTHY")
        #expect(summary?["filesWrittenTotal"] as? Int == 2)
        #expect(summary?["bytesWrittenTotal"] as? Int == 128)
    }

    // MARK: - Port file

    @Test func `DaemonPortFile reads fallback when file absent`() throws {
        let tmp = try #require(
            try? URL(fileURLWithPath: NSTemporaryDirectory())
                .appendingPathComponent(UUID().uuidString, isDirectory: true)
        )
        try FileManager.default.createDirectory(at: tmp, withIntermediateDirectories: true)
        let port = try DaemonPortFile.read(at: tmp.appendingPathComponent("absent.port"))
        #expect(port == DaemonPortFile.defaultPort)
    }

    @Test func `DaemonPortFile reads numeric content`() throws {
        let tmp = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: tmp, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tmp) }
        let file = tmp.appendingPathComponent("daemon.port")
        try Data("58743\n".utf8).write(to: file)
        #expect(try DaemonPortFile.read(at: file) == 58743)
    }

    @Test func `DaemonPortFile rejects empty file`() throws {
        let tmp = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: tmp, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tmp) }
        let file = tmp.appendingPathComponent("daemon.port")
        try Data().write(to: file)
        let error = #expect(throws: (any Error).self) {
            try DaemonPortFile.read(at: file)
        }
        if case DaemonPortFileError.empty? = error {} else {
            Issue.record("expected .empty, got \(String(describing: error))")
        }
    }

    @Test func `DaemonPortFile rejects non numeric content`() throws {
        let tmp = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: tmp, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tmp) }
        let file = tmp.appendingPathComponent("daemon.port")
        try Data("not-a-port".utf8).write(to: file)
        let error = #expect(throws: (any Error).self) {
            try DaemonPortFile.read(at: file)
        }
        if case DaemonPortFileError.invalid? = error {} else {
            Issue.record("expected .invalid, got \(String(describing: error))")
        }
    }

    @Test func `DaemonPortFile baseURL builds loopback http`() throws {
        let tmp = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: tmp, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tmp) }
        let file = tmp.appendingPathComponent("daemon.port")
        try Data("4321".utf8).write(to: file)
        let url = try DaemonPortFile.baseURL(at: file)
        #expect(url.absoluteString == "http://127.0.0.1:4321")
    }

    @Test func `DaemonEndpointFile prefers live endpoint record over port file`() throws {
        let tmp = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: tmp, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tmp) }

        let endpointFile = tmp.appendingPathComponent("daemon-endpoint.json")
        let portFile = tmp.appendingPathComponent("daemon.port")
        try Data(#"{"pid":4242,"port":64446}"#.utf8).write(to: endpointFile)
        try Data("4000".utf8).write(to: portFile)

        let endpointURL = try #require(
            try DaemonEndpointFile.baseURL(
                at: endpointFile,
                isLiveDaemonPID: { $0 == 4242 }
            )
        )
        #expect(endpointURL.absoluteString == "http://127.0.0.1:64446")
    }

    @Test func `DaemonEndpointFile ignores stale record and falls back to port file`() throws {
        let tmp = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: tmp, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tmp) }

        let endpointFile = tmp.appendingPathComponent("daemon-endpoint.json")
        let portFile = tmp.appendingPathComponent("daemon.port")
        try Data(#"{"pid":4242,"port":64446}"#.utf8).write(to: endpointFile)
        try Data("4000".utf8).write(to: portFile)

        #expect(
            try DaemonEndpointFile.baseURL(
                at: endpointFile,
                isLiveDaemonPID: { _ in false }
            ) == nil
        )
        let portURL = try DaemonPortFile.baseURL(at: portFile)
        #expect(portURL.absoluteString == "http://127.0.0.1:4000")
    }

    // MARK: - Lifecycle state predicates

    @Test func `DaemonLifecycleState is live matches rust contract`() {
        #expect(DaemonLifecycleState.ready.isLive)
        #expect(DaemonLifecycleState.degraded.isLive)
        #expect(!DaemonLifecycleState.starting.isLive)
        #expect(!DaemonLifecycleState.failed.isLive)
    }

    @Test func `DaemonLifecycleState is terminal matches rust contract`() {
        #expect(DaemonLifecycleState.failed.isTerminal)
        #expect(DaemonLifecycleState.shutdown.isTerminal)
        #expect(!DaemonLifecycleState.ready.isTerminal)
        #expect(!DaemonLifecycleState.degraded.isTerminal)
    }

    // MARK: - No-inference contract

    @Test func `Client never infers state outside contract`() async {
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
        #expect(vm.status == nil, "no synthetic status on transport failure")
        #expect(vm.isUnavailable, "UI must render Unavailable, not Ready/Failed")
        #expect(vm.lastError != nil, "transport error is surfaced as-is")
    }

    @Test func `refresh clears stale ready snapshot on transport failure`() async {
        let ready = DaemonStatus(
            state: .ready,
            schemaVersion: 1,
            binarySchemaVersion: 1,
            buildSha: "test-build",
            startedAt: nil,
            lastStateChangeAt: Date(),
            degraded: [],
            failure: nil,
            restartCountSinceBoot: 0,
            pid: 123
        )
        let endpoint = DaemonClientEndpoint(
            baseURL: URL(string: "http://127.0.0.1:1")!,
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

        vm.apply(frame: ready)
        await vm.refresh()

        #expect(vm.status == nil, "transport failure must not leave stale Ready rendered")
        #expect(vm.isUnavailable, "UI should move to the Unavailable banner")
        #expect(vm.lastError != nil, "the transport error remains visible for diagnostics")
    }

    // MARK: - WS URL derivation

    @Test func `endpoint graphqlWSURL rewrites scheme to ws`() {
        let endpoint = DaemonClientEndpoint(
            baseURL: URL(string: "http://127.0.0.1:4000")!,
            bearerToken: "x"
        )
        #expect(endpoint.graphqlWSURL.absoluteString == "ws://127.0.0.1:4000/graphql/ws")
    }
}
