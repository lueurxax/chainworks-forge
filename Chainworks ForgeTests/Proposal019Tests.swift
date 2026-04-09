import Testing
import Foundation
import SwiftData
@testable import Chainworks_Forge

@MainActor
@Suite("Proposal 019")
struct Proposal019Tests {

    @Test("Run start snapshot freezes selected context strategy into persisted run truth")
    func runStartSnapshotFreezesSelectedContextStrategy() throws {
        let (_, context) = try makeTestModelContainer()
        let workspace = makeTestWorkspace()
        let run = makeTestRun(workspace: workspace, context: context)

        let profile = try #require(
            StewardConfig.defaultConfig.contextStrategyProfiles["selective_compression_and_escalation"]?
                .runtimeProfile(profileID: "selective_compression_and_escalation")
        )
        let snapshot = RunStartSnapshot(
            providerBindingSnapshotJSON: nil,
            bindingProvenanceJSON: nil,
            startOptionsJSON: nil,
            frozenWorkspaceRootPath: nil,
            deliveryConfiguration: nil,
            deliveryPreflightJSON: nil,
            contextStrategyProfileID: "selective_compression_and_escalation",
            strategyAssignmentMode: "manual_override",
            strategyRecommendationState: StrategyRecommendationStatus.notEvaluated.rawValue,
            contextStrategySnapshotJSON: try JSONEncoder().encode(profile)
        )

        snapshot.apply(to: run)
        let roundTrip = RunStartSnapshot.from(run: run)

        #expect(run.contextStrategyProfileID == "selective_compression_and_escalation")
        #expect(run.strategyAssignmentMode == "manual_override")
        #expect(roundTrip.contextStrategyProfileID == "selective_compression_and_escalation")
        #expect(roundTrip.strategyAssignmentMode == "manual_override")
        let restored = try JSONDecoder().decode(ContextStrategyProfile.self, from: try #require(roundTrip.contextStrategySnapshotJSON))
        #expect(restored.defaultHandoffMode == .selective)
    }

    @Test("Handoff compiler compiles selective compression into packet-friendly strategy payload")
    func handoffCompilerCompilesSelectiveCompression() throws {
        let profile = try #require(StewardConfig.defaultConfig.contextStrategyProfiles["selective_compression_and_escalation"])
        let compiler = HandoffCompiler()
        let context = makeTestExecutionContext(
            inputArtifacts: [
                "idea_brief": Data("short idea".utf8),
                "proposal_current": Data("full proposal body".utf8),
                "proposal_review_po": Data("{}".utf8),
                "proposal_review_ux": Data("{}".utf8),
                "proposal_review_ui": Data("{}".utf8),
                "proposal_review_architect": Data("{}".utf8),
                "proposal_review_summary": Data(String(repeating: "review ", count: 80).utf8),
                "score_lift_backlog": Data("{}".utf8),
                "security_audit_raw": Data("sensitive raw audit".utf8)
            ]
        )
        let agent = makeTestAgent(
            id: "proposal_writer",
            outputs: ["proposal_current", "proposal_revision_summary"]
        )
        let task = makeTestTask(
            agent: "proposal_writer",
            task: "refine_proposal",
            inputs: [
                "idea_brief",
                "proposal_current",
                "proposal_review_po",
                "proposal_review_ux",
                "proposal_review_ui",
                "proposal_review_architect",
                "proposal_review_summary",
                "score_lift_backlog",
                "security_audit_raw"
            ],
            outputs: ["proposal_current", "proposal_revision_summary"]
        )

        let handoff = compiler.compile(
            profileID: "selective_compression_and_escalation",
            profile: profile,
            agent: agent,
            task: task,
            context: context
        )

        #expect(handoff.profileID == "selective_compression_and_escalation")
        #expect(handoff.mode == .selective)
        #expect(handoff.mandatoryArtifacts.keys.sorted() == [
            "idea_brief",
            "proposal_current",
            "proposal_review_architect",
            "proposal_review_po",
            "proposal_review_summary",
            "proposal_review_ui",
            "proposal_review_ux",
            "score_lift_backlog"
        ])
        #expect(handoff.summaries.keys.isEmpty)
        #expect(handoff.lazyArtifactRefs.keys.sorted() == ["security_audit_raw"])
        #expect(handoff.summaryMetrics.compactionCount == 0)
        #expect(handoff.summaryMetrics.lazyArtifactCount == 1)
    }

    @Test("Operator-promoted artifacts become mandatory in strategy handoff")
    func promotedArtifactsBecomeMandatoryInHandoffPacket() throws {
        let profile = try #require(StewardConfig.defaultConfig.contextStrategyProfiles["selective_compression_and_escalation"])
        let compiler = HandoffCompiler()
        let context = makeTestExecutionContext(
            inputArtifacts: [
                "idea_brief": Data("short idea".utf8),
                "proposal_current": Data("full proposal body".utf8),
                "proposal_review_po": Data("{}".utf8),
                "proposal_review_ux": Data("{}".utf8),
                "proposal_review_ui": Data("{}".utf8),
                "proposal_review_architect": Data("{}".utf8),
                "proposal_review_summary": Data("review summary".utf8),
                "score_lift_backlog": Data("{}".utf8),
                "security_audit_raw": Data("sensitive raw audit".utf8)
            ]
        )
        let agent = makeTestAgent(
            id: "proposal_writer",
            outputs: ["proposal_current", "proposal_revision_summary"]
        )
        let task = makeTestTask(
            agent: "proposal_writer",
            task: "refine_proposal",
            inputs: [
                "idea_brief",
                "proposal_current",
                "proposal_review_po",
                "proposal_review_ux",
                "proposal_review_ui",
                "proposal_review_architect",
                "proposal_review_summary",
                "score_lift_backlog",
                "security_audit_raw"
            ],
            outputs: ["proposal_current", "proposal_revision_summary"]
        )

        let handoff = compiler.compile(
            profileID: "selective_compression_and_escalation",
            profile: profile,
            agent: agent,
            task: task,
            context: context,
            promotedArtifacts: ["security_audit_raw"]
        )

        #expect(handoff.mandatoryArtifacts["security_audit_raw"] != nil)
        #expect(handoff.lazyArtifactRefs["security_audit_raw"] == nil)
        #expect(handoff.promotedArtifacts == ["security_audit_raw"])
    }

    @Test("Proposal reviewers receive declared review inputs as mandatory evidence instead of lazy refs")
    func proposalReviewersPromoteDeclaredInputsOutOfLazyContext() throws {
        let profile = try #require(StewardConfig.defaultConfig.contextStrategyProfiles["selective_compression_and_escalation"])
        let compiler = HandoffCompiler()
        let context = makeTestExecutionContext(
            inputArtifacts: [
                "idea_brief": Data("brief".utf8),
                "proposal_current": Data(String(repeating: "proposal ", count: 300).utf8),
                "proposal_feedback_coverage": Data("{\"coverage\":\"full\"}".utf8),
                "reviewer_scope_plan": Data("{\"scope\":\"review\"}".utf8),
                "score_lift_backlog": Data("{\"items\":[]}".utf8),
                "security_audit_raw": Data("sensitive raw audit".utf8)
            ]
        )
        let agent = makeTestAgent(
            id: "proposal_reviewer_product_owner",
            mode: "proposal_review.product_owner",
            outputs: ["proposal_review_po"]
        )
        let task = makeTestTask(
            agent: "proposal_reviewer_product_owner",
            task: "review_proposal_as_product_owner",
            inputs: [
                "idea_brief",
                "proposal_current",
                "proposal_feedback_coverage",
                "reviewer_scope_plan",
                "score_lift_backlog"
            ],
            outputs: ["proposal_review_po"]
        )

        let handoff = compiler.compile(
            profileID: "selective_compression_and_escalation",
            profile: profile,
            agent: agent,
            task: task,
            context: context
        )

        #expect(handoff.mandatoryArtifacts.keys.sorted() == [
            "idea_brief",
            "proposal_current",
            "proposal_feedback_coverage",
            "reviewer_scope_plan",
            "score_lift_backlog"
        ])
        #expect(handoff.lazyArtifactRefs["idea_brief"] == nil)
        #expect(handoff.lazyArtifactRefs["proposal_current"] == nil)
        #expect(handoff.lazyArtifactRefs["proposal_feedback_coverage"] == nil)
        #expect(handoff.lazyArtifactRefs["reviewer_scope_plan"] == nil)
        #expect(handoff.lazyArtifactRefs["score_lift_backlog"] == nil)
        #expect(handoff.lazyArtifactRefs["security_audit_raw"] != nil)
    }

    @Test("Lead orchestrator receives aggregate proposal review inputs as mandatory evidence instead of lazy refs")
    func leadOrchestratorPromotesAggregateReviewInputsOutOfLazyContext() throws {
        let profile = try #require(StewardConfig.defaultConfig.contextStrategyProfiles["selective_compression_and_escalation"])
        let compiler = HandoffCompiler()
        let context = makeTestExecutionContext(
            inputArtifacts: [
                "proposal_review_po": Data("{\"decision\":\"approve\"}".utf8),
                "proposal_review_ux": Data("{\"decision\":\"approve\"}".utf8),
                "proposal_review_ui": Data("{\"decision\":\"approve\"}".utf8),
                "proposal_review_architect": Data("{\"decision\":\"approve\"}".utf8),
                "proposal_current": Data(String(repeating: "proposal ", count: 400).utf8),
                "security_audit_raw": Data("sensitive raw audit".utf8)
            ]
        )
        let agent = makeTestAgent(
            id: "lead_orchestrator",
            mode: "orchestration",
            outputs: [
                "proposal_review_summary",
                "review_corpus_bundle",
                "score_lift_backlog",
                "proposal_fact_digest",
                "reviewer_scope_plan",
                "orchestrator_summary",
                "run_state"
            ]
        )
        let task = makeTestTask(
            agent: "lead_orchestrator",
            task: "aggregate_proposal_reviews",
            inputs: [
                "proposal_review_po",
                "proposal_review_ux",
                "proposal_review_ui",
                "proposal_review_architect",
                "proposal_current"
            ],
            outputs: [
                "proposal_review_summary",
                "review_corpus_bundle",
                "score_lift_backlog",
                "proposal_fact_digest",
                "reviewer_scope_plan",
                "orchestrator_summary",
                "run_state"
            ]
        )

        let handoff = compiler.compile(
            profileID: "selective_compression_and_escalation",
            profile: profile,
            agent: agent,
            task: task,
            context: context
        )

        #expect(handoff.mandatoryArtifacts.keys.sorted() == [
            "proposal_current",
            "proposal_review_architect",
            "proposal_review_po",
            "proposal_review_ui",
            "proposal_review_ux"
        ])
        #expect(handoff.lazyArtifactRefs["proposal_review_po"] == nil)
        #expect(handoff.lazyArtifactRefs["proposal_review_ux"] == nil)
        #expect(handoff.lazyArtifactRefs["proposal_review_ui"] == nil)
        #expect(handoff.lazyArtifactRefs["proposal_review_architect"] == nil)
        #expect(handoff.lazyArtifactRefs["proposal_current"] == nil)
        #expect(handoff.lazyArtifactRefs["security_audit_raw"] != nil)
    }

    @Test("Proposal-loop comparison tracks feedback carry-forward and rationale")
    func proposalLoopComparisonTracksFeedbackCarryForwardAndRationale() throws {
        let (_, context) = try makeTestModelContainer()
        let workspaceA = makeTestWorkspace()
        let workspaceB = makeTestWorkspace()
        let idea = Idea(title: "Proposal-loop comparison", body: "Test")
        context.insert(idea)

        let runA = makeTestRun(
            workspace: workspaceA,
            context: context,
            ideaTitle: "Proposal-loop comparison",
            ideaBody: "Test",
            workflowID: "proposal_loop_live",
            workflowTitle: "Proposal Loop"
        )
        runA.idea = idea
        runA.workflowFamily = "proposal_loop_live"
        runA.contextStrategyProfileID = "current_mixed_baseline"
        runA.strategyAssignmentMode = "default"

        let runB = makeTestRun(
            workspace: workspaceB,
            context: context,
            ideaTitle: "Proposal-loop comparison",
            ideaBody: "Test",
            workflowID: "proposal_loop_live",
            workflowTitle: "Proposal Loop"
        )
        runB.idea = idea
        runB.workflowFamily = "proposal_loop_live"
        runB.contextStrategyProfileID = "selective_compression_and_escalation"
        runB.strategyAssignmentMode = "manual_override"

        context.insert(runA)
        context.insert(runB)

        let stageA = StageExecution(
            stageID: "state_5_proposal_refined",
            label: "Proposal refined",
            status: .completed,
            iteration: 5,
            attemptNumber: 1
        )
        stageA.run = runA
        context.insert(stageA)

        let stageB = StageExecution(
            stageID: "state_5_proposal_refined",
            label: "Proposal refined",
            status: .completed,
            iteration: 5,
            attemptNumber: 1
        )
        stageB.run = runB
        context.insert(stageB)

        let agentA = AgentExecution(
            agentID: "lead_orchestrator",
            agentTitle: "Lead / Orchestrator",
            taskName: "aggregate_proposal_reviews",
            status: .completed,
            provider: "claude_code",
            effort: "high"
        )
        agentA.stageExecution = stageA
        context.insert(agentA)

        let agentB = AgentExecution(
            agentID: "lead_orchestrator",
            agentTitle: "Lead / Orchestrator",
            taskName: "aggregate_proposal_reviews",
            status: .completed,
            provider: "claude_code",
            effort: "high"
        )
        agentB.stageExecution = stageB
        context.insert(agentB)

        let aBacklogPath = workspaceA.artifactRoot.appendingPathComponent("score_lift_backlog.json").path
        let aCoveragePath = workspaceA.artifactRoot.appendingPathComponent("proposal_feedback_coverage.json").path
        let bBacklogPath = workspaceB.artifactRoot.appendingPathComponent("score_lift_backlog.json").path
        let bCoveragePath = workspaceB.artifactRoot.appendingPathComponent("proposal_feedback_coverage.json").path

        let runABacklogJSON = """
        {
          "review_pass_id": "pass-a",
          "source_proposal_artifact": "proposal_current",
          "items": [
            {"id":"ui-1","status":"open"},
            {"id":"ux-1","status":"open"},
            {"id":"po-1","status":"deferred"}
          ],
          "reviewer_scope_summary": {
            "full_rerun": ["proposal_reviewer_ui"]
          }
        }
        """

        let runACoverageJSON = """
        {
          "proposal_revision_id": "rev-a",
          "source_review_pass_id": "pass-a",
          "backlog_items_addressed": ["ui-1"],
          "backlog_items_unresolved": ["ux-1"],
          "backlog_items_deferred": ["po-1"],
          "backlog_items_disputed": [],
          "sections_changed": ["Initial pass"],
          "factual_claims_added_or_corrected": ["Added UI backlog handling"]
        }
        """

        let runBBacklogJSON = """
        {
          "review_pass_id": "pass-b",
          "source_proposal_artifact": "proposal_current",
          "items": [
            {"id":"ui-1","status":"open"},
            {"id":"ux-1","status":"open"},
            {"id":"po-1","status":"open"},
            {"id":"ux-2","status":"deferred"}
          ],
          "reviewer_scope_summary": {
            "full_rerun": ["proposal_reviewer_ui"],
            "verification_only": ["proposal_reviewer_architect"]
          }
        }
        """

        let runBCoverageJSON = """
        {
          "proposal_revision_id": "rev-b",
          "source_review_pass_id": "pass-b",
          "backlog_items_addressed": ["ui-1", "ux-2"],
          "backlog_items_unresolved": ["ux-1"],
          "backlog_items_deferred": ["po-1"],
          "backlog_items_disputed": [],
          "sections_changed": ["Improved architecture review"],
          "factual_claims_added_or_corrected": ["Addressed verification scope"]
        }
        """

        try runABacklogJSON.write(toFile: aBacklogPath, atomically: true, encoding: .utf8)
        try runACoverageJSON.write(toFile: aCoveragePath, atomically: true, encoding: .utf8)
        try runBBacklogJSON.write(toFile: bBacklogPath, atomically: true, encoding: .utf8)
        try runBCoverageJSON.write(toFile: bCoveragePath, atomically: true, encoding: .utf8)

        let artifactA1 = Artifact(
            name: "score_lift_backlog",
            contractID: "score_lift_backlog",
            format: .json,
            filePath: aBacklogPath,
            runID: runA.id,
            stageID: stageA.stageID,
            agentID: "lead_orchestrator",
            provider: "system"
        )
        artifactA1.agentExecution = agentA
        context.insert(artifactA1)

        let artifactA2 = Artifact(
            name: "proposal_feedback_coverage",
            contractID: "proposal_feedback_coverage",
            format: .json,
            filePath: aCoveragePath,
            runID: runA.id,
            stageID: stageA.stageID,
            agentID: "proposal_writer",
            provider: "system"
        )
        artifactA2.agentExecution = agentA
        context.insert(artifactA2)

        let artifactB1 = Artifact(
            name: "score_lift_backlog",
            contractID: "score_lift_backlog",
            format: .json,
            filePath: bBacklogPath,
            runID: runB.id,
            stageID: stageB.stageID,
            agentID: "lead_orchestrator",
            provider: "system"
        )
        artifactB1.agentExecution = agentB
        context.insert(artifactB1)

        let artifactB2 = Artifact(
            name: "proposal_feedback_coverage",
            contractID: "proposal_feedback_coverage",
            format: .json,
            filePath: bCoveragePath,
            runID: runB.id,
            stageID: stageB.stageID,
            agentID: "proposal_writer",
            provider: "system"
        )
        artifactB2.agentExecution = agentB
        context.insert(artifactB2)

        let comparison = try #require(RunComparisonService(modelContext: context).compare(runA, runB))
        let proposalDelta = comparison.proposalLoopComparison

        #expect(proposalDelta.backlogItemCountA == 3)
        #expect(proposalDelta.backlogItemCountB == 4)
        #expect(proposalDelta.unresolvedItemCountA == 2)
        #expect(proposalDelta.unresolvedItemCountB == 3)
        #expect(proposalDelta.unresolvedDelta == 1)
        #expect(proposalDelta.coverageDelta == 1)
        #expect(proposalDelta.targetedRereviewRationaleA?.contains("full: proposal_reviewer_ui") == true)
        #expect(proposalDelta.targetedRereviewRationaleB?.contains("verification: proposal_reviewer_architect") == true)
        #expect(proposalDelta.rationale == "Run B has higher unresolved backlog and likely requires narrower rerun than before.")
    }

    @Test("Strategy comparison degrades to insufficient evidence without canonical telemetry set")
    func strategyComparisonRequiresCanonicalEvidenceBeforeRecommendation() throws {
        let (_, context) = try makeTestModelContainer()
        let idea = Idea(title: "Strategy compare", body: "Test")
        context.insert(idea)

        let runA = Run(
            workflowID: "proposal_loop_live",
            workflowTitle: "Proposal Loop",
            workflowSnapshotHash: "wf",
            catalogSnapshotHash: "cat",
            workflowSourcePath: "workflow.yaml",
            catalogSourcePath: "agents.yaml",
            workflowSnapshotJSON: Data("{}".utf8),
            catalogSnapshotJSON: Data("{}".utf8)
        )
        runA.idea = idea
        runA.contextStrategyProfileID = "current_mixed_baseline"
        runA.strategyAssignmentMode = "default"

        let runB = Run(
            workflowID: "proposal_loop_live",
            workflowTitle: "Proposal Loop",
            workflowSnapshotHash: "wf",
            catalogSnapshotHash: "cat",
            workflowSourcePath: "workflow.yaml",
            catalogSourcePath: "agents.yaml",
            workflowSnapshotJSON: Data("{}".utf8),
            catalogSnapshotJSON: Data("{}".utf8)
        )
        runB.idea = idea
        runB.contextStrategyProfileID = "selective_compression_and_escalation"
        runB.strategyAssignmentMode = "manual_override"

        context.insert(runA)
        context.insert(runB)

        let comparison = try #require(RunComparisonService(modelContext: context).compare(runA, runB))
        let recommendation = try #require(comparison.strategyRecommendation)

        #expect(recommendation.status == .insufficientEvidence)
        #expect(recommendation.proofOwner == "shell_comparison_lane")
        #expect(recommendation.evaluationSetComplete == false)
    }

    @Test("Strategy comparison requires canonical strategy telemetry fields, not only coarse KPI totals")
    func strategyComparisonRequiresCanonicalStrategyTelemetry() throws {
        let (_, context) = try makeTestModelContainer()
        let idea = Idea(title: "Strategy telemetry", body: "Test")
        context.insert(idea)

        let runA = Run(
            workflowID: "proposal_loop_live",
            workflowTitle: "Proposal Loop",
            workflowSnapshotHash: "wf",
            catalogSnapshotHash: "cat",
            workflowSourcePath: "workflow.yaml",
            catalogSourcePath: "agents.yaml",
            workflowSnapshotJSON: Data("{}".utf8),
            catalogSnapshotJSON: Data("{}".utf8)
        )
        runA.idea = idea
        runA.contextStrategyProfileID = "current_mixed_baseline"
        runA.strategyAssignmentMode = "default"
        runA.strategyRecommendationState = StrategyRecommendationStatus.notEvaluated.rawValue
        runA.sessionKPIExportJSON = Data(#"{"runID":"A","totalExecutions":4}"#.utf8)

        let runB = Run(
            workflowID: "proposal_loop_live",
            workflowTitle: "Proposal Loop",
            workflowSnapshotHash: "wf",
            catalogSnapshotHash: "cat",
            workflowSourcePath: "workflow.yaml",
            catalogSourcePath: "agents.yaml",
            workflowSnapshotJSON: Data("{}".utf8),
            catalogSnapshotJSON: Data("{}".utf8)
        )
        runB.idea = idea
        runB.contextStrategyProfileID = "selective_compression_and_escalation"
        runB.strategyAssignmentMode = "manual_override"
        runB.strategyRecommendationState = StrategyRecommendationStatus.notEvaluated.rawValue
        runB.sessionKPIExportJSON = Data(#"{"runID":"B","totalExecutions":4}"#.utf8)

        context.insert(runA)
        context.insert(runB)

        let comparison = try #require(RunComparisonService(modelContext: context).compare(runA, runB))
        let recommendation = try #require(comparison.strategyRecommendation)

        #expect(recommendation.status == .insufficientEvidence)
        #expect(recommendation.evaluationSetComplete == false)
        #expect(comparison.strategyComparison.evidenceComplete == false)
    }

    @Test("KPI export extends canonical run telemetry with strategy signals")
    func kpiExportIncludesStrategyTelemetrySummary() throws {
        let (_, context) = try makeTestModelContainer()
        let workspace = makeTestWorkspace()
        let run = makeTestRun(workspace: workspace, context: context)
        run.promotedHandoffArtifactsJSON = try JSONEncoder().encode(["security_audit_raw"])

        let stage = StageExecution(stageID: "state_4_proposal_reviewed", label: "Proposal reviewed", status: .completed)
        stage.run = run
        context.insert(stage)

        let execution = AgentExecution(
            agentID: "lead_orchestrator",
            agentTitle: "Lead / Orchestrator",
            taskName: "aggregate_proposal_reviews",
            status: .completed,
            provider: "claude_code",
            effort: "high"
        )
        execution.stageExecution = stage
        execution.limitPressureSignalsJSON = try JSONEncoder().encode(
            StrategyLimitPressureSignals(
                inputPayloadBytes: 1200,
                payloadBytesBeforeStrategy: 2400,
                payloadBytesAfterStrategy: 1200,
                payloadReductionBytes: 1200,
                mandatoryArtifactCount: 2,
                summarizedArtifactCount: 1,
                lazyArtifactCount: 3,
                lazyEvidenceHitCount: 2,
                lazyEvidenceHitRate: 2.0 / 3.0,
                compactionCount: 1,
                cacheEffectiveness: 0.0,
                compactionChurnCount: 0,
                escalationCount: 0,
                retryableEscalationCount: 0,
                contractFailureCount: 0,
                operatorPromotedArtifactCount: 1
            )
        )
        context.insert(execution)

        let summary = SessionReuseKPIExporter.exportKPIs(for: run.id, context: context)

        #expect(summary.strategyTelemetry.totalPayloadReductionBytes == 1200)
        #expect(summary.strategyTelemetry.averageLazyArtifactCount == 3.0)
        #expect(summary.strategyTelemetry.totalLazyEvidenceHitCount == 2)
        #expect(summary.strategyTelemetry.averageLazyEvidenceHitRate == 2.0 / 3.0)
        #expect(summary.strategyTelemetry.operatorPromotedArtifactCount == 1)
        #expect(summary.strategyTelemetry.totalPromotedArtifactUsages == 1)
    }

    @Test("Experiment coordinator assigns a deterministic cohort profile from frozen steward config")
    func experimentCoordinatorAssignsDeterministicCohortProfile() throws {
        let coordinator = StrategyExperimentCoordinator(config: .defaultConfig)
        let cohortID = UUID(uuidString: "00000000-0000-0000-0000-000000000123")!

        let first = coordinator.resolveSelection(selectedProfileID: nil, cohortID: cohortID)
        let second = coordinator.resolveSelection(selectedProfileID: nil, cohortID: cohortID)

        #expect(first.profileID == second.profileID)
        #expect(first.assignmentMode == "experiment_cohort")
        #expect(StewardConfig.defaultConfig.contextStrategyProfiles[first.profileID] != nil)
    }

    @Test("Frozen snapshot clone preserves strategy assignment and frozen profile snapshot")
    func frozenSnapshotClonePreservesStrategySnapshot() throws {
        let (_, context) = try makeTestModelContainer()
        let idea = Idea(title: "Frozen strategy clone", body: "Test")
        context.insert(idea)

        let workflow = try loadTestCanonicalWorkflow()
        let catalog = try loadTestCanonicalCatalog()
        let compiler = RunPlanCompiler(modelContext: context)
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)
        let profile = try #require(
            StewardConfig.defaultConfig.contextStrategyProfiles["selective_compression_and_escalation"]?
                .runtimeProfile(profileID: "selective_compression_and_escalation")
        )
        let startSnapshot = RunStartSnapshot(
            contextStrategyProfileID: "selective_compression_and_escalation",
            strategyAssignmentMode: "manual_override",
            strategyRecommendationState: StrategyRecommendationStatus.notEvaluated.rawValue,
            contextStrategySnapshotJSON: try JSONEncoder().encode(profile),
            promotedHandoffArtifactsJSON: try JSONEncoder().encode(["security_audit_raw"])
        )

        let (original, _) = try compiler.createRun(
            for: idea,
            plan: plan,
            workflowSourcePath: "/tmp/workflow.yaml",
            catalogSourcePath: "/tmp/agents.yaml",
            startSnapshot: startSnapshot
        )
        original.status = .blocked

        let clone = try RecoveryCoordinator(modelContext: context).cloneRunFrozenSnapshot(
            original: original,
            idea: idea,
            compiler: compiler
        )

        #expect(clone.id != original.id)
        #expect(clone.contextStrategyProfileID == "selective_compression_and_escalation")
        #expect(clone.strategyAssignmentMode == "manual_override")
        #expect(clone.contextStrategySnapshotJSON == original.contextStrategySnapshotJSON)
        #expect(clone.promotedHandoffArtifactsJSON == original.promotedHandoffArtifactsJSON)
    }

    @Test("Current-config clone can adopt a newer strategy selection than the frozen source run")
    func currentConfigCloneUsesCurrentStrategySelection() throws {
        let (_, context) = try makeTestModelContainer()
        let idea = Idea(title: "Current config strategy clone", body: "Test")
        context.insert(idea)

        let workflow = try loadTestCanonicalWorkflow()
        let catalog = try loadTestCanonicalCatalog()
        let compiler = RunPlanCompiler(modelContext: context)
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)
        let originalProfile = try #require(
            StewardConfig.defaultConfig.contextStrategyProfiles["current_mixed_baseline"]?
                .runtimeProfile(profileID: "current_mixed_baseline")
        )
        let originalSnapshot = RunStartSnapshot(
            contextStrategyProfileID: "current_mixed_baseline",
            strategyAssignmentMode: "default",
            strategyRecommendationState: StrategyRecommendationStatus.notEvaluated.rawValue,
            contextStrategySnapshotJSON: try JSONEncoder().encode(originalProfile)
        )

        let (original, _) = try compiler.createRun(
            for: idea,
            plan: plan,
            workflowSourcePath: "/tmp/workflow.yaml",
            catalogSourcePath: "/tmp/agents.yaml",
            startSnapshot: originalSnapshot
        )
        original.status = .blocked

        let selection = StrategyExperimentCoordinator(config: .defaultConfig)
            .resolveSelection(selectedProfileID: "fresh_control", cohortID: original.experimentCohortID)
        let currentSnapshot = RunStartSnapshot(
            contextStrategyProfileID: selection.profileID,
            strategyAssignmentMode: selection.assignmentMode,
            strategyRecommendationState: selection.recommendationState,
            contextStrategySnapshotJSON: try JSONEncoder().encode(selection.profile),
            promotedHandoffArtifactsJSON: try JSONEncoder().encode(["proposal_review_summary"])
        )

        let clone = try RecoveryCoordinator(modelContext: context).cloneRunCurrentConfig(
            original: original,
            idea: idea,
            workflow: workflow,
            catalog: catalog,
            compiler: compiler,
            workflowSourcePath: "/tmp/workflow.yaml",
            catalogSourcePath: "/tmp/agents.yaml",
            startSnapshot: currentSnapshot
        )

        #expect(clone.id != original.id)
        #expect(clone.contextStrategyProfileID == "fresh_control")
        #expect(clone.strategyAssignmentMode == selection.assignmentMode)
        let restored = try JSONDecoder().decode(ContextStrategyProfile.self, from: try #require(clone.contextStrategySnapshotJSON))
        #expect(restored.profileID == "fresh_control")
        let promoted = try JSONDecoder().decode([String].self, from: try #require(clone.promotedHandoffArtifactsJSON))
        #expect(promoted == ["proposal_review_summary"])
    }

    @Test("Continuity mode overrides runtime session reuse scope for long continuity and fresh control profiles")
    func continuityModeOverridesRuntimeSessionReuseScope() throws {
        let longContinuity = try #require(
            StewardConfig.defaultConfig.contextStrategyProfiles["manual_like_long_continuity"]?
                .runtimeProfile(profileID: "manual_like_long_continuity")
        )
        let freshControl = try #require(
            StewardConfig.defaultConfig.contextStrategyProfiles["fresh_control"]?
                .runtimeProfile(profileID: "fresh_control")
        )

        let lead = makeTestAgent(id: "lead_orchestrator", outputs: ["proposal_review_summary"])
        let writer = makeTestAgent(id: "proposal_writer", outputs: ["proposal_current"])
        let reviewer = makeTestAgent(id: "proposal_reviewer_ui", outputs: ["proposal_review_ui"])

        #expect(
            WorkflowOrchestrator.effectiveSessionReuseScope(for: lead, profile: longContinuity)
                == .same_agent_family_within_run
        )
        #expect(
            WorkflowOrchestrator.effectiveSessionReuseScope(for: writer, profile: longContinuity)
                == .same_agent_family_within_run
        )
        #expect(
            WorkflowOrchestrator.effectiveSessionReuseScope(for: reviewer, profile: longContinuity)
                == .none
        )
        #expect(
            WorkflowOrchestrator.effectiveSessionReuseScope(for: lead, profile: freshControl)
                == .none
        )
    }

    @Test("Canonical comparison recommendation cites evaluation set when evidence is complete")
    func strategyComparisonProducesRecommendationWithEvaluationSet() throws {
        let (_, context) = try makeTestModelContainer()
        let idea = Idea(title: "Strategy recommendation", body: "Test")
        context.insert(idea)

        let runA = Run(
            workflowID: "proposal_loop_live",
            workflowTitle: "Proposal Loop",
            workflowSnapshotHash: "wf",
            catalogSnapshotHash: "cat",
            workflowSourcePath: "workflow.yaml",
            catalogSourcePath: "agents.yaml",
            workflowSnapshotJSON: Data("{}".utf8),
            catalogSnapshotJSON: Data("{}".utf8)
        )
        runA.idea = idea
        runA.workflowFamily = "proposal_loop_live"
        runA.contextStrategyProfileID = "selective_compression_and_escalation"
        runA.strategyAssignmentMode = "manual_override"
        runA.strategyRecommendationState = StrategyRecommendationStatus.notEvaluated.rawValue
        runA.status = .completed
        runA.totalCostCents = 0
        runA.completedAt = runA.startedAt.addingTimeInterval(30)
        runA.sessionKPIExportJSON = makeCanonicalStrategyKPIJSON(runID: runA.id)

        let runB = Run(
            workflowID: "proposal_loop_live",
            workflowTitle: "Proposal Loop",
            workflowSnapshotHash: "wf",
            catalogSnapshotHash: "cat",
            workflowSourcePath: "workflow.yaml",
            catalogSourcePath: "agents.yaml",
            workflowSnapshotJSON: Data("{}".utf8),
            catalogSnapshotJSON: Data("{}".utf8)
        )
        runB.idea = idea
        runB.workflowFamily = "proposal_loop_live"
        runB.contextStrategyProfileID = "current_mixed_baseline"
        runB.strategyAssignmentMode = "default"
        runB.strategyRecommendationState = StrategyRecommendationStatus.notEvaluated.rawValue
        runB.status = .blocked
        runB.totalCostCents = 100_000
        runB.completedAt = runB.startedAt.addingTimeInterval(30)
        runB.sessionKPIExportJSON = makeCanonicalStrategyKPIJSON(runID: runB.id)

        context.insert(runA)
        context.insert(runB)

        let comparison = try #require(RunComparisonService(modelContext: context).compare(runA, runB))
        let recommendation = comparison.strategyRecommendation

        #expect(recommendation.status == .candidateWinner)
        #expect(recommendation.proofOwner == "shell_comparison_lane")
        #expect(recommendation.evaluationSetComplete == true)
        #expect(recommendation.evaluationSetSummary.contains(String(runA.id.uuidString.prefix(8))))
        #expect(recommendation.evaluationSetSummary.contains(String(runB.id.uuidString.prefix(8))))
        #expect(recommendation.recommendedProfileID == "selective_compression_and_escalation")
    }

    @Test("Selective compression demonstrates measurable savings through canonical KPI export lane")
    func selectiveCompressionDemonstratesMeasurableSavings() throws {
        let (_, context) = try makeTestModelContainer()
        let idea = Idea(title: "Strategy savings proof", body: "Test")
        context.insert(idea)

        let runA = Run(
            workflowID: "proposal_loop_live",
            workflowTitle: "Proposal Loop",
            workflowSnapshotHash: "wf",
            catalogSnapshotHash: "cat",
            workflowSourcePath: "workflow.yaml",
            catalogSourcePath: "agents.yaml",
            workflowSnapshotJSON: Data("{}".utf8),
            catalogSnapshotJSON: Data("{}".utf8)
        )
        runA.idea = idea
        runA.workflowFamily = "proposal_loop_live"
        runA.contextStrategyProfileID = "selective_compression_and_escalation"
        runA.strategyAssignmentMode = "manual_override"
        runA.strategyRecommendationState = StrategyRecommendationStatus.notEvaluated.rawValue
        runA.status = .completed
        runA.totalCostCents = 15
        runA.completedAt = runA.startedAt.addingTimeInterval(45)
        context.insert(runA)

        let runB = Run(
            workflowID: "proposal_loop_live",
            workflowTitle: "Proposal Loop",
            workflowSnapshotHash: "wf",
            catalogSnapshotHash: "cat",
            workflowSourcePath: "workflow.yaml",
            catalogSourcePath: "agents.yaml",
            workflowSnapshotJSON: Data("{}".utf8),
            catalogSnapshotJSON: Data("{}".utf8)
        )
        runB.idea = idea
        runB.workflowFamily = "proposal_loop_live"
        runB.contextStrategyProfileID = "current_mixed_baseline"
        runB.strategyAssignmentMode = "default"
        runB.strategyRecommendationState = StrategyRecommendationStatus.notEvaluated.rawValue
        runB.status = .blocked
        runB.totalCostCents = 180
        runB.completedAt = runB.startedAt.addingTimeInterval(75)
        context.insert(runB)

        try attachStrategyProofData(
            to: runA,
            context: context,
            payloadBefore: 4_096,
            payloadAfter: 1_536,
            reduction: 2_560,
            compactionCount: 2,
            lazyCount: 3
        )
        try attachStrategyProofData(
            to: runB,
            context: context,
            payloadBefore: 4_096,
            payloadAfter: 4_096,
            reduction: 0,
            compactionCount: 0,
            lazyCount: 0
        )

        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        runA.sessionKPIExportJSON = try encoder.encode(SessionReuseKPIExporter.exportKPIs(for: runA.id, context: context))
        runB.sessionKPIExportJSON = try encoder.encode(SessionReuseKPIExporter.exportKPIs(for: runB.id, context: context))

        let comparison = try #require(RunComparisonService(modelContext: context).compare(runA, runB))

        #expect(comparison.strategyComparison.evidenceComplete)
        #expect(comparison.strategyRecommendation.status == .candidateWinner)
        #expect(comparison.strategyRecommendation.recommendedProfileID == "selective_compression_and_escalation")
        #expect(comparison.strategyRecommendation.rationale.contains("outperformed"))
    }

    @Test("Strategy recommendation prefers normalized telemetry over cheaper but escalation-heavy runs")
    func strategyRecommendationPrefersCanonicalTelemetryOverCoarseCost() throws {
        let (_, context) = try makeTestModelContainer()
        let idea = Idea(title: "Telemetry scoring", body: "Test")
        context.insert(idea)

        let runA = Run(
            workflowID: "proposal_loop_live",
            workflowTitle: "Proposal Loop",
            workflowSnapshotHash: "wf",
            catalogSnapshotHash: "cat",
            workflowSourcePath: "workflow.yaml",
            catalogSourcePath: "agents.yaml",
            workflowSnapshotJSON: Data("{}".utf8),
            catalogSnapshotJSON: Data("{}".utf8)
        )
        runA.idea = idea
        runA.workflowFamily = "proposal_loop_live"
        runA.contextStrategyProfileID = "selective_compression_and_escalation"
        runA.strategyAssignmentMode = "manual_override"
        runA.strategyRecommendationState = StrategyRecommendationStatus.notEvaluated.rawValue
        runA.status = .completed
        runA.totalCostCents = 220
        runA.completedAt = runA.startedAt.addingTimeInterval(75)
        runA.sessionKPIExportJSON = makeCanonicalStrategyKPIJSON(
            runID: runA.id,
            totalCostCents: 220,
            payloadReductionBytes: 2_560,
            averageCacheEffectiveness: 0.82,
            totalCompactionChurn: 1,
            totalEscalationCount: 0,
            totalRetryableEscalationCount: 0,
            totalContractFailureCount: 0,
            operatorPromotedArtifactCount: 1,
            totalPromotedArtifactUsages: 1
        )

        let runB = Run(
            workflowID: "proposal_loop_live",
            workflowTitle: "Proposal Loop",
            workflowSnapshotHash: "wf",
            catalogSnapshotHash: "cat",
            workflowSourcePath: "workflow.yaml",
            catalogSourcePath: "agents.yaml",
            workflowSnapshotJSON: Data("{}".utf8),
            catalogSnapshotJSON: Data("{}".utf8)
        )
        runB.idea = idea
        runB.workflowFamily = "proposal_loop_live"
        runB.contextStrategyProfileID = "fresh_control"
        runB.strategyAssignmentMode = "manual_override"
        runB.strategyRecommendationState = StrategyRecommendationStatus.notEvaluated.rawValue
        runB.status = .completed
        runB.totalCostCents = 40
        runB.completedAt = runB.startedAt.addingTimeInterval(20)
        runB.sessionKPIExportJSON = makeCanonicalStrategyKPIJSON(
            runID: runB.id,
            totalCostCents: 40,
            payloadReductionBytes: 0,
            averageCacheEffectiveness: 0.05,
            totalCompactionChurn: 5,
            totalEscalationCount: 3,
            totalRetryableEscalationCount: 3,
            totalContractFailureCount: 2,
            operatorPromotedArtifactCount: 0,
            totalPromotedArtifactUsages: 4
        )

        context.insert(runA)
        context.insert(runB)

        let comparison = try #require(RunComparisonService(modelContext: context).compare(runA, runB))

        #expect(comparison.strategyComparison.evidenceComplete)
        #expect(comparison.strategyRecommendation.status == .candidateWinner)
        #expect(comparison.strategyRecommendation.recommendedProfileID == "selective_compression_and_escalation")
    }
}

private func makeCanonicalStrategyKPIJSON(
    runID: UUID,
    totalCostCents: Int64 = 256,
    payloadReductionBytes: Int64 = 2048,
    averageCacheEffectiveness: Double = 0.75,
    totalCompactionChurn: Int = 1,
    totalEscalationCount: Int = 0,
    totalRetryableEscalationCount: Int = 0,
    totalContractFailureCount: Int = 0,
    operatorPromotedArtifactCount: Int = 1,
    totalPromotedArtifactUsages: Int = 1
) -> Data {
    let summary = SessionReuseKPIExporter.RunKPISummary(
        runID: runID,
        exportedAt: Date(timeIntervalSince1970: 1_000),
        totalExecutions: 4,
        totalReusedExecutions: 2,
        overallReusePercentage: 0.5,
        totalColdStartTokensSaved: 128,
        totalSessionGrowthTokens: 64,
        totalForcedBudgetResets: 0,
        totalTokenSavingsVersusFreshBaseline: totalCostCents,
        perAgentKPIs: [],
        mcpTelemetry: SessionReuseKPIExporter.MCPTelemetrySummary(
            totalExecutionsWithMCPProfile: 0,
            totalZeroMCPExecutions: 0,
            totalRequestedExtensionCount: 0,
            totalPredictedExtensionCount: 0,
            totalActualExtensionCount: 0,
            totalDeniedExtensionCount: 0,
            totalPolicyReductionExecutions: 0,
            totalPredictionDriftExecutions: 0,
            averageRequestedExtensionsPerExecution: 0,
            averageActualExtensionsPerExecution: 0,
            totalStartupLatencyMilliseconds: 0,
            averageStartupLatencyMilliseconds: 0,
            startupLatencyByExtensionSet: [],
            serverUsage: [],
            totalPromptContextDeltaBytes: 0,
            totalMCPPreflightBlockedRuns: 0
        ),
        strategyTelemetry: SessionReuseKPIExporter.StrategyTelemetrySummary(
            totalPayloadBytesBeforeStrategy: 4096,
            totalPayloadBytesAfterStrategy: 4096 - payloadReductionBytes,
            totalPayloadReductionBytes: payloadReductionBytes,
            averageLazyArtifactCount: 2.0,
            totalLazyEvidenceHitCount: 1,
            averageLazyEvidenceHitRate: 0.5,
            averageCacheEffectiveness: averageCacheEffectiveness,
            totalCompactionChurn: totalCompactionChurn,
            totalEscalationCount: totalEscalationCount,
            totalRetryableEscalationCount: totalRetryableEscalationCount,
            totalContractFailureCount: totalContractFailureCount,
            operatorPromotedArtifactCount: operatorPromotedArtifactCount,
            totalPromotedArtifactUsages: totalPromotedArtifactUsages
        )
    )

    let encoder = JSONEncoder()
    encoder.dateEncodingStrategy = .iso8601
    return (try? encoder.encode(summary)) ?? Data()
}

@MainActor
private func attachStrategyProofData(
    to run: Run,
    context: ModelContext,
    payloadBefore: Int,
    payloadAfter: Int,
    reduction: Int,
    compactionCount: Int,
    lazyCount: Int
) throws {
    let stage = StageExecution(
        stageID: "state_4_proposal_reviewed",
        label: "Proposal reviewed",
        status: .completed,
        iteration: 1,
        attemptNumber: 1
    )
    stage.run = run
    context.insert(stage)

    let execution = AgentExecution(
        agentID: "lead_orchestrator",
        agentTitle: "Lead / Orchestrator",
        taskName: "aggregate_proposal_reviews",
        status: .completed,
        provider: "claude_code",
        effort: "high"
    )
    execution.stageExecution = stage
    execution.limitPressureSignalsJSON = try JSONEncoder().encode(
        StrategyLimitPressureSignals(
            inputPayloadBytes: payloadAfter,
            payloadBytesBeforeStrategy: payloadBefore,
            payloadBytesAfterStrategy: payloadAfter,
            payloadReductionBytes: reduction,
            mandatoryArtifactCount: 2,
            summarizedArtifactCount: compactionCount,
            lazyArtifactCount: lazyCount,
            lazyEvidenceHitCount: lazyCount == 0 ? 0 : 1,
            lazyEvidenceHitRate: lazyCount == 0 ? 0.0 : (1.0 / Double(lazyCount)),
            compactionCount: compactionCount,
            cacheEffectiveness: 0.8,
            compactionChurnCount: compactionCount,
            escalationCount: 0,
            retryableEscalationCount: 0,
            contractFailureCount: 0,
            operatorPromotedArtifactCount: 0
        )
    )
    context.insert(execution)

    let lineage = AgentSessionLineage(
        runID: run.id,
        agentID: "lead_orchestrator",
        lineageID: UUID().uuidString,
        sessionReuseScope: .same_invocation_owner
    )
    context.insert(lineage)

    let generation = AgentSessionGeneration(
        generation: 1,
        invocationOwnerKey: "proof-owner",
        providerSessionID: "session-\(UUID().uuidString)",
        bindingFingerprint: "binding",
        workingDirectory: "/tmp",
        workspaceMode: "read_only",
        runtimeProvider: "claude",
        runtimeModel: "opus"
    )
    generation.turnCount = 1
    generation.estimatedInputTokens = Int64(payloadBefore / 4)
    generation.cumulativePromptTokens = Int64(payloadAfter / 4)
    generation.cumulativeCostCents = run.totalCostCents ?? 0
    generation.lineage = lineage
    lineage.generations.append(generation)
    lineage.activeGenerationID = generation.id
    context.insert(generation)

    let created = AgentSessionEvent(generationID: generation.id, eventType: .created)
    created.lineage = lineage
    lineage.events.append(created)
    context.insert(created)
}
