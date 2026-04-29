// ProposalReviewRoutingTests.swift
//
// P060 Phase 3 Swift parity tests. These tests do NOT exercise the
// scoring algorithm (which is not ported yet); they exercise the
// Codable wire-shape parity with Rust serde and the
// RoutingEvidenceProjectionAuthorizer redaction policy.

import XCTest

@testable import Chainworks_Forge

final class ProposalReviewRoutingTests: XCTestCase {

    // MARK: - JSON wire-shape parity with Rust serde

    func test_reviewRoutingMode_decodesSnakeCaseFromRust() throws {
        // Mirror of Rust `#[serde(rename_all = "snake_case")]`.
        XCTAssertEqual(try JSONDecoder().decode(ReviewRoutingMode.self, from: Data(#""legacy_fixed""#.utf8)), .legacyFixed)
        XCTAssertEqual(try JSONDecoder().decode(ReviewRoutingMode.self, from: Data(#""shadow_dynamic""#.utf8)), .shadowDynamic)
        XCTAssertEqual(try JSONDecoder().decode(ReviewRoutingMode.self, from: Data(#""dynamic""#.utf8)), .dynamic)
    }

    func test_reviewRoutingMode_defaultsToDynamic() {
        XCTAssertEqual(ReviewRoutingMode.defaultMode, .dynamic)
        XCTAssertEqual(ReviewRoutingOptions().mode, .dynamic)
    }

    func test_reviewRoutingOptions_encodesRustWireShape() throws {
        let options = ReviewRoutingOptions(
            mode: .legacyFixed,
            forceInclude: ["proposal_reviewer_security"],
            forceExclude: ["proposal_reviewer_ui"],
            operatorOverrideRationale: "Security-sensitive API"
        )
        let payload = try JSONSerialization.jsonObject(
            with: try JSONEncoder().encode(options)
        ) as? [String: Any]

        XCTAssertEqual(payload?["mode"] as? String, "legacy_fixed")
        XCTAssertEqual(payload?["force_include"] as? [String], ["proposal_reviewer_security"])
        XCTAssertEqual(payload?["force_exclude"] as? [String], ["proposal_reviewer_ui"])
        XCTAssertEqual(payload?["override_reason"] as? String, "Security-sensitive API")
        XCTAssertNil(payload?["operator_override_rationale"])
    }

    func test_routingEvidenceRef_roundTripPreservesAllFields() throws {
        let original = RoutingEvidenceRef(
            evidenceId: "e1",
            evidenceType: "keyword",
            hash: "abc123",
            normalizedValue: "security",
            path: "src/auth.rs",
            symbol: "validate_token",
            span: "10:15"
        )
        let json = try JSONEncoder().encode(original)
        let decoded = try JSONDecoder().decode(RoutingEvidenceRef.self, from: json)
        XCTAssertEqual(decoded, original)
    }

    func test_routingEvidenceRef_decodesFromRustSerdeShape() throws {
        // Rust serde serialises with snake_case keys; Swift Codable
        // must accept that wire shape directly.
        let wire = """
        {
          "evidence_id": "e1",
          "evidence_type": "keyword",
          "hash": "abc123",
          "normalized_value": "security",
          "path": "src/auth.rs",
          "symbol": "validate_token",
          "span": "10:15"
        }
        """
        let decoded = try JSONDecoder().decode(
            RoutingEvidenceRef.self, from: Data(wire.utf8))
        XCTAssertEqual(decoded.evidenceId, "e1")
        XCTAssertEqual(decoded.evidenceType, "keyword")
        XCTAssertEqual(decoded.hash, "abc123")
        XCTAssertEqual(decoded.normalizedValue, "security")
        XCTAssertEqual(decoded.path, "src/auth.rs")
        XCTAssertEqual(decoded.symbol, "validate_token")
        XCTAssertEqual(decoded.span, "10:15")
    }

    func test_routingEvidenceRef_redactedDropsRawFields() {
        let evidence = RoutingEvidenceRef(
            evidenceId: "e1",
            evidenceType: "keyword",
            hash: "abc123",
            normalizedValue: "security",
            path: "src/auth.rs",
            symbol: "validate_token",
            span: "10:15"
        )
        let redacted = evidence.redacted()
        XCTAssertEqual(redacted.evidenceId, "e1")
        XCTAssertEqual(redacted.hash, "abc123")
        XCTAssertNil(redacted.normalizedValue)
        XCTAssertNil(redacted.path)
        XCTAssertNil(redacted.symbol)
        XCTAssertNil(redacted.span)
    }

    func test_scoreTerms_totalMatchesProposalFormula() {
        // Same fixture as the Rust test `score_terms_formula_matches_proposal`
        // — proves the formula is bit-for-bit identical across languages.
        let terms = ScoreTerms(
            forceInclude: 1,
            stackMatches: 2,
            surfaceMatches: 1,
            riskMatches: 1,
            strongKeywordMatches: 3,
            repoSignalMatches: 1,
            crossStackDependencyMatches: 0,
            baselineGapMatches: 1,
            overlapPenalty: 1
        )
        // 1*5 + 2*4 + 1*3 + 1*3 + 3*2 + 1*2 + 0*2 + 1*1 - 1*3 = 25
        XCTAssertEqual(terms.total(), 25)
    }

    func test_agentSelectionPlanV1_decodesFromRustEmittedJson() throws {
        // Synthetic-but-realistic wire shape that mirrors the Rust
        // `agent_selection_plan_v1` artifact.
        let wire = """
        {
          "schema_version": "1",
          "routing_rules_version": "1",
          "proposal_md5": "abc",
          "plan_hash": "deadbeef",
          "mode": "dynamic",
          "fingerprint": ["macos", "reliability"],
          "selected_agents": [
            {
              "agent_id": "proposal_reviewer_macos",
              "routing_id": "routing-macos",
              "score": 14,
              "mandatory": false,
              "override_source": "force_include",
              "score_terms": {
                "force_include": 1, "stack_matches": 1, "surface_matches": 1,
                "risk_matches": 0, "strong_keyword_matches": 0,
                "repo_signal_matches": 0, "cross_stack_dependency_matches": 0,
                "baseline_gap_matches": 0, "overlap_penalty": 0
              },
              "rationale": "macOS surface match",
              "evidence_refs": [
                {
                  "evidence_id": "e1",
                  "evidence_type": "keyword",
                  "hash": "h1",
                  "normalized_value": "macOS",
                  "path": "docs/proposals/060.md",
                  "symbol": "routing",
                  "span": "10:15"
                }
              ],
              "materialization_binding_id": "bind-1"
            }
          ],
          "rejected_alternatives": [
            {
              "agent_id": "proposal_reviewer_security",
              "routing_id": "routing-security",
              "score": 3,
              "reason": "not_selected",
              "score_terms": {
                "force_include": 0, "stack_matches": 0, "surface_matches": 1,
                "risk_matches": 0, "strong_keyword_matches": 0,
                "repo_signal_matches": 0, "cross_stack_dependency_matches": 0,
                "baseline_gap_matches": 0, "overlap_penalty": 0
              }
            }
          ],
          "ineligible_candidates": [],
          "warnings": ["under_specified_selection"],
          "input_snapshot_hashes": {
            "workflow_snapshot_hash": "workflow-hash",
            "catalog_snapshot_hash": "catalog-hash",
            "routing_metadata_hash": "routing-metadata-hash",
            "candidate_binding_hash": "candidate-binding-hash",
            "evidence_hash": "evidence-hash",
            "override_hash": null
          }
        }
        """
        let plan = try JSONDecoder().decode(
            AgentSelectionPlanV1.self, from: Data(wire.utf8))
        XCTAssertEqual(plan.schemaVersion, "1")
        XCTAssertEqual(plan.proposalMd5, "abc")
        XCTAssertEqual(plan.mode, .dynamic)
        XCTAssertEqual(plan.selectedAgents.count, 1)
        XCTAssertEqual(plan.selectedAgents[0].agentId, "proposal_reviewer_macos")
        XCTAssertEqual(plan.selectedAgents[0].score, 14)
        XCTAssertEqual(plan.selectedAgents[0].overrideSource, "force_include")
        XCTAssertEqual(plan.selectedAgents[0].rationale, "macOS surface match")
        XCTAssertEqual(plan.selectedAgents[0].evidenceRefs.count, 1)
        XCTAssertEqual(plan.selectedAgents[0].evidenceRefs[0].path, "docs/proposals/060.md")
        XCTAssertEqual(plan.selectedAgents[0].materializationBindingId, "bind-1")
        XCTAssertEqual(plan.fingerprint, ["macos", "reliability"])
        XCTAssertEqual(plan.warnings, ["under_specified_selection"])
        XCTAssertEqual(plan.inputSnapshotHashes.workflowSnapshotHash, "workflow-hash")
        XCTAssertEqual(plan.inputSnapshotHashes.overrideHash, nil)
        XCTAssertEqual(plan.rejectedAlternatives[0].score, 3)
        XCTAssertEqual(plan.planHash, "deadbeef")
    }

    // MARK: - RoutingEvidenceProjectionAuthorizer

    func test_authorizer_defaultIsRedacted() {
        let evidence = sampleEvidence()
        let auth = RoutingEvidenceProjectionAuthorizer.redactedOnly
        let projected = auth.project(evidence)
        XCTAssertEqual(projected.evidenceId, evidence.evidenceId)
        XCTAssertEqual(projected.hash, evidence.hash)
        XCTAssertNil(projected.normalizedValue)
        XCTAssertNil(projected.path)
        XCTAssertNil(projected.symbol)
        XCTAssertNil(projected.span)
    }

    func test_authorizer_fullPreservesAllFields() {
        let evidence = sampleEvidence()
        let auth = RoutingEvidenceProjectionAuthorizer.full
        let projected = auth.project(evidence)
        XCTAssertEqual(projected, evidence)
    }

    func test_authorizer_envWithoutGrantStaysRedacted() {
        let env: [String: String] = [:]
        let auth = RoutingEvidenceProjectionAuthorizer.fromEnvironment(environment: env)
        XCTAssertEqual(auth.projection, .redacted)
    }

    func test_authorizer_envWithDisableValueStaysRedacted() {
        let env: [String: String] = [
            "CHAINWORKS_OPERATOR_DEBUG_ROUTING_EVIDENCE": "0",
        ]
        let auth = RoutingEvidenceProjectionAuthorizer.fromEnvironment(environment: env)
        XCTAssertEqual(auth.projection, .redacted)
    }

    func test_authorizer_envWithEnableGrantsFull() {
        let env: [String: String] = [
            "CHAINWORKS_OPERATOR_DEBUG_ROUTING_EVIDENCE": "1",
        ]
        let auth = RoutingEvidenceProjectionAuthorizer.fromEnvironment(environment: env)
        XCTAssertEqual(auth.projection, .full)
    }

    func test_authorizer_projectsEntirePlanRedactingEvidenceOnly() throws {
        let plan = AgentSelectionPlanV1(
            schemaVersion: "1",
            routingRulesVersion: "1",
            proposalMd5: "abc",
            planHash: "deadbeef",
            mode: .dynamic,
            fingerprint: ["macos"],
            selectedAgents: [],
            rejectedAlternatives: [],
            ineligibleCandidates: [],
            warnings: [],
            inputSnapshotHashes: sampleInputSnapshotHashes()
        )
        let auth = RoutingEvidenceProjectionAuthorizer.redactedOnly
        let projected = auth.project(plan)
        XCTAssertEqual(projected.planHash, plan.planHash)
        XCTAssertEqual(projected.mode, plan.mode)
    }

    func test_authorizer_projectsEntirePlanRedactingSelectedAgentEvidenceOnly() throws {
        let selected = SelectedAgent(
            agentId: "proposal_reviewer_macos",
            routingId: "routing-macos",
            score: 14,
            mandatory: false,
            overrideSource: nil,
            scoreTerms: ScoreTerms(
                forceInclude: 0,
                stackMatches: 1,
                surfaceMatches: 1,
                riskMatches: 0,
                strongKeywordMatches: 0,
                repoSignalMatches: 0,
                crossStackDependencyMatches: 0,
                baselineGapMatches: 0,
                overlapPenalty: 0
            ),
            rationale: "macOS surface match",
            evidenceRefs: [sampleEvidence()],
            materializationBindingId: "bind-1"
        )
        let plan = AgentSelectionPlanV1(
            schemaVersion: "1",
            routingRulesVersion: "1",
            proposalMd5: "abc",
            planHash: "deadbeef",
            mode: .dynamic,
            fingerprint: ["macos"],
            selectedAgents: [selected],
            rejectedAlternatives: [],
            ineligibleCandidates: [],
            warnings: [],
            inputSnapshotHashes: sampleInputSnapshotHashes()
        )
        let auth = RoutingEvidenceProjectionAuthorizer.redactedOnly
        let projected = auth.project(plan)
        XCTAssertEqual(projected.selectedAgents.count, 1)
        XCTAssertNil(projected.selectedAgents[0].evidenceRefs[0].normalizedValue)
        XCTAssertNil(projected.selectedAgents[0].evidenceRefs[0].path)
        XCTAssertEqual(projected.selectedAgents[0].evidenceRefs[0].evidenceId, "e1")
    }

    // MARK: - Feature-flag cutover resolver

    func test_resolveEffectiveMode_noEnvUsesPerRun() {
        let res = resolveEffectiveRoutingMode(perRunMode: .dynamic, environment: [:])
        XCTAssertEqual(res.effective(), .dynamic)
        if case .usedPerRunMode = res {
            // ok
        } else {
            XCTFail("expected usedPerRunMode, got \(res)")
        }
    }

    func test_resolveEffectiveMode_envLegacyOverridesDynamic() {
        let env = ["CHAINWORKS_P060_ROUTING_MODE_OVERRIDE": "legacy_fixed"]
        let res = resolveEffectiveRoutingMode(perRunMode: .dynamic, environment: env)
        XCTAssertEqual(res.effective(), .legacyFixed)
        if case let .overriddenByEnv(from, to) = res {
            XCTAssertEqual(from, .dynamic)
            XCTAssertEqual(to, .legacyFixed)
        } else {
            XCTFail("expected overriddenByEnv")
        }
    }

    func test_resolveEffectiveMode_envShadowOverridesLegacy() {
        let env = ["CHAINWORKS_P060_ROUTING_MODE_OVERRIDE": "shadow_dynamic"]
        let res = resolveEffectiveRoutingMode(perRunMode: .legacyFixed, environment: env)
        XCTAssertEqual(res.effective(), .shadowDynamic)
    }

    func test_resolveEffectiveMode_unrecognizedFallsBack() {
        let env = ["CHAINWORKS_P060_ROUTING_MODE_OVERRIDE": "totally_made_up"]
        let res = resolveEffectiveRoutingMode(perRunMode: .dynamic, environment: env)
        XCTAssertEqual(res.effective(), .dynamic)
        if case let .overrideUnrecognized(raw, perRun) = res {
            XCTAssertEqual(raw, "totally_made_up")
            XCTAssertEqual(perRun, .dynamic)
        } else {
            XCTFail("expected overrideUnrecognized")
        }
    }

    // MARK: - Helpers

    private func sampleEvidence() -> RoutingEvidenceRef {
        RoutingEvidenceRef(
            evidenceId: "e1",
            evidenceType: "keyword",
            hash: "abc123",
            normalizedValue: "security",
            path: "src/auth.rs",
            symbol: "validate_token",
            span: "10:15"
        )
    }

    private func sampleInputSnapshotHashes() -> InputSnapshotHashes {
        InputSnapshotHashes(
            workflowSnapshotHash: "workflow-hash",
            catalogSnapshotHash: "catalog-hash",
            routingMetadataHash: "routing-metadata-hash",
            candidateBindingHash: "candidate-binding-hash",
            evidenceHash: "evidence-hash",
            overrideHash: nil
        )
    }
}
