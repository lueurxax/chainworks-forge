import Foundation
import Testing
@testable import Chainworks_Forge

@Suite("Proposal 086 continuation readback", .tags(.fast))
struct Proposal086ContinuationReadbackTests {

    @MainActor
    @Test("P086 continuation readback decodes and presents resurrection evidence")
    func continuationReadbackDecodesAndPresentsResurrectionEvidence() throws {
        let record = P086ContinuationRecordReadModel(
            id: "cont-1",
            runID: "run-1",
            stageExecutionID: "stage-exec-1",
            agentExecutionID: "agent-exec-1",
            modeRaw: "provider_session_resurrection",
            modeDisplay: "Provider session resurrection",
            triggerKindRaw: "operator_mcp",
            triggerKindDisplay: "Operator MCP",
            statusRaw: "succeeded",
            statusDisplay: "Succeeded",
            isTerminal: true,
            failureReason: nil,
            reconciliationStatus: nil,
            requestFingerprintSHA256: String(repeating: "a", count: 64),
            canonicalRequestArtifactID: "canonical-request-artifact",
            attachReceiptArtifactID: "attach-receipt-artifact",
            evidenceBundleArtifactID: "evidence-bundle-artifact",
            worktreeReadbackArtifactID: "worktree-readback-artifact",
            continuationReportArtifactID: "continuation-report-artifact",
            responseFingerprintSHA256: String(repeating: "b", count: 64),
            responseArtifactID: "response-artifact",
            resultOrNoProgressArtifactID: "result-artifact",
            conflictCount: 0,
            createdAt: "2026-06-20T00:00:00Z",
            updatedAt: "2026-06-20T00:01:00Z",
            freshnessState: .live,
            projectionLagMS: 10
        )
        let metrics = P086ContinuationMetricsSummaryReadModel(
            runID: "run-1",
            admissionTotal: 1,
            acceptedTotal: 1,
            rejectedTotal: 0,
            replayTotal: 0,
            successTotal: 1,
            noProgressTotal: 0,
            failedTotal: 0,
            cancelledTotal: 0,
            freshSessionAvoidedTotal: 1,
            leadAutoTotal: 0,
            operatorMCPTotal: 1,
            changedFilesTotal: 0,
            testsOrGatesTotal: 1,
            terminalTotal: 1,
            usefulProgressTotal: 1,
            usefulProgressRate: 1.0,
            noProgressRate: 0.0,
            testsPassedAfterContinuationTotal: 1,
            followupValidationTotal: 1,
            followupValidationSuccessTotal: 1,
            followupValidationSuccessRate: 1.0,
            leadAutoSuccessTotal: 0,
            leadAutoSuccessRate: 0.0,
            operatorMCPSuccessTotal: 1,
            operatorMCPSuccessRate: 1.0,
            timeSavedSecondsTotal: 120,
            timeSavedSampleCount: 1,
            averageTimeSavedSeconds: 120.0,
            providerSessionBudgetInputTokensTotal: 10,
            providerSessionBudgetOutputTokensTotal: 20,
            providerSessionBudgetCachedInputTokensTotal: 0,
            providerSessionBudgetCostCentsTotal: 1,
            providerSessionResurrectionAttachSuccessTotal: 1,
            providerSessionResurrectionAttachFailureTotal: 0,
            providerSessionResurrectionPromptSentTotal: 1,
            orphanReapAttemptedTotal: 0,
            orphanReapVerifiedTotal: 0,
            resurrectionUnsupportedTotal: 0
        )

        let presentation = try #require(
            P086ContinuationReadbackPresenter.presentationIfPresent(
                records: [record],
                metrics: metrics
            )
        )

        #expect(presentation.title == "Agent Continuation")
        #expect(presentation.latestStatus == "Succeeded")
        #expect(presentation.latestMode == "Provider session resurrection")
        #expect(presentation.latestContinuationID == "cont-1")
        #expect(presentation.artifactSummary == "7 evidence artifacts")
        #expect(presentation.metricSummary.contains("1 fresh session avoided"))
        #expect(presentation.metricSummary.contains("useful progress 100%"))
        #expect(presentation.accessibilityLabel.contains("P086 continuation readback"))
    }
}
