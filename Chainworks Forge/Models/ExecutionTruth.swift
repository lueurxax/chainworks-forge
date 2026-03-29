import Foundation

enum AgentCanonicalOutcome: String, Codable, Sendable, Equatable {
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

enum TransportErrorKind: String, Codable, Sendable, Equatable {
    case timeout = "timeout"
    case stream = "stream"
    case provider = "provider"
    case unknown = "unknown"
}

enum OutputPresence: String, Codable, Sendable, Equatable {
    case none = "none"
    case durableOutput = "durable_output"
}

enum StageSettlementKind: String, Codable, Sendable, Equatable {
    case completed = "completed"
    case blocked = "blocked"
    case failed = "failed"
    case repaired = "repaired"
    case superseded = "superseded"
}

struct OutcomeEnvelope: Codable, Sendable, Equatable {
    let canonicalOutcome: AgentCanonicalOutcome?
    let transportErrorKind: TransportErrorKind?
    let providerStopReason: String?
    let outputPresence: OutputPresence
    let rawErrorMessage: String?
    let rawFinishEvent: String?
}
