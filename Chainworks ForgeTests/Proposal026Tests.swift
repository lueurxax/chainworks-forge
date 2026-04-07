import Testing
import Foundation
import SwiftData
@testable import Chainworks_Forge

@MainActor
@Suite("Proposal026", .serialized)
struct Proposal026Tests {

    // MARK: - Test 1: Runtime profile resolution

    @Test("BackendProfileResolverV2 resolves ACP runtime profiles from catalog")
    func runtimeProfileResolutionFromCatalog() throws {
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("p026-test-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempDir) }

        let store = ProviderSettingsStore(
            fileURL: tempDir.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [
                    ConfiguredProvider(
                        family: .claude,
                        displayName: "Claude Test",
                        transport: .gooseServer,
                        authMode: .apiKey,
                        defaultModel: "sonnet"
                    )
                ],
                preferredProviderIDsByFamily: [:],
                notificationOnProviderFailure: false,
                runStartRequiresCleanPreflight: false
            )
        )
        let registry = ProviderRegistry(
            settingsStore: store,
            secretStore: KeychainSecretStore(serviceName: "com.chainworks.tests.p026-resolve", useInMemoryStore: true)
        )
        let resolver = BackendProfileResolverV2(providerRegistry: registry)

        let claudeAgent = ResolvedAgent(
            id: "acp_writer",
            title: "ACP Writer",
            mode: "tool_use",
            backendProfileID: "claude_acp_profile",
            provider: "claude_code",
            model: "sonnet",
            effort: "high",
            maxTurns: 10,
            temperature: 0.0,
            permissionProfile: "ORCH",
            skillRef: "sk1",
            skillRole: nil,
            prompt: "Write code",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["output"],
            runtimeProfileID: "claude_agent_acp"
        )

        let plan = RunPlan(
            workflowID: "p026_test",
            workflowTitle: "P026 Test",
            states: [:],
            initialStateID: "state_1",
            agentBindings: [claudeAgent.id: claudeAgent],
            variables: [:],
            scoring: nil,
            failurePolicy: nil,
            workflowSnapshotHash: "wf",
            catalogSnapshotHash: "cat",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: RunPlan.currentCompilerVersion
        )

        let runtimeProfiles: [String: RuntimeProfile] = [
            "claude_agent_acp": RuntimeProfile(
                capabilityClass: .operatorGrade,
                adapterFamily: "claude_agent_acp",
                requires: ["streaming", "tools"],
                transportKind: "acp_stdio",
                mcpRealizationPath: "acp_native"
            ),
            "gemini_cli_acp": RuntimeProfile(
                capabilityClass: .operatorGrade,
                adapterFamily: "gemini_cli_acp",
                requires: ["streaming", "tools"],
                transportKind: "acp_stdio",
                mcpRealizationPath: "acp_native"
            )
        ]

        let bindings = try resolver.resolveBindings(
            plan: plan,
            startOptions: .empty,
            runtimeProfiles: runtimeProfiles
        )

        let binding = try #require(bindings["acp_writer"])
        #expect(binding.runtimeProfileID == "claude_agent_acp")
        #expect(binding.adapterFamily == "claude_agent_acp")
        #expect(binding.capabilityClass == .operatorGrade)
    }

    // MARK: - Test 2: ResolvedProviderBinding round-trip encoding

    @Test("ResolvedProviderBinding with ACP fields survives JSON round-trip")
    func resolvedProviderBindingRoundTrip() throws {
        let original = ResolvedProviderBinding(
            agentID: "acp_agent",
            backendProfileID: "acp_profile",
            configuredProviderID: UUID(),
            providerFamily: "claude",
            providerIdentifier: "claude_code",
            model: "sonnet",
            effort: "high",
            transport: "goose_server",
            adapterVersion: "v1",
            runtimeProfileID: "claude_agent_acp",
            adapterFamily: "claude_agent_acp",
            capabilityClass: .operatorGrade
        )

        let data = try JSONEncoder().encode(original)
        let decoded = try JSONDecoder().decode(ResolvedProviderBinding.self, from: data)

        #expect(decoded.runtimeProfileID == "claude_agent_acp")
        #expect(decoded.adapterFamily == "claude_agent_acp")
        #expect(decoded.capabilityClass == .operatorGrade)
        #expect(decoded.agentID == original.agentID)
        #expect(decoded.model == original.model)
        #expect(decoded == original)
    }

    // MARK: - Test 3: ACP transport types conform to RuntimeTransportProtocol

    @Test("ACP transport classes conform to RuntimeTransportProtocol")
    func acpTransportConformance() {
        let claudeTransport = ClaudeAgentACPTransport()
        let geminiTransport = GeminiCLIACPTransport()

        #expect(claudeTransport is RuntimeTransportProtocol)
        #expect(geminiTransport is RuntimeTransportProtocol)

        // Verify protocol-level properties
        #expect(claudeTransport.mcpRuntimeNamespace == "claude_agent")
        #expect(geminiTransport.mcpRuntimeNamespace == "gemini_cli")
    }

    // MARK: - Test 4: ACPStreamEventMapper maps ACP events correctly

    @Test("ACPStreamEventMapper maps canonical ACP event taxonomy")
    func acpStreamEventMapperEventTaxonomy() {
        // agent_message_chunk -> .textChunk
        let messageChunk = ACPStreamEventMapper.mapSessionUpdate([
            "type": "agent_message_chunk",
            "content": "Hello world"
        ])
        if case .textChunk(let text) = messageChunk {
            #expect(text == "Hello world")
        } else {
            Issue.record("Expected .textChunk, got \(String(describing: messageChunk))")
        }

        // tool_call with pending status -> .toolCallStarted
        let toolCallPending = ACPStreamEventMapper.mapSessionUpdate([
            "type": "tool_call",
            "status": "pending",
            "name": "read_file"
        ])
        if case .toolCallStarted(let toolName, _) = toolCallPending {
            #expect(toolName == "read_file")
        } else {
            Issue.record("Expected .toolCallStarted, got \(String(describing: toolCallPending))")
        }

        // tool_call_update with completed status -> .toolCallFinished
        let toolCallCompleted = ACPStreamEventMapper.mapSessionUpdate([
            "type": "tool_call_update",
            "status": "completed",
            "name": "write_file"
        ])
        if case .toolCallFinished(let toolName, _) = toolCallCompleted {
            #expect(toolName == "write_file")
        } else {
            Issue.record("Expected .toolCallFinished, got \(String(describing: toolCallCompleted))")
        }

        // agent_thought_chunk -> .textChunk with [thinking] prefix
        let thoughtChunk = ACPStreamEventMapper.mapSessionUpdate([
            "type": "agent_thought_chunk",
            "content": "Analyzing the problem"
        ])
        if case .textChunk(let text) = thoughtChunk {
            #expect(text == "[thinking] Analyzing the problem")
        } else {
            Issue.record("Expected .textChunk with thinking prefix, got \(String(describing: thoughtChunk))")
        }

        // error notification -> .error
        let errorEvent = ACPStreamEventMapper.mapNotification(
            method: "session/error",
            params: ["message": "Connection lost"]
        )
        if case .error(let message) = errorEvent {
            #expect(message == "Connection lost")
        } else {
            Issue.record("Expected .error, got \(String(describing: errorEvent))")
        }
    }

    // MARK: - Test 5: ACP transports can be instantiated

    @Test("ClaudeAgentACPTransport and GeminiCLIACPTransport instantiate cleanly")
    func acpTransportInstantiation() {
        let claude = ClaudeAgentACPTransport(executablePath: "/usr/bin/false")
        let gemini = GeminiCLIACPTransport(executablePath: "/usr/bin/false")

        #expect(claude.executablePath == "/usr/bin/false")
        #expect(gemini.executablePath == "/usr/bin/false")

        // Verify they are valid protocol witnesses
        let claudeProtocol: any RuntimeTransportProtocol = claude
        let geminiProtocol: any RuntimeTransportProtocol = gemini
        #expect(claudeProtocol.mcpRuntimeNamespace == "claude_agent")
        #expect(geminiProtocol.mcpRuntimeNamespace == "gemini_cli")
    }

    // MARK: - Test 6: Goose default path when runtimeProfileID is nil

    @Test("Resolver defaults to goose adapter family when runtimeProfileID is nil")
    func gooseDefaultPathWhenNoRuntimeProfile() throws {
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("p026-goose-default-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempDir) }

        let store = ProviderSettingsStore(
            fileURL: tempDir.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [
                    ConfiguredProvider(
                        family: .claude,
                        displayName: "Claude Default",
                        transport: .gooseServer,
                        authMode: .apiKey,
                        defaultModel: "sonnet"
                    )
                ],
                preferredProviderIDsByFamily: [:],
                notificationOnProviderFailure: false,
                runStartRequiresCleanPreflight: false
            )
        )
        let registry = ProviderRegistry(
            settingsStore: store,
            secretStore: KeychainSecretStore(serviceName: "com.chainworks.tests.p026-goose-default", useInMemoryStore: true)
        )
        let resolver = BackendProfileResolverV2(providerRegistry: registry)

        let agent = ResolvedAgent(
            id: "legacy_agent",
            title: "Legacy Agent",
            mode: "tool_use",
            provider: "claude_code",
            model: "sonnet",
            effort: "medium",
            maxTurns: 8,
            temperature: 0.0,
            permissionProfile: "ORCH",
            skillRef: "sk1",
            skillRole: nil,
            prompt: "Do work",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["output"]
            // runtimeProfileID omitted -- nil by default
        )

        let plan = RunPlan(
            workflowID: "p026_goose_test",
            workflowTitle: "P026 Goose Default",
            states: [:],
            initialStateID: "state_1",
            agentBindings: [agent.id: agent],
            variables: [:],
            scoring: nil,
            failurePolicy: nil,
            workflowSnapshotHash: "wf",
            catalogSnapshotHash: "cat",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: RunPlan.currentCompilerVersion
        )

        let bindings = try resolver.resolveBindings(
            plan: plan,
            startOptions: .empty,
            runtimeProfiles: [:]
        )

        let binding = try #require(bindings["legacy_agent"])
        #expect(binding.adapterFamily == "goose")
        #expect(binding.capabilityClass == .legacyOperatorGrade)
    }

    // MARK: - Test 7: AgentExecution persists runtime settlement fields

    @Test("AgentExecution stores and retrieves runtime settlement fields")
    func agentExecutionRuntimeSettlement() throws {
        let context = try makeTestModelContext()

        let agent = AgentExecution(
            agentID: "acp_test_agent",
            agentTitle: "ACP Test Agent",
            taskName: "test_task",
            status: .completed,
            provider: "claude_code",
            effort: "high"
        )
        agent.runtimeProvider = "claude_agent_acp"
        agent.runtimeModel = "sonnet"
        context.insert(agent)

        #expect(agent.runtimeProvider == "claude_agent_acp")
        #expect(agent.runtimeModel == "sonnet")
        #expect(agent.runtimeSessionID == nil)

        agent.runtimeSessionID = "session-abc-123"
        #expect(agent.runtimeSessionID == "session-abc-123")
        #expect(agent.gooseSessionID == "session-abc-123")
    }
}
