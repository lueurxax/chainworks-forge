import Foundation
import Testing
@testable import Chainworks_Forge

@Suite("Proposal 078 operator readback", .tags(.fast))
struct Proposal078OperatorReadbackTests {
    @Test("Side-effect readback decodes and presents blocking operator action")
    func sideEffectReadbackDecodesAndPresentsBlockingOperatorAction() throws {
        let json = """
        {
          "schema_version": "p078_side_effect_readback_v1",
          "run_id": "run-p078",
          "unresolved_count": 1,
          "blocked": true,
          "readback_source": "side_effects_ledger",
          "effects": [{
            "id": "effect-1",
            "run_id": "run-p078",
            "stage_execution_id": "stage-1",
            "agent_execution_id": null,
            "effect_kind": "git_push",
            "status": "needs_reconciliation",
            "target_key": "git_push://run-p078:stage-1",
            "external_write_attempted": true,
            "evidence_root": ".chainworks/runs/run-p078/evidence",
            "readback_source": "side_effects_ledger",
            "report_path": ".chainworks/runs/run-p078/evidence-manifest.json",
            "blocked_reason": "effect_needs_reconciliation",
            "operator_next_action": "effects.reconcile",
            "recommended_mcp_tool": "effects.reconcile",
            "retry_forbidden": true,
            "last_error_kind": "lease_or_deadline_expired",
            "updated_at": "2026-05-12T13:00:00Z"
          }]
        }
        """

        let readback = try JSONDecoder().decode(
            SideEffectReadbackSummary.self,
            from: Data(json.utf8)
        )
        let presentation = P078SideEffectReadbackPresenter.presentation(for: readback)

        #expect(presentation.compactSignalLabel == "Release Side Effects: 1 unresolved")
        #expect(presentation.statusLabel == "Release blocked")
        #expect(presentation.nextOperatorActionLabel == "Next: effects.reconcile")
        #expect(presentation.diagnosticRows.contains("git_push: needs_reconciliation"))
        #expect(presentation.diagnosticRows.contains("Blocked: effect_needs_reconciliation"))
        #expect(presentation.visualState == .blocking)
    }

    @Test("Run row includes P078 side-effect signal")
    func runRowIncludesSideEffectSignal() {
        let readback = SideEffectReadbackSummary(
            schemaVersion: "p078_side_effect_readback_v1",
            runID: "run-p078",
            unresolvedCount: 1,
            blocked: true,
            readbackSource: "side_effects_ledger",
            effects: [
                SideEffectReadbackItem(
                    id: "effect-1",
                    runID: "run-p078",
                    stageExecutionID: "stage-1",
                    agentExecutionID: nil,
                    effectKind: "connect_upload",
                    status: "unrecoverable",
                    targetKey: "connect_upload://run-p078:stage-1",
                    externalWriteAttempted: true,
                    evidenceRoot: nil,
                    readbackSource: "side_effects_ledger",
                    reportPath: nil,
                    blockedReason: "effect_unrecoverable_requires_manual_clear",
                    operatorNextAction: "effects.clear_after_manual_verification",
                    recommendedMCPTool: "effects.clear_after_manual_verification",
                    retryForbidden: true,
                    lastErrorKind: "evidence_integrity_failed",
                    updatedAt: "2026-05-12T13:00:00Z"
                )
            ]
        )
        let run = P031RunRowReadModel(
            id: "run-p078",
            status: "blocked",
            workflowTitle: "Release",
            freshnessState: .live,
            totalStages: 5,
            completedStages: 4,
            failedStages: 0,
            pendingApprovals: 0,
            sideEffectReadback: readback
        )

        let row = P031RunsHomePresenter.rowPresentation(for: run)

        #expect(row.sideEffectSignalLabel == "Release Side Effects: 1 unresolved")
        #expect(row.accessibilityLabel.contains("Release Side Effects: 1 unresolved"))
    }

    @Test("P078 read-only UI accessibility proof has no mutation affordances")
    func readOnlyUIAccessibilityProofHasNoMutationAffordances() throws {
        let repoRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let proofURL = repoRoot.appendingPathComponent(
            "docs/evidence/rollout-contract/operator-readback/p078-macos-accessibility.fixture.json",
            isDirectory: false
        )
        let sourceURL = repoRoot.appendingPathComponent(
            "Chainworks Forge/Views/RunsHomeView.swift",
            isDirectory: false
        )
        let proofData = try Data(contentsOf: proofURL)
        let proof = try #require(
            JSONSerialization.jsonObject(with: proofData) as? [String: Any]
        )
        let elements = try #require(proof["elements"] as? [[String: Any]])
        let identifiers = Set(elements.compactMap { $0["accessibility_identifier"] as? String })
        let forbiddenAbsent = try #require(proof["forbidden_controls_absent"] as? [String])
        let source = try String(contentsOf: sourceURL, encoding: .utf8)

        #expect(proof["schema_version"] as? String == "p078_macos_accessibility_view_hierarchy_v1")
        #expect(identifiers.contains("p078-side-effect-readback-card"))
        #expect(identifiers.contains("p078-side-effect-sidebar-signal"))
        #expect(identifiers.contains("p078-side-effect-next-action"))
        #expect(identifiers.contains("p078-side-effect-diagnostics"))
        #expect(elements.allSatisfy { ($0["mutation_control"] as? Bool) == false })
        #expect(forbiddenAbsent.contains("reconcile"))
        #expect(forbiddenAbsent.contains("retry"))
        #expect(forbiddenAbsent.contains("clear"))
        #expect(source.contains(#".accessibilityIdentifier("p078-side-effect-readback-card")"#))
        #expect(source.contains(#".accessibilityElement(children: .combine)"#))
        #expect(!source.contains("effects.mark_unrecoverable"))
        #expect(!source.contains("effects.clear_after_manual_verification"))
    }
}
