import Foundation

/// Deterministic Goose transport used for Proposal 004 integration/UI proof without a real backend.
/// Proposal 005: conforms to `GooseTransportProtocol` directly (no longer subclasses `GooseTransport`).
/// LOCKED-004: Fixture mode is not touched — fixture transport continues to work unchanged behind the protocol.
final class FixtureGooseTransport: GooseTransportProtocol, @unchecked Sendable {
    enum Scenario {
        case proposalLoopSuccess
        case proposal022FeedbackCycle
        case proposal013AggregateFailure
        case fullMVPSuccess
        case fullMVPRefineThenSuccess
    }

    private let scenario: Scenario
    private let stateQueue = DispatchQueue(label: "FixtureGooseTransport.state")
    private var sessionRequests: [String: GooseSessionRequest] = [:]
    private var taskInvocationCounts: [String: Int] = [:]

    nonisolated init(scenario: Scenario = .proposalLoopSuccess) {
        self.scenario = scenario
    }

    // MARK: - GooseTransportProtocol

    func createSession(request: GooseSessionRequest) async throws -> GooseSessionResponse {
        let sessionID = "fixture-\(UUID().uuidString.prefix(8))"
        stateQueue.sync {
            sessionRequests[sessionID] = request
        }
        return GooseSessionResponse(
            sessionId: sessionID,
            status: "active",
            policyAcknowledgement: GoosePolicyAcknowledgement(
                accepted: true,
                capabilityToken: "fixture-read-only",
                backendPolicyVersion: "fixture-v1"
            )
        )
    }

    func submitPrompt(
        sessionID: String,
        prompt: GoosePromptRequest
    ) -> AsyncThrowingStream<GooseStreamEvent, Error> {
        let request = stateQueue.sync { sessionRequests[sessionID] }
        let events = buildEvents(sessionID: sessionID, prompt: prompt, request: request)

        return AsyncThrowingStream { continuation in
            Task {
                for event in events {
                    continuation.yield(event)
                }
                continuation.finish()
            }
        }
    }

    func closeSession(sessionID: String) async throws {
        _ = stateQueue.sync {
            sessionRequests.removeValue(forKey: sessionID)
        }
    }

    // MARK: - Private: Event Building

    private func buildEvents(
        sessionID: String,
        prompt: GoosePromptRequest,
        request: GooseSessionRequest?
    ) -> [GooseStreamEvent] {
        switch scenario {
        case .proposalLoopSuccess:
            return proposalLoopSuccessEvents(sessionID: sessionID, prompt: prompt, request: request)
        case .proposal022FeedbackCycle:
            return proposalLoopSuccessEvents(sessionID: sessionID, prompt: prompt, request: request)
        case .proposal013AggregateFailure:
            return proposalLoopSuccessEvents(sessionID: sessionID, prompt: prompt, request: request)
        case .fullMVPSuccess:
            return fullMVPSuccessEvents(sessionID: sessionID, prompt: prompt, request: request)
        case .fullMVPRefineThenSuccess:
            return fullMVPSuccessEvents(sessionID: sessionID, prompt: prompt, request: request)
        }
    }

    private func proposalLoopSuccessEvents(
        sessionID: String,
        prompt: GoosePromptRequest,
        request: GooseSessionRequest?
    ) -> [GooseStreamEvent] {
        let taskName = parseTaskName(from: prompt.content)
        let agentID = request?.metadata?["agent_id"] ?? "unknown_agent"
        let outputDirectory = parseOutputDirectory(from: prompt.content)
        let outputNames = parseOutputNames(from: prompt.content)
        let invocation = nextInvocation(for: taskName)

        if let outputDirectory {
            writeFixtureOutputs(
                for: taskName,
                agentID: agentID,
                outputNames: outputNames,
                outputDirectory: outputDirectory,
                invocation: invocation
            )
        }

        let finalOutput = makeFinalOutput(taskName: taskName, agentID: agentID, invocation: invocation)

        return [
            .sessionStarted(raw: #"{"session_id":"\#(sessionID)"}"#),
            .promptSubmitted(raw: #"{"task":"\#(taskName)","request_id":"fixture-request-\#(sessionID)"}"#),
            .toolCallStarted(toolName: "read_workspace", raw: "{}"),
            .toolCallFinished(toolName: "read_workspace", raw: "{}"),
            .textChunk(text: "Fixture backend executing \(taskName)..."),
            .finalOutput(content: finalOutput),
            .sessionClosed(raw: #"{"session_id":"\#(sessionID)"}"#)
        ]
    }

    private func fullMVPSuccessEvents(
        sessionID: String,
        prompt: GoosePromptRequest,
        request: GooseSessionRequest?
    ) -> [GooseStreamEvent] {
        let taskName = parseTaskName(from: prompt.content)
        let agentID = request?.metadata?["agent_id"] ?? "unknown_agent"
        let outputDirectory = parseOutputDirectory(from: prompt.content)
        let outputNames = parseOutputNames(from: prompt.content)
        let invocation = nextInvocation(for: taskName)

        if let outputDirectory {
            writeFixtureOutputs(
                for: taskName,
                agentID: agentID,
                outputNames: outputNames,
                outputDirectory: outputDirectory,
                invocation: invocation
            )
        }

        if shouldWriteToWorktree(taskName: taskName, request: request),
           let workingDirectory = request?.workingDirectory {
            writeFixtureWorktreeChange(taskName: taskName, workingDirectory: URL(fileURLWithPath: workingDirectory, isDirectory: true))
        }

        let finalOutput = makeFinalOutput(taskName: taskName, agentID: agentID, invocation: invocation)

        return [
            .sessionStarted(raw: #"{"session_id":"\#(sessionID)"}"#),
            .promptSubmitted(raw: #"{"task":"\#(taskName)","request_id":"fixture-request-\#(sessionID)"}"#),
            .toolCallStarted(toolName: "inspect_repo_context", raw: "{}"),
            .toolCallFinished(toolName: "inspect_repo_context", raw: "{}"),
            .textChunk(text: "Fixture backend executing \(taskName) for full_mvp_live..."),
            .finalOutput(content: finalOutput),
            .sessionClosed(raw: #"{"session_id":"\#(sessionID)"}"#)
        ]
    }

    // MARK: - Private: Prompt Parsing

    private func parseTaskName(from prompt: String) -> String {
        guard let line = prompt
            .components(separatedBy: .newlines)
            .first(where: { $0.hasPrefix("## Task:") }) else {
            return "unknown_task"
        }
        return line.replacingOccurrences(of: "## Task:", with: "").trimmingCharacters(in: .whitespaces)
    }

    private func parseOutputNames(from prompt: String) -> [String] {
        guard let sectionStart = prompt.range(of: "### Expected Outputs") else { return [] }
        let tail = prompt[sectionStart.upperBound...]
        var names: [String] = []
        for rawLine in tail.components(separatedBy: .newlines) {
            let line = rawLine.trimmingCharacters(in: .whitespaces)
            if line.hasPrefix("Output directory:") || line.hasPrefix("### Stop Condition") {
                break
            }
            if line.hasPrefix("- ") {
                names.append(String(line.dropFirst(2)))
            }
        }
        return names
    }

    private func parseOutputDirectory(from prompt: String) -> URL? {
        guard let line = prompt
            .components(separatedBy: .newlines)
            .first(where: { $0.hasPrefix("Output directory:") }) else {
            return nil
        }
        let path = line.replacingOccurrences(of: "Output directory:", with: "").trimmingCharacters(in: .whitespaces)
        guard !path.isEmpty else { return nil }
        return URL(fileURLWithPath: path, isDirectory: true)
    }

    // MARK: - Private: Fixture Output Writing

    private func writeFixtureOutputs(
        for taskName: String,
        agentID: String,
        outputNames: [String],
        outputDirectory: URL,
        invocation: Int
    ) {
        try? FileManager.default.createDirectory(at: outputDirectory, withIntermediateDirectories: true)

        for outputName in outputNames {
            let content = makeOutputContent(
                taskName: taskName,
                agentID: agentID,
                outputName: outputName,
                invocation: invocation
            )
            let url = outputDirectory.appendingPathComponent(outputName)
            try? content.data(using: .utf8)?.write(to: url)
        }
    }

    private func makeFinalOutput(taskName: String, agentID: String, invocation: Int) -> String {
        makeOutputContent(taskName: taskName, agentID: agentID, outputName: "final_output", invocation: invocation)
    }

    private func makeOutputContent(taskName: String, agentID: String, outputName: String, invocation: Int) -> String {
        switch outputName {
        case "proposal_review_po":
            if case .proposal022FeedbackCycle = scenario, taskName == "aggregate_proposal_reviews", invocation >= 2 {
                return reviewerJSON(agentID: agentID, role: "product_owner", score: 9.3)
            }
            return reviewerJSON(agentID: agentID, role: "product_owner", score: 9.4)
        case "proposal_review_ux":
            if case .proposal022FeedbackCycle = scenario, taskName == "aggregate_proposal_reviews", invocation >= 2 {
                return reviewerJSON(agentID: agentID, role: "ux", score: 9.2)
            }
            return reviewerJSON(agentID: agentID, role: "ux", score: 9.2)
        case "proposal_review_ui":
            if case .proposal022FeedbackCycle = scenario, taskName == "aggregate_proposal_reviews", invocation >= 2 {
                return reviewerJSON(agentID: agentID, role: "ui", score: 9.1)
            }
            return reviewerJSON(agentID: agentID, role: "ui", score: 9.1)
        case "proposal_review_architect":
            if case .proposal022FeedbackCycle = scenario, taskName == "aggregate_proposal_reviews", invocation >= 2 {
                return reviewerJSON(agentID: agentID, role: "architect", score: 9.2)
            }
            return reviewerJSON(agentID: agentID, role: "architect", score: 9.3)
        case "proposal_review_summary":
            if case .proposal022FeedbackCycle = scenario {
                if invocation == 1 {
                    return """
                    {
                      "pass": false,
                      "average_score": 7.3,
                      "aggregate_score": 7.3,
                      "min_individual_score": 7.0,
                      "blocker_count": 2,
                      "summary": "Proposal still needs a refine pass driven by the full review corpus.",
                      "required_changes": [
                        "Narrow MVP scope boundaries",
                        "Keep unresolved operator rationale explicit"
                      ],
                      "recurring_themes": [
                        "Scope discipline",
                        "Operator-facing rationale"
                      ],
                      "decision": "revise"
                    }
                    """
                }

                return """
                {
                  "pass": true,
                  "average_score": 9.2,
                  "aggregate_score": 9.2,
                  "min_individual_score": 9.1,
                  "blocker_count": 0,
                  "summary": "Proposal reaches the approval threshold while leaving one bounded follow-up visible.",
                  "required_changes": [],
                  "recurring_themes": [
                    "Scope is bounded",
                    "Residual operator rationale is explicit"
                  ],
                  "decision": "proceed"
                }
                """
            }
            if case .proposal013AggregateFailure = scenario {
                return """
                # Proposal Review Summary

                Average score: 9.25
                Decision: proceed
                """
            }
            return """
            {
              "pass": true,
              "average_score": 9.25,
              "aggregate_score": 9.25,
              "min_individual_score": 9.1,
              "blocker_count": 0,
              "summary": "Proposal clears the review threshold.",
              "required_changes": [],
              "recurring_themes": ["Scope is clear", "Safety boundary is explicit"],
              "decision": "proceed"
            }
            """
        case "score_lift_backlog":
            if case .proposal022FeedbackCycle = scenario {
                if invocation == 1 {
                    return """
                    {
                      "review_pass_id": "review-pass-1",
                      "review_iteration_id": "state_3_proposal_reviewed.1",
                      "source_proposal_artifact": "proposal_current",
                      "proposal_byte_size": 1680,
                      "previous_proposal_byte_size": 1240,
                      "proposal_growth_ratio": 1.35,
                      "score_delta_since_last_review": 0.20,
                      "backlog_items_closed_count": 0,
                      "reopened_item_count": 0,
                      "growth_guard_recommendation": "replan_or_split_required",
                      "bounded_next_action": "targeted_rereview",
                      "unresolved_high_lift_count": 1,
                      "reviewer_scope_summary": {
                        "full_rerun": [],
                        "delta_rerun": ["proposal_reviewer_product_owner", "proposal_reviewer_architect"],
                        "verification_only": ["proposal_reviewer_ui", "proposal_reviewer_ux"]
                      },
                      "items": [
                        {
                          "id": "scope-boundary",
                          "review_pass_id": "review-pass-1",
                          "source_reviewer": "proposal_reviewer_product_owner",
                          "severity": "high",
                          "blocker": true,
                          "category": "scope",
                          "score_impact_class": "high_lift",
                          "description": "The proposal still mixes MVP and future roadmap work.",
                          "evidence_refs": ["proposal_review_po"],
                          "status": "open",
                          "writer_coverage_ref": null,
                          "last_touched_iteration": 1
                        },
                        {
                          "id": "operator-rationale",
                          "review_pass_id": "review-pass-1",
                          "source_reviewer": "proposal_reviewer_ux",
                          "severity": "medium",
                          "blocker": false,
                          "category": "ux",
                          "score_impact_class": "medium_lift",
                          "description": "Operator-facing unresolved rationale must stay visible after reruns.",
                          "evidence_refs": ["proposal_review_ux"],
                          "status": "open",
                          "merge_provenance": {
                            "merged_issue_refs": ["proposal_review_ux:operator-rationale", "proposal_review_ui:blocked-state-context"],
                            "rationale": "Collapsed overlapping operator-context findings into one carry-forward backlog item."
                          },
                          "writer_coverage_ref": null,
                          "last_touched_iteration": 1
                        }
                      ]
                    }
                    """
                }

                return """
                {
                  "review_pass_id": "review-pass-2",
                  "review_iteration_id": "state_3_proposal_reviewed.2",
                  "source_proposal_artifact": "proposal_current",
                  "proposal_byte_size": 1848,
                  "previous_proposal_byte_size": 1680,
                  "proposal_growth_ratio": 1.10,
                  "score_delta_since_last_review": 1.90,
                  "backlog_items_closed_count": 1,
                  "reopened_item_count": 0,
                  "growth_guard_recommendation": "within_budget",
                  "bounded_next_action": "targeted_architecture_rerun",
                  "unresolved_high_lift_count": 0,
                  "reviewer_scope_summary": {
                    "full_rerun": [],
                    "delta_rerun": ["proposal_reviewer_architect"],
                    "verification_only": ["proposal_reviewer_product_owner", "proposal_reviewer_ui", "proposal_reviewer_ux"]
                  },
                      "items": [
                        {
                          "id": "operator-rationale",
                          "review_pass_id": "review-pass-2",
                          "source_reviewer": "proposal_reviewer_architect",
                      "severity": "low",
                      "blocker": false,
                          "category": "operator_context",
                          "score_impact_class": "low_lift",
                          "description": "Residual operator rationale remains visible as a bounded follow-up.",
                          "evidence_refs": ["proposal_review_architect"],
                          "status": "partially_resolved",
                          "merge_provenance": {
                            "merged_issue_refs": ["proposal_review_architect:operator-rationale", "proposal_review_ux:operator-rationale"],
                            "rationale": "Merged architecture and UX rationale threads into a single bounded follow-up item."
                          },
                          "writer_coverage_ref": "proposal_feedback_coverage",
                          "last_touched_iteration": 2
                        }
                      ]
                    }
                """
            }
        case "review_corpus_bundle":
            if case .proposal022FeedbackCycle = scenario {
                return """
                {
                  "review_pass_id": "review-pass-\(max(invocation, 1))",
                  "review_iteration_id": "state_3_proposal_reviewed.\(max(invocation, 1))",
                  "source_proposal_artifact": "proposal_current",
                  "raw_review_artifacts": [
                    "proposal_review_po",
                    "proposal_review_ux",
                    "proposal_review_ui",
                    "proposal_review_architect"
                  ],
                  "aggregate_summary_artifact": "proposal_review_summary"
                }
                """
            }
        case "proposal_fact_digest":
            if case .proposal022FeedbackCycle = scenario {
                return """
                {
                  "proposal_revision_id": "proposal-revision-\(max(invocation, 1))",
                  "claims": [
                    {
                      "claim_id": "mvp-scope",
                      "statement": "The MVP excludes future roadmap automation.",
                      "evidence_refs": ["proposal_current"],
                      "verification_state": "verified"
                    }
                  ]
                }
                """
            }
        case "reviewer_scope_plan":
            if case .proposal022FeedbackCycle = scenario {
                if invocation == 1 {
                    return """
                    {
                      "full_rerun": [],
                      "delta_rerun": ["proposal_reviewer_product_owner", "proposal_reviewer_architect"],
                      "verification_only": ["proposal_reviewer_ui", "proposal_reviewer_ux"],
                      "rationale": "Scope and architecture need focused rereview; UI/UX only need verification."
                    }
                    """
                }

                return """
                {
                  "full_rerun": [],
                  "delta_rerun": ["proposal_reviewer_architect"],
                  "verification_only": ["proposal_reviewer_product_owner", "proposal_reviewer_ui", "proposal_reviewer_ux"],
                  "rationale": "Only architecture needs a targeted delta check after the refine pass."
                }
                """
            }
        case "proposal_feedback_coverage":
            if case .proposal022FeedbackCycle = scenario {
                return """
                {
                  "proposal_revision_id": "proposal-revision-1",
                  "source_review_pass_id": "review-pass-1",
                  "backlog_items_addressed": ["scope-boundary"],
                  "backlog_items_unresolved": ["operator-rationale"],
                  "backlog_items_deferred": [],
                  "backlog_items_disputed": [],
                  "sections_changed": ["MVP scope", "Operator recovery"],
                  "factual_claims_added_or_corrected": ["The MVP excludes future roadmap automation."],
                  "notes": "Scope was tightened; operator rationale remains explicit for targeted rerun."
                }
                """
            }
        case "implementation_self_assessment":
            return """
            {
              "seemingly_complete": true,
              "remaining_tasks": [],
              "known_risks": [],
              "tests_run": true,
              "docs_impacted": ["README.md"]
            }
            """
        case "tests_result":
            return """
            {
              "green": true,
              "passed": 12,
              "failed": 0,
              "summary": "Fixture repo-backed tests are green."
            }
            """
        case "security_report":
            return """
            {
              "status": "pass",
              "critical": 0,
              "high": 0,
              "medium": 0,
              "low": 0,
              "findings": [],
              "required_fixes": []
            }
            """
        case "docs_report":
            return """
            {
              "status": "pass",
              "changed_docs": ["README.md"],
              "missing_docs": [],
              "followups": []
            }
            """
        case "docs_delta":
            return """
            # Docs Delta

            Updated documentation to match the implementation changes.
            """
        case "audit_report":
            if case .fullMVPRefineThenSuccess = scenario,
               taskName == "audit_implementation_against_proposal",
               invocation == 1 {
                return """
                {
                  "status": "Needs Work",
                  "matches_proposal": false,
                  "missing_items": [],
                  "extra_items": [],
                  "defects": ["Refine implementation once more before release."],
                  "required_fixes": ["Apply follow-up implementation pass"]
                }
                """
            }
            return """
            {
              "status": "Implemented",
              "matches_proposal": true,
              "missing_items": [],
              "extra_items": [],
              "defects": [],
              "required_fixes": []
            }
            """
        case "implementation_review_summary":
            if case .fullMVPRefineThenSuccess = scenario,
               taskName == "aggregate_implementation_reviews",
               invocation == 1 {
                return """
                {
                  "status": "Needs Work",
                  "open_blockers": 1,
                  "must_fix": ["Apply follow-up implementation pass"],
                  "recommended_next_step": "refine_implementation"
                }
                """
            }
            return """
            {
              "status": "Implemented",
              "open_blockers": 0,
              "must_fix": [],
              "recommended_next_step": "proceed_to_release"
            }
            """
        case "prepush_review_report":
            return """
            {
              "status": "pass",
              "major_concerns": [],
              "cleanup_items": [],
              "test_coverage_notes": "Fixture proof covers the repo-backed happy path.",
              "release_note": "Ready for manual release."
            }
            """
        case "release_manifest":
            return """
            {
              "commit_sha": "fixture-commit-sha",
              "branch": "chainworks/cw-fixture-release",
              "remote": "origin",
              "commit_message": "Fixture delivery commit",
              "files_changed": 1,
              "insertions": 12,
              "deletions": 0,
              "timestamp": "\(ISO8601DateFormatter().string(from: Date()))"
            }
            """
        case "git_push_receipt":
            return """
            {
              "status": "success",
              "branch": "chainworks/cw-fixture-release",
              "commit_sha": "fixture-commit-sha",
              "remote": "origin"
            }
            """
        case "release_bundle_manifest":
            return """
            {
              "status": "success",
              "bundle_path": "Build/Fixture/ChainworksForge.xcarchive",
              "distribution_target": "sandbox",
              "timestamp": "\(ISO8601DateFormatter().string(from: Date()))"
            }
            """
        case "connect_upload_receipt":
            return """
            {
              "status": "success",
              "artifact_id": "fixture-upload-artifact",
              "channel": "sandbox"
            }
            """
        case "changed_files_manifest":
            return """
            {
              "files": ["CHAINWORKS_DOGFOOD_PROOF.txt"],
              "summary": "Fixture implementation touched one worktree-scoped file."
            }
            """
        default:
            break
        }

        switch taskName {
        case "normalize_idea_and_prepare_proposal_brief":
            return """
            # Idea Brief

            Agent: \(agentID)
            Output: \(outputName)
            The idea is normalized and ready for proposal drafting.
            """
        case "draft_initial_proposal", "refine_proposal_based_on_review":
            if case .proposal022FeedbackCycle = scenario, outputName == "proposal_current" {
                return """
                # Proposal Draft

                ## MVP Scope
                This revision narrows the MVP to the bounded operator shell and keeps future automation out of scope.

                ## Open Follow-up
                One residual operator-rationale note remains explicit for the next targeted review pass.
                """
            }
            if outputName == "proposal_revision_summary" {
                return """
                # Proposal Revision Summary

                Updated from reviewer feedback.
                """
            }
            return """
            # Proposal Draft

            This proposal is ready for review and approval.
            """
        case "review_proposal_from_product_perspective":
            return reviewerJSON(agentID: agentID, role: "product_owner", score: 9.4)
        case "review_proposal_from_ux_perspective":
            return reviewerJSON(agentID: agentID, role: "ux", score: 9.2)
        case "review_proposal_from_ui_perspective":
            return reviewerJSON(agentID: agentID, role: "ui", score: 9.1)
        case "review_proposal_from_architecture_perspective":
            return reviewerJSON(agentID: agentID, role: "architect", score: 9.3)
        case "aggregate_proposal_reviews":
            return """
            {
              "pass": true,
              "average_score": 9.25,
              "aggregate_score": 9.25,
              "min_individual_score": 9.1,
              "blocker_count": 0,
              "summary": "Proposal passes the review target.",
              "required_changes": [],
              "recurring_themes": [
                "Scope is clear",
                "Approval context is strong"
              ],
              "decision": "proceed"
            }
            """
        default:
            return """
            # Fixture Output

            Task: \(taskName)
            Agent: \(agentID)
            Output: \(outputName)
            """
        }
    }

    private func shouldWriteToWorktree(taskName: String, request: GooseSessionRequest?) -> Bool {
        guard request?.executionPolicy?.repoWritesAllowed == true else { return false }
        return taskName == "initial_implementation"
            || taskName == "continue_implementation"
            || taskName == "refine_implementation"
    }

    private func nextInvocation(for taskName: String) -> Int {
        stateQueue.sync {
            let next = (taskInvocationCounts[taskName] ?? 0) + 1
            taskInvocationCounts[taskName] = next
            return next
        }
    }

    private func writeFixtureWorktreeChange(taskName: String, workingDirectory: URL) {
        let proofFile = workingDirectory.appendingPathComponent("CHAINWORKS_DOGFOOD_PROOF.txt")
        let line = "\(taskName) @ \(ISO8601DateFormatter().string(from: Date()))\n"
        if FileManager.default.fileExists(atPath: proofFile.path) {
            if let handle = try? FileHandle(forWritingTo: proofFile) {
                try? handle.seekToEnd()
                try? handle.write(contentsOf: Data(line.utf8))
                try? handle.close()
            }
        } else {
            try? Data(line.utf8).write(to: proofFile)
        }
    }

    private func reviewerJSON(agentID: String, role: String, score: Double) -> String {
        """
        {
          "agent_id": "\(agentID)",
          "role": "\(role)",
          "score": \(score),
          "decision": "approve_with_suggestions",
          "verdict": "approve_with_suggestions",
          "summary": "Looks good for the live proposal loop.",
          "issues": [],
          "blocker_count": 0,
          "blocking_issues": [],
          "non_blocking_issues": [
            "Refine final polish before implementation."
          ],
          "suggestions": [
            "Carry the approval summary into the completed run report."
          ],
          "assumptions": [
            "The current app-facing live slice remains read-only."
          ]
        }
        """
    }
}
