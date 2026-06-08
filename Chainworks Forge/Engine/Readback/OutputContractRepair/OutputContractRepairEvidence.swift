import Foundation

// ---------------------------------------------------------------------------
// P079: Contract-Aware Output Repair and Provider Fallback
// Swift readback DTO module (APPLE-001, APPLE-R3-001)
//
// Decodes output_contract_repair.v1 evidence records from GraphQL or MCP responses.
// Pre-P079 runs and feature-disabled runs decode as nil without throwing.
// Unknown enum values map to .unknownDiagnostic(rawString) — the original provider
// raw string is preserved in the associated value for inspector forensics (DEFECT-006).
// The conservative fallback never authorizes output settlement, transition truth,
// or operator-initiated dispatch.
// ---------------------------------------------------------------------------

// MARK: - Top-level status enum (api-contract-r2-001)

nonisolated enum OutputContractRepairStatus: Codable, Sendable, Equatable {
    case notAttempted
    case inProgress
    case recovered
    case blocked
    case skipped
    case cancelled
    case failed
    /// Conservative fallback for unknown future values. Never authorizes settlement.
    /// The original provider raw string is preserved as the associated value for inspector forensics.
    case unknownDiagnostic(String)

    var rawValue: String {
        switch self {
        case .notAttempted: return "not_attempted"
        case .inProgress: return "in_progress"
        case .recovered: return "recovered"
        case .blocked: return "blocked"
        case .skipped: return "skipped"
        case .cancelled: return "cancelled"
        case .failed: return "failed"
        case .unknownDiagnostic(let raw): return raw
        }
    }

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        switch raw {
        case "not_attempted": self = .notAttempted
        case "in_progress": self = .inProgress
        case "recovered": self = .recovered
        case "blocked": self = .blocked
        case "skipped": self = .skipped
        case "cancelled": self = .cancelled
        case "failed": self = .failed
        default: self = .unknownDiagnostic(raw)
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(rawValue)
    }
}

// MARK: - Presentation category (macos-r3-001)

nonisolated enum PresentationCategory: Codable, Sendable, Equatable {
    case informational
    case recovered
    case blocked
    case skipped
    case failed
    case cancelled
    /// Conservative fallback. Rendered as caution symbol; never authorizes settlement.
    /// The original provider raw string is preserved as the associated value.
    case unknownDiagnostic(String)

    var rawValue: String {
        switch self {
        case .informational: return "informational"
        case .recovered: return "recovered"
        case .blocked: return "blocked"
        case .skipped: return "skipped"
        case .failed: return "failed"
        case .cancelled: return "cancelled"
        case .unknownDiagnostic(let raw): return raw
        }
    }

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        switch raw {
        case "informational": self = .informational
        case "recovered": self = .recovered
        case "blocked": self = .blocked
        case "skipped": self = .skipped
        case "failed": self = .failed
        case "cancelled": self = .cancelled
        default: self = .unknownDiagnostic(raw)
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(rawValue)
    }
}

// MARK: - Recommended next action (approved closed vocabulary)

nonisolated enum RecommendedNextAction: Codable, Sendable, Equatable {
    case `continue`
    case inspectRepairEvidence
    case configureFallbackPolicy
    case operatorResolveApproval
    case operatorResolveWorkflowConflict
    case retryAfterTransportRestored
    case cancelAcknowledged
    case manualInvestigation
    case unknownDiagnostic(String)

    var rawValue: String {
        switch self {
        case .continue: return "continue"
        case .inspectRepairEvidence: return "inspect_repair_evidence"
        case .configureFallbackPolicy: return "configure_fallback_policy"
        case .operatorResolveApproval: return "operator_resolve_approval"
        case .operatorResolveWorkflowConflict: return "operator_resolve_workflow_conflict"
        case .retryAfterTransportRestored: return "retry_after_transport_restored"
        case .cancelAcknowledged: return "cancel_acknowledged"
        case .manualInvestigation: return "manual_investigation"
        case .unknownDiagnostic(let raw): return raw
        }
    }

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        switch raw {
        case "continue": self = .continue
        case "inspect_repair_evidence": self = .inspectRepairEvidence
        case "configure_fallback_policy": self = .configureFallbackPolicy
        case "operator_resolve_approval": self = .operatorResolveApproval
        case "operator_resolve_workflow_conflict": self = .operatorResolveWorkflowConflict
        case "retry_after_transport_restored": self = .retryAfterTransportRestored
        case "cancel_acknowledged": self = .cancelAcknowledged
        case "manual_investigation": self = .manualInvestigation
        default: self = .unknownDiagnostic(raw)
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(rawValue)
    }
}

// MARK: - Initial failure classification

nonisolated enum InitialFailureClass: Codable, Sendable, Equatable {
    case noOutputProduced
    case emptyOutput
    case missingRequiredOutputs
    case invalidRequiredOutputs
    case outputContractMismatch
    case providerModeMismatch
    case unknownDiagnostic(String)

    var rawValue: String {
        switch self {
        case .noOutputProduced: return "no_output_produced"
        case .emptyOutput: return "empty_output"
        case .missingRequiredOutputs: return "missing_required_outputs"
        case .invalidRequiredOutputs: return "invalid_required_outputs"
        case .outputContractMismatch: return "output_contract_mismatch"
        case .providerModeMismatch: return "provider_mode_mismatch"
        case .unknownDiagnostic(let raw): return raw
        }
    }

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        switch raw {
        case "no_output_produced": self = .noOutputProduced
        case "empty_output": self = .emptyOutput
        case "missing_required_outputs": self = .missingRequiredOutputs
        case "invalid_required_outputs": self = .invalidRequiredOutputs
        case "output_contract_mismatch": self = .outputContractMismatch
        case "provider_mode_mismatch": self = .providerModeMismatch
        default: self = .unknownDiagnostic(raw)
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(rawValue)
    }
}

// MARK: - Adapter and provider family

nonisolated enum OutputContractAdapterFamily: Codable, Sendable, Equatable {
    case codex, claude, gemini, junie, auggie, fixture
    case unknownDiagnostic(String)

    var rawValue: String {
        switch self {
        case .codex: return "codex"
        case .claude: return "claude"
        case .gemini: return "gemini"
        case .junie: return "junie"
        case .auggie: return "auggie"
        case .fixture: return "fixture"
        case .unknownDiagnostic(let raw): return raw
        }
    }

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        switch raw {
        case "codex": self = .codex
        case "claude": self = .claude
        case "gemini": self = .gemini
        case "junie": self = .junie
        case "auggie": self = .auggie
        case "fixture": self = .fixture
        default: self = .unknownDiagnostic(raw)
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(rawValue)
    }
}

nonisolated enum OutputContractProviderFamily: Codable, Sendable, Equatable {
    case codex, claude, gemini, junie, auggie, fixture
    case unknownDiagnostic(String)

    var rawValue: String {
        switch self {
        case .codex: return "codex"
        case .claude: return "claude"
        case .gemini: return "gemini"
        case .junie: return "junie"
        case .auggie: return "auggie"
        case .fixture: return "fixture"
        case .unknownDiagnostic(let raw): return raw
        }
    }

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        switch raw {
        case "codex": self = .codex
        case "claude": self = .claude
        case "gemini": self = .gemini
        case "junie": self = .junie
        case "auggie": self = .auggie
        case "fixture": self = .fixture
        default: self = .unknownDiagnostic(raw)
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(rawValue)
    }
}

// MARK: - Lease

nonisolated enum LeaseState: Codable, Sendable, Equatable {
    case reserved
    case promptSent
    case settled
    case unknownDiagnostic(String)

    var rawValue: String {
        switch self {
        case .reserved: return "reserved"
        case .promptSent: return "prompt_sent"
        case .settled: return "settled"
        case .unknownDiagnostic(let raw): return raw
        }
    }

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        switch raw {
        case "reserved": self = .reserved
        case "prompt_sent": self = .promptSent
        case "settled": self = .settled
        default: self = .unknownDiagnostic(raw)
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(rawValue)
    }
}

nonisolated struct OutputContractLease: Codable, Sendable, Equatable {
    let key: String
    let kind: String
    let state: LeaseState
    let settledResult: String?
    let reclamationReason: String?
    let ownerPrincipalId: String
    /// RFC3339/ISO8601; decoder accepts with/without fractional seconds.
    let acquiredAt: String
    let expiresAt: String
    let leaseSeconds: Int

    enum CodingKeys: String, CodingKey {
        case key
        case kind
        case state
        case settledResult = "settled_result"
        case reclamationReason = "reclamation_reason"
        case ownerPrincipalId = "owner_principal_id"
        case acquiredAt = "acquired_at"
        case expiresAt = "expires_at"
        case leaseSeconds = "lease_seconds"
    }
}

// MARK: - Budget (v1: max 1 repair + 1 fallback per invocation)

nonisolated struct OutputContractBudget: Codable, Sendable, Equatable {
    let repairConsumed: Bool
    let fallbackConsumed: Bool
    /// Maximum repair attempts per invocation (always 1 in v1). Optional for pre-v1 compatibility.
    let repairMaxPerInvocation: Int?
    /// Maximum fallback attempts per invocation (always 1 in v1). Optional for pre-v1 compatibility.
    let fallbackMaxPerInvocation: Int?

    enum CodingKeys: String, CodingKey {
        case repairConsumed = "repair_consumed"
        case fallbackConsumed = "fallback_consumed"
        case repairMaxPerInvocation = "repair_max_per_invocation"
        case fallbackMaxPerInvocation = "fallback_max_per_invocation"
    }
}

// MARK: - Required output binding (v1 closed schema: name, contract_id, canonical_path)

nonisolated struct RequiredOutputBinding: Codable, Sendable, Equatable {
    /// Logical output name as declared in the agent contract.
    let name: String
    let contractId: String
    /// Absolute frozen canonical path from RunPlanSnapshot (never redacted in this field).
    let canonicalPath: String

    enum CodingKeys: String, CodingKey {
        case name
        case contractId = "contract_id"
        case canonicalPath = "canonical_path"
    }
}

// MARK: - Same-session repair

/// Closed v1 result enum. `Attempted` is not a valid v1 value.
nonisolated enum SameSessionRepairResult: Codable, Sendable, Equatable {
    case notNeeded
    case accepted
    case rejectedInvalid
    case unavailable
    case skippedIneligible
    case failedTransport
    case budgetExhausted
    case deadlineExceeded
    case cancelled
    case supersededIgnored
    case unknownDiagnostic(String)

    var rawValue: String {
        switch self {
        case .notNeeded: return "not_needed"
        case .accepted: return "accepted"
        case .rejectedInvalid: return "rejected_invalid"
        case .unavailable: return "unavailable"
        case .skippedIneligible: return "skipped_ineligible"
        case .failedTransport: return "failed_transport"
        case .budgetExhausted: return "budget_exhausted"
        case .deadlineExceeded: return "deadline_exceeded"
        case .cancelled: return "cancelled"
        case .supersededIgnored: return "superseded_ignored"
        case .unknownDiagnostic(let raw): return raw
        }
    }

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        switch raw {
        case "not_needed": self = .notNeeded
        case "accepted": self = .accepted
        case "rejected_invalid": self = .rejectedInvalid
        case "unavailable": self = .unavailable
        case "skipped_ineligible": self = .skippedIneligible
        case "failed_transport": self = .failedTransport
        case "budget_exhausted": self = .budgetExhausted
        case "deadline_exceeded": self = .deadlineExceeded
        case "cancelled": self = .cancelled
        case "superseded_ignored": self = .supersededIgnored
        default: self = .unknownDiagnostic(raw)
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(rawValue)
    }
}

nonisolated struct OutputContractRepairAttempt: Codable, Sendable, Equatable {
    let result: SameSessionRepairResult
    let turnCount: Int
    let deadlineSeconds: Int?
    let reason: String?
    let repairAttemptId: String?

    enum CodingKeys: String, CodingKey {
        case result
        case turnCount = "turn_count"
        case deadlineSeconds = "deadline_seconds"
        case reason
        case repairAttemptId = "repair_attempt_id"
    }
}

// MARK: - Transcript recovery

/// Closed v1 result enum.
/// `oversized_payload` and `unattributable_envelope` are subtypes under `unavailable` (sec-002).
nonisolated enum TranscriptRecoveryResult: Codable, Sendable, Equatable {
    case notNeeded
    case accepted
    case rejectedInvalid
    case skippedIneligible
    case failedTransport
    case cancelled
    case unavailable
    case unknownDiagnostic(String)

    var rawValue: String {
        switch self {
        case .notNeeded: return "not_needed"
        case .accepted: return "accepted"
        case .rejectedInvalid: return "rejected_invalid"
        case .skippedIneligible: return "skipped_ineligible"
        case .failedTransport: return "failed_transport"
        case .cancelled: return "cancelled"
        case .unavailable: return "unavailable"
        case .unknownDiagnostic(let raw): return raw
        }
    }

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        switch raw {
        case "not_needed": self = .notNeeded
        case "accepted": self = .accepted
        case "rejected_invalid": self = .rejectedInvalid
        case "skipped_ineligible": self = .skippedIneligible
        case "failed_transport": self = .failedTransport
        case "cancelled": self = .cancelled
        case "unavailable": self = .unavailable
        default: self = .unknownDiagnostic(raw)
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(rawValue)
    }
}

nonisolated struct OutputContractTranscriptRecovery: Codable, Sendable, Equatable {
    let result: TranscriptRecoveryResult
    /// Subtype for `unavailable` result: `oversized_payload` or `unattributable_envelope`.
    let resultSubtype: String?
    let recoverySource: String?
    let bytesExamined: Int?
    let maxRecoveryPayloadBytes: Int
    let maxJsonDepth: Int
    let maxChunksExamined: Int
    let recoveryParserVersion: String

    enum CodingKeys: String, CodingKey {
        case result
        case resultSubtype = "result_subtype"
        case recoverySource = "recovery_source"
        case bytesExamined = "bytes_examined"
        case maxRecoveryPayloadBytes = "max_recovery_payload_bytes"
        case maxJsonDepth = "max_json_depth"
        case maxChunksExamined = "max_chunks_examined"
        case recoveryParserVersion = "recovery_parser_version"
    }
}

// MARK: - Provider fallback

nonisolated enum ProviderFallbackResult: Codable, Sendable, Equatable {
    case notNeeded
    case scheduled
    case accepted
    case rejectedInvalid
    case unavailable
    case skippedIneligible
    case failedTransport
    case deadlineExceeded
    case cancelled
    case budgetExhausted
    case leaseContended
    case supersededIgnored
    case unknownDiagnostic(String)

    var rawValue: String {
        switch self {
        case .notNeeded: return "not_needed"
        case .scheduled: return "scheduled"
        case .accepted: return "accepted"
        case .rejectedInvalid: return "rejected_invalid"
        case .unavailable: return "unavailable"
        case .skippedIneligible: return "skipped_ineligible"
        case .failedTransport: return "failed_transport"
        case .deadlineExceeded: return "deadline_exceeded"
        case .cancelled: return "cancelled"
        case .budgetExhausted: return "budget_exhausted"
        case .leaseContended: return "lease_contended"
        case .supersededIgnored: return "superseded_ignored"
        case .unknownDiagnostic(let raw): return raw
        }
    }

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        switch raw {
        case "not_needed": self = .notNeeded
        case "scheduled": self = .scheduled
        case "accepted": self = .accepted
        case "rejected_invalid": self = .rejectedInvalid
        case "unavailable": self = .unavailable
        case "skipped_ineligible": self = .skippedIneligible
        case "failed_transport": self = .failedTransport
        case "deadline_exceeded": self = .deadlineExceeded
        case "cancelled": self = .cancelled
        case "budget_exhausted": self = .budgetExhausted
        case "lease_contended": self = .leaseContended
        case "superseded_ignored": self = .supersededIgnored
        default: self = .unknownDiagnostic(raw)
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(rawValue)
    }
}

nonisolated struct OutputContractProviderFallback: Codable, Sendable, Equatable {
    let result: ProviderFallbackResult
    let fallbackProfile: String?
    let fallbackAgentExecutionId: String?
    let parentFailedAgentExecutionId: String?
    let fallbackPacketHash: String?
    let fallbackPrincipalId: String?
    let fallbackPrincipalCapabilityHash: String?
    let deadlineSeconds: Int?

    enum CodingKeys: String, CodingKey {
        case result
        case fallbackProfile = "fallback_profile"
        case fallbackAgentExecutionId = "fallback_agent_execution_id"
        case parentFailedAgentExecutionId = "parent_failed_agent_execution_id"
        case fallbackPacketHash = "fallback_packet_hash"
        case fallbackPrincipalId = "fallback_principal_id"
        case fallbackPrincipalCapabilityHash = "fallback_principal_capability_hash"
        case deadlineSeconds = "deadline_seconds"
    }
}

// MARK: - Provider plan evidence

nonisolated struct OutputContractPlanEvidence: Codable, Sendable, Equatable {
    /// Run-meta-root-relative paths only (never absolute outside meta-root).
    let paths: [String]
    /// Redaction markers applied, e.g. `["[redacted:abs_path]", "[redacted:credential]"]`.
    let redactionsApplied: [String]
    let truncatedAtCap: Bool
    let acceptedAsOutput: Bool

    enum CodingKeys: String, CodingKey {
        case paths
        case redactionsApplied = "redactions_applied"
        case truncatedAtCap = "truncated_at_cap"
        case acceptedAsOutput = "accepted_as_output"
    }
}

// MARK: - Permission decisions

nonisolated enum PermissionDecisionValue: Codable, Sendable, Equatable {
    case allowed
    case denied
    case unknownDiagnostic(String)

    var rawValue: String {
        switch self {
        case .allowed: return "allowed"
        case .denied: return "denied"
        case .unknownDiagnostic(let raw): return raw
        }
    }

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        switch raw {
        case "allowed": self = .allowed
        case "denied": self = .denied
        default: self = .unknownDiagnostic(raw)
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(rawValue)
    }
}

nonisolated struct OutputContractPermissionDecision: Codable, Sendable, Equatable {
    let method: String
    let resourceKind: String
    let decision: PermissionDecisionValue
    let reason: String

    enum CodingKeys: String, CodingKey {
        case method
        case resourceKind = "resource_kind"
        case decision
        case reason
    }
}

// MARK: - Top-level evidence record

/// Versioned readback DTO for output_contract_repair.v1.
///
/// SwiftUI identity contract (APPLE-R3-001):
///   Stable identity = (repairAttemptId, agentExecutionId).
///   evidenceVersion is a monotonic content-version field used by Equatable and view-body
///   recomputation. It MUST NOT participate in Identifiable.id or ForEach keys.
nonisolated struct OutputContractRepairEvidence: Codable, Sendable, Equatable {
    let schemaVersion: String
    let repairAttemptId: String
    let runId: String
    let stageExecutionId: String
    let agentExecutionId: String
    let sessionGenerationId: String
    let role: String
    let providerFamily: OutputContractProviderFamily
    let adapterFamily: OutputContractAdapterFamily
    let requiredOutputMode: String
    let initialFailureClass: InitialFailureClass
    let initialFailureSubtype: String?
    let status: OutputContractRepairStatus
    let presentationCategory: PresentationCategory
    let recommendedNextAction: RecommendedNextAction?
    let requiredOutputs: [RequiredOutputBinding]
    let sameSessionRepair: OutputContractRepairAttempt?
    let transcriptRecovery: OutputContractTranscriptRecovery?
    let providerFallback: OutputContractProviderFallback?
    let providerPlanEvidence: OutputContractPlanEvidence?
    let permissionDecisions: [OutputContractPermissionDecision]
    let budget: OutputContractBudget?
    let lease: OutputContractLease?
    let policyFeatureFlags: [String]
    let repairPromptTemplateVersion: String?
    let finalOutputSettlement: String?
    let evidenceArtifactPath: String?
    /// Monotonic content version; participates in Equatable but NOT in Identifiable.id.
    let evidenceVersion: Int
    let projectionIntegrity: String
    let projectionStaleSince: String?
    let createdAt: String
    let updatedAt: String

    enum CodingKeys: String, CodingKey {
        case schemaVersion = "schema_version"
        case repairAttemptId = "repair_attempt_id"
        case runId = "run_id"
        case stageExecutionId = "stage_execution_id"
        case agentExecutionId = "agent_execution_id"
        case sessionGenerationId = "session_generation_id"
        case role
        case providerFamily = "provider_family"
        case adapterFamily = "adapter_family"
        case requiredOutputMode = "required_output_mode"
        case initialFailureClass = "initial_failure_class"
        case initialFailureSubtype = "initial_failure_subtype"
        case status
        case presentationCategory = "presentation_category"
        case recommendedNextAction = "recommended_next_action"
        case requiredOutputs = "required_outputs"
        case sameSessionRepair = "same_session_repair"
        case transcriptRecovery = "transcript_recovery"
        case providerFallback = "provider_fallback"
        case providerPlanEvidence = "provider_plan_evidence"
        case permissionDecisions = "permission_decisions"
        case budget
        case lease
        case policyFeatureFlags = "policy_feature_flags"
        case repairPromptTemplateVersion = "repair_prompt_template_version"
        case finalOutputSettlement = "final_output_settlement"
        case evidenceArtifactPath = "evidence_artifact_path"
        case evidenceVersion = "evidence_version"
        case projectionIntegrity = "projection_integrity"
        case projectionStaleSince = "projection_stale_since"
        case createdAt = "created_at"
        case updatedAt = "updated_at"
    }
}

// MARK: - Identifiable conformance (APPLE-R3-001)

extension OutputContractRepairEvidence: Identifiable {
    /// Stable SwiftUI row identity: (repairAttemptId, agentExecutionId).
    /// evidenceVersion deliberately excluded — it is a content-version field.
    nonisolated var id: String {
        "\(repairAttemptId):\(agentExecutionId)"
    }
}

// MARK: - Presentation status (macos-r3-001)

extension OutputContractRepairStatus {
    /// Normative projection from status to presentation category.
    /// For unknownDiagnostic, the raw string is propagated to the category for inspector forensics.
    var presentationCategory: PresentationCategory {
        switch self {
        case .notAttempted: return .informational
        case .inProgress: return .informational
        case .recovered: return .recovered
        case .blocked: return .blocked
        case .skipped: return .skipped
        case .cancelled: return .cancelled
        case .failed: return .failed
        case .unknownDiagnostic(let raw): return .unknownDiagnostic(raw)
        }
    }
}

// MARK: - Container decoding helper (optional parent field)

/// Decodes an optional OutputContractRepairEvidence from a keyed container.
/// Returns nil for pre-P079 runs and feature-disabled runs without throwing.
extension KeyedDecodingContainer {
    func decodeOutputContractRepairEvidenceIfPresent(
        forKey key: Key
    ) throws -> OutputContractRepairEvidence? {
        guard contains(key) else { return nil }
        if try decodeNil(forKey: key) { return nil }
        return try decode(OutputContractRepairEvidence.self, forKey: key)
    }
}
