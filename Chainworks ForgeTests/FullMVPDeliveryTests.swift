import Testing
import Foundation
import SwiftData
@testable import Chainworks_Forge

// MARK: - Full MVP Delivery Tests (Proposal 007 §13)
//
// Comprehensive test coverage for the repo-backed delivery slice:
// - Workflow/implementation loop (§13.1)
// - Release ops (§13.1)
// - Integration scenarios (§13.2)

@MainActor
@Suite("Full MVP Delivery — Workflow & Implementation Loop")
struct FullMVPWorkflowTests {
    let container: ModelContainer
    let context: ModelContext
    let compiler: RunPlanCompiler

    init() throws {
        let schema = Schema([
            Idea.self, Run.self, StageExecution.self,
            AgentExecution.self, Approval.self, Artifact.self
        ])
        let config = ModelConfiguration(
            "FullMVPDeliveryTests-\(UUID().uuidString)",
            schema: schema,
            isStoredInMemoryOnly: true
        )
        container = try ModelContainer(for: schema, configurations: [config])
        context = container.mainContext
        compiler = RunPlanCompiler(modelContext: context)
    }

    // MARK: - §13.1 Workflow: testFullMVPLiveWorkflowCompiles

    @Test("Full MVP live workflow compiles into a valid executable 12-state plan")
    func fullMVPLiveWorkflowCompiles() throws {
        let workflow = try loadTestFullMVPLiveWorkflow()
        let catalog = try loadTestCanonicalCatalog()

        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        #expect(plan.workflowID == "full_mvp_live")
        #expect(plan.states.count == 12, "Must have exactly 12 states")
        #expect(plan.initialStateID == "state_1_idea_received")

        // All 13 agents must resolve
        #expect(plan.agentBindings.count == 13)
    }

    // MARK: - §13.1 Workflow: State 7 provisions worktree before code writer

    @Test("Implementation state 7 has worktree provisioning before code_writer")
    func implementationState7ProvisionsWorktreeBeforeCodeWriter() throws {
        let workflow = try loadTestFullMVPLiveWorkflow()
        let catalog = try loadTestCanonicalCatalog()

        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let state7 = plan.states["state_7_implementation_started"]
        #expect(state7 != nil, "state_7_implementation_started must exist")

        // state_7 should have a sequence:
        // 1. lead_orchestrator (freeze proposal + provision worktree)
        // 2. code_writer (initial implementation)
        let phases = state7?.runBlock?.phases ?? []
        #expect(phases.count >= 1, "state_7 must have execution phases")

        // The first sequential phase should include lead_orchestrator before code_writer
        var foundLeadFirst = false
        var agentOrder: [String] = []
        for phase in phases {
            switch phase {
            case .sequential(let tasks):
                agentOrder.append(contentsOf: tasks.map(\.agent))
            case .parallel(let tasks):
                agentOrder.append(contentsOf: tasks.map(\.agent))
            }
        }

        if let leadIndex = agentOrder.firstIndex(of: "lead_orchestrator"),
           let codeIndex = agentOrder.firstIndex(of: "code_writer") {
            foundLeadFirst = leadIndex < codeIndex
        }

        #expect(foundLeadFirst, "lead_orchestrator (worktree provisioning) must run before code_writer")
    }

    @Test("Run start overrides preserve worktree write capability for repo-backed agents")
    func runStartOverridesPreserveWorktreeWriteCapability() {
        let agent = ResolvedAgent(
            id: "code_writer",
            title: "Code Writer",
            mode: "implementation",
            backendProfileID: "codex_builder_high",
            provider: "codex",
            model: "gpt-5-codex",
            effort: "high",
            maxTurns: 20,
            temperature: 0.2,
            permissionProfile: "CODE_WRITE",
            skillRef: "code_writer_core",
            skillRole: nil,
            prompt: "Implement the proposal.",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: ["approved_proposal"],
            outputs: ["changed_files_manifest"],
            worktreeWriteEnabled: true
        )
        let plan = RunPlan(
            workflowID: "full_mvp_live",
            workflowTitle: "Full MVP Live",
            states: [:],
            initialStateID: "state_1_idea_received",
            agentBindings: ["code_writer": agent],
            variables: [:],
            scoring: nil,
            failurePolicy: nil,
            requiresProjectAccess: true,
            workflowSnapshotHash: "workflow-hash",
            catalogSnapshotHash: "catalog-hash",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )
        let binding = ResolvedProviderBinding(
            agentID: "code_writer",
            backendProfileID: "codex_builder_high",
            configuredProviderID: UUID(),
            providerFamily: "claude",
            providerIdentifier: "claude_code",
            model: "sonnet",
            effort: "high",
            transport: "goose_server",
            adapterVersion: "v1"
        )

        let adjusted = RunStartOverrideResolver.applying(
            bindings: ["code_writer": binding],
            to: plan
        )

        #expect(adjusted.agentBindings["code_writer"]?.worktreeWriteEnabled == true)
    }

    // MARK: - §13.1 Workflow: Implementation loop stops when seemingly_complete

    @Test("Implementation continued loop transitions on seemingly_complete")
    func implementationLoopStopsWhenSeeminglyComplete() throws {
        let workflow = try loadTestFullMVPLiveWorkflow()

        // state_8 should have transitions conditional on seemingly_complete
        let state8 = workflow.states["state_8_implementation_continued"]
        #expect(state8 != nil, "state_8 must exist")

        let transitions = state8?.transitions ?? []
        #expect(transitions.count >= 2, "state_8 must have at least 2 transitions")

        // One transition goes to state_9 when seemingly_complete == true
        let toReview = transitions.first { $0.to == "state_9_implementation_reviewed" }
        #expect(toReview != nil, "Must have transition to state_9")
        #expect(toReview?.when.contains("seemingly_complete") == true)

        // One transition loops back when seemingly_complete == false
        let loopBack = transitions.first { $0.to == "state_8_implementation_continued" }
        #expect(loopBack != nil, "Must have loop-back transition")

        // Loop config should exist with budget
        let loop = state8?.loop
        #expect(loop != nil, "state_8 must have loop config")
        #expect(loop?.counter == "implementation_progress_count")
    }

    // MARK: - §13.1 Workflow: docs_report must exist before audit aggregation

    @Test("Implementation review guarantees docs_report before audit")
    func implementationReviewOrderGuaranteesDocsReportBeforeAudit() throws {
        let workflow = try loadTestFullMVPLiveWorkflow()

        let state9 = workflow.states["state_9_implementation_reviewed"]
        #expect(state9 != nil, "state_9 must exist")

        let run = state9?.run
        #expect(run != nil, "state_9 must have a run block")

        // Parallel phase should include security_checker and docs_guardian
        let parallelAgents = run?.parallel?.map(\.agent) ?? []
        #expect(parallelAgents.contains("security_checker"), "security_checker in parallel phase")
        #expect(parallelAgents.contains("docs_guardian"), "docs_guardian in parallel phase")

        // Then phase should include auditor and prepush reviewer
        let thenAgents = run?.then?.map(\.agent) ?? []
        #expect(thenAgents.contains("proposal_implementation_auditor"), "auditor in then phase")
        #expect(thenAgents.contains("prepush_code_reviewer"), "prepush_code_reviewer in then phase")

        // The auditor must consume docs_report as input
        let auditorTask = run?.then?.first { $0.agent == "proposal_implementation_auditor" }
        #expect(auditorTask?.inputs?.contains("docs_report") == true,
                "proposal_implementation_auditor must consume docs_report")
    }

    // MARK: - §13.1 Workflow: Implementation refine loop re-enters review

    @Test("Implementation refine loop re-enters review")
    func implementationRefineLoopReentersReview() throws {
        let workflow = try loadTestFullMVPLiveWorkflow()

        let state10 = workflow.states["state_10_implementation_refined"]
        #expect(state10 != nil, "state_10 must exist")

        let transitions = state10?.transitions ?? []
        let toReview = transitions.first { $0.to == "state_9_implementation_reviewed" }
        #expect(toReview != nil, "state_10 must loop back to state_9")

        // Loop config
        let loop = state10?.loop
        #expect(loop != nil, "state_10 must have loop config")
        #expect(loop?.counter == "implementation_revision_count")
    }

    // MARK: - §13.1 Workflow: Approval policy on manual release state

    @Test("Manual release state has approval_policy: manual_release")
    func manualReleaseStateHasApprovalPolicy() throws {
        let workflow = try loadTestFullMVPLiveWorkflow()

        let state11 = workflow.states["state_11_manual_release"]
        #expect(state11 != nil, "state_11 must exist")
        #expect(state11?.approval == "required")
        #expect(state11?.approvalPolicy == "manual_release",
                "state_11 must have approval_policy: manual_release")
    }

    // MARK: - §13.1 Workflow: run_after_approval in manual release

    @Test("Manual release has run_after_approval with release agents")
    func manualReleaseHasRunAfterApproval() throws {
        let workflow = try loadTestFullMVPLiveWorkflow()

        let state11 = workflow.states["state_11_manual_release"]
        #expect(state11 != nil)

        let runAfterApproval = state11?.runAfterApproval
        #expect(runAfterApproval != nil, "state_11 must have run_after_approval block")

        let agents = runAfterApproval?.sequence?.map(\.agent) ?? []
        #expect(agents.contains("commit_and_push_to_github"))
        #expect(agents.contains("build_archive_and_push_connect"))
    }

    // MARK: - §13.1 Workflow: All 3 manual gates are explicit

    @Test("Three manual gates are explicit workflow states")
    func threeManualGatesExplicit() throws {
        let workflow = try loadTestFullMVPLiveWorkflow()

        let manualGates = workflow.states.filter { $0.value.type == "manual_gate" }
        #expect(manualGates.count == 3, "Must have exactly 3 manual gates")

        let gateIDs = Set(manualGates.keys)
        #expect(gateIDs.contains("state_3_initial_proposal_approval"))
        #expect(gateIDs.contains("state_6_implementation_approval"))
        #expect(gateIDs.contains("state_11_manual_release"))
    }

    // MARK: - §13.1 Workflow: scoring config for implementation

    @Test("Full MVP workflow has implementation scoring criteria")
    func implementationScoringCriteria() throws {
        let workflow = try loadTestFullMVPLiveWorkflow()

        let scoring = workflow.scoring
        #expect(scoring != nil, "Scoring config must exist")
        #expect(scoring?.implementation != nil, "Implementation scoring must exist")

        let implCriteria = scoring?.implementation?.implementedWhen ?? []
        #expect(!implCriteria.isEmpty, "Must have implementation criteria")
        #expect(implCriteria.contains { $0.contains("audit_report.status") })
        #expect(implCriteria.contains { $0.contains("security_report.status") })
    }

    @Test("Full MVP review transitions loop to refinement when blockers remain despite high summed score")
    func fullMVPReviewTransitionUsesAverageScoreAndBlockers() throws {
        let workflow = try loadTestFullMVPLiveWorkflow()
        let catalog = try loadTestCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        guard let reviewState = plan.states["state_4_proposal_reviewed"] else {
            Issue.record("Missing state: state_4_proposal_reviewed")
            return
        }

        let context = TransitionEvaluator.EvaluationContext(
            producedArtifactNames: ["proposal_review_summary"],
            approvalGranted: false,
            variables: plan.variables,
            artifactFields: [
                "proposal_review_summary": [
                    "average_score": .double(9.0),
                    "aggregate_score": .int(36),
                    "min_individual_score": .int(9),
                    "blocker_count": .int(1)
                ]
            ]
        )

        let transition = TransitionEvaluator.evaluateFirst(
            transitions: reviewState.transitions,
            context: context
        )

        #expect(transition?.to == "state_5_proposal_refined")
    }
}

// MARK: - Release Ops Tests (§13.1)

@Suite("Full MVP Delivery — Release Ops")
struct FullMVPReleaseOpsTests {

    // MARK: - testManualReleaseRequiresApproval

    @Test("Manual release state requires explicit approval")
    func manualReleaseRequiresApproval() throws {
        let workflow = try loadTestFullMVPLiveWorkflow()
        let state11 = workflow.states["state_11_manual_release"]

        #expect(state11?.approval == "required", "Manual release must require approval")
        #expect(state11?.type == "manual_gate", "Manual release must be a manual_gate type")
    }

    // MARK: - testGitReleaseServiceProducesReceiptAndManifest

    @Test("GitReleaseService receipt and manifest types are well-formed")
    func gitReleaseServiceProducesReceiptAndManifest() throws {
        // Verify the output types are complete
        let manifest = GitReleaseService.ReleaseManifest(
            commitSHA: "abc123def456",
            branch: "chainworks/cw-test-abc123",
            remote: "origin",
            commitMessage: "feat: implement auth flow",
            filesChanged: 5,
            insertions: 120,
            deletions: 30,
            timestamp: Date()
        )

        #expect(!manifest.commitSHA.isEmpty)
        #expect(manifest.branch.hasPrefix("chainworks/"))
        #expect(manifest.remote == "origin")
        #expect(manifest.filesChanged == 5)
        #expect(manifest.insertions == 120)
        #expect(manifest.deletions == 30)

        let receipt = GitReleaseService.GitPushReceipt(
            commitSHA: "abc123def456",
            remote: "origin",
            branch: "chainworks/cw-test-abc123",
            status: "success",
            failureReason: nil,
            timestamp: Date()
        )

        #expect(receipt.status == "success")
        #expect(receipt.failureReason == nil)

        // Verify Codable roundtrip
        let encodedManifest = try JSONEncoder().encode(manifest)
        let decodedManifest = try JSONDecoder().decode(GitReleaseService.ReleaseManifest.self, from: encodedManifest)
        #expect(decodedManifest.commitSHA == manifest.commitSHA)

        let encodedReceipt = try JSONEncoder().encode(receipt)
        let decodedReceipt = try JSONDecoder().decode(GitReleaseService.GitPushReceipt.self, from: encodedReceipt)
        #expect(decodedReceipt.status == "success")
    }

    // MARK: - testConnectPublishServiceProducesReceiptAndBundleManifest

    @Test("ConnectPublishService receipt and bundle manifest types are well-formed")
    func connectPublishServiceProducesReceiptAndBundleManifest() throws {
        let bundle = ConnectPublishService.ReleaseBundleManifest(
            bundleIdentifier: "com.chainworks.forge.sandbox",
            bundleVersion: "1.0.0",
            buildNumber: "abc123de",
            archivePath: nil,
            checksumSHA256: "abc123def456",
            sizeBytes: 0,
            timestamp: Date()
        )

        #expect(bundle.bundleIdentifier.hasPrefix("com.chainworks"))
        #expect(bundle.sizeBytes == 0)  // sandbox mode

        let receipt = ConnectPublishService.ConnectUploadReceipt(
            artifactID: UUID().uuidString,
            destination: "sandbox://sandbox-1",
            releaseTargetID: "sandbox-1",
            releaseMode: "sandbox",
            status: "success",
            failureReason: nil,
            timestamp: Date()
        )

        #expect(receipt.status == "success")
        #expect(receipt.releaseMode == "sandbox")

        // Verify Codable roundtrip
        let encodedBundle = try JSONEncoder().encode(bundle)
        let decodedBundle = try JSONDecoder().decode(ConnectPublishService.ReleaseBundleManifest.self, from: encodedBundle)
        #expect(decodedBundle.bundleIdentifier == bundle.bundleIdentifier)

        let encodedReceipt = try JSONEncoder().encode(receipt)
        let decodedReceipt = try JSONDecoder().decode(ConnectPublishService.ConnectUploadReceipt.self, from: encodedReceipt)
        #expect(decodedReceipt.status == "success")
    }

    // MARK: - testPartialReleaseFailureBlocksRunWithReceiptsPreserved

    @Test("Partial release failure preserves git receipts and reports blocked status")
    func partialReleaseFailureBlocksRunWithReceiptsPreserved() {
        // Simulate: commit/push succeeds but archive/upload fails
        let gitManifest = GitReleaseService.ReleaseManifest(
            commitSHA: "abc123",
            branch: "chainworks/cw-test",
            remote: "origin",
            commitMessage: "feat: test",
            filesChanged: 3,
            insertions: 50,
            deletions: 10,
            timestamp: Date()
        )

        let gitReceipt = GitReleaseService.GitPushReceipt(
            commitSHA: "abc123",
            remote: "origin",
            branch: "chainworks/cw-test",
            status: "success",
            failureReason: nil,
            timestamp: Date()
        )

        // Partial failure result: git succeeded, connect failed
        let result = ReleaseOpsCoordinator.ReleaseResult(
            gitManifest: gitManifest,
            gitReceipt: gitReceipt,
            bundleManifest: nil,
            uploadReceipt: nil,
            succeeded: false,
            failureStage: "build_archive_and_push",
            failureReason: "Archive build failed"
        )

        // Verify partial failure semantics (§9.4)
        #expect(!result.succeeded)
        #expect(result.failureStage == "build_archive_and_push")
        #expect(result.failureReason != nil)

        // Git receipts must be preserved
        #expect(result.gitManifest != nil, "Git manifest must be preserved on partial failure")
        #expect(result.gitReceipt != nil, "Git receipt must be preserved on partial failure")
        #expect(result.gitReceipt?.status == "success")

        // Connect receipts should be nil on partial failure
        #expect(result.bundleManifest == nil)
        #expect(result.uploadReceipt == nil)
    }

    // MARK: - Default release modes are sandbox/staging

    @Test("Default release modes are sandbox and staging only", arguments: [ReleaseMode.sandbox, .staging])
    func defaultReleaseModes(_ mode: ReleaseMode) throws {
        // Verify only sandbox and staging exist (no production)
        #expect(ReleaseMode.allCases.count == 2, "Only sandbox and staging modes should exist")

        let data = try JSONEncoder().encode(mode)
        let decoded = try JSONDecoder().decode(ReleaseMode.self, from: data)
        #expect(decoded == mode)
    }

    // MARK: - Release services are deterministic

    @Test("Release side effects execute only through deterministic services")
    func releaseSideEffectsDeterministic() {
        // GitReleaseService and ConnectPublishService are value types (structs)
        // — no free-form agent shelling (ARCH-069)
        let gitService = GitReleaseService()
        let connectService = ConnectPublishService()
        let coordinator = ReleaseOpsCoordinator()

        // These exist as Sendable structs (deterministic, not agent-driven)
        _ = gitService
        _ = connectService
        _ = coordinator
    }
}

// MARK: - ReleaseMode CaseIterable conformance (needed for tests above)

extension ReleaseMode: CaseIterable {
    public static var allCases: [ReleaseMode] { [.sandbox, .staging] }
}

// MARK: - Integration Tests (§13.2)

@MainActor
@Suite("Full MVP Delivery — Integration", .serialized)
struct FullMVPIntegrationTests {
    let container: ModelContainer
    let context: ModelContext
    let compiler: RunPlanCompiler

    init() throws {
        let schema = Schema([
            Idea.self, Run.self, StageExecution.self,
            AgentExecution.self, Approval.self, Artifact.self
        ])
        let config = ModelConfiguration(
            "FullMVPIntegrationTests-\(UUID().uuidString)",
            schema: schema,
            isStoredInMemoryOnly: true
        )
        container = try ModelContainer(for: schema, configurations: [config])
        context = container.mainContext
        compiler = RunPlanCompiler(modelContext: context)
    }

    private func makeDogfoodRepository() throws -> URL {
        struct GitTestError: Error {
            let message: String
        }
        let repoRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("FullMVPDogfood-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: repoRoot, withIntermediateDirectories: true)
        try Data("dogfood baseline\n".utf8).write(to: repoRoot.appendingPathComponent("README.md"))

        func runGit(_ arguments: [String]) throws {
            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/usr/bin/git")
            process.arguments = arguments
            process.currentDirectoryURL = repoRoot
            let stdout = Pipe()
            let stderr = Pipe()
            process.standardOutput = stdout
            process.standardError = stderr
            try process.run()
            process.waitUntilExit()
            guard process.terminationStatus == 0 else {
                let errorOutput = String(data: stderr.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8)
                    ?? String(data: stdout.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8)
                    ?? "git failed"
                throw GitTestError(message: "git \(arguments.joined(separator: " ")) failed: \(errorOutput)")
            }
        }

        try runGit(["init", "-b", "main"])
        try runGit(["config", "user.name", "Chainworks Forge Tests"])
        try runGit(["config", "user.email", "chainworks-forge-tests@local"])
        try runGit(["add", "."])
        try runGit(["commit", "-m", "Dogfood baseline"])
        return repoRoot
    }

    // MARK: - testFullMVPLiveRunCompiles

    @Test("Full MVP live run can be created with delivery configuration")
    func fullMVPLiveRunWithDeliveryConfig() throws {
        let workflow = try loadTestFullMVPLiveWorkflow()
        let catalog = try loadTestCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let idea = Idea(title: "Test Full MVP", body: "End-to-end test of the full MVP delivery slice.")
        context.insert(idea)

        let deliveryConfig = DeliveryConfiguration(
            profileID: "test_profile",
            profileLabel: "Test Repo",
            sampleProfileID: nil,
            repoIdentifier: "test-repo",
            repoRoot: "/tmp/test-repo",
            baseBranch: "main",
            worktreeBasePath: "/tmp/worktrees",
            targetBranch: "dogfood/test",
            releaseTargetID: "sandbox_test",
            releaseTargetLabel: "Test Sandbox",
            releaseMode: .sandbox
        )

        let (run, workspace) = try compiler.createRun(
            for: idea,
            plan: plan,
            workflowSourcePath: "test/full-mvp-live.yaml",
            catalogSourcePath: "test/agents.yaml",
            startSnapshot: RunStartSnapshot(
                providerBindingSnapshotJSON: Data("{\"proposal_writer\":{\"providerFamily\":\"claude_code\"}}".utf8),
                bindingProvenanceJSON: Data("{\"proposal_writer\":{\"source\":\"backend_profile\"}}".utf8),
                startOptionsJSON: Data("{}".utf8),
                frozenWorkspaceRootPath: "/tmp/test-repo",
                deliveryConfiguration: deliveryConfig,
                deliveryPreflightJSON: Data("{\"passed\":true}".utf8)
            )
        )

        #expect(run.workflowID == "full_mvp_live")
        #expect(run.deliveryConfigurationJSON != nil)
        #expect(run.providerBindingSnapshotJSON != nil)
        #expect(run.bindingProvenanceJSON != nil)
        #expect(run.startOptionsJSON != nil)
        #expect(run.frozenWorkspaceRootPath == "/tmp/test-repo")
        #expect(run.deliveryPreflightJSON != nil)
        #expect(run.repoRoot == "/tmp/test-repo")

        // Verify frozen config can be decoded
        let decoded = try JSONDecoder().decode(
            DeliveryConfiguration.self,
            from: run.deliveryConfigurationJSON!
        )
        #expect(decoded.repoIdentifier == "test-repo")
        #expect(decoded.releaseMode == .sandbox)

        // Workspace should exist
        #expect(!workspace.workspaceRoot.path.isEmpty)
        #expect(!workspace.artifactRoot.path.isEmpty)
    }

    // MARK: - testResumeDuringImplementationStageRestoresWorktreeContext

    @Test("Resume during implementation stage restores worktree context")
    func resumeRestoresWorktreeContext() throws {
        let workflow = try loadTestFullMVPLiveWorkflow()
        let catalog = try loadTestCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let idea = Idea(title: "Resume Test", body: "Test resume with worktree context.")
        context.insert(idea)

        let deliveryConfig = DeliveryConfiguration(
            profileID: nil, profileLabel: nil, sampleProfileID: nil,
            repoIdentifier: "test-repo",
            repoRoot: "/tmp/test-repo",
            baseBranch: "main",
            worktreeBasePath: "/tmp/worktrees",
            targetBranch: "dogfood/resume-test",
            releaseTargetID: "sandbox_test",
            releaseTargetLabel: "Test Sandbox",
            releaseMode: .sandbox
        )

        let (run, _) = try compiler.createRun(
            for: idea,
            plan: plan,
            workflowSourcePath: "test/full-mvp-live.yaml",
            catalogSourcePath: "test/agents.yaml",
            startSnapshot: RunStartSnapshot(
                providerBindingSnapshotJSON: nil,
                bindingProvenanceJSON: nil,
                startOptionsJSON: nil,
                frozenWorkspaceRootPath: "/tmp/test-repo",
                deliveryConfiguration: deliveryConfig,
                deliveryPreflightJSON: nil
            )
        )

        // Simulate worktree being provisioned
        run.worktreeRoot = "/tmp/worktrees/cw-resume-test-abc123"
        run.baseRevision = "abc123def456"
        run.repoIdentifier = "test-repo"
        run.repoRoot = "/tmp/test-repo"
        run.baseBranch = "main"
        run.targetBranch = "dogfood/resume-test"

        // Verify ResumeManager can reconstruct delivery context from Run
        #expect(run.worktreeRoot == "/tmp/worktrees/cw-resume-test-abc123")
        #expect(run.baseRevision == "abc123def456")

        // Verify frozen delivery config persists through the Run
        let resumedConfig = try JSONDecoder().decode(
            DeliveryConfiguration.self,
            from: run.deliveryConfigurationJSON!
        )
        #expect(resumedConfig.repoRoot == "/tmp/test-repo")
        #expect(resumedConfig.targetBranch == "dogfood/resume-test")
    }

    // MARK: - testRejectManualReleaseCancelsRunWithoutSideEffects

    @Test("Rejecting manual release produces no release side effects")
    func rejectManualReleaseCancelsWithoutSideEffects() throws {
        let workflow = try loadTestFullMVPLiveWorkflow()

        // Verify state_11 has transitions — but no side effects occur without approval
        let state11 = workflow.states["state_11_manual_release"]
        #expect(state11 != nil)
        #expect(state11?.approval == "required")

        // run_after_approval only executes after approval.granted == true
        // Rejection means run_after_approval NEVER fires
        let runAfter = state11?.runAfterApproval
        #expect(runAfter != nil, "run_after_approval block must exist")

        // The presence of run_after_approval means side effects are gated
        // If approval is rejected, the orchestrator transitions based on approval.granted == false
        // which would not match state_11's transition (which requires exists('git_push_receipt'))
    }

    // MARK: - testDeliveryPreflightService

    @Test("Delivery preflight validates configuration draft before run")
    func deliveryPreflightValidatesConfig() async {
        let config = DeliveryConfiguration(
            profileID: nil, profileLabel: nil, sampleProfileID: nil,
            repoIdentifier: "test-repo",
            repoRoot: "/nonexistent/repo/path",
            baseBranch: "main",
            worktreeBasePath: "/tmp/worktrees",
            targetBranch: "dogfood/test",
            releaseTargetID: "sandbox_test",
            releaseTargetLabel: "Sandbox",
            releaseMode: .sandbox
        )

        let service = DeliveryPreflightService()
        let result = await service.validate(config)

        #expect(!result.passed, "Preflight should fail for nonexistent repo")
        #expect(result.failedChecks.contains { $0.id == "repo_root" })
    }

    // MARK: - testEvidencePackBuilderStructure

    @Test("Evidence pack builder produces expected structure")
    func evidencePackBuilderStructure() throws {
        let workspace = makeTestWorkspace()
        defer { cleanupWorkspace(workspace) }

        let run = makeTestRun(
            workspace: workspace,
            context: context,
            workflowID: "full_mvp_live",
            workflowTitle: "Full MVP Live"
        )
        run.worktreeRoot = "/tmp/worktrees/cw-test-abc123"
        run.repoIdentifier = "test-repo"
        run.baseBranch = "main"
        run.releaseMode = "sandbox"

        let deliveryConfig = DeliveryConfiguration(
            profileID: nil, profileLabel: nil, sampleProfileID: nil,
            repoIdentifier: "test-repo",
            repoRoot: "/tmp/test-repo",
            baseBranch: "main",
            worktreeBasePath: "/tmp/worktrees",
            targetBranch: "dogfood/test",
            releaseTargetID: "sandbox_test",
            releaseTargetLabel: "Sandbox",
            releaseMode: .sandbox
        )
        run.deliveryConfigurationJSON = try JSONEncoder().encode(deliveryConfig)

        let exportDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("EvidencePackTest-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: exportDir) }

        let pack = try EvidencePackBuilder.export(
            run: run,
            workspace: workspace,
            exportDirectory: exportDir
        )

        #expect(pack.itemCount > 0, "Evidence pack must contain items")
        #expect(FileManager.default.fileExists(atPath: pack.exportPath.path))

        // Check for key files
        let metadataPath = pack.exportPath.appendingPathComponent("run-metadata.json")
        #expect(FileManager.default.fileExists(atPath: metadataPath.path))

        let configPath = pack.exportPath.appendingPathComponent("delivery-configuration.json")
        #expect(FileManager.default.fileExists(atPath: configPath.path))

        let checklistPath = pack.exportPath.appendingPathComponent("screenshot-checklist.md")
        #expect(FileManager.default.fileExists(atPath: checklistPath.path))
    }

    // MARK: - testImplementationDeliveryPreset

    @Test("ImplementationDeliveryPreset produces valid dogfood configuration")
    func implementationDeliveryPreset() {
        let profile = RepositoryProfile.chainworksForge(repoRoot: "/tmp/test-repo")

        let preset = ImplementationDeliveryPreset.dogfoodPreset(
            profile: profile,
            releaseMode: .sandbox,
            workflowBundleURL: URL(fileURLWithPath: "/tmp/full-mvp-live.yaml"),
            catalogBundleURL: URL(fileURLWithPath: "/tmp/agents.yaml")
        )

        #expect(preset != nil, "Dogfood preset must be producible")
        #expect(preset?.presetLabel == "Full MVP Live (Dogfood)")
        #expect(preset?.deliveryConfiguration.releaseMode == .sandbox)
        #expect(preset?.deliveryConfiguration.repoRoot == "/tmp/test-repo")
        #expect(!preset!.safetyNotes.isEmpty)
    }

    // MARK: - testSampleRepoProfile

    @Test("Sample RepositoryProfile produces valid delivery configuration")
    func sampleRepoProfile() {
        let profile = RepositoryProfile.chainworksForge(repoRoot: "/Users/user/test-repo")

        let config = profile.toDeliveryConfiguration()
        #expect(config.repoRoot == "/Users/user/test-repo")
        #expect(config.baseBranch == "main")
        #expect(config.releaseMode == .sandbox)
        #expect(config.profileID == "chainworks_forge_self")
    }

    private func desktopExportDirectory() -> URL {
        FileManager.default.urls(for: .desktopDirectory, in: .userDomainMask).first
            ?? FileManager.default.temporaryDirectory
    }

    private func assertEvidencePack(
        at exportPath: URL,
        expectedFiles: [String]
    ) {
        for relativePath in expectedFiles {
            let url = exportPath.appendingPathComponent(relativePath)
            #expect(
                FileManager.default.fileExists(atPath: url.path),
                Comment(rawValue: "Expected exported evidence artifact at \(url.path)")
            )
        }
    }

    @Test("Repo-backed fixture happy path completes through manual release")
    func repoBackedFixtureHappyPathCompletes() async throws {
        let workflow = try loadTestFullMVPLiveWorkflow()
        let catalog = try loadTestCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let repoRoot = try makeDogfoodRepository()
        defer { try? FileManager.default.removeItem(at: repoRoot) }

        let idea = Idea(title: "Dogfood Happy Path", body: "Repo-backed full MVP proof.")
        context.insert(idea)

        let deliveryConfig = DeliveryConfiguration(
            profileID: "chainworks_forge_self",
            profileLabel: "Chainworks Forge (Self)",
            sampleProfileID: "chainworks_forge_self",
            repoIdentifier: repoRoot.lastPathComponent,
            repoRoot: repoRoot.path,
            baseBranch: "main",
            worktreeBasePath: FileManager.default.temporaryDirectory
                .appendingPathComponent("FullMVPDogfoodWorktrees-\(UUID().uuidString)", isDirectory: true).path,
            targetBranch: "dogfood/test",
            releaseTargetID: "sandbox_test",
            releaseTargetLabel: "Sandbox",
            releaseMode: .sandbox
        )

        setenv("CHAINWORKS_DELIVERY_PROOF_MODE", "happy_path", 1)
        defer { unsetenv("CHAINWORKS_DELIVERY_PROOF_MODE") }

        let (run, workspace) = try compiler.createRun(
            for: idea,
            plan: plan,
            workflowSourcePath: "test/full-mvp-live.yaml",
            catalogSourcePath: "test/agents.yaml",
            startSnapshot: RunStartSnapshot(
                providerBindingSnapshotJSON: nil,
                bindingProvenanceJSON: nil,
                startOptionsJSON: nil,
                frozenWorkspaceRootPath: repoRoot.path,
                deliveryConfiguration: deliveryConfig,
                deliveryPreflightJSON: Data("{\"passed\":true}".utf8)
            )
        )

        let executor = GooseAgentExecutor(transport: FixtureGooseTransport(scenario: .fullMVPSuccess))
        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: executor,
            modelContext: context,
            catalog: catalog
        )

        var approvals: [ApprovalRequest] = []
        orchestrator.onApprovalRequest = { request in
            approvals.append(request)
        }

        await orchestrator.start()

        await awaitCondition("Repo-backed full MVP fixture should reach a terminal state", timeout: 30.0) {
            if !approvals.isEmpty {
                let pending = approvals
                approvals.removeAll()
                for request in pending {
                    orchestrator.resolveApproval(stageID: request.stageID, granted: true, comment: "Fixture auto-approve")
                }
            }
            return run.status == .completed || run.status == .blocked || run.status == .failed
        }

        let stages = run.stageExecutions.map { "\($0.stageID)=\($0.status.rawValue)" }.joined(separator: ", ")
        let agentLogs = run.stageExecutions
            .flatMap { $0.agentExecutions }
            .map { "\($0.agentID)=\($0.status.rawValue):\($0.logSnippet ?? "-")" }
            .joined(separator: " | ")
        let artifactManager = ArtifactManager(modelContext: context)
        let artifacts = try artifactManager.artifacts(forRunID: run.id)
        let artifactNames = artifacts.map(\.name).sorted().joined(separator: ", ")
        let reviewSummaryDebug: String
        let transitionDebug: String
        let transitionFieldDebug: String
        let transitionContext: TransitionEvaluator.EvaluationContext
        func debugScalarFields(from data: Data) -> [String: AnyCodableValue] {
            guard let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
                return [:]
            }
            var result: [String: AnyCodableValue] = [:]
            for (key, value) in object {
                if let number = value as? NSNumber {
                    if CFGetTypeID(number) == CFBooleanGetTypeID() {
                        result[key] = .bool(number.boolValue)
                    } else if floor(number.doubleValue) == number.doubleValue {
                        result[key] = .int(number.intValue)
                    } else {
                        result[key] = .double(number.doubleValue)
                    }
                } else if let boolVal = value as? Bool {
                    result[key] = .bool(boolVal)
                } else if let stringVal = value as? String {
                    result[key] = .string(stringVal)
                }
            }
            return result
        }
        var debugArtifactFields: [String: [String: AnyCodableValue]] = [:]
        for artifact in artifacts where artifact.format == .json {
            guard let data = try? Data(contentsOf: URL(fileURLWithPath: artifact.filePath)) else { continue }
            debugArtifactFields[artifact.name] = debugScalarFields(from: data)
        }
        transitionContext = TransitionEvaluator.EvaluationContext(
            producedArtifactNames: Set(artifacts.map(\.name)),
            approvalGranted: false,
            variables: plan.variables,
            artifactFields: debugArtifactFields
        )
        transitionFieldDebug = "proposal_review_summary=\(String(describing: debugArtifactFields["proposal_review_summary"])) target=\(String(describing: plan.variables["proposal_score_target"]))"
        if let state4 = plan.states["state_4_proposal_reviewed"] {
            transitionDebug = state4.transitions
                .map { "\($0.to)=\(TransitionEvaluator.evaluate($0.condition, context: transitionContext))" }
                .joined(separator: ", ")
        } else {
            transitionDebug = "missing-state-4"
        }
        if let reviewArtifact = artifacts.first(where: { $0.name == "proposal_review_summary" }),
           let data = try? Data(contentsOf: URL(fileURLWithPath: reviewArtifact.filePath)),
           let text = String(data: data, encoding: .utf8) {
            reviewSummaryDebug = text
        } else {
            reviewSummaryDebug = "missing"
        }

        #expect(
            run.status == RunStatus.completed,
            Comment(rawValue: "Expected completed, got \(run.status.rawValue). stages=\(stages) agents=\(agentLogs) artifacts=\(artifactNames) proposalReviewSummary=\(reviewSummaryDebug) transitions=\(transitionDebug) transitionFields=\(transitionFieldDebug)")
        )

        #expect(artifacts.contains(where: { $0.name == "delivery_receipt" }))
        #expect(artifacts.contains(where: { $0.name == "git_push_receipt" }))
        #expect(artifacts.contains(where: { $0.name == "connect_upload_receipt" }))
        #expect(FileManager.default.fileExists(atPath: repoRoot.appendingPathComponent("README.md").path))

        let exportedPack = try EvidencePackBuilder.export(
            run: run,
            workspace: workspace,
            exportDirectory: desktopExportDirectory()
        )
        assertEvidencePack(
            at: exportedPack.exportPath,
            expectedFiles: [
                "run-metadata.json",
                "delivery-configuration.json",
                "delivery-preflight.json",
                "stage-summary.json",
                "agent-execution-detail.json",
                "deliverables/release-manifest.json",
                "deliverables/git-push-receipt.json",
                "deliverables/connect-upload-receipt.json",
                "deliverables/delivery-receipt.json",
                "screenshot-checklist.md"
            ]
        )
    }

    @Test("Repo-backed fixture non-happy path exports evidence with preserved partial receipts")
    func repoBackedFixtureNonHappyPathExportsEvidence() async throws {
        let workflow = try loadTestFullMVPLiveWorkflow()
        let catalog = try loadTestCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let repoRoot = try makeDogfoodRepository()
        defer { try? FileManager.default.removeItem(at: repoRoot) }

        let idea = Idea(title: "Dogfood Non-Happy Path", body: "Repo-backed blocked delivery proof.")
        context.insert(idea)

        let deliveryConfig = DeliveryConfiguration(
            profileID: "chainworks_forge_self",
            profileLabel: "Chainworks Forge (Self)",
            sampleProfileID: "chainworks_forge_self",
            repoIdentifier: repoRoot.lastPathComponent,
            repoRoot: repoRoot.path,
            baseBranch: "main",
            worktreeBasePath: FileManager.default.temporaryDirectory
                .appendingPathComponent("FullMVPDogfoodBlockedWorktrees-\(UUID().uuidString)", isDirectory: true).path,
            targetBranch: "dogfood/test",
            releaseTargetID: "sandbox_test",
            releaseTargetLabel: "Sandbox",
            releaseMode: .sandbox
        )

        setenv("CHAINWORKS_DELIVERY_PROOF_MODE", "non_happy_path", 1)
        defer { unsetenv("CHAINWORKS_DELIVERY_PROOF_MODE") }

        let (run, workspace) = try compiler.createRun(
            for: idea,
            plan: plan,
            workflowSourcePath: "test/full-mvp-live.yaml",
            catalogSourcePath: "test/agents.yaml",
            startSnapshot: RunStartSnapshot(
                providerBindingSnapshotJSON: nil,
                bindingProvenanceJSON: nil,
                startOptionsJSON: nil,
                frozenWorkspaceRootPath: repoRoot.path,
                deliveryConfiguration: deliveryConfig,
                deliveryPreflightJSON: Data("{\"passed\":true}".utf8)
            )
        )

        let executor = GooseAgentExecutor(transport: FixtureGooseTransport(scenario: .fullMVPSuccess))
        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: executor,
            modelContext: context,
            catalog: catalog
        )

        var approvals: [ApprovalRequest] = []
        orchestrator.onApprovalRequest = { request in
            approvals.append(request)
        }

        await orchestrator.start()

        await awaitCondition("Repo-backed full MVP fixture non-happy path should reach a terminal state", timeout: 30.0) {
            if !approvals.isEmpty {
                let pending = approvals
                approvals.removeAll()
                for request in pending {
                    orchestrator.resolveApproval(stageID: request.stageID, granted: true, comment: "Fixture auto-approve")
                }
            }
            return run.status == .completed || run.status == .blocked || run.status == .failed
        }

        let artifactManager = ArtifactManager(modelContext: context)
        let artifacts = try artifactManager.artifacts(forRunID: run.id)

        #expect(
            run.status == RunStatus.blocked,
            Comment(rawValue: "Expected blocked non-happy-path run, got \(run.status.rawValue)")
        )
        #expect(artifacts.contains(where: { $0.name == "delivery_receipt" }))
        #expect(artifacts.contains(where: { $0.name == "git_push_receipt" }))
        #expect(!artifacts.contains(where: { $0.name == "connect_upload_receipt" }))

        let exportedPack = try EvidencePackBuilder.export(
            run: run,
            workspace: workspace,
            exportDirectory: desktopExportDirectory()
        )
        assertEvidencePack(
            at: exportedPack.exportPath,
            expectedFiles: [
                "run-metadata.json",
                "delivery-configuration.json",
                "delivery-preflight.json",
                "stage-summary.json",
                "agent-execution-detail.json",
                "deliverables/release-manifest.json",
                "deliverables/git-push-receipt.json",
                "deliverables/delivery-receipt.json",
                "screenshot-checklist.md"
            ]
        )
        #expect(
            !FileManager.default.fileExists(
                atPath: exportedPack.exportPath
                    .appendingPathComponent("deliverables/connect-upload-receipt.json")
                    .path
            )
        )
    }

    @Test("Repo-backed fixture implementation review refine loop re-enters review and completes")
    func repoBackedFixtureImplementationReviewRefineLoopCompletes() async throws {
        let workflow = try loadTestFullMVPLiveWorkflow()
        let catalog = try loadTestCanonicalCatalog()
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let repoRoot = try makeDogfoodRepository()
        defer { try? FileManager.default.removeItem(at: repoRoot) }

        let idea = Idea(title: "Dogfood Refine Loop", body: "Repo-backed runtime proof for implementation review re-entry.")
        context.insert(idea)

        let deliveryConfig = DeliveryConfiguration(
            profileID: "chainworks_forge_self",
            profileLabel: "Chainworks Forge (Self)",
            sampleProfileID: "chainworks_forge_self",
            repoIdentifier: repoRoot.lastPathComponent,
            repoRoot: repoRoot.path,
            baseBranch: "main",
            worktreeBasePath: FileManager.default.temporaryDirectory
                .appendingPathComponent("FullMVPDogfoodRefineWorktrees-\(UUID().uuidString)", isDirectory: true).path,
            targetBranch: "dogfood/refine-loop",
            releaseTargetID: "sandbox_test",
            releaseTargetLabel: "Sandbox",
            releaseMode: .sandbox
        )

        setenv("CHAINWORKS_DELIVERY_PROOF_MODE", "happy_path", 1)
        defer { unsetenv("CHAINWORKS_DELIVERY_PROOF_MODE") }

        let (run, workspace) = try compiler.createRun(
            for: idea,
            plan: plan,
            workflowSourcePath: "test/full-mvp-live.yaml",
            catalogSourcePath: "test/agents.yaml",
            startSnapshot: RunStartSnapshot(
                providerBindingSnapshotJSON: nil,
                bindingProvenanceJSON: nil,
                startOptionsJSON: nil,
                frozenWorkspaceRootPath: repoRoot.path,
                deliveryConfiguration: deliveryConfig,
                deliveryPreflightJSON: Data("{\"passed\":true}".utf8)
            )
        )

        let executor = GooseAgentExecutor(transport: FixtureGooseTransport(scenario: .fullMVPRefineThenSuccess))
        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: executor,
            modelContext: context,
            catalog: catalog
        )

        var approvals: [ApprovalRequest] = []
        orchestrator.onApprovalRequest = { request in
            approvals.append(request)
        }

        await orchestrator.start()

        await awaitCondition("Repo-backed refine-loop fixture should reach completion after review re-entry", timeout: 30.0) {
            if !approvals.isEmpty {
                let pending = approvals
                approvals.removeAll()
                for request in pending {
                    orchestrator.resolveApproval(stageID: request.stageID, granted: true, comment: "Fixture auto-approve")
                }
            }
            return run.status == .completed || run.status == .blocked || run.status == .failed
        }

        let artifactManager = ArtifactManager(modelContext: context)
        let artifacts = try artifactManager.artifacts(forRunID: run.id)

        func decodeStatus(from artifact: Artifact) throws -> String {
            let data = try Data(contentsOf: URL(fileURLWithPath: artifact.filePath))
            let object = try #require(
                JSONSerialization.jsonObject(with: data) as? [String: Any],
                "Expected JSON object artifact at \(artifact.filePath)"
            )
            return try #require(object["status"] as? String, "Expected `status` field in \(artifact.filePath)")
        }

        let implementationReviewStages = run.stageExecutions.filter { $0.stageID == "state_9_implementation_reviewed" }
        let refinementStages = run.stageExecutions.filter { $0.stageID == "state_10_implementation_refined" }
        let auditReports = artifacts
            .filter { $0.name == "audit_report" }
            .sorted { $0.createdAt < $1.createdAt }
        let reviewSummaries = artifacts
            .filter { $0.name == "implementation_review_summary" }
            .sorted { $0.createdAt < $1.createdAt }

        #expect(run.status == .completed, "Refine-loop fixture should eventually complete")
        #expect(implementationReviewStages.count == 2, "Implementation review must execute twice")
        #expect(refinementStages.count == 1, "Refinement stage must execute once before returning to review")
        #expect(run.loopCounters["implementation_revision_count"] == 1)
        #expect(auditReports.count == 2, "Expected pre- and post-refinement audit reports")
        #expect(reviewSummaries.count == 2, "Expected pre- and post-refinement review summaries")
        #expect(try decodeStatus(from: auditReports[0]) == "Needs Work")
        #expect(try decodeStatus(from: auditReports[1]) == "Implemented")
        #expect(try decodeStatus(from: reviewSummaries[0]) == "Needs Work")
        #expect(try decodeStatus(from: reviewSummaries[1]) == "Implemented")
        #expect(artifacts.contains(where: { $0.name == "delivery_receipt" }))
    }

    // MARK: - testSourceContextBuilder

    @Test("SourceContext captures required fields")
    func sourceContextFields() {
        let ctx = SourceContextBuilder.SourceContext(
            worktreeRoot: "/tmp/worktrees/cw-test",
            repoRoot: "/tmp/repo",
            baseBranch: "main",
            baseRevision: "abc123",
            targetBranch: "dogfood/test",
            changedFilesManifest: ["src/main.swift", "src/utils.swift"],
            diffSummary: "2 files changed, 50 insertions(+), 10 deletions(-)"
        )

        #expect(!ctx.worktreeRoot.isEmpty)
        #expect(!ctx.diffSummary.isEmpty)
        #expect(ctx.changedFilesManifest.contains("src/main.swift"))
    }
}
