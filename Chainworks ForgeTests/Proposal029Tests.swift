import Testing
import Foundation
@testable import Chainworks_Forge

@MainActor
@Suite("Proposal029", .serialized)
struct Proposal029Tests {

    // MARK: - Test 1: Fail-closed factory throws for unknown adapter family

    @Test("Transport factory throws unknownAdapterFamily for unregistered family")
    func failClosedFactoryThrowsForUnknownFamily() throws {
        let factory = DefaultRuntimeTransportFactory(gooseTransport: nil)
        let agent = ResolvedAgent(
            id: "test_agent",
            title: "Test Agent",
            mode: "tool_use",
            provider: "unknown_provider",
            model: "test",
            effort: "medium",
            maxTurns: 5,
            temperature: 0.0,
            permissionProfile: "ORCH",
            skillRef: "sk1",
            skillRole: nil,
            prompt: "Test prompt",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["output"]
        )
        let binding = ResolvedProviderBinding(
            agentID: "test_agent",
            backendProfileID: nil,
            configuredProviderID: UUID(),
            providerFamily: "unknown",
            providerIdentifier: "unknown",
            model: "test",
            effort: "medium",
            transport: "acp_stdio",
            adapterVersion: "v1",
            runtimeProfileID: "unknown_profile",
            adapterFamily: "unknown_family",
            capabilityClass: .controlCapable
        )
        #expect(throws: RuntimeTransportError.self) {
            _ = try factory.transport(for: agent, binding: binding)
        }
    }

    // MARK: - Test 2: Factory still works for registered families

    @Test("Transport factory resolves registered ACP families without throwing")
    func factoryResolvesRegisteredFamilies() throws {
        let factory = DefaultRuntimeTransportFactory(gooseTransport: nil)
        let agent = ResolvedAgent(
            id: "test_agent",
            title: "Test Agent",
            mode: "tool_use",
            provider: "acp_provider",
            model: "test",
            effort: "medium",
            maxTurns: 5,
            temperature: 0.0,
            permissionProfile: "ORCH",
            skillRef: "sk1",
            skillRole: nil,
            prompt: "Test prompt",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["output"]
        )
        for family in ["claude_agent_acp", "gemini_cli_acp", "codex_acp", "auggie_cli_acp", "junie_cli_acp"] {
            let binding = ResolvedProviderBinding(
                agentID: "test_agent", backendProfileID: nil, configuredProviderID: UUID(),
                providerFamily: family, providerIdentifier: family, model: "test", effort: "medium",
                transport: "acp_stdio", adapterVersion: "v1",
                adapterFamily: family, capabilityClass: .controlCapable
            )
            let transport = try factory.transport(for: agent, binding: binding)
            #expect(transport.mcpRuntimeNamespace != nil)
        }
    }

    // MARK: - Test 3: New provider families exist

    @Test("ProviderFamily includes second-wave ACP families")
    func providerFamilyIncludesSecondWave() {
        let families = ProviderFamily.allCases.map(\.rawValue)
        #expect(families.contains("codexACP"))
        #expect(families.contains("auggie"))
        #expect(families.contains("junie"))
    }

    // MARK: - Test 4: New ACP transports have correct namespaces

    @Test("Second-wave ACP transports declare correct runtime namespaces")
    func secondWaveNamespaces() {
        #expect(CodexACPTransport().mcpRuntimeNamespace == "codex")
        #expect(AuggieCLIACPTransport().mcpRuntimeNamespace == "auggie")
        #expect(JunieCLIACPTransport().mcpRuntimeNamespace == "junie")
    }

    // MARK: - Test 5: effectiveRuntimeNamespace includes second-wave families

    @Test("ResolvedProviderBinding namespace resolves for second-wave adapters")
    func namespaceResolvesForSecondWave() {
        for (family, expected) in [("codex_acp", "codex"), ("auggie_cli_acp", "auggie"), ("junie_cli_acp", "junie")] {
            let binding = ResolvedProviderBinding(
                agentID: "test", backendProfileID: nil, configuredProviderID: UUID(),
                providerFamily: family, providerIdentifier: family, model: "test", effort: "medium",
                transport: "acp_stdio", adapterVersion: "v1",
                adapterFamily: family
            )
            #expect(binding.effectiveRuntimeNamespace == expected)
        }
    }

    // MARK: - Test 6: ProviderCapabilities.satisfies maps tokens correctly

    @Test("ProviderCapabilities.satisfies maps requires tokens to capability fields")
    func capabilitiesSatisfiesMapping() {
        let caps = ProviderCapabilities.default(for: .codexACP)
        #expect(caps.satisfies("streaming") == true)
        #expect(caps.satisfies("tools") == true)
        #expect(caps.satisfies("session_resume") == true)  // codexACP supports this
        #expect(caps.satisfies("structured_output") == false)
        #expect(caps.satisfies("nonexistent_token") == false)

        let auggieCaps = ProviderCapabilities.default(for: .auggie)
        #expect(auggieCaps.satisfies("session_resume") == false)
    }

    // MARK: - Test 7: preferredProvider filters disabled providers

    @Test("Disabled providers are not returned by preferredProvider")
    func disabledProviderFiltering() throws {
        // Create a provider settings store with a disabled provider
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("p029-test-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempDir) }

        let disabledProvider = ConfiguredProvider(
            family: .codexACP,
            displayName: "Codex ACP",
            transport: .gooseServer,
            authMode: .apiKey,
            defaultModel: "gpt-5",
            isEnabled: false
        )
        let store = ProviderSettingsStore(
            fileURL: tempDir.appendingPathComponent("settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [disabledProvider],
                preferredProviderIDsByFamily: [:],
                notificationOnProviderFailure: false,
                runStartRequiresCleanPreflight: false
            )
        )
        let registry = ProviderRegistry(
            settingsStore: store,
            secretStore: KeychainSecretStore(serviceName: "com.chainworks.tests.p029-disabled", useInMemoryStore: true)
        )

        let resolved = registry.preferredProvider(for: .codexACP)
        #expect(resolved == nil)
    }

    @Test("ProviderSettingsStore migrates empty persisted stores to seeded defaults")
    func emptyPersistedStoreReseedsDefaults() throws {
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("p029-empty-store-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempDir) }

        let fileURL = tempDir.appendingPathComponent("settings.json")
        try JSONEncoder().encode(ProviderSettings.empty).write(to: fileURL)

        let store = ProviderSettingsStore(fileURL: fileURL)
        let families = Set(store.settings.configuredProviders.map(\.family))

        #expect(families.contains(.codex))
        #expect(families.contains(.claude))
        #expect(families.contains(.gemini))
        #expect(!store.settings.configuredProviders.isEmpty)
    }

    // MARK: - Test 9: CodexACPTransport session creation fails with subprocess error, not stub error

    @Test("CodexACPTransport session creation fails with subprocess error, not stub error")
    func codexTransportIsNotStubbed() async {
        let transport = CodexACPTransport(executablePath: "/nonexistent/codex-acp")
        do {
            _ = try await transport.createSession(request: RuntimeSessionRequest(
                systemPrompt: "test",
                workingDirectory: nil,
                model: "gpt-5",
                provider: nil,
                executionPolicy: nil,
                metadata: nil,
                requestedExtensions: nil
            ))
            Issue.record("Expected error")
        } catch {
            let message = error.localizedDescription
            // Should NOT contain "not yet implemented"
            #expect(!message.contains("not yet implemented"), "Transport is still stubbed: \(message)")
        }
    }

    @Test("CodexACPTransport prepares isolated CODEX_HOME with auth only")
    func codexTransportPreparesIsolatedRuntimeHome() throws {
        let fileManager = FileManager.default
        let tempRoot = fileManager.temporaryDirectory
            .appendingPathComponent("p029-codex-home-\(UUID().uuidString)", isDirectory: true)
        try fileManager.createDirectory(at: tempRoot, withIntermediateDirectories: true)
        defer { try? fileManager.removeItem(at: tempRoot) }

        let sourceHome = tempRoot.appendingPathComponent("source-home", isDirectory: true)
        try fileManager.createDirectory(at: sourceHome, withIntermediateDirectories: true)
        let authURL = sourceHome.appendingPathComponent("auth.json", isDirectory: false)
        let configURL = sourceHome.appendingPathComponent("config.toml", isDirectory: false)
        let stateURL = sourceHome.appendingPathComponent("state_5.sqlite", isDirectory: false)
        try Data("{\"token\":\"abc\"}".utf8).write(to: authURL)
        try Data("[profiles]\n".utf8).write(to: configURL)
        try Data("sqlite".utf8).write(to: stateURL)

        let runtimeHome = try CodexACPTransport.prepareRuntimeHome(
            workingDirectory: "/tmp/work",
            fileManager: fileManager,
            environment: ["CODEX_HOME": sourceHome.path],
            tempRootURL: tempRoot
        )
        defer { CodexACPTransport.cleanupRuntimeHomeIfPresent(runtimeHome, fileManager: fileManager) }

        #expect(fileManager.fileExists(atPath: runtimeHome.appendingPathComponent("auth.json").path))
        #expect(!fileManager.fileExists(atPath: runtimeHome.appendingPathComponent("config.toml").path))
        #expect(!fileManager.fileExists(atPath: runtimeHome.appendingPathComponent("state_5.sqlite").path))
    }

    @Test("ACPSubprocessManager sendJSON throws instead of crashing when runtime closes stdin")
    func acpSubprocessManagerHandlesClosedStdinGracefully() throws {
        let manager = ACPSubprocessManager(
            executablePath: "/usr/bin/python3",
            arguments: ["-c", "import os,time; os.close(0); time.sleep(1)"]
        )
        try manager.launch()
        defer { manager.terminate() }
        Thread.sleep(forTimeInterval: 0.1)

        do {
            try manager.sendJSON([
                "jsonrpc": "2.0",
                "id": 1,
                "method": "ping"
            ])
            Issue.record("Expected sendJSON to fail once runtime stdin is closed")
        } catch let error as ACPSubprocessError {
            switch error {
            case .brokenPipe, .notRunning:
                break
            default:
                Issue.record("Unexpected ACPSubprocessError: \(error.localizedDescription)")
            }
        } catch {
            Issue.record("Unexpected error type: \(error)")
        }
    }

    // MARK: - Test 10: AuggieCLIACPTransport session creation fails with subprocess error, not stub error

    @Test("AuggieCLIACPTransport session creation fails with subprocess error, not stub error")
    func auggieTransportIsNotStubbed() async {
        let transport = AuggieCLIACPTransport(executablePath: "/nonexistent/auggie")
        do {
            _ = try await transport.createSession(request: RuntimeSessionRequest(
                systemPrompt: "test",
                workingDirectory: nil,
                model: "default",
                provider: nil,
                executionPolicy: nil,
                metadata: nil,
                requestedExtensions: nil
            ))
            Issue.record("Expected error")
        } catch {
            let message = error.localizedDescription
            // Should NOT contain "not yet implemented"
            #expect(!message.contains("not yet implemented"), "Transport is still stubbed: \(message)")
        }
    }

    // MARK: - Test 11: JunieCLIACPTransport session creation fails with subprocess error, not stub error

    @Test("JunieCLIACPTransport session creation fails with subprocess error, not stub error")
    func junieTransportIsNotStubbed() async {
        let transport = JunieCLIACPTransport(executablePath: "/nonexistent/junie")
        do {
            _ = try await transport.createSession(request: RuntimeSessionRequest(
                systemPrompt: "test",
                workingDirectory: nil,
                model: "default",
                provider: nil,
                executionPolicy: nil,
                metadata: nil,
                requestedExtensions: nil
            ))
            Issue.record("Expected error")
        } catch {
            let message = error.localizedDescription
            // Should NOT contain "not yet implemented"
            #expect(!message.contains("not yet implemented"), "Transport is still stubbed: \(message)")
        }
    }

    @Test("Example catalog maps codex runtime namespace for rich MCP servers")
    func exampleCatalogMapsCodexRuntimeNamespaceForRichMCPServers() throws {
        let repoRoot = URL(fileURLWithPath: "/Users/user/Documents/Chainworks Forge", isDirectory: true)
        let catalogURL = repoRoot.appendingPathComponent("examples/agents/agents.yaml")
        let catalog = try YAMLParser.loadAgentCatalog(from: catalogURL)

        for serverID in ["developer", "analyze", "xcode", "context7"] {
            let runtimeID = catalog.mcpServerRegistry[serverID]?.runtimeIDs["codex"]
            #expect(runtimeID == serverID)
        }
    }
}
