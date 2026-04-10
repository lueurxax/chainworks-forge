import Testing
import SwiftData
import Foundation
#if os(macOS)
import AppKit
#endif
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

    private func clearSecurityScopedBookmarks() {
        SecurityScopedAccess.resetForTesting()
        #expect(SecurityScopedAccess.bookmarkedPathsForTesting().isEmpty)
    }

    private func makeCanonicalYAMLCopies(in tempDirectory: URL) throws -> (workflowURL: URL, catalogURL: URL) {
        let fileManager = FileManager.default
        let repoRoot = testRepositoryRootURL()
        let workflowSourceURL = repoRoot.appendingPathComponent("examples/workflows/workflow.yaml")
        let catalogSourceURL = repoRoot.appendingPathComponent("examples/agents/agents.yaml")
        let workflowCopyURL = tempDirectory.appendingPathComponent("workflow.yaml")
        let catalogCopyURL = tempDirectory.appendingPathComponent("agents.yaml")

        if fileManager.isReadableFile(atPath: workflowSourceURL.path) {
            try fileManager.copyItem(at: workflowSourceURL, to: workflowCopyURL)
            try writePortableCatalogCopy(from: catalogSourceURL, to: catalogCopyURL)
        } else {
            // Source tree not accessible from sandboxed test process —
            // write minimal inline fixtures sufficient for provider-platform proof.
            try minimalWorkflowFixture.write(to: workflowCopyURL, atomically: true, encoding: .utf8)
            try minimalCatalogFixture.write(to: catalogCopyURL, atomically: true, encoding: .utf8)
        }
        return (workflowCopyURL, catalogCopyURL)
    }

    private let minimalWorkflowFixture = """
    schema_version: 1
    workflow:
      id: provider_platform_test
      name: Provider Platform Test
      uses_agent_catalog: ./agents.yaml
      description: Minimal fixture for provider-platform proof.
      idea_input:
        mode: text_with_optional_file
      execution:
        single_active_run_per_idea: true
      required_providers:
        - codex
    variables: {}
    states:
      state_1_write:
        agents: [code_writer]
        transitions:
          - to: end
    initial_state: state_1_write
    """

    private let minimalCatalogFixture = """
    schema_version: 1
    app:
      name: Chainworks
      runtime: claude_agent
      transport: rest_sse
    agents:
      code_writer:
        title: Code Writer
        mode: tool_use
        provider: codex
        model: codex
        effort: high
        max_turns: 5
        temperature: 0.0
        permission_profile: ORCH
        prompt: Write code.
        output_contract: implementation
        outputs: [output]
    """

    private func makeAdapters() -> [ProviderFamily: any ProviderAdapter] {
        ProviderAdapterFactory.makeAdapters()
    }

    private func unzipArchive(_ archiveURL: URL, to destinationURL: URL) throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/unzip")
        process.arguments = ["-qq", archiveURL.path, "-d", destinationURL.path]
        try process.run()
        process.waitUntilExit()
        #expect(process.terminationStatus == 0, "Expected unzip to succeed for \(archiveURL.path)")
    }

    private func makePlan(
        provider: String,
        model: String = "default-model",
        backendProfileID: String = "reviewer_profile"
    ) -> RunPlan {
        let agent = ResolvedAgent(
            id: "proposal_writer",
            title: "Proposal Writer",
            mode: "tool_use",
            backendProfileID: backendProfileID,
            provider: provider,
            model: model,
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

    @Test("App configuration store does not auto-bookmark seeded or persisted paths")
    mutating func appConfigurationStoreDoesNotAutoBookmarkSeededOrPersistedPaths() throws {
        clearSecurityScopedBookmarks()

        let tempDirectory = try makeTempDirectory()
        defer {
            clearSecurityScopedBookmarks()
            try? FileManager.default.removeItem(at: tempDirectory)
        }

        let configuration = AppConfiguration(
            runStorageBasePath: tempDirectory.appendingPathComponent("runs").path,
            worktreeBasePath: tempDirectory.appendingPathComponent("worktrees").path,
            workflowSourcePath: tempDirectory.appendingPathComponent("workflow.yaml").path,
            agentCatalogSourcePath: tempDirectory.appendingPathComponent("agents.yaml").path,
            supportBundleExportPath: tempDirectory.appendingPathComponent("exports").path,
            activeConfigurationSource: .persistedSettings
        )

        _ = retain(AppConfigurationStore(
            fileURL: tempDirectory.appendingPathComponent("app-config.json"),
            initialConfiguration: configuration
        ))

        #expect(SecurityScopedAccess.bookmarkedPathsForTesting().isEmpty)
    }

    @Test("Fixture live runtime is ready")
    mutating func fixtureLiveRuntimeIsReady() throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let (_, context) = try makeTestModelContainer()
        let configuration = LiveRuntimeConfiguration(
            override: LiveExecutionOverride(
                enabled: true,
                provider: "claude_acp",
                model: "fixture-model",
                effort: "high"
            ),
            transportMode: .fixtureFullMVPSuccess
        )

        let service = ExecutionService(
            modelContext: context,
            executor: SimulatedAgentExecutor(simulatedDelay: 0),
            catalog: nil,
            stewardConfig: nil,
            liveRuntimeConfiguration: configuration
        )

        #expect(service.supportsLiveExecution)
        switch service.liveRuntimeReadiness {
        case .ready(let summary, let source):
            #expect(summary.contains("fixture-model"))
            #expect(source == "Fixture backend")
        case .unavailable(let reason, let recovery):
            Issue.record("Fixture runtime should be ready, got unavailable: \(reason) / \(recovery)")
        }
    }

    @Test("Backend profile resolver resolves preferred provider and overrides")
    mutating func backendProfileResolverResolvesPreferredProviderAndOverrides() throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let claude = ConfiguredProvider(
            family: .claudeACP,
            displayName: "Claude CLI",
            transport: .cli,
            authMode: .none,
            defaultModel: "sonnet"
        )
        let alternateClaude = ConfiguredProvider(
            family: .claudeACP,
            displayName: "Claude HTTP",
            transport: .httpAPI,
            endpoint: "http://localhost:8080",
            authMode: .none,
            defaultModel: "opus"
        )
        let settings = ProviderSettings(
            configuredProviders: [claude, alternateClaude],
            preferredProviderIDsByFamily: [ProviderFamily.claudeACP.rawValue: claude.id],
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
        #expect(binding?.providerIdentifier == "claude_acp")
    }

    @Test("Backend profile resolver supports mixed providers across agents")
    mutating func backendProfileResolverSupportsMixedProvidersAcrossAgents() throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let codex = ConfiguredProvider(
            family: .codexACP,
            displayName: "Codex CLI",
            transport: .cli,
            authMode: .none,
            defaultModel: "gpt-5-codex"
        )
        let gemini = ConfiguredProvider(
            family: .geminiACP,
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
                    ProviderFamily.codexACP.rawValue: codex.id,
                    ProviderFamily.geminiACP.rawValue: gemini.id
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

        #expect(bindings?["proposal_writer"]?.providerIdentifier == "codex_acp")
        #expect(bindings?["proposal_reviewer"]?.providerIdentifier == "gemini_acp")
    }

    @Test("Backend profile resolver prefers backend profile model over configured provider default")
    mutating func backendProfileResolverPrefersBackendProfileModelOverConfiguredProviderDefault() throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let claude = ConfiguredProvider(
            family: .claudeACP,
            displayName: "Claude ACP",
            transport: .cli,
            endpoint: "https://127.0.0.1:51200",
            authMode: .none,
            defaultModel: "sonnet"
        )
        let store = retain(ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [claude],
                preferredProviderIDsByFamily: [ProviderFamily.claudeACP.rawValue: claude.id],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: true
            )
        ))
        let registry = retain(ProviderRegistry(
            settingsStore: store,
            secretStore: makeTestSecretStore("com.chainworks.tests.backend-profile-precedence")
        ))
        let resolver = BackendProfileResolverV2(providerRegistry: registry)

        let orchestrator = ResolvedAgent(
            id: "lead_orchestrator",
            title: "Lead / Orchestrator",
            mode: "orchestration",
            backendProfileID: "claude_orchestrator_high",
            provider: "claude_code",
            model: "opus",
            effort: "high",
            maxTurns: 20,
            temperature: 0.1,
            permissionProfile: "ORCH",
            skillRef: "orchestrator_core",
            skillRole: nil,
            prompt: "Drive the workflow forward",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["run_state"]
        )

        let plan = RunPlan(
            workflowID: "resolver_precedence_test",
            workflowTitle: "Resolver Precedence Test",
            states: [:],
            initialStateID: "state_1",
            agentBindings: [orchestrator.id: orchestrator],
            variables: [:],
            scoring: nil,
            failurePolicy: nil,
            workflowSnapshotHash: "workflow-hash",
            catalogSnapshotHash: "catalog-hash",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: RunPlan.currentCompilerVersion
        )

        let binding = try #require(resolver.resolveBindings(plan: plan, startOptions: .empty)["lead_orchestrator"])
        #expect(binding.model == "opus")

        let provenance = try #require(resolver.resolveProvenances(plan: plan, startOptions: .empty)["lead_orchestrator"])
        #expect(provenance.source == .backendProfileDefault)
        #expect(provenance.resolvedModel == "opus")
        #expect(provenance.configuredProviderDefaultModel == "sonnet")
    }

    @Test("Provider registry caches troubleshooting reports after refresh")
    mutating func providerRegistryCachesTroubleshootingReports() async throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let appConfiguration = AppConfiguration(
            runStorageBasePath: tempDirectory.appendingPathComponent("runs").path,
            worktreeBasePath: tempDirectory.appendingPathComponent("worktrees").path,
            workflowSourcePath: tempDirectory.appendingPathComponent("workflow.yaml").path,
            agentCatalogSourcePath: tempDirectory.appendingPathComponent("agents.yaml").path,
            supportBundleExportPath: tempDirectory.appendingPathComponent("exports").path,
            activeConfigurationSource: .persistedSettings
        )
        let provider = ConfiguredProvider(
            family: .claudeACP,
            displayName: "Claude ACP",
            transport: .cli,
            endpoint: "https://127.0.0.1:51200",
            authMode: .apiKey,
            defaultModel: "opus"
        )
        let secretStore = makeTestSecretStore("com.chainworks.tests.cached-reports")
        try secretStore.setSecret("test-key", for: ProviderAdapterSupport.secretKey(for: provider))
        let providerStore = retain(ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [provider],
                preferredProviderIDsByFamily: [ProviderFamily.claudeACP.rawValue: provider.id],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: true
            )
        ))
        let registry = retain(ProviderRegistry(
            settingsStore: providerStore,
            secretStore: secretStore,
            adapters: makeAdapters()
        ))

        await registry.refreshDiagnostics(appConfiguration: appConfiguration)

        let report = try #require(registry.troubleshootingReport(for: provider.id))
        #expect(report.displayName == "Claude ACP")
        #expect(registry.lastRefreshedAt != nil)
        #expect(report.evidence.contains { $0.label == "Configured transport" })
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
                        family: .codexACP,
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
            family: .codexACP,
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
                preferredProviderIDsByFamily: [ProviderFamily.codexACP.rawValue: initialProvider.id],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: true
            )
        ))

        let importProvider = ConfiguredProvider(
            family: .geminiACP,
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
                preferredProviderIDsByFamily: [ProviderFamily.geminiACP.rawValue: importProvider.id],
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

        let canonicalCopies = try makeCanonicalYAMLCopies(in: tempDirectory)
        let configuration = AppConfiguration(
            runStorageBasePath: tempDirectory.appendingPathComponent("runs").path,
            worktreeBasePath: tempDirectory.appendingPathComponent("worktrees").path,
            workflowSourcePath: canonicalCopies.workflowURL.path,
            agentCatalogSourcePath: canonicalCopies.catalogURL.path,
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

        let canonicalCopies = try makeCanonicalYAMLCopies(in: tempDirectory)
        let configuration = AppConfiguration(
            runStorageBasePath: tempDirectory.appendingPathComponent("runs").path,
            worktreeBasePath: tempDirectory.appendingPathComponent("worktrees").path,
            workflowSourcePath: canonicalCopies.workflowURL.path,
            agentCatalogSourcePath: canonicalCopies.catalogURL.path,
            supportBundleExportPath: tempDirectory.appendingPathComponent("exports").path,
            activeConfigurationSource: .persistedSettings
        )

        let appStore = retain(AppConfigurationStore(
            fileURL: tempDirectory.appendingPathComponent("app-config.json"),
            initialConfiguration: configuration
        ))
        let provider = ConfiguredProvider(
            family: .geminiACP,
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
                preferredProviderIDsByFamily: [ProviderFamily.geminiACP.rawValue: provider.id],
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

        let canonicalCopies = try makeCanonicalYAMLCopies(in: tempDirectory)
        let configuration = AppConfiguration(
            runStorageBasePath: tempDirectory.appendingPathComponent("runs").path,
            worktreeBasePath: tempDirectory.appendingPathComponent("worktrees").path,
            workflowSourcePath: canonicalCopies.workflowURL.path,
            agentCatalogSourcePath: canonicalCopies.catalogURL.path,
            supportBundleExportPath: tempDirectory.appendingPathComponent("exports").path,
            activeConfigurationSource: .persistedSettings
        )

        let appStore = retain(AppConfigurationStore(
            fileURL: tempDirectory.appendingPathComponent("app-config.json"),
            initialConfiguration: configuration
        ))
        let claude = ConfiguredProvider(
            family: .claudeACP,
            displayName: "Claude CLI",
            transport: .cli,
            authMode: .none,
            defaultModel: "sonnet"
        )
        let providerStore = retain(ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [claude],
                preferredProviderIDsByFamily: [ProviderFamily.claudeACP.rawValue: claude.id],
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

    @Test("Preflight accepts non-default Gemini family models when provider default differs")
    mutating func preflightAcceptsGeminiFlashWhenProviderDefaultIsPro() async throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let canonicalCopies = try makeCanonicalYAMLCopies(in: tempDirectory)
        let configuration = AppConfiguration(
            runStorageBasePath: tempDirectory.appendingPathComponent("runs").path,
            worktreeBasePath: tempDirectory.appendingPathComponent("worktrees").path,
            workflowSourcePath: canonicalCopies.workflowURL.path,
            agentCatalogSourcePath: canonicalCopies.catalogURL.path,
            supportBundleExportPath: tempDirectory.appendingPathComponent("exports").path,
            activeConfigurationSource: .persistedSettings
        )

        let appStore = retain(AppConfigurationStore(
            fileURL: tempDirectory.appendingPathComponent("app-config.json"),
            initialConfiguration: configuration
        ))
        let gemini = ConfiguredProvider(
            family: .geminiACP,
            displayName: "Gemini",
            transport: .cli,
            endpoint: "https://127.0.0.1:51200",
            authMode: .none,
            defaultModel: "gemini-2.5-pro"
        )
        let providerStore = retain(ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [gemini],
                preferredProviderIDsByFamily: [ProviderFamily.geminiACP.rawValue: gemini.id],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: true
            )
        ))
        let registry = retain(ProviderRegistry(
            settingsStore: providerStore,
            secretStore: makeTestSecretStore("com.chainworks.tests.gemini-family-models"),
            adapters: makeAdapters()
        ))
        let preflight = PreflightService(appConfigurationStore: appStore, providerRegistry: registry)

        let report = await preflight.runReport(
            workflowURL: URL(fileURLWithPath: configuration.workflowSourcePath),
            catalogURL: URL(fileURLWithPath: configuration.agentCatalogSourcePath),
            plan: makePlan(provider: "gemini", model: "gemini-2.5-flash")
        )

        #expect(!report.blockingIssues.contains { $0.localizedCaseInsensitiveContains("gemini-2.5-flash is not available") })
        #expect(report.checks.contains {
            $0.title == "Gemini Model"
                && $0.message == "Model gemini-2.5-flash is available for Gemini"
        })
    }

    @Test("Preflight matches provider models case-insensitively")
    mutating func preflightMatchesProviderModelsCaseInsensitively() async throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let canonicalCopies = try makeCanonicalYAMLCopies(in: tempDirectory)
        let configuration = AppConfiguration(
            runStorageBasePath: tempDirectory.appendingPathComponent("runs").path,
            worktreeBasePath: tempDirectory.appendingPathComponent("worktrees").path,
            workflowSourcePath: canonicalCopies.workflowURL.path,
            agentCatalogSourcePath: canonicalCopies.catalogURL.path,
            supportBundleExportPath: tempDirectory.appendingPathComponent("exports").path,
            activeConfigurationSource: .persistedSettings
        )

        let appStore = retain(AppConfigurationStore(
            fileURL: tempDirectory.appendingPathComponent("app-config.json"),
            initialConfiguration: configuration
        ))
        let codex = ConfiguredProvider(
            family: .codexACP,
            displayName: "Codex ACP",
            transport: .cli,
            authMode: .none,
            defaultModel: "gpt-5"
        )
        let providerStore = retain(ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [codex],
                preferredProviderIDsByFamily: [ProviderFamily.codexACP.rawValue: codex.id],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: true
            )
        ))
        let registry = retain(ProviderRegistry(
            settingsStore: providerStore,
            secretStore: makeTestSecretStore("com.chainworks.tests.model-case-insensitive"),
            adapters: makeAdapters()
        ))
        let preflight = PreflightService(appConfigurationStore: appStore, providerRegistry: registry)

        let architect = ResolvedAgent(
            id: "proposal_reviewer_architect",
            title: "Proposal Reviewer / Architect",
            mode: "tool_use",
            backendProfileID: "codex_architect_high",
            provider: "codex_acp",
            model: "GPT-5",
            effort: "high",
            maxTurns: 8,
            temperature: 0.0,
            permissionProfile: "read_only",
            skillRef: "skill",
            skillRole: nil,
            prompt: "Review the proposal as an architect",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: ["proposal_current"],
            outputs: ["proposal_review_architect"]
        )

        let plan = RunPlan(
            workflowID: "model_case_insensitive",
            workflowTitle: "Model Case Insensitive",
            states: [:],
            initialStateID: "state_1",
            agentBindings: [architect.id: architect],
            variables: [:],
            scoring: nil,
            failurePolicy: nil,
            workflowSnapshotHash: "workflow-hash",
            catalogSnapshotHash: "catalog-hash",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: RunPlan.currentCompilerVersion
        )

        let report = await preflight.runReport(
            workflowURL: URL(fileURLWithPath: configuration.workflowSourcePath),
            catalogURL: URL(fileURLWithPath: configuration.agentCatalogSourcePath),
            plan: plan
        )

        let modelBlockingIssues = report.blockingIssues.filter { $0.contains("Model GPT-5 is not available") }
        #expect(modelBlockingIssues.isEmpty, "GPT-5 should be case-insensitively matched to gpt-5. Blocking issues: \(report.blockingIssues)")
    }

    @Test("Preflight does not block codex ACP bindings on legacy provider health or missing codex MCP mappings")
    mutating func preflightAllowsCodexACPBindingsWithoutLegacyCredentialRequirement() async throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let canonicalCopies = try makeCanonicalYAMLCopies(in: tempDirectory)
        let configuration = AppConfiguration(
            runStorageBasePath: tempDirectory.appendingPathComponent("runs").path,
            worktreeBasePath: tempDirectory.appendingPathComponent("worktrees").path,
            workflowSourcePath: canonicalCopies.workflowURL.path,
            agentCatalogSourcePath: canonicalCopies.catalogURL.path,
            supportBundleExportPath: tempDirectory.appendingPathComponent("exports").path,
            activeConfigurationSource: .persistedSettings
        )

        let appStore = retain(AppConfigurationStore(
            fileURL: tempDirectory.appendingPathComponent("app-config.json"),
            initialConfiguration: configuration
        ))
        let codex = ConfiguredProvider(
            family: .codexACP,
            displayName: "Codex ACP",
            transport: .cli,
            endpoint: "https://127.0.0.1:51200",
            authMode: .apiKey,
            defaultModel: "gpt-5-codex"
        )
        let providerStore = retain(ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [codex],
                preferredProviderIDsByFamily: [ProviderFamily.codexACP.rawValue: codex.id],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: true
            )
        ))
        let registry = retain(ProviderRegistry(
            settingsStore: providerStore,
            secretStore: makeTestSecretStore("com.chainworks.tests.codex-acp-preflight"),
            adapters: makeAdapters()
        ))
        let preflight = PreflightService(appConfigurationStore: appStore, providerRegistry: registry)

        let agent = ResolvedAgent(
            id: "code_writer",
            title: "Code Writer",
            mode: "tool_use",
            backendProfileID: "codex_writer_high",
            provider: "codex_acp",
            model: "GPT-5",
            effort: "high",
            maxTurns: 18,
            temperature: 0.12,
            permissionProfile: "IMPLEMENT_WRITE",
            mcpProfileID: "code_build_rich",
            skillRef: "implementation_writer_core",
            skillRole: nil,
            prompt: "Implement the approved proposal.",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["implementation_summary"],
            worktreeWriteEnabled: true,
            runtimeProfileID: "codex_acp"
        )

        let plan = RunPlan(
            workflowID: "codex_acp_preflight",
            workflowTitle: "Codex ACP Preflight",
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

        let report = await preflight.runReport(
            workflowURL: URL(fileURLWithPath: configuration.workflowSourcePath),
            catalogURL: URL(fileURLWithPath: configuration.agentCatalogSourcePath),
            plan: plan
        )

        #expect(!report.blockingIssues.contains { $0.contains("runtime mapping for 'codex'") })
        #expect(!report.blockingIssues.contains { $0.localizedCaseInsensitiveContains("API key is missing") })
        #expect(!report.blockingIssues.contains { $0.localizedCaseInsensitiveContains("provider requires attention") })
    }

    @Test("Sample run launcher creates frozen provider binding snapshot")
    mutating func sampleRunLauncherCreatesFrozenProviderBindingSnapshot() async throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let (container, context) = try makeTestModelContainer()
        _ = container

        let canonicalCopies = try makeCanonicalYAMLCopies(in: tempDirectory)
        let configuration = AppConfiguration(
            runStorageBasePath: tempDirectory.appendingPathComponent("runs").path,
            worktreeBasePath: tempDirectory.appendingPathComponent("worktrees").path,
            workflowSourcePath: canonicalCopies.workflowURL.path,
            agentCatalogSourcePath: canonicalCopies.catalogURL.path,
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
                        family: .codexACP,
                        displayName: "Codex ACP",
                        transport: .cli,
                        endpoint: "https://127.0.0.1:51200",
                        authMode: .none,
                        defaultModel: "gpt-5.4"
                    ),
                    ConfiguredProvider(
                        family: .claudeACP,
                        displayName: "Claude ACP",
                        transport: .cli,
                        endpoint: "https://127.0.0.1:51200",
                        authMode: .none,
                        defaultModel: "opus"
                    ),
                    ConfiguredProvider(
                        family: .geminiACP,
                        displayName: "Gemini ACP",
                        transport: .cli,
                        endpoint: "https://127.0.0.1:51200",
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
            secretStore: makeTestSecretStore("com.chainworks.tests.sample-run"),
            adapters: makeAdapters()
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
        #expect(run.frozenWorkspaceRootPath?.isEmpty == false)
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
        agent.resolvedModel = "sonnet"
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
                        family: .claudeACP,
                        displayName: "Claude CLI",
                        transport: .cli,
                        authMode: .none,
                        defaultModel: "sonnet"
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

    @Test("Provider draft resets generated model when switching from Codex to Claude")
    mutating func providerDraftResetsGeneratedModelWhenSwitchingFamilies() {
        var draft = ProviderDraft()
        let configuration = AppConfiguration.seededDefault()

        draft.applyFamilyDefaults(.codexACP, configuration: configuration)
        #expect(draft.displayName == "Codex ACP CLI")
        #expect(draft.defaultModel == "gpt-5")

        draft.applyFamilyDefaults(.claudeACP, configuration: configuration)

        #expect(draft.family == .claudeACP)
        #expect(draft.displayName == "Claude ACP CLI")
        #expect(draft.defaultModel == "sonnet")
    }

    @Test("Provider draft normalizes cross-family model before save")
    mutating func providerDraftNormalizesCrossFamilyModelBeforeSave() {
        var draft = ProviderDraft()
        draft.family = .claudeACP
        draft.displayName = "Claude ACP"
        draft.transport = .cli
        draft.defaultModel = "gpt-5-codex"

        draft.normalizeForSave()
        let provider = draft.makeProvider()

        #expect(provider.family == .claudeACP)
        #expect(provider.defaultModel == "sonnet")
    }

    @Test("Provider settings store sanitizes stale cross-family provider defaults")
    mutating func providerSettingsStoreSanitizesStaleCrossFamilyDefaults() throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let staleClaude = ConfiguredProvider(
            family: .claudeACP,
            displayName: "Codex ACP CLI",
            transport: .cli,
            endpoint: "https://127.0.0.1:51200",
            authMode: .none,
            defaultModel: "gpt-5-codex"
        )

        let store = retain(ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [staleClaude],
                preferredProviderIDsByFamily: [ProviderFamily.claudeACP.rawValue: staleClaude.id],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: true
            )
        ))

        let sanitized = try #require(store.settings.configuredProviders.first)
        #expect(sanitized.family == .claudeACP)
        #expect(sanitized.defaultModel == "sonnet")
        #expect(sanitized.displayName == "Claude ACP CLI")
    }

    @Test("Provider settings store canonicalizes legacy Claude ACP model identifiers")
    mutating func providerSettingsStoreCanonicalizesLegacyClaudeACPModelIdentifiers() throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let legacyClaude = ConfiguredProvider(
            family: .claudeACP,
            displayName: "Claude ACP",
            transport: .cli,
            endpoint: "https://127.0.0.1:51200",
            authMode: .none,
            defaultModel: "claude-opus-4.6"
        )

        let fileURL = tempDirectory.appendingPathComponent("provider-settings.json")
        let store = retain(ProviderSettingsStore(
            fileURL: fileURL,
            initialSettings: ProviderSettings(
                configuredProviders: [legacyClaude],
                preferredProviderIDsByFamily: [ProviderFamily.claudeACP.rawValue: legacyClaude.id],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: true
            )
        ))

        let sanitized = try #require(store.settings.configuredProviders.first)
        #expect(sanitized.defaultModel == "opus")

        let persisted = try JSONDecoder().decode(ProviderSettings.self, from: Data(contentsOf: fileURL))
        let persistedClaude = try #require(persisted.configuredProviders.first)
        #expect(persistedClaude.defaultModel == "opus")
    }
}
