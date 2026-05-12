import Foundation

nonisolated enum AgentCanonicalOutcome: String, Codable, Sendable, Equatable {
    case completed = "completed"
    case completedWithTransportError = "completed_with_transport_error"
    case failedBeforeOutput = "failed_before_output"
    case failedAfterOutputValidation = "failed_after_output_validation"
    case timedOutBeforeOutput = "timed_out_before_output"
    case timedOutAfterOutput = "timed_out_after_output"
    case cancelledBeforeOutput = "cancelled_before_output"
    case cancelledAfterOutput = "cancelled_after_output"
    case limitExhaustedBeforeOutput = "limit_exhausted_before_output"
    case limitExhaustedAfterOutput = "limit_exhausted_after_output"
}

nonisolated enum TransportErrorKind: String, Codable, Sendable, Equatable {
    case timeout = "timeout"
    case stream = "stream"
    case provider = "provider"
    case unknown = "unknown"
}

nonisolated enum SupervisionClassification: String, Codable, Sendable, Equatable {
    case idleHangBeforeFirstProgress = "idle_hang_before_first_progress"
    case idleHangAfterProgress = "idle_hang_after_progress"
    case idleHangReadLoop = "idle_hang_read_loop"
    case idleHangAfterFirstEdit = "idle_hang_after_first_edit"
    case waitingOnPermissionRoundtrip = "waiting_on_permission_roundtrip"
    case providerActiveWithoutTerminalResponse = "provider_active_without_terminal_response"
    case mutationSideEffectMissing = "mutation_side_effect_missing"

    var defaultSummary: String {
        switch self {
        case .idleHangBeforeFirstProgress:
            return "Execution stalled before first meaningful progress after prompt submission"
        case .idleHangAfterProgress:
            return "Execution stalled after meaningful progress stopped"
        case .idleHangReadLoop:
            return "Execution stalled in a weak read loop without strong progress"
        case .idleHangAfterFirstEdit:
            return "Execution stalled after the first edit boundary"
        case .waitingOnPermissionRoundtrip:
            return "Execution stalled while waiting for a permission round-trip to settle"
        case .providerActiveWithoutTerminalResponse:
            return "Execution stayed active and emitted progress, but never produced a terminal response"
        case .mutationSideEffectMissing:
            return "Mutating tool reported success, but no filesystem side effect was observed"
        }
    }
}

nonisolated enum OutputPresence: String, Codable, Sendable, Equatable {
    case none = "none"
    case durableOutput = "durable_output"
}

nonisolated enum StageSettlementKind: String, Codable, Sendable, Equatable {
    case completed = "completed"
    case blocked = "blocked"
    case failed = "failed"
    case repaired = "repaired"
    case superseded = "superseded"
}

nonisolated struct OutcomeEnvelope: Codable, Sendable, Equatable {
    let canonicalOutcome: AgentCanonicalOutcome?
    let transportErrorKind: TransportErrorKind?
    let providerStopReason: String?
    let outputPresence: OutputPresence
    let rawErrorMessage: String?
    let rawFinishEvent: String?
}

nonisolated struct SideEffectReadbackSummary: Codable, Sendable, Equatable {
    let schemaVersion: String
    let runID: String
    let unresolvedCount: Int
    let blocked: Bool
    let readbackSource: String
    let effects: [SideEffectReadbackItem]

    enum CodingKeys: String, CodingKey {
        case schemaVersion = "schema_version"
        case runID = "run_id"
        case unresolvedCount = "unresolved_count"
        case blocked
        case readbackSource = "readback_source"
        case effects
    }
}

nonisolated struct SideEffectReadbackItem: Codable, Sendable, Equatable {
    let id: String
    let runID: String
    let stageExecutionID: String
    let agentExecutionID: String?
    let effectKind: String
    let status: String
    let targetKey: String
    let externalWriteAttempted: Bool
    let evidenceRoot: String?
    let readbackSource: String
    let reportPath: String?
    let blockedReason: String
    let operatorNextAction: String
    let recommendedMCPTool: String
    let retryForbidden: Bool
    let lastErrorKind: String?
    let updatedAt: String

    enum CodingKeys: String, CodingKey {
        case id
        case runID = "run_id"
        case stageExecutionID = "stage_execution_id"
        case agentExecutionID = "agent_execution_id"
        case effectKind = "effect_kind"
        case status
        case targetKey = "target_key"
        case externalWriteAttempted = "external_write_attempted"
        case evidenceRoot = "evidence_root"
        case readbackSource = "readback_source"
        case reportPath = "report_path"
        case blockedReason = "blocked_reason"
        case operatorNextAction = "operator_next_action"
        case recommendedMCPTool = "recommended_mcp_tool"
        case retryForbidden = "retry_forbidden"
        case lastErrorKind = "last_error_kind"
        case updatedAt = "updated_at"
    }
}
