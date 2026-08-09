import Foundation
import Testing
@testable import Chainworks_Forge

// MARK: - Fixtures

private let disabledPayloadJSON = #"""
{
  "schema_version": "temp_artifact_inventory_v1",
  "status": "disabled",
  "enabled_state": "disabled",
  "mode": "disabled",
  "disabled_reason_code": "mode_disabled",
  "generated_at": "2026-01-01T00:00:00Z",
  "limits_applied": {"limit": 500, "timeout_ms": 5000, "scan_deadline_at": null, "queue_wait_ms": 0},
  "summary": {
    "artifact_tree_count": 0, "estimated_bytes": "0",
    "active_or_recent_count": 0, "terminal_candidate_count": 0,
    "orphan_candidate_count": 0, "legacy_unmanaged_count": 0,
    "scan_error_count": 0, "dry_run_candidate_count": 0, "truncated": false,
    "queue_wait_ms": 0
  },
  "rows": [],
  "errors": [],
  "mutation_guard": {
    "status": "skipped",
    "checked_at": "2026-01-01T00:00:00Z",
    "no_delete": true,
    "no_prune": true,
    "no_chmod": true,
    "no_persist": true,
    "no_retry": true
  }
}
"""#

private let completeRowPayloadJSON = #"""
{
  "schema_version": "temp_artifact_inventory_v1",
  "status": "complete",
  "enabled_state": "enabled",
  "mode": "operator_visible",
  "disabled_reason_code": null,
  "generated_at": "2026-06-28T12:00:00Z",
  "limits_applied": {"limit": 500, "timeout_ms": 5000, "scan_deadline_at": "2026-06-28T12:00:05Z", "queue_wait_ms": 10},
  "summary": {
    "artifact_tree_count": 1, "estimated_bytes": "2147483648",
    "active_or_recent_count": 1, "terminal_candidate_count": 0,
    "orphan_candidate_count": 0, "legacy_unmanaged_count": 0,
    "scan_error_count": 0, "dry_run_candidate_count": 1, "truncated": false,
    "queue_wait_ms": 10
  },
  "rows": [
    {
      "path_display": "<redacted>/runs/abc123",
      "path_hash": "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899",
      "path_hash_short": "aabbccddeeff",
      "correlation_key": "aabbccddeeff",
      "root_kind": "run_meta_root",
      "artifact_kind": null,
      "manifest_state": null,
      "lifecycle_classification": "active_or_recent",
      "dry_run_recommendation": "would_keep_active",
      "estimated_size_bytes": "2147483648",
      "last_touched_at": "2026-06-28T11:00:00Z",
      "active_process_evidence": null,
      "owner": null,
      "owner_inference": null,
      "status_token": "active",
      "generated_at": "2026-06-28T12:00:00Z",
      "partial_errors": []
    }
  ],
  "errors": [],
  "dry_run": {
    "generated_at": "2026-06-28T12:00:00Z",
    "mutation_guard": {
      "status": "pass",
      "checked_at": "2026-06-28T12:00:00Z",
      "no_delete": true,
      "no_prune": true,
      "no_chmod": true,
      "no_persist": true,
      "no_retry": true
    },
    "recommendation_counts": {"would_keep_active": 1}
  },
  "mutation_guard": {
    "status": "pass",
    "checked_at": "2026-06-28T12:00:00Z",
    "no_delete": true,
    "no_prune": true,
    "no_chmod": true,
    "no_persist": true,
    "no_retry": true
  }
}
"""#

/// Mirrors the exact wire shape the production `TempArtifactInventoryGraphQLFetcher`
/// actually receives: field *keys* are already snake_case (the GraphQL query document
/// aliases every field to its canonical name), but every enum-typed field *value* is
/// SCREAMING_SNAKE_CASE — async-graphql's wire casing for the typed backend enums
/// (`InventoryStatus`, `EnabledState`, `RootKind`, `LifecycleClassification`,
/// `DryRunRecommendation`, `InventoryErrorCode`, `MutationGuardStatus`). Regression
/// fixture for the GraphQL/Swift enum-casing wire mismatch: without normalization,
/// decoding this exact shape would leave `status == "COMPLETE"` etc., which never
/// matches the lowercase literals `ViewModel.viewState` switches on.
private let graphqlCasedCompletePayloadJSON = #"""
{
  "schema_version": "temp_artifact_inventory_v1",
  "status": "COMPLETE",
  "enabled_state": "ENABLED",
  "mode": "OPERATOR_VISIBLE",
  "disabled_reason_code": null,
  "generated_at": "2026-06-28T12:00:00Z",
  "limits_applied": {"limit": 500, "timeout_ms": 5000, "scan_deadline_at": "2026-06-28T12:00:05Z", "queue_wait_ms": 10},
  "summary": {
    "artifact_tree_count": 1, "estimated_bytes": "2147483648",
    "active_or_recent_count": 1, "terminal_candidate_count": 0,
    "orphan_candidate_count": 0, "legacy_unmanaged_count": 0,
    "scan_error_count": 0, "dry_run_candidate_count": 1, "truncated": false,
    "queue_wait_ms": 10
  },
  "rows": [
    {
      "path_display": "<redacted>/runs/abc123",
      "path_hash": "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899",
      "path_hash_short": "aabbccddeeff",
      "correlation_key": "aabbccddeeff",
      "root_kind": "RUN_META_ROOT",
      "artifact_kind": null,
      "manifest_state": null,
      "lifecycle_classification": "ACTIVE_OR_RECENT",
      "dry_run_recommendation": "WOULD_KEEP_ACTIVE",
      "estimated_size_bytes": "2147483648",
      "last_touched_at": "2026-06-28T11:00:00Z",
      "active_process_evidence": null,
      "owner": null,
      "owner_inference": null,
      "status_token": "active",
      "generated_at": "2026-06-28T12:00:00Z",
      "partial_errors": []
    }
  ],
  "errors": [],
  "dry_run": {
    "generated_at": "2026-06-28T12:00:00Z",
    "mutation_guard": {
      "status": "PASS",
      "checked_at": "2026-06-28T12:00:00Z"
    },
    "recommendation_counts": {"would_keep_active": 1}
  },
  "mutation_guard": {
    "status": "PASS",
    "checked_at": "2026-06-28T12:00:00Z",
    "no_delete": true,
    "no_prune": true,
    "no_chmod": true,
    "no_persist": true,
    "no_retry": true
  }
}
"""#

/// Same GraphQL SCREAMING_SNAKE_CASE wire shape as above, but for the terminal
/// statuses the real backend actually returns for a disabled/degraded scan
/// (DISABLED, RESOURCE_EXHAUSTED, TIMEOUT, CANCELLED) — the exact values the
/// audit/prepush review reports flagged as falling through to the wrong UI state.
private func graphqlCasedTerminalPayloadJSON(status: String, errorCode: String?) -> String {
    let errorsJSON: String
    if let errorCode {
        errorsJSON = #"[{"code": "\#(errorCode)", "message": "<redacted>", "root_kind": null}]"#
    } else {
        errorsJSON = "[]"
    }
    return #"""
        {
          "schema_version": "temp_artifact_inventory_v1",
          "status": "\#(status)",
          "enabled_state": "ENABLED",
          "mode": "OPERATOR_VISIBLE",
          "disabled_reason_code": null,
          "generated_at": "2026-06-28T12:00:00Z",
          "limits_applied": {"limit": 500, "timeout_ms": 5000, "scan_deadline_at": null, "queue_wait_ms": 0},
          "summary": {
            "artifact_tree_count": 0, "estimated_bytes": "0",
            "active_or_recent_count": 0, "terminal_candidate_count": 0,
            "orphan_candidate_count": 0, "legacy_unmanaged_count": 0,
            "scan_error_count": 0, "dry_run_candidate_count": 0, "truncated": false,
            "queue_wait_ms": 0
          },
          "rows": [],
          "errors": \#(errorsJSON),
          "mutation_guard": {
            "status": "SKIPPED",
            "checked_at": "2026-06-28T12:00:00Z",
            "no_delete": true,
            "no_prune": true,
            "no_chmod": true,
            "no_persist": true,
            "no_retry": true
          }
        }
        """#
}

// MARK: - Immediate fetcher stubs

private struct ImmediateDisabledFetcher: TempArtifactInventoryFetching {
    func fetchInventory(runID: String) async -> Result<TempArtifactInventoryResponse, Error> {
        let data = disabledPayloadJSON.data(using: .utf8)!
        let r = try! JSONDecoder().decode(TempArtifactInventoryResponse.self, from: data)
        return .success(r)
    }
}

private struct ImmediateCompleteFetcher: TempArtifactInventoryFetching {
    func fetchInventory(runID: String) async -> Result<TempArtifactInventoryResponse, Error> {
        let data = completeRowPayloadJSON.data(using: .utf8)!
        let r = try! JSONDecoder().decode(TempArtifactInventoryResponse.self, from: data)
        return .success(r)
    }
}

private struct OperatorVisibleCapabilityDisabledRefreshFetcher: TempArtifactInventoryFetching {
    func fetchInventory(runID: String) async -> Result<TempArtifactInventoryResponse, Error> {
        let data = disabledPayloadJSON.data(using: .utf8)!
        let response = try! JSONDecoder().decode(TempArtifactInventoryResponse.self, from: data)
        return .success(response)
    }

    func fetchInventoryCapability(runID: String) async -> Result<TempArtifactInventoryResponse, Error> {
        let data = completeRowPayloadJSON.data(using: .utf8)!
        let response = try! JSONDecoder().decode(TempArtifactInventoryResponse.self, from: data)
        return .success(response)
    }
}

private struct NullAnnouncer: TempArtifactAccessibilityAnnouncing {
    @MainActor func announce(_ message: String) {}
}

@MainActor
private final class SpyAnnouncer: TempArtifactAccessibilityAnnouncing {
    private(set) var messages: [String] = []
    func announce(_ message: String) { messages.append(message) }
}

private enum P089TestFetchError: Error {
    case failed
}

private struct CancellationAcknowledgingFetcher: TempArtifactInventoryFetching {
    func fetchInventory(runID: String) async -> Result<TempArtifactInventoryResponse, Error> {
        do {
            try await Task.sleep(for: .seconds(30))
            return .failure(P089TestFetchError.failed)
        } catch {
            // Returning after Task cancellation models URLSession completing its
            // cancellation path and therefore acknowledges the transport close.
            return .failure(error)
        }
    }
}

@MainActor
private final class SequencedFetcher: TempArtifactInventoryFetching {
    private var results: [Result<TempArtifactInventoryResponse, Error>]

    init(results: [Result<TempArtifactInventoryResponse, Error>]) {
        self.results = results
    }

    func fetchInventory(runID: String) async -> Result<TempArtifactInventoryResponse, Error> {
        if results.isEmpty {
            return .failure(P089TestFetchError.failed)
        }
        return results.removeFirst()
    }
}

// MARK: - Test suite

@MainActor
@Suite("P089TempArtifactInventory", .serialized, .tags(.fast))
struct P089TempArtifactInventoryTests {

    // MARK: - Visibility store

    @Test("Visibility store defaults to false when key is absent")
    func visibilityStoreDefaultsToFalse() throws {
        let suiteName = "p089-vis-store-test-\(UUID().uuidString)"
        let defaults = try #require(UserDefaults(suiteName: suiteName))
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let store = TempArtifactDiagnosticsVisibilityStore(defaults: defaults)
        #expect(store.isVisible == false)
    }

    @Test("Visibility store reflects setVisible(true)")
    func visibilityStoreSetTrue() throws {
        let suiteName = "p089-vis-store-test-\(UUID().uuidString)"
        let defaults = try #require(UserDefaults(suiteName: suiteName))
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let store = TempArtifactDiagnosticsVisibilityStore(defaults: defaults)
        store.setVisible(true)
        #expect(store.isVisible == true)
    }

    @Test("Visibility store reflects setVisible(false) after true")
    func visibilityStoreRoundTrip() throws {
        let suiteName = "p089-vis-store-test-\(UUID().uuidString)"
        let defaults = try #require(UserDefaults(suiteName: suiteName))
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let store = TempArtifactDiagnosticsVisibilityStore(defaults: defaults)
        store.setVisible(true)
        store.setVisible(false)
        #expect(store.isVisible == false)
    }

    @Test("Visibility store domain is com.chainworks.forge")
    func visibilityStoreDomain() {
        #expect(TempArtifactDiagnosticsVisibilityStore.domain == "com.chainworks.forge")
    }

    @Test("Visibility store key is TempArtifactDiagnosticsVisible")
    func visibilityStoreKey() {
        #expect(TempArtifactDiagnosticsVisibilityStore.visibilityKey == "TempArtifactDiagnosticsVisible")
    }

    @Test("Backend visibility fails closed until capability mode is known")
    func backendVisibilityFailsClosedBeforeCapabilityReadback() {
        let viewModel = TempArtifactInventoryViewModel(
            fetcher: ImmediateCompleteFetcher(),
            announcer: NullAnnouncer()
        )
        #expect(viewModel.backendVisibilityMode == nil)
        #expect(viewModel.isBackendAuthorizedForVisibleSurface == false)
    }

    @Test("operator_visible capability authorizes the surface without accepting scan rows")
    func operatorVisibleCapabilityAuthorizesSurface() async {
        let viewModel = TempArtifactInventoryViewModel(
            fetcher: ImmediateCompleteFetcher(),
            announcer: NullAnnouncer()
        )
        viewModel.resolveBackendVisibility(runID: "run-1")
        await Task.yield()
        await Task.yield()
        #expect(viewModel.backendVisibilityMode == "operator_visible")
        #expect(viewModel.isBackendAuthorizedForVisibleSurface)
        #expect(viewModel.lastAcceptedPayload == nil)
        #expect(viewModel.viewState == .firstLoad)
    }

    @Test("disabled capability keeps the surface hidden")
    func disabledCapabilityKeepsSurfaceHidden() async {
        let viewModel = TempArtifactInventoryViewModel(
            fetcher: ImmediateDisabledFetcher(),
            announcer: NullAnnouncer()
        )
        viewModel.resolveBackendVisibility(runID: "run-1")
        await Task.yield()
        await Task.yield()
        #expect(viewModel.backendVisibilityMode == "disabled")
        #expect(viewModel.isBackendAuthorizedForVisibleSurface == false)
    }

    @Test("Accepted refresh mode revokes stale operator-visible capability")
    func acceptedRefreshModeRevokesStaleCapability() async {
        let viewModel = TempArtifactInventoryViewModel(
            fetcher: OperatorVisibleCapabilityDisabledRefreshFetcher(),
            announcer: NullAnnouncer()
        )

        viewModel.resolveBackendVisibility(runID: "run-1")
        for _ in 0..<20 where viewModel.backendVisibilityMode == nil {
            await Task.yield()
        }
        #expect(viewModel.backendVisibilityMode == "operator_visible")

        viewModel.beginRefresh(runID: "run-1")
        for _ in 0..<20 where viewModel.inFlightGenerationID != nil {
            await Task.yield()
        }

        #expect(viewModel.backendVisibilityMode == "disabled")
        #expect(viewModel.isBackendAuthorizedForVisibleSurface == false)
    }

    // MARK: - DTO decoding

    @Test("Disabled payload decodes to correct fields")
    func disabledPayloadDecodes() throws {
        let data = try #require(disabledPayloadJSON.data(using: .utf8))
        let response = try JSONDecoder().decode(TempArtifactInventoryResponse.self, from: data)
        #expect(response.schemaVersion == "temp_artifact_inventory_v1")
        #expect(response.status == "disabled")
        #expect(response.enabledState == "disabled")
        #expect(response.disabledReasonCode == "mode_disabled")
        #expect(response.rows.isEmpty)
        #expect(response.errors.isEmpty)
        #expect(response.dryRun == nil)
        #expect(response.mutationGuard.status == "skipped")
        #expect(response.mutationGuard.noDelete == true)
        #expect(response.mutationGuard.noPersist == true)
    }

    @Test("Complete payload decodes rows with all required fields")
    func completePayloadDecodesRows() throws {
        let data = try #require(completeRowPayloadJSON.data(using: .utf8))
        let response = try JSONDecoder().decode(TempArtifactInventoryResponse.self, from: data)
        #expect(response.status == "complete")
        #expect(response.rows.count == 1)
        let row = response.rows[0]
        #expect(row.pathHash.count == 64)
        #expect(row.pathHashShort.count == 12)
        #expect(row.lifecycleClassification == "active_or_recent")
        #expect(row.estimatedSizeBytes == "2147483648")
        #expect(row.dryRunRecommendation == "would_keep_active")
        #expect(row.partialErrors.isEmpty)
    }

    // MARK: - GraphQL SCREAMING_SNAKE_CASE enum wire-casing regression

    @Test("GraphQL-cased COMPLETE payload decodes to canonical lowercase enum fields")
    func graphqlCasedCompletePayloadDecodesToCanonicalLowercase() throws {
        // Regression for the audit/prepush-flagged GraphQL/Swift wire mismatch:
        // the GraphQL lane emits enum values as SCREAMING_SNAKE_CASE
        // (async-graphql's default enum wire casing), not the canonical
        // lowercase snake_case the MCP/report/receipt lanes emit. Decoding must
        // normalize every enum-typed field so `ViewModel.viewState` — which
        // switches on the lowercase canonical literals — resolves identically
        // regardless of which lane the payload came from.
        let data = try #require(graphqlCasedCompletePayloadJSON.data(using: .utf8))
        let response = try JSONDecoder().decode(TempArtifactInventoryResponse.self, from: data)
        #expect(response.status == "complete")
        #expect(response.enabledState == "enabled")
        #expect(response.mutationGuard.status == "pass")
        #expect(response.dryRun?.mutationGuard.status == "pass")
        #expect(response.rows.count == 1)
        let row = response.rows[0]
        #expect(row.rootKind == "run_meta_root")
        #expect(row.lifecycleClassification == "active_or_recent")
        #expect(row.dryRunRecommendation == "would_keep_active")
    }

    @Test(
        "GraphQL-cased terminal statuses resolve to the correct ViewModel viewState, not .error/.completeEmpty fallthrough",
        arguments: [
            ("DISABLED", nil as String?, "disabled"),
            ("RESOURCE_EXHAUSTED", nil, "busy"),
            ("TIMEOUT", "DEADLINE_EXCEEDED", "partialTimeoutCancelled"),
            ("CANCELLED", "CANCELLED", "partialTimeoutCancelled"),
        ]
    )
    func graphqlCasedTerminalStatusesResolveCorrectViewState(
        status: String,
        errorCode: String?,
        expectedState: String
    ) async throws {
        let json = graphqlCasedTerminalPayloadJSON(status: status, errorCode: errorCode)
        let data = try #require(json.data(using: .utf8))
        let response = try JSONDecoder().decode(TempArtifactInventoryResponse.self, from: data)
        // Decoding must normalize the status (and, when present, the error code)
        // to canonical lowercase — asserting this directly is what would have
        // caught the wire mismatch before it ever reached the view model.
        #expect(response.status == status.lowercased())
        if let errorCode {
            #expect(response.errors.first?.code == errorCode.lowercased())
        }

        let fetcher = SequencedFetcher(results: [.success(response)])
        let viewModel = TempArtifactInventoryViewModel(fetcher: fetcher, announcer: NullAnnouncer())
        viewModel.beginRefresh(runID: "test-run")
        await Task.yield()
        await Task.yield()

        switch expectedState {
        case "disabled":
            guard case .disabled = viewModel.viewState else {
                Issue.record("expected .disabled, got \(viewModel.viewState)")
                return
            }
        case "busy":
            #expect(viewModel.viewState == .busy)
        case "partialTimeoutCancelled":
            #expect(viewModel.viewState == .partialTimeoutCancelled)
        default:
            Issue.record("unhandled expected state \(expectedState)")
        }
    }

    @Test("Decoded row identity uses path_hash when it is a valid 64-char hex string")
    func rowIdentityUsesPathHashWhenValid() throws {
        // completeRowPayloadJSON's fixture row has a 64-hex-char path_hash distinct
        // from its (12-char) correlation_key, so this exercises the real proposal
        // contract: path_hash takes priority whenever it is present and valid.
        let data = try #require(completeRowPayloadJSON.data(using: .utf8))
        let response = try JSONDecoder().decode(TempArtifactInventoryResponse.self, from: data)
        let row = response.rows[0]
        #expect(row.pathHash.count == 64)
        #expect(row.pathHash != row.correlationKey)
        let identity = TempArtifactRowIdentity.from(row: row)
        #expect(identity.value == row.pathHash)
        #expect(row.id == row.pathHash)
    }

    @Test("Decoded row identity falls back to correlationKey when path_hash is not valid 64-char hex")
    func rowIdentityFallsBackToCorrelationKey() {
        let identity = TempArtifactInventoryResponse.Row.stableIdentity(
            pathHash: "too-short",
            correlationKey: "fallback-correlation-key"
        )
        #expect(identity == "fallback-correlation-key")
    }

    @Test("Summary estimated bytes can hold over-2GB decimal string")
    func summaryBytesOver2GB() throws {
        let data = try #require(completeRowPayloadJSON.data(using: .utf8))
        let response = try JSONDecoder().decode(TempArtifactInventoryResponse.self, from: data)
        #expect(response.summary.estimatedBytes == "2147483648")
        let parsed = UInt64(response.summary.estimatedBytes)
        #expect(parsed == 2_147_483_648)
    }

    @Test("isValidByteCountString accepts zero and positive decimals, rejects malformed values")
    func byteCountStringValidation() {
        #expect(isValidByteCountString("0"))
        #expect(isValidByteCountString("1"))
        #expect(isValidByteCountString("2147483648"))
        #expect(!isValidByteCountString("-1"))
        #expect(!isValidByteCountString("01"))
        #expect(!isValidByteCountString(""))
        #expect(!isValidByteCountString(" "))
        #expect(!isValidByteCountString("1.5"))
        #expect(!isValidByteCountString("1e10"))
    }

    @Test(
        "Decoding rejects malformed estimated_bytes instead of accepting any string",
        arguments: ["-1", "01", "", " ", "1.5"]
    )
    func decodingRejectsMalformedSummaryByteCount(malformed: String) throws {
        let json = """
        {
          "schema_version": "temp_artifact_inventory_v1",
          "status": "complete",
          "enabled_state": "enabled",
          "mode": "operator_visible",
          "disabled_reason_code": null,
          "generated_at": "2026-06-28T12:00:00Z",
          "limits_applied": {"limit": 500, "timeout_ms": 5000, "scan_deadline_at": null, "queue_wait_ms": 0},
          "summary": {
            "artifact_tree_count": 0, "estimated_bytes": "\(malformed)",
            "active_or_recent_count": 0, "terminal_candidate_count": 0,
            "orphan_candidate_count": 0, "legacy_unmanaged_count": 0,
            "scan_error_count": 0, "dry_run_candidate_count": 0, "truncated": false,
            "queue_wait_ms": 0
          },
          "rows": [], "errors": [],
          "mutation_guard": {
            "status": "skipped", "checked_at": "2026-01-01T00:00:00Z",
            "no_delete": true, "no_prune": true, "no_chmod": true, "no_persist": true, "no_retry": true
          }
        }
        """
        let data = try #require(json.data(using: .utf8))
        #expect(throws: (any Error).self) {
            try JSONDecoder().decode(TempArtifactInventoryResponse.self, from: data)
        }
    }

    @Test("Mutation guard is present in disabled payload")
    func mutationGuardPresentInDisabledPayload() throws {
        let data = try #require(disabledPayloadJSON.data(using: .utf8))
        let response = try JSONDecoder().decode(TempArtifactInventoryResponse.self, from: data)
        #expect(response.mutationGuard.status == "skipped")
        #expect(response.mutationGuard.checkedAt == "2026-01-01T00:00:00Z")
        #expect(response.mutationGuard.noDelete == true)
        #expect(response.mutationGuard.noPrune == true)
        #expect(response.mutationGuard.noChmod == true)
        #expect(response.mutationGuard.noPersist == true)
        #expect(response.mutationGuard.noRetry == true)
    }

    @Test("DryRun mutation guard is pass in complete payload")
    func dryRunMutationGuardPassInCompletePayload() throws {
        let data = try #require(completeRowPayloadJSON.data(using: .utf8))
        let response = try JSONDecoder().decode(TempArtifactInventoryResponse.self, from: data)
        let dryRun = try #require(response.dryRun)
        #expect(dryRun.mutationGuard.status == "pass")
        #expect(dryRun.mutationGuard.noDelete == true)
        #expect(dryRun.recommendationCounts?["would_keep_active"] == 1)
    }

    @Test("Production GraphQL fetcher requests the full typed canonical projection")
    func graphqlFetcherRequestsTypedProjection() {
        let document = TempArtifactInventoryGraphQLFetcher.document
        #expect(document.contains("temp_artifact_inventory: tempArtifactInventory"))
        #expect(document.contains("estimated_size_bytes: estimatedSizeBytes"))
        #expect(document.contains("partial_errors: partialErrors"))
        #expect(document.contains("mutation_guard: mutationGuard"))
        #expect(!document.contains("canonicalJson"))
    }

    @Test("Right-click targeting prefers context row without changing keyboard selection")
    func rightClickTargetingPrefersContextRow() {
        let keyboardSelection = "keyboard-row"
        let target = TempArtifactContextMenuTargeting.targetID(
            contextSelection: ["right-click-row"],
            keyboardSelection: keyboardSelection
        )
        #expect(target == "right-click-row")
        #expect(keyboardSelection == "keyboard-row")
    }

    @Test("Context copy falls back to keyboard selection when menu has no row")
    func rightClickTargetingFallsBackToKeyboardSelection() {
        let target = TempArtifactContextMenuTargeting.targetID(
            contextSelection: [],
            keyboardSelection: "keyboard-row"
        )
        #expect(target == "keyboard-row")
    }

    // MARK: - View model initial state

    @MainActor
    @Test("View model initial state is firstLoad")
    func viewModelInitialStateIsFirstLoad() {
        let vm = TempArtifactInventoryViewModel(
            fetcher: ImmediateDisabledFetcher(),
            announcer: NullAnnouncer()
        )
        #expect(vm.viewState == .firstLoad)
        #expect(vm.acceptedGenerationID == nil)
        #expect(vm.inFlightGenerationID == nil)
        #expect(vm.selectedRowIdentity == nil)
        #expect(vm.focusedCopyCommandEnabled == false)
        #expect(vm.displayRows.isEmpty)
    }

    @MainActor
    @Test("beginRefresh transitions to loading state")
    func beginRefreshTransitionsToLoading() {
        let vm = TempArtifactInventoryViewModel(
            fetcher: ImmediateDisabledFetcher(),
            announcer: NullAnnouncer()
        )
        vm.beginRefresh(runID: "run-1")
        #expect(vm.viewState == .loadingWithoutPrior)
        #expect(vm.inFlightGenerationID != nil)
        vm.cancelRefresh()
    }

    @MainActor
    @Test("cancelRefresh clears inFlightGenerationID after task acknowledgement")
    func cancelRefreshClearsInFlight() async {
        let vm = TempArtifactInventoryViewModel(
            fetcher: ImmediateDisabledFetcher(),
            announcer: NullAnnouncer()
        )
        vm.beginRefresh(runID: "run-1")
        vm.cancelRefresh()
        for _ in 0..<20 where vm.inFlightGenerationID != nil {
            await Task.yield()
        }
        #expect(vm.inFlightGenerationID == nil)
    }

    @MainActor
    @Test("Explicit cancellation waits for acknowledgement and renders cancelled terminal state")
    func explicitCancellationRendersTerminalState() async {
        let announcer = SpyAnnouncer()
        let vm = TempArtifactInventoryViewModel(
            fetcher: CancellationAcknowledgingFetcher(),
            announcer: announcer
        )
        vm.setSceneActivity(isVisible: true, isFocused: true)
        vm.beginRefresh(runID: "run-1")
        await Task.yield()

        vm.cancelRefresh()
        for _ in 0..<50 where vm.inFlightGenerationID != nil {
            await Task.yield()
        }

        #expect(vm.inFlightGenerationID == nil)
        #expect(vm.viewState == .partialTimeoutCancelled)
        #expect(vm.topLevelErrors.map(\.code) == ["cancelled"])
        #expect(announcer.messages == ["Temporary artifact inventory was cancelled."])
    }

    @MainActor
    @Test("selectRow enables copy command")
    func selectRowEnablesCopyCommand() throws {
        let data = try #require(completeRowPayloadJSON.data(using: .utf8))
        let response = try JSONDecoder().decode(TempArtifactInventoryResponse.self, from: data)
        let vm = TempArtifactInventoryViewModel(
            fetcher: ImmediateCompleteFetcher(),
            announcer: NullAnnouncer()
        )
        let row = response.rows[0]
        vm.selectRow(row)
        #expect(vm.focusedCopyCommandEnabled == true)
        #expect(vm.selectedRowIdentity != nil)
    }

    @MainActor
    @Test("selectRow(nil) disables copy command")
    func deselectRowDisablesCopyCommand() throws {
        let data = try #require(completeRowPayloadJSON.data(using: .utf8))
        let response = try JSONDecoder().decode(TempArtifactInventoryResponse.self, from: data)
        let vm = TempArtifactInventoryViewModel(
            fetcher: ImmediateCompleteFetcher(),
            announcer: NullAnnouncer()
        )
        vm.selectRow(response.rows[0])
        vm.selectRow(nil)
        #expect(vm.focusedCopyCommandEnabled == false)
        #expect(vm.selectedRowIdentity == nil)
    }

    @MainActor
    @Test("onSceneClose cancels refresh and clears state")
    func onSceneCloseClearsState() {
        let vm = TempArtifactInventoryViewModel(
            fetcher: ImmediateDisabledFetcher(),
            announcer: NullAnnouncer()
        )
        vm.beginRefresh(runID: "run-1")
        vm.onSceneClose()
        #expect(vm.inFlightGenerationID == nil)
        #expect(vm.selectedRowIdentity == nil)
        #expect(vm.focusedCopyCommandEnabled == false)
    }

    @MainActor
    @Test("onFocusTransfer disables copy command without clearing selection")
    func onFocusTransferDisablesCopyCommand() throws {
        let data = try #require(completeRowPayloadJSON.data(using: .utf8))
        let response = try JSONDecoder().decode(TempArtifactInventoryResponse.self, from: data)
        let vm = TempArtifactInventoryViewModel(
            fetcher: ImmediateCompleteFetcher(),
            announcer: NullAnnouncer()
        )
        vm.selectRow(response.rows[0])
        vm.onFocusTransfer()
        #expect(vm.focusedCopyCommandEnabled == false)
        #expect(vm.selectedRowIdentity != nil)
    }

    @Test("Focus transfer clears an in-flight refresh without a cancellation banner")
    func focusTransferClearsInFlightRefresh() async {
        let announcer = SpyAnnouncer()
        let vm = TempArtifactInventoryViewModel(
            fetcher: CancellationAcknowledgingFetcher(),
            announcer: announcer
        )
        vm.setSceneActivity(isVisible: true, isFocused: true)
        vm.beginRefresh(runID: "run-1")
        #expect(vm.inFlightGenerationID != nil)

        vm.onFocusTransfer()

        #expect(vm.inFlightGenerationID == nil)
        #expect(vm.viewState == .firstLoad)
        await Task.yield()
        await Task.yield()
        #expect(announcer.messages.isEmpty)
    }

    @MainActor
    @Test("beginRefresh preserves selectedRowIdentity — stale rows stay selectable")
    func beginRefreshPreservesSelectedRowIdentity() throws {
        let data = try #require(completeRowPayloadJSON.data(using: .utf8))
        let response = try JSONDecoder().decode(TempArtifactInventoryResponse.self, from: data)
        let vm = TempArtifactInventoryViewModel(
            fetcher: ImmediateCompleteFetcher(),
            announcer: NullAnnouncer()
        )
        vm.selectRow(response.rows[0])
        #expect(vm.selectedRowIdentity != nil)
        #expect(vm.focusedCopyCommandEnabled == true)
        vm.beginRefresh(runID: "run-1")
        // Selection must be preserved while the new generation is in flight.
        #expect(vm.selectedRowIdentity != nil)
        #expect(vm.focusedCopyCommandEnabled == true)
        vm.cancelRefresh()
    }

    @MainActor
    @Test("cancelRefresh preserves selectedRowIdentity — stale rows stay selectable")
    func cancelRefreshPreservesSelectedRowIdentity() throws {
        let data = try #require(completeRowPayloadJSON.data(using: .utf8))
        let response = try JSONDecoder().decode(TempArtifactInventoryResponse.self, from: data)
        let vm = TempArtifactInventoryViewModel(
            fetcher: ImmediateCompleteFetcher(),
            announcer: NullAnnouncer()
        )
        vm.selectRow(response.rows[0])
        vm.beginRefresh(runID: "run-1")
        vm.cancelRefresh()
        // After cancel the selection must still be set; copy must remain enabled when row is in display set.
        #expect(vm.selectedRowIdentity != nil)
    }

    @MainActor
    @Test("Failed refresh preserves prior rows as stale")
    func failedRefreshPreservesPriorRowsAsStale() async throws {
        let data = try #require(completeRowPayloadJSON.data(using: .utf8))
        let response = try JSONDecoder().decode(TempArtifactInventoryResponse.self, from: data)
        let fetcher = SequencedFetcher(results: [
            .success(response),
            .failure(P089TestFetchError.failed),
        ])
        let vm = TempArtifactInventoryViewModel(
            fetcher: fetcher,
            announcer: NullAnnouncer()
        )

        vm.beginRefresh(runID: "run-1")
        await Task.yield()
        await Task.yield()
        #expect(vm.viewState == .completeWithRows)
        #expect(vm.displayRows.count == 1)

        vm.beginRefresh(runID: "run-1")
        #expect(vm.viewState == .loadingOverStale)
        await Task.yield()
        await Task.yield()

        #expect(vm.viewState == .error)
        #expect(vm.displayRows.count == 1)
        #expect(vm.displayRows.first?.pathHash == response.rows[0].pathHash)
        #expect(vm.isDisplayingStaleRows == true)
    }

    // MARK: - Pasteboard writer spy

    @MainActor
    @Test("Pasteboard writer spy records write call")
    func pasteboardWriterSpyRecordsCall() throws {
        let data = try #require(completeRowPayloadJSON.data(using: .utf8))
        let response = try JSONDecoder().decode(TempArtifactInventoryResponse.self, from: data)
        let spy = TempArtifactRowPasteboardWriterSpy()
        spy.writeRedactedRow(response.rows[0], stale: false)
        #expect(spy.writeCalls.count == 1)
        #expect(spy.writeCalls[0].stale == false)
    }

    @MainActor
    @Test("Pasteboard writer spy records stale flag")
    func pasteboardWriterSpyRecordsStaleFlag() throws {
        let data = try #require(completeRowPayloadJSON.data(using: .utf8))
        let response = try JSONDecoder().decode(TempArtifactInventoryResponse.self, from: data)
        let spy = TempArtifactRowPasteboardWriterSpy()
        spy.writeRedactedRow(response.rows[0], stale: true)
        #expect(spy.writeCalls[0].stale == true)
    }

    // MARK: - Disabled stub fetcher

    @Test("Disabled stub fetcher returns disabled status payload")
    func disabledStubFetcherReturnsDisabledPayload() async throws {
        let fetcher = TempArtifactInventoryFetcherDisabledStub()
        let result = await fetcher.fetchInventory(runID: "test-run")
        switch result {
        case .success(let response):
            #expect(response.status == "disabled")
                #expect(response.enabledState == "disabled")
                #expect(response.schemaVersion == "temp_artifact_inventory_v1")
                #expect(response.mutationGuard.status == "skipped")
                #expect(response.mutationGuard.noDelete == true)
            case .failure(let error):
                Issue.record("Expected success, got failure: \(error)")
            }
    }

    // MARK: - Status-specific accessibility announcements

    @MainActor
    @Test("Complete payload announces item count, not a generic success message")
    func completeAnnouncementIncludesItemCount() async throws {
        let announcer = SpyAnnouncer()
        let vm = TempArtifactInventoryViewModel(
            fetcher: ImmediateCompleteFetcher(),
            announcer: announcer
        )
        vm.setSceneActivity(isVisible: true, isFocused: true)
        vm.beginRefresh(runID: "run-1")
        await Task.yield()
        await Task.yield()
        #expect(announcer.messages == ["Temporary artifact inventory complete. 1 item found."])
    }

    @MainActor
    @Test("Disabled payload announces disabled, not inventory-complete")
    func disabledAnnouncementDoesNotClaimSuccess() async throws {
        // Regression: the prior implementation always announced "inventory
        // complete" regardless of status, so a disabled backend told VoiceOver
        // users a scan had succeeded.
        let announcer = SpyAnnouncer()
        let vm = TempArtifactInventoryViewModel(
            fetcher: ImmediateDisabledFetcher(),
            announcer: announcer
        )
        vm.setSceneActivity(isVisible: true, isFocused: true)
        vm.beginRefresh(runID: "run-1")
        await Task.yield()
        await Task.yield()
        #expect(announcer.messages == ["Temporary artifact inventory is disabled."])
    }

    @MainActor
    @Test("Transport failure announces a terminal failure, not silence")
    func transportFailureAnnouncesFailure() async {
        // Regression: the prior `.failure` branch never announced anything, so a
        // total transport failure was indistinguishable from "still loading" to
        // VoiceOver users.
        let announcer = SpyAnnouncer()
        let fetcher = SequencedFetcher(results: [.failure(P089TestFetchError.failed)])
        let vm = TempArtifactInventoryViewModel(fetcher: fetcher, announcer: announcer)
        vm.setSceneActivity(isVisible: true, isFocused: true)
        vm.beginRefresh(runID: "run-1")
        await Task.yield()
        await Task.yield()
        #expect(announcer.messages == ["Temporary artifact inventory failed to load."])
    }

    @MainActor
    @Test("Terminal announcement is suppressed when the scene cannot announce")
    func announcementSuppressedWhenSceneCannotAnnounce() async {
        let announcer = SpyAnnouncer()
        let vm = TempArtifactInventoryViewModel(
            fetcher: ImmediateCompleteFetcher(),
            announcer: announcer
        )
        // No setSceneActivity call: sceneCanAnnounce defaults to false.
        vm.beginRefresh(runID: "run-1")
        await Task.yield()
        await Task.yield()
        #expect(announcer.messages.isEmpty)
    }
}
