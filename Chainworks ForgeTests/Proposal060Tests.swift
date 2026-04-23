import Foundation
import Testing

@Suite("Proposal060", .serialized)
struct Proposal060Tests {
    @Test("Proposal 060 scratch artifacts include every gate contract")
    func proposal060ScratchArtifactsIncludeEveryGateContract() throws {
        let scratchRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("proposal060-gate-\(UUID().uuidString)", isDirectory: true)
        let artifactRoot = scratchRoot
            .appendingPathComponent("docs/proposals/060-control-artifacts", isDirectory: true)
        try FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: scratchRoot) }

        try writeProposal060ScratchArtifacts(to: artifactRoot)

        let expectedSchemas = Self.expectedArtifactSchemas
        let filenames = try FileManager.default.contentsOfDirectory(atPath: artifactRoot.path)
        #expect(Set(filenames) == Set(expectedSchemas.keys))

        for (filename, expectedSchema) in expectedSchemas {
            let metadata = try artifactMetadata(at: artifactRoot.appendingPathComponent(filename))
            #expect(metadata.schema == expectedSchema)
            #expect(metadata.proposalRevisionID == "P060-r16-2026-04-22")
            #expect(metadata.sourceCommit == "scratch-fixture")
        }
    }

    @Test("Proposal 060 scratch artifacts detect missing required contracts")
    func proposal060ScratchArtifactsDetectMissingRequiredContracts() throws {
        let scratchRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("proposal060-empty-gate-\(UUID().uuidString)", isDirectory: true)
        let artifactRoot = scratchRoot
            .appendingPathComponent("docs/proposals/060-control-artifacts", isDirectory: true)
        try FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: scratchRoot) }

        try writeProposal060ScratchArtifacts(to: artifactRoot)
        try FileManager.default.removeItem(
            at: artifactRoot.appendingPathComponent("proposal-review-baseline.v1.json")
        )

        let filenames = try FileManager.default.contentsOfDirectory(atPath: artifactRoot.path)
        let missing = Set(Self.expectedArtifactSchemas.keys).subtracting(filenames)
        #expect(missing == Set(["proposal-review-baseline.v1.json"]))

        let result = try runProposal060Gate("proposal-060", rootOverride: scratchRoot)
        #expect(result.exitCode != 0)
        #expect(result.output.contains("proposal-060-baseline"))
        #expect(result.output.contains("missing control artifact"))
    }

    @Test("Proposal 060 scratch artifacts detect stale control artifact schema")
    func proposal060ScratchArtifactsDetectStaleControlArtifactSchema() throws {
        let scratchRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("proposal060-stale-schema-gate-\(UUID().uuidString)", isDirectory: true)
        let artifactRoot = scratchRoot
            .appendingPathComponent("docs/proposals/060-control-artifacts", isDirectory: true)
        try FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: scratchRoot) }

        try writeProposal060ScratchArtifacts(to: artifactRoot)
        let staleBaseline = Self.proposalReviewBaseline
            .replacingOccurrences(of: "\"schema\": \"proposal_review_baseline_v1\"", with: "\"schema\": \"proposal_review_baseline_v0\"")
        try staleBaseline.write(
            to: artifactRoot.appendingPathComponent("proposal-review-baseline.v1.json"),
            atomically: true,
            encoding: .utf8
        )

        let metadata = try artifactMetadata(at: artifactRoot.appendingPathComponent("proposal-review-baseline.v1.json"))
        #expect(metadata.schema == "proposal_review_baseline_v0")
        #expect(metadata.schema != Self.expectedArtifactSchemas["proposal-review-baseline.v1.json"])

        let result = try runProposal060Gate("proposal-060-baseline", rootOverride: scratchRoot)
        #expect(result.exitCode != 0)
        #expect(result.output.contains("schema mismatch"))
        #expect(result.output.contains("proposal_review_baseline_v1"))
        #expect(result.output.contains("proposal_review_baseline_v0"))
    }

    @Test("Proposal 060 scratch artifacts detect stale proposal revision")
    func proposal060ScratchArtifactsDetectStaleProposalRevision() throws {
        let scratchRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("proposal060-stale-revision-gate-\(UUID().uuidString)", isDirectory: true)
        let artifactRoot = scratchRoot
            .appendingPathComponent("docs/proposals/060-control-artifacts", isDirectory: true)
        try FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: scratchRoot) }

        try writeProposal060ScratchArtifacts(to: artifactRoot)
        let staleBaseline = Self.proposalReviewBaseline
            .replacingOccurrences(of: "\"proposal_revision_id\": \"P060-r16-2026-04-22\"", with: "\"proposal_revision_id\": \"P060-r15-2026-04-21\"")
        try staleBaseline.write(
            to: artifactRoot.appendingPathComponent("proposal-review-baseline.v1.json"),
            atomically: true,
            encoding: .utf8
        )

        let metadata = try artifactMetadata(at: artifactRoot.appendingPathComponent("proposal-review-baseline.v1.json"))
        #expect(metadata.proposalRevisionID == "P060-r15-2026-04-21")
        #expect(metadata.proposalRevisionID != "P060-r16-2026-04-22")

        let result = try runProposal060Gate("proposal-060-baseline", rootOverride: scratchRoot)
        #expect(result.exitCode != 0)
        #expect(result.output.contains("proposal_revision_id mismatch"))
        #expect(result.output.contains("P060-r16-2026-04-22"))
        #expect(result.output.contains("P060-r15-2026-04-21"))
    }

    @Test("Proposal 060 scratch ticket map covers acceptance criteria and parity controls")
    func proposal060ScratchTicketMapCoversAcceptanceCriteriaAndParityControls() throws {
        let data = try #require(Self.implementationTicketMap.data(using: .utf8))
        let object = try JSONSerialization.jsonObject(with: data)
        let payload = try #require(object as? [String: Any])
        let acceptanceCriteria = try #require(payload["acceptance_criteria"] as? [[String: Any]])

        let summaries = Set(acceptanceCriteria.compactMap { $0["summary"] as? String })
        for criterion in 1...13 {
            #expect(summaries.contains(where: { $0.contains("AC\(criterion)") }))
        }
        #expect(summaries.contains(where: { $0.contains("routing_contract_fixtures_v1 parity deliverable") }))
        #expect(summaries.contains(where: { $0.contains("frozen_snapshot_helper_inventory_v1 parity deliverable") }))

        for row in acceptanceCriteria {
            #expect(row["owner"] as? String != nil)
            #expect(row["phase"] as? String != nil)
            #expect(row["files"] as? [String] != nil)
            #expect(row["modules"] as? [String] != nil)
            #expect(row["test_gate"] as? String != nil)
            #expect(row["reviewer_lens"] as? String != nil)
            #expect(row["high_parity_risk"] as? Bool != nil)
            #expect(row["joint_review"] as? String != nil)
            #expect(row["cross_phase_slip"] as? String != nil)
        }

        let highParitySummaries = Set(
            acceptanceCriteria
                .filter { ($0["high_parity_risk"] as? Bool) == true }
                .compactMap { $0["summary"] as? String }
        )
        #expect(highParitySummaries.contains(where: { $0.contains("routing_contract_fixtures_v1") }))
        #expect(highParitySummaries.contains(where: { $0.contains("frozen_snapshot_helper_inventory_v1") }))
    }

    @Test("Proposal 060 scratch artifacts satisfy executable wrapper gate")
    func proposal060ScratchArtifactsSatisfyExecutableWrapperGate() throws {
        let scratchRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("proposal060-wrapper-gate-\(UUID().uuidString)", isDirectory: true)
        let artifactRoot = scratchRoot
            .appendingPathComponent("docs/proposals/060-control-artifacts", isDirectory: true)
        try FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: scratchRoot) }

        try writeProposal060ScratchArtifacts(to: artifactRoot)

        let result = try runProposal060Gate("proposal-060", rootOverride: scratchRoot)
        #expect(result.exitCode == 0, "proposal-060 gate failed: \(result.output)")
        #expect(result.output.contains("Proposal 060 wrapper gate passed"))
    }

    private func writeProposal060ScratchArtifacts(to artifactRoot: URL) throws {
        let artifacts: [String: String] = [
            "proposal-review-baseline.v1.json": Self.proposalReviewBaseline,
            "storage-compatibility-matrix.v1.json": Self.storageCompatibilityMatrix,
            "routing-contract-fixtures.v1.json": Self.routingContractFixtures,
            "frozen-snapshot-helper-inventory.v1.json": Self.frozenSnapshotInventory,
            "fixed-quartet-inventory.v1.json": Self.fixedQuartetInventory,
            "implementation-ticket-map.v1.json": Self.implementationTicketMap,
            "routing-calibration-report.v1.json": Self.routingCalibrationReport
        ]

        for (filename, content) in artifacts {
            let url = artifactRoot.appendingPathComponent(filename)
            try content.write(to: url, atomically: true, encoding: .utf8)
        }
    }

    private func runProposal060Gate(_ gate: String, rootOverride: URL) throws -> (exitCode: Int32, output: String) {
        let repoRoot = testRepositoryRootURL()
        let scriptURL = repoRoot.appendingPathComponent("scripts/test-gate.sh")
        let process = Process()
        let outputPipe = Pipe()

        process.executableURL = URL(fileURLWithPath: "/bin/bash")
        process.arguments = [scriptURL.path, gate]
        process.currentDirectoryURL = repoRoot
        process.standardOutput = outputPipe
        process.standardError = outputPipe

        var environment = ProcessInfo.processInfo.environment
        environment["CHAINWORKS_TEST_GATE_ROOT_DIR"] = rootOverride.path
        process.environment = environment

        try process.run()
        process.waitUntilExit()

        let output = String(data: outputPipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
        return (process.terminationStatus, output)
    }

    private func artifactMetadata(at url: URL) throws -> (schema: String?, proposalRevisionID: String?, sourceCommit: String?) {
        let data = try Data(contentsOf: url)
        let object = try JSONSerialization.jsonObject(with: data)
        let payload = try #require(object as? [String: Any])
        return (
            schema: payload["schema"] as? String,
            proposalRevisionID: payload["proposal_revision_id"] as? String,
            sourceCommit: payload["source_commit"] as? String
        )
    }

    private static let expectedArtifactSchemas = [
        "proposal-review-baseline.v1.json": "proposal_review_baseline_v1",
        "storage-compatibility-matrix.v1.json": "storage_compatibility_matrix_v1",
        "routing-contract-fixtures.v1.json": "routing_contract_fixtures_v1",
        "frozen-snapshot-helper-inventory.v1.json": "frozen_snapshot_helper_inventory_v1",
        "fixed-quartet-inventory.v1.json": "hardcoded_fixed_quartet_inventory_v1",
        "implementation-ticket-map.v1.json": "implementation_ticket_map_v1",
        "routing-calibration-report.v1.json": "routing_calibration_report_v1"
    ]

    private static let proposalReviewBaseline = """
    {
      "schema": "proposal_review_baseline_v1",
      "proposal_revision_id": "P060-r16-2026-04-22",
      "source_commit": "scratch-fixture",
      "fixed_quartet": { "reviewer_count": 4 },
      "token_measurement_mode": "proxy",
      "corpus_membership": [
        { "id": "ui", "tags": ["ui_only", "macos", "swiftui"], "reviewer_outputs": ["proposal_review_v1"], "quality_baseline": "fixed_quartet reviewer_count" },
        { "id": "rust", "tags": ["rust_backend", "control_plane"], "reviewer_outputs": ["proposal_review_v1"], "quality_baseline": "fixed_quartet reviewer_count" },
        { "id": "security", "tags": ["security_api", "graphql", "security"], "reviewer_outputs": ["proposal_review_v1"], "quality_baseline": "fixed_quartet reviewer_count" },
        { "id": "retry", "tags": ["reliability_retry", "resume", "retry"], "reviewer_outputs": ["proposal_review_v1"], "quality_baseline": "fixed_quartet reviewer_count" },
        { "id": "rollout", "tags": ["rollout", "cutover", "release"], "reviewer_outputs": ["proposal_review_v1"], "quality_baseline": "fixed_quartet reviewer_count" },
        { "id": "mixed", "tags": ["mixed_stack", "swift", "rust"], "reviewer_outputs": ["proposal_review_v1"], "quality_baseline": "fixed_quartet reviewer_count" }
      ],
      "dependency_confirmations": {
        "p017": "informative_only",
        "p047": "extension fallback confirmed"
      }
    }
    """

    private static let storageCompatibilityMatrix = """
    {
      "schema": "storage_compatibility_matrix_v1",
      "proposal_revision_id": "P060-r16-2026-04-22",
      "source_commit": "scratch-fixture",
      "matrix": [
        { "entity": "SystemExecution", "old_run_readback": "hash-only", "rollback_readback": "preserved", "graphql": "exposed", "mcp": "exposed", "report_precedence": "newer wins", "recovery_precedence": "routing first", "migration_behavior": "additive" },
        { "entity": "RoutingReceipt", "old_run_readback": "missing means none", "rollback_readback": "preserved", "graphql": "exposed", "mcp": "exposed", "report_precedence": "receipt wins", "recovery_precedence": "receipt first", "migration_behavior": "additive" },
        { "entity": "AgentSelectionPlanV1", "old_run_readback": "absent", "rollback_readback": "preserved", "graphql": "exposed", "mcp": "exposed", "report_precedence": "selected plan", "recovery_precedence": "plan first", "migration_behavior": "additive" },
        { "entity": "DynamicMaterializationRecord", "old_run_readback": "absent", "rollback_readback": "preserved", "graphql": "exposed", "mcp": "exposed", "report_precedence": "materialization", "recovery_precedence": "idempotency", "migration_behavior": "additive" },
        { "entity": "ReviewCorpusBundleV2", "old_run_readback": "bundle v1 fallback", "rollback_readback": "preserved", "graphql": "exposed", "mcp": "exposed", "report_precedence": "selected artifacts", "recovery_precedence": "selected outputs", "migration_behavior": "additive" }
      ]
    }
    """

    private static let routingContractFixtures = """
    {
      "schema": "routing_contract_fixtures_v1",
      "proposal_revision_id": "P060-r16-2026-04-22",
      "source_commit": "scratch-fixture",
      "canonicalization": { "evidence_ids": ["e1"], "scores": [9], "selected_order": ["proposal_reviewer_macos"], "plan_hash": "hash", "warnings": ["under_specified"], "validation_failures": ["override_conflict"], "under_specified": true, "mandatory_overflow": true },
      "dynamic_phase_contract": {
        "system.routing": "yaml",
        "system_task": "proposal_review_router",
        "dynamic_parallel": "yaml",
        "candidate_bindings": ["compiler-owned"],
        "selected_outputs_from": { "run_id": "run", "attempt": 1, "plan_hash": "hash", "binding_id": "binding", "agent_execution_id": "agent", "proposal_review_v1": true },
        "swift": "codable",
        "rust": "serde",
        "compiled_plan": "snapshot",
        "invalid_case": "parity_matrix"
      },
      "selected_agents": [
        { "reviewer_count": 2, "dynamic_parallel": true, "selected_outputs_from": true },
        { "reviewer_count": 3, "dynamic_parallel": true, "selected_outputs_from": true },
        { "reviewer_count": 4, "dynamic_parallel": true, "selected_outputs_from": true },
        { "reviewer_count": 5, "dynamic_parallel": true, "selected_outputs_from": true }
      ],
      "validation_guard_cases": ["force_include", "force_exclude", "mandatory_conflict", "disabled_rollout_wave", "unknown_agent", "placeholder_resolved_agent"],
      "deterministic_parity": { "same_input": true, "swift": "implementation", "rust": "implementation", "selected_order": ["a"], "evidence_ids": ["e1"], "plan_hash": "hash" },
      "routing_metadata_required_fields": ["routing_id", "family", "capabilities", "stacks", "surfaces", "risks", "enabled_for_proposal_review", "rollout_wave"],
      "catalog_launch_scope": {
        "existing": ["proposal_reviewer_product_owner", "proposal_reviewer_ux", "proposal_reviewer_ui", "proposal_reviewer_architect"],
        "phase_3_core": ["proposal_reviewer_macos", "proposal_reviewer_apple_architect", "proposal_reviewer_rust_architect", "proposal_reviewer_reliability", "proposal_reviewer_security", "proposal_reviewer_api_contract", "proposal_reviewer_observability_rollout"],
        "phase_3_core_enabled": { "phase_3_core": true, "enabled_for_proposal_review": true },
        "later_wave": ["proposal_reviewer_ios", "proposal_reviewer_web", "proposal_reviewer_design", "proposal_reviewer_golang", "proposal_reviewer_dba", "proposal_reviewer_performance"],
        "later_wave_disabled": { "later_wave": true, "enabled_for_proposal_review": false },
        "disabled_by_rollout_wave": "cannot_materialize"
      }
    }
    """

    private static let frozenSnapshotInventory = """
    {
      "schema": "frozen_snapshot_helper_inventory_v1",
      "proposal_revision_id": "P060-r16-2026-04-22",
      "source_commit": "scratch-fixture",
      "repo_search_evidence": "rg workflow catalog readers",
      "readers": [
        { "reader": "post_start workflow reader", "surface": "workflow", "status": "snapshot_backed", "workflow_snapshot": true },
        { "reader": "post_start catalog reader", "surface": "catalog", "status": "legacy_fallback", "catalog_snapshot": true }
      ]
    }
    """

    private static let fixedQuartetInventory = """
    {
      "schema": "hardcoded_fixed_quartet_inventory_v1",
      "proposal_revision_id": "P060-r16-2026-04-22",
      "source_commit": "scratch-fixture",
      "reviewers": ["product_owner", "ux", "ui", "architect"],
      "rows": [
        { "surface": "Swift", "assumption": "fixed_quartet product_owner ux ui architect", "migration": "legacy_behavior" },
        { "surface": "Rust", "assumption": "fixed_quartet product_owner ux ui architect", "migration": "legacy_behavior" },
        { "surface": "fixtures", "assumption": "fixed_quartet product_owner ux ui architect", "migration": "legacy_behavior" },
        { "surface": "reports", "assumption": "fixed_quartet product_owner ux ui architect", "migration": "legacy_behavior" },
        { "surface": "feedback_fidelity", "assumption": "fixed_quartet product_owner ux ui architect", "migration": "legacy_behavior" },
        { "surface": "rereview_scope", "assumption": "fixed_quartet product_owner ux ui architect", "migration": "legacy_behavior" },
        { "surface": "ui_views", "assumption": "fixed_quartet product_owner ux ui architect", "migration": "legacy_behavior" }
      ]
    }
    """

    private static let implementationTicketMap = """
    {
      "schema": "implementation_ticket_map_v1",
      "proposal_revision_id": "P060-r16-2026-04-22",
      "source_commit": "scratch-fixture",
      "review_routing_ingress_path": "RunStartOptionsV2 review_routing Swift launch clone StartRunCmd GraphQL startRun MCP runs.start command_journal redaction persisted snapshot RoutingReceipt operator_actions AgentSelectionPlanV1 input_snapshot_hashes",
      "acceptance_criteria": [
        { "summary": "AC1 proposal_review_router SystemTask", "owner": "runtime", "phase": "phase_1", "files": ["router"], "modules": ["engine"], "test_gate": "proposal-060", "reviewer_lens": "apple_arch", "high_parity_risk": false, "joint_review": "swift_rust", "cross_phase_slip": "none" },
        { "summary": "AC2 AgentSelectionPlanV1 RoutingReceipt validationFailureJSON", "owner": "runtime", "phase": "phase_1", "files": ["router"], "modules": ["engine"], "test_gate": "proposal-060", "reviewer_lens": "chainworks_execution_truth", "high_parity_risk": false, "joint_review": "swift_rust", "cross_phase_slip": "none" },
        { "summary": "AC3 CompiledDynamicAgentBinding ResolvedAgent rollout_wave", "owner": "runtime", "phase": "phase_1", "files": ["compiler"], "modules": ["engine"], "test_gate": "proposal-060", "reviewer_lens": "api_contract", "high_parity_risk": true, "joint_review": "swift_rust", "cross_phase_slip": "none" },
        { "summary": "AC4 dynamic_parallel idempotent materialization", "owner": "runtime", "phase": "phase_2", "files": ["materializer"], "modules": ["engine"], "test_gate": "proposal-060", "reviewer_lens": "reliability", "high_parity_risk": true, "joint_review": "swift_rust", "cross_phase_slip": "none" },
        { "summary": "AC5 selected_outputs_from stale unselected aggregation", "owner": "runtime", "phase": "phase_2", "files": ["aggregation"], "modules": ["engine"], "test_gate": "proposal-060", "reviewer_lens": "api_contract", "high_parity_risk": true, "joint_review": "swift_rust", "cross_phase_slip": "none" },
        { "summary": "AC6 legacy_fixed fixed_quartet", "owner": "runtime", "phase": "phase_2", "files": ["legacy"], "modules": ["engine"], "test_gate": "proposal-060", "reviewer_lens": "observability", "high_parity_risk": false, "joint_review": "swift_rust", "cross_phase_slip": "none" },
        { "summary": "AC7 phase_3_core golden_output", "owner": "catalog", "phase": "phase_2_5", "files": ["agents"], "modules": ["catalog"], "test_gate": "proposal-060", "reviewer_lens": "product", "high_parity_risk": false, "joint_review": "swift_rust", "cross_phase_slip": "none" },
        { "summary": "AC8 disabled_later_wave materialized guard", "owner": "catalog", "phase": "phase_3", "files": ["agents"], "modules": ["catalog"], "test_gate": "proposal-060", "reviewer_lens": "api_contract", "high_parity_risk": false, "joint_review": "swift_rust", "cross_phase_slip": "none" },
        { "summary": "AC9 operator_ui routing_progress under_specified evidence_redaction", "owner": "ui", "phase": "phase_3", "files": ["views"], "modules": ["swiftui"], "test_gate": "proposal-060", "reviewer_lens": "macos_ui", "high_parity_risk": false, "joint_review": "swift_rust", "cross_phase_slip": "none" },
        { "summary": "AC10 graphql mcp swift_debug evidence_projection", "owner": "runtime", "phase": "phase_3", "files": ["projection"], "modules": ["api"], "test_gate": "proposal-060", "reviewer_lens": "security", "high_parity_risk": true, "joint_review": "swift_rust", "cross_phase_slip": "none" },
        { "summary": "AC11 phase_0b control_artifacts proposal_060", "owner": "docs", "phase": "phase_0b", "files": ["docs"], "modules": ["control"], "test_gate": "proposal-060", "reviewer_lens": "observability", "high_parity_risk": true, "joint_review": "swift_rust", "cross_phase_slip": "none" },
        { "summary": "AC12 implementation_ticket_map reviewer_lens cross_phase_slip", "owner": "docs", "phase": "phase_0b", "files": ["docs"], "modules": ["control"], "test_gate": "proposal-060", "reviewer_lens": "observability", "high_parity_risk": true, "joint_review": "swift_rust", "cross_phase_slip": "none" },
        { "summary": "AC13 shadow_cutover quality budget rollback", "owner": "release", "phase": "phase_3", "files": ["rollout"], "modules": ["ops"], "test_gate": "proposal-060", "reviewer_lens": "observability", "high_parity_risk": false, "joint_review": "swift_rust", "cross_phase_slip": "none" },
        { "summary": "routing_contract_fixtures_v1 parity deliverable", "owner": "runtime", "phase": "phase_0b", "files": ["fixtures"], "modules": ["swift", "rust"], "test_gate": "proposal-060-router-fixtures", "reviewer_lens": "api_contract", "high_parity_risk": true, "joint_review": "swift_rust", "cross_phase_slip": "none" },
        { "summary": "frozen_snapshot_helper_inventory_v1 parity deliverable", "owner": "runtime", "phase": "phase_0b", "files": ["inventory"], "modules": ["swift", "rust"], "test_gate": "proposal-060-snapshot-inventory", "reviewer_lens": "chainworks_execution_truth", "high_parity_risk": true, "joint_review": "swift_rust", "cross_phase_slip": "none" }
      ]
    }
    """

    private static let routingCalibrationReport = """
    {
      "schema": "routing_calibration_report_v1",
      "proposal_revision_id": "P060-r16-2026-04-22",
      "source_commit": "scratch-fixture",
      "cases": [
        { "scenario": "UI macos", "expected_selection": ["proposal_reviewer_ui", "proposal_reviewer_macos"] },
        { "scenario": "rust retry resume", "expected_selection": ["proposal_reviewer_rust_architect", "proposal_reviewer_reliability"] },
        { "scenario": "security api", "expected_selection": ["proposal_reviewer_security", "proposal_reviewer_api_contract"] },
        { "scenario": "reliability", "expected_selection": ["proposal_reviewer_reliability"] },
        { "scenario": "rollout", "expected_selection": ["proposal_reviewer_observability_rollout"] },
        { "scenario": "under_specified", "expected_selection": ["proposal_reviewer_product_owner", "proposal_reviewer_architect"], "warning": "caution" },
        { "scenario": "override force_include force_exclude", "expected_selection": ["proposal_reviewer_security"] },
        { "scenario": "mandatory_overflow", "expected_outcome": "fail_closed routing_conflict no_agentselectionplanv1" }
      ]
    }
    """
}
