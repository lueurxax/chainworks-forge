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
