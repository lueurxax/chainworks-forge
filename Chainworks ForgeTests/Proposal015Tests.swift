import Testing
import Foundation
import SwiftData
@testable import Chainworks_Forge

@MainActor
@Suite("Proposal 015", .serialized)
struct Proposal015Tests {
    private final class Retainer {
        var appStores: [AppConfigurationStore] = []
        var providerStores: [ProviderSettingsStore] = []
        var registries: [ProviderRegistry] = []
    }

    private let container: ModelContainer
    private let context: ModelContext
    private let tempDirectory: URL
    private let retainer = Retainer()

    init() throws {
        let (container, context) = try makeTestModelContainer()
        self.container = container
        self.context = context
        self.tempDirectory = FileManager.default.temporaryDirectory
            .appendingPathComponent("Proposal015Tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: tempDirectory, withIntermediateDirectories: true)
    }

    @Test("External skill resolution applies current proposal review mode mapping")
    func externalSkillResolutionAppliesCurrentProposalReviewModeMapping() throws {
        let bundleURL = try makeSkillBundle(
            named: "proposal-review-triad",
            content: """
            Shared review skill root.
            """
        )
        let resolved = try SkillResolver.resolve(
            skillID: "proposal_review_triad",
            skillRef: SkillRef(type: "external_skill", path: bundleURL.path, name: nil, description: nil),
            skillRole: "product_owner",
            context: SkillResolverContext(catalogBaseURL: bundleURL)
        )

        #expect(resolved.type == .external)
        #expect(resolved.sourcePath == bundleURL.path)
        #expect(resolved.injectedContent.contains("product-only"))
        #expect(resolved.specializationSummary?.contains("product-only") == true)
        #expect(resolved.injectedContentHash != DefinitionHasher.hashString("proposal_review_triad"))
    }

    @Test("Relative external skill path is normalized against catalog base URL")
    func relativeExternalSkillPathIsNormalizedAgainstCatalogBaseURL() throws {
        let repoRoot = testRepositoryRootURL()
        let catalogURL = repoRoot.appendingPathComponent("examples/agents/agents.yaml", isDirectory: false)
        let resolved = try SkillResolver.resolve(
            skillID: "proposal_review_triad",
            skillRef: SkillRef(type: "external_skill", path: "../skills/proposal-review-triad", name: nil, description: nil),
            skillRole: "product_owner",
            context: SkillResolverContext(catalogBaseURL: catalogURL)
        )

        #expect(
            resolved.sourcePath
                == repoRoot.appendingPathComponent("examples/skills/proposal-review-triad", isDirectory: true)
                .standardizedFileURL.path
        )
        #expect(resolved.sourcePath?.contains("/../") == false)
    }

    @Test("Goose session packet injects resolved skill content before agent prompt")
    func gooseSessionPacketInjectsResolvedSkillContentBeforeAgentPrompt() throws {
        let resolvedSkill = ResolvedSkill(
            id: "inline_writer",
            type: .inline,
            resolvedContent: "Write concise structured proposals.",
            contentHash: DefinitionHasher.hashString("Write concise structured proposals."),
            injectedContent: SkillInjector.injectedContent(
                skillID: "inline_writer",
                type: .inline,
                content: "Write concise structured proposals."
            ),
            injectedContentHash: DefinitionHasher.hashString(
                SkillInjector.injectedContent(
                    skillID: "inline_writer",
                    type: .inline,
                    content: "Write concise structured proposals."
                )
            ),
            sourcePath: nil,
            sourceDescription: "Write concise structured proposals.",
            bundleManifest: nil,
            role: nil,
            specializationSummary: nil,
            injectionPolicy: .prependToSystemPrompt
        )
        let agent = ResolvedAgent(
            id: "writer",
            title: "Writer",
            mode: "tool_use",
            provider: "claude_code",
            model: "opus",
            effort: "high",
            maxTurns: 8,
            temperature: 0,
            permissionProfile: "ORCH",
            skillRef: "inline_writer",
            skillRole: nil,
            resolvedSkill: resolvedSkill,
            prompt: "Produce the requested draft artifact.",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["proposal"]
        )

        let packet = GooseSessionBridge.buildExecutionPacket(
            agent: agent,
            task: makeTestTask(agent: "writer", task: "draft_proposal"),
            context: makeTestExecutionContext()
        )

        let skillIndex = try #require(packet.systemPrompt.range(of: "## Skill: inline_writer")?.lowerBound)
        let promptIndex = try #require(packet.systemPrompt.range(of: "## Role and Instructions")?.lowerBound)
        #expect(skillIndex < promptIndex)
        #expect(packet.systemPrompt.contains("Write concise structured proposals."))
        #expect(packet.systemPrompt.contains("Produce the requested draft artifact."))
    }

    @Test("Preflight reports missing external skill path as blocking failure")
    func preflightReportsMissingExternalSkillPathAsBlockingFailure() async throws {
        let repoRoot = AppConfiguration.defaultRepositoryRoot()
        let workflowURL = try fixtureURL(name: "workflow", ext: "yaml")
        let catalogFixtureURL = try localizedCatalogFixtureURL()
        let mutatedCatalogURL = tempDirectory.appendingPathComponent("agents-missing-skill.yaml")
        let fixtureContent = try String(contentsOf: catalogFixtureURL, encoding: .utf8)
        let mutated = fixtureContent.replacingOccurrences(
            of: tempDirectory.appendingPathComponent("skills/proposal-review-triad").path,
            with: tempDirectory.appendingPathComponent("missing-proposal-review-triad").path
        )
        try mutated.write(to: mutatedCatalogURL, atomically: true, encoding: .utf8)

        let configuration = AppConfiguration(
            runStorageBasePath: tempDirectory.appendingPathComponent("runs").path,
            worktreeBasePath: tempDirectory.appendingPathComponent("worktrees").path,
            workflowSourcePath: workflowURL.path,
            agentCatalogSourcePath: mutatedCatalogURL.path,
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
            workflowURL: URL(fileURLWithPath: workflowURL.path),
            catalogURL: mutatedCatalogURL,
            plan: nil
        )

        #expect(report.checks.contains {
            $0.category == "Skills"
                && $0.status == .fail
                && $0.message.localizedCaseInsensitiveContains("failed resolution")
        })
        #expect(report.blockingIssues.contains {
            $0.localizedCaseInsensitiveContains("failed resolution")
                && $0.localizedCaseInsensitiveContains("proposal_review_triad")
        })
    }

    @Test("Run start snapshot freezes resolved and injected skill hashes")
    func runStartSnapshotFreezesResolvedAndInjectedSkillHashes() throws {
        let resolvedSkill = ResolvedSkill(
            id: "inline_writer",
            type: .inline,
            resolvedContent: "Write concise structured proposals.",
            contentHash: DefinitionHasher.hashString("Write concise structured proposals."),
            injectedContent: SkillInjector.injectedContent(
                skillID: "inline_writer",
                type: .inline,
                content: "Write concise structured proposals."
            ),
            injectedContentHash: DefinitionHasher.hashString(
                SkillInjector.injectedContent(
                    skillID: "inline_writer",
                    type: .inline,
                    content: "Write concise structured proposals."
                )
            ),
            sourcePath: nil,
            sourceDescription: "Write concise structured proposals.",
            bundleManifest: nil,
            role: nil,
            specializationSummary: nil,
            injectionPolicy: .prependToSystemPrompt
        )

        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        let snapshot = RunStartSnapshot(
            resolvedSkillsJSON: try encoder.encode(["inline_writer": resolvedSkill]),
            skillContentHashesJSON: try encoder.encode(["inline_writer": resolvedSkill.contentHash]),
            skillInjectedContentHashesJSON: try encoder.encode(["inline_writer": resolvedSkill.injectedContentHash])
        )

        let workspace = makeTestWorkspace(tempDir: tempDirectory)
        let run = makeTestRun(workspace: workspace, context: context)
        snapshot.apply(to: run)
        let roundTrip = RunStartSnapshot.from(run: run)

        let decodedSkills = try #require(roundTrip.resolvedSkillsJSON).decoded([String: ResolvedSkill].self)
        let decodedContentHashes = try #require(roundTrip.skillContentHashesJSON).decoded([String: String].self)
        let decodedInjectedHashes = try #require(roundTrip.skillInjectedContentHashesJSON).decoded([String: String].self)

        #expect(decodedSkills["inline_writer"]?.resolvedContent == resolvedSkill.resolvedContent)
        #expect(decodedContentHashes["inline_writer"] == resolvedSkill.contentHash)
        #expect(decodedInjectedHashes["inline_writer"] == resolvedSkill.injectedContentHash)
    }

    @Test("Execution rows and run reports persist injected skill truth")
    func executionRowsAndRunReportsPersistInjectedSkillTruth() async throws {
        let workspace = makeTestWorkspace(tempDir: tempDirectory)
        let run = makeTestRun(workspace: workspace, context: context)

        let resolvedSkill = ResolvedSkill(
            id: "inline_writer",
            type: .inline,
            resolvedContent: "Write concise structured proposals.",
            contentHash: DefinitionHasher.hashString("Write concise structured proposals."),
            injectedContent: SkillInjector.injectedContent(
                skillID: "inline_writer",
                type: .inline,
                content: "Write concise structured proposals."
            ),
            injectedContentHash: DefinitionHasher.hashString(
                SkillInjector.injectedContent(
                    skillID: "inline_writer",
                    type: .inline,
                    content: "Write concise structured proposals."
                )
            ),
            sourcePath: nil,
            sourceDescription: "Write concise structured proposals.",
            bundleManifest: nil,
            role: "primary_writer",
            specializationSummary: "generic role block: primary_writer",
            injectionPolicy: .prependToSystemPrompt
        )
        run.resolvedSkillsJSON = try JSONEncoder().encode(["inline_writer": resolvedSkill])
        run.skillContentHashesJSON = try JSONEncoder().encode(["inline_writer": resolvedSkill.contentHash])
        run.skillInjectedContentHashesJSON = try JSONEncoder().encode(["inline_writer": resolvedSkill.injectedContentHash])

        let agent = ResolvedAgent(
            id: "writer",
            title: "Writer",
            mode: "tool_use",
            provider: "claude_code",
            model: "opus",
            effort: "high",
            maxTurns: 8,
            temperature: 0,
            permissionProfile: "ORCH",
            skillRef: "inline_writer",
            skillRole: "primary_writer",
            resolvedSkill: resolvedSkill,
            prompt: "Produce the requested draft artifact.",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["proposal"]
        )

        let plan = RunPlan(
            workflowID: "wf",
            workflowTitle: "WF",
            states: [
                "start": ExecutableState(
                    id: "start",
                    label: "Start",
                    type: .start,
                    ownerAgentID: "writer",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([makeTestTask(agent: "writer", task: "draft_proposal", outputs: ["proposal"])])
                    ]),
                    runAfterApproval: nil,
                    transitions: [
                        ExecutableTransition(to: "end", condition: .artifactExists("proposal"))
                    ],
                    approvalRequired: false,
                    approvalPolicy: nil,
                    loop: nil
                ),
                "end": ExecutableState(
                    id: "end",
                    label: "End",
                    type: .end,
                    ownerAgentID: "writer",
                    runBlock: nil,
                    runAfterApproval: nil,
                    transitions: [],
                    approvalRequired: false,
                    approvalPolicy: nil,
                    loop: nil
                )
            ],
            initialStateID: "start",
            agentBindings: ["writer": agent],
            variables: [:],
            scoring: nil,
            failurePolicy: nil,
            workflowSnapshotHash: "wf-hash",
            catalogSnapshotHash: "cat-hash",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: RunPlan.currentCompilerVersion
        )

        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: SimulatedAgentExecutor(),
            modelContext: context
        )

        await orchestrator.start()

        #expect(run.status == .completed)
        let execution = try #require(run.stageExecutions.flatMap(\.agentExecutions).first)
        #expect(execution.skillRef == "inline_writer")
        #expect(execution.skillType == "inline")
        #expect(execution.skillRole == "primary_writer")
        #expect(execution.skillSnapshotHash == resolvedSkill.injectedContentHash)
        #expect(execution.skillContentSummary == resolvedSkill.contentSummary)

        let payload = RunReportBuilder(modelContext: context).buildReportPayload(for: run, version: 1)
        let reportAgent = try #require(payload.agentsUsed.first)
        #expect(reportAgent.skillRef == "inline_writer")
        #expect(reportAgent.skillType == "inline")
        #expect(reportAgent.skillRole == "primary_writer")
        #expect(reportAgent.skillSnapshotHash == resolvedSkill.injectedContentHash)
        #expect(reportAgent.resolvedSkillContent == resolvedSkill.resolvedContent)
    }

    @Test("Inline skill resolves from YAML through execution and provenance")
    func inlineSkillResolvesFromYAMLThroughExecutionAndProvenance() async throws {
        let result = try await executeFixtureSkillE2E(
            agentID: "proposal_writer",
            taskName: "draft_initial_proposal",
            outputName: "proposal_current"
        )

        #expect(result.agent.resolvedSkill?.type == .inline)
        #expect(result.execution.skillType == "inline")
        #expect(result.execution.skillRef == "proposal_writer_core")
        #expect(result.execution.skillSnapshotHash == result.agent.resolvedSkill?.injectedContentHash)
        #expect(result.reportAgent.skillRef == "proposal_writer_core")
        #expect(result.reportAgent.resolvedSkillContent == result.agent.resolvedSkill?.resolvedContent)
    }

    @Test("Builtin skill resolves from YAML through execution and provenance")
    func builtinSkillResolvesFromYAMLThroughExecutionAndProvenance() async throws {
        let result = try await executeFixtureSkillE2E(
            agentID: "docs_guardian",
            taskName: "align_docs_to_implementation",
            outputName: "docs_report"
        )

        #expect(result.agent.resolvedSkill?.type == .builtin)
        #expect(result.execution.skillType == "builtin")
        #expect(result.execution.skillRef == "docs_quality_guardian")
        #expect(result.execution.skillSnapshotHash == result.agent.resolvedSkill?.injectedContentHash)
        #expect(result.reportAgent.skillRef == "docs_quality_guardian")
        #expect(result.reportAgent.resolvedSkillContent == result.agent.resolvedSkill?.resolvedContent)
    }

    @Test("External skill resolves from YAML through execution and provenance")
    func externalSkillResolvesFromYAMLThroughExecutionAndProvenance() async throws {
        let result = try await executeFixtureSkillE2E(
            agentID: "proposal_reviewer_product_owner",
            taskName: "review_proposal_as_product_owner",
            outputName: "proposal_review_po"
        )

        #expect(result.agent.resolvedSkill?.type == .external)
        #expect(result.execution.skillType == "external")
        #expect(result.execution.skillRole == "product_owner")
        #expect(result.execution.skillSnapshotHash == result.agent.resolvedSkill?.injectedContentHash)
        #expect(result.reportAgent.skillRef == "proposal_review_triad")
        #expect(result.reportAgent.resolvedSkillContent == result.agent.resolvedSkill?.resolvedContent)
    }

    @Test("Unknown builtin skill blocks preflight as a blocking failure")
    func unknownBuiltinSkillBlocksPreflightAsBlockingFailure() async throws {
        let workflowURL = try fixtureURL(name: "workflow", ext: "yaml")
        let catalogFixtureURL = try localizedCatalogFixtureURL()
        let mutatedCatalogURL = tempDirectory.appendingPathComponent("agents-unknown-builtin.yaml")
        let fixtureContent = try String(contentsOf: catalogFixtureURL, encoding: .utf8)
        let mutated = fixtureContent.replacingOccurrences(
            of: "name: docs-quality-guardian",
            with: "name: missing-docs-guardian"
        )
        try mutated.write(to: mutatedCatalogURL, atomically: true, encoding: .utf8)

        let configuration = AppConfiguration(
            runStorageBasePath: tempDirectory.appendingPathComponent("runs-builtin").path,
            worktreeBasePath: tempDirectory.appendingPathComponent("worktrees-builtin").path,
            workflowSourcePath: workflowURL.path,
            agentCatalogSourcePath: mutatedCatalogURL.path,
            supportBundleExportPath: tempDirectory.appendingPathComponent("exports-builtin").path,
            activeConfigurationSource: .persistedSettings
        )
        let appStore = retain(AppConfigurationStore(
            fileURL: tempDirectory.appendingPathComponent("app-config-builtin.json"),
            initialConfiguration: configuration
        ))
        let providerStore = retain(ProviderSettingsStore(
            fileURL: tempDirectory.appendingPathComponent("provider-settings-builtin.json"),
            initialSettings: .empty
        ))
        let registry = retain(ProviderRegistry(settingsStore: providerStore))
        let preflight = PreflightService(appConfigurationStore: appStore, providerRegistry: registry)

        let report = await preflight.runReport(
            workflowURL: URL(fileURLWithPath: workflowURL.path),
            catalogURL: mutatedCatalogURL,
            plan: nil
        )

        #expect(report.checks.contains {
            $0.category == "Skills"
                && $0.status == .fail
                && $0.message.localizedCaseInsensitiveContains("missing-docs-guardian")
        })
        #expect(report.blockingIssues.contains {
            $0.localizedCaseInsensitiveContains("missing-docs-guardian")
        })
    }

    @Test("Role specialization changes injected hash and execution prompt")
    func roleSpecializationChangesInjectedHashAndExecutionPrompt() throws {
        let bundleURL = try makeSkillBundle(
            named: "proposal-review-triad-diff",
            content: """
            Shared review skill root.
            """
        )
        let skillRef = SkillRef(type: "external_skill", path: bundleURL.path, name: nil, description: nil)
        let product = try SkillResolver.resolve(
            skillID: "proposal_review_triad",
            skillRef: skillRef,
            skillRole: "product_owner",
            context: SkillResolverContext(catalogBaseURL: bundleURL)
        )
        let architect = try SkillResolver.resolve(
            skillID: "proposal_review_triad",
            skillRef: skillRef,
            skillRole: "architect",
            context: SkillResolverContext(catalogBaseURL: bundleURL)
        )

        #expect(product.injectedContentHash != architect.injectedContentHash)

        let productAgent = ResolvedAgent(
            id: "proposal_reviewer_product_owner",
            title: "Product Owner",
            mode: "proposal_review.product_owner",
            provider: "claude_code",
            model: "opus",
            effort: "high",
            maxTurns: 8,
            temperature: 0,
            permissionProfile: "RO_REVIEW",
            skillRef: "proposal_review_triad",
            skillRole: "product_owner",
            resolvedSkill: product,
            prompt: "Review as product owner.",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["proposal_review_po"]
        )
        let architectAgent = ResolvedAgent(
            id: "proposal_reviewer_architect",
            title: "Architect",
            mode: "proposal_review.architect",
            provider: "claude_code",
            model: "opus",
            effort: "high",
            maxTurns: 8,
            temperature: 0,
            permissionProfile: "RO_REVIEW",
            skillRef: "proposal_review_triad",
            skillRole: "architect",
            resolvedSkill: architect,
            prompt: "Review as architect.",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["proposal_review_architect"]
        )

        let productPacket = GooseSessionBridge.buildExecutionPacket(
            agent: productAgent,
            task: makeTestTask(agent: productAgent.id, task: "review_proposal_as_product_owner"),
            context: makeTestExecutionContext()
        )
        let architectPacket = GooseSessionBridge.buildExecutionPacket(
            agent: architectAgent,
            task: makeTestTask(agent: architectAgent.id, task: "review_proposal_as_architect"),
            context: makeTestExecutionContext()
        )

        #expect(productPacket.systemPrompt != architectPacket.systemPrompt)
        #expect(productPacket.systemPrompt.contains("product-only"))
        #expect(architectPacket.systemPrompt.contains("architecture-only"))
    }

    private func makeSkillBundle(named name: String, content: String) throws -> URL {
        let bundleURL = tempDirectory.appendingPathComponent(name, isDirectory: true)
        try FileManager.default.createDirectory(at: bundleURL, withIntermediateDirectories: true)
        try content.write(
            to: bundleURL.appendingPathComponent("SKILL.md"),
            atomically: true,
            encoding: .utf8
        )
        return bundleURL
    }

    private func fixtureURL(name: String, ext: String) throws -> URL {
        try #require(
            Bundle(for: TestBundleMarker.self).url(forResource: name, withExtension: ext),
            "Missing fixture \(name).\(ext)"
        )
    }

    private func retain(_ store: AppConfigurationStore) -> AppConfigurationStore {
        retainer.appStores.append(store)
        return store
    }

    private func retain(_ store: ProviderSettingsStore) -> ProviderSettingsStore {
        retainer.providerStores.append(store)
        return store
    }

    private func retain(_ registry: ProviderRegistry) -> ProviderRegistry {
        retainer.registries.append(registry)
        return registry
    }

    private func executeFixtureSkillE2E(
        agentID: String,
        taskName: String,
        outputName: String
    ) async throws -> (agent: ResolvedAgent, execution: AgentExecution, reportAgent: RunReportPayload.AgentEntry) {
        let catalogURL = try localizedCatalogFixtureURL()
        let workflowURL = try makeMinimalWorkflow(
            agentID: agentID,
            taskName: taskName,
            outputName: outputName
        )
        let catalog = try YAMLParser.loadAgentCatalog(from: catalogURL)
        let workflow = try YAMLParser.loadWorkflow(from: workflowURL)
        let compiler = RunPlanCompiler(modelContext: context)
        let plan = try compiler.previewCompile(
            workflow: workflow,
            catalog: catalog,
            catalogSourcePath: catalogURL.path
        )
        let agent = try #require(plan.agentBindings[agentID])

        let workspace = makeTestWorkspace(tempDir: tempDirectory)
        let idea = Idea(title: "Skill E2E \(agentID)", body: "Proposal 015 skill proof")
        context.insert(idea)
        let run = Run(
            id: workspace.runID,
            workflowID: workflow.workflow.id,
            workflowTitle: workflow.workflow.name,
            workflowSnapshotHash: plan.workflowSnapshotHash,
            catalogSnapshotHash: plan.catalogSnapshotHash,
            workflowSourcePath: workflowURL.path,
            catalogSourcePath: catalogURL.path,
            workflowSnapshotJSON: plan.workflowSnapshotJSON,
            catalogSnapshotJSON: plan.catalogSnapshotJSON,
            workspaceRoot: workspace.workspaceRoot.path,
            artifactRoot: workspace.artifactRoot.path,
            planCompilerVersion: RunPlan.currentCompilerVersion
        )
        run.idea = idea
        context.insert(run)
        run.resolvedSkillsJSON = try JSONEncoder().encode([agent.skillRef: try #require(agent.resolvedSkill)])
        run.skillContentHashesJSON = try JSONEncoder().encode([agent.skillRef: try #require(agent.resolvedSkill).contentHash])
        run.skillInjectedContentHashesJSON = try JSONEncoder().encode([agent.skillRef: try #require(agent.resolvedSkill).injectedContentHash])

        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: SimulatedAgentExecutor(),
            modelContext: context
        )
        await orchestrator.start()

        #expect(run.status == .completed)
        let execution = try #require(run.stageExecutions.flatMap(\.agentExecutions).first)
        let reportAgent = try #require(RunReportBuilder(modelContext: context).buildReportPayload(for: run, version: 1).agentsUsed.first)
        return (agent, execution, reportAgent)
    }

    private func makeMinimalWorkflow(
        agentID: String,
        taskName: String,
        outputName: String
    ) throws -> URL {
        let url = tempDirectory.appendingPathComponent("workflow-\(agentID)-\(UUID().uuidString).yaml")
        let content = """
        schema_version: 1
        workflow:
          id: skill_resolution_\(agentID)
          name: Skill Resolution \(agentID)
          uses_agent_catalog: ./agents.yaml
          description: Minimal workflow for Proposal 015 end-to-end proof.
          idea_input:
            mode: text_with_optional_file
          execution:
            single_active_run_per_idea: true
            resume_policy: automatic_on_launch
          required_providers: []
        initial_state: start
        states:
          start:
            label: Start
            type: start
            owner: \(agentID)
            run:
              sequence:
                - agent: \(agentID)
                  task: \(taskName)
                  outputs:
                    - \(outputName)
            transitions:
              - to: end
                when: exists('\(outputName)')
          end:
            label: End
            type: end
            owner: \(agentID)
        """
        try content.write(to: url, atomically: true, encoding: .utf8)
        return url
    }

    private func localizedCatalogFixtureURL() throws -> URL {
        let catalogFixtureURL = try fixtureURL(name: "agents", ext: "yaml")
        let skillsRoot = tempDirectory.appendingPathComponent("skills", isDirectory: true)
        let triadRoot = skillsRoot.appendingPathComponent("proposal-review-triad", isDirectory: true)
        let auditRoot = skillsRoot.appendingPathComponent("proposal-implementation-audit", isDirectory: true)
        try FileManager.default.createDirectory(at: triadRoot, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: auditRoot, withIntermediateDirectories: true)
        try """
        Shared proposal review instructions.
        Focus on score-limiting issues and make the specialization mode explicit.
        """.write(
            to: triadRoot.appendingPathComponent("SKILL.md"),
            atomically: true,
            encoding: .utf8
        )
        try """
        Compare implementation evidence against approved proposal requirements.
        Preserve authoritative artifact truth and fail closed on missing proof.
        """.write(
            to: auditRoot.appendingPathComponent("SKILL.md"),
            atomically: true,
            encoding: .utf8
        )

        let localizedURL = tempDirectory.appendingPathComponent("agents-localized.yaml")
        let fixtureContent = try String(contentsOf: catalogFixtureURL, encoding: .utf8)
        let legacySkillsRoot = ["", "Users", "user", ".codex", "skills"].joined(separator: "/")
        let localized = fixtureContent
            .replacingOccurrences(of: "\(legacySkillsRoot)/proposal-review-triad", with: triadRoot.path)
            .replacingOccurrences(of: "\(legacySkillsRoot)/proposal-implementation-audit", with: auditRoot.path)
            .replacingOccurrences(of: "../skills/proposal-review-triad", with: triadRoot.path)
            .replacingOccurrences(of: "../skills/proposal-implementation-audit", with: auditRoot.path)
            .replacingOccurrences(of: "../../examples/skills/proposal-review-triad", with: triadRoot.path)
            .replacingOccurrences(of: "../../examples/skills/proposal-implementation-audit", with: auditRoot.path)
        try localized.write(to: localizedURL, atomically: true, encoding: .utf8)
        return localizedURL
    }
}

private extension Data {
    func decoded<T: Decodable>(_ type: T.Type) throws -> T {
        try JSONDecoder().decode(type, from: self)
    }
}
