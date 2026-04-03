import Foundation

// MARK: - OutputContractTemplates

/// Generates structurally valid mock output data for all 11 artifact contracts.
/// Used by SimulatedAgentExecutor for deterministic testing.
struct OutputContractTemplates {

    /// Generate mock data for a given contract ID.
    /// Returns the artifact data and the format (json/markdown/diff/report).
    static func generate(
        contractID: String,
        agentID: String,
        stageID: String
    ) -> (data: Data, format: ArtifactFormat) {
        switch contractID {
        case "proposal_review_v1":
            return (proposalReview(agentID: agentID), .json)
        case "proposal_review_summary_v1":
            return (proposalReviewSummary(), .json)
        case "review_corpus_bundle_v1":
            return (reviewCorpusBundle(), .json)
        case "score_lift_backlog_v1":
            return (scoreLiftBacklog(), .json)
        case "proposal_feedback_coverage_v1":
            return (proposalFeedbackCoverage(), .json)
        case "proposal_fact_digest_v1":
            return (proposalFactDigest(), .json)
        case "implementation_self_assessment_v1":
            return (implementationSelfAssessment(), .json)
        case "implementation_progress":
            return (implementationProgress(), .json)
        case "changed_files_manifest":
            return (changedFilesManifest(), .json)
        case "tests_result":
            return (testsResult(), .json)
        case "audit_report_v1":
            return (auditReport(), .json)
        case "security_report_v1":
            return (securityReport(), .json)
        case "prepush_review_v1":
            return (prepushReview(), .json)
        case "implementation_review_summary_v1":
            return (implementationReviewSummary(), .json)
        case "docs_report_v1":
            return (docsReport(), .json)
        case "docs_delta":
            return (docsDelta(), .json)
        case "release_manifest":
            return (releaseManifest(), .json)
        case "git_push_receipt_v1":
            return (gitPushReceipt(), .json)
        case "release_bundle_manifest":
            return (releaseBundleManifest(), .json)
        case "connect_upload_receipt_v1":
            return (connectUploadReceipt(), .json)
        // Steward contracts (Proposal 003)
        case "sdlc_health_report_v1":
            return (sdlcHealthReport(), .json)
        case "degradation_alert":
            return (degradationAlert(), .json)
        case "agent_tuning_proposal":
            return (agentTuningProposal(), .json)
        case "workflow_tuning_proposal":
            return (workflowTuningProposal(), .json)
        case "experiment_plan":
            return (experimentPlan(), .json)
        case "stewardship_audit_report_v1":
            return (stewardshipAuditReport(), .json)
        case "agent_retrospective_report_v1":
            return (agentRetrospectiveReport(), .json)
        default:
            // For markdown outputs (proposal_current, idea_brief, etc.) or unknown contracts
            return (genericMarkdown(stageID: stageID, agentID: agentID), .markdown)
        }
    }

    /// Generate a mock artifact for a named output (using catalog contract lookup).
    static func generateForOutput(
        outputName: String,
        agent: ResolvedAgent,
        stageID: String,
        catalog: AgentCatalog? = nil
    ) -> (data: Data, format: ArtifactFormat) {
        // Proposal 013: V2 resolver — catalog-driven contract resolution
        if let contractID = OutputContractResolverV2.resolveContractID(
            for: outputName,
            agent: agent,
            catalog: catalog
        ) {
            let data = generate(contractID: contractID, agentID: agent.id, stageID: stageID).data
            let format: ArtifactFormat
            if let schema = OutputContractResolverV2.resolveSchema(for: outputName, agent: agent, catalog: catalog) {
                format = artifactFormat(from: schema.machineFormat.rawValue)
            } else {
                format = .report
            }
            return (data, format)
        }

        if let hintedPath = catalog?.artifacts[outputName] {
            let format = ArtifactFormat.detect(from: hintedPath, contract: nil)
            switch format {
            case .json, .report:
                return (genericJSON(outputName: outputName, stageID: stageID, agentID: agent.id), format)
            case .markdown:
                return (genericMarkdown(stageID: stageID, agentID: agent.id), format)
            case .diff:
                return (genericDiff(outputName: outputName, agentID: agent.id), format)
            }
        }

        // Look up the contract from the catalog if available
        if let catalog = catalog {
            // Check if the output name matches a contract key
            for (contractID, contract) in catalog.contracts {
                if outputName.contains(contractID.replacingOccurrences(of: "_v1", with: "")) {
                    let format = artifactFormat(from: contract.format)
                    return (generate(contractID: contractID, agentID: agent.id, stageID: stageID).data, format)
                }
            }
        }

        // Default: markdown
        return (genericMarkdown(stageID: stageID, agentID: agent.id), .markdown)
    }

    // MARK: - Contract Templates

    private static func proposalReview(agentID: String) -> Data {
        let json: [String: Any] = [
            "agent_id": agentID,
            "role": "reviewer",
            "score": 8.5,
            "decision": "approve_with_suggestions",
            "verdict": "approve_with_suggestions",
            "summary": "The proposal is viable with a small set of follow-up refinements.",
            "issues": ["Clarify edge-case behavior for missing attachments"],
            "blocking_issues": [] as [String],
            "non_blocking_issues": ["Consider edge case handling for empty inputs"],
            "suggestions": ["Add more unit tests for boundary conditions"],
            "assumptions": ["Existing API contracts remain stable"]
        ]
        return try! JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
    }

    private static func proposalReviewSummary() -> Data {
        let json: [String: Any] = [
            "pass": true,
            "average_score": 8.7,
            "aggregate_score": 8.7,
            "min_individual_score": 8.0,
            "blocker_count": 0,
            "summary": "The proposal clears the review threshold and can move to approval.",
            "required_changes": [] as [String],
            "recurring_themes": ["Testing coverage", "Error handling"],
            "decision": "proceed"
        ]
        return try! JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
    }

    private static func reviewCorpusBundle() -> Data {
        let json: [String: Any] = [
            "review_pass_id": "review-pass-1",
            "review_iteration_id": "state_3_proposal_reviewed.1",
            "source_proposal_artifact": "proposal_current",
            "raw_review_artifacts": [
                "proposal_review_po",
                "proposal_review_ux",
                "proposal_review_ui",
                "proposal_review_architect"
            ],
            "aggregate_summary_artifact": "proposal_review_summary"
        ]
        return try! JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
    }

    private static func scoreLiftBacklog() -> Data {
        let json: [String: Any] = [
            "review_pass_id": "review-pass-1",
            "review_iteration_id": "state_3_proposal_reviewed.1",
            "source_proposal_artifact": "proposal_current",
            "proposal_byte_size": 1600,
            "previous_proposal_byte_size": 1280,
            "proposal_growth_ratio": 1.25,
            "score_delta_since_last_review": 0.55,
            "backlog_items_closed_count": 1,
            "reopened_item_count": 0,
            "growth_guard_recommendation": "within_budget",
            "bounded_next_action": "targeted_rereview",
            "items": [
                [
                    "id": "issue-1",
                    "source_reviewer": "proposal_reviewer_product_owner",
                    "severity": "high",
                    "blocker": true,
                    "category": "scope",
                    "score_impact_class": "high_lift",
                    "description": "Clarify scope boundaries for the first release.",
                    "evidence_refs": ["proposal_review_po"],
                    "status": "open",
                    "merge_provenance": [
                        "merged_issue_refs": [
                            "proposal_review_po:first-release-boundary",
                            "proposal_review_architect:mvp-vs-roadmap-scope"
                        ],
                        "rationale": "Merged overlapping scope findings into one score-limiting backlog item."
                    ] as [String: Any],
                    "last_touched_iteration": 1
                ]
            ]
        ]
        return try! JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
    }

    private static func proposalFeedbackCoverage() -> Data {
        let json: [String: Any] = [
            "proposal_revision_id": "proposal-revision-2",
            "source_review_pass_id": "review-pass-1",
            "backlog_items_addressed": [
                [
                    "id": "issue-1",
                    "resolution": "Clarified the release scope and added a dedicated section on first-release boundaries."
                ]
            ],
            "backlog_items_unresolved": [] as [[String: Any]],
            "backlog_items_deferred": [] as [[String: Any]],
            "backlog_items_disputed": [] as [[String: Any]],
            "sections_changed": ["Scope", "Rollout Plan"],
            "factual_claims_added_or_corrected": [] as [String],
            "notes": "Coverage is complete for the current refine pass."
        ]
        return try! JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
    }

    private static func proposalFactDigest() -> Data {
        let json: [String: Any] = [
            "proposal_revision_id": "proposal-revision-2",
            "claims": [
                [
                    "claim_id": "claim-1",
                    "statement": "The current workflow already persists proposal review summary as JSON.",
                    "evidence_refs": ["proposal_review_summary", "workflow.yaml"],
                    "verification_state": "verified"
                ]
            ]
        ]
        return try! JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
    }

    private static func implementationSelfAssessment() -> Data {
        let json: [String: Any] = [
            "seemingly_complete": true,
            "remaining_tasks": [] as [String],
            "known_risks": ["No external API integration tested"],
            "tests_run": true,
            "docs_impacted": ["README.md"]
        ]
        return try! JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
    }

    private static func implementationProgress() -> Data {
        let json: [String: Any] = [
            "status": "partial",
            "current_phase": "phase_1",
            "completed_items": ["Provisioned the worktree and landed the first implementation slice."],
            "deferred_items": ["Broader cleanup remains for a later pass."],
            "notes": ["Keep the repository buildable after each implementation loop."]
        ]
        return try! JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
    }

    private static func changedFilesManifest() -> Data {
        let json: [String: Any] = [
            "files": ["Chainworks Forge/App.swift"],
            "summary": "Implementation changed one tracked file in the worktree."
        ]
        return try! JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
    }

    private static func testsResult() -> Data {
        let json: [String: Any] = [
            "green": true,
            "summary": "Targeted verification passed for this implementation slice.",
            "commands": ["swift test"],
            "blocking_failures": [] as [String]
        ]
        return try! JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
    }

    private static func auditReport() -> Data {
        let json: [String: Any] = [
            "status": "pass",
            "matches_proposal": true,
            "missing_items": [] as [String],
            "extra_items": [] as [String],
            "defects": [] as [String],
            "required_fixes": [] as [String]
        ]
        return try! JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
    }

    private static func securityReport() -> Data {
        let json: [String: Any] = [
            "status": "pass",
            "critical": 0,
            "high": 0,
            "medium": 0,
            "low": 1,
            "findings": [["severity": "low", "description": "Unused import detected"]],
            "required_fixes": [] as [String]
        ]
        return try! JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
    }

    private static func prepushReview() -> Data {
        let json: [String: Any] = [
            "status": "pass",
            "major_concerns": [] as [String],
            "cleanup_items": ["Remove debug print statements"],
            "test_coverage_notes": "All new code paths covered",
            "release_note": "Added workflow execution engine"
        ]
        return try! JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
    }

    private static func implementationReviewSummary() -> Data {
        let json: [String: Any] = [
            "status": "pass",
            "open_blockers": 0,
            "must_fix": [] as [String],
            "recommended_next_step": "proceed_to_release"
        ]
        return try! JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
    }

    private static func docsReport() -> Data {
        let json: [String: Any] = [
            "status": "pass",
            "changed_docs": ["docs/README.md"],
            "missing_docs": [] as [String],
            "followups": [] as [String]
        ]
        return try! JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
    }

    private static func docsDelta() -> Data {
        let json: [String: Any] = [
            "files": ["README.md", "docs/reference/full-mvp-delivery.md"],
            "summary": "Documentation updated to match the current implementation and release flow."
        ]
        return try! JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
    }

    private static func releaseManifest() -> Data {
        let json: [String: Any] = [
            "commitSHA": "abc123def456",
            "branch": "feature/simulated-run",
            "remote": "origin",
            "commitMessage": "Apply approved proposal via Chainworks Forge",
            "filesChanged": 3,
            "insertions": 42,
            "deletions": 5,
            "timestamp": ISO8601DateFormatter().string(from: Date())
        ]
        return try! JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
    }

    private static func gitPushReceipt() -> Data {
        let json: [String: Any] = [
            "status": "success",
            "branch": "feature/simulated-run",
            "commit_sha": "abc123def456",
            "remote": "origin"
        ]
        return try! JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
    }

    private static func releaseBundleManifest() -> Data {
        let json: [String: Any] = [
            "bundleIdentifier": "com.chainworks.forge.sandbox",
            "bundleVersion": "1.0.0",
            "buildNumber": "abc123de",
            "archivePath": "Build/Fixture/ChainworksForge.xcarchive",
            "checksumSHA256": "deadbeef",
            "sizeBytes": 1024,
            "timestamp": ISO8601DateFormatter().string(from: Date())
        ]
        return try! JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
    }

    private static func connectUploadReceipt() -> Data {
        let json: [String: Any] = [
            "status": "success",
            "artifact_id": "sim-\(UUID().uuidString.prefix(8))"
        ]
        return try! JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
    }

    private static func genericMarkdown(stageID: String, agentID: String) -> Data {
        let md = """
        # Simulated Output

        **Stage:** \(stageID)
        **Agent:** \(agentID)
        **Generated:** \(ISO8601DateFormatter().string(from: Date()))

        This is a simulated artifact produced by the simulated agent executor
        for testing purposes. The content is structurally valid but does not
        represent actual agent output.
        """
        return Data(md.utf8)
    }

    private static func genericJSON(outputName: String, stageID: String, agentID: String) -> Data {
        let json: [String: Any] = [
            "output_name": outputName,
            "stage_id": stageID,
            "agent_id": agentID,
            "generated_at": ISO8601DateFormatter().string(from: Date()),
            "status": "simulated"
        ]
        return try! JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
    }

    private static func genericDiff(outputName: String, agentID: String) -> Data {
        let diff = """
        diff --git a/\(outputName) b/\(outputName)
        --- a/\(outputName)
        +++ b/\(outputName)
        @@ -1 +1 @@
        -Simulated placeholder
        +Updated by \(agentID)
        """
        return Data(diff.utf8)
    }

    // MARK: - Steward Contract Templates (Proposal 003)

    private static func sdlcHealthReport() -> Data {
        let json: [String: Any] = [
            "analysis_id": UUID().uuidString,
            "window_start": ISO8601DateFormatter().string(from: Date().addingTimeInterval(-604800)),
            "window_end": ISO8601DateFormatter().string(from: Date()),
            "cohort_keys": ["workflowFamily": "proposal_to_release", "riskClass": "standard"],
            "cohort_quality": "strong",
            "run_count": 20,
            "metrics_summary": ["lead_time_median_seconds": 3600, "proposal_loop_mean": 1.5],
            "baseline_summary": ["lead_time_median_seconds": 3200, "proposal_loop_mean": 1.4],
            "degradations": [] as [[String: Any]],
            "improvements": [] as [[String: Any]],
            "executive_summary": "No significant degradations detected in the current observation window.",
            "confidence": "high"
        ]
        return try! JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
    }

    private static func degradationAlert() -> Data {
        let json: [String: Any] = [
            "analysis_id": UUID().uuidString,
            "metric_name": "proposal_loop_mean",
            "metric_family": "quality",
            "observed_value": 2.4,
            "baseline_value": 1.6,
            "delta_percentage": 50.0,
            "threshold_used": 25.0,
            "implicated_run_ids": [UUID().uuidString],
            "severity": "medium",
            "likely_causes": ["Recent catalog changes increased review iteration count."],
            "confidence": "medium"
        ]
        return try! JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
    }

    private static func agentTuningProposal() -> Data {
        let json: [String: Any] = [
            "analysis_id": UUID().uuidString,
            "category": "agentTuning",
            "summary": "Increase architect reviewer reasoning depth for proposal review loops.",
            "target_metric": "proposal_loop_mean",
            "proposed_patch_path": "examples/agents/agents.yaml",
            "confidence_level": "medium",
            "status": "proposed"
        ]
        return try! JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
    }

    private static func workflowTuningProposal() -> Data {
        let json: [String: Any] = [
            "analysis_id": UUID().uuidString,
            "category": "workflowTuning",
            "summary": "Relax the proposal approval threshold when all reviewers approve and blockers are zero.",
            "target_metric": "proposal_approval_rate",
            "proposed_patch_path": "examples/workflows/full-mvp-live.yaml",
            "confidence_level": "medium",
            "status": "proposed"
        ]
        return try! JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
    }

    private static func experimentPlan() -> Data {
        let json: [String: Any] = [
            "analysis_id": UUID().uuidString,
            "experiment_type": "limitedRollout",
            "control_config_hash": "control-hash",
            "treatment_config_hash": "treatment-hash",
            "minimum_sample_size": 10,
            "rollback_condition": "Rollback if approval rate drops by more than 10%.",
            "status": "planned"
        ]
        return try! JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
    }

    private static func stewardshipAuditReport() -> Data {
        let json: [String: Any] = [
            "analysis_id": UUID().uuidString,
            "claims_reviewed": 0,
            "claims_supported": 0,
            "claims_undersupported": 0,
            "alternate_explanations": [] as [String],
            "recommendation_risk_review": "No recommendations to review.",
            "safer_next_step": "Continue monitoring with current configuration.",
            "confidence": "high"
        ]
        return try! JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
    }

    private static func agentRetrospectiveReport() -> Data {
        let json: [String: Any] = [
            "analysis_id": UUID().uuidString,
            "agent_id": "simulated_agent",
            "run_id": UUID().uuidString,
            "situation_reconstruction": "Agent was assigned standard task with normal inputs.",
            "expected_vs_actual": "Output matched expected structure and quality.",
            "likely_failure_modes": [] as [String],
            "evidence_refs": [] as [String],
            "suggested_changes": [] as [String],
            "confidence": "high"
        ]
        return try! JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
    }

    // MARK: - Helpers

    private static func artifactFormat(from format: String) -> ArtifactFormat {
        switch format.lowercased() {
        case "json": return .json
        case "markdown", "md": return .markdown
        case "diff": return .diff
        case "report": return .report
        default: return .json
        }
    }
}
