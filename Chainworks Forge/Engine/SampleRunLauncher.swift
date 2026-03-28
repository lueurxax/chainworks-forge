import Foundation
import SwiftData

@MainActor
struct SampleRunLauncher {
    let modelContext: ModelContext
    let executionService: ExecutionService
    let appConfigurationStore: AppConfigurationStore
    let providerRegistry: ProviderRegistry

    func launchSampleRun(autostart: Bool = true) async throws -> Run {
        let workflowURL = try resolveWorkflowURL()
        let catalogURL = try resolveCatalogURL()

        let compiler = RunPlanCompiler(modelContext: modelContext)
        let workflow = try YAMLParser.loadWorkflow(from: workflowURL)
        let catalog = try YAMLParser.loadAgentCatalog(from: catalogURL)
        let compiledPlan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let preflight = PreflightService(
            appConfigurationStore: appConfigurationStore,
            providerRegistry: providerRegistry
        )
        let report = await preflight.runReport(
            workflowURL: workflowURL,
            catalogURL: catalogURL,
            plan: compiledPlan
        )
        guard report.status != .fail else {
            throw SampleRunLauncherError.preflightFailed(report.blockingIssues)
        }

        let resolver = BackendProfileResolverV2(providerRegistry: providerRegistry)
        let providerBindings = try resolver.resolveBindings(plan: compiledPlan, startOptions: .empty)
        let adjustedPlan = RunStartOverrideResolver.applying(bindings: providerBindings, to: compiledPlan)
        let provenances = resolver.resolveProvenances(plan: adjustedPlan, startOptions: .empty)

        let idea = Idea(
            title: sampleIdeaTitle(),
            body: sampleIdeaBody(for: adjustedPlan),
            attachmentPath: nil
        )
        modelContext.insert(idea)

        let (run, workspace) = try compiler.createRun(
            for: idea,
            plan: adjustedPlan,
            workflowSourcePath: workflowURL.path,
            catalogSourcePath: catalogURL.path,
            startSnapshot: RunStartSnapshot(
                providerBindingSnapshotJSON: encodeProviderBindings(providerBindings),
                bindingProvenanceJSON: encodeProvenances(provenances),
                startOptionsJSON: encodeStartOptions(.empty),
                frozenWorkspaceRootPath: idea.workspaceRootPath,
                deliveryConfiguration: nil,
                deliveryPreflightJSON: nil
            )
        )
        idea.status = .active
        try modelContext.save()

        if autostart {
            executionService.startRun(run: run, plan: adjustedPlan, workspace: workspace)
        }
        return run
    }

    private func resolveWorkflowURL() throws -> URL {
        let liveWorkflowURL = candidateURL(
            configuredPath: nil,
            repoRelativePath: "examples/workflows/proposal-loop-live.yaml",
            bundleResourceName: "proposal-loop-live"
        )

        if executionService.supportsLiveExecution, let liveWorkflowURL {
            return liveWorkflowURL
        }

        if let configuredWorkflowURL = candidateURL(
            configuredPath: appConfigurationStore.configuration.workflowSourcePath,
            repoRelativePath: "examples/workflows/workflow.yaml",
            bundleResourceName: "workflow"
        ) {
            return configuredWorkflowURL
        }

        throw SampleRunLauncherError.missingWorkflow
    }

    private func resolveCatalogURL() throws -> URL {
        if let catalogURL = candidateURL(
            configuredPath: appConfigurationStore.configuration.agentCatalogSourcePath,
            repoRelativePath: "examples/agents/agents.yaml",
            bundleResourceName: "agents"
        ) {
            return catalogURL
        }
        throw SampleRunLauncherError.missingCatalog
    }

    private func candidateURL(
        configuredPath: String?,
        repoRelativePath: String,
        bundleResourceName: String
    ) -> URL? {
        let candidates: [URL?] = [
            configuredPath.map { URL(fileURLWithPath: $0) },
            URL(fileURLWithPath: FileManager.default.currentDirectoryPath).appendingPathComponent(repoRelativePath),
            AppConfiguration.defaultRepositoryRoot().appendingPathComponent(repoRelativePath),
            Bundle.main.url(forResource: bundleResourceName, withExtension: "yaml")
        ]

        for case let url? in candidates where FileManager.default.isReadableFile(atPath: url.path) {
            return url
        }
        return nil
    }

    private func encodeProviderBindings(_ bindings: [String: ResolvedProviderBinding]) -> Data? {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        return try? encoder.encode(bindings)
    }

    private func encodeStartOptions(_ options: RunStartOptions) -> Data? {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        return try? encoder.encode(options)
    }

    private func encodeProvenances(_ provenances: [String: FrozenBindingProvenance]) -> Data? {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        return try? encoder.encode(provenances)
    }

    private func sampleIdeaTitle() -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd HH:mm"
        return "Sample Provider Run \(formatter.string(from: Date()))"
    }

    private func sampleIdeaBody(for plan: RunPlan) -> String {
        let providerCount = Set(plan.agentBindings.values.map(\.provider)).count
        return """
        Sample Proposal 006 validation run.
        Goal: verify provider settings, diagnostics, immutable bindings, and operator receipts.
        Providers in play: \(providerCount)
        """
    }
}

enum SampleRunLauncherError: Error, LocalizedError {
    case missingWorkflow
    case missingCatalog
    case preflightFailed([String])

    var errorDescription: String? {
        switch self {
        case .missingWorkflow:
            return "Sample workflow could not be located from the current configuration."
        case .missingCatalog:
            return "Agent catalog could not be located from the current configuration."
        case .preflightFailed(let issues):
            return issues.isEmpty
                ? "Sample run is blocked by preflight."
                : "Sample run is blocked by preflight: \(issues.joined(separator: "; "))"
        }
    }
}
