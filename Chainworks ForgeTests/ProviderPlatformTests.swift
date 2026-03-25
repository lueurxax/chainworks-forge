import Testing
import SwiftData
import Foundation
@testable import Chainworks_Forge

@MainActor
@Suite("ProviderPlatform", .tags(.fast, .provider))
struct ProviderPlatformTests {
    private static var retainedObjects: [AnyObject] = []
    private static var retainedRegistries: [ProviderRegistry] = []

    private func makeTestSecretStore(_ serviceName: String) -> KeychainSecretStore {
        KeychainSecretStore(serviceName: serviceName, useInMemoryStore: true)
    }

    private mutating func retain<T: AnyObject>(_ object: T) -> T {
        Self.retainedObjects.append(object)
        return object
    }

    private mutating func retain(_ registry: ProviderRegistry) -> ProviderRegistry {
        Self.retainedRegistries.append(registry)
        return registry
    }

    private func makeTempDirectory() throws -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }

    private func unzipArchive(_ archiveURL: URL, to destinationURL: URL) throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/unzip")
        process.arguments = ["-qq", archiveURL.path, "-d", destinationURL.path]
        try process.run()
        process.waitUntilExit()
        #expect(process.terminationStatus == 0, "Expected unzip to succeed for \(archiveURL.path)")
    }

    private func makePlan(provider: String, backendProfileID: String = "reviewer_profile") -> RunPlan {
        let agent = ResolvedAgent(
            id: "proposal_writer",
            title: "Proposal Writer",
            mode: "tool_use",
            backendProfileID: backendProfileID,
            provider: provider,
            model: "default-model",
            effort: "medium",
            maxTurns: 8,
            temperature: 0.0,
            permissionProfile: "read_only",
            skillRef: "skill",
            skillRole: nil,
            prompt: "Write a proposal",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["proposal_current"]
        )

        return RunPlan(
            workflowID: "proposal_test",
            workflowTitle: "Proposal Test",
            states: [:],
            initialStateID: "state_1",
            agentBindings: [agent.id: agent],
            variables: [:],
            scoring: nil,
            failurePolicy: nil,
            workflowSnapshotHash: "workflow-hash",
            catalogSnapshotHash: "catalog-hash",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: RunPlan.currentCompilerVersion
        )
    }

    private func makeMixedProviderPlan() -> RunPlan {
        let writer = ResolvedAgent(
            id: "proposal_writer",
            title: "Proposal Writer",
            mode: "tool_use",
            backendProfileID: "writer_profile",
            provider: "codex",
            model: "gpt-5-codex",
            effort: "medium",
            maxTurns: 8,
            temperature: 0.0,
            permissionProfile: "read_only",
            skillRef: "skill",
            skillRole: nil,
            prompt: "Write a proposal",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["proposal_current"]
        )

        let reviewer = ResolvedAgent(
            id: "proposal_reviewer",
            title: "Proposal Reviewer",
            mode: "tool_use",
            backendProfileID: "reviewer_profile",
            provider: "gemini",
            model: "gemini-2.5-pro",
            effort: "high",
            maxTurns: 8,
            temperature: 0.0,
            permissionProfile: "read_only",
            skillRef: "skill",
            skillRole: nil,
            prompt: "Review the proposal",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: ["proposal_current"],
            outputs: ["proposal_review"]
        )

        return RunPlan(
            workflowID: "mixed_provider_test",
            workflowTitle: "Mixed Provider Test",
            states: [:],
            initialStateID: "state_1",
            agentBindings: [
                writer.id: writer,
                reviewer.id: reviewer
            ],
            variables: [:],
            scoring: nil,
            failurePolicy: nil,
            workflowSnapshotHash: "workflow-hash",
            catalogSnapshotHash: "catalog-hash",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: RunPlan.currentCompilerVersion
        )
    }

    @Test("Bootstrap resolver uses persisted settings unless env override enabled")
    mutating func bootstrapResolverUsesPersistedSettingsUnlessEnvOverrideEnabled() throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let persisted = AppConfiguration(
            runStorageBasePath: tempDirectory.appendingPathComponent("runs").path,
            worktreeBasePath: tempDirectory.appendingPathComponent("worktrees").path,
            workflowSourcePath: tempDirectory.appendingPathComponent("workflow.yaml").path,
            agentCatalogSourcePath: tempDirectory.appendingPathComponent("agents.yaml").path,
            supportBundleExportPath: tempDirectory.appendingPathComponent("exports").path,
            activeConfigurationSource: .persistedSettings
        )
        let store = retain(AppConfigurationStore(
            fileURL: tempDirectory.appendingPathComponent("app-config.json"),
            initialConfiguration: persisted
        ))

        let resolved = BootstrapConfigurationResolver.resolve(store: store, environment: [:])
        #expect(resolved == persisted)

        let overridden = BootstrapConfigurationResolver.resolve(
            store: store,
            environment: [
                "CHAINWORKS_ALLOW_ENV_OVERRIDE": "1",
                "CHAINWORKS_WORKFLOW_SOURCE_PATH": "/tmp/override-workflow.yaml"
            ]
        )
        #expect(overridden.workflowSourcePath == "/tmp/override-workflow.yaml")
        #expect(overridden.activeConfigurationSource == .developmentEnvOverride)
    }

    @Test("Backend profile resolver resolves preferred provider and overrides")
    mutating func backendProfileResolverResolvesPreferredProviderAndOverrides() throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let claude = ConfiguredProvider(
            family: .claude,
            displayName: "Claude CLI",
            transport: .cli,
            authMode: .none,
            defaultModel: "claude-sonnet-4"
        )
        let alternateClaude = ConfiguredProvider(
            family: .claude,
            displayName: "Claude HTTP",
            transport: .httpAPI,
            endpoint: "http://localhost:8080",
            authMode: .none,
            defaultModel: "claude-opus-4"
        )
        let settings = ProviderSettings(
            configuredProviders: [claude, alternateClaude],
            preferredProviderIDsByFamily: [ProviderFamily.claude.rawValue: claude.id],
            notificationOnProviderFailure: true,
            runStartRequiresCleanPreflight: true
        )
        let store = retain(ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: settings
        ))
        let registry = retain(ProviderRegistry(
            settingsStore: store,
            secretStore: makeTestSecretStore("com.chainworks.tests.backend-profile")
        ))
        let resolver = BackendProfileResolverV2(providerRegistry: registry)

        var startOptions = RunStartOptions.empty
        startOptions.overridesByBackendProfileID["reviewer_profile"] = RunStartOverride(
            configuredProviderID: alternateClaude.id,
            model: "claude-custom",
            effort: "high"
        )

        let bindings = try? resolver.resolveBindings(
            plan: makePlan(provider: "claude_code"),
            startOptions: startOptions
        )

        let binding = bindings?["proposal_writer"]
        #expect(binding?.configuredProviderID == alternateClaude.id)
        #expect(binding?.model == "claude-custom")
        #expect(binding?.effort == "high")
        #expect(binding?.providerIdentifier == "claude_code")
    }

    @Test("Backend profile resolver supports mixed providers across agents")
    mutating func backendProfileResolverSupportsMixedProvidersAcrossAgents() throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let codex = ConfiguredProvider(
            family: .codex,
            displayName: "Codex CLI",
            transport: .cli,
            authMode: .none,
            defaultModel: "gpt-5-codex"
        )
        let gemini = ConfiguredProvider(
            family: .gemini,
            displayName: "Gemini API",
            transport: .httpAPI,
            endpoint: "https://generativelanguage.googleapis.com",
            authMode: .none,
            defaultModel: "gemini-2.5-pro"
        )
        let store = retain(ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [codex, gemini],
                preferredProviderIDsByFamily: [
                    ProviderFamily.codex.rawValue: codex.id,
                    ProviderFamily.gemini.rawValue: gemini.id
                ],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: true
            )
        ))
        let registry = retain(ProviderRegistry(
            settingsStore: store,
            secretStore: makeTestSecretStore("com.chainworks.tests.mixed-providers")
        ))
        let resolver = BackendProfileResolverV2(providerRegistry: registry)

        let bindings = try? resolver.resolveBindings(
            plan: makeMixedProviderPlan(),
            startOptions: .empty
        )

        #expect(bindings?["proposal_writer"]?.providerIdentifier == "codex")
        #expect(bindings?["proposal_reviewer"]?.providerIdentifier == "gemini")
    }

    @Test("Settings transfer exports schema version")
    mutating func settingsTransferExportsSchemaVersion() throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let configuration = AppConfiguration(
            runStorageBasePath: tempDirectory.appendingPathComponent("runs").path,
            worktreeBasePath: nil,
            workflowSourcePath: tempDirectory.appendingPathComponent("workflow.yaml").path,
            agentCatalogSourcePath: tempDirectory.appendingPathComponent("agents.yaml").path,
            supportBundleExportPath: tempDirectory.appendingPathComponent("exports").path,
            activeConfigurationSource: .persistedSettings
        )
        let appStore = retain(AppConfigurationStore(
            fileURL: tempDirectory.appendingPathComponent("app-config.json"),
            initialConfiguration: configuration
        ))
        let providerStore = retain(ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [
                    ConfiguredProvider(
                        family: .codex,
                        displayName: "Codex CLI",
                        transport: .cli,
                        authMode: .none,
                        defaultModel: "gpt-5-codex"
                    )
                ],
                preferredProviderIDsByFamily: [:],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: true
            )
        ))

        let service = SettingsTransferService(
            appConfigurationStore: appStore,
            providerSettingsStore: providerStore,
            secretStore: makeTestSecretStore("com.chainworks.tests.settings")
        )

        let exportURL = try service.exportSettings(to: tempDirectory)
        let data = try Data(contentsOf: exportURL)
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let package = try decoder.decode(ExportableSettingsPackage.self, from: data)

        #expect(package.transferSchemaVersion == SettingsTransferService.currentSchemaVersion)
        #expect(package.providerSettings.configuredProviders.count == 1)
    }

    @Test("Settings import fails closed when secrets are missing")
    mutating func settingsImportFailsClosedWhenSecretsAreMissing() throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let initialConfiguration = AppConfiguration(
            runStorageBasePath: "/initial/runs",
            worktreeBasePath: nil,
            workflowSourcePath: "/initial/workflow.yaml",
            agentCatalogSourcePath: "/initial/agents.yaml",
            supportBundleExportPath: nil,
            activeConfigurationSource: .persistedSettings
        )
        let initialProvider = ConfiguredProvider(
            family: .codex,
            displayName: "Initial Codex",
            transport: .cli,
            authMode: .none,
            defaultModel: "gpt-5-codex"
        )

        let appStore = retain(AppConfigurationStore(
            fileURL: tempDirectory.appendingPathComponent("app-config.json"),
            initialConfiguration: initialConfiguration
        ))
        let providerStore = retain(ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [initialProvider],
                preferredProviderIDsByFamily: [ProviderFamily.codex.rawValue: initialProvider.id],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: true
            )
        ))

        let importProvider = ConfiguredProvider(
            family: .gemini,
            displayName: "Imported Gemini",
            transport: .httpAPI,
            endpoint: "https://generativelanguage.googleapis.com",
            authMode: .apiKey,
            defaultModel: "gemini-2.5-pro"
        )
        let package = ExportableSettingsPackage(
            transferSchemaVersion: SettingsTransferService.currentSchemaVersion,
            appConfiguration: AppConfiguration(
                runStorageBasePath: "/imported/runs",
                worktreeBasePath: nil,
                workflowSourcePath: "/imported/workflow.yaml",
                agentCatalogSourcePath: "/imported/agents.yaml",
                supportBundleExportPath: nil,
                activeConfigurationSource: .persistedSettings
            ),
            providerSettings: ProviderSettings(
                configuredProviders: [importProvider],
                preferredProviderIDsByFamily: [ProviderFamily.gemini.rawValue: importProvider.id],
                notificationOnProviderFailure: false,
                runStartRequiresCleanPreflight: false
            ),
            exportedAt: Date(),
            appVersion: "dev",
            secretPlaceholders: [ProviderAdapterSupport.secretKey(for: importProvider)]
        )

        let packageURL = tempDirectory.appendingPathComponent("chainworks-settings.json")
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        try encoder.encode(package).write(to: packageURL, options: .atomic)

        let service = SettingsTransferService(
            appConfigurationStore: appStore,
            providerSettingsStore: providerStore,
            secretStore: makeTestSecretStore("com.chainworks.tests.import-empty")
        )

        #expect(throws: (any Error).self) {
            try service.importSettings(from: packageURL)
        }
        #expect(appStore.configuration == initialConfiguration)
        #expect(providerStore.settings.configuredProviders.map(\.displayName) == ["Initial Codex"])
    }

    @Test("Preflight fails when required provider family is missing")
    mutating func preflightFailsWhenRequiredProviderFamilyIsMissing() async throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let repoRoot = AppConfiguration.defaultRepositoryRoot()
        let configuration = AppConfiguration(
            runStorageBasePath: tempDirectory.appendingPathComponent("runs").path,
            worktreeBasePath: tempDirectory.appendingPathComponent("worktrees").path,
            workflowSourcePath: repoRoot.appendingPathComponent("examples/workflows/workflow.yaml").path,
            agentCatalogSourcePath: repoRoot.appendingPathComponent("examples/agents/agents.yaml").path,
            supportBundleExportPath: tempDirectory.appendingPathComponent("exports").path,
            activeConfigurationSource: .persistedSettings
        )

        let appStore = retain(AppConfigurationStore(
            fileURL: tempDirectory.appendingPathComponent("app-config.json"),
            initialConfiguration: configuration
        ))
        let providerStore = retain(ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: .empty
        ))
        let registry = retain(ProviderRegistry(settingsStore: providerStore))
        let preflight = PreflightService(appConfigurationStore: appStore, providerRegistry: registry)

        let report = await preflight.runReport(
            workflowURL: URL(fileURLWithPath: configuration.workflowSourcePath),
            catalogURL: URL(fileURLWithPath: configuration.agentCatalogSourcePath),
            plan: makePlan(provider: "claude_code")
        )

        #expect(report.status == .fail)
        #expect(report.blockingIssues.contains { $0.contains("Claude") || $0.contains("No provider configured") })
    }

    @Test("Preflight fails when provider credential is missing")
    mutating func preflightFailsWhenProviderCredentialIsMissing() async throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let repoRoot = AppConfiguration.defaultRepositoryRoot()
        let configuration = AppConfiguration(
            runStorageBasePath: tempDirectory.appendingPathComponent("runs").path,
            worktreeBasePath: tempDirectory.appendingPathComponent("worktrees").path,
            workflowSourcePath: repoRoot.appendingPathComponent("examples/workflows/workflow.yaml").path,
            agentCatalogSourcePath: repoRoot.appendingPathComponent("examples/agents/agents.yaml").path,
            supportBundleExportPath: tempDirectory.appendingPathComponent("exports").path,
            activeConfigurationSource: .persistedSettings
        )

        let appStore = retain(AppConfigurationStore(
            fileURL: tempDirectory.appendingPathComponent("app-config.json"),
            initialConfiguration: configuration
        ))
        let provider = ConfiguredProvider(
            family: .gemini,
            displayName: "Gemini API",
            transport: .httpAPI,
            endpoint: "https://generativelanguage.googleapis.com",
            authMode: .apiKey,
            defaultModel: "gemini-2.5-pro"
        )
        let providerStore = retain(ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [provider],
                preferredProviderIDsByFamily: [ProviderFamily.gemini.rawValue: provider.id],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: true
            )
        ))
        let registry = retain(ProviderRegistry(
            settingsStore: providerStore,
            secretStore: makeTestSecretStore("com.chainworks.tests.missing-gemini-secret")
        ))
        let preflight = PreflightService(appConfigurationStore: appStore, providerRegistry: registry)

        let report = await preflight.runReport(
            workflowURL: URL(fileURLWithPath: configuration.workflowSourcePath),
            catalogURL: URL(fileURLWithPath: configuration.agentCatalogSourcePath),
            plan: makePlan(provider: "gemini")
        )

        #expect(report.status == .fail)
        #expect(report.blockingIssues.contains { $0.localizedCaseInsensitiveContains("API key is missing") })
    }

    @Test("Preflight fails when override selects unavailable model")
    mutating func preflightFailsWhenOverrideSelectsUnavailableModel() async throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let repoRoot = AppConfiguration.defaultRepositoryRoot()
        let configuration = AppConfiguration(
            runStorageBasePath: tempDirectory.appendingPathComponent("runs").path,
            worktreeBasePath: tempDirectory.appendingPathComponent("worktrees").path,
            workflowSourcePath: repoRoot.appendingPathComponent("examples/workflows/workflow.yaml").path,
            agentCatalogSourcePath: repoRoot.appendingPathComponent("examples/agents/agents.yaml").path,
            supportBundleExportPath: tempDirectory.appendingPathComponent("exports").path,
            activeConfigurationSource: .persistedSettings
        )

        let appStore = retain(AppConfigurationStore(
            fileURL: tempDirectory.appendingPathComponent("app-config.json"),
            initialConfiguration: configuration
        ))
        let claude = ConfiguredProvider(
            family: .claude,
            displayName: "Claude CLI",
            transport: .cli,
            authMode: .none,
            defaultModel: "claude-sonnet-4"
        )
        let providerStore = retain(ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [claude],
                preferredProviderIDsByFamily: [ProviderFamily.claude.rawValue: claude.id],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: true
            )
        ))
        let registry = retain(ProviderRegistry(
            settingsStore: providerStore,
            secretStore: makeTestSecretStore("com.chainworks.tests.unavailable-model")
        ))
        let preflight = PreflightService(appConfigurationStore: appStore, providerRegistry: registry)

        var startOptions = RunStartOptions.empty
        startOptions.overridesByBackendProfileID["reviewer_profile"] = RunStartOverride(
            configuredProviderID: claude.id,
            model: "claude-imaginary-9",
            effort: "high"
        )

        let report = await preflight.runReport(
            workflowURL: URL(fileURLWithPath: configuration.workflowSourcePath),
            catalogURL: URL(fileURLWithPath: configuration.agentCatalogSourcePath),
            plan: makePlan(provider: "claude_code"),
            startOptions: startOptions
        )

        #expect(report.status == .fail)
        #expect(report.blockingIssues.contains { $0.localizedCaseInsensitiveContains("not available") })
    }

    @Test("Sample run launcher creates frozen provider binding snapshot")
    mutating func sampleRunLauncherCreatesFrozenProviderBindingSnapshot() async throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let (container, context) = try makeTestModelContainer()
        _ = container

        let repoRoot = AppConfiguration.defaultRepositoryRoot()
        let configuration = AppConfiguration(
            runStorageBasePath: tempDirectory.appendingPathComponent("runs").path,
            worktreeBasePath: tempDirectory.appendingPathComponent("worktrees").path,
            workflowSourcePath: repoRoot.appendingPathComponent("examples/workflows/workflow.yaml").path,
            agentCatalogSourcePath: repoRoot.appendingPathComponent("examples/agents/agents.yaml").path,
            supportBundleExportPath: tempDirectory.appendingPathComponent("exports").path,
            activeConfigurationSource: .persistedSettings
        )
        let appStore = retain(AppConfigurationStore(
            fileURL: tempDirectory.appendingPathComponent("app-config.json"),
            initialConfiguration: configuration
        ))
        let providerStore = retain(ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [
                    ConfiguredProvider(
                        family: .codex,
                        displayName: "Codex API",
                        transport: .httpAPI,
                        endpoint: "https://codex.test.local",
                        authMode: .none,
                        defaultModel: "gpt-5-codex"
                    ),
                    ConfiguredProvider(
                        family: .claude,
                        displayName: "Claude API",
                        transport: .httpAPI,
                        endpoint: "https://claude.test.local",
                        authMode: .none,
                        defaultModel: "claude-sonnet-4"
                    ),
                    ConfiguredProvider(
                        family: .gemini,
                        displayName: "Gemini API",
                        transport: .httpAPI,
                        endpoint: "https://gemini.test.local",
                        authMode: .none,
                        defaultModel: "gemini-2.5-pro"
                    )
                ],
                preferredProviderIDsByFamily: [:],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: false
            )
        ))
        let registry = retain(ProviderRegistry(
            settingsStore: providerStore,
            secretStore: makeTestSecretStore("com.chainworks.tests.sample-run")
        ))
        let executor = SimulatedAgentExecutor()
        let executionService = ExecutionService(
            modelContext: context,
            executor: executor
        )
        let launcher = SampleRunLauncher(
            modelContext: context,
            executionService: executionService,
            appConfigurationStore: appStore,
            providerRegistry: registry
        )

        let run = try await launcher.launchSampleRun(autostart: false)

        #expect(run.providerBindingSnapshotJSON != nil)
        #expect(run.startOptionsJSON != nil)

        let decoder = JSONDecoder()
        let bindings = try decoder.decode([String: ResolvedProviderBinding].self, from: try #require(run.providerBindingSnapshotJSON))
        #expect(!bindings.isEmpty)
        #expect(run.workflowID == "proposal_to_release")
    }

    @Test("Support bundle export includes artifact index and selected artifacts")
    mutating func supportBundleExportIncludesArtifactIndexAndSelectedArtifacts() async throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let (container, context) = try makeTestModelContainer()
        _ = container
        let workspace = makeTestWorkspace(tempDir: tempDirectory)
        let run = makeTestRun(workspace: workspace, context: context)
        run.status = .completed
        run.totalCostCents = 321

        let stage = StageExecution(stageID: "proposal", label: "Proposal")
        stage.status = .completed
        stage.run = run
        context.insert(stage)

        let agent = AgentExecution(
            agentID: "proposal_writer",
            agentTitle: "Proposal Writer",
            taskName: "write_proposal",
            provider: "claude_code",
            effort: "high"
        )
        agent.status = .completed
        agent.stageExecution = stage
        agent.resolvedModel = "claude-sonnet-4"
        agent.logSnippet = "provider call completed"
        context.insert(agent)

        let artifactURL = workspace.artifactRoot.appendingPathComponent("proposal.md")
        try Data("# Proposal".utf8).write(to: artifactURL, options: .atomic)
        let artifact = Artifact(
            name: "proposal.md",
            contractID: "proposal_current",
            format: .markdown,
            filePath: artifactURL.path,
            runID: run.id,
            stageID: "proposal",
            agentID: "proposal_writer",
            provider: "claude_code"
        )
        artifact.isPinned = true
        artifact.sizeBytes = 10
        artifact.agentExecution = agent
        context.insert(artifact)
        try context.save()

        let appStore = retain(AppConfigurationStore(
            fileURL: tempDirectory.appendingPathComponent("app-config.json"),
            initialConfiguration: AppConfiguration(
                runStorageBasePath: tempDirectory.appendingPathComponent("runs").path,
                worktreeBasePath: tempDirectory.appendingPathComponent("worktrees").path,
                workflowSourcePath: tempDirectory.appendingPathComponent("workflow.yaml").path,
                agentCatalogSourcePath: tempDirectory.appendingPathComponent("agents.yaml").path,
                supportBundleExportPath: tempDirectory.appendingPathComponent("exports").path,
                activeConfigurationSource: .persistedSettings
            )
        ))
        let providerStore = retain(ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [
                    ConfiguredProvider(
                        family: .claude,
                        displayName: "Claude CLI",
                        transport: .cli,
                        authMode: .none,
                        defaultModel: "claude-sonnet-4"
                    )
                ],
                preferredProviderIDsByFamily: [:],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: true
            )
        ))
        let registry = retain(ProviderRegistry(
            settingsStore: providerStore,
            secretStore: makeTestSecretStore("com.chainworks.tests.bundle")
        ))

        let exporter = SupportBundleExporter(
            modelContext: context,
            appConfigurationStore: appStore,
            providerRegistry: registry
        )
        let archiveURL = try await exporter.exportBundle()
        let unzipURL = tempDirectory.appendingPathComponent("unzipped", isDirectory: true)
        try FileManager.default.createDirectory(at: unzipURL, withIntermediateDirectories: true)
        try unzipArchive(archiveURL, to: unzipURL)

        let contents = try FileManager.default.subpathsOfDirectory(atPath: unzipURL.path)
        #expect(contents.contains { $0.hasSuffix("app-version.json") })
        #expect(contents.contains { $0.hasSuffix("provider-health.json") })
        #expect(contents.contains { $0.hasSuffix("artifact-index.json") })
        #expect(contents.contains { $0.hasSuffix("artifacts/proposal.md") })
    }
}
