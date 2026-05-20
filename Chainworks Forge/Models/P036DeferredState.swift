import Foundation

#if canImport(SwiftUI)
import SwiftUI
#endif

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
    
    #if canImport(SwiftUI)
    var tint: Color {
        switch self {
        case .unauthorized, .conflict, .duplicate: return .red
        case .stale, .projectionLag, .unavailable, .unsupported, .redacted, .alreadyResolved: return .orange
        }
    }
    #endif

    nonisolated init?(from affordance: P085ApprovalAffordanceState) {
        // P085 state mapping: FRESHNESS takes precedence
        switch affordance.freshnessState {
        case .projectionLag: self = .projectionLag; return
        case .stale: self = .stale; return
        case .unauthorized: self = .unauthorized; return
        case .unavailable: self = .unavailable; return
        default: break
        }
        
        // Check both approve and reject availability so reject-only disabled states
        // (e.g. redacted, conflict) also produce an explicit deferred state.
        for availability in [affordance.approveAvailability, affordance.rejectAvailability] {
            if case .disabled(let reasonCode, _) = availability, let code = reasonCode {
                switch code {
                case .unauthorized: self = .unauthorized; return
                case .staleRead: self = .stale; return
                case .projectionLag: self = .projectionLag; return
                case .managedOutsideUI, .unsupportedAction, .ambiguousApprovalIdentity: self = .unsupported; return
                case .redacted: self = .redacted; return
                case .conflict: self = .conflict; return
                case .duplicate: self = .duplicate; return
                case .alreadyResolved: self = .alreadyResolved; return
                case .writePathNotAvailable: self = .unavailable; return
                }
            }
        }
        return nil
    }
}

// MARK: - P036 UI-side metric event counters

/// Accumulates P036 operational metric counts from UI event sites.
/// Counters are in-memory and process-scoped; they are not persisted between app launches.
/// MetricsCollector reads from run loopCounters for engine-side metrics; these complement
/// the UI-side event sites that cannot write directly to per-run loopCounters.
@MainActor
final class P036UICounters {
    static let shared = P036UICounters()

    private(set) var tabRouteResolutionTotal: Int = 0
    private(set) var inlineApprovalRenderTotal: Int = 0
    private(set) var timelineBatchFlushTotal: Int = 0
    private(set) var artifactPayloadStateTotal: Int = 0
    private(set) var projectionGapDeferredTotal: Int = 0
    private(set) var attentionIndicatorTotal: Int = 0

    func recordTabRouteResolution(source: String, target: String, result: String) {
        tabRouteResolutionTotal += 1
    }

    func recordInlineApprovalRender(count: Int, freshnessState: String, actionabilityState: String) {
        inlineApprovalRenderTotal += count
    }

    func recordTimelineBatchFlush(entryCount: Int, reduceMotion: Bool) {
        timelineBatchFlushTotal += 1
    }

    func recordArtifactPayloadState(count: Int, payloadAvailabilityState: String, renderKind: String) {
        artifactPayloadStateTotal += count
    }

    func recordProjectionGapDeferred(count: Int, surface: String, gapClass: String) {
        projectionGapDeferredTotal += count
    }

    func recordAttentionIndicatorRecompute(attentionKind: String, countBucket: String, freshnessState: String) {
        attentionIndicatorTotal += 1
    }

    func reset() {
        tabRouteResolutionTotal = 0
        inlineApprovalRenderTotal = 0
        timelineBatchFlushTotal = 0
        artifactPayloadStateTotal = 0
        projectionGapDeferredTotal = 0
        attentionIndicatorTotal = 0
    }
}
