import Foundation

// MARK: - Forward-compatible DTOs

/// Forward-compatible DTO for one escalation chain decoded from GraphQL.
/// All enum-like fields use raw String for forward compatibility per proposal wire contract.
struct EscalationChainStateDTO: Codable, Sendable, Equatable {
    let id: String
    let runId: String
    let stageId: String
    let agentId: String
    let policyId: String
    let policyHash: String
    /// Raw status: active | paused | exhausted | cancelled
    let statusRaw: String
    let currentTierId: String?
    /// Raw tier kind: same_backend_retry | backend_profile | lead_mediation | pause
    let currentTierKindRaw: String?
    let chainAttemptIndex: Int
    /// Raw trigger vocabulary — may contain future values.
    let triggerRaw: String?
    /// Raw pause reason code from escalation_policy_v1 catalog.
    let pauseReasonRaw: String?
    let operatorActionHint: String?
    let runbookAnchor: String?
    let createdAt: String
    let updatedAt: String

    enum CodingKeys: String, CodingKey {
        case id
        case runId
        case stageId
        case agentId
        case policyId
        case policyHash
        case statusRaw
        case currentTierId
        case currentTierKindRaw
        case chainAttemptIndex
        case triggerRaw
        case pauseReasonRaw
        case operatorActionHint
        case runbookAnchor
        case createdAt
        case updatedAt
    }
}

// MARK: - Stable vocabulary helpers

/// Stable pause reason codes from escalation_policy_v1.
/// Use .rawValue to match against EscalationChainStateDTO.pauseReasonRaw.
enum EscalationPauseReasonCode: String, Sendable, CaseIterable {
    case escalationPolicyUnknownBackendProfile = "escalation_policy_unknown_backend_profile"
    case escalationPolicyAmbiguousAtCompile = "escalation_policy_ambiguous_at_compile"
    case escalationPolicyUnsafeForSideEffectStage = "escalation_policy_unsafe_for_side_effect_stage"
    case escalationPolicyDisabled = "escalation_policy_disabled"
    case escalationKillSwitchEngaged = "escalation_kill_switch_engaged"
    case escalationChainExhausted = "escalation_chain_exhausted"
    case capacityProbeFailed = "capacity_probe_failed"
    case providerSessionForceDetached = "provider_session_force_detached"
    case escalationRecoveryInconsistent = "escalation_recovery_inconsistent"
    case escalationRepeatedDigestNoProgress = "escalation_repeated_digest_no_progress"
    case escalationDeadlineElapsed = "escalation_deadline_elapsed"
    case humanTierDeadlineElapsed = "human_tier_deadline_elapsed"
    case escalationPolicyDrift = "escalation_policy_drift"
}

/// Stable tier kind codes.
enum EscalationTierKindCode: String, Sendable, CaseIterable {
    case sameBackendRetry = "same_backend_retry"
    case backendProfile = "backend_profile"
    case leadMediation = "lead_mediation"
    case pause = "pause"
}

// MARK: - Presentation snapshot

/// Immutable presentation snapshot produced by EscalationReadAdapter.
/// Consumed by SwiftUI; never mutated after MainActor publication.
struct EscalationSnapshot: Sendable, Equatable {
    let runId: String
    let activeChains: [EscalationChainStateDTO]
    let pauseReasonRaw: String?
    let isKillSwitchEngaged: Bool
    let isPolicyDrift: Bool
    let hasActiveEscalation: Bool
    let pausedChainCount: Int
}

extension EscalationSnapshot {
    static let empty = EscalationSnapshot(
        runId: "",
        activeChains: [],
        pauseReasonRaw: nil,
        isKillSwitchEngaged: false,
        isPolicyDrift: false,
        hasActiveEscalation: false,
        pausedChainCount: 0
    )

    static func build(runId: String, chains: [EscalationChainStateDTO]) -> EscalationSnapshot {
        let pausedChains = chains.filter {
            $0.statusRaw == "paused" || $0.statusRaw == "exhausted"
        }
        let isKillSwitch = chains.contains {
            $0.pauseReasonRaw == EscalationPauseReasonCode.escalationKillSwitchEngaged.rawValue
        }
        let isDrift = chains.contains {
            $0.pauseReasonRaw == EscalationPauseReasonCode.escalationPolicyDrift.rawValue
        }
        return EscalationSnapshot(
            runId: runId,
            activeChains: chains,
            pauseReasonRaw: pausedChains.first?.pauseReasonRaw,
            isKillSwitchEngaged: isKillSwitch,
            isPolicyDrift: isDrift,
            hasActiveEscalation: !chains.isEmpty,
            pausedChainCount: pausedChains.count
        )
    }
}
