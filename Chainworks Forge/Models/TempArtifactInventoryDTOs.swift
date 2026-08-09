import Foundation

/// Validates the ByteCountString contract shared with the backend scalar: "0" or
/// decimal digits with no leading zero, no sign, no whitespace — matching the
/// canonical `^(0|[1-9][0-9]*)$` regex. A malformed value ("-1", "01", "", " ")
/// must fail decoding rather than being silently accepted as a plain String.
func isValidByteCountString(_ s: String) -> Bool {
    guard !s.isEmpty else { return false }
    guard s.allSatisfy({ $0.isASCII && $0.isNumber }) else { return false }
    return s == "0" || s.first != "0"
}

extension KeyedDecodingContainer {
    /// Decodes a ByteCountString field, throwing if the value does not match
    /// `^(0|[1-9][0-9]*)$` rather than accepting any string.
    func decodeByteCountString(forKey key: Key) throws -> String {
        let raw = try decode(String.self, forKey: key)
        guard isValidByteCountString(raw) else {
            throw DecodingError.dataCorruptedError(
                forKey: key,
                in: self,
                debugDescription: "ByteCountString must match ^(0|[1-9][0-9]*)$, got \(raw.debugDescription)"
            )
        }
        return raw
    }

    /// Decodes a closed-vocabulary, enum-shaped field and canonicalizes it to the
    /// lowercase snake_case form the shared `temp_artifact_inventory_v1` contract
    /// defines (e.g. "complete", "resource_exhausted"). The GraphQL lane emits
    /// these same fields as SCREAMING_SNAKE_CASE ("COMPLETE", "RESOURCE_EXHAUSTED")
    /// — async-graphql's wire casing for the typed backend enums — while the MCP/
    /// run-report/release-receipt lanes already emit the canonical lowercase form
    /// directly from the domain enum's serde impl. Lowercasing is a no-op for the
    /// latter and a real normalization for the former, so every lane's response
    /// decodes to the same canonical string this DTO's consumers switch on.
    func decodeCanonicalEnumString(forKey key: Key) throws -> String {
        try decode(String.self, forKey: key).lowercased()
    }

    /// `decodeCanonicalEnumString`, but for an optional/nullable field.
    func decodeCanonicalEnumStringIfPresent(forKey key: Key) throws -> String? {
        try decodeIfPresent(String.self, forKey: key)?.lowercased()
    }
}

/// P089: Codable DTOs for decoding the canonical snake_case JSON from the MCP/backend response.
/// Swift never receives raw absolute paths; path_display is always a redacted string from the backend.
/// ByteCountString fields (estimatedBytes, estimatedSizeBytes) are unsigned decimal strings, validated
/// via `decodeByteCountString` above rather than accepted as plain unvalidated strings.
struct TempArtifactInventoryResponse: Decodable, Sendable {
    let schemaVersion: String
    let status: String
    let enabledState: String
    /// Backend process-start mode: "disabled" | "hidden_readback" | "operator_visible".
    /// Distinct from `enabledState` — hidden_readback and operator_visible both report
    /// `enabledState == "enabled"`, but only operator_visible authorizes the packaged
    /// app to show the diagnostics surface. Composed with the local
    /// `TempArtifactDiagnosticsVisibilityStore` preference rather than trusted alone
    /// (that preference is app-local and cannot itself reflect the daemon's mode).
    let mode: String
    let disabledReasonCode: String?
    let generatedAt: String
    let limitsApplied: LimitsApplied
    let summary: Summary
    let rows: [Row]
    let errors: [ErrorEntry]
    let dryRun: DryRun?
    let mutationGuard: MutationGuard

    enum CodingKeys: String, CodingKey {
        case schemaVersion = "schema_version"
        case status
        case enabledState = "enabled_state"
        case mode
        case disabledReasonCode = "disabled_reason_code"
        case generatedAt = "generated_at"
        case limitsApplied = "limits_applied"
        case summary
        case rows
        case errors
        case dryRun = "dry_run"
        case mutationGuard = "mutation_guard"
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        schemaVersion = try c.decode(String.self, forKey: .schemaVersion)
        status = try c.decodeCanonicalEnumString(forKey: .status)
        enabledState = try c.decodeCanonicalEnumString(forKey: .enabledState)
        mode = try c.decodeCanonicalEnumString(forKey: .mode)
        disabledReasonCode = try c.decodeIfPresent(String.self, forKey: .disabledReasonCode)
        generatedAt = try c.decode(String.self, forKey: .generatedAt)
        limitsApplied = try c.decode(LimitsApplied.self, forKey: .limitsApplied)
        summary = try c.decode(Summary.self, forKey: .summary)
        rows = try c.decode([Row].self, forKey: .rows)
        errors = try c.decode([ErrorEntry].self, forKey: .errors)
        dryRun = try c.decodeIfPresent(DryRun.self, forKey: .dryRun)
        mutationGuard = try c.decode(MutationGuard.self, forKey: .mutationGuard)
    }

    struct LimitsApplied: Decodable, Sendable {
        let limit: Int
        let timeoutMs: Int
        let scanDeadlineAt: String?
        let queueWaitMs: Int

        enum CodingKeys: String, CodingKey {
            case limit
            case timeoutMs = "timeout_ms"
            case scanDeadlineAt = "scan_deadline_at"
            case queueWaitMs = "queue_wait_ms"
        }
    }

    struct Summary: Decodable, Sendable {
        let artifactTreeCount: Int
        let estimatedBytes: String
        let activeOrRecentCount: Int
        let terminalCandidateCount: Int
        let orphanCandidateCount: Int
        let legacyUnmanagedCount: Int
        let scanErrorCount: Int
        let dryRunCandidateCount: Int
        let truncated: Bool
        let queueWaitMs: Int

        enum CodingKeys: String, CodingKey {
            case artifactTreeCount = "artifact_tree_count"
            case estimatedBytes = "estimated_bytes"
            case activeOrRecentCount = "active_or_recent_count"
            case terminalCandidateCount = "terminal_candidate_count"
            case orphanCandidateCount = "orphan_candidate_count"
            case legacyUnmanagedCount = "legacy_unmanaged_count"
            case scanErrorCount = "scan_error_count"
            case dryRunCandidateCount = "dry_run_candidate_count"
            case truncated
            case queueWaitMs = "queue_wait_ms"
        }

        init(from decoder: Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            artifactTreeCount = try c.decode(Int.self, forKey: .artifactTreeCount)
            estimatedBytes = try c.decodeByteCountString(forKey: .estimatedBytes)
            activeOrRecentCount = try c.decode(Int.self, forKey: .activeOrRecentCount)
            terminalCandidateCount = try c.decode(Int.self, forKey: .terminalCandidateCount)
            orphanCandidateCount = try c.decode(Int.self, forKey: .orphanCandidateCount)
            legacyUnmanagedCount = try c.decode(Int.self, forKey: .legacyUnmanagedCount)
            scanErrorCount = try c.decode(Int.self, forKey: .scanErrorCount)
            dryRunCandidateCount = try c.decode(Int.self, forKey: .dryRunCandidateCount)
            truncated = try c.decode(Bool.self, forKey: .truncated)
            queueWaitMs = try c.decode(Int.self, forKey: .queueWaitMs)
        }
    }

    struct Row: Decodable, Identifiable, Sendable {
        let pathDisplay: String
        let pathHash: String
        let pathHashShort: String
        let correlationKey: String
        let rootKind: String
        let artifactKind: String?
        let manifestState: String?
        let lifecycleClassification: String
        let dryRunRecommendation: String?
        let estimatedSizeBytes: String
        let lastTouchedAt: String?
        let activeProcessEvidence: String?
        let owner: String?
        let ownerInference: String?
        let statusToken: String
        let generatedAt: String
        let partialErrors: [String]

        /// Proposal: uses path_hash when present (64-char hex), otherwise correlation_key.
        var id: String { Self.stableIdentity(pathHash: pathHash, correlationKey: correlationKey) }

        /// Shared path_hash-first / correlation_key-fallback rule so `id` and
        /// `TempArtifactRowIdentity.from` cannot silently diverge from each other
        /// or from the documented contract.
        static func stableIdentity(pathHash: String, correlationKey: String) -> String {
            pathHash.count == 64 && pathHash.allSatisfy(\.isHexDigit) ? pathHash : correlationKey
        }

        enum CodingKeys: String, CodingKey {
            case pathDisplay = "path_display"
            case pathHash = "path_hash"
            case pathHashShort = "path_hash_short"
            case correlationKey = "correlation_key"
            case rootKind = "root_kind"
            case artifactKind = "artifact_kind"
            case manifestState = "manifest_state"
            case lifecycleClassification = "lifecycle_classification"
            case dryRunRecommendation = "dry_run_recommendation"
            case estimatedSizeBytes = "estimated_size_bytes"
            case lastTouchedAt = "last_touched_at"
            case activeProcessEvidence = "active_process_evidence"
            case owner
            case ownerInference = "owner_inference"
            case statusToken = "status_token"
            case generatedAt = "generated_at"
            case partialErrors = "partial_errors"
        }

        init(from decoder: Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            pathDisplay = try c.decode(String.self, forKey: .pathDisplay)
            pathHash = try c.decode(String.self, forKey: .pathHash)
            pathHashShort = try c.decode(String.self, forKey: .pathHashShort)
            correlationKey = try c.decode(String.self, forKey: .correlationKey)
            rootKind = try c.decodeCanonicalEnumString(forKey: .rootKind)
            artifactKind = try c.decodeIfPresent(String.self, forKey: .artifactKind)
            manifestState = try c.decodeIfPresent(String.self, forKey: .manifestState)
            lifecycleClassification = try c.decodeCanonicalEnumString(forKey: .lifecycleClassification)
            dryRunRecommendation = try c.decodeCanonicalEnumStringIfPresent(forKey: .dryRunRecommendation)
            estimatedSizeBytes = try c.decodeByteCountString(forKey: .estimatedSizeBytes)
            lastTouchedAt = try c.decodeIfPresent(String.self, forKey: .lastTouchedAt)
            activeProcessEvidence = try c.decodeIfPresent(String.self, forKey: .activeProcessEvidence)
            owner = try c.decodeIfPresent(String.self, forKey: .owner)
            ownerInference = try c.decodeIfPresent(String.self, forKey: .ownerInference)
            statusToken = try c.decode(String.self, forKey: .statusToken)
            generatedAt = try c.decode(String.self, forKey: .generatedAt)
            partialErrors = try c.decode([String].self, forKey: .partialErrors)
        }
    }

    struct ErrorEntry: Decodable, Sendable {
        let code: String
        let message: String
        let rootKind: String?
        let phase: String?

        init(code: String, message: String, rootKind: String?, phase: String? = nil) {
            self.code = code
            self.message = message
            self.rootKind = rootKind
            self.phase = phase
        }

        enum CodingKeys: String, CodingKey {
            case code
            case message
            case rootKind = "root_kind"
            case phase
        }

        init(from decoder: Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            code = try c.decodeCanonicalEnumString(forKey: .code)
            message = try c.decode(String.self, forKey: .message)
            rootKind = try c.decodeCanonicalEnumStringIfPresent(forKey: .rootKind)
            phase = try c.decodeIfPresent(String.self, forKey: .phase)
        }
    }

    struct DryRun: Decodable, Sendable {
        let generatedAt: String?
        let mutationGuard: MutationGuard
        let recommendationCounts: [String: Int]?

        enum CodingKeys: String, CodingKey {
            case generatedAt = "generated_at"
            case mutationGuard = "mutation_guard"
            case recommendationCounts = "recommendation_counts"
        }
    }

    struct MutationGuard: Decodable, Sendable {
        let status: String
        let checkedAt: String?
        let noDelete: Bool?
        let noPrune: Bool?
        let noChmod: Bool?
        let noPersist: Bool?
        let noRetry: Bool?
        let evidence: String?

        enum CodingKeys: String, CodingKey {
            case status
            case checkedAt = "checked_at"
            case noDelete = "no_delete"
            case noPrune = "no_prune"
            case noChmod = "no_chmod"
            case noPersist = "no_persist"
            case noRetry = "no_retry"
            case evidence
        }

        init(from decoder: Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            status = try c.decodeCanonicalEnumString(forKey: .status)
            checkedAt = try c.decodeIfPresent(String.self, forKey: .checkedAt)
            noDelete = try c.decodeIfPresent(Bool.self, forKey: .noDelete)
            noPrune = try c.decodeIfPresent(Bool.self, forKey: .noPrune)
            noChmod = try c.decodeIfPresent(Bool.self, forKey: .noChmod)
            noPersist = try c.decodeIfPresent(Bool.self, forKey: .noPersist)
            noRetry = try c.decodeIfPresent(Bool.self, forKey: .noRetry)
            evidence = try c.decodeIfPresent(String.self, forKey: .evidence)
        }
    }
}

/// Stable row identity using path_hash when present (64-char hex), otherwise correlation_key.
/// Stable only within the current daemon process and not across daemon restarts.
struct TempArtifactRowIdentity: Equatable, Sendable {
    let value: String

    static func from(row: TempArtifactInventoryResponse.Row) -> TempArtifactRowIdentity {
        TempArtifactRowIdentity(
            value: TempArtifactInventoryResponse.Row.stableIdentity(
                pathHash: row.pathHash,
                correlationKey: row.correlationKey
            )
        )
    }
}
