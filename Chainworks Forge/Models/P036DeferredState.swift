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
                case .approvalNotActionable: self = .alreadyResolved; return
                case .observerScope: self = .unauthorized; return
                case .nonApprovalMutation, .capabilityOutOfScope: self = .unsupported; return
                case .writePathNotAvailable: self = .unavailable; return
                }
            }
        }
        return nil
    }
}

// MARK: - P036 UI-side metric event counters

struct P036OperatorTaskAttemptSample: Codable, Hashable, Sendable {
    let taskID: String
    let result: String
    let blockedReason: String?
    let count: Int
}

enum P036UICounterStore {
    nonisolated static let tabRouteResolutionTotal = "p036_tab_route_resolution_total"
    nonisolated static let inlineApprovalRenderTotal = "p036_inline_approval_render_total"
    nonisolated static let timelineBatchFlushTotal = "p036_timeline_batch_flush_total"
    nonisolated static let timelineCardCollapseTotal = "p036_timeline_card_collapse_total"
    nonisolated static let artifactPayloadStateTotal = "p036_artifact_payload_state_total"
    nonisolated static let projectionGapDeferredTotal = "p036_projection_gap_deferred_total"
    nonisolated static let attentionIndicatorTotal = "p036_global_attention_indicator_total"
    nonisolated static let operatorTaskAttemptTotal = "p036_operator_task_attempt_total"

    nonisolated private static let prefix = "chainworks.uiCounter."
    nonisolated private static let operatorTaskAttemptSamplesKey = prefix + "p036_operator_task_attempt_total.samples"

    nonisolated private static var allKeys: [String] {
        [
            tabRouteResolutionTotal,
            inlineApprovalRenderTotal,
            timelineBatchFlushTotal,
            timelineCardCollapseTotal,
            artifactPayloadStateTotal,
            projectionGapDeferredTotal,
            attentionIndicatorTotal,
            operatorTaskAttemptTotal
        ]
    }

    nonisolated static func value(for metric: String) -> Int {
        UserDefaults.standard.integer(forKey: prefix + metric)
    }

    nonisolated static func increment(_ metric: String, by count: Int = 1) {
        let key = prefix + metric
        UserDefaults.standard.set(UserDefaults.standard.integer(forKey: key) + count, forKey: key)
    }

    nonisolated static func recordOperatorTaskAttempt(taskID: String, result: String, blockedReason: String?) {
        increment(operatorTaskAttemptTotal)
        let sample = P036OperatorTaskAttemptSample(
            taskID: normalized(taskID, fallback: "unknown_task"),
            result: normalized(result, fallback: "unknown_result"),
            blockedReason: blockedReason.flatMap { normalizedOptional($0) },
            count: 1
        )
        var samples = operatorTaskAttemptSamples()
        if let index = samples.firstIndex(where: {
            $0.taskID == sample.taskID
                && $0.result == sample.result
                && $0.blockedReason == sample.blockedReason
        }) {
            let current = samples[index]
            samples[index] = P036OperatorTaskAttemptSample(
                taskID: current.taskID,
                result: current.result,
                blockedReason: current.blockedReason,
                count: current.count + 1
            )
        } else {
            samples.append(sample)
        }
        persistOperatorTaskAttemptSamples(samples)
    }

    nonisolated static func operatorTaskAttemptSamples() -> [P036OperatorTaskAttemptSample] {
        guard let data = UserDefaults.standard.data(forKey: operatorTaskAttemptSamplesKey),
              let samples = try? JSONDecoder().decode([P036OperatorTaskAttemptSample].self, from: data)
        else {
            return []
        }
        return samples.sorted { lhs, rhs in
            if lhs.taskID != rhs.taskID { return lhs.taskID < rhs.taskID }
            if lhs.result != rhs.result { return lhs.result < rhs.result }
            return (lhs.blockedReason ?? "") < (rhs.blockedReason ?? "")
        }
    }

    nonisolated static func resetAll() {
        for metric in allKeys {
            UserDefaults.standard.removeObject(forKey: prefix + metric)
        }
        UserDefaults.standard.removeObject(forKey: operatorTaskAttemptSamplesKey)
    }

    nonisolated private static func persistOperatorTaskAttemptSamples(_ samples: [P036OperatorTaskAttemptSample]) {
        guard let data = try? JSONEncoder().encode(samples) else { return }
        UserDefaults.standard.set(data, forKey: operatorTaskAttemptSamplesKey)
    }

    nonisolated private static func normalized(_ value: String, fallback: String) -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? fallback : trimmed
    }

    nonisolated private static func normalizedOptional(_ value: String) -> String? {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }
}

/// Accumulates P036 operational metric counts from UI event sites.
/// Counters are mirrored into UserDefaults so MetricsCollector can read UI event totals
/// even when the thin GraphQL UI does not own a mutable SwiftData Run row.
@MainActor
final class P036UICounters {
    static let shared = P036UICounters()

    private(set) var tabRouteResolutionTotal: Int = P036UICounterStore.value(for: P036UICounterStore.tabRouteResolutionTotal)
    private(set) var inlineApprovalRenderTotal: Int = P036UICounterStore.value(for: P036UICounterStore.inlineApprovalRenderTotal)
    private(set) var timelineBatchFlushTotal: Int = P036UICounterStore.value(for: P036UICounterStore.timelineBatchFlushTotal)
    private(set) var artifactPayloadStateTotal: Int = P036UICounterStore.value(for: P036UICounterStore.artifactPayloadStateTotal)
    private(set) var projectionGapDeferredTotal: Int = P036UICounterStore.value(for: P036UICounterStore.projectionGapDeferredTotal)
    private(set) var attentionIndicatorTotal: Int = P036UICounterStore.value(for: P036UICounterStore.attentionIndicatorTotal)
    private(set) var timelineCardCollapseTotal: Int = P036UICounterStore.value(for: P036UICounterStore.timelineCardCollapseTotal)
    private(set) var operatorTaskAttemptTotal: Int = P036UICounterStore.value(for: P036UICounterStore.operatorTaskAttemptTotal)

    func recordTabRouteResolution(source: String, target: String, result: String) {
        tabRouteResolutionTotal += 1
        P036UICounterStore.increment(P036UICounterStore.tabRouteResolutionTotal)
    }

    func recordInlineApprovalRender(count: Int, freshnessState: String, actionabilityState: String) {
        inlineApprovalRenderTotal += count
        P036UICounterStore.increment(P036UICounterStore.inlineApprovalRenderTotal, by: count)
    }

    func recordTimelineBatchFlush(entryCount: Int, reduceMotion: Bool) {
        timelineBatchFlushTotal += 1
        P036UICounterStore.increment(P036UICounterStore.timelineBatchFlushTotal)
    }

    func recordTimelineCardCollapse(count: Int, reason: String) {
        timelineCardCollapseTotal += count
        P036UICounterStore.increment(P036UICounterStore.timelineCardCollapseTotal, by: count)
    }

    func recordArtifactPayloadState(count: Int, payloadAvailabilityState: String, renderKind: String) {
        artifactPayloadStateTotal += count
        P036UICounterStore.increment(P036UICounterStore.artifactPayloadStateTotal, by: count)
    }

    func recordProjectionGapDeferred(count: Int, surface: String, gapClass: String) {
        projectionGapDeferredTotal += count
        P036UICounterStore.increment(P036UICounterStore.projectionGapDeferredTotal, by: count)
    }

    func recordAttentionIndicatorRecompute(attentionKind: String, countBucket: String, freshnessState: String) {
        attentionIndicatorTotal += 1
        P036UICounterStore.increment(P036UICounterStore.attentionIndicatorTotal)
    }

    func recordOperatorTaskAttempt(taskID: String, result: String, blockedReason: String?) {
        operatorTaskAttemptTotal += 1
        P036UICounterStore.recordOperatorTaskAttempt(taskID: taskID, result: result, blockedReason: blockedReason)
    }

    func reset() {
        P036UICounterStore.resetAll()
        tabRouteResolutionTotal = 0
        inlineApprovalRenderTotal = 0
        timelineBatchFlushTotal = 0
        artifactPayloadStateTotal = 0
        projectionGapDeferredTotal = 0
        attentionIndicatorTotal = 0
        timelineCardCollapseTotal = 0
        operatorTaskAttemptTotal = 0
    }
}
