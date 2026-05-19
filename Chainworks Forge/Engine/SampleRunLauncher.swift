import Foundation
import SwiftData

/// Creates a sample run from the canonical workflow and agent catalog.
/// Used to verify that provider binding snapshot and start options are correctly captured at run creation time.
@MainActor
final class SampleRunLauncher {
    private let modelContext: ModelContext
    private let executionService: ExecutionService
    private let appConfigurationStore: AppConfigurationStore
    private let providerRegistry: ProviderRegistry

    init(
        modelContext: ModelContext,
        executionService: ExecutionService,
        appConfigurationStore: AppConfigurationStore,
        providerRegistry: ProviderRegistry
    ) {
        self.modelContext = modelContext
        self.executionService = executionService
        self.appConfigurationStore = appConfigurationStore
        self.providerRegistry = providerRegistry
    }

    func launchSampleRun(autostart: Bool) throws -> Run {
        let configuration = appConfigurationStore.configuration
        let workflowURL = URL(fileURLWithPath: configuration.workflowSourcePath)
        let catalogURL = URL(fileURLWithPath: configuration.agentCatalogSourcePath)

        let workflow = try YAMLParser.loadWorkflow(from: workflowURL)
        let catalog = try YAMLParser.loadAgentCatalog(from: catalogURL)

        let compiler = RunPlanCompiler(modelContext: modelContext)
        let plan = try compiler.previewCompile(
            workflow: workflow,
            catalog: catalog,
            catalogSourcePath: catalogURL.path
        )

        let startOptions = RunStartOptions.empty
        let resolver = BackendProfileResolverV2(providerRegistry: providerRegistry)
        let bindings = try resolver.resolveBindings(
            plan: plan,
            startOptions: startOptions,
            runtimeProfiles: catalog.runtimeProfiles
        )
        let adjustedPlan = RunStartOverrideResolver.applying(bindings: bindings, to: plan)

        let encoder = JSONEncoder()
        let startSnapshot = RunStartSnapshot(
            providerBindingSnapshotJSON: try encoder.encode(bindings),
            startOptionsJSON: try encoder.encode(startOptions),
            frozenWorkspaceRootPath: NSHomeDirectory()
        )

        let idea = Idea(title: "Sample Run", body: "Sample run created by SampleRunLauncher")
        modelContext.insert(idea)
        try modelContext.save()

        let (run, workspace) = try compiler.createRun(
            for: idea,
            plan: adjustedPlan,
            workflowSourcePath: workflowURL.path,
            catalogSourcePath: catalogURL.path,
            startSnapshot: startSnapshot
        )

        if autostart {
            executionService.startRun(run: run, plan: adjustedPlan, workspace: workspace)
        }

        return run
    }
}
