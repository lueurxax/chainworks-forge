import Foundation
import Testing
@testable import Chainworks_Forge

@MainActor
@Suite("Proposal 058 escalation readback", .tags(.fast))
struct Proposal058Tests {
    @Test("Elapsed deadline pause decodes without losing operator recovery context")
    func elapsedDeadlinePauseDecodes() throws {
        let payload = """
        {
          "id": "ledger-1",
          "runId": "run-1",
          "stageId": "state_5_proposal_refined",
          "agentId": "proposal_writer",
          "policyId": "proposal_writer_escalation",
          "policyHash": "sha256:policy",
          "statusRaw": "paused",
          "currentTierId": "gemini_reasoning_fallback",
          "currentTierKindRaw": "backend_profile",
          "chainAttemptIndex": 2,
          "triggerRaw": "provider_capacity_exhausted",
          "pauseReasonRaw": "escalation_deadline_elapsed",
          "operatorActionHint": "An operator may open a new deadline window.",
          "runbookAnchor": "escalation/deadline-elapsed",
          "createdAt": "2026-08-09T06:32:55Z",
          "updatedAt": "2026-08-09T18:45:53Z"
        }
        """

        let chain = try JSONDecoder().decode(
            EscalationChainStateDTO.self,
            from: Data(payload.utf8)
        )

        #expect(chain.statusRaw == "paused")
        #expect(chain.pauseReasonRaw == EscalationPauseReasonCode.escalationDeadlineElapsed.rawValue)
        #expect(chain.currentTierId == "gemini_reasoning_fallback")
        #expect(chain.currentTierKindRaw == EscalationTierKindCode.backendProfile.rawValue)
        #expect(chain.chainAttemptIndex == 2)
        #expect(chain.createdAt == "2026-08-09T06:32:55Z")
    }

    @Test("Elapsed deadline remains paused in the passive read model")
    func elapsedDeadlineDoesNotAppearAutomaticallyResumed() {
        let snapshot = EscalationSnapshot.build(
            runId: "run-1",
            chains: [makeDeadlinePausedChain()]
        )

        #expect(snapshot.pausedChainCount == 1)
        #expect(snapshot.pauseReasonRaw == EscalationPauseReasonCode.escalationDeadlineElapsed.rawValue)
        #expect(snapshot.hasActiveEscalation)
        #expect(EscalationPresentationStyle.stateLabel(for: snapshot) == "Paused")
    }

    @Test("Adapter publishes deadline pause and frozen fallback tier")
    func adapterPublishesDeadlinePause() {
        let adapter = EscalationReadAdapter(runId: "run-1")

        adapter.applyChains([makeDeadlinePausedChain()])

        #expect(adapter.snapshot.readPipelineState == .ready)
        #expect(adapter.snapshot.activeChains.first?.currentTierId == "gemini_reasoning_fallback")
        #expect(adapter.snapshot.pauseReasonRaw == EscalationPauseReasonCode.escalationDeadlineElapsed.rawValue)
        #expect(adapter.lastOperatorNotice?.requiresUserAttention == true)
        #expect(adapter.dockBadgeEscalationCount == 1)
    }

    @Test("Detailed status presentation exposes paused tier and trigger")
    func detailedStatusPresentationIncludesRecoveryContext() throws {
        let snapshot = EscalationSnapshot.build(
            runId: "run-1",
            chains: [makeDeadlinePausedChain()]
        )

        let presentation = EscalationStatusCapsulePresentation.presentation(
            for: snapshot,
            density: .detailed
        )

        #expect(presentation.stateLabel == "Paused")
        #expect(presentation.tierLabel == "gemini_reas...ing_fallback")
        #expect(presentation.triggerLabel == "provider_ca...ty_exhausted")
        #expect(presentation.accessibilityLabel.contains("ledger-1"))
        #expect(presentation.helpText.contains("gemini_reasoning_fallback"))
    }

    @Test("P058 pause reason vocabulary retains elapsed deadline and non-resumable reasons")
    func pauseReasonVocabularyRetainsRecoveryBoundary() {
        let reasons = Set(EscalationPauseReasonCode.allCases.map(\.rawValue))

        #expect(reasons.count == 13)
        #expect(reasons.contains("escalation_deadline_elapsed"))
        #expect(reasons.contains("provider_session_force_detached"))
        #expect(reasons.contains("escalation_policy_drift"))
    }

    private func makeDeadlinePausedChain() -> EscalationChainStateDTO {
        EscalationChainStateDTO(
            id: "ledger-1",
            runId: "run-1",
            stageId: "state_5_proposal_refined",
            agentId: "proposal_writer",
            policyId: "proposal_writer_escalation",
            policyHash: "sha256:policy",
            statusRaw: "paused",
            currentTierId: "gemini_reasoning_fallback",
            currentTierKindRaw: EscalationTierKindCode.backendProfile.rawValue,
            chainAttemptIndex: 2,
            triggerRaw: "provider_capacity_exhausted",
            pauseReasonRaw: EscalationPauseReasonCode.escalationDeadlineElapsed.rawValue,
            operatorActionHint: "An operator may open a new deadline window.",
            runbookAnchor: "escalation/deadline-elapsed",
            createdAt: "2026-08-09T06:32:55Z",
            updatedAt: "2026-08-09T18:45:53Z"
        )
    }
}
