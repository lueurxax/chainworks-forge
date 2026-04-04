import Foundation
import SwiftData

enum Proposal007DogfoodMode: String, Codable, Sendable {
    case happyPath = "happy_path"
    case nonHappyPath = "non_happy_path"
}

struct Proposal007DogfoodResult: Codable, Sendable {
    let mode: Proposal007DogfoodMode
    let runID: UUID
    let terminalStatus: String
    let approvalCount: Int
    let workspaceRoot: String
    let artifactRoot: String
    let exportPath: String
    let exportedItemCount: Int
}

enum Proposal007DogfoodHarnessError: LocalizedError {
    case missingWorkspaceRoot
    case missingWorkflowResource
    case missingCatalogResource
    case noConfiguredProvider(String)
    case deliveryPreflightFailed(String)
    case terminalFailure(String)

    var errorDescription: String? {
        switch self {
        case .missingWorkspaceRoot:
            return "Proposal 007 dogfood harness requires a repo workspace root path."
        case .missingWorkflowResource:
            return "Could not load bundled full-mvp-live workflow."
        case .missingCatalogResource:
            return "Could not load bundled agents catalog."
        case .noConfiguredProvider(let family):
            return "No configured provider is available for \(family)."
        case .deliveryPreflightFailed(let summary):
            return "Delivery preflight failed: \(summary)"
        case .terminalFailure(let message):
            return message
        }
    }
}

@MainActor
final class Proposal007DogfoodHarness {
    static let isEnabled = ProcessInfo.processInfo.environment["CHAINWORKS_P007_DOGFOOD_AUTORUN"] == "1"

    private let modelContext: ModelContext
    private let executionService: ExecutionService
    private let appConfiguration: AppConfiguration
    private let providerRegistry: ProviderRegistry

    init(
        modelContext: ModelContext,
        executionService: ExecutionService,
        appConfiguration: AppConfiguration,
        providerRegistry: ProviderRegistry
    ) {
        self.modelContext = modelContext
        self.executionService = executionService
        self.appConfiguration = appConfiguration
        self.providerRegistry = providerRegistry
    }

    func runFromEnvironment() async throws -> Proposal007DogfoodResult {
        let environment = ProcessInfo.processInfo.environment
        let mode = Proposal007DogfoodMode(rawValue: environment["CHAINWORKS_DELIVERY_PROOF_MODE"] ?? "happy_path") ?? .happyPath
        let idea = try upsertIdeaFromEnvironment(mode: mode)

        guard let workflowURL = bundledWorkflowURL() else {
            throw Proposal007DogfoodHarnessError.missingWorkflowResource
        }
        guard let catalogURL = bundledCatalogURL() else {
            throw Proposal007DogfoodHarnessError.missingCatalogResource
        }

        let workflow = try YAMLParser.loadWorkflow(from: workflowURL)
        let catalog = try YAMLParser.loadAgentCatalog(from: catalogURL)
        let compiler = RunPlanCompiler(modelContext: modelContext)
        let compiledPlan = try compiler.previewCompile(
            workflow: workflow,
            catalog: catalog,
            catalogSourcePath: catalogURL.path
        )

        let startOptions = RunStartOptions.empty
        let resolver = BackendProfileResolverV2(providerRegistry: providerRegistry)
        let bindings = try resolver.resolveBindings(plan: compiledPlan, startOptions: startOptions)
        try ensureNoCrossFamilyMismatches(bindings: bindings)
        let adjustedPlan = RunStartOverrideResolver.applying(bindings: bindings, to: compiledPlan)
        let provenances = resolver.resolveProvenances(plan: adjustedPlan, startOptions: startOptions)
        let strategySelection = StrategyExperimentCoordinator(config: executionService.stewardConfig)
            .resolveSelection(
                selectedProfileID: nil,
                cohortID: nil
            )

        let deliveryConfig = makeDeliveryConfiguration(for: idea, mode: mode)
        let preflight = await DeliveryPreflightService().validate(deliveryConfig)
        guard preflight.passed else {
            let summary = preflight.failedChecks.map(\.label).joined(separator: "; ")
            throw Proposal007DogfoodHarnessError.deliveryPreflightFailed(summary)
        }

        let startSnapshot = RunStartSnapshot(
            providerBindingSnapshotJSON: encode(bindings),
            bindingProvenanceJSON: encode(provenances),
            startOptionsJSON: encode(startOptions),
            frozenWorkspaceRootPath: idea.workspaceRootPath,
            deliveryConfiguration: deliveryConfig,
            deliveryPreflightJSON: try? JSONEncoder().encode(preflight),
            contextStrategyProfileID: strategySelection.profileID,
            strategyAssignmentMode: strategySelection.assignmentMode,
            strategyRecommendationState: strategySelection.recommendationState,
            contextStrategySnapshotJSON: try? JSONEncoder().encode(strategySelection.profile)
        )

        let (run, workspace) = try compiler.createRun(
            for: idea,
            plan: adjustedPlan,
            workflowSourcePath: workflowURL.path,
            catalogSourcePath: catalogURL.path,
            startSnapshot: startSnapshot
        )

        executionService.startRun(run: run, plan: adjustedPlan, workspace: workspace)
        let terminalStatus = try await driveRunToTerminal(runID: run.id)

        let exportDirectory = exportDirectoryFromEnvironment()
        let pack = try EvidencePackBuilder.export(
            run: run,
            workspace: workspace,
            exportDirectory: exportDirectory
        )

        let result = Proposal007DogfoodResult(
            mode: mode,
            runID: run.id,
            terminalStatus: terminalStatus.rawValue,
            approvalCount: run.approvals.filter { $0.decision == .granted }.count,
            workspaceRoot: workspace.workspaceRoot.path,
            artifactRoot: workspace.artifactRoot.path,
            exportPath: pack.exportPath.path,
            exportedItemCount: pack.itemCount
        )
        try persistResult(result)
        return result
    }

    private func upsertIdeaFromEnvironment(mode: Proposal007DogfoodMode) throws -> Idea {
        let environment = ProcessInfo.processInfo.environment
        let rawTitle = environment["CHAINWORKS_UI_TEST_SEED_IDEA_TITLE"]?
            .trimmingCharacters(in: .whitespacesAndNewlines)
        let title = (rawTitle?.isEmpty == false ? rawTitle! : nil)
            ?? "Proposal 007 Dogfood \(mode.rawValue)"
        let body = environment["CHAINWORKS_UI_TEST_SEED_IDEA_BODY"] ?? "App-launched Proposal 007 repo-backed proof."
        guard let workspaceRoot = environment["CHAINWORKS_UI_TEST_SEED_IDEA_WORKSPACE_ROOT"]?.trimmingCharacters(in: .whitespacesAndNewlines),
              !workspaceRoot.isEmpty else {
            throw Proposal007DogfoodHarnessError.missingWorkspaceRoot
        }

        let descriptor = FetchDescriptor<Idea>()
        let ideas = (try? modelContext.fetch(descriptor)) ?? []
        if let existing = ideas.first(where: { $0.title == title }) {
            existing.body = body
            existing.workspaceRootPath = workspaceRoot
            try modelContext.save()
            return existing
        }

        let idea = Idea(
            title: title,
            body: body,
            attachmentPath: nil,
            workspaceRootPath: workspaceRoot
        )
        modelContext.insert(idea)
        try modelContext.save()
        return idea
    }

    private func bundledWorkflowURL() -> URL? {
        AppConfiguration.preferredExampleURL(
            repoRelativePath: "examples/workflows/full-mvp-live.yaml",
            bundledURL: Bundle.main.url(forResource: "full-mvp-live", withExtension: "yaml"),
            allowsDocumentsFallback: false,
            sourceFilePath: #filePath
        )
    }

    private func bundledCatalogURL() -> URL? {
        AppConfiguration.preferredExampleURL(
            repoRelativePath: "examples/agents/agents.yaml",
            bundledURL: Bundle.main.url(forResource: "agents", withExtension: "yaml"),
            allowsDocumentsFallback: false,
            sourceFilePath: #filePath
        )
    }

    private func makeDeliveryConfiguration(
        for idea: Idea,
        mode: Proposal007DogfoodMode
    ) -> DeliveryConfiguration {
        let repoRoot = idea.workspaceRootPath ?? AppConfiguration.defaultRepositoryRoot(
            currentDirectoryPath: FileManager.default.currentDirectoryPath,
            bundleURL: Bundle.main.bundleURL,
            allowsDocumentsFallback: false,
            sourceFilePath: #filePath
        ).path
        let repoIdentifier = RepositoryIdentityNormalizer.canonicalIdentifier(
            configuredIdentifier: nil,
            repoRoot: repoRoot
        )
        let branchSuffix = UUID().uuidString.prefix(8)
        let worktreeBasePath = ProcessInfo.processInfo.environment["CHAINWORKS_DOGFOOD_WORKTREE_BASE_PATH"]?
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .nilIfEmpty
            ?? appConfiguration.worktreeBasePath
            ?? appConfiguration.runStorageBaseURL.appendingPathComponent("worktrees", isDirectory: true).path

        return DeliveryConfiguration(
            profileID: "chainworks_forge_self",
            profileLabel: "Chainworks Forge (Self)",
            sampleProfileID: "chainworks_forge_self",
            repoIdentifier: repoIdentifier,
            repoRoot: repoRoot,
            baseBranch: "main",
            worktreeBasePath: worktreeBasePath,
            targetBranch: "dogfood/\(mode.rawValue)-\(branchSuffix)",
            releaseTargetID: "sandbox_local",
            releaseTargetLabel: "Local Sandbox",
            releaseMode: .sandbox
        )
    }

    private func ensureNoCrossFamilyMismatches(bindings: [String: ResolvedProviderBinding]) throws {
        if let mismatch = bindings.values.first(where: \.hasCrossFamilyMismatch) {
            throw Proposal007DogfoodHarnessError.terminalFailure(
                "Resolved provider/model mismatch for \(mismatch.agentID): \(mismatch.providerIdentifier) / \(mismatch.model)"
            )
        }
    }

    private func driveRunToTerminal(runID: UUID) async throws -> RunStatus {
        while true {
            if let request = executionService.pendingApprovals.values.first(where: { $0.runID == runID }) {
                executionService.resolveApproval(
                    approvalID: request.id,
                    granted: true,
                    comment: "Auto-approved by Proposal 007 dogfood harness"
                )
            }

            let descriptor = FetchDescriptor<Run>()
            let runs = (try? modelContext.fetch(descriptor)) ?? []
            guard let run = runs.first(where: { $0.id == runID }) else {
                throw Proposal007DogfoodHarnessError.terminalFailure("Dogfood run disappeared before completion.")
            }

            switch run.status {
            case .completed, .blocked:
                return run.status
            case .failed, .cancelled:
                throw Proposal007DogfoodHarnessError.terminalFailure("Dogfood run ended as \(run.status.rawValue).")
            default:
                break
            }

            try await Task.sleep(for: .milliseconds(250))
        }
    }

    private func exportDirectoryFromEnvironment() -> URL {
        if let path = ProcessInfo.processInfo.environment["CHAINWORKS_DOGFOOD_EXPORT_BASE_PATH"]?
            .trimmingCharacters(in: .whitespacesAndNewlines),
           !path.isEmpty {
            return URL(fileURLWithPath: path, isDirectory: true)
        }

        return FileManager.default.urls(for: .desktopDirectory, in: .userDomainMask).first
            ?? FileManager.default.temporaryDirectory
    }

    private func persistResult(_ result: Proposal007DogfoodResult) throws {
        guard let path = ProcessInfo.processInfo.environment["CHAINWORKS_DOGFOOD_RESULT_PATH"]?
            .trimmingCharacters(in: .whitespacesAndNewlines),
              !path.isEmpty else {
            return
        }

        let url = URL(fileURLWithPath: path)
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        let data = try encoder.encode(result)
        try data.write(to: url, options: .atomic)
    }

    private func encode<T: Encodable>(_ value: T) -> Data? {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        return try? encoder.encode(value)
    }
}

private extension String {
    var nilIfEmpty: String? {
        isEmpty ? nil : self
    }
}
