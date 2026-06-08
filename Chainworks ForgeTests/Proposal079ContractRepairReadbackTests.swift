import Foundation
import Testing
@testable import Chainworks_Forge

// ---------------------------------------------------------------------------
// P079 Swift readback DTO fixture-decode tests (APPLE-001)
//
// Coverage:
//  - Old-run / pre-P079 (parent null)
//  - Feature-disabled (parent null)
//  - not_attempted
//  - recovered (same-session repair)
//  - recovered (transcript recovery)
//  - recovered (provider fallback)
//  - blocked
//  - cancelled
//  - stale projection (macos-r3-005)
//  - unknown enum future-tolerance (unknownDiagnostic)
//  - lease-reclaimed
//  - in_progress with progress chip
//  - MainActor coalescing: reserved -> prompt_sent -> settled keeps single row identity
//  - Unchanged evidence_version snapshot replay causes zero churn
// ---------------------------------------------------------------------------

@Suite("Proposal 079 contract repair readback", .tags(.fast))
struct Proposal079ContractRepairReadbackTests {

    private func decode(_ json: String) throws -> OutputContractRepairEvidence {
        try JSONDecoder().decode(OutputContractRepairEvidence.self, from: Data(json.utf8))
    }

    // MARK: - Pre-P079 / nil parent

    @Test("Old-run (pre-P079) decodes output_contract_repair as nil without throwing")
    func oldRunDecodesAsNil() throws {
        struct RunRowStub: Codable {
            let runId: String
            let outputContractRepair: OutputContractRepairEvidence?
            enum CodingKeys: String, CodingKey {
                case runId = "run_id"
                case outputContractRepair = "output_contract_repair"
            }
        }
        let json = #"{"run_id":"run-pre-p079","output_contract_repair":null}"#
        let row = try JSONDecoder().decode(RunRowStub.self, from: Data(json.utf8))
        #expect(row.outputContractRepair == nil)
    }

    @Test("Missing output_contract_repair field decodes as nil without throwing")
    func missingFieldDecodesAsNil() throws {
        struct RunRowStub: Codable {
            let runId: String
            let outputContractRepair: OutputContractRepairEvidence?
            enum CodingKeys: String, CodingKey {
                case runId = "run_id"
                case outputContractRepair = "output_contract_repair"
            }
        }
        let json = #"{"run_id":"run-no-p079"}"#
        let row = try JSONDecoder().decode(RunRowStub.self, from: Data(json.utf8))
        #expect(row.outputContractRepair == nil)
    }

    // MARK: - not_attempted

    @Test("not_attempted evidence decodes and presents as informational")
    func notAttemptedDecodesCorrectly() throws {
        let ev = try decode(P079Fixtures.notAttempted)
        #expect(ev.status == .notAttempted)
        #expect(ev.status.presentationCategory == .informational)
        #expect(ev.schemaVersion == "output_contract_repair.v1")

        let p = OutputContractRepairPresenter.presentation(for: ev)
        #expect(p.category == .informational)
        #expect(p.showProgressChip == false)
    }

    // MARK: - recovered via same-session repair

    @Test("Recovered (same-session repair) decodes and presents correctly")
    func recoveredSameSessionRepairDecodes() throws {
        let ev = try decode(P079Fixtures.recoveredSameSession(evidenceVersion: 1))
        #expect(ev.status == .recovered)
        #expect(ev.status.presentationCategory == .recovered)
        #expect(ev.sameSessionRepair?.result == .accepted)
        #expect(ev.finalOutputSettlement == "valid_outputs_from_repair")
        #expect(ev.budget?.repairConsumed == true)

        let p = OutputContractRepairPresenter.presentation(for: ev)
        #expect(p.category == .recovered)
        #expect(p.showProgressChip == false)
    }

    // MARK: - recovered via transcript recovery

    @Test("Recovered (transcript recovery) decodes with parser version")
    func recoveredTranscriptRecoveryDecodes() throws {
        let ev = try decode(P079Fixtures.recoveredTranscript)
        #expect(ev.status == .recovered)
        #expect(ev.transcriptRecovery?.result == .accepted)
        #expect(ev.transcriptRecovery?.recoveryParserVersion == "p079_recovery_v1")
        #expect(ev.transcriptRecovery?.maxRecoveryPayloadBytes == 262144)
        #expect(ev.transcriptRecovery?.maxJsonDepth == 32)
        #expect(ev.transcriptRecovery?.maxChunksExamined == 64)
    }

    // MARK: - recovered via provider fallback

    @Test("Recovered (provider fallback) decodes with packet hash")
    func recoveredFallbackDecodes() throws {
        let ev = try decode(P079Fixtures.recoveredFallback)
        #expect(ev.status == .recovered)
        #expect(ev.providerFallback?.result == .accepted)
        #expect(ev.providerFallback?.fallbackPacketHash == "sha256:abc123deadbeef")
        #expect(ev.budget?.fallbackConsumed == true)
    }

    // MARK: - blocked

    @Test("Blocked evidence decodes with manual_investigation recommended action")
    func blockedDecodesCorrectly() throws {
        let ev = try decode(P079Fixtures.blocked)
        #expect(ev.status == .blocked)
        #expect(ev.status.presentationCategory == .blocked)
        #expect(ev.recommendedNextAction == .manualInvestigation)

        let p = OutputContractRepairPresenter.presentation(for: ev)
        #expect(p.category == .blocked)
        #expect(p.showProgressChip == false)
    }

    // MARK: - cancelled

    @Test("Cancelled evidence decodes and presents as cancelled")
    func cancelledDecodesCorrectly() throws {
        let ev = try decode(P079Fixtures.cancelled)
        #expect(ev.status == .cancelled)
        #expect(ev.status.presentationCategory == .cancelled)
    }

    // MARK: - stale projection (macos-r3-005)

    @Test("Stale projection surfaces isStale=true in presentation")
    func staleProjectionPresents() throws {
        let ev = try decode(P079Fixtures.staleProjection)
        #expect(ev.projectionIntegrity == "stale")
        #expect(ev.projectionStaleSince != nil)

        let p = OutputContractRepairPresenter.presentation(for: ev)
        #expect(p.isStale == true)
        #expect(p.staleSinceLabel != nil)
        #expect(p.diagnosticRows.contains(where: { $0.contains("Stale") }))
    }

    // MARK: - unknown enum future-tolerance

    @Test("Unknown status enum maps to unknownDiagnostic preserving raw string for inspector forensics")
    func unknownStatusMapsToUnknownDiagnostic() throws {
        let ev = try decode(P079Fixtures.unknownEnum)
        // Raw string preserved in associated value (DEFECT-006 fix)
        guard case .unknownDiagnostic(let statusRaw) = ev.status else {
            Issue.record("Expected .unknownDiagnostic status, got \(ev.status)")
            return
        }
        #expect(statusRaw == "future_unknown_value_v99")
        // Derived presentation category also maps to unknownDiagnostic, propagating raw string
        guard case .unknownDiagnostic = ev.status.presentationCategory else {
            Issue.record("Expected .unknownDiagnostic presentation category")
            return
        }
        let p = OutputContractRepairPresenter.presentation(for: ev)
        guard case .unknownDiagnostic = p.category else {
            Issue.record("Expected .unknownDiagnostic presentation category in presentation")
            return
        }
    }

    // MARK: - lease reclamation

    @Test("Lease reclamation decodes reclamation_reason; reserved reclaim consumes no budget")
    func leaseReclaimedDecodes() throws {
        let ev = try decode(P079Fixtures.leaseReclaimed)
        #expect(ev.lease?.state == .settled)
        #expect(ev.lease?.reclamationReason != nil)
        #expect(ev.budget?.repairConsumed == false)
    }

    // MARK: - in_progress progress chip (ui-001)

    @Test("in_progress + prompt_sent lease shows progress chip")
    func inProgressPromptSentShowsChip() throws {
        let ev = try decode(P079Fixtures.inProgress(leaseState: "prompt_sent", evidenceVersion: 1))
        #expect(ev.status == .inProgress)
        #expect(ev.lease?.state == .promptSent)
        let p = OutputContractRepairPresenter.presentation(for: ev)
        #expect(p.showProgressChip == true)
    }

    @Test("in_progress + reserved lease shows progress chip")
    func inProgressReservedShowsChip() throws {
        let ev = try decode(P079Fixtures.inProgress(leaseState: "reserved", evidenceVersion: 1))
        let p = OutputContractRepairPresenter.presentation(for: ev)
        #expect(p.showProgressChip == true)
    }

    @Test("in_progress + settled lease does not show progress chip")
    func inProgressSettledHidesChip() throws {
        let ev = try decode(P079Fixtures.inProgress(leaseState: "settled", evidenceVersion: 1))
        let p = OutputContractRepairPresenter.presentation(for: ev)
        #expect(p.showProgressChip == false)
    }

    // MARK: - SwiftUI identity contract (APPLE-R3-001)

    @Test("Identity is (repairAttemptId, agentExecutionId); evidence_version excluded")
    func identityExcludesEvidenceVersion() throws {
        let ev1 = try decode(P079Fixtures.recoveredSameSession(evidenceVersion: 1))
        let ev2 = try decode(P079Fixtures.recoveredSameSession(evidenceVersion: 2))
        #expect(ev1.id == ev2.id)           // same identity
        #expect(ev1 != ev2)                 // but content differs
        #expect(ev1.evidenceVersion != ev2.evidenceVersion)
    }

    @Test("Unchanged evidence_version produces equal snapshot (zero churn)")
    func unchangedEvidenceVersionNoChurn() throws {
        let json = P079Fixtures.recoveredSameSession(evidenceVersion: 5)
        let ev1 = try decode(json)
        let ev2 = try decode(json)
        #expect(ev1 == ev2)
        #expect(ev1.id == ev2.id)
    }

    @Test("reserved->prompt_sent->settled transitions keep single row identity")
    func leaseTransitionsKeepSingleRowIdentity() throws {
        let evReserved = try decode(P079Fixtures.inProgress(leaseState: "reserved", evidenceVersion: 1))
        let evPromptSent = try decode(P079Fixtures.inProgress(leaseState: "prompt_sent", evidenceVersion: 2))
        let evSettled = try decode(P079Fixtures.recoveredSameSession(evidenceVersion: 3))

        #expect(evReserved.id == evPromptSent.id)
        #expect(evReserved.id == evSettled.id)
        #expect(evReserved != evPromptSent)
        #expect(evPromptSent != evSettled)
    }

    // MARK: - Permission decisions

    @Test("Permission decisions decode with method/resource_kind/decision/reason")
    func permissionDecisionsDecoded() throws {
        let ev = try decode(P079Fixtures.blockedWithPermissions)
        #expect(ev.permissionDecisions.count == 2)
        let denied = ev.permissionDecisions.first(where: { $0.decision == .denied })
        #expect(denied?.resourceKind == "shell")
        let allowed = ev.permissionDecisions.first(where: { $0.decision == .allowed })
        #expect(allowed?.resourceKind == "fs_write_canonical_output_path")
    }

    // MARK: - presentationIfPresent

    @Test("presentationIfPresent returns nil for nil evidence")
    func presentationIfPresentNil() {
        #expect(OutputContractRepairPresenter.presentationIfPresent(for: nil) == nil)
    }

    // MARK: - safeRelativePath (SEC-SWIFT-001)

    @Test("safeRelativePath rejects absolute paths")
    func safeRelativePathRejectsAbsolute() {
        #expect(OutputContractRepairPresenter.safeRelativePath("/etc/passwd") == nil)
        #expect(OutputContractRepairPresenter.safeRelativePath("/Users/user/secret") == nil)
    }

    @Test("safeRelativePath rejects literal backslash")
    func safeRelativePathRejectsBackslash() {
        #expect(OutputContractRepairPresenter.safeRelativePath("foo\\bar") == nil)
        #expect(OutputContractRepairPresenter.safeRelativePath("a\\..\\b") == nil)
    }

    @Test("safeRelativePath rejects URL-encoded traversal including backslash")
    func safeRelativePathRejectsUrlEncodedTraversal() {
        #expect(OutputContractRepairPresenter.safeRelativePath("%2e%2e/foo") == nil)
        #expect(OutputContractRepairPresenter.safeRelativePath("foo/%2F..") == nil)
        // SEC-SWIFT-001: %5c is URL-encoded backslash; must be rejected case-insensitively.
        #expect(OutputContractRepairPresenter.safeRelativePath("foo%5cbar") == nil)
        #expect(OutputContractRepairPresenter.safeRelativePath("foo%5Cbar") == nil)
    }

    @Test("safeRelativePath rejects dotdot path components")
    func safeRelativePathRejectsDotDot() {
        #expect(OutputContractRepairPresenter.safeRelativePath("../outside") == nil)
        #expect(OutputContractRepairPresenter.safeRelativePath("foo/../../etc") == nil)
    }

    @Test("safeRelativePath accepts valid relative paths")
    func safeRelativePathAcceptsValid() {
        #expect(OutputContractRepairPresenter.safeRelativePath("outputs/report.md") == "outputs/report.md")
        #expect(OutputContractRepairPresenter.safeRelativePath("nested/deep/file.json") == "nested/deep/file.json")
        #expect(OutputContractRepairPresenter.safeRelativePath(nil) == nil)
    }
}

// MARK: - Fixtures

private enum P079Fixtures {

    static let notAttempted = evidenceJson(
        status: "not_attempted",
        category: "informational"
    )

    static let recoveredTranscript = evidenceJson(
        status: "recovered",
        category: "recovered",
        transcriptRecoveryJson: """
        {
          "result": "accepted",
          "recovery_source": "transcript",
          "bytes_examined": 4096,
          "max_recovery_payload_bytes": 262144,
          "max_json_depth": 32,
          "max_chunks_examined": 64,
          "recovery_parser_version": "p079_recovery_v1"
        }
        """,
        finalOutputSettlement: "valid_outputs_from_transcript_recovery"
    )

    static let recoveredFallback = evidenceJson(
        status: "recovered",
        category: "recovered",
        repairBudgetConsumed: false,
        fallbackBudgetConsumed: true,
        providerFallbackJson: """
        {
          "result": "accepted",
          "fallback_profile": "claude_orchestrator_high",
          "fallback_agent_execution_id": "agent-fallback-001",
          "parent_failed_agent_execution_id": "agent-001",
          "fallback_packet_hash": "sha256:abc123deadbeef",
          "fallback_principal_id": "principal-001",
          "fallback_principal_capability_hash": "sha256:capabilityhash",
          "deadline_seconds": 900
        }
        """,
        finalOutputSettlement: "valid_outputs_from_fallback"
    )

    static let blocked = evidenceJson(
        status: "blocked",
        category: "blocked",
        recommendedNextAction: "manual_investigation",
        sameSessionRepairJson: """
        {
          "result": "rejected_invalid",
          "turn_count": 1,
          "deadline_seconds": 120,
          "reason": "output_validation_failed",
          "repair_attempt_id": "rep-001"
        }
        """,
        finalOutputSettlement: "blocked_missing_required_outputs"
    )

    static let cancelled = evidenceJson(
        status: "cancelled",
        category: "cancelled",
        recommendedNextAction: "cancel_acknowledged",
        finalOutputSettlement: "cancelled"
    )

    static let staleProjection = evidenceJson(
        status: "in_progress",
        category: "informational",
        projectionIntegrity: "stale",
        projectionStaleSince: "2026-05-30T09:55:00Z"
    )

    static let unknownEnum = evidenceJson(
        status: "future_unknown_value_v99",
        category: "future_unknown_category"
    )

    static let leaseReclaimed = evidenceJson(
        status: "failed",
        category: "failed",
        leaseJson: """
        {
          "key": "lk-reclaimed-001",
          "kind": "repair",
          "state": "settled",
          "settled_result": "unavailable",
          "reclamation_reason": "ttl_expired_reserved",
          "owner_principal_id": "principal-001",
          "acquired_at": "2026-05-29T10:00:00Z",
          "expires_at": "2026-05-29T10:03:00Z",
          "lease_seconds": 180
        }
        """
    )

    static let blockedWithPermissions = evidenceJson(
        status: "blocked",
        category: "blocked",
        permissionsJson: """
        [
          {
            "method": "session/request_permission",
            "resource_kind": "shell",
            "decision": "denied",
            "reason": "repair_turn_denies_shell_execution"
          },
          {
            "method": "session/request_permission",
            "resource_kind": "fs_write_canonical_output_path",
            "decision": "allowed",
            "reason": "canonical_output_path_allowed"
          }
        ]
        """
    )

    static func recoveredSameSession(evidenceVersion: Int) -> String {
        evidenceJson(
            status: "recovered",
            category: "recovered",
            evidenceVersion: evidenceVersion,
            repairBudgetConsumed: true,
            sameSessionRepairJson: """
            {
              "result": "accepted",
              "turn_count": 1,
              "deadline_seconds": 120,
              "reason": null,
              "repair_attempt_id": "rep-001"
            }
            """,
            finalOutputSettlement: "valid_outputs_from_repair"
        )
    }

    static func inProgress(leaseState: String, evidenceVersion: Int) -> String {
        let settledResult = leaseState == "settled" ? #""accepted""# : "null"
        let leaseJson = """
        {
          "key": "lk-fixture-001",
          "kind": "repair",
          "state": "\(leaseState)",
          "settled_result": \(settledResult),
          "reclamation_reason": null,
          "owner_principal_id": "principal-001",
          "acquired_at": "2026-05-30T10:00:00Z",
          "expires_at": "2026-05-30T10:03:00Z",
          "lease_seconds": 180
        }
        """
        return evidenceJson(
            status: "in_progress",
            category: "informational",
            evidenceVersion: evidenceVersion,
            leaseJson: leaseJson
        )
    }

    // MARK: - JSON builder

    private static func evidenceJson(
        status: String,
        category: String,
        evidenceVersion: Int = 1,
        recommendedNextAction: String? = nil,
        repairBudgetConsumed: Bool = false,
        fallbackBudgetConsumed: Bool = false,
        leaseJson: String? = nil,
        permissionsJson: String = "[]",
        projectionIntegrity: String = "fresh",
        projectionStaleSince: String? = nil,
        sameSessionRepairJson: String? = nil,
        transcriptRecoveryJson: String? = nil,
        providerFallbackJson: String? = nil,
        providerPlanEvidenceJson: String? = nil,
        finalOutputSettlement: String? = nil
    ) -> String {
        let nextActionStr = recommendedNextAction.map { "\"\($0)\"" } ?? "null"
        let leaseStr = leaseJson ?? "null"
        let staleSinceStr = projectionStaleSince.map { "\"\($0)\"" } ?? "null"
        let sameSessionStr = sameSessionRepairJson ?? "null"
        let transcriptStr = transcriptRecoveryJson ?? "null"
        let fallbackStr = providerFallbackJson ?? "null"
        let planEvidenceStr = providerPlanEvidenceJson ?? "null"
        let settlementStr = finalOutputSettlement.map { "\"\($0)\"" } ?? "null"
        return """
        {
          "schema_version": "output_contract_repair.v1",
          "repair_attempt_id": "rep-001",
          "run_id": "run-p079-fixture",
          "stage_execution_id": "stage-001",
          "agent_execution_id": "agent-001",
          "session_generation_id": "session-001",
          "role": "lead_orchestrator",
          "provider_family": "gemini",
          "adapter_family": "gemini",
          "required_output_mode": "strict_structured",
          "initial_failure_class": "missing_required_outputs",
          "initial_failure_subtype": null,
          "status": "\(status)",
          "presentation_category": "\(category)",
          "recommended_next_action": \(nextActionStr),
          "required_outputs": [],
          "same_session_repair": \(sameSessionStr),
          "transcript_recovery": \(transcriptStr),
          "provider_fallback": \(fallbackStr),
          "provider_plan_evidence": \(planEvidenceStr),
          "permission_decisions": \(permissionsJson),
          "budget": {"repair_consumed": \(repairBudgetConsumed), "fallback_consumed": \(fallbackBudgetConsumed)},
          "lease": \(leaseStr),
          "policy_feature_flags": [],
          "repair_prompt_template_version": "p079_repair_v1",
          "final_output_settlement": \(settlementStr),
          "evidence_artifact_path": null,
          "evidence_version": \(evidenceVersion),
          "projection_integrity": "\(projectionIntegrity)",
          "projection_stale_since": \(staleSinceStr),
          "created_at": "2026-05-30T10:00:00Z",
          "updated_at": "2026-05-30T10:00:00Z"
        }
        """
    }
}
