import Testing
import SwiftData
import Foundation
@testable import Chainworks_Forge

@MainActor
@Suite("ProviderPlatform", .tags(.fast, .provider))
struct ProviderPlatformTests {
    private static var retainedObjects: [AnyObject] = []
    private static var retainedRegistries: [ProviderRegistry] = []

    final class TestGooseHandle: GooseServerProcessHandle {
        var isRunning: Bool = true

        func terminate() {
            isRunning = false
        }
    }

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

    private func makeAdapters(
        gooseProbe: @escaping @Sendable (URL) async -> GooseServerReachability = { _ in .reachable(statusCode: 200) }
    ) -> [ProviderFamily: any ProviderAdapter] {
        [
            .codex: CodexProviderAdapter(gooseProbe: gooseProbe),
            .claude: ClaudeProviderAdapter(gooseProbe: gooseProbe),
            .gemini: GeminiProviderAdapter(gooseProbe: gooseProbe)
        ]
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

    @Test("Managed Goose server launches on bootstrap when autostart is enabled")
    mutating func managedGooseServerLaunchesOnBootstrap() async throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let store = retain(AppConfigurationStore(
            fileURL: tempDirectory.appendingPathComponent("app-config.json"),
            initialConfiguration: AppConfiguration(
                runStorageBasePath: tempDirectory.appendingPathComponent("runs").path,
                worktreeBasePath: tempDirectory.appendingPathComponent("worktrees").path,
                workflowSourcePath: tempDirectory.appendingPathComponent("workflow.yaml").path,
                agentCatalogSourcePath: tempDirectory.appendingPathComponent("agents.yaml").path,
                supportBundleExportPath: tempDirectory.appendingPathComponent("exports").path,
                gooseServerAutostart: true,
                gooseServerBinaryPath: "/bin/sh",
                gooseServerSecretKey: "dev-secret",
                activeConfigurationSource: .persistedSettings
            )
        ))

        var launchPlan: GooseManagedServerLaunchPlan?
        var launchCount = 0
        var probeCount = 0
        let manager = GooseServerManager(
            appConfigurationStore: store,
            probe: { _ in
                probeCount += 1
                return probeCount >= 2 ? .reachable(statusCode: 200) : .unreachable(reason: "Connection refused")
            },
            launcher: { plan in
                launchPlan = plan
                launchCount += 1
                return TestGooseHandle()
            }
        )

        await manager.bootstrap()

        #expect(manager.launchState == .running)
        #expect(launchCount == 1)
        #expect(launchPlan?.arguments == ["agent"])
        #expect(launchPlan?.environment["GOOSE_PORT"] == "51200")
        #expect(launchPlan?.environment["GOOSE_HOST"] == "127.0.0.1")
        #expect(launchPlan?.environment["GOOSE_TLS"] == "true")
        #expect(launchPlan?.environment["PATH"]?.contains("/opt/homebrew/bin") == true)
    }

    @Test("Managed Goose server does not auto-launch when autostart is disabled")
    mutating func managedGooseServerDoesNotAutolaunchWhenDisabled() async throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let store = retain(AppConfigurationStore(
            fileURL: tempDirectory.appendingPathComponent("app-config.json"),
            initialConfiguration: AppConfiguration(
                runStorageBasePath: tempDirectory.appendingPathComponent("runs").path,
                worktreeBasePath: tempDirectory.appendingPathComponent("worktrees").path,
                workflowSourcePath: tempDirectory.appendingPathComponent("workflow.yaml").path,
                agentCatalogSourcePath: tempDirectory.appendingPathComponent("agents.yaml").path,
                supportBundleExportPath: tempDirectory.appendingPathComponent("exports").path,
                gooseServerAutostart: false,
                gooseServerBinaryPath: "/bin/sh",
                gooseServerSecretKey: "dev-secret",
                activeConfigurationSource: .persistedSettings
            )
        ))

        var launchCount = 0
        let manager = GooseServerManager(
            appConfigurationStore: store,
            probe: { _ in .reachable(statusCode: 200) },
            launcher: { _ in
                launchCount += 1
                return TestGooseHandle()
            }
        )

        await manager.bootstrap()

        #expect(manager.launchState == .external)
        #expect(launchCount == 0)
        #expect(manager.liveRuntimeConfiguration?.baseURL.absoluteString == "https://127.0.0.1:51200")
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

    @Test("Provider troubleshooting reports Goose-first guidance for Codex")
    mutating func providerTroubleshootingReportsGooseFirstGuidanceForCodex() async throws {
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
        let appStore = retain(AppConfigurationStore(
            fileURL: tempDirectory.appendingPathComponent("app-config.json"),
            initialConfiguration: appConfiguration
        ))
        let provider = ConfiguredProvider(
            family: .codex,
            displayName: "Codex Goose",
            transport: .gooseServer,
            endpoint: "https://127.0.0.1:51200",
            authMode: .none,
            defaultModel: "gpt-5-codex"
        )
        let providerStore = retain(ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [provider],
                preferredProviderIDsByFamily: [ProviderFamily.codex.rawValue: provider.id],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: true
            )
        ))
        let registry = retain(ProviderRegistry(
            settingsStore: providerStore,
            secretStore: makeTestSecretStore("com.chainworks.tests.goose-first"),
            adapters: makeAdapters()
        ))

        await registry.refreshDiagnostics(appConfiguration: appStore.configuration)
        let report = try #require(registry.troubleshootingReport(for: provider.id))

        #expect(report.status == .healthy)
        #expect(report.gooseFirstGuidance != nil)
        #expect(report.evidence.contains { $0.label == "Endpoint" })
        #expect(report.evidence.contains { $0.label == "Goose server reachability" && $0.value.contains("Reachable via") })
    }

    @Test("Goose handshake probe starts configured_unverified then verifies")
    mutating func gooseHandshakeProbeStartsConfiguredUnverifiedThenVerifies() async throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

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
        let provider = ConfiguredProvider(
            family: .claude,
            displayName: "Claude Goose",
            transport: .gooseServer,
            endpoint: "https://127.0.0.1:51200",
            authMode: .none,
            defaultModel: "claude-sonnet-4"
        )
        let providerStore = retain(ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [provider],
                preferredProviderIDsByFamily: [ProviderFamily.claude.rawValue: provider.id],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: true
            )
        ))
        let registry = retain(ProviderRegistry(
            settingsStore: providerStore,
            secretStore: makeTestSecretStore("com.chainworks.tests.goose-journey"),
            adapters: makeAdapters()
        ))

        let probe = GooseProviderHandshakeProbe(
            providerRegistry: registry,
            appConfigurationStore: appStore
        )
        let initial = try #require(probe.configuredSnapshot(for: provider.id, origin: .providerSettings))
        #expect(initial.journeyState == .configuredUnverified)

        let verified = try #require(await probe.probe(providerID: provider.id, origin: .providerSettings))
        #expect(verified.journeyState == .verified)
        #expect(verified.report?.status == .healthy)
    }

    @Test("Goose handshake probe maps blocked troubleshooting to failing")
    mutating func gooseHandshakeProbeMapsBlockedTroubleshootingToFailing() async throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

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
        let provider = ConfiguredProvider(
            family: .codex,
            displayName: "Codex Goose",
            transport: .gooseServer,
            endpoint: "https://127.0.0.1:51200",
            authMode: .none,
            defaultModel: "gpt-5-codex"
        )
        let providerStore = retain(ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [provider],
                preferredProviderIDsByFamily: [ProviderFamily.codex.rawValue: provider.id],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: true
            )
        ))
        let registry = retain(ProviderRegistry(
            settingsStore: providerStore,
            secretStore: makeTestSecretStore("com.chainworks.tests.goose-journey-blocked"),
            adapters: makeAdapters(gooseProbe: { _ in
                .unreachable(reason: "Could not connect to the server.")
            })
        ))

        let probe = GooseProviderHandshakeProbe(
            providerRegistry: registry,
            appConfigurationStore: appStore
        )
        let failing = try #require(await probe.probe(providerID: provider.id, origin: .firstRunWizard))
        #expect(failing.journeyState == .failing)
        #expect(failing.report?.failureLayer == .gooseReachability)
    }

    @Test("Provider troubleshooting blocks Goose-backed provider without endpoint")
    mutating func providerTroubleshootingBlocksMissingGooseEndpoint() async throws {
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
            family: .codex,
            displayName: "Codex Goose",
            transport: .gooseServer,
            endpoint: nil,
            authMode: .none,
            defaultModel: "gpt-5-codex"
        )
        let providerStore = retain(ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [provider],
                preferredProviderIDsByFamily: [ProviderFamily.codex.rawValue: provider.id],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: true
            )
        ))
        let registry = retain(ProviderRegistry(
            settingsStore: providerStore,
            secretStore: makeTestSecretStore("com.chainworks.tests.goose-endpoint")
        ))

        let service = ProviderTroubleshootingService()
        let report = await service.report(
            for: provider,
            providerRegistry: registry,
            appConfiguration: appConfiguration
        )

        #expect(report.status == .blocked)
        #expect(report.failureLayer == .gooseEndpoint)
        #expect(report.evidence.contains { $0.label == "Endpoint" && $0.state == .blocked })
    }

    @Test("Provider troubleshooting blocks Codex CLI fallback when executable is missing")
    mutating func providerTroubleshootingBlocksMissingCLIExecutable() async throws {
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
            family: .codex,
            displayName: "Codex CLI",
            transport: .cli,
            authMode: .none,
            defaultModel: "gpt-5-codex"
        )
        let providerStore = retain(ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [provider],
                preferredProviderIDsByFamily: [ProviderFamily.codex.rawValue: provider.id],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: true
            )
        ))
        let registry = retain(ProviderRegistry(
            settingsStore: providerStore,
            secretStore: makeTestSecretStore("com.chainworks.tests.missing-cli")
        ))

        let service = ProviderTroubleshootingService(whichExecutable: { _ in nil })
        let report = await service.report(
            for: provider,
            providerRegistry: registry,
            appConfiguration: appConfiguration
        )

        #expect(report.status == .blocked)
        #expect(report.failureLayer == .cliExecutable)
        #expect(report.remediation.contains { $0.contains("Goose Server transport") })
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
            family: .claude,
            displayName: "Claude Goose",
            transport: .gooseServer,
            endpoint: "https://127.0.0.1:51200",
            authMode: .apiKey,
            defaultModel: "claude-opus-4"
        )
        let secretStore = makeTestSecretStore("com.chainworks.tests.cached-reports")
        try secretStore.setSecret("test-key", for: ProviderAdapterSupport.secretKey(for: provider))
        let providerStore = retain(ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [provider],
                preferredProviderIDsByFamily: [ProviderFamily.claude.rawValue: provider.id],
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
        #expect(report.displayName == "Claude Goose")
        #expect(registry.lastRefreshedAt != nil)
        #expect(report.evidence.contains { $0.label == "Goose server reachability" && $0.state == .info })
    }

    @Test("Goose-backed provider health marks server as unavailable when status probe fails")
    mutating func gooseBackedProviderHealthMarksServerUnavailableWhenProbeFails() async throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let provider = ConfiguredProvider(
            family: .codex,
            displayName: "Codex Goose",
            transport: .gooseServer,
            endpoint: "https://127.0.0.1:51200",
            authMode: .none,
            defaultModel: "gpt-5-codex"
        )
        let providerStore = retain(ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [provider],
                preferredProviderIDsByFamily: [ProviderFamily.codex.rawValue: provider.id],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: true
            )
        ))
        let registry = retain(ProviderRegistry(
            settingsStore: providerStore,
            secretStore: makeTestSecretStore("com.chainworks.tests.goose-unreachable"),
            adapters: makeAdapters(gooseProbe: { _ in
                .unreachable(reason: "Could not connect to the server.")
            })
        ))

        await registry.refreshHealth()

        let snapshot = try #require(registry.healthSnapshot(for: provider.id))
        #expect(snapshot.status == .unavailable)
        #expect(snapshot.summary.contains("Goose server is unreachable"))
        #expect(snapshot.blockingIssues.contains { $0.contains("Could not connect to the server.") })
    }

    @Test("Provider troubleshooting reports unreachable Goose server explicitly")
    mutating func providerTroubleshootingReportsUnreachableGooseServerExplicitly() async throws {
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
            family: .claude,
            displayName: "Claude Goose",
            transport: .gooseServer,
            endpoint: "https://127.0.0.1:51200",
            authMode: .none,
            defaultModel: "claude-opus-4"
        )
        let providerStore = retain(ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [provider],
                preferredProviderIDsByFamily: [ProviderFamily.claude.rawValue: provider.id],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: true
            )
        ))
        let registry = retain(ProviderRegistry(
            settingsStore: providerStore,
            secretStore: makeTestSecretStore("com.chainworks.tests.goose-report"),
            adapters: makeAdapters(gooseProbe: { _ in
                .unreachable(reason: "Could not connect to the server.")
            })
        ))

        await registry.refreshDiagnostics(appConfiguration: appConfiguration)

        let report = try #require(registry.troubleshootingReport(for: provider.id))
        #expect(report.status == .blocked)
        #expect(report.failureLayer == .gooseReachability)
        #expect(report.headline.contains("cannot reach Goose server"))
        #expect(report.evidence.contains {
            $0.label == "Goose server reachability"
                && $0.state == .blocked
                && $0.value.contains("Could not connect to the server.")
        })
    }

    @Test("Goose assistant probe transitions to verified after clean diagnostics")
    mutating func gooseAssistantProbeTransitionsToVerified() async throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

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
        let provider = ConfiguredProvider(
            family: .codex,
            displayName: "Codex Goose",
            transport: .gooseServer,
            endpoint: "https://127.0.0.1:51200",
            authMode: .none,
            defaultModel: "gpt-5-codex"
        )
        let providerStore = retain(ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [provider],
                preferredProviderIDsByFamily: [ProviderFamily.codex.rawValue: provider.id],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: true
            )
        ))
        let registry = retain(ProviderRegistry(
            settingsStore: providerStore,
            secretStore: makeTestSecretStore("com.chainworks.tests.goose-assistant-verified"),
            adapters: makeAdapters()
        ))

        let probe = GooseProviderHandshakeProbe(
            providerRegistry: registry,
            appConfigurationStore: appStore
        )

        let configured = try #require(probe.configuredSnapshot(for: provider.id, origin: .providerSettings))
        #expect(configured.journeyState == .configuredUnverified)
        #expect(configured.handshakeSteps.map(\.label).contains("Transport"))
        #expect(configured.handshakeSteps.map(\.label).contains("Handshake Probe"))
        #expect(configured.transport == .gooseServer)
        #expect(configured.providerIdentifier == "codex")

        let verified = try #require(await probe.probe(providerID: provider.id, origin: .providerSettings))
        #expect(verified.journeyState == .verified)
        #expect(verified.report?.status == .healthy)
        #expect(verified.handshakeSteps.contains { $0.label == "Handshake Probe" && $0.state == .passed })
        #expect(verified.availableModels.contains("gpt-5-codex"))
    }

    @Test("Goose assistant probe transitions to failing when Goose endpoint is invalid")
    mutating func gooseAssistantProbeTransitionsToFailing() async throws {
        let tempDirectory = try makeTempDirectory()
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

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
        let provider = ConfiguredProvider(
            family: .claude,
            displayName: "Claude Goose",
            transport: .gooseServer,
            endpoint: nil,
            authMode: .none,
            defaultModel: "claude-sonnet-4"
        )
        let providerStore = retain(ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [provider],
                preferredProviderIDsByFamily: [ProviderFamily.claude.rawValue: provider.id],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: true
            )
        ))
        let registry = retain(ProviderRegistry(
            settingsStore: providerStore,
            secretStore: makeTestSecretStore("com.chainworks.tests.goose-assistant-failing")
        ))

        let probe = GooseProviderHandshakeProbe(
            providerRegistry: registry,
            appConfigurationStore: appStore
        )

        let failing = try #require(await probe.probe(providerID: provider.id, origin: .firstRunWizard))
        #expect(failing.journeyState == .failing)
        #expect(failing.report?.failureLayer == .gooseEndpoint)
        #expect(failing.handshakeSteps.contains { $0.label == "Endpoint" && $0.state == .failed })
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

    @Test("Preflight fails when Goose server is unreachable")
    mutating func preflightFailsWhenGooseServerIsUnreachable() async throws {
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
            family: .codex,
            displayName: "Codex Goose",
            transport: .gooseServer,
            endpoint: "https://127.0.0.1:51200",
            authMode: .none,
            defaultModel: "gpt-5-codex"
        )
        let providerStore = retain(ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [provider],
                preferredProviderIDsByFamily: [ProviderFamily.codex.rawValue: provider.id],
                notificationOnProviderFailure: true,
                runStartRequiresCleanPreflight: true
            )
        ))
        let registry = retain(ProviderRegistry(
            settingsStore: providerStore,
            secretStore: makeTestSecretStore("com.chainworks.tests.goose-preflight"),
            adapters: makeAdapters(gooseProbe: { _ in
                .unreachable(reason: "Could not connect to the server.")
            })
        ))
        let preflight = PreflightService(appConfigurationStore: appStore, providerRegistry: registry)

        let report = await preflight.runReport(
            workflowURL: URL(fileURLWithPath: configuration.workflowSourcePath),
            catalogURL: URL(fileURLWithPath: configuration.agentCatalogSourcePath),
            plan: makePlan(provider: "codex")
        )

        #expect(report.status == .fail)
        #expect(report.checks.contains {
            $0.title == "Codex Goose Reachability"
                && $0.status == .fail
                && $0.message.contains("Could not connect to the server.")
        })
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
