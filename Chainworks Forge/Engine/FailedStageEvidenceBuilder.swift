import Foundation

// MARK: - Proposal 013 Layer O: Failed Stage Evidence Builder

/// Persists raw output, transcript, receipt, validation errors, and timing
/// evidence even when the stage later fails.
///
/// Key principle (§6.2): Validation must never be the point where all
/// downstream evidence disappears.
struct FailedStageEvidenceBuilder {

    /// Build a complete evidence packet for a failed stage.
    /// This is the canonical reference for recovery UI, reports, and export.
    static func buildEvidencePacket(
        stageExecution: StageExecution,
        failedAgent: AgentExecution?,
        validationFailure: ValidationFailureRecord?,
        outputEnvelopes: [StructuredOutputEnvelope],
        recoverySnapshot: RecoveryActionSnapshot?
    ) -> FailedStageEvidencePacket {
        let stageAgents = stageExecution.agentExecutions
        let rawOutputsExist = (validationFailure?.rawOutputExists == true)
            || outputEnvelopes.contains { $0.rawPayloadPersisted }
            || stageAgents.contains { !$0.artifacts.isEmpty }
        let receiptExists = (validationFailure?.receiptExists == true) || stageAgents.contains {
            $0.providerReceiptJSON != nil || $0.artifacts.contains(where: { $0.name.hasSuffix("_receipt.json") })
        }
        let transcriptExists = (validationFailure?.transcriptExists == true) || stageAgents.contains {
            $0.transcriptPath != nil
                || $0.transcriptArtifactPath != nil
                || $0.artifacts.contains(where: { $0.name.hasSuffix("_transcript.md") })
        }

        let failureSummary: String
        if let vf = validationFailure {
            failureSummary = vf.failureSummary
        } else if let agent = failedAgent,
                  let supervision = agent.supervisionClassification {
            failureSummary = supervision.defaultSummary
        } else if let agent = failedAgent,
                  let envelope = decodeOutcomeEnvelope(from: agent),
                  let rawErrorMessage = envelope.rawErrorMessage?.trimmingCharacters(in: .whitespacesAndNewlines),
                  !rawErrorMessage.isEmpty {
            failureSummary = rawErrorMessage
        } else if let agent = failedAgent, let msg = agent.logSnippet {
            failureSummary = msg
        } else {
            failureSummary = "Stage failed with no detailed failure record"
        }

        let timing = StageTiming(
            stageStartedAt: stageExecution.startedAt,
            stageCompletedAt: stageExecution.completedAt,
            agentStartedAt: failedAgent?.startedAt,
            agentCompletedAt: failedAgent?.completedAt,
            agentDurationSeconds: failedAgent?.completedAt.flatMap { completed in
                completed.timeIntervalSince(failedAgent!.startedAt)
            }
        )

        let failedAgentEnvelope = failedAgent.flatMap(decodeOutcomeEnvelope(from:))

        return FailedStageEvidencePacket(
            id: UUID(),
            timestamp: Date(),
            stageID: stageExecution.stageID,
            stageLabel: stageExecution.label,
            stageAttemptNumber: stageExecution.attemptNumber,
            failedAgentID: failedAgent?.agentID,
            failedAgentTitle: failedAgent?.agentTitle,
            failureSummary: failureSummary,
            failureClass: validationFailure?.failureClass ?? .agentReportedFailure,
            supervisionClassification: failedAgent?.supervisionClassification,
            canonicalOutcome: failedAgent?.canonicalOutcome,
            transportErrorKind: failedAgent?.transportErrorKind ?? failedAgentEnvelope?.transportErrorKind,
            outputPresence: failedAgent?.outputPresence ?? failedAgentEnvelope?.outputPresence,
            rawOutputsExist: rawOutputsExist,
            receiptExists: receiptExists,
            transcriptExists: transcriptExists,
            validationFailure: validationFailure,
            outputEnvelopes: outputEnvelopes,
            timing: timing,
            recoverySnapshot: recoverySnapshot
        )
    }

    private static func decodeOutcomeEnvelope(from agent: AgentExecution) -> OutcomeEnvelope? {
        guard let data = agent.outcomeEnvelopeJSON else { return nil }
        return try? JSONDecoder().decode(OutcomeEnvelope.self, from: data)
    }
}

// MARK: - Failed Stage Evidence Packet (§6.3)

/// Complete evidence packet for a failed stage.
/// Both blocked-run operator UI and exported reports read from this same packet.
struct FailedStageEvidencePacket: Codable, Sendable, Identifiable {
    let id: UUID
    let timestamp: Date

    // Stage identification
    let stageID: String
    let stageLabel: String
    let stageAttemptNumber: Int

    // Failed agent (if specific agent failed)
    let failedAgentID: String?
    let failedAgentTitle: String?

    // Failure description
    let failureSummary: String
    let failureClass: ValidationFailureClass
    let supervisionClassification: SupervisionClassification?
    let canonicalOutcome: AgentCanonicalOutcome?
    let transportErrorKind: TransportErrorKind?
    let outputPresence: OutputPresence?

    // Evidence availability
    let rawOutputsExist: Bool
    let receiptExists: Bool
    let transcriptExists: Bool

    // Detailed records
    let validationFailure: ValidationFailureRecord?
    let outputEnvelopes: [StructuredOutputEnvelope]

    // Timing
    let timing: StageTiming

    // Recovery
    let recoverySnapshot: RecoveryActionSnapshot?
}

// MARK: - Stage Timing

struct StageTiming: Codable, Sendable {
    let stageStartedAt: Date
    let stageCompletedAt: Date?
    let agentStartedAt: Date?
    let agentCompletedAt: Date?
    let agentDurationSeconds: Double?
}
