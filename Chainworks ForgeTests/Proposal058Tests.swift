import Foundation
import Testing
@testable import Chainworks_Forge

@MainActor
@Suite("Proposal 058 — Configurable Agent Escalation Chains")
struct Proposal058Tests {

    // MARK: - EscalationChainStateDTO decoding

    @Test("EscalationChainStateDTO decodes known active chain without error")
    func chainStateDTODecodesActiveChain() throws {
        let json = """
        {
          "id": "ledger-001",
          "runId": "run-abc",
          "stageId": "state_3_implementation",
          "agentId": "code_writer",
          "policyId": "code_writer_default_escalation",
          "policyHash": "sha256:abc123",
          "statusRaw": "active",
          "currentTierId": "primary_retry",
          "currentTierKindRaw": "same_backend_retry",
          "chainAttemptIndex": 1,
          "triggerRaw": "repeated_same_blocker_digest",
          "pauseReasonRaw": null,
          "operatorActionHint": null,
          "runbookAnchor": null,
          "createdAt": "2026-05-07T10:00:00Z",
          "updatedAt": "2026-05-07T10:01:00Z"
        }
        """
        let data = Data(json.utf8)
        let dto = try JSONDecoder().decode(EscalationChainStateDTO.self, from: data)
        #expect(dto.id == "ledger-001")
        #expect(dto.statusRaw == "active")
        #expect(dto.currentTierKindRaw == "same_backend_retry")
        #expect(dto.triggerRaw == "repeated_same_blocker_digest")
        #expect(dto.pauseReasonRaw == nil)
        #expect(dto.chainAttemptIndex == 1)
    }

    @Test("EscalationChainStateDTO decodes unknown future trigger without error")
    func chainStateDTODecodesUnknownTrigger() throws {
        let json = """
        {
          "id": "ledger-002",
          "runId": "run-abc",
          "stageId": "state_3",
          "agentId": "code_writer",
          "policyId": "policy-x",
          "policyHash": "sha256:xyz",
          "statusRaw": "paused",
          "currentTierId": null,
          "currentTierKindRaw": null,
          "chainAttemptIndex": 3,
          "triggerRaw": "future_unknown_trigger_v99",
          "pauseReasonRaw": "escalation_chain_exhausted",
          "operatorActionHint": "Extend the chain or accept terminal pause.",
          "runbookAnchor": "escalation/chain-exhausted",
          "createdAt": "2026-05-07T11:00:00Z",
          "updatedAt": "2026-05-07T11:05:00Z"
        }
        """
        let data = Data(json.utf8)
        let dto = try JSONDecoder().decode(EscalationChainStateDTO.self, from: data)
        // Unknown triggers must round-trip unchanged.
        #expect(dto.triggerRaw == "future_unknown_trigger_v99")
        #expect(dto.pauseReasonRaw == "escalation_chain_exhausted")
        #expect(dto.statusRaw == "paused")
    }

    // MARK: - EscalationSnapshot.build

    @Test("EscalationSnapshot.build with empty chains returns empty snapshot")
    func snapshotBuildEmpty() {
        let snap = EscalationSnapshot.build(runId: "run-1", chains: [])
        #expect(snap.activeChains.isEmpty)
        #expect(!snap.hasActiveEscalation)
        #expect(snap.pausedChainCount == 0)
        #expect(snap.pauseReasonRaw == nil)
        #expect(!snap.isKillSwitchEngaged)
        #expect(!snap.isPolicyDrift)
    }

    @Test("EscalationSnapshot.build counts paused chains correctly")
    func snapshotBuildCountsPaused() {
        let chains = [
            makeChain(id: "l1", status: "active", pauseReason: nil),
            makeChain(id: "l2", status: "paused", pauseReason: "escalation_chain_exhausted"),
            makeChain(id: "l3", status: "exhausted", pauseReason: "escalation_deadline_elapsed"),
        ]
        let snap = EscalationSnapshot.build(runId: "run-2", chains: chains)
        #expect(snap.pausedChainCount == 2)
        #expect(snap.hasActiveEscalation)
        // First paused chain's reason is dominant.
        #expect(snap.pauseReasonRaw == "escalation_chain_exhausted")
    }

    @Test("EscalationSnapshot.build detects kill switch engagement")
    func snapshotBuildDetectsKillSwitch() {
        let chains = [
            makeChain(id: "l1", status: "paused", pauseReason: EscalationPauseReasonCode.escalationKillSwitchEngaged.rawValue),
        ]
        let snap = EscalationSnapshot.build(runId: "run-3", chains: chains)
        #expect(snap.isKillSwitchEngaged)
        #expect(!snap.isPolicyDrift)
    }

    @Test("EscalationSnapshot.build detects policy drift")
    func snapshotBuildDetectsPolicyDrift() {
        let chains = [
            makeChain(id: "l1", status: "paused", pauseReason: EscalationPauseReasonCode.escalationPolicyDrift.rawValue),
        ]
        let snap = EscalationSnapshot.build(runId: "run-4", chains: chains)
        #expect(snap.isPolicyDrift)
        #expect(!snap.isKillSwitchEngaged)
    }

    // MARK: - EscalationPauseReasonCode vocabulary coverage

    @Test("EscalationPauseReasonCode covers all 13 pause reasons from proposal catalog")
    func pauseReasonCodeCoverage() {
        let expected: Set<String> = [
            "escalation_policy_unknown_backend_profile",
            "escalation_policy_ambiguous_at_compile",
            "escalation_policy_unsafe_for_side_effect_stage",
            "escalation_policy_disabled",
            "escalation_kill_switch_engaged",
            "escalation_chain_exhausted",
            "capacity_probe_failed",
            "provider_session_force_detached",
            "escalation_recovery_inconsistent",
            "escalation_repeated_digest_no_progress",
            "escalation_deadline_elapsed",
            "human_tier_deadline_elapsed",
            "escalation_policy_drift",
        ]
        let actual = Set(EscalationPauseReasonCode.allCases.map(\.rawValue))
        #expect(actual == expected)
    }

    @Test("EscalationTierKindCode covers all 4 tier kinds from proposal")
    func tierKindCodeCoverage() {
        let expected: Set<String> = [
            "same_backend_retry", "backend_profile", "lead_mediation", "pause",
        ]
        let actual = Set(EscalationTierKindCode.allCases.map(\.rawValue))
        #expect(actual == expected)
    }

    // MARK: - EscalationReadAdapter write boundary

    @Test("EscalationReadAdapter applyChains updates snapshot immutably")
    func adapterApplyChainsUpdatesSnapshot() async {
        let adapter = EscalationReadAdapter(runId: "run-wb")
        #expect(adapter.snapshot == .empty)

        let chains = [makeChain(id: "l1", status: "active", pauseReason: nil)]
        adapter.applyChains(chains)

        #expect(adapter.snapshot.runId == "run-wb")
        #expect(adapter.snapshot.activeChains.count == 1)
        #expect(!adapter.snapshot.hasActiveEscalation == false)
    }

    @Test("EscalationReadAdapter reset clears snapshot")
    func adapterResetClearsSnapshot() {
        let adapter = EscalationReadAdapter(runId: "run-reset")
        adapter.applyChains([makeChain(id: "l1", status: "active", pauseReason: nil)])
        adapter.reset()
        #expect(adapter.snapshot == .empty)
    }

    @Test("EscalationReadAdapterRegistry returns same instance for same runId")
    func registryReturnsSameInstance() {
        let registry = EscalationReadAdapterRegistry.shared
        let a1 = registry.adapter(for: "run-registry-test")
        let a2 = registry.adapter(for: "run-registry-test")
        #expect(a1 === a2)
        registry.removeAdapter(for: "run-registry-test")
    }

    // MARK: - Helpers

    private func makeChain(id: String, status: String, pauseReason: String?) -> EscalationChainStateDTO {
        EscalationChainStateDTO(
            id: id,
            runId: "run-test",
            stageId: "state_3",
            agentId: "code_writer",
            policyId: "policy-test",
            policyHash: "sha256:test",
            statusRaw: status,
            currentTierId: nil,
            currentTierKindRaw: nil,
            chainAttemptIndex: 0,
            triggerRaw: nil,
            pauseReasonRaw: pauseReason,
            operatorActionHint: nil,
            runbookAnchor: nil,
            createdAt: "2026-05-07T00:00:00Z",
            updatedAt: "2026-05-07T00:00:00Z"
        )
    }
}
