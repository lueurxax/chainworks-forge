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
}
