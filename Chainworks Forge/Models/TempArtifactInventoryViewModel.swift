import Foundation
import Observation
import AppKit

// MARK: - Fetcher protocol

/// P089: Injectable protocol for the backend inventory fetch. The production implementation
/// calls the MCP tool or GraphQL query. The stub returns a disabled-mode payload.
protocol TempArtifactInventoryFetching: Sendable {
    func fetchInventory(runID: String) async -> Result<TempArtifactInventoryResponse, Error>
    func fetchInventoryCapability(runID: String) async -> Result<TempArtifactInventoryResponse, Error>
}

extension TempArtifactInventoryFetching {
    /// Test doubles may reuse their normal response. Production overrides this
    /// with a `limit: 0` request so visibility can be resolved without enumerating
    /// artifact rows.
    func fetchInventoryCapability(runID: String) async -> Result<TempArtifactInventoryResponse, Error> {
        await fetchInventory(runID: runID)
    }
}

// MARK: - Accessibility announcer protocol

/// P089: Injectable accessibility announcer. Terminal state announcements are scoped to the
/// visible scene that initiated or accepted the generation; suppressed for hidden scenes.
protocol TempArtifactAccessibilityAnnouncing: Sendable {
    @MainActor func announce(_ message: String)
}

// MARK: - View state

extension TempArtifactInventoryViewModel {
    enum ViewState: Equatable {
        case firstLoad
        case loadingWithoutPrior
        case loadingOverStale
        case completeWithRows
        case completeEmpty
        case partialTimeoutCancelled
        case error
        case disabled(reasonCode: String?)
        case busy
    }
}

// MARK: - View model

/// P089: Scene-scoped owner of temp artifact inventory presentation state.
/// Not a singleton; one instance per Run Report diagnostics scene.
/// Swift never performs filesystem scanning or path inference; all data comes from the backend.
@MainActor
@Observable
final class TempArtifactInventoryViewModel {

    private(set) var acceptedGenerationID: String? = nil
    private(set) var inFlightGenerationID: String? = nil
    private(set) var selectedRowIdentity: TempArtifactRowIdentity? = nil
    private(set) var staleSnapshot: TempArtifactInventoryResponse? = nil
    private(set) var lastAcceptedPayload: TempArtifactInventoryResponse? = nil
    private(set) var lastTerminalAnnouncementGenerationID: String? = nil
    private(set) var focusedCopyCommandEnabled: Bool = false
    private(set) var backendVisibilityMode: String? = nil

    private var refreshTaskHandle: Task<Void, Never>? = nil
    private var visibilityTaskHandle: Task<Void, Never>? = nil
    private var lastRefreshFailed = false
    private var lastRefreshCancelled = false
    private var explicitCancellationGenerationID: String? = nil
    private var sceneCanAnnounce = false

    private let fetcher: any TempArtifactInventoryFetching
    private let announcer: any TempArtifactAccessibilityAnnouncing

    var viewState: ViewState {
        if inFlightGenerationID != nil {
            return staleSnapshot != nil ? .loadingOverStale : .loadingWithoutPrior
        }
        if lastRefreshFailed {
            return .error
        }
        if lastRefreshCancelled {
            return .partialTimeoutCancelled
        }
        guard let response = lastAcceptedPayload else {
            return .firstLoad
        }
        switch response.status {
        case "disabled": return .disabled(reasonCode: response.disabledReasonCode)
        case "resource_exhausted": return .busy
        case "complete": return response.rows.isEmpty ? .completeEmpty : .completeWithRows
        case "partial", "timeout", "cancelled": return .partialTimeoutCancelled
        case "error": return .error
        default: return response.rows.isEmpty ? .completeEmpty : .completeWithRows
        }
    }

    /// Rows to display. Stale rows from the prior generation are shown while a new fetch is in flight.
    var displayRows: [TempArtifactInventoryResponse.Row] {
        if let live = lastAcceptedPayload, !live.rows.isEmpty { return live.rows }
        return staleSnapshot?.rows ?? []
    }

    var displayPayload: TempArtifactInventoryResponse? {
        if let live = lastAcceptedPayload, !live.rows.isEmpty { return live }
        return staleSnapshot ?? lastAcceptedPayload
    }

    /// True when the displayed rows come from a prior accepted generation while a new fetch is in flight.
    var isDisplayingStaleRows: Bool {
        guard staleSnapshot != nil else { return false }
        return inFlightGenerationID != nil
            || lastRefreshFailed
            || lastRefreshCancelled
            || lastAcceptedPayload?.status == "resource_exhausted"
            || lastAcceptedPayload?.status == "error"
    }

    var selectedRow: TempArtifactInventoryResponse.Row? {
        guard let identity = selectedRowIdentity else { return nil }
        return displayRows.first { TempArtifactRowIdentity.from(row: $0) == identity }
    }

    var topLevelErrors: [TempArtifactInventoryResponse.ErrorEntry] {
        if lastRefreshCancelled {
            return [
                TempArtifactInventoryResponse.ErrorEntry(
                    code: "cancelled",
                    message: "<redacted>",
                    rootKind: nil
                )
            ]
        }
        return lastAcceptedPayload?.errors ?? []
    }

    /// Composes the local `TempArtifactDiagnosticsVisibilityStore` preference with the
    /// backend's actual `mode`, so a stale/true local preference alone can never keep
    /// the surface visible once the daemon is confirmed to be in `hidden_readback` or
    /// `disabled` mode — only `operator_visible` authorizes the packaged app surface.
    /// Fail closed until a capability response establishes backend truth. The view
    /// issues a row-free capability request when the local preference is enabled;
    /// hidden_readback and disabled therefore never flash operator UI on first load.
    var isBackendAuthorizedForVisibleSurface: Bool {
        backendVisibilityMode == "operator_visible"
    }

    init(
        fetcher: any TempArtifactInventoryFetching = TempArtifactInventoryGraphQLFetcher(),
        announcer: any TempArtifactAccessibilityAnnouncing = TempArtifactAccessibilityAnnouncer()
    ) {
        self.fetcher = fetcher
        self.announcer = announcer
    }

    // MARK: - Commands

    func resolveBackendVisibility(runID: String) {
        visibilityTaskHandle?.cancel()
        visibilityTaskHandle = Task { [weak self, fetcher] in
            let result = await fetcher.fetchInventoryCapability(runID: runID)
            guard let self, !Task.isCancelled else { return }
            switch result {
            case .success(let payload):
                self.backendVisibilityMode = payload.mode
            case .failure:
                self.backendVisibilityMode = nil
            }
        }
    }

    func beginRefresh(runID: String) {
        let generationID = UUID().uuidString
        if let current = lastAcceptedPayload {
            staleSnapshot = current
        }
        lastRefreshFailed = false
        lastRefreshCancelled = false
        explicitCancellationGenerationID = nil
        inFlightGenerationID = generationID
        // Do NOT clear selectedRowIdentity or focusedCopyCommandEnabled here.
        // Stale rows from the prior generation remain selectable and copyable
        // while the new generation is in flight.

        refreshTaskHandle?.cancel()
        refreshTaskHandle = Task { [weak self, fetcher] in
            let result = await fetcher.fetchInventory(runID: runID)
            guard let self else { return }
            if Task.isCancelled {
                // Awaiting the fetch completion is the transport acknowledgement
                // boundary. Only explicit operator cancellation for the still-current
                // generation becomes visible terminal state. Superseded, unfocused,
                // and closed-scene generations remain discarded.
                if self.inFlightGenerationID == generationID,
                   self.explicitCancellationGenerationID == generationID {
                    self.acceptCancellationAcknowledgement(generationID: generationID)
                }
                return
            }
            guard self.inFlightGenerationID == generationID else { return }
            self.acceptResult(generationID: generationID, result: result)
        }
    }

    func cancelRefresh() {
        guard let generationID = inFlightGenerationID else { return }
        // Keep the generation in flight until the fetch returns after cancellation.
        // That completion is the client-visible backend/transport acknowledgement.
        explicitCancellationGenerationID = generationID
        refreshTaskHandle?.cancel()
        lastRefreshFailed = false
    }

    func selectRow(_ row: TempArtifactInventoryResponse.Row?) {
        if let row {
            selectedRowIdentity = TempArtifactRowIdentity.from(row: row)
            focusedCopyCommandEnabled = true
        } else {
            selectedRowIdentity = nil
            focusedCopyCommandEnabled = false
        }
    }

    func onSceneClose() {
        sceneCanAnnounce = false
        visibilityTaskHandle?.cancel()
        visibilityTaskHandle = nil
        cancelRefresh()
        // The scene is the owner of this generation. Once it closes there is no
        // presentation state left to retain; the cancelled task's generation guard
        // prevents a late completion from being accepted.
        inFlightGenerationID = nil
        explicitCancellationGenerationID = nil
        staleSnapshot = nil
        selectedRowIdentity = nil
        focusedCopyCommandEnabled = false
        lastRefreshFailed = false
        lastRefreshCancelled = false
    }

    func onFocusTransfer() {
        sceneCanAnnounce = false
        focusedCopyCommandEnabled = false
        // Cancel the in-flight refresh when focus leaves the diagnostics surface.
        // Accepted stale rows are preserved; only the pending request is cancelled.
        refreshTaskHandle?.cancel()
        refreshTaskHandle = nil
        inFlightGenerationID = nil
        explicitCancellationGenerationID = nil
    }

    func setSceneActivity(isVisible: Bool, isFocused: Bool) {
        sceneCanAnnounce = isVisible && isFocused
        if !sceneCanAnnounce {
            onFocusTransfer()
        } else {
            updateCopyEnablement()
        }
    }

    // MARK: - Private

    private func acceptResult(
        generationID: String,
        result: Result<TempArtifactInventoryResponse, Error>
    ) {
        inFlightGenerationID = nil
        explicitCancellationGenerationID = nil
        lastRefreshCancelled = false
        switch result {
        case .success(let payload):
            // Every accepted response carries the daemon's current kill-switch mode.
            // Apply it immediately so a runtime rollback cannot leave a previously
            // authorized diagnostics surface visible.
            backendVisibilityMode = payload.mode
            let shouldPreserveStaleRows = payload.rows.isEmpty
                && (payload.status == "resource_exhausted" || payload.status == "error")
                && staleSnapshot != nil
            if !shouldPreserveStaleRows {
                staleSnapshot = nil
            }
            lastRefreshFailed = false
            lastAcceptedPayload = payload
            acceptedGenerationID = generationID
            updateCopyEnablement()
            announceTerminalIfNeeded(generationID: generationID)
        case .failure:
            if staleSnapshot == nil {
                staleSnapshot = lastAcceptedPayload
            }
            lastRefreshFailed = true
            updateCopyEnablement()
            announceTransportFailureIfNeeded(generationID: generationID)
        }
    }

    private func acceptCancellationAcknowledgement(generationID: String) {
        inFlightGenerationID = nil
        explicitCancellationGenerationID = nil
        refreshTaskHandle = nil
        lastRefreshFailed = false
        lastRefreshCancelled = true
        acceptedGenerationID = generationID
        updateCopyEnablement()

        guard sceneCanAnnounce else { return }
        guard lastTerminalAnnouncementGenerationID != generationID else { return }
        lastTerminalAnnouncementGenerationID = generationID
        announcer.announce("Temporary artifact inventory was cancelled.")
    }

    private func updateCopyEnablement() {
        focusedCopyCommandEnabled = selectedRowIdentity != nil && selectedRow != nil
    }

    private func announceTerminalIfNeeded(generationID: String) {
        guard sceneCanAnnounce else { return }
        guard acceptedGenerationID == generationID else { return }
        guard lastTerminalAnnouncementGenerationID != generationID else { return }
        lastTerminalAnnouncementGenerationID = generationID
        announcer.announce(terminalAnnouncementMessage())
    }

    /// Transport-level failure (no payload was ever accepted for this generation) is
    /// its own terminal case, distinct from a backend-reported `status: "error"`
    /// payload — both must announce something, since silence reads as "still
    /// working" to VoiceOver rather than "this request ended."
    private func announceTransportFailureIfNeeded(generationID: String) {
        guard sceneCanAnnounce else { return }
        guard lastTerminalAnnouncementGenerationID != generationID else { return }
        lastTerminalAnnouncementGenerationID = generationID
        announcer.announce("Temporary artifact inventory failed to load.")
    }

    /// Status-specific terminal announcement. Regression: the prior implementation
    /// always announced "inventory complete" regardless of status (disabled, partial,
    /// timeout, cancelled, resource_exhausted, error all included), which told
    /// VoiceOver users a scan succeeded when it did not.
    private func terminalAnnouncementMessage() -> String {
        guard let response = lastAcceptedPayload else {
            return "Temporary artifact inventory failed to load."
        }
        let count = response.rows.count
        let itemsFound = "\(count) item\(count == 1 ? "" : "s") found."
        switch response.status {
        case "complete":
            return "Temporary artifact inventory complete. \(itemsFound)"
        case "disabled":
            return "Temporary artifact inventory is disabled."
        case "partial":
            return "Temporary artifact inventory partially completed. \(itemsFound)"
        case "timeout":
            return "Temporary artifact inventory timed out."
        case "cancelled":
            return "Temporary artifact inventory was cancelled."
        case "resource_exhausted":
            return "Temporary artifact inventory is busy. Try again shortly."
        case "error":
            return "Temporary artifact inventory failed to load."
        default:
            return "Temporary artifact inventory finished with status \(response.status)."
        }
    }
}

// MARK: - Production GraphQL fetcher

/// Reads the Operator-only GraphQL projection from the bundled daemon. GraphQL
/// aliases keep the response byte-for-byte compatible with the canonical
/// snake_case DTO decoded by `TempArtifactInventoryResponse`.
struct TempArtifactInventoryGraphQLFetcher: TempArtifactInventoryFetching {
    private let client: P031GraphQLReadClient<P031URLSessionGraphQLReadTransport>

    init(
        endpoint: DaemonClientEndpoint = .operatorDefault(),
        urlSession: URLSession = .shared
    ) {
        client = P031GraphQLReadClient(
            transport: P031URLSessionGraphQLReadTransport(
                endpoint: endpoint,
                urlSession: urlSession
            )
        )
    }

    func fetchInventory(runID: String) async -> Result<TempArtifactInventoryResponse, Error> {
        await fetchInventory(runID: runID, limit: 500)
    }

    func fetchInventoryCapability(runID: String) async -> Result<TempArtifactInventoryResponse, Error> {
        await fetchInventory(runID: runID, limit: 0)
    }

    private func fetchInventory(
        runID: String,
        limit: Int
    ) async -> Result<TempArtifactInventoryResponse, Error> {
        do {
            let payload = try await client.execute(
                GraphQLPayload.self,
                operationName: "TempArtifactInventoryPreview",
                document: Self.document,
                variables: [
                    "runId": .string(runID),
                    "limit": .int(limit),
                ]
            )
            return .success(payload.inventory)
        } catch {
            return .failure(error)
        }
    }

    private struct GraphQLPayload: Decodable {
        let inventory: TempArtifactInventoryResponse

        enum CodingKeys: String, CodingKey {
            case inventory = "temp_artifact_inventory"
        }
    }

    static let document = """
        query TempArtifactInventoryPreview($runId: ID!, $limit: Int!) {
          temp_artifact_inventory: tempArtifactInventory(
            input: {
              runId: $runId
              limit: $limit
              timeoutMs: 5000
              includeDryRun: true
            }
          ) {
            schema_version: schemaVersion
            status
            enabled_state: enabledState
            mode
            disabled_reason_code: disabledReasonCode
            generated_at: generatedAt
            limits_applied: limitsApplied {
              limit
              timeout_ms: timeoutMs
              scan_deadline_at: scanDeadlineAt
              queue_wait_ms: queueWaitMs
            }
            summary {
              artifact_tree_count: artifactTreeCount
              estimated_bytes: estimatedBytes
              active_or_recent_count: activeOrRecentCount
              terminal_candidate_count: terminalCandidateCount
              orphan_candidate_count: orphanCandidateCount
              legacy_unmanaged_count: legacyUnmanagedCount
              scan_error_count: scanErrorCount
              dry_run_candidate_count: dryRunCandidateCount
              truncated
              queue_wait_ms: queueWaitMs
            }
            rows {
              path_display: pathDisplay
              path_hash: pathHash
              path_hash_short: pathHashShort
              correlation_key: correlationKey
              root_kind: rootKind
              artifact_kind: artifactKind
              manifest_state: manifestState
              lifecycle_classification: lifecycleClassification
              dry_run_recommendation: dryRunRecommendation
              estimated_size_bytes: estimatedSizeBytes
              last_touched_at: lastTouchedAt
              active_process_evidence: activeProcessEvidence
              owner
              owner_inference: ownerInference
              status_token: statusToken
              generated_at: generatedAt
              partial_errors: partialErrors
            }
            errors {
              code
              message
              root_kind: rootKind
              phase
            }
            dry_run: dryRun {
              generated_at: generatedAt
              recommendation_counts: recommendationCounts
              mutation_guard: mutationGuard {
                status
                checked_at: checkedAt
              }
            }
            mutation_guard: mutationGuard {
              status
              checked_at: checkedAt
              no_delete: noDelete
              no_prune: noPrune
              no_chmod: noChmod
              no_persist: noPersist
              no_retry: noRetry
            }
          }
        }
        """
}

// MARK: - Disabled fixture fetcher

/// Deterministic disabled payload used by focused unit tests and previews.
struct TempArtifactInventoryFetcherDisabledStub: TempArtifactInventoryFetching {
    func fetchInventory(runID: String) async -> Result<TempArtifactInventoryResponse, Error> {
        let json = #"""
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
        """#.data(using: .utf8)!
        do {
            let response = try JSONDecoder().decode(TempArtifactInventoryResponse.self, from: json)
            return .success(response)
        } catch {
            return .failure(error)
        }
    }
}

// MARK: - Production accessibility announcer

struct TempArtifactAccessibilityAnnouncer: TempArtifactAccessibilityAnnouncing {
    @MainActor
    func announce(_ message: String) {
        NSAccessibility.post(
            element: NSApp as AnyObject,
            notification: .announcementRequested,
            userInfo: [NSAccessibility.NotificationUserInfoKey.announcement: message as NSString]
        )
    }
}
