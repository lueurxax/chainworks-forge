import Testing
import Foundation
import SwiftData
@testable import Chainworks_Forge

@MainActor
@Suite("Proposal 022")
struct Proposal022Tests {

    private func loadExampleCatalog() throws -> AgentCatalog {
        try loadTestCanonicalCatalog()
    }

    private func loadExampleStewardConfig() throws -> StewardConfig {
        try loadTestStewardConfig()
    }

    private func makeArtifact(
        name: String,
        filePath: String,
        runID: UUID,
        stageID: String,
        contractID: String = "default",
        agentID: String = "system"
    ) -> Artifact {
        Artifact(
            name: name,
            contractID: contractID,
            format: .json,
            filePath: filePath,
            runID: runID,
            stageID: stageID,
            agentID: agentID,
            provider: "system"
        )
    }

    private func makeFixtureExecutionService(context: ModelContext) -> ExecutionService {
        ExecutionService(
            modelContext: context,
            executor: SimulatedAgentExecutor(),
            liveRuntimeConfiguration: LiveRuntimeConfiguration(
                baseURL: URL(string: "https://127.0.0.1:51200")!,
                apiKey: "proposal-022-proof",
                override: nil,
                transportMode: .fixtureProposal022FeedbackCycle,
                transportAPI: .gooseServer
            ),
            notificationService: NotificationService()
        )
    }

    @Test("Proposal 022 fixture cycle emits backlog, coverage, fact digest, and targeted rerun plan artifacts")
    func proposal022FixtureCycleEmitsCanonicalArtifacts() async throws {
        let transport = FixtureGooseTransport(scenario: .proposal022FeedbackCycle)
        let workspace = FileManager.default.temporaryDirectory
            .appendingPathComponent("Proposal022Fixture-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: workspace, withIntermediateDirectories: true)

        let request = GooseSessionRequest(
            systemPrompt: "Proposal 022 proof",
            workingDirectory: workspace.path,
            model: "fixture-model",
            provider: "claude_code",
            executionPolicy: GooseExecutionPolicy(
                permissionProfileID: "fixture-read-only",
                workspaceMode: "artifact_only",
                gitOperationsAllowed: false,
                releaseOperationsAllowed: false,
                repoWritesAllowed: false,
            ),
            metadata: ["agent_id": "lead_orchestrator"]
        )
        let session = try await transport.createSession(request: request)

        let prompt = GoosePromptRequest(content: """
        ## Task: aggregate_proposal_reviews

        ### Expected Outputs
        - proposal_review_summary
        - review_corpus_bundle
        - score_lift_backlog
        - proposal_fact_digest
        - reviewer_scope_plan
        Output directory: \(workspace.path)
        ### Stop Condition
        Stop only after all expected outputs exist.
        """, context: nil)

        var eventKinds: [String] = []
        for try await event in transport.submitPrompt(sessionID: session.sessionId, prompt: prompt) {
            switch event {
            case .finalOutput:
                eventKinds.append("final_output")
            case .textChunk:
                eventKinds.append("text_chunk")
            case .sessionClosed:
                eventKinds.append("session_closed")
            default:
                break
            }
        }

        #expect(FileManager.default.fileExists(atPath: workspace.appendingPathComponent("review_corpus_bundle").path))
        #expect(FileManager.default.fileExists(atPath: workspace.appendingPathComponent("score_lift_backlog").path))
        #expect(FileManager.default.fileExists(atPath: workspace.appendingPathComponent("proposal_fact_digest").path))
        #expect(FileManager.default.fileExists(atPath: workspace.appendingPathComponent("reviewer_scope_plan").path))
        #expect(eventKinds.contains("final_output"))
    }

    @Test("Proposal 022 app harness proves corpus fidelity, backlog persistence, and targeted rereview")
    func proposal022AppHarnessProducesCanonicalProof() async throws {
        let (_, context) = try makeTestModelContainer()
        let executionService = makeFixtureExecutionService(context: context)
        let harness = Proposal022AppProofHarness(modelContext: context, executionService: executionService)

        let (run, _, result) = try await harness.run()

        #expect(result.terminalStatus.isEmpty == false)
        #expect(result.refineCorpusInputCount == 5)
        #expect(result.reviewCorpusBundleExists)
        #expect(result.reviewCorpusBundleConsumed)
        #expect(result.scoreLiftBacklogExists)
        #expect(result.scoreLiftBacklogMergeProvenanceExists)
        #expect(result.proposalFeedbackCoverageExists)
        #expect(result.unresolvedBacklogItemIDs.isEmpty == false)
        #expect(result.targetedRerunRationale.contains("delta") || result.targetedRerunRationale.contains("verification"))
        #expect(result.proofStatus.contains("PASS"))
    }

    @Test("Proposal 022 app harness can persist canonical app-proof export for gate consumption")
    func proposal022AppHarnessPersistsCanonicalProofExport() async throws {
        let (_, context) = try makeTestModelContainer()
        let executionService = makeFixtureExecutionService(context: context)
        let harness = Proposal022AppProofHarness(modelContext: context, executionService: executionService)
        let exportURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("proposal022-app-proof-\(UUID().uuidString).json")

        let export = try await harness.runAndPersist(to: exportURL)
        let data = try Data(contentsOf: exportURL)
        let persisted = try JSONDecoder().decode(Proposal022AppProofExport.self, from: data)

        #expect(export.result.runID == persisted.result.runID)
        #expect(persisted.result.reviewCorpusBundleExists)
        #expect(persisted.result.reviewCorpusBundleConsumed)
        #expect(persisted.result.scoreLiftBacklogExists)
        #expect(persisted.result.scoreLiftBacklogMergeProvenanceExists)
        #expect(persisted.result.proposalFeedbackCoverageExists)
        #expect(persisted.result.proofStatus.contains("PASS"))
        #expect(persisted.summary.reviewCorpusBundlePresent)
        #expect(persisted.summary.mergeProvenanceItemCount > 0)
    }

    @Test("Motivating failure-class replay preserves backlog fidelity and bounds proposal growth")
    func motivatingFailureClassReplayProvesBoundedLoopTruth() async throws {
        let (_, context) = try makeTestModelContainer()
        let executionService = makeFixtureExecutionService(context: context)
        let harness = Proposal022AppProofHarness(modelContext: context, executionService: executionService)

        let (run, summary, result) = try await harness.run()
        let artifacts = run.stageExecutions
            .flatMap(\.agentExecutions)
            .flatMap(\.artifacts)
            .sorted { $0.createdAt < $1.createdAt }

        let firstBacklogArtifact = try #require(
            artifacts.first(where: { $0.name == ProposalLoopFeedbackArtifactName.scoreLiftBacklog })
        )
        let firstBacklogData = try Data(contentsOf: URL(fileURLWithPath: firstBacklogArtifact.filePath))
        let firstBacklogObject = try #require(
            try JSONSerialization.jsonObject(with: firstBacklogData) as? [String: Any]
        )
        let firstItems = try #require(firstBacklogObject["items"] as? [[String: Any]])
        #expect(firstItems.count >= 2)
        #expect(firstItems.allSatisfy { ($0["evidence_refs"] as? [String])?.isEmpty == false })

        #expect(result.refineCorpusInputCount == 5)
        #expect(result.reviewCorpusBundleExists)
        #expect(result.reviewCorpusBundleConsumed)
        #expect(result.scoreLiftBacklogExists)
        #expect(result.scoreLiftBacklogMergeProvenanceExists)
        #expect(result.proposalFeedbackCoverageExists)
        #expect(summary.reviewCorpusBundlePresent)
        #expect(summary.reviewCorpusRawArtifactCount == 4)
        #expect(summary.mergeProvenanceItemCount > 0)
        #expect(summary.targetedReviewerSummary?.contains("delta") == true)
        #expect(summary.proposalGrowthRatio != nil)
        #expect(summary.scoreDeltaSinceLastReview != nil)
        #expect(summary.growthGuardRecommendation != nil)
        #expect(summary.boundedNextAction != nil)
    }

    @Test("Selective strategy retires proposal_review_all and carries full review corpus into refine handoff")
    func selectiveStrategyRetiresProposalReviewAll() throws {
        let config = try loadExampleStewardConfig()
        let profile = try #require(config.contextStrategyProfiles["selective_compression_and_escalation"])
        let policy = try #require(profile.agents["proposal_writer"]?.handoffPolicy)

        #expect(policy.mandatory.contains("proposal_review_po"))
        #expect(policy.mandatory.contains("proposal_review_ux"))
        #expect(policy.mandatory.contains("proposal_review_ui"))
        #expect(policy.mandatory.contains("proposal_review_architect"))
        #expect(policy.mandatory.contains("proposal_review_summary"))
        #expect(policy.mandatory.contains("score_lift_backlog"))
        #expect(policy.summarized.contains("proposal_review_all") == false)
        #expect(policy.lazy.contains("proposal_review_all") == false)
    }

    @Test("Selective writer handoff keeps canonical reviewer-derived refine inputs in both example and fallback configs")
    func selectiveWriterHandoffKeepsCanonicalReviewerTruth() throws {
        let workflow = try loadTestFullMVPLiveWorkflow()
        let refineState = try #require(workflow.states["state_5_proposal_refined"])
        let writerTask = try #require(refineState.run?.sequence?.first(where: { $0.agent == "proposal_writer" }))
        let requiredReviewerInputs: Set<String> = [
            "proposal_review_po",
            "proposal_review_ux",
            "proposal_review_ui",
            "proposal_review_architect",
            "proposal_review_summary",
            "review_corpus_bundle",
            "score_lift_backlog",
            "proposal_fact_digest"
        ]
        let writerInputs = Set(writerTask.inputs ?? [])
        #expect(requiredReviewerInputs.isSubset(of: writerInputs))

        for config in [StewardConfig.defaultConfig, try loadExampleStewardConfig()] {
            let profile = try #require(config.contextStrategyProfiles["selective_compression_and_escalation"])
            let policy = try #require(profile.agents["proposal_writer"]?.handoffPolicy)
            let mandatory = Set(policy.mandatory)

            #expect(requiredReviewerInputs.isSubset(of: mandatory))
            #expect(policy.summarized.contains("proposal_review_all") == false)
            #expect(policy.lazy.contains("proposal_review_all") == false)
        }
    }

    @Test("Proposal writer consumes backlog and emits feedback coverage")
    func proposalWriterConsumesBacklogAndEmitsCoverage() throws {
        let catalog = try loadExampleCatalog()
        let writer = try #require(catalog.agents.first(where: { $0.id == "proposal_writer" }))

        #expect(writer.inputs.contains("proposal_review_po"))
        #expect(writer.inputs.contains("proposal_review_ux"))
        #expect(writer.inputs.contains("proposal_review_ui"))
        #expect(writer.inputs.contains("proposal_review_architect"))
        #expect(writer.inputs.contains("proposal_review_summary"))
        #expect(writer.inputs.contains("review_corpus_bundle"))
        #expect(writer.inputs.contains("score_lift_backlog"))
        #expect(writer.inputs.contains("proposal_fact_digest"))
        #expect(writer.outputs.contains("proposal_feedback_coverage"))
    }

    @Test("Full MVP refine workflow requires raw quartet plus backlog and coverage")
    func fullMVPRefineWorkflowUsesCanonicalArtifacts() throws {
        let workflow = try loadTestFullMVPLiveWorkflow()
        let reviewState = try #require(workflow.states["state_4_proposal_reviewed"])
        let aggregateTask = try #require(reviewState.run?.then?.first(where: { $0.agent == "lead_orchestrator" }))
        #expect(aggregateTask.outputs?.contains("review_corpus_bundle") == true)
        #expect(aggregateTask.outputs?.contains("score_lift_backlog") == true)
        #expect(aggregateTask.outputs?.contains("proposal_fact_digest") == true)

        let refineState = try #require(workflow.states["state_5_proposal_refined"])
        let writerTask = try #require(refineState.run?.sequence?.first(where: { $0.agent == "proposal_writer" }))
        #expect(writerTask.inputs?.contains("proposal_review_po") == true)
        #expect(writerTask.inputs?.contains("proposal_review_ux") == true)
        #expect(writerTask.inputs?.contains("proposal_review_ui") == true)
        #expect(writerTask.inputs?.contains("proposal_review_architect") == true)
        #expect(writerTask.inputs?.contains("proposal_review_summary") == true)
        #expect(writerTask.inputs?.contains("review_corpus_bundle") == true)
        #expect(writerTask.inputs?.contains("score_lift_backlog") == true)
        #expect(writerTask.inputs?.contains("proposal_fact_digest") == true)
        #expect(writerTask.outputs?.contains("proposal_feedback_coverage") == true)
    }

    @Test("Review fanout can consume targeted rereview inputs on later passes")
    func reviewFanoutConsumesTargetedRereviewInputs() throws {
        let workflow = try loadTestFullMVPLiveWorkflow()
        let reviewState = try #require(workflow.states["state_4_proposal_reviewed"])
        let reviewerTasks = try #require(reviewState.run?.parallel)

        for task in reviewerTasks {
            #expect(task.inputs?.contains("proposal_feedback_coverage") == true)
            #expect(task.inputs?.contains("reviewer_scope_plan") == true)
            #expect(task.inputs?.contains("score_lift_backlog") == true)
        }
    }

    @Test("Run report surfaces proposal-loop backlog and coverage truth from canonical artifacts")
    func runReportSurfacesBacklogAndCoverageTruth() throws {
        let (_, context) = try makeTestModelContainer()
        let workspace = makeTestWorkspace()
        let run = makeTestRun(workspace: workspace, context: context, workflowID: "full_mvp_live", workflowTitle: "Full MVP")
        run.status = .blocked

        let reviewStage = StageExecution(stageID: "state_4_proposal_reviewed", label: "Proposal reviewed", status: .completed, iteration: 3, attemptNumber: 1)
        reviewStage.run = run
        context.insert(reviewStage)

        let writerStage = StageExecution(stageID: "state_5_proposal_refined", label: "Proposal refined", status: .failed, iteration: 4, attemptNumber: 1)
        writerStage.run = run
        context.insert(writerStage)

        let reviewAgent = AgentExecution(
            agentID: "lead_orchestrator",
            agentTitle: "Lead / Orchestrator",
            taskName: "aggregate_proposal_reviews",
            status: .completed,
            provider: "claude_code",
            effort: "high"
        )
        reviewAgent.stageExecution = reviewStage
        context.insert(reviewAgent)

        let writerAgent = AgentExecution(
            agentID: "proposal_writer",
            agentTitle: "Proposal Writer",
            taskName: "refine_proposal",
            status: .failed,
            provider: "codex",
            effort: "high"
        )
        writerAgent.stageExecution = writerStage
        context.insert(writerAgent)

        let scoreLiftPath = workspace.artifactRoot.appendingPathComponent("score_lift_backlog.json").path
        let coveragePath = workspace.artifactRoot.appendingPathComponent("proposal_feedback_coverage.json").path
        let summaryPath = workspace.artifactRoot.appendingPathComponent("proposal_review_summary.json").path
        let bundlePath = workspace.artifactRoot.appendingPathComponent("review_corpus_bundle.json").path

        let backlogJSON = """
        {
          "review_pass_id": "review-pass-3",
          "review_iteration_id": "state_3_proposal_reviewed.3",
          "source_proposal_artifact": "proposal_current",
          "proposal_byte_size": 2120,
          "previous_proposal_byte_size": 1660,
          "proposal_growth_ratio": 1.28,
          "score_delta_since_last_review": 0.25,
          "backlog_items_closed_count": 1,
          "reopened_item_count": 0,
          "growth_guard_recommendation": "replan_or_split_required",
          "bounded_next_action": "targeted_rereview",
          "total_items": 3,
          "unresolved_high_lift_count": 2,
          "reviewer_scope_summary": {
            "full_rerun": ["proposal_reviewer_ui"],
            "delta_rerun": ["proposal_reviewer_ux"],
            "verification_only": ["proposal_reviewer_product_owner", "proposal_reviewer_architect"]
          },
          "items": [
            {
              "id": "ui-1",
              "review_pass_id": "review-pass-3",
              "source_reviewer": "proposal_reviewer_ui",
              "severity": "high",
              "blocker": true,
              "category": "ui_state_coverage",
              "score_impact_class": "ceiling_blocker",
              "description": "Missing blocked and loading states",
              "evidence_refs": ["proposal_review_ui"],
              "status": "open",
              "writer_coverage_ref": null,
              "last_touched_iteration": 4
            },
            {
              "id": "ux-1",
              "review_pass_id": "review-pass-3",
              "source_reviewer": "proposal_reviewer_ux",
              "severity": "medium",
              "blocker": false,
              "category": "task_flow",
              "score_impact_class": "high_lift",
              "description": "Recovery flow remains underspecified",
              "evidence_refs": ["proposal_review_ux"],
              "status": "open",
              "merge_provenance": {
                "merged_issue_refs": [
                  "proposal_review_ux:recovery-flow-gap",
                  "proposal_review_ui:blocked-state-gap"
                ],
                "rationale": "Combined overlapping flow and blocked-state findings into a single carry-forward item."
              },
              "writer_coverage_ref": null,
              "last_touched_iteration": 4
            },
            {
              "id": "po-1",
              "review_pass_id": "review-pass-3",
              "source_reviewer": "proposal_reviewer_product_owner",
              "severity": "medium",
              "blocker": false,
              "category": "acceptance",
              "score_impact_class": "medium_lift",
              "description": "Success metrics remain underspecified",
              "evidence_refs": ["proposal_review_po"],
              "status": "deferred",
              "writer_coverage_ref": "proposal_feedback_coverage",
              "last_touched_iteration": 4
            }
          ]
        }
        """

        let coverageJSON = """
        {
          "proposal_revision_id": "proposal-revision-4",
          "source_review_pass_id": "review-pass-3",
          "backlog_items_addressed": ["ui-1"],
          "backlog_items_unresolved": ["ux-1"],
          "backlog_items_deferred": ["po-1"],
          "backlog_items_disputed": [],
          "sections_changed": ["UI states", "Recovery"],
          "factual_claims_added_or_corrected": ["Documented loading and blocked states"],
          "notes": "UI states were added, but recovery flow still needs product clarification."
        }
        """

        let bundleJSON = """
        {
          "review_pass_id": "review-pass-3",
          "review_iteration_id": "state_3_proposal_reviewed.3",
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

        let summaryJSON = """
        {
          "pass": false,
          "average_score": 8.4,
          "aggregate_score": 8.4,
          "min_individual_score": 7.8,
          "blocker_count": 1,
          "summary": "Proposal still needs another refine pass.",
          "required_changes": ["Close remaining recovery and metrics gaps"],
          "recurring_themes": ["State coverage", "Measurement"],
          "decision": "revise"
        }
        """

        try backlogJSON.write(toFile: scoreLiftPath, atomically: true, encoding: .utf8)
        try coverageJSON.write(toFile: coveragePath, atomically: true, encoding: .utf8)
        try bundleJSON.write(toFile: bundlePath, atomically: true, encoding: .utf8)
        try summaryJSON.write(toFile: summaryPath, atomically: true, encoding: .utf8)

        let backlogArtifact = makeArtifact(name: "score_lift_backlog", filePath: scoreLiftPath, runID: run.id, stageID: reviewStage.stageID, agentID: "lead_orchestrator")
        backlogArtifact.agentExecution = reviewAgent
        context.insert(backlogArtifact)

        let bundleArtifact = makeArtifact(name: "review_corpus_bundle", filePath: bundlePath, runID: run.id, stageID: reviewStage.stageID, agentID: "lead_orchestrator")
        bundleArtifact.agentExecution = reviewAgent
        context.insert(bundleArtifact)

        let summaryArtifact = makeArtifact(name: "proposal_review_summary", filePath: summaryPath, runID: run.id, stageID: reviewStage.stageID, agentID: "lead_orchestrator")
        summaryArtifact.agentExecution = reviewAgent
        context.insert(summaryArtifact)

        let coverageArtifact = makeArtifact(name: "proposal_feedback_coverage", filePath: coveragePath, runID: run.id, stageID: writerStage.stageID, agentID: "proposal_writer")
        coverageArtifact.agentExecution = writerAgent
        context.insert(coverageArtifact)

        let payload = RunReportBuilder(modelContext: context).buildReportPayload(for: run, version: 1)

        #expect(payload.proposalLoopSummary != nil)
        #expect(payload.proposalLoopSummary?.reviewCorpusBundlePresent == true)
        #expect(payload.proposalLoopSummary?.reviewCorpusRawArtifactCount == 4)
        #expect(payload.proposalLoopSummary?.backlogItemCount == 3)
        #expect(payload.proposalLoopSummary?.mergeProvenanceItemCount == 1)
        #expect(payload.proposalLoopSummary?.unresolvedItemCount == 2)
        #expect(payload.proposalLoopSummary?.deferredItemCount == 1)
        #expect(payload.proposalLoopSummary?.targetedReviewerSummary == "full: proposal_reviewer_ui; delta: proposal_reviewer_ux; verification: proposal_reviewer_architect, proposal_reviewer_product_owner")
        #expect(payload.proposalLoopSummary?.coverageStatusSummary.contains("addressed 1") == true)
        #expect(payload.proposalLoopSummary?.proposalGrowthRatio == 1.28)
        #expect(payload.proposalLoopSummary?.scoreDeltaSinceLastReview == 0.25)
        #expect(payload.proposalLoopSummary?.growthGuardRecommendation == "replan_or_split_required")
        #expect(payload.proposalLoopSummary?.boundedNextAction == "targeted_rereview")
    }
}
