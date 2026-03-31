import Foundation
import SwiftData

struct Proposal013AppProofResult: Sendable {
    let runID: UUID
    let terminalStatus: String
    let fanoutArtifactCount: Int
    let aggregateFailureSummary: String
    let narrowestActionSummary: String
    let evidencePreserved: Bool
    let proofStatus: String
}

enum Proposal013AppProofHarnessError: LocalizedError {
    case missingWorkflow
    case missingCatalog
    case runDisappeared
    case terminalFailure(String)
    case timedOut
    case missingReviewStage
    case missingAggregateAgent

    var errorDescription: String? {
        switch self {
        case .missingWorkflow:
            return "Could not locate proposal-loop-live workflow for Proposal 013 proof."
        case .missingCatalog:
            return "Could not locate agents catalog for Proposal 013 proof."
        case .runDisappeared:
            return "Proposal 013 proof run disappeared before the app reached a terminal checkpoint."
        case .terminalFailure(let message):
            return message
        case .timedOut:
            return "Proposal 013 proof timed out before reaching blocked state."
        case .missingReviewStage:
            return "Proposal 013 proof did not produce the expected Proposal reviewed stage."
        case .missingAggregateAgent:
            return "Proposal 013 proof did not produce the aggregate agent execution."
        }
    }
}

@MainActor
final class Proposal013AppProofHarness {
    private let modelContext: ModelContext
    private let executionService: ExecutionService

    init(modelContext: ModelContext, executionService: ExecutionService) {
        self.modelContext = modelContext
        self.executionService = executionService
    }

    func run() async throws -> (Run, FailedStageEvidencePacket, Proposal013AppProofResult) {
        let compiler = RunPlanCompiler(modelContext: modelContext)
        let workflowURL = try resolveWorkflowURL()
        let catalogURL = try resolveCatalogURL()
        let workflow = try YAMLParser.loadWorkflow(from: workflowURL)
        let catalog = try YAMLParser.loadAgentCatalog(from: catalogURL)
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let idea = Idea(
            title: "Proposal 013 App Proof",
            body: "Fixture-backed aggregate contract mismatch proof for Proposal 013.",
            workspaceRootPath: AppConfiguration.defaultRepositoryRoot().path
        )
        modelContext.insert(idea)

        let (run, workspace) = try compiler.createRun(
            for: idea,
            plan: plan,
            workflowSourcePath: workflowURL.path,
            catalogSourcePath: catalogURL.path
        )

        executionService.startRun(run: run, plan: plan, workspace: workspace)
        let blockedRun = try await waitForTerminalBlocked(runID: run.id)

        guard let reviewStage = blockedRun.stageExecutions.last(where: { $0.stageID == "state_3_proposal_reviewed" }) else {
            throw Proposal013AppProofHarnessError.missingReviewStage
        }
        guard let aggregateAgent = reviewStage.agentExecutions.first(where: { $0.agentID == "lead_orchestrator" }) else {
            throw Proposal013AppProofHarnessError.missingAggregateAgent
        }
        guard let packetData = reviewStage.evidencePacketJSON,
              let packet = try? JSONDecoder().decode(FailedStageEvidencePacket.self, from: packetData) else {
            throw Proposal013AppProofHarnessError.terminalFailure("Proposal 013 proof did not persist an evidence packet.")
        }

        let fanoutArtifacts = Set(reviewStage.agentExecutions.flatMap(\.artifacts).map(\.name))
        let fanoutCount = [
            "proposal_review_architect",
            "proposal_review_po",
            "proposal_review_ui",
            "proposal_review_ux",
        ].filter(fanoutArtifacts.contains).count

        let recoveryCoordinator = RecoveryCoordinator(modelContext: modelContext)
        let actions = recoveryCoordinator.availableActions(for: blockedRun)
        let narrowestAction = actions.first?.label ?? "None"
        let hasAggregateRetry = actions.contains { action in
            if case .retryAggregateStep = action {
                return true
            }
            return false
        }
        let evidencePreserved = packet.rawOutputsExist || packet.receiptExists || packet.transcriptExists

        let proofPassed = Self.isCanonicalPass(
            terminalStatus: blockedRun.status,
            fanoutArtifactCount: fanoutCount,
            evidencePacket: packet,
            narrowestAction: narrowestAction,
            hasAggregateRetry: hasAggregateRetry
        )

        let result = Proposal013AppProofResult(
            runID: blockedRun.id,
            terminalStatus: blockedRun.status.rawValue,
            fanoutArtifactCount: fanoutCount,
            aggregateFailureSummary: packet.failureSummary,
            narrowestActionSummary: narrowestAction,
            evidencePreserved: evidencePreserved,
            proofStatus: proofPassed
                ? "PASS — Proposal 013 app proof verified"
                : "FAIL — Proposal 013 app proof did not meet expected blocked/evidence/recovery conditions"
        )
        return (blockedRun, packet, result)
    }

    private func waitForTerminalBlocked(runID: UUID) async throws -> Run {
        let deadline = Date().addingTimeInterval(20)
        while Date() < deadline {
            let descriptor = FetchDescriptor<Run>()
            guard let run = try modelContext.fetch(descriptor).first(where: { $0.id == runID }) else {
                throw Proposal013AppProofHarnessError.runDisappeared
            }

            switch run.status {
            case .blocked:
                return run
            case .failed, .cancelled, .completed:
                throw Proposal013AppProofHarnessError.terminalFailure(
                    "Proposal 013 proof run ended as \(run.status.rawValue) instead of blocked aggregate contract failure."
                )
            default:
                break
            }

            try await Task.sleep(for: .milliseconds(100))
        }
        throw Proposal013AppProofHarnessError.timedOut
    }

    private func resolveWorkflowURL() throws -> URL {
        if let bundled = Bundle.main.url(forResource: "proposal-loop-live", withExtension: "yaml") {
            return bundled
        }
        let fallback = AppConfiguration.defaultRepositoryRoot()
            .appendingPathComponent("examples/workflows/proposal-loop-live.yaml")
        guard FileManager.default.fileExists(atPath: fallback.path) else {
            throw Proposal013AppProofHarnessError.missingWorkflow
        }
        return fallback
    }

    private func resolveCatalogURL() throws -> URL {
        if let bundled = Bundle.main.url(forResource: "agents", withExtension: "yaml") {
            return bundled
        }
        let fallback = AppConfiguration.defaultRepositoryRoot()
            .appendingPathComponent("examples/agents/agents.yaml")
        guard FileManager.default.fileExists(atPath: fallback.path) else {
            throw Proposal013AppProofHarnessError.missingCatalog
        }
        return fallback
    }

    nonisolated static func isCanonicalPass(
        terminalStatus: RunStatus,
        fanoutArtifactCount: Int,
        evidencePacket: FailedStageEvidencePacket,
        narrowestAction: String,
        hasAggregateRetry: Bool
    ) -> Bool {
        let hasCanonicalAggregateEvidence =
            !evidencePacket.failureSummary.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty &&
            (evidencePacket.rawOutputsExist || evidencePacket.receiptExists || evidencePacket.transcriptExists)

        return terminalStatus == .blocked &&
            fanoutArtifactCount == 4 &&
            hasCanonicalAggregateEvidence &&
            narrowestAction == "Retry Aggregate Step" &&
            hasAggregateRetry
    }
}
