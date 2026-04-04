import Foundation
import SwiftData

/// Two-phase compiler: previewCompile (no persistence) + createRun (irreversible) — ARCH-021.
@MainActor
final class RunPlanCompiler {
    private let modelContext: ModelContext

    init(modelContext: ModelContext) {
        self.modelContext = modelContext
    }

    // MARK: - Phase 1: Preview Compile (no persistence, safe to cancel)

    /// Validate and assemble an in-memory RunPlan. No SwiftData mutations.
    /// Safe to call from the Start Run sheet for preview/validation.
    func previewCompile(
        workflow: WorkflowDefinition,
        catalog: AgentCatalog,
        catalogSourcePath: String? = nil
    ) throws -> RunPlan {
        // Step 1: Validate
        let issues = YAMLValidator.validateAll(workflow: workflow, catalog: catalog)
        let errors = issues.filter { $0.severity == .error }
        if !errors.isEmpty {
            throw CompilationError.validationFailed(issues)
        }

        // Step 2: Resolve agents
        let agentBindings = try resolveAgents(
            workflow: workflow,
            catalog: catalog,
            catalogSourcePath: catalogSourcePath
        )

        // Step 3: Parse transitions
        // Step 4: Resolve loop budgets (compile-time vars.* for loop.max only)
        // Step 5: Build ExecutableStates
        let executableStates = try buildExecutableStates(
            workflow: workflow,
            variables: workflow.variables ?? [:]
        )

        // Step 6: Compute provenance
        let (workflowData, workflowHash) = try DefinitionHasher.hash(workflow)
        let (catalogData, catalogHash) = try DefinitionHasher.hash(catalog)

        // Step 7: Assemble RunPlan
        return RunPlan(
            workflowID: workflow.workflow.id,
            workflowTitle: workflow.workflow.name,
            states: executableStates,
            initialStateID: workflow.initialState,
            agentBindings: agentBindings,
            variables: workflow.variables ?? [:],
            scoring: workflow.scoring,
            failurePolicy: workflow.failurePolicy,
            requiresProjectAccess: workflow.workflow.execution.requiresProjectAccess,
            workflowSnapshotHash: workflowHash,
            catalogSnapshotHash: catalogHash,
            workflowSnapshotJSON: workflowData,
            catalogSnapshotJSON: catalogData,
            planCompilerVersion: RunPlan.currentCompilerVersion
        )
    }

    /// Phase 1 variant: normalize compact to full, then preview-compile.
    func previewCompileCompact(
        compact: CompactWorkflowDefinition,
        catalog: AgentCatalog,
        catalogSourcePath: String? = nil
    ) throws -> RunPlan {
        let normalized = try CompactNormalizer.normalize(compact, catalog: catalog)
        return try previewCompile(
            workflow: normalized,
            catalog: catalog,
            catalogSourcePath: catalogSourcePath
        )
    }

    // MARK: - Phase 2: Create Run (irreversible persistence)

    /// Persist a previewed RunPlan as a Run in SwiftData.
    /// Creates RunWorkspace and Run record. StageExecutions are created lazily (ARCH-027).
    func createRun(
        for idea: Idea,
        plan: RunPlan,
        workflowSourcePath: String,
        catalogSourcePath: String,
        startSnapshot: RunStartSnapshot
    ) throws -> (Run, RunWorkspace) {
        // Step 8: Generate run identity
        let runID = UUID()

        // Step 9: Provision workspace (ARCH-025)
        let workspace = try provisionWorkspace(runID: runID)

        // Step 10: Persist Run
        let repository = RunRepository(context: modelContext)
        let run = try repository.createRunFromPlan(
            for: idea,
            plan: plan,
            workspace: workspace,
            workflowSourcePath: workflowSourcePath,
            catalogSourcePath: catalogSourcePath,
            startSnapshot: startSnapshot
        )

        return (run, workspace)
    }

    func createRun(
        for idea: Idea,
        plan: RunPlan,
        workflowSourcePath: String,
        catalogSourcePath: String
    ) throws -> (Run, RunWorkspace) {
        try createRun(
            for: idea,
            plan: plan,
            workflowSourcePath: workflowSourcePath,
            catalogSourcePath: catalogSourcePath,
            startSnapshot: RunStartSnapshot()
        )
    }

    // MARK: - Resume Path

    /// Rebuild an in-memory RunPlan from a persisted Run's snapshots.
    /// Does NOT create a new Run. Used by ResumeManager on app launch (ARCH-029).
    func rebuildPlanFromSnapshot(run: Run) throws -> (RunPlan, RunWorkspace) {
        // Check compiler version
        guard run.planCompilerVersion == RunPlan.currentCompilerVersion else {
            throw ResumeError.compilerVersionMismatch(
                persisted: run.planCompilerVersion,
                current: RunPlan.currentCompilerVersion
            )
        }

        // Decode frozen snapshots
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let workflow = try decoder.decode(WorkflowDefinition.self, from: run.workflowSnapshotJSON)
        let catalog = try decoder.decode(AgentCatalog.self, from: run.catalogSnapshotJSON)

        // Rebuild plan via previewCompile (no persistence)
        let plan = try previewCompile(
            workflow: workflow,
            catalog: catalog,
            catalogSourcePath: run.catalogSourcePath
        )

        // Reconstruct workspace from persisted paths
        // Proposal 007: restore worktreeRoot from persisted delivery config
        let worktreeRoot: URL? = run.worktreeRoot.flatMap { URL(fileURLWithPath: $0) }
        let workspace = RunWorkspace(
            runID: run.id,
            workspaceRoot: URL(fileURLWithPath: run.workspaceRoot),
            artifactRoot: URL(fileURLWithPath: run.artifactRoot),
            worktreeRoot: worktreeRoot
        )

        return (plan, workspace)
    }

    // MARK: - Private: Agent Resolution

    private func resolveAgents(
        workflow: WorkflowDefinition,
        catalog: AgentCatalog,
        catalogSourcePath: String?
    ) throws -> [String: ResolvedAgent] {
        let agentMap = Dictionary(uniqueKeysWithValues: catalog.agents.map { ($0.id, $0) })
        var bindings: [String: ResolvedAgent] = [:]
        let resolverContext = SkillResolverContext(
            catalogBaseURL: catalogSourcePath.map { URL(fileURLWithPath: $0) }
        )

        // Collect all agent IDs referenced in run blocks
        var referencedAgentIDs: Set<String> = []
        for (_, state) in workflow.states {
            referencedAgentIDs.insert(state.owner)
            collectAgentIDs(from: state.run, into: &referencedAgentIDs)
            collectAgentIDs(from: state.runAfterApproval, into: &referencedAgentIDs)
        }

        // Resolve each referenced agent
        for agentID in referencedAgentIDs {
            guard let agentDef = agentMap[agentID] else {
                // Find which state references this agent for the error message
                let stateID = workflow.states.first { _, state in
                    state.owner == agentID
                    || tasksContain(block: state.run, agentID: agentID)
                    || tasksContain(block: state.runAfterApproval, agentID: agentID)
                }?.key ?? "unknown"
                throw CompilationError.agentNotFound(agentID: agentID, stateID: stateID)
            }

            guard let backend = catalog.backendProfiles[agentDef.backendProfile] else {
                throw CompilationError.backendProfileNotFound(
                    profileID: agentDef.backendProfile, agentID: agentID
                )
            }

            guard let skillRef = catalog.skills[agentDef.skillRef] else {
                throw CompilationError.skillResolutionFailed(
                    agentID: agentID,
                    skillRef: agentDef.skillRef,
                    reason: SkillResolutionError.skillNotFound(agentDef.skillRef).localizedDescription
                )
            }

            let resolvedSkill: ResolvedSkill
            do {
                resolvedSkill = try SkillResolver.resolve(
                    skillID: agentDef.skillRef,
                    skillRef: skillRef,
                    skillRole: agentDef.skillRole,
                    context: resolverContext
                )
            } catch {
                throw CompilationError.skillResolutionFailed(
                    agentID: agentID,
                    skillRef: agentDef.skillRef,
                    reason: error.localizedDescription
                )
            }

            bindings[agentID] = ResolvedAgent(
                id: agentDef.id,
                title: agentDef.title,
                mode: agentDef.mode,
                backendProfileID: agentDef.backendProfile,
                provider: backend.provider,
                model: backend.model,
                effort: backend.effort,
                maxTurns: backend.maxTurns,
                temperature: backend.temperature,
                permissionProfile: agentDef.permissionProfile,
                mcpProfileID: agentDef.mcpProfile,
                skillRef: agentDef.skillRef,
                skillRole: agentDef.skillRole,
                resolvedSkill: resolvedSkill,
                prompt: agentDef.prompt,
                outputContract: agentDef.outputContract,
                requiresHumanApproval: agentDef.requiresHumanApproval,
                inputs: agentDef.inputs,
                outputs: agentDef.outputs,
                worktreeWriteEnabled: agentDef.worktreePolicy?.writeEnabled ?? false,
                sessionReuseScope: SessionReuseScope(rawValue: agentDef.sessionReuseScope ?? "") ?? .same_invocation_owner,
                sessionFamilyID: agentDef.sessionFamilyID
            )
        }

        return bindings
    }

    private func collectAgentIDs(from block: RunBlock?, into set: inout Set<String>) {
        guard let block else { return }
        block.sequence?.forEach { set.insert($0.agent) }
        block.parallel?.forEach { set.insert($0.agent) }
        block.then?.forEach { set.insert($0.agent) }
    }

    private func tasksContain(block: RunBlock?, agentID: String) -> Bool {
        guard let block else { return false }
        return (block.sequence ?? []).contains { $0.agent == agentID }
            || (block.parallel ?? []).contains { $0.agent == agentID }
            || (block.then ?? []).contains { $0.agent == agentID }
    }

    // MARK: - Private: Build Executable States

    private func buildExecutableStates(
        workflow: WorkflowDefinition,
        variables: [String: AnyCodableValue]
    ) throws -> [String: ExecutableState] {
        var result: [String: ExecutableState] = [:]

        for (stateID, state) in workflow.states {
            let execRunBlock = buildRunBlock(state.run)
            let execRunAfterApproval = buildRunBlock(state.runAfterApproval)

            let transitions = (state.transitions ?? []).map { t in
                ExecutableTransition(to: t.to, condition: parseCondition(t.when))
            }

            let loop: ResolvedLoopConfig?
            if let loopConfig = state.loop {
                loop = try resolveLoop(loopConfig, variables: variables)
            } else {
                loop = nil
            }

            result[stateID] = ExecutableState(
                id: stateID,
                label: state.label,
                type: StateType(rawValue: state.type ?? ""),
                ownerAgentID: state.owner,
                runBlock: execRunBlock,
                runAfterApproval: execRunAfterApproval,
                transitions: transitions,
                approvalRequired: state.approval == "required",
                approvalPolicy: state.approvalPolicy,
                loop: loop
            )
        }

        return result
    }

    private func buildRunBlock(_ block: RunBlock?) -> ExecutableRunBlock? {
        guard let block else { return nil }
        var phases: [ExecutionPhase] = []

        if let seq = block.sequence, !seq.isEmpty {
            phases.append(.sequential(seq))
        }
        if let par = block.parallel, !par.isEmpty {
            phases.append(.parallel(par))
        }
        if let then = block.then, !then.isEmpty {
            phases.append(.sequential(then))
        }

        return phases.isEmpty ? nil : ExecutableRunBlock(phases: phases)
    }

    // MARK: - Private: Transition Condition Parsing

    private func parseCondition(_ when: String) -> TransitionCondition {
        let trimmed = when.trimmingCharacters(in: .whitespaces)

        if trimmed == "true" || trimmed == "'true'" {
            return .always
        }

        if trimmed == "approval.granted == true" {
            return .approvalGranted
        }

        // exists('artifact_name')
        if trimmed.hasPrefix("exists(") {
            let inner = trimmed
                .dropFirst(7).dropLast(1)   // remove exists( and )
                .trimmingCharacters(in: CharacterSet(charactersIn: "'\""))
            return .artifactExists(String(inner))
        }

        return .expression(trimmed)
    }

    // MARK: - Private: Loop Resolution

    private func resolveLoop(
        _ config: LoopConfig,
        variables: [String: AnyCodableValue]
    ) throws -> ResolvedLoopConfig {
        let maxValue: Int

        if config.max.hasPrefix("vars.") {
            let varName = String(config.max.dropFirst(5))
            guard let value = variables[varName] else {
                throw CompilationError.variableNotFound(
                    name: varName, context: "loop.max for counter '\(config.counter)'"
                )
            }
            switch value {
            case .int(let v): maxValue = v
            case .double(let v): maxValue = Int(v)
            case .string(let v):
                guard let parsed = Int(v) else {
                    throw CompilationError.invalidLoopMax(value: v, counter: config.counter)
                }
                maxValue = parsed
            default:
                throw CompilationError.invalidLoopMax(
                    value: String(describing: value), counter: config.counter
                )
            }
        } else if let parsed = Int(config.max) {
            maxValue = parsed
        } else {
            throw CompilationError.invalidLoopMax(value: config.max, counter: config.counter)
        }

        return ResolvedLoopConfig(counter: config.counter, resolvedMax: maxValue)
    }

    // MARK: - Private: Workspace Provisioning

    private func provisionWorkspace(runID: UUID) throws -> RunWorkspace {
        let configuredBasePath = ProcessInfo.processInfo.environment["CHAINWORKS_RUN_STORAGE_BASE_PATH"]?
            .trimmingCharacters(in: .whitespacesAndNewlines)
        let runsBase = configuredBasePath?.isEmpty == false
            ? URL(fileURLWithPath: configuredBasePath!, isDirectory: true)
            : AppConfiguration.defaultSupportRoot().appendingPathComponent("runs", isDirectory: true)
        let workspaceRoot = runsBase
            .appendingPathComponent(runID.uuidString, isDirectory: true)
        let artifactRoot = workspaceRoot
            .appendingPathComponent("artifacts", isDirectory: true)

        try FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)

        return RunWorkspace(
            runID: runID,
            workspaceRoot: workspaceRoot,
            artifactRoot: artifactRoot,
            worktreeRoot: nil
        )
    }
}

// MARK: - Resume Errors

enum ResumeError: Error, LocalizedError {
    case compilerVersionMismatch(persisted: Int, current: Int)

    var errorDescription: String? {
        switch self {
        case .compilerVersionMismatch(let persisted, let current):
            return "Compiler version mismatch: run was compiled with v\(persisted), current is v\(current). Cancel and re-create the run."
        }
    }
}
