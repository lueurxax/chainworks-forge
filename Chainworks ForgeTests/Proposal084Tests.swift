import Foundation
import SwiftUI
import Testing
@testable import Chainworks_Forge

@MainActor
@Suite("Proposal 084")
struct Proposal084Tests {
    @Test("RolloutDecisionSummary decodes backend operator readback without recomputing authority")
    func rolloutDecisionSummaryDecodesBackendReadback() throws {
        let data = Data("""
        {
          "schema_version": "operator_readback_v1",
          "authoritative_record_id": "rollout-contract-check:test",
          "run_id": "run-test",
          "proposal_id": "proposal-084",
          "proposal_revision_id": "p084-r5",
          "status": "fail",
          "backend_decision": "hold",
          "failure_reasons": ["missing_metrics"],
          "waiver_state": "none",
          "waiver_expires_at": null,
          "enforcement_mode": "enforce",
          "enforcement_mode_reason": null,
          "hold_conditions": ["missing_metrics"],
          "rollback_disposition": {
            "mode": "feature_flag_disable_or_enforcement_mode_permissive",
            "data_loss_risk": "none",
            "steps": ["Move enforcement mode through an audited mutation."]
          },
          "enabled_state": "enabled",
          "disabled_reason_code": null,
          "action_id": "rollout_contract_check:test",
          "operator_message": "Rollout contract preflight held implementation scheduling.",
          "source_lane": "run_start_preflight",
          "projection_integrity": "valid",
          "cutover_policy_revision": "p084-cutover-v1",
          "diagnostic_redaction": "bounded",
          "next_steps": ["repair_rollout_contract_or_apply_privileged_waiver"],
          "updated_at": "2026-05-02T09:00:00Z"
        }
        """.utf8)

        let summary = try JSONDecoder().decode(RolloutDecisionSummary.self, from: data)

        #expect(summary.schemaVersion == "operator_readback_v1")
        #expect(summary.backendDecision == "hold")
        #expect(summary.failureReasons == ["missing_metrics"])
        #expect(summary.holdConditions == ["missing_metrics"])
        #expect(summary.rollbackDisposition.mode == "feature_flag_disable_or_enforcement_mode_permissive")
        #expect(summary.cutoverPolicyRevision == "p084-cutover-v1")
        #expect(summary.diagnosticRedaction == "bounded")
    }

    @Test("PreflightReport carries rollout decision summary for read-only SwiftUI presentation")
    func preflightReportCarriesRolloutDecisionSummary() throws {
        let summary = RolloutDecisionSummary(
            schemaVersion: "operator_readback_v1",
            authoritativeRecordID: "rollout-contract-check:test",
            runID: "run-test",
            proposalID: "proposal-084",
            proposalRevisionID: "p084-r5",
            status: "pass",
            backendDecision: "release",
            failureReasons: [],
            waiverState: "none",
            waiverExpiresAt: nil,
            enforcementMode: "enforce",
            enforcementModeReason: nil,
            holdConditions: [],
            rollbackDisposition: .init(mode: "not_applicable", dataLossRisk: "none", steps: []),
            enabledState: "enabled",
            disabledReasonCode: nil,
            actionID: "rollout_contract_check:test",
            operatorMessage: "Rollout contract preflight passed; implementation scheduling may continue.",
            sourceLane: "run_start_preflight",
            projectionIntegrity: "valid",
            cutoverPolicyRevision: "p084-cutover-v1",
            diagnosticRedaction: "none",
            nextSteps: ["continue_implementation_scheduling"],
            updatedAt: "2026-05-02T09:00:00Z"
        )
        let report = PreflightReport(
            status: .pass,
            configurationSource: .persistedSettings,
            checks: [],
            warnings: [],
            blockingIssues: [],
            rolloutDecisionSummary: summary
        )

        let encoded = try JSONEncoder().encode(report)
        let decoded = try JSONDecoder().decode(PreflightReport.self, from: encoded)
        _ = PreflightReportView(report: decoded).body

        #expect(decoded.rolloutDecisionSummary?.backendDecision == "release")
        #expect(decoded.rolloutDecisionSummary?.nextSteps == ["continue_implementation_scheduling"])
    }
}
