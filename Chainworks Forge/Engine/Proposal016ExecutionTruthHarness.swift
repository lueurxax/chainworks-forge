import Foundation
import SwiftData

// RunRepository-exempt: Proposal 016 proof harness seeds synthetic runs intentionally.

struct Proposal016ExecutionTruthProofResult: Codable, Sendable {
    let passed: Bool
    let proofStatus: String
    let steps: [String]
    let limitReason: String
    let runtimeTrust: String
    let bindingSummary: String
    let limitRecoverySummary: String
    let policySummary: String
    let repairSummary: String
    let reportPath: String?
}

enum Proposal016ExecutionTruthHarnessError: LocalizedError {
    case missingResultPath

    var errorDescription: String? {
        switch self {
        case .missingResultPath:
            return "Proposal 016 proof harness requires CHAINWORKS_P016_RESULT_PATH."
        }
    }
}

@MainActor
final class Proposal016ExecutionTruthHarness {
    static let isEnabled = ProcessInfo.processInfo.environment["CHAINWORKS_P016_PROOF_AUTORUN"] == "1"

    private let modelContext: ModelContext
    private let repositoryRoot: URL

    init(modelContext: ModelContext, repositoryRoot: URL? = nil) {
        self.modelContext = modelContext
        self.repositoryRoot = repositoryRoot ?? AppConfiguration.defaultRepositoryRoot()
    }

    func runFromEnvironment() async throws -> Proposal016ExecutionTruthProofResult {
        guard let path = ProcessInfo.processInfo.environment["CHAINWORKS_P016_RESULT_PATH"]?
            .trimmingCharacters(in: .whitespacesAndNewlines),
            !path.isEmpty else {
            throw Proposal016ExecutionTruthHarnessError.missingResultPath
        }

        do {
            let result = try runProof()
            try persist(result: result, to: URL(fileURLWithPath: path))
            return result
        } catch {
            let result = Proposal016ExecutionTruthProofResult(
                passed: false,
                proofStatus: "FAIL — \(error.localizedDescription)",
                steps: [],
                limitReason: "—",
                runtimeTrust: "—",
                bindingSummary: "—",
                limitRecoverySummary: "—",
                policySummary: "—",
                repairSummary: "—",
                reportPath: nil
            )
            try persist(result: result, to: URL(fileURLWithPath: path))
            throw error
        }
    }

    func runProof() throws -> Proposal016ExecutionTruthProofResult {
        var steps: [String] = []

        let plan = try loadProposalLoopPlan()
        let compiler = RunPlanCompiler(modelContext: modelContext)

        let repairRun = try makeProofRun(title: "P016 Repair Proof", plan: plan)
        seedRepairProof(into: repairRun)
        let interruptedActions = try ResumeManager(modelContext: modelContext)
            .classifyInterruptedRuns(compiler: compiler)
        let repairActiveCount = repairRun.stageExecutions.filter {
            $0.lineageID == "state_2_proposal_drafted::iteration:1"
                && ($0.status == .running || $0.status == .ready || $0.status == .waitingApproval)
        }.count
        let repairRequestedApprovals = repairRun.approvals.filter {
            $0.stageID == "state_4_proposal_approval" && $0.decision == .requested
        }.count
        let repairExpiredApprovals = repairRun.approvals.filter {
            $0.stageID == "state_4_proposal_approval" && $0.decision == .expired
        }.count
        let repairSummary =
            "actions=\(interruptedActions.count) " +
            "active-stage-siblings=\(repairActiveCount) " +
            "requested-approvals=\(repairRequestedApprovals) " +
            "expired-approvals=\(repairExpiredApprovals)"
        steps.append("[1/4] Startup repair collapsed stale active lineage and duplicate approvals")

        let limitRun = try makeProofRun(title: "P016 Limit Exhaustion Proof", plan: plan)
        let limitEvidence = try seedLimitExhaustionProof(into: limitRun)
        let reportBuilder = RunReportBuilder(modelContext: modelContext)
        let limitPayload = reportBuilder.buildReportPayload(for: limitRun, version: 1)
        let limitRecovery = RecoveryCoordinator(modelContext: modelContext).recoveryContext(for: limitRun)
        let limitReason = limitPayload.blockedReason ?? "no blocked reason"
        let runtimeTrust = "runtimeTrust=\(limitPayload.runtimeTrustLevel)"
        let bindingSummary =
            "frozen=\(limitEvidence.frozen.providerFamily)/\(limitEvidence.frozen.model) " +
            "runtime=\(limitEvidence.runtimeProvider)/\(limitEvidence.runtimeModel)"
        let limitRecoverySummary =
            "suggested=\(limitRecovery.suggestedAction?.label ?? "none") " +
            "allowed=\(limitRecovery.allowedActions.map(\.label).joined(separator: ", "))"
        steps.append("[2/4] Limit exhaustion preserved durable output and emitted canonical blocked reason")

        let policyRun = try makeProofRun(title: "P016 Policy Stop Proof", plan: plan)
        let policyRecovery = try seedPolicyStopProof(into: policyRun)
        let policySummary =
            "suggested=\(policyRecovery.suggestedAction?.label ?? "none") " +
            "allowed=\(policyRecovery.allowedActions.map(\.label).joined(separator: ", "))"
        steps.append("[3/4] Policy-bound stop suppressed default same-run retry")

        let legacyRun = try makeProofRun(title: "P016 Legacy Unverifiable Proof", plan: plan)
        seedLegacyUnverifiableProof(into: legacyRun)
        let legacyActions = try ResumeManager(modelContext: modelContext)
            .classifyInterruptedRuns(compiler: compiler)
        let legacyAction = legacyActions.first { action in
            switch action {
            case .resume(let run, _, _), .needsDecision(let run, _), .cannotResume(let run, _):
                return run.id == legacyRun.id
            }
        }
        let legacyRequiresDecision: Bool
        let legacyReason: String
        switch legacyAction {
        case .needsDecision(_, let reason):
            legacyRequiresDecision = true
            legacyReason = reason
        case .cannotResume(_, let reason):
            legacyRequiresDecision = false
            legacyReason = reason
        case .resume, nil:
            legacyRequiresDecision = false
            legacyReason = "unexpected resume"
        }
        let legacySummary =
            "legacy-action=\(legacyRequiresDecision ? "needsDecision" : "unexpected") " +
            "legacy-runtime-trust=\(legacyRun.runtimeTrustLevel ?? "unknown")"
        steps.append("[4/5] Legacy rows without canonical truth stay unverifiable and require explicit operator decision")

        let limitHasNoRetry = limitRecovery.allowedActions.allSatisfy { action in
            switch action {
            case .retryAgent, .retryAggregateStep, .retryStage:
                return false
            case .resumeRun, .resumeFromApprovalGate, .cloneRunFrozenSnapshot, .cloneRunCurrentConfig:
                return true
            }
        }
        let policyHasNoRetry = policyRecovery.allowedActions.allSatisfy { action in
            switch action {
            case .retryAgent, .retryAggregateStep, .retryStage:
                return false
            case .resumeRun, .resumeFromApprovalGate, .cloneRunFrozenSnapshot, .cloneRunCurrentConfig:
                return true
            }
        }
        let limitPass =
            limitEvidence.outputArtifactExists
            && limitEvidence.agent.canonicalOutcome == .limitExhaustedAfterOutput
            && limitPayload.runtimeTrustLevel == RuntimeBindingTrustLevel.unverifiable.rawValue
            && (limitPayload.blockedReason?.localizedCaseInsensitiveContains("limit") == true
                || limitPayload.blockedReason?.localizedCaseInsensitiveContains("exhaust") == true)
            && limitRecovery.suggestedAction == nil
            && limitHasNoRetry
            && limitEvidence.reportPath != nil

        let policyPass =
            policyRecovery.suggestedAction == nil
            && policyHasNoRetry

        let repairPass = repairActiveCount == 1
            && repairRequestedApprovals == 1
            && repairExpiredApprovals == 1
        let legacyPass = legacyRequiresDecision
            && legacyRun.runtimeTrustLevel == RuntimeBindingTrustLevel.unverifiable.rawValue
            && legacyReason.localizedCaseInsensitiveContains("legacy")
        steps.append("[5/5] Report/recovery surfaces label unverifiable binding truth honestly and keep clone-only manual recovery")

        let passed = limitPass && policyPass && repairPass && legacyPass
        return Proposal016ExecutionTruthProofResult(
            passed: passed,
            proofStatus: passed
                ? "PASS — Proposal 016 app-level proof verified"
                : "FAIL — Proposal 016 app-level proof did not verify",
            steps: steps,
            limitReason: limitReason,
            runtimeTrust: runtimeTrust,
            bindingSummary: bindingSummary,
            limitRecoverySummary: limitRecoverySummary,
            policySummary: policySummary,
            repairSummary: "\(repairSummary) \(legacySummary)",
            reportPath: limitEvidence.reportPath
        )
    }

    private func persist(result: Proposal016ExecutionTruthProofResult, to url: URL) throws {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        let data = try encoder.encode(result)
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try data.write(to: url)
    }

    private func loadProposalLoopPlan() throws -> RunPlan {
        let workflowURL = repositoryRoot.appendingPathComponent("examples/workflows/proposal-loop-live.yaml")
        let catalogURL = repositoryRoot.appendingPathComponent("examples/agents/agents.yaml")
        let workflow = try YAMLParser.loadWorkflow(from: workflowURL)
        let catalog = try YAMLParser.loadAgentCatalog(from: catalogURL)
        return try RunPlanCompiler(modelContext: modelContext).previewCompile(workflow: workflow, catalog: catalog)
    }

    private func makeProofRun(title: String, plan: RunPlan) throws -> Run {
        let baseURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("Proposal016Proof-\(UUID().uuidString)", isDirectory: true)
        let workspaceRoot = baseURL.appendingPathComponent("workspace", isDirectory: true)
        let artifactRoot = baseURL.appendingPathComponent("artifacts", isDirectory: true)
        try FileManager.default.createDirectory(at: workspaceRoot, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)

        let idea = Idea(title: title, body: "Proposal 016 proof run", status: .active)
        modelContext.insert(idea)

        let run = Run(
            startedAt: Date(),
            status: .running,
            workflowID: plan.workflowID,
            workflowTitle: plan.workflowTitle,
            workflowSnapshotHash: plan.workflowSnapshotHash,
            catalogSnapshotHash: plan.catalogSnapshotHash,
            workflowSourcePath: repositoryRoot.appendingPathComponent("examples/workflows/proposal-loop-live.yaml").path,
            catalogSourcePath: repositoryRoot.appendingPathComponent("examples/agents/agents.yaml").path,
            workflowSnapshotJSON: plan.workflowSnapshotJSON,
            catalogSnapshotJSON: plan.catalogSnapshotJSON,
            workspaceRoot: workspaceRoot.path,
            artifactRoot: artifactRoot.path,
            planCompilerVersion: plan.planCompilerVersion
        )
        run.idea = idea
        idea.runs.append(run)
        run.frozenWorkspaceRootPath = workspaceRoot.path
        modelContext.insert(run)
        return run
    }

    private func seedRepairProof(into run: Run) {
        let staleStage = StageExecution(
            stageID: "state_2_proposal_drafted",
            label: "Proposal drafted",
            startedAt: Date(timeIntervalSinceNow: -180),
            status: .running,
            iteration: 1,
            attemptNumber: 1
        )
        staleStage.lineageID = "state_2_proposal_drafted::iteration:1"
        staleStage.activeOwnerToken = "stale-owner"
        staleStage.run = run
        run.stageExecutions.append(staleStage)
        modelContext.insert(staleStage)

        let activeStage = StageExecution(
            stageID: "state_2_proposal_drafted",
            label: "Proposal drafted",
            startedAt: Date(timeIntervalSinceNow: -60),
            status: .running,
            iteration: 1,
            attemptNumber: 2
        )
        activeStage.lineageID = "state_2_proposal_drafted::iteration:1"
        activeStage.run = run
        run.stageExecutions.append(activeStage)
        modelContext.insert(activeStage)

        let approvalStage = StageExecution(
            stageID: "state_4_proposal_approval",
            label: "Human approval: proposal quality",
            startedAt: Date(timeIntervalSinceNow: -40),
            status: .waitingApproval,
            iteration: 1,
            attemptNumber: 1
        )
        approvalStage.lineageID = "state_4_proposal_approval::iteration:1"
        approvalStage.run = run
        run.stageExecutions.append(approvalStage)
        modelContext.insert(approvalStage)

        let staleApproval = Approval(
            stageID: approvalStage.stageID,
            requestedAt: Date(timeIntervalSinceNow: -30),
            decision: .requested
        )
        staleApproval.run = run
        run.approvals.append(staleApproval)
        modelContext.insert(staleApproval)

        let activeApproval = Approval(
            stageID: approvalStage.stageID,
            requestedAt: Date(timeIntervalSinceNow: -10),
            decision: .requested
        )
        activeApproval.run = run
        run.approvals.append(activeApproval)
        modelContext.insert(activeApproval)
    }

    private func seedLimitExhaustionProof(into run: Run) throws -> (
        agent: AgentExecution,
        frozen: ResolvedProviderBinding,
        runtimeProvider: String,
        runtimeModel: String,
        outputArtifactExists: Bool,
        reportPath: String?
    ) {
        run.status = .blocked
        let stage = StageExecution(
            stageID: "state_5_proposal_refined",
            label: "Proposal refined",
            startedAt: Date(timeIntervalSinceNow: -40),
            status: .failed,
            iteration: 1,
            attemptNumber: 1
        )
        stage.lineageID = "state_5_proposal_refined::iteration:1"
        stage.run = run
        run.stageExecutions.append(stage)
        modelContext.insert(stage)

        let agent = AgentExecution(
            agentID: "proposal_writer",
            agentTitle: "Proposal Writer",
            taskName: "refine_proposal",
            startedAt: Date(timeIntervalSinceNow: -35),
            status: .failed,
            provider: "claude_code",
            effort: "high"
        )
        agent.stageExecution = stage
        stage.agentExecutions.append(agent)
        modelContext.insert(agent)

        let outputURL = URL(fileURLWithPath: run.artifactRoot, isDirectory: true)
            .appendingPathComponent("state_5_proposal_refined.1/proposal_writer/1", isDirectory: true)
        try FileManager.default.createDirectory(at: outputURL, withIntermediateDirectories: true)
        let artifactURL = outputURL.appendingPathComponent("proposal_current.md")
        try "# Partial Proposal\n\nUseful output survived the interruption.\n"
            .write(to: artifactURL, atomically: true, encoding: .utf8)

        let artifact = Artifact(
            name: "proposal_current.md",
            contractID: "proposal_current",
            format: .markdown,
            filePath: artifactURL.path,
            runID: run.id,
            stageID: stage.stageID,
            agentID: agent.agentID,
            provider: agent.provider
        )
        artifact.agentExecution = agent
        agent.artifacts.append(artifact)
        modelContext.insert(artifact)

        let frozenProviderID = UUID()
        let frozenBinding = ResolvedProviderBinding(
            agentID: agent.agentID,
            backendProfileID: "proposal_writer_profile",
            configuredProviderID: frozenProviderID,
            providerFamily: "claude_code",
            providerIdentifier: "claude_code",
            model: "claude-3-5-sonnet",
            effort: "high",
            transport: "goose",
            adapterVersion: "proof"
        )
        run.providerBindingSnapshotJSON = try JSONEncoder().encode([agent.agentID: frozenBinding])
        run.bindingProvenanceJSON = try JSONEncoder().encode([
            agent.agentID: FrozenBindingProvenance(
                source: .backendProfileDefault,
                backendProfileID: "proposal_writer_profile",
                backendProfileModel: "claude-3-5-sonnet",
                configuredProviderID: frozenProviderID,
                configuredProviderDefaultModel: "claude-3-5-sonnet",
                runOverrideModel: nil,
                resolvedModel: "claude-3-5-sonnet",
                resolvedProviderFamily: "claude_code"
            )
        ])

        agent.completedAt = Date()
        agent.logSnippet = "Provider or app limit exhausted after output was produced"
        ExecutionTruthSupport.persistTerminalTruth(
            for: agent,
            canonicalOutcome: .limitExhaustedAfterOutput,
            transportErrorKind: .provider,
            providerStopReason: "max_tokens",
            outputPresence: .durableOutput,
            runtimeProvider: "claude_code",
            runtimeModel: "claude-3-7-sonnet",
            rawErrorMessage: "Maximum token budget exhausted",
            rawFinishEvent: "stop"
        )

        let snapshot = StageRetryCoordinator(modelContext: modelContext).narrowestRecoveryAction(
            for: run,
            failedStage: stage,
            failedAgent: agent,
            validationFailure: nil
        )
        stage.recoverySnapshotJSON = try JSONEncoder().encode(snapshot)
        stage.evidencePacketJSON = try JSONEncoder().encode(
            FailedStageEvidenceBuilder.buildEvidencePacket(
                stageExecution: stage,
                failedAgent: agent,
                validationFailure: nil,
                outputEnvelopes: [],
                recoverySnapshot: snapshot
            )
        )

        try modelContext.save()
        let report = try RunReportBuilder(modelContext: modelContext).emitReport(for: run)
        return (
            agent: agent,
            frozen: frozenBinding,
            runtimeProvider: "claude_code",
            runtimeModel: "claude-3-7-sonnet",
            outputArtifactExists: FileManager.default.fileExists(atPath: artifactURL.path),
            reportPath: report.jsonArtifact.filePath
        )
    }

    private func seedPolicyStopProof(into run: Run) throws -> RecoveryContext {
        run.status = .blocked
        let stage = StageExecution(
            stageID: "state_4_proposal_reviewed",
            label: "Proposal reviewed",
            startedAt: Date(timeIntervalSinceNow: -20),
            status: .failed,
            iteration: 1,
            attemptNumber: 1
        )
        stage.lineageID = "state_4_proposal_reviewed::iteration:1"
        stage.run = run
        run.stageExecutions.append(stage)
        modelContext.insert(stage)

        let agent = AgentExecution(
            agentID: "proposal_reviewer_ui",
            agentTitle: "Proposal Reviewer / UI",
            taskName: "review_proposal_from_ui_perspective",
            startedAt: Date(timeIntervalSinceNow: -18),
            status: .failed,
            provider: "gemini",
            effort: "medium"
        )
        agent.stageExecution = stage
        stage.agentExecutions.append(agent)
        modelContext.insert(agent)

        agent.completedAt = Date()
        agent.logSnippet = "Provider policy-bound terminal stop detected"
        ExecutionTruthSupport.persistTerminalTruth(
            for: agent,
            canonicalOutcome: .failedBeforeOutput,
            transportErrorKind: .provider,
            providerStopReason: "policy_violation",
            outputPresence: .none,
            runtimeProvider: "gemini",
            runtimeModel: "gemini-2.5-pro",
            rawErrorMessage: "Provider policy stop",
            rawFinishEvent: "stop"
        )

        let snapshot = StageRetryCoordinator(modelContext: modelContext).narrowestRecoveryAction(
            for: run,
            failedStage: stage,
            failedAgent: agent,
            validationFailure: nil
        )
        stage.recoverySnapshotJSON = try JSONEncoder().encode(snapshot)
        stage.evidencePacketJSON = try JSONEncoder().encode(
            FailedStageEvidenceBuilder.buildEvidencePacket(
                stageExecution: stage,
                failedAgent: agent,
                validationFailure: nil,
                outputEnvelopes: [],
                recoverySnapshot: snapshot
            )
        )

        try modelContext.save()
        return RecoveryCoordinator(modelContext: modelContext).recoveryContext(for: run)
    }

    private func seedLegacyUnverifiableProof(into run: Run) {
        run.status = .running
        run.runtimeTrustLevel = RuntimeBindingTrustLevel.serverVerified.rawValue

        let stage = StageExecution(
            stageID: "state_2_proposal_drafted",
            label: "Proposal drafted",
            startedAt: Date(timeIntervalSinceNow: -30),
            status: .running,
            iteration: 1,
            attemptNumber: 1
        )
        stage.run = run
        run.stageExecutions.append(stage)
        modelContext.insert(stage)

        let agent = AgentExecution(
            agentID: "proposal_writer",
            agentTitle: "Proposal Writer",
            taskName: "draft_initial_proposal",
            startedAt: Date(timeIntervalSinceNow: -29),
            status: .failed,
            provider: "claude_code",
            effort: "high"
        )
        agent.completedAt = Date(timeIntervalSinceNow: -28)
        agent.stageExecution = stage
        stage.agentExecutions.append(agent)
        modelContext.insert(agent)
    }
}
