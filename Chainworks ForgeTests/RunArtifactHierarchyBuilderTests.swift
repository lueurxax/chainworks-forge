import Testing
import Foundation
import SwiftData
@testable import Chainworks_Forge

@MainActor
@Suite("RunArtifactHierarchyBuilder", .tags(.fast))
struct RunArtifactHierarchyBuilderTests {
    @Test("Builder groups artifacts by stage, agent, semantic bucket, and promoted priority")
    func builderProjectsCanonicalHierarchy() throws {
        let context = try makeTestModelContext()
        let workspace = makeTestWorkspace()
        let run = makeTestRun(
            workspace: workspace,
            context: context,
            workflowID: "proposal_loop",
            workflowTitle: "Proposal Loop"
        )

        let draftStage = StageExecution(
            id: UUID(),
            stageID: "draft",
            label: "Draft",
            startedAt: Date(timeIntervalSince1970: 100),
            status: .completed,
            iteration: 1,
            attemptNumber: 1
        )
        draftStage.run = run
        run.stageExecutions.append(draftStage)
        context.insert(draftStage)

        let reviewStage = StageExecution(
            id: UUID(),
            stageID: "review",
            label: "Review",
            startedAt: Date(timeIntervalSince1970: 200),
            status: .running,
            iteration: 2,
            attemptNumber: 1
        )
        reviewStage.run = run
        run.stageExecutions.append(reviewStage)
        context.insert(reviewStage)

        let writer = AgentExecution(
            id: UUID(),
            agentID: "proposal_writer",
            agentTitle: "Proposal Writer",
            taskName: "draft_proposal",
            startedAt: Date(timeIntervalSince1970: 110),
            status: .completed,
            provider: "claude_code",
            effort: "high"
        )
        writer.stageExecution = draftStage
        draftStage.agentExecutions.append(writer)
        context.insert(writer)

        let reviewer = AgentExecution(
            id: UUID(),
            agentID: "proposal_reviewer",
            agentTitle: "Proposal Reviewer",
            taskName: "review_proposal",
            startedAt: Date(timeIntervalSince1970: 210),
            status: .running,
            provider: "codex",
            effort: "medium"
        )
        reviewer.stageExecution = reviewStage
        reviewStage.agentExecutions.append(reviewer)
        context.insert(reviewer)

        let immutableReport = makeArtifact(
            name: "run_report_v1",
            contractID: "run_report",
            format: .report,
            filePath: workspace.artifactRoot.appendingPathComponent("run_report_v1.md").path,
            createdAt: Date(timeIntervalSince1970: 120),
            runID: run.id,
            stageID: draftStage.stageID,
            agentID: writer.agentID,
            provider: "claude_code",
            attemptNumber: 1
        )
        immutableReport.agentExecution = writer
        immutableReport.reportKind = "immutable_history"
        immutableReport.reportVersion = 1
        writer.artifacts.append(immutableReport)
        context.insert(immutableReport)

        let supersededSummaryID = UUID()
        let latestSummary = makeArtifact(
            name: "proposal_review_summary",
            contractID: "run_summary",
            format: .markdown,
            filePath: workspace.artifactRoot.appendingPathComponent("proposal_review_summary.md").path,
            createdAt: Date(timeIntervalSince1970: 220),
            runID: run.id,
            stageID: reviewStage.stageID,
            agentID: reviewer.agentID,
            provider: "codex",
            attemptNumber: 1
        )
        latestSummary.agentExecution = reviewer
        latestSummary.reportKind = "latest_summary"
        latestSummary.reportVersion = 2
        latestSummary.supersedesArtifactID = supersededSummaryID
        reviewer.artifacts.append(latestSummary)
        context.insert(latestSummary)

        let promotedReceipt = makeArtifact(
            name: "qa_receipt",
            contractID: "review_receipt",
            format: .json,
            filePath: workspace.artifactRoot.appendingPathComponent("qa_receipt.json").path,
            createdAt: Date(timeIntervalSince1970: 230),
            runID: run.id,
            stageID: reviewStage.stageID,
            agentID: reviewer.agentID,
            provider: "codex",
            attemptNumber: 1
        )
        promotedReceipt.agentExecution = reviewer
        reviewer.artifacts.append(promotedReceipt)
        context.insert(promotedReceipt)

        let pinnedDelivery = makeArtifact(
            name: "release_manifest",
            contractID: "release_manifest",
            format: .json,
            filePath: workspace.artifactRoot.appendingPathComponent("release_manifest.json").path,
            createdAt: Date(timeIntervalSince1970: 130),
            runID: run.id,
            stageID: draftStage.stageID,
            agentID: writer.agentID,
            provider: "claude_code",
            attemptNumber: 1
        )
        pinnedDelivery.agentExecution = writer
        pinnedDelivery.isPinned = true
        writer.artifacts.append(pinnedDelivery)
        context.insert(pinnedDelivery)

        let stageDiagnostic = makeArtifact(
            name: "review_trace_log",
            contractID: "diagnostic_trace",
            format: .json,
            filePath: workspace.artifactRoot.appendingPathComponent("review_trace_log.json").path,
            createdAt: Date(timeIntervalSince1970: 240),
            runID: run.id,
            stageID: reviewStage.stageID,
            agentID: "system",
            provider: "system",
            attemptNumber: 1
        )
        context.insert(stageDiagnostic)

        run.latestImmutableReportArtifactID = immutableReport.id
        run.latestSummaryArtifactID = latestSummary.id
        run.latestReportVersion = 2
        run.promotedHandoffArtifactsJSON = try JSONEncoder().encode(["qa_receipt"])

        let hierarchy = RunArtifactHierarchyBuilder().build(for: run)

        #expect(hierarchy.runID == run.id)
        #expect(hierarchy.latestSummaryArtifactID == latestSummary.id)
        #expect(hierarchy.latestImmutableReportArtifactID == immutableReport.id)
        #expect(hierarchy.latestReportVersion == 2)
        #expect(hierarchy.promotedArtifacts.map(\.name) == ["qa_receipt", "release_manifest"])
        #expect(hierarchy.stageGroups.map(\.stageID) == ["review", "draft"])

        let reviewGroup = try #require(hierarchy.stageGroups.first(where: { $0.stageID == "review" }))
        #expect(reviewGroup.stageExecutionID == reviewStage.id)
        #expect(reviewGroup.iteration == 2)

        let reviewerGroup = try #require(reviewGroup.agentGroups.first(where: { $0.agentID == reviewer.agentID }))
        #expect(reviewerGroup.agentExecutionID == reviewer.id)
        #expect(reviewerGroup.semanticBuckets.map(\.bucket) == [.summary, .receipt])

        let summaryBucket = try #require(reviewerGroup.semanticBuckets.first(where: { $0.bucket == .summary }))
        let summaryLeaf = try #require(summaryBucket.artifacts.first)
        #expect(summaryLeaf.artifactID == latestSummary.id)
        #expect(summaryLeaf.reportKind == "latest_summary")
        #expect(summaryLeaf.reportVersion == 2)
        #expect(summaryLeaf.supersedesArtifactID == supersededSummaryID)
        #expect(summaryLeaf.isLatestSummaryReport)

        let stageDiagnosticBucket = try #require(reviewGroup.stageBuckets.first(where: { $0.bucket == .diagnostic }))
        #expect(stageDiagnosticBucket.artifacts.map(\.name) == ["review_trace_log"])
    }

    @Test("Builder keeps repeated stage executions separate and resolves stage-only artifacts by attempt number")
    func builderSeparatesStageAttempts() throws {
        let context = try makeTestModelContext()
        let workspace = makeTestWorkspace()
        let run = makeTestRun(
            workspace: workspace,
            context: context,
            workflowID: "retry_workflow",
            workflowTitle: "Retry Workflow"
        )

        let firstAttempt = StageExecution(
            id: UUID(),
            stageID: "review",
            label: "Review",
            startedAt: Date(timeIntervalSince1970: 100),
            status: .failed,
            iteration: 1,
            attemptNumber: 1
        )
        firstAttempt.run = run
        run.stageExecutions.append(firstAttempt)
        context.insert(firstAttempt)

        let secondAttempt = StageExecution(
            id: UUID(),
            stageID: "review",
            label: "Review",
            startedAt: Date(timeIntervalSince1970: 200),
            status: .running,
            iteration: 1,
            attemptNumber: 2
        )
        secondAttempt.run = run
        run.stageExecutions.append(secondAttempt)
        context.insert(secondAttempt)

        let firstArtifact = makeArtifact(
            name: "review_debug_log",
            contractID: "diagnostic_trace",
            format: .json,
            filePath: workspace.artifactRoot.appendingPathComponent("review_debug_log.json").path,
            createdAt: Date(timeIntervalSince1970: 150),
            runID: run.id,
            stageID: "review",
            agentID: "system",
            provider: "system",
            attemptNumber: 1
        )
        context.insert(firstArtifact)

        let secondArtifact = makeArtifact(
            name: "review_receipt",
            contractID: "review_receipt",
            format: .json,
            filePath: workspace.artifactRoot.appendingPathComponent("review_receipt.json").path,
            createdAt: Date(timeIntervalSince1970: 250),
            runID: run.id,
            stageID: "review",
            agentID: "system",
            provider: "system",
            attemptNumber: 2
        )
        context.insert(secondArtifact)

        let hierarchy = RunArtifactHierarchyBuilder().build(for: run)

        #expect(hierarchy.stageGroups.count == 2)
        #expect(hierarchy.stageGroups.map(\.attemptNumber) == [2, 1])

        let latestGroup = try #require(hierarchy.stageGroups.first)
        #expect(latestGroup.stageExecutionID == secondAttempt.id)
        #expect(latestGroup.stageBuckets.flatMap(\.artifacts).map(\.name) == ["review_receipt"])

        let firstGroup = try #require(hierarchy.stageGroups.last)
        #expect(firstGroup.stageExecutionID == firstAttempt.id)
        #expect(firstGroup.stageBuckets.flatMap(\.artifacts).map(\.name) == ["review_debug_log"])
    }

    private func makeArtifact(
        name: String,
        contractID: String,
        format: ArtifactFormat,
        filePath: String,
        createdAt: Date,
        runID: UUID,
        stageID: String,
        agentID: String,
        provider: String,
        attemptNumber: Int
    ) -> Artifact {
        Artifact(
            name: name,
            contractID: contractID,
            format: format,
            filePath: filePath,
            createdAt: createdAt,
            runID: runID,
            stageID: stageID,
            agentID: agentID,
            provider: provider,
            attemptNumber: attemptNumber
        )
    }
}
