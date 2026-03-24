import XCTest
import SwiftData
@testable import Chainworks_Forge

@MainActor
final class ProviderPlatformTests: XCTestCase {

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
        XCTAssertEqual(process.terminationStatus, 0, "Expected unzip to succeed for \(archiveURL.path)")
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

    func testBootstrapResolverUsesPersistedSettingsUnlessEnvOverrideEnabled() throws {
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
        let store = AppConfigurationStore(
            fileURL: tempDirectory.appendingPathComponent("app-config.json"),
            initialConfiguration: persisted
        )

        let resolved = BootstrapConfigurationResolver.resolve(store: store, environment: [:])
        XCTAssertEqual(resolved, persisted)

        let overridden = BootstrapConfigurationResolver.resolve(
            store: store,
            environment: [
                "CHAINWORKS_ALLOW_ENV_OVERRIDE": "1",
                "CHAINWORKS_WORKFLOW_SOURCE_PATH": "/tmp/override-workflow.yaml"
            ]
        )
        XCTAssertEqual(overridden.workflowSourcePath, "/tmp/override-workflow.yaml")
        XCTAssertEqual(overridden.activeConfigurationSource, .developmentEnvOverride)
    }

    func testBackendProfileResolverResolvesPreferredProviderAndOverrides() {
        let tempDirectory = try! makeTempDirectory()
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
        let store = ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: settings
        )
        let registry = ProviderRegistry(
            settingsStore: store,
            secretStore: KeychainSecretStore(serviceName: "com.chainworks.tests.backend-profile")
        )
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
        XCTAssertEqual(binding?.configuredProviderID, alternateClaude.id)
        XCTAssertEqual(binding?.model, "claude-custom")
        XCTAssertEqual(binding?.effort, "high")
        XCTAssertEqual(binding?.providerIdentifier, "claude_code")
    }

    func testBackendProfileResolverSupportsMixedProvidersAcrossAgents() {
        let tempDirectory = try! makeTempDirectory()
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
        let store = ProviderSettingsStore(
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
        )
        let registry = ProviderRegistry(
            settingsStore: store,
            secretStore: KeychainSecretStore(serviceName: "com.chainworks.tests.mixed-providers")
        )
        let resolver = BackendProfileResolverV2(providerRegistry: registry)

        let bindings = try? resolver.resolveBindings(
            plan: makeMixedProviderPlan(),
            startOptions: .empty
        )

        XCTAssertEqual(bindings?["proposal_writer"]?.providerIdentifier, "codex")
        XCTAssertEqual(bindings?["proposal_reviewer"]?.providerIdentifier, "gemini")
    }

    func testSettingsTransferExportsSchemaVersion() throws {
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
        let appStore = AppConfigurationStore(
            fileURL: tempDirectory.appendingPathComponent("app-config.json"),
            initialConfiguration: configuration
        )
        let providerStore = ProviderSettingsStore(
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
        )

        let service = SettingsTransferService(
            appConfigurationStore: appStore,
            providerSettingsStore: providerStore,
            secretStore: KeychainSecretStore(serviceName: "com.chainworks.tests.settings")
        )

        let exportURL = try service.exportSettings(to: tempDirectory)
        let data = try Data(contentsOf: exportURL)
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let package = try decoder.decode(ExportableSettingsPackage.self, from: data)

        XCTAssertEqual(package.transferSchemaVersion, SettingsTransferService.currentSchemaVersion)
        XCTAssertEqual(package.providerSettings.configuredProviders.count, 1)
    }

    func testSettingsImportFailsClosedWhenSecretsAreMissing() throws {
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

        let appStore = AppConfigurationStore(
            fileURL: tempDirectory.appendingPathComponent("app-config.json"),
            initialConfiguration: initialConfiguration
        )
        let providerStore = ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [initialProvider],
                preferredProviderIDsByFamily: [ProviderFamily.codex.rawValue: initialProvider.id],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: true
            )
        )

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
            secretStore: KeychainSecretStore(serviceName: "com.chainworks.tests.import-empty")
        )

        XCTAssertThrowsError(try service.importSettings(from: packageURL))
        XCTAssertEqual(appStore.configuration, initialConfiguration)
        XCTAssertEqual(providerStore.settings.configuredProviders.map(\.displayName), ["Initial Codex"])
    }

    func testPreflightFailsWhenRequiredProviderFamilyIsMissing() async throws {
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

        let appStore = AppConfigurationStore(
            fileURL: tempDirectory.appendingPathComponent("app-config.json"),
            initialConfiguration: configuration
        )
        let providerStore = ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: .empty
        )
        let registry = ProviderRegistry(settingsStore: providerStore)
        let preflight = PreflightService(appConfigurationStore: appStore, providerRegistry: registry)

        let report = await preflight.runReport(
            workflowURL: URL(fileURLWithPath: configuration.workflowSourcePath),
            catalogURL: URL(fileURLWithPath: configuration.agentCatalogSourcePath),
            plan: makePlan(provider: "claude_code")
        )

        XCTAssertEqual(report.status, .fail)
        XCTAssertTrue(report.blockingIssues.contains { $0.contains("Claude") || $0.contains("No provider configured") })
    }

    func testPreflightFailsWhenProviderCredentialIsMissing() async throws {
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

        let appStore = AppConfigurationStore(
            fileURL: tempDirectory.appendingPathComponent("app-config.json"),
            initialConfiguration: configuration
        )
        let provider = ConfiguredProvider(
            family: .gemini,
            displayName: "Gemini API",
            transport: .httpAPI,
            endpoint: "https://generativelanguage.googleapis.com",
            authMode: .apiKey,
            defaultModel: "gemini-2.5-pro"
        )
        let providerStore = ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [provider],
                preferredProviderIDsByFamily: [ProviderFamily.gemini.rawValue: provider.id],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: true
            )
        )
        let registry = ProviderRegistry(
            settingsStore: providerStore,
            secretStore: KeychainSecretStore(serviceName: "com.chainworks.tests.missing-gemini-secret")
        )
        let preflight = PreflightService(appConfigurationStore: appStore, providerRegistry: registry)

        let report = await preflight.runReport(
            workflowURL: URL(fileURLWithPath: configuration.workflowSourcePath),
            catalogURL: URL(fileURLWithPath: configuration.agentCatalogSourcePath),
            plan: makePlan(provider: "gemini")
        )

        XCTAssertEqual(report.status, .fail)
        XCTAssertTrue(report.blockingIssues.contains { $0.localizedCaseInsensitiveContains("API key is missing") })
    }

    func testPreflightFailsWhenOverrideSelectsUnavailableModel() async throws {
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

        let appStore = AppConfigurationStore(
            fileURL: tempDirectory.appendingPathComponent("app-config.json"),
            initialConfiguration: configuration
        )
        let claude = ConfiguredProvider(
            family: .claude,
            displayName: "Claude CLI",
            transport: .cli,
            authMode: .none,
            defaultModel: "claude-sonnet-4"
        )
        let providerStore = ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [claude],
                preferredProviderIDsByFamily: [ProviderFamily.claude.rawValue: claude.id],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: true
            )
        )
        let registry = ProviderRegistry(
            settingsStore: providerStore,
            secretStore: KeychainSecretStore(serviceName: "com.chainworks.tests.unavailable-model")
        )
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

        XCTAssertEqual(report.status, .fail)
        XCTAssertTrue(report.blockingIssues.contains { $0.localizedCaseInsensitiveContains("not available") })
    }

    func testSampleRunLauncherCreatesFrozenProviderBindingSnapshot() async throws {
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
        let appStore = AppConfigurationStore(
            fileURL: tempDirectory.appendingPathComponent("app-config.json"),
            initialConfiguration: configuration
        )
        let providerStore = ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [
                    ConfiguredProvider(
                        family: .codex,
                        displayName: "Codex CLI",
                        transport: .cli,
                        authMode: .none,
                        defaultModel: "gpt-5-codex"
                    ),
                    ConfiguredProvider(
                        family: .claude,
                        displayName: "Claude CLI",
                        transport: .cli,
                        authMode: .none,
                        defaultModel: "claude-sonnet-4"
                    ),
                    ConfiguredProvider(
                        family: .gemini,
                        displayName: "Gemini CLI",
                        transport: .cli,
                        authMode: .none,
                        defaultModel: "gemini-2.5-pro"
                    )
                ],
                preferredProviderIDsByFamily: [:],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: false
            )
        )
        let registry = ProviderRegistry(
            settingsStore: providerStore,
            secretStore: KeychainSecretStore(serviceName: "com.chainworks.tests.sample-run")
        )
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

        let run = try await launcher.launchSampleRun()

        XCTAssertNotNil(run.providerBindingSnapshotJSON)
        XCTAssertNotNil(run.startOptionsJSON)

        let decoder = JSONDecoder()
        let bindings = try decoder.decode([String: ResolvedProviderBinding].self, from: try XCTUnwrap(run.providerBindingSnapshotJSON))
        XCTAssertFalse(bindings.isEmpty)
        XCTAssertEqual(run.workflowID, "proposal_to_release")
    }

    func testSupportBundleExportIncludesArtifactIndexAndSelectedArtifacts() async throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let (_, context) = try makeTestModelContainer()
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

        let appStore = AppConfigurationStore(
            fileURL: tempDirectory.appendingPathComponent("app-config.json"),
            initialConfiguration: AppConfiguration(
                runStorageBasePath: tempDirectory.appendingPathComponent("runs").path,
                worktreeBasePath: tempDirectory.appendingPathComponent("worktrees").path,
                workflowSourcePath: tempDirectory.appendingPathComponent("workflow.yaml").path,
                agentCatalogSourcePath: tempDirectory.appendingPathComponent("agents.yaml").path,
                supportBundleExportPath: tempDirectory.appendingPathComponent("exports").path,
                activeConfigurationSource: .persistedSettings
            )
        )
        let providerStore = ProviderSettingsStore(
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
        )
        let registry = ProviderRegistry(
            settingsStore: providerStore,
            secretStore: KeychainSecretStore(serviceName: "com.chainworks.tests.bundle")
        )
        await registry.refreshHealth()

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
        XCTAssertTrue(contents.contains { $0.hasSuffix("app-version.json") })
        XCTAssertTrue(contents.contains { $0.hasSuffix("provider-health.json") })
        XCTAssertTrue(contents.contains { $0.hasSuffix("artifact-index.json") })
        XCTAssertTrue(contents.contains { $0.hasSuffix("artifacts/proposal.md") })
    }
}
