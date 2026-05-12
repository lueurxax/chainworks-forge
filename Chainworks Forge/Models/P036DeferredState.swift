import Foundation

/// P036 shared deferred-state types cover stale, projection-lag, unauthorized, redacted, 
/// unavailable, unsupported, conflict, duplicate, and already-resolved states.
enum P036DeferredState: String, Codable, CaseIterable {
    case stale
    case projectionLag = "projection_lag"
    case unauthorized
    case redacted
    case unavailable
    case unsupported
    case conflict
    case duplicate
    case alreadyResolved = "already_resolved"
    
    var displayLabel: String {
        switch self {
        case .stale: return "Stale"
        case .projectionLag: return "Projection Lag"
        case .unauthorized: return "Unauthorized"
        case .redacted: return "Redacted"
        case .unavailable: return "Unavailable"
        case .unsupported: return "Unsupported"
        case .conflict: return "Conflict"
        case .duplicate: return "Duplicate"
        case .alreadyResolved: return "Already Resolved"
        }
    }
}
