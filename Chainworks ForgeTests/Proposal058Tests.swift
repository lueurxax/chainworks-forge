import Foundation
import AppKit
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

        // A claim-start ledger with status=active and triggerRaw=nil is NOT an active
        // escalation (P058: server is the authority; no trigger_raw means no escalation fired).
        let chains = [makeChain(id: "l1", status: "active", pauseReason: nil)]
        adapter.applyChains(chains)

        #expect(adapter.snapshot.runId == "run-wb")
        #expect(adapter.snapshot.activeChains.count == 1)
        #expect(!adapter.snapshot.hasActiveEscalation)
    }

    @Test("EscalationSnapshot.build claim-start ledger with null triggerRaw is not active escalation")
    func snapshotBuildClaimStartIsNotActiveEscalation() {
        // A chain with status=active but no trigger is a claim-start ledger, not an escalation.
        let chains = [makeChain(id: "l1", status: "active", pauseReason: nil)]
        let snap = EscalationSnapshot.build(runId: "run-cs", chains: chains)
        #expect(!snap.hasActiveEscalation)
        #expect(snap.activeChains.count == 1)
    }

    @Test("EscalationSnapshot.build triggered chain sets hasActiveEscalation")
    func snapshotBuildTriggeredChainSetsActiveEscalation() {
        let chain = EscalationChainStateDTO(
            id: "l1", runId: "run-t", stageId: "state_3", agentId: "code_writer",
            policyId: "policy-test", policyHash: "sha256:test",
            statusRaw: "active", currentTierId: "primary_retry",
            currentTierKindRaw: "same_backend_retry", chainAttemptIndex: 1,
            triggerRaw: "repeated_same_blocker_digest",
            pauseReasonRaw: nil, operatorActionHint: nil, runbookAnchor: nil,
            createdAt: "2026-05-07T00:00:00Z", updatedAt: "2026-05-07T00:00:00Z"
        )
        let snap = EscalationSnapshot.build(runId: "run-t", chains: [chain])
        #expect(snap.hasActiveEscalation)
    }

    @Test("EscalationReadAdapter reset clears snapshot")
    func adapterResetClearsSnapshot() {
        let adapter = EscalationReadAdapter(runId: "run-reset")
        adapter.applyChains([makeChain(id: "l1", status: "active", pauseReason: nil)])
        adapter.reset()
        #expect(adapter.snapshot == .empty)
    }

    @Test("EscalationReadAdapter publishes read pipeline stale and disconnected states")
    func adapterPublishesReadPipelineStates() {
        let adapter = EscalationReadAdapter(runId: "run-pipeline")
        adapter.applyChains([makeChain(id: "l1", status: "paused", pauseReason: EscalationPauseReasonCode.escalationChainExhausted.rawValue)])
        #expect(adapter.snapshot.readPipelineState == .ready)
        #expect(adapter.lastOperatorNotice?.requiresUserAttention == true)
        #expect(adapter.dockBadgeEscalationCount == 1)

        adapter.markStaleSnapshot()
        #expect(adapter.snapshot.readPipelineState == .stale)
        adapter.markTransportDisconnected()
        #expect(adapter.snapshot.readPipelineState == .transportDisconnected)
        adapter.markDecodeFailed()
        #expect(adapter.snapshot.readPipelineState == .decodeFailed)
    }

    @Test("EscalationReadAdapter builds runbook URLs without mutating escalation state")
    func adapterBuildsRunbookURL() {
        let adapter = EscalationReadAdapter(runId: "run-runbook")
        let url = adapter.runbookURL(for: "escalation/chain-exhausted")
        #expect(url.path.hasSuffix("docs/runbooks/escalation/chain-exhausted.md"))
    }

    @Test("EscalationReadAdapterRegistry returns same instance for same runId")
    func registryReturnsSameInstance() {
        let registry = EscalationReadAdapterRegistry.shared
        let a1 = registry.adapter(for: "run-registry-test")
        let a2 = registry.adapter(for: "run-registry-test")
        #expect(a1 === a2)
        registry.removeAdapter(for: "run-registry-test")
    }

    // MARK: - Governed macOS read surface

    @Test("EscalationPresentationStyle keeps raw ids in accessibility summary")
    func presentationStyleAccessibilitySummaryIncludesRawIds() {
        let chain = EscalationChainStateDTO(
            id: "ledger-accessibility-1",
            runId: "run-ui",
            stageId: "state_3",
            agentId: "code_writer",
            policyId: "policy-ui",
            policyHash: "sha256:ui",
            statusRaw: "paused",
            currentTierId: "lead_review",
            currentTierKindRaw: "lead_mediation",
            chainAttemptIndex: 2,
            triggerRaw: "contract_output_failure",
            pauseReasonRaw: EscalationPauseReasonCode.escalationPolicyDrift.rawValue,
            operatorActionHint: "Review policy drift externally.",
            runbookAnchor: "escalation/policy-drift",
            createdAt: "2026-05-07T00:00:00Z",
            updatedAt: "2026-05-07T00:00:00Z"
        )
        let snapshot = EscalationSnapshot.build(runId: "run-ui", chains: [chain])
        let summary = EscalationPresentationStyle.accessibilitySummary(for: snapshot)

        #expect(summary.contains("Policy drift"))
        #expect(summary.contains("lead_review"))
        #expect(summary.contains("contract_output_failure"))
        #expect(summary.contains("policy-ui"))
        #expect(summary.contains("ledger-accessibility-1"))
    }

    @Test("P058 governed SwiftUI components are constructible from adapter snapshot")
    func governedMacOSReadSurfaceComponentsConstruct() {
        let chain = EscalationChainStateDTO(
            id: "ledger-components-1",
            runId: "run-ui",
            stageId: "state_3",
            agentId: "code_writer",
            policyId: "policy-ui",
            policyHash: "sha256:ui",
            statusRaw: "paused",
            currentTierId: "human_pause",
            currentTierKindRaw: "pause",
            chainAttemptIndex: 4,
            triggerRaw: "contract_output_failure",
            pauseReasonRaw: EscalationPauseReasonCode.escalationChainExhausted.rawValue,
            operatorActionHint: "Open runbook before resuming.",
            runbookAnchor: "escalation/chain-exhausted",
            createdAt: "2026-05-07T00:00:00Z",
            updatedAt: "2026-05-07T00:00:00Z"
        )
        let snapshot = EscalationSnapshot.build(runId: "run-ui", chains: [chain])

        _ = EscalationStatusCapsule(snapshot: snapshot, density: .standard)
        _ = EscalationBannerStack(snapshot: snapshot)
        _ = EscalationLineageView(snapshot: snapshot)
        _ = EscalationPauseCard(chain: chain)
        _ = EscalationTraceTimeline(traceJSONRedacted: #"{"schema_version":"p058_escalation_trace_redacted_v1","events":[]}"#)
        _ = DriftReviewSheet(
            frozenPolicyHash: "sha256:frozen",
            currentPolicyHash: "sha256:current",
            acknowledgementCommand: "agents.escalation_drift_ack run-ui",
            onClose: {}
        )
        _ = EscalationInspector(snapshot: snapshot, traceJSONRedacted: nil)
    }

    @Test("P031 run detail query includes P058 escalation readback")
    func p031RunDetailQueryIncludesEscalationReadback() {
        let query = P031GraphQLDocuments.runDetail
        #expect(query.contains("runEscalationReadback(runId: $runId)"))
        #expect(query.contains("escalationTraceJsonRedacted"))
        #expect(query.contains("featureFlagState"))
    }

    @Test("P031 presenter maps P058 readback into system-tab escalation snapshot")
    func p031PresenterMapsEscalationReadback() {
        let chain = EscalationChainStateDTO(
            id: "ledger-p031",
            runId: "run-p031",
            stageId: "state_3",
            agentId: "code_writer",
            policyId: "policy-p031",
            policyHash: "sha256:p031",
            statusRaw: "paused",
            currentTierId: "human_pause",
            currentTierKindRaw: "pause",
            chainAttemptIndex: 2,
            triggerRaw: "contract_output_failure",
            pauseReasonRaw: EscalationPauseReasonCode.providerSessionForceDetached.rawValue,
            operatorActionHint: "Open runbook before action.",
            runbookAnchor: "escalation/provider-session-force-detached",
            escalationTraceJSONRedacted: #"{"schema_version":"p058_escalation_trace_redacted_v1","events":[]}"#,
            createdAt: "2026-05-07T00:00:00Z",
            updatedAt: "2026-05-07T00:01:00Z"
        )
        let detail = P031RunDetailReadModel(
            run: nil,
            stages: [],
            artifacts: [],
            runEscalationReadback: P058EscalationRunReadbackReadModel(
                runID: "run-p031",
                chains: [chain],
                chainsTruncated: false,
                chainsTotal: 1,
                pausedChainCount: 1,
                hasActiveEscalation: true,
                dominantPauseReasonRaw: EscalationPauseReasonCode.providerSessionForceDetached.rawValue
            )
        )

        let presentation = P031RunDetailPresenter.presentation(
            for: detail,
            currentFreshness: P031FreshnessSnapshot(state: .live),
            checkedAt: Date(timeIntervalSince1970: 0)
        )

        #expect(presentation.escalationSnapshot?.runId == "run-p031")
        #expect(presentation.escalationSnapshot?.pausedChainCount == 1)
        #expect(presentation.escalationTraceJSONRedacted?.contains("p058_escalation_trace_redacted_v1") == true)
    }

    @Test("Escalation trace copy writes string and public.json atomically")
    func escalationTraceCopyWritesStringAndJSON() throws {
        let pasteboardName = NSPasteboard.Name("p058.trace.copy.\(UUID().uuidString)")
        let pasteboard = NSPasteboard(name: pasteboardName)
        let trace = #"{"schema_version":"p058_escalation_trace_redacted_v1","events":[{"event_kind_raw":"escalation.tier_selected"}]}"#

        EscalationTracePasteboardWriter.copy(redactedTraceJSON: trace, pasteboard: pasteboard)

        #expect(pasteboard.string(forType: NSPasteboard.PasteboardType.string) == trace)
        let jsonData = try #require(pasteboard.data(forType: NSPasteboard.PasteboardType("public.json")))
        #expect(String(decoding: jsonData, as: UTF8.self) == trace)
        pasteboard.clearContents()
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
