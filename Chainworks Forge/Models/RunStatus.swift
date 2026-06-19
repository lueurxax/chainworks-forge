import Foundation

/// Canonical run status vocabulary shared by the operator UI and the control-plane GraphQL
/// read boundary. Decoupled from any persistence model — the live app reads runs from the
/// daemon, so this is a pure value type.
enum RunStatus: String, Codable, Equatable {
    case pending
    case ready
    case running
    case waitingApproval
    case blocked
    case completed
    case failed
    case cancelled
    case cancelling

    /// Parses a server-emitted status string, handling both Swift camelCase (legacy) and
    /// Rust snake_case (control-plane GraphQL responses, e.g. "waiting_approval").
    nonisolated static func from(serverValue: String) -> RunStatus? {
        if let status = RunStatus(rawValue: serverValue) { return status }
        switch serverValue {
        case "waiting_approval": return .waitingApproval
        default: return nil
        }
    }
}
