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
                        family: .claudeACP,
                        displayName: "Claude Test",
                        transport: .cli,
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
        #expect(binding.transport == "acp_stdio")
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
            transport: "acp_stdio",
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

    @Test("ExecutionEventBridge reuses tool name for in-progress ACP updates by toolCallId")
    func executionEventBridgeReusesToolNameForInProgressUpdate() async throws {
        let bridge = ExecutionEventBridge()
        let stream = AsyncThrowingStream<RuntimeStreamEvent, Error> { continuation in
            continuation.yield(.toolCallStarted(
                toolName: "search",
                raw: #"{"toolCallId":"call_123","name":"search"}"#
            ))
            continuation.yield(.toolCallStarted(
                toolName: "unknown",
                raw: #"{"toolCallId":"call_123","content":"progress"}"#
            ))
            continuation.finish()
        }

        let collector = SharedEventCollector()
        _ = try await bridge.processStream(stream) { event in
            collector.append(event)
        }

        let toolEvents = collector.events.filter { $0.type == .toolCallStarted }
        #expect(toolEvents.count == 2)
        #expect(toolEvents.last?.toolName == "search")
        #expect(toolEvents.last?.detail == "Tool: search")
    }

    @Test("ExecutionEventBridge marks failed ACP tool updates as unsuccessful tool calls")
    func executionEventBridgeMarksFailedToolUpdateUnsuccessful() async throws {
        let bridge = ExecutionEventBridge()
        let stream = AsyncThrowingStream<RuntimeStreamEvent, Error> { continuation in
            continuation.yield(.toolCallStarted(
                toolName: "get_lazy_artifact",
                raw: #"{"toolCallId":"call_123","name":"get_lazy_artifact"}"#
            ))
            continuation.yield(.toolCallFinished(
                toolName: "get_lazy_artifact",
                raw: #"{"toolCallId":"call_123","name":"get_lazy_artifact","status":"failed","rawOutput":{"stdout":"lazy artifact not found: proposal_review_architect_json\n"}}"#
            ))
            continuation.finish()
        }

        _ = try await bridge.processStream(stream) { _ in }

        #expect(bridge.toolCalls.count == 1)
        #expect(bridge.toolCalls[0].toolName == "get_lazy_artifact")
        #expect(bridge.toolCalls[0].succeeded == false)
    }

    @Test("ExecutionEventBridge meaningful progress stays nil for thinking chunks and weak discovery tools")
    func executionEventBridgeMeaningfulProgressIgnoresWeakActivity() async throws {
        let bridge = ExecutionEventBridge()
        let stream = AsyncThrowingStream<RuntimeStreamEvent, Error> { continuation in
            continuation.yield(.textChunk(text: "[thinking] analyzing"))
            continuation.yield(.toolCallStarted(
                toolName: "search",
                raw: #"{"toolCallId":"call_weak","name":"search"}"#
            ))
            continuation.yield(.toolCallFinished(
                toolName: "search",
                raw: #"{"toolCallId":"call_weak","name":"search","status":"completed"}"#
            ))
            continuation.finish()
        }

        _ = try await bridge.processStream(stream) { _ in }

        #expect(bridge.lastMeaningfulProgressAt == nil)
        #expect(bridge.toolCalls.count == 1)
    }

    @Test("ExecutionEventBridge meaningful progress records strong tool activity")
    func executionEventBridgeMeaningfulProgressRecordsStrongToolActivity() async throws {
        let bridge = ExecutionEventBridge()
        let stream = AsyncThrowingStream<RuntimeStreamEvent, Error> { continuation in
            continuation.yield(.toolCallStarted(
                toolName: "review",
                raw: #"{"toolCallId":"call_strong","name":"review"}"#
            ))
            continuation.finish()
        }

        _ = try await bridge.processStream(stream) { _ in }

        #expect(bridge.lastMeaningfulProgressAt != nil)
        #expect(bridge.toolCalls.count == 1)
    }

    // MARK: - Test 3: Default path when runtimeProfileID is nil

    @Test("Resolver defaults to claude_agent_acp adapter family when runtimeProfileID is nil")
    func defaultPathWhenNoRuntimeProfile() throws {
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("p026-default-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempDir) }

        let store = ProviderSettingsStore(
            fileURL: tempDir.appendingPathComponent("provider-settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [
                    ConfiguredProvider(
                        family: .claudeACP,
                        displayName: "Claude Default",
                        transport: .cli,
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
            secretStore: KeychainSecretStore(serviceName: "com.chainworks.tests.p026-default", useInMemoryStore: true)
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
        )

        let plan = RunPlan(
            workflowID: "p026_default_test",
            workflowTitle: "P026 Default",
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
        #expect(binding.adapterFamily == "claude_agent_acp")
        #expect(binding.capabilityClass == .legacyOperatorGrade)
    }

    // MARK: - Test 4: AgentExecution persists runtime settlement fields

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
        #expect(agent.runtimeSessionID == "session-abc-123")
    }

    @Test("Canonical catalog routes all Claude backends through Claude Agent ACP and limits Claude MCP to ACP-materializable lanes")
    func canonicalCatalogClaudeACPIntent() throws {
        let catalog = try loadTestCanonicalCatalog()

        let claudeBackendProfiles = catalog.backendProfiles.filter {
            ProviderFamily.from(runtimeIdentifier: $0.value.provider) == .claudeACP
        }
        #expect(!claudeBackendProfiles.isEmpty)
        #expect(claudeBackendProfiles.values.allSatisfy { $0.runtimeProfile == "claude_agent_acp" })

        let claudeBackendIDs = Set(claudeBackendProfiles.keys)
        let supportedClaudeMCP = Set(["xcode", "context7"])

        for (backendID, profile) in claudeBackendProfiles where claudeBackendIDs.contains(backendID) {
            #expect(Set(profile.mcp).isSubset(of: supportedClaudeMCP))
        }
    }
}
