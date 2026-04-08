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

    // MARK: - Test 3: ACP transport types conform to RuntimeTransportProtocol

    @Test("ACP transport classes conform to RuntimeTransportProtocol")
    func acpTransportConformance() {
        let claudeTransport = ClaudeAgentACPTransport()
        let geminiTransport = GeminiCLIACPTransport()

        #expect(claudeTransport is RuntimeTransportProtocol)
        #expect(geminiTransport is RuntimeTransportProtocol)
        #expect(claudeTransport.mcpRuntimeNamespace == "claude_agent")
        #expect(geminiTransport.mcpRuntimeNamespace == "gemini_cli")
    }

    // MARK: - Test 4: ACPStreamEventMapper maps ACP events correctly

    @Test("ACPStreamEventMapper maps canonical ACP event taxonomy")
    func acpStreamEventMapperEventTaxonomy() {
        let messageChunk = ACPStreamEventMapper.mapSessionUpdate([
            "type": "agent_message_chunk",
            "content": "Hello world"
        ])
        if case .textChunk(let text) = messageChunk {
            #expect(text == "Hello world")
        } else {
            Issue.record("Expected .textChunk, got \(String(describing: messageChunk))")
        }

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

        let thoughtChunk = ACPStreamEventMapper.mapSessionUpdate([
            "type": "agent_thought_chunk",
            "content": "Analyzing the problem"
        ])
        if case .textChunk(let text) = thoughtChunk {
            #expect(text == "[thinking] Analyzing the problem")
        } else {
            Issue.record("Expected .textChunk with thinking prefix, got \(String(describing: thoughtChunk))")
        }

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

    // MARK: - Test 8: Executed canonical ACP proposal loop proof

    @Test("Claude Agent ACP-backed canonical proposal loop completes without downgrading runtime truth")
    func claudeACPBackedCanonicalProposalLoopProof() async throws {
        try await assertCanonicalProposalLoopProof(
            runtimeProfileID: "claude_agent_acp",
            transportFactory: { ClaudeAgentACPTransport(executablePath: $0) }
        )
    }

    @Test("Canonical catalog routes all Claude backends through Claude Agent ACP and limits Claude MCP to ACP-materializable lanes")
    func canonicalCatalogClaudeACPIntent() throws {
        let catalog = try loadTestCanonicalCatalog()

        let claudeBackendProfiles = catalog.backendProfiles.filter { $0.value.provider == "claude_code" }
        #expect(!claudeBackendProfiles.isEmpty)
        #expect(claudeBackendProfiles.values.allSatisfy { $0.runtimeProfile == "claude_agent_acp" })

        let claudeBackendIDs = Set(claudeBackendProfiles.keys)
        let supportedClaudeMCP = Set(["xcode", "context7"])

        for agent in catalog.agents where claudeBackendIDs.contains(agent.backendProfile) {
            let profileID = agent.mcpProfile ?? catalog.mcpPolicy.defaultProfile
            guard let profile = catalog.mcpProfiles[profileID] else {
                continue
            }
            #expect(Set(profile.allRequestedExtensions).isSubset(of: supportedClaudeMCP))
        }
    }

    @Test("Gemini CLI ACP-backed canonical proposal loop completes without downgrading runtime truth")
    func geminiACPBackedCanonicalProposalLoopProof() async throws {
        try await assertCanonicalProposalLoopProof(
            runtimeProfileID: "gemini_cli_acp",
            transportFactory: { GeminiCLIACPTransport(executablePath: $0) }
        )
    }

    // MARK: - Test 9: Executed canonical ACP implementation proof

    @Test("Claude Agent ACP-backed implementation path reaches manual release gate without downgrading runtime truth")
    func claudeACPBackedImplementationPathProof() async throws {
        try await assertImplementationPathProof(
            runtimeProfileID: "claude_agent_acp",
            transportFactory: { ClaudeAgentACPTransport(executablePath: $0) }
        )
    }

    @Test("Gemini CLI ACP-backed implementation path reaches manual release gate without downgrading runtime truth")
    func geminiACPBackedImplementationPathProof() async throws {
        try await assertImplementationPathProof(
            runtimeProfileID: "gemini_cli_acp",
            transportFactory: { GeminiCLIACPTransport(executablePath: $0) }
        )
    }

    // MARK: - Helpers

    private func assertCanonicalProposalLoopProof(
        runtimeProfileID: String,
        transportFactory: (String) -> any RuntimeTransportProtocol
    ) async throws {
        let (container, context) = try makeTestModelContainer()
        _ = container
        let compiler = RunPlanCompiler(modelContext: context)
        let workflow = try loadTestLiveWorkflow()
        let catalog = try loadTestCanonicalCatalog()
        let basePlan = try compiler.previewCompile(workflow: workflow, catalog: catalog)
        let plan = forceACPProfile(on: basePlan, runtimeProfileID: runtimeProfileID)

        let fixture = try makeACPFixture(plan: plan, catalog: catalog)
        defer { try? FileManager.default.removeItem(at: fixture.root) }

        let repoRoot = try makeMinimalRepository(prefix: "P026-ProposalLoop-\(runtimeProfileID)")
        defer { try? FileManager.default.removeItem(at: repoRoot) }

        let idea = Idea(title: "P026 Proposal Loop", body: "Executed ACP-backed proposal loop proof.")
        context.insert(idea)

        let (run, workspace) = try compiler.createRun(
            for: idea,
            plan: plan,
            workflowSourcePath: repositoryRootURL().appendingPathComponent("examples/workflows/proposal-loop-live.yaml").path,
            catalogSourcePath: repositoryRootURL().appendingPathComponent("examples/agents/agents.yaml").path,
            startSnapshot: RunStartSnapshot(
                providerBindingSnapshotJSON: try encodeACPBindings(for: plan, runtimeProfileID: runtimeProfileID),
                bindingProvenanceJSON: nil,
                startOptionsJSON: Data("{}".utf8),
                frozenWorkspaceRootPath: repoRoot.path
            )
        )

        let executor = RuntimeAgentExecutor(
            transport: transportFactory(fixture.executablePath)
        )
        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: executor,
            modelContext: context,
            catalog: catalog
        )

        var approvalRequests: [ApprovalRequest] = []
        orchestrator.onApprovalRequest = { request in
            approvalRequests.append(request)
        }

        await orchestrator.start()

        await awaitCondition("ACP-backed proposal loop should complete", timeout: 30.0) {
            if !approvalRequests.isEmpty {
                let pending = approvalRequests
                approvalRequests.removeAll()
                for request in pending {
                    orchestrator.resolveApproval(stageID: request.stageID, granted: true, comment: "ACP proof auto-approve")
                }
            }
            return run.status == .completed || run.status == .failed || run.status == .blocked
        }

        expectRunCompleted(run)
        let agentExecutions = run.stageExecutions.flatMap(\.agentExecutions)
        #expect(!agentExecutions.isEmpty)
        #expect(agentExecutions.allSatisfy { $0.runtimeProfileID == runtimeProfileID })
        #expect(agentExecutions.allSatisfy { $0.actualAdapterFamily == runtimeProfileID })
        #expect(agentExecutions.allSatisfy { $0.actualCapabilityClass == RuntimeCapabilityClass.operatorGrade.rawValue })

        let report = RunReportBuilder(modelContext: context).buildReportPayload(for: run, version: 1)
        let reportAgents = report.agentsUsed.filter { !$0.agentID.isEmpty }
        #expect(!reportAgents.isEmpty)
        #expect(reportAgents.allSatisfy { $0.runtimeProfileID == runtimeProfileID })
        #expect(reportAgents.allSatisfy { $0.actualAdapterFamily == runtimeProfileID })
        #expect(reportAgents.allSatisfy { $0.actualCapabilityClass == RuntimeCapabilityClass.operatorGrade.rawValue })
        if runtimeProfileID == "gemini_cli_acp" {
            let visualReviewExecutions = agentExecutions.filter { ["proposal_reviewer_ui", "proposal_reviewer_ux"].contains($0.agentID) }
            #expect(!visualReviewExecutions.isEmpty)
            #expect(visualReviewExecutions.allSatisfy { decodeStringArray($0.effectiveMCPRuntimeExtensionIDsJSON).contains("xcode") })
        } else if runtimeProfileID == "claude_agent_acp" {
            let productReviewerExecutions = agentExecutions.filter { $0.agentID == "proposal_reviewer_product_owner" }
            #expect(!productReviewerExecutions.isEmpty)
            #expect(productReviewerExecutions.allSatisfy { decodeStringArray($0.effectiveMCPRuntimeExtensionIDsJSON).contains("xcode") })
        }
    }

    private func assertImplementationPathProof(
        runtimeProfileID: String,
        transportFactory: (String) -> any RuntimeTransportProtocol
    ) async throws {
        let (container, context) = try makeTestModelContainer()
        _ = container
        let compiler = RunPlanCompiler(modelContext: context)
        let workflow = try loadTestFullMVPLiveWorkflow()
        let catalog = try loadTestCanonicalCatalog()
        let basePlan = try compiler.previewCompile(workflow: workflow, catalog: catalog)
        let plan = forceACPProfile(on: basePlan, runtimeProfileID: runtimeProfileID)

        let fixture = try makeACPFixture(plan: plan, catalog: catalog)
        defer { try? FileManager.default.removeItem(at: fixture.root) }

        let repoRoot = try makeMinimalRepository(prefix: "P026-ImplementationPath-\(runtimeProfileID)")
        defer { try? FileManager.default.removeItem(at: repoRoot) }

        let idea = Idea(title: "P026 Implementation Path", body: "Executed ACP-backed implementation proof.")
        context.insert(idea)

        let deliveryConfig = DeliveryConfiguration(
            profileID: "p026_acp_proof",
            profileLabel: "P026 ACP Proof",
            sampleProfileID: nil,
            repoIdentifier: repoRoot.lastPathComponent,
            repoRoot: repoRoot.path,
            baseBranch: "main",
            worktreeBasePath: FileManager.default.temporaryDirectory
                .appendingPathComponent("P026-Worktrees-\(UUID().uuidString)", isDirectory: true).path,
            targetBranch: "p026/acp-proof",
            releaseTargetID: "sandbox_test",
            releaseTargetLabel: "Sandbox",
            releaseMode: .sandbox
        )

        let (run, workspace) = try compiler.createRun(
            for: idea,
            plan: plan,
            workflowSourcePath: repositoryRootURL().appendingPathComponent("examples/workflows/full-mvp-live.yaml").path,
            catalogSourcePath: repositoryRootURL().appendingPathComponent("examples/agents/agents.yaml").path,
            startSnapshot: RunStartSnapshot(
                providerBindingSnapshotJSON: try encodeACPBindings(for: plan, runtimeProfileID: runtimeProfileID),
                bindingProvenanceJSON: nil,
                startOptionsJSON: Data("{}".utf8),
                frozenWorkspaceRootPath: repoRoot.path,
                deliveryConfiguration: deliveryConfig,
                deliveryPreflightJSON: Data("{\"passed\":true}".utf8)
            )
        )

        let executor = RuntimeAgentExecutor(
            transport: transportFactory(fixture.executablePath)
        )
        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: executor,
            modelContext: context,
            catalog: catalog
        )

        var approvalRequests: [ApprovalRequest] = []
        orchestrator.onApprovalRequest = { request in
            approvalRequests.append(request)
        }

        await orchestrator.start()

        await awaitCondition("ACP-backed implementation path should reach manual release", timeout: 45.0) {
            if !approvalRequests.isEmpty {
                let pending = approvalRequests
                approvalRequests.removeAll()
                for request in pending {
                    if request.stageID == "state_11_manual_release" {
                        approvalRequests.append(request)
                        continue
                    }
                    orchestrator.resolveApproval(stageID: request.stageID, granted: true, comment: "ACP implementation proof auto-approve")
                }
            }
            return (run.status == .waitingApproval && run.currentStageID == "state_11_manual_release")
                || run.status == .completed
                || run.status == .failed
                || run.status == .blocked
        }

        #expect(run.status == .waitingApproval, "Implementation proof should stop at manual release gate")
        #expect(run.currentStageID == "state_11_manual_release")
        #expect(run.stageExecutions.contains { $0.stageID == "state_7_implementation_started" && $0.status == .completed })
        #expect(run.stageExecutions.contains { $0.stageID == "state_9_implementation_reviewed" && $0.status == .completed })
        let agentExecutions = run.stageExecutions.flatMap(\.agentExecutions)
        #expect(!agentExecutions.isEmpty)
        #expect(agentExecutions.contains { $0.agentID == "code_writer" })
        #expect(agentExecutions.allSatisfy { $0.runtimeProfileID == runtimeProfileID })
        #expect(agentExecutions.allSatisfy { $0.actualAdapterFamily == runtimeProfileID })
        #expect(agentExecutions.allSatisfy { $0.actualCapabilityClass == RuntimeCapabilityClass.operatorGrade.rawValue })

        let report = RunReportBuilder(modelContext: context).buildReportPayload(for: run, version: 1)
        let reportAgents = report.agentsUsed.filter { !$0.agentID.isEmpty }
        #expect(reportAgents.contains { $0.agentID == "code_writer" })
        #expect(reportAgents.allSatisfy { $0.runtimeProfileID == runtimeProfileID })
        #expect(reportAgents.allSatisfy { $0.actualAdapterFamily == runtimeProfileID })
        #expect(reportAgents.allSatisfy { $0.actualCapabilityClass == RuntimeCapabilityClass.operatorGrade.rawValue })
        if runtimeProfileID == "claude_agent_acp" {
            let securityExecutions = agentExecutions.filter { $0.agentID == "security_checker" }
            let prepushExecutions = agentExecutions.filter { $0.agentID == "prepush_code_reviewer" }
            #expect(!securityExecutions.isEmpty)
            #expect(!prepushExecutions.isEmpty)
            #expect(securityExecutions.allSatisfy { decodeStringArray($0.effectiveMCPRuntimeExtensionIDsJSON).contains("context7") })
            #expect(prepushExecutions.allSatisfy { decodeStringArray($0.effectiveMCPRuntimeExtensionIDsJSON).contains("xcode") })
        }
    }

    private struct ACPFixture {
        let root: URL
        let executablePath: String
    }

    private func repositoryRootURL(file: StaticString = #filePath) -> URL {
        URL(fileURLWithPath: "\(file)")
            .deletingLastPathComponent()
            .deletingLastPathComponent()
    }

    private func forceACPProfile(on plan: RunPlan, runtimeProfileID: String) -> RunPlan {
        let bindings = plan.agentBindings.mapValues { agent in
            ResolvedAgent(
                id: agent.id,
                title: agent.title,
                mode: agent.mode,
                backendProfileID: agent.backendProfileID,
                provider: agent.provider,
                model: agent.model,
                effort: agent.effort,
                maxTurns: agent.maxTurns,
                temperature: agent.temperature,
                permissionProfile: agent.permissionProfile,
                mcpProfileID: agent.mcpProfileID,
                skillRef: agent.skillRef,
                skillRole: agent.skillRole,
                resolvedSkill: agent.resolvedSkill,
                prompt: agent.prompt,
                outputContract: agent.outputContract,
                requiresHumanApproval: agent.requiresHumanApproval,
                inputs: agent.inputs,
                outputs: agent.outputs,
                worktreeWriteEnabled: agent.worktreeWriteEnabled,
                sessionReuseScope: agent.sessionReuseScope,
                sessionFamilyID: agent.sessionFamilyID,
                runtimeProfileID: runtimeProfileID
            )
        }

        return RunPlan(
            workflowID: plan.workflowID,
            workflowTitle: plan.workflowTitle,
            states: plan.states,
            initialStateID: plan.initialStateID,
            agentBindings: bindings,
            variables: plan.variables,
            scoring: plan.scoring,
            failurePolicy: plan.failurePolicy,
            requiresProjectAccess: plan.requiresProjectAccess,
            workflowSnapshotHash: plan.workflowSnapshotHash,
            catalogSnapshotHash: plan.catalogSnapshotHash,
            workflowSnapshotJSON: plan.workflowSnapshotJSON,
            catalogSnapshotJSON: plan.catalogSnapshotJSON,
            planCompilerVersion: plan.planCompilerVersion
        )
    }

    private func encodeACPBindings(for plan: RunPlan, runtimeProfileID: String) throws -> Data {
        let bindings = Dictionary(uniqueKeysWithValues: plan.agentBindings.map { key, agent in
            let providerFamily = ProviderFamily.from(runtimeIdentifier: agent.provider)?.rawValue ?? agent.provider
            return (
                key,
                ResolvedProviderBinding(
                    agentID: agent.id,
                    backendProfileID: agent.backendProfileID,
                    configuredProviderID: UUID(),
                    providerFamily: providerFamily,
                    providerIdentifier: agent.provider,
                    model: agent.model,
                    effort: agent.effort,
                    transport: "acp_stdio",
                    adapterVersion: "proposal-026-proof",
                    runtimeProfileID: runtimeProfileID,
                    adapterFamily: runtimeProfileID,
                    capabilityClass: .operatorGrade
                )
            )
        })
        return try JSONEncoder().encode(bindings)
    }

    private func makeACPFixture(plan: RunPlan, catalog: AgentCatalog) throws -> ACPFixture {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("p026-acp-fixture-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)

        let payloadsURL = root.appendingPathComponent("payloads.json")
        let executableURL = root.appendingPathComponent("fake-claude-agent-acp.py")

        let payloads = buildOutputPayloads(plan: plan, catalog: catalog)
        let payloadData = try JSONEncoder().encode(payloads)
        try payloadData.write(to: payloadsURL, options: .atomic)

        let payloadPath = payloadsURL.path
            .replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\"", with: "\\\"")

        let script = """
        #!/usr/bin/env python3
        import json
        import pathlib
        import sys
        import uuid

        PAYLOADS = json.loads(pathlib.Path("\(payloadPath)").read_text(encoding="utf-8"))
        SESSION_ID = "acp-proof-" + uuid.uuid4().hex[:12]

        def extract_expected_outputs(prompt_items):
            texts = []
            for item in prompt_items or []:
                if isinstance(item, dict) and item.get("type") == "text":
                    text = item.get("text")
                    if isinstance(text, str):
                        texts.append(text)
            lines = "\\n".join(texts).splitlines()
            outputs = []
            in_expected_section = False
            for raw_line in lines:
                line = raw_line.strip()
                if line == "### Expected Outputs":
                    in_expected_section = False
                    continue
                if line.startswith("You MUST return the following outputs in your final response:"):
                    in_expected_section = True
                    continue
                if in_expected_section:
                    if line.startswith("Use the exact output names listed above as envelope keys."):
                        break
                    if line.startswith("- "):
                        outputs.append(line[2:].strip())
            return outputs

        def payload_for(name):
            body = PAYLOADS.get(name)
            if body is None:
                return f"# {name}\\n\\nSynthetic ACP proof output.\\n"
            return body

        def chunk_text(text, size=900):
            return [text[i:i + size] for i in range(0, len(text), size)] or [text]

        def send(obj):
            sys.stdout.write(json.dumps(obj, sort_keys=True) + "\\n")
            sys.stdout.flush()

        for raw in sys.stdin:
            if not raw.strip():
                continue

            request = json.loads(raw)
            method = request.get("method")
            request_id = request.get("id")
            params = request.get("params") or {}

            if method == "initialize":
                send({
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "result": {
                        "protocolVersion": 1,
                        "agentInfo": {"name": "fake-claude-agent-acp", "version": "1.0"},
                    },
                })
                continue

            if method == "session/new":
                enabled_extensions = []
                for server in params.get("mcpServers") or []:
                    name = server.get("name")
                    if isinstance(name, str) and name:
                        enabled_extensions.append(name)
                send({
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "result": {
                        "sessionId": SESSION_ID,
                        "enabledExtensions": sorted(set(enabled_extensions))
                    },
                })
                continue

            if method == "session/prompt":
                requested = extract_expected_outputs(params.get("prompt"))
                body = "\\n\\n".join(
                    f"<<<CHAINWORKS_OUTPUT:{name}>>>\\n{payload_for(name)}\\n<<<END_CHAINWORKS_OUTPUT>>>"
                    for name in requested
                )
                for chunk in chunk_text(body):
                    send({
                        "jsonrpc": "2.0",
                        "method": "session/update",
                        "params": {
                            "type": "agent_message_chunk",
                            "content": chunk
                        },
                    })
                send({
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "result": {
                        "stopReason": "end_turn",
                        "usage": {"totalTokens": max(128, len(body) // 4)}
                    },
                })
                continue

            if method == "session/close":
                send({
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "result": {"closed": True},
                })
                break

            send({
                "jsonrpc": "2.0",
                "id": request_id,
                "result": {}
            })
        """

        try script.write(to: executableURL, atomically: true, encoding: .utf8)
        try FileManager.default.setAttributes(
            [.posixPermissions: 0o755],
            ofItemAtPath: executableURL.path
        )

        return ACPFixture(root: root, executablePath: executableURL.path)
    }

    private func buildOutputPayloads(plan: RunPlan, catalog: AgentCatalog) -> [String: String] {
        var payloads: [String: String] = [:]

        func register(task: AgentTask, stageID: String) {
            guard let agent = plan.agentBindings[task.agent] else { return }
            for outputName in agent.outputs where payloads[outputName] == nil {
                let generated = OutputContractTemplates.generateForOutput(
                    outputName: outputName,
                    agent: agent,
                    stageID: stageID,
                    catalog: catalog
                )
                payloads[outputName] = String(decoding: generated.data, as: UTF8.self)
            }
        }

        for state in plan.states.values {
            if let runBlock = state.runBlock {
                for task in tasks(in: runBlock) {
                    register(task: task, stageID: state.id)
                }
            }
            if let runAfterApproval = state.runAfterApproval {
                for task in tasks(in: runAfterApproval) {
                    register(task: task, stageID: state.id)
                }
            }
        }

        return payloads
    }

    private func decodeStringArray(_ data: Data?) -> [String] {
        guard let data else { return [] }
        return (try? JSONDecoder().decode([String].self, from: data)) ?? []
    }

    private func tasks(in runBlock: ExecutableRunBlock) -> [AgentTask] {
        runBlock.phases.flatMap { phase in
            switch phase {
            case .sequential(let tasks): return tasks
            case .parallel(let tasks): return tasks
            }
        }
    }

    private func makeMinimalRepository(prefix: String) throws -> URL {
        let repoRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("\(prefix)-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: repoRoot, withIntermediateDirectories: true)

        let readme = repoRoot.appendingPathComponent("README.md")
        try "# ACP Proof Repo\\n\\nThis repository exists only for Proposal 026 proof tests.\\n"
            .write(to: readme, atomically: true, encoding: .utf8)

        try runProcess("/usr/bin/git", ["init", "-b", "main"], currentDirectory: repoRoot)
        try runProcess("/usr/bin/git", ["config", "user.name", "Chainworks Forge Tests"], currentDirectory: repoRoot)
        try runProcess("/usr/bin/git", ["config", "user.email", "chainworks-forge-tests@local"], currentDirectory: repoRoot)
        try runProcess("/usr/bin/git", ["add", "."], currentDirectory: repoRoot)
        try runProcess("/usr/bin/git", ["commit", "-m", "ACP proof baseline"], currentDirectory: repoRoot)

        return repoRoot
    }

    private func runProcess(_ executable: String, _ arguments: [String], currentDirectory: URL) throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: executable)
        process.arguments = arguments
        process.currentDirectoryURL = currentDirectory

        let stdout = Pipe()
        let stderr = Pipe()
        process.standardOutput = stdout
        process.standardError = stderr

        try process.run()
        process.waitUntilExit()

        guard process.terminationStatus == 0 else {
            let errorOutput = String(decoding: stderr.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
            throw NSError(
                domain: "Proposal026Tests",
                code: Int(process.terminationStatus),
                userInfo: [NSLocalizedDescriptionKey: "\(executable) \(arguments.joined(separator: " ")) failed: \(errorOutput)"]
            )
        }
    }
}
