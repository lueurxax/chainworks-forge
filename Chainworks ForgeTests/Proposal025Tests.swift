import Testing
import Foundation
import SwiftData
@testable import Chainworks_Forge

@MainActor
@Suite("Proposal 025")
struct Proposal025Tests {
    @Test("Portability-sensitive runtime sources avoid workstation-specific absolute paths")
    func portabilitySensitiveSourcesAvoidHardcodedUserPaths() throws {
        let repoRoot = testRepositoryRootURL()
        let sensitiveFiles = [
            "Chainworks Forge/Support/PreviewSupport.swift",
            "Chainworks Forge/Views/DeliveryPreflightReportView.swift",
            "Chainworks Forge/Views/ReleaseGateView.swift",
            "Chainworks Forge/Views/IdeaListView.swift",
            "Chainworks ForgeTests/Chainworks_ForgeTests.swift",
            "Chainworks ForgeTests/GooseSessionBridgeTests.swift"
        ]

        for relativePath in sensitiveFiles {
            let fileURL = repoRoot.appendingPathComponent(relativePath, isDirectory: false)
            let source = try String(contentsOf: fileURL, encoding: .utf8)
            #expect(
                source.contains("/Users/user/") == false,
                "\(relativePath) still hardcodes a workstation-specific user path"
            )
        }
    }

    @Test("Preferred example URL resolves repo copy before bundled fallback")
    func preferredExampleURLPrefersRepositoryCopy() throws {
        let tempRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(
            at: tempRoot.appendingPathComponent("examples/agents", isDirectory: true),
            withIntermediateDirectories: true
        )
        defer { try? FileManager.default.removeItem(at: tempRoot) }

        let repoCopy = tempRoot.appendingPathComponent("examples/agents/agents.yaml", isDirectory: false)
        let bundledCopy = tempRoot.appendingPathComponent("agents.bundle.yaml", isDirectory: false)
        try "repo".write(to: repoCopy, atomically: true, encoding: .utf8)
        try "bundle".write(to: bundledCopy, atomically: true, encoding: .utf8)

        let resolved = AppConfiguration.preferredExampleURL(
            repoRelativePath: "examples/agents/agents.yaml",
            bundledURL: bundledCopy,
            currentDirectoryPath: tempRoot.path,
            allowsDocumentsFallback: false
        )

        #expect(resolved?.path == repoCopy.path)
    }

    @Test("Preferred example URL can anchor repository lookup to caller source file")
    func preferredExampleURLUsesCallerSourceFilePath() throws {
        let tempRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let sourceRoot = tempRoot.appendingPathComponent("repo", isDirectory: true)
        let sourceFile = sourceRoot
            .appendingPathComponent("Chainworks Forge/Engine/Proposal022FeedbackFidelity.swift", isDirectory: false)
        let bundledCopy = tempRoot.appendingPathComponent("proposal-loop-live.bundle.yaml", isDirectory: false)

        try FileManager.default.createDirectory(
            at: sourceRoot.appendingPathComponent("examples/workflows", isDirectory: true),
            withIntermediateDirectories: true
        )
        try FileManager.default.createDirectory(
            at: sourceRoot.appendingPathComponent("Chainworks Forge/Engine", isDirectory: true),
            withIntermediateDirectories: true
        )
        defer { try? FileManager.default.removeItem(at: tempRoot) }

        let repoCopy = sourceRoot.appendingPathComponent("examples/workflows/proposal-loop-live.yaml", isDirectory: false)
        try "repo".write(to: repoCopy, atomically: true, encoding: .utf8)
        try "source".write(to: sourceFile, atomically: true, encoding: .utf8)
        try "bundle".write(to: bundledCopy, atomically: true, encoding: .utf8)

        let resolved = AppConfiguration.preferredExampleURL(
            repoRelativePath: "examples/workflows/proposal-loop-live.yaml",
            bundledURL: bundledCopy,
            currentDirectoryPath: tempRoot.appendingPathComponent("elsewhere", isDirectory: true).path,
            allowsDocumentsFallback: false,
            sourceFilePath: sourceFile.path
        )

        #expect(resolved?.path == repoCopy.path)
    }

    @Test("Repository root resolution prefers source checkout over Documents fallback")
    func defaultRepositoryRootPrefersSourceCheckout() throws {
        let tempRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let sourceRoot = tempRoot.appendingPathComponent("repo", isDirectory: true)
        let sourceFile = sourceRoot
            .appendingPathComponent("Chainworks Forge/Support/AppConfiguration.swift", isDirectory: false)
        let documentsRoot = tempRoot.appendingPathComponent("Documents/Chainworks Forge", isDirectory: true)

        try FileManager.default.createDirectory(
            at: sourceRoot.appendingPathComponent("examples/agents", isDirectory: true),
            withIntermediateDirectories: true
        )
        try FileManager.default.createDirectory(
            at: sourceRoot.appendingPathComponent("Chainworks Forge/Support", isDirectory: true),
            withIntermediateDirectories: true
        )
        try FileManager.default.createDirectory(
            at: documentsRoot.appendingPathComponent("examples/agents", isDirectory: true),
            withIntermediateDirectories: true
        )
        defer { try? FileManager.default.removeItem(at: tempRoot) }

        try "repo".write(
            to: sourceRoot.appendingPathComponent("examples/agents/agents.yaml", isDirectory: false),
            atomically: true,
            encoding: .utf8
        )
        try "source".write(to: sourceFile, atomically: true, encoding: .utf8)
        try "documents".write(
            to: documentsRoot.appendingPathComponent("examples/agents/agents.yaml", isDirectory: false),
            atomically: true,
            encoding: .utf8
        )

        let resolved = AppConfiguration.defaultRepositoryRoot(
            currentDirectoryPath: tempRoot.appendingPathComponent("elsewhere", isDirectory: true).path,
            bundleURL: nil,
            allowsDocumentsFallback: true,
            sourceFilePath: sourceFile.path
        )

        #expect(resolved.standardizedFileURL == sourceRoot.standardizedFileURL)
    }

    @Test("Repo-backed seed surfaces avoid cwd-derived repository roots")
    func seededRuntimeSurfacesAvoidCurrentDirectoryRepoTruth() throws {
        let repoRoot = testRepositoryRootURL()
        let sensitiveFiles = [
            "Chainworks Forge/Chainworks_ForgeApp.swift",
            "Chainworks Forge/Views/UITestDirectSurfaces.swift",
            "Chainworks Forge/Engine/SampleRunLauncher.swift"
        ]
        let forbiddenFragments = [
            "repoRoot: FileManager.default.currentDirectoryPath",
            "run.repoRoot = FileManager.default.currentDirectoryPath",
            "workspaceRootPath: FileManager.default.currentDirectoryPath"
        ]

        for relativePath in sensitiveFiles {
            let fileURL = repoRoot.appendingPathComponent(relativePath, isDirectory: false)
            let source = try String(contentsOf: fileURL, encoding: .utf8)
            for fragment in forbiddenFragments {
                #expect(
                    source.contains(fragment) == false,
                    "\(relativePath) still derives repo-backed runtime truth from cwd via: \(fragment)"
                )
            }
        }
    }

    @Test("Run report payload exposes requested predicted actual MCP contract and telemetry")
    func runReportPayloadExposesMCPTruth() throws {
        let (_, context) = try makeTestModelContainer()
        let workspace = makeTestWorkspace()
        let run = makeTestRun(workspace: workspace, context: context)

        let stage = StageExecution(
            stageID: "state_4_proposal_reviewed",
            label: "Proposal Reviewed",
            status: .completed
        )
        stage.run = run
        context.insert(stage)

        let agent = AgentExecution(
            agentID: "proposal_reviewer_ui",
            agentTitle: "UI Reviewer",
            taskName: "review_ui",
            status: .completed,
            provider: "gemini",
            effort: "high"
        )
        agent.stageExecution = stage
        agent.mcpProfileID = "ui_review_visual"
        agent.requestedMCPExtensionsJSON = try JSONEncoder().encode(["xcode", "context7"] as [String])
        agent.effectiveMCPRuntimeExtensionIDsJSON = try JSONEncoder().encode(["xcode"] as [String])
        agent.deniedMCPExtensionsJSON = try JSONEncoder().encode(["context7"] as [String])
        agent.mcpSessionStartupLatencyMilliseconds = 240
        agent.mcpServerTelemetryJSON = try JSONEncoder().encode([
            MCPServerExecutionMetric(
                serverID: "xcode",
                toolCallCount: 2,
                requestBytes: 128,
                responseBytes: 512,
                promptContextDeltaBytes: 512
            )
        ])
        context.insert(agent)

        let resolvedPolicies: [String: MCPPolicyResolutionReport] = [
            "proposal_reviewer_ui": MCPPolicyResolutionReport(
                profileID: "ui_review_visual",
                requiredExtensions: ["xcode"],
                optionalExtensions: ["context7"],
                requestedExtensions: ["xcode", "context7"],
                requiredRuntimeExtensionIDs: ["xcode"],
                optionalRuntimeExtensionIDs: ["context7"],
                predictedEffectiveExtensions: ["xcode", "context7"],
                predictedEffectiveRuntimeExtensionIDs: ["xcode", "context7"],
                deniedExtensions: [],
                warnings: [],
                blockingIssues: []
            )
        ]
        run.resolvedMCPPoliciesJSON = try JSONEncoder().encode(resolvedPolicies)
        run.sessionKPIExportJSON = SessionReuseKPIExporter.exportJSON(for: run.id, context: context)

        let payload = RunReportBuilder(modelContext: context).buildReportPayload(for: run, version: 1)
        let reportAgent = try #require(payload.agentsUsed.first(where: { $0.agentID == "proposal_reviewer_ui" }))
        let mcpTelemetry = try #require(payload.mcpTelemetry)

        #expect(reportAgent.mcpProfileID == "ui_review_visual")
        #expect(reportAgent.requestedMCPExtensions == ["xcode", "context7"])
        #expect(reportAgent.predictedMCPExtensions == ["xcode", "context7"])
        #expect(reportAgent.actualMCPExtensions == ["xcode"])
        #expect(reportAgent.deniedMCPExtensions == ["context7"])
        #expect(mcpTelemetry.totalExecutionsWithMCPProfile == 1)
        #expect(mcpTelemetry.totalRequestedExtensionCount == 2)
        #expect(mcpTelemetry.totalActualExtensionCount == 1)
        #expect(mcpTelemetry.totalDeniedExtensionCount == 1)
        #expect(mcpTelemetry.totalPolicyReductionExecutions == 1)
        #expect(mcpTelemetry.totalPredictionDriftExecutions == 1)
        #expect(mcpTelemetry.totalStartupLatencyMilliseconds == 240)
        #expect(mcpTelemetry.averageStartupLatencyMilliseconds == 240)
        #expect(mcpTelemetry.totalPromptContextDeltaBytes == 512)
        #expect(mcpTelemetry.totalMCPPreflightBlockedRuns == 0)
        let latencyBucket = try #require(mcpTelemetry.startupLatencyByExtensionSet.first)
        #expect(latencyBucket.extensionSet == "xcode")
        #expect(latencyBucket.executionCount == 1)
        let usage = try #require(mcpTelemetry.serverUsage.first(where: { $0.serverID == "xcode" }))
        #expect(usage.toolCallCount == 2)
        #expect(usage.requestBytes == 128)
        #expect(usage.responseBytes == 512)
        #expect(usage.promptContextDeltaBytes == 512)
    }

    @Test("KPI summary counts MCP-preflight blocked runs")
    func mcpPreflightBlockedRunsAreCounted() throws {
        let (_, context) = try makeTestModelContainer()
        let workspace = makeTestWorkspace()
        let run = makeTestRun(workspace: workspace, context: context)
        run.status = .blocked
        run.driftDetails = "Goose extension registry is unavailable, but one or more agents request MCP extensions."

        let summary = SessionReuseKPIExporter.exportKPIs(for: run.id, context: context)
        #expect(summary.mcpTelemetry.totalMCPPreflightBlockedRuns == 1)
    }

    @Test("Run comparison surfaces MCP contract deltas for each agent binding")
    func runComparisonSurfacesMCPContract() throws {
        let (_, context) = try makeTestModelContainer()
        let idea = Idea(title: "Proposal 025", body: "Compare MCP truth")
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
        runA.idea = idea
        runB.idea = idea
        context.insert(runA)
        context.insert(runB)

        let stageA = StageExecution(stageID: "state_4_proposal_reviewed", label: "Proposal Reviewed", status: .completed)
        stageA.run = runA
        context.insert(stageA)
        let stageB = StageExecution(stageID: "state_4_proposal_reviewed", label: "Proposal Reviewed", status: .completed)
        stageB.run = runB
        context.insert(stageB)

        let agentA = AgentExecution(agentID: "proposal_reviewer_ui", agentTitle: "UI Reviewer", taskName: "review_ui", status: .completed, provider: "gemini", effort: "high")
        agentA.stageExecution = stageA
        agentA.mcpProfileID = "ui_review_visual"
        agentA.requestedMCPExtensionsJSON = try JSONEncoder().encode(["xcode", "context7"] as [String])
        agentA.effectiveMCPRuntimeExtensionIDsJSON = try JSONEncoder().encode(["xcode"] as [String])
        agentA.deniedMCPExtensionsJSON = try JSONEncoder().encode(["context7"] as [String])
        context.insert(agentA)

        let agentB = AgentExecution(agentID: "proposal_reviewer_ui", agentTitle: "UI Reviewer", taskName: "review_ui", status: .completed, provider: "gemini", effort: "high")
        agentB.stageExecution = stageB
        agentB.mcpProfileID = "ui_review_visual"
        agentB.requestedMCPExtensionsJSON = try JSONEncoder().encode(["xcode"] as [String])
        agentB.effectiveMCPRuntimeExtensionIDsJSON = try JSONEncoder().encode(["xcode"] as [String])
        agentB.deniedMCPExtensionsJSON = try JSONEncoder().encode([String]())
        context.insert(agentB)

        let policiesA = [
            "proposal_reviewer_ui": MCPPolicyResolutionReport(
                profileID: "ui_review_visual",
                requiredExtensions: ["xcode"],
                optionalExtensions: ["context7"],
                requestedExtensions: ["xcode", "context7"],
                requiredRuntimeExtensionIDs: ["xcode"],
                optionalRuntimeExtensionIDs: ["context7"],
                predictedEffectiveExtensions: ["xcode", "context7"],
                predictedEffectiveRuntimeExtensionIDs: ["xcode", "context7"],
                deniedExtensions: [],
                warnings: [],
                blockingIssues: []
            )
        ]
        let policiesB = [
            "proposal_reviewer_ui": MCPPolicyResolutionReport(
                profileID: "ui_review_visual",
                requiredExtensions: ["xcode"],
                optionalExtensions: [],
                requestedExtensions: ["xcode"],
                requiredRuntimeExtensionIDs: ["xcode"],
                optionalRuntimeExtensionIDs: [],
                predictedEffectiveExtensions: ["xcode"],
                predictedEffectiveRuntimeExtensionIDs: ["xcode"],
                deniedExtensions: [],
                warnings: [],
                blockingIssues: []
            )
        ]
        runA.resolvedMCPPoliciesJSON = try JSONEncoder().encode(policiesA)
        runB.resolvedMCPPoliciesJSON = try JSONEncoder().encode(policiesB)

        let comparison = try #require(RunComparisonService(modelContext: context).compare(runA, runB))
        let bindingA = try #require(comparison.bindingsA.first(where: { $0.agentID == "proposal_reviewer_ui" }))
        let bindingB = try #require(comparison.bindingsB.first(where: { $0.agentID == "proposal_reviewer_ui" }))

        #expect(bindingA.requestedMCPExtensions == ["xcode", "context7"])
        #expect(bindingA.predictedMCPExtensions == ["xcode", "context7"])
        #expect(bindingA.actualMCPExtensions == ["xcode"])
        #expect(bindingA.deniedMCPExtensions == ["context7"])
        #expect(bindingB.requestedMCPExtensions == ["xcode"])
        #expect(bindingB.predictedMCPExtensions == ["xcode"])
        #expect(bindingB.actualMCPExtensions == ["xcode"])
        #expect(bindingB.deniedMCPExtensions.isEmpty)
    }
}
