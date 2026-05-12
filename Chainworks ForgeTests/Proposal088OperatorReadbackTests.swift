import Foundation
import Testing
@testable import Chainworks_Forge

@Suite("Proposal 088 operator readback", .tags(.fast))
struct Proposal088OperatorReadbackTests {
    @Test("Implementation completion readback decodes and presents operator diagnostics")
    func implementationCompletionReadbackDecodesAndPresentsOperatorDiagnostics() throws {
        let json = """
        {
          "status": {"value": "failed", "raw": "failed", "known": true},
          "failureClass": "terminal_response_completed_missing_required_outputs",
          "workChangeKind": "current_attempt_diff",
          "activationSource": "p037_idle_terminalization",
          "ingestionBoundaryFailure": {"value": "chainworks_output_not_extracted", "raw": "chainworks_output_not_extracted", "known": true},
          "completionTurnAttempted": true,
          "completionTurnResult": {"value": "failed_missing_outputs", "raw": "failed_missing_outputs", "known": true},
          "terminalResponseStatus": "completed",
          "freshRequiredOutputCount": 1,
          "staleRequiredOutputCount": 2,
          "missingRequiredOutputCount": 3,
          "controlPlaneOutputCount": 1,
          "receiptArtifactPath": ".chainworks/p088/receipt.json",
          "failedStageEvidencePath": ".chainworks/p088/failed-stage.json",
          "nextOperatorAction": {"value": "fix_chainworks_output_extraction", "raw": "fix_chainworks_output_extraction", "known": true},
          "completionTextCaptures": [{
            "promptKind": "original",
            "turnIndex": 0,
            "terminalResponseStatus": "completed",
            "completionTextStatus": "captured",
            "completionTextCaptureSource": "terminal_final_response",
            "completionTextRawByteLimit": 65536,
            "completionTextCapturedByteCount": 2048,
            "completionTextTruncated": false,
            "extractionInputTruncated": true,
            "extractionInputSha256": "sha256:abc",
            "redactedTextArtifactPath": ".chainworks/p088/final-response.txt",
            "textAbsenceReason": null,
            "createdAt": "2026-05-11T19:16:27Z"
          }]
        }
        """

        let readback = try JSONDecoder().decode(
            P088ImplementationCompletionReadModel.self,
            from: Data(json.utf8)
        )
        let presentation = P088ImplementationCompletionPresenter.presentation(for: readback)

        #expect(presentation.compactSignalLabel == "Implementation Completion: Failed")
        #expect(presentation.outputFreshnessLabel == "Outputs: 1 fresh, 2 stale, 3 missing, 1 control-plane")
        #expect(presentation.primaryEvidencePath == ".chainworks/p088/receipt.json")
        #expect(presentation.nextOperatorActionLabel == "Next: Fix Chainworks Output Extraction")
        #expect(presentation.diagnosticRows.contains("Failure class: terminal_response_completed_missing_required_outputs"))
        #expect(presentation.diagnosticRows.contains("Work change: current_attempt_diff"))
        #expect(presentation.diagnosticRows.contains("Capture: 1 captured, 0 absent"))
    }

    @Test("Run row presentation includes implementation completion signal")
    func runRowPresentationIncludesImplementationCompletionSignal() {
        let run = P031RunRowReadModel(
            id: "run-p088",
            status: "blocked",
            workflowTitle: "Implementation",
            freshnessState: .live,
            totalStages: 4,
            completedStages: 2,
            failedStages: 1,
            pendingApprovals: 0,
            implementationCompletion: P088ImplementationCompletionReadModel(
                status: .known(value: "partial_evidence"),
                failureClass: "work_completed_missing_current_attempt_outputs",
                workChangeKind: "current_attempt_diff",
                activationSource: "declared_output_settlement_failed",
                ingestionBoundaryFailure: .known(value: "none"),
                completionTurnAttempted: false,
                completionTurnResult: .known(value: "not_attempted"),
                terminalResponseStatus: "completed",
                completionTextCaptures: [],
                freshRequiredOutputCount: 0,
                staleRequiredOutputCount: 1,
                missingRequiredOutputCount: 2,
                controlPlaneOutputCount: 1,
                receiptArtifactPath: nil,
                failedStageEvidencePath: ".chainworks/p088/failed-stage.json",
                nextOperatorAction: .known(value: "retry_with_completion_recovery")
            )
        )

        let row = P031RunsHomePresenter.rowPresentation(for: run)

        #expect(row.implementationCompletionSignalLabel == "Implementation Completion: Partial Evidence")
        #expect(row.accessibilityLabel.contains("Implementation Completion: Partial Evidence"))
    }

    @Test("Implementation completion readback preserves unknown future enum values")
    func implementationCompletionReadbackPreservesUnknownFutureEnumValues() throws {
        let json = """
        {
          "status": {"value": "unknown", "raw": "future_status", "known": false},
          "failureClass": "future_failure",
          "workChangeKind": "current_attempt_diff",
          "activationSource": "operator_retry_completion_recovery",
          "ingestionBoundaryFailure": {"value": "unknown", "raw": "future_ingestion", "known": false},
          "completionTurnAttempted": true,
          "completionTurnResult": {"value": "unknown", "raw": "future_result", "known": false},
          "terminalResponseStatus": "completed",
          "freshRequiredOutputCount": 0,
          "staleRequiredOutputCount": 0,
          "missingRequiredOutputCount": 1,
          "controlPlaneOutputCount": 0,
          "receiptArtifactPath": ".chainworks/p088/receipt.json",
          "failedStageEvidencePath": null,
          "nextOperatorAction": {"value": "unknown", "raw": "future_action", "known": false},
          "completionTextCaptures": []
        }
        """

        let readback = try JSONDecoder().decode(
            P088ImplementationCompletionReadModel.self,
            from: Data(json.utf8)
        )
        let presentation = P088ImplementationCompletionPresenter.presentation(for: readback)

        #expect(readback.status.raw == "future_status")
        #expect(readback.status.known == false)
        #expect(readback.ingestionBoundaryFailure.raw == "future_ingestion")
        #expect(readback.completionTurnResult.raw == "future_result")
        #expect(readback.nextOperatorAction.raw == "future_action")
        #expect(presentation.compactSignalLabel == "Implementation Completion: Unknown")
        #expect(presentation.nextOperatorActionLabel == "Next: Unknown")
        #expect(presentation.visualState == .neutral)
    }
}
