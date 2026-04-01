import Foundation

struct ValidationIssue: Identifiable, Sendable {
    let id: UUID
    let severity: Severity
    let message: String
    let location: String?

    enum Severity: String, Codable, Sendable {
        case error
        case warning
    }

    nonisolated init(severity: Severity, message: String, location: String? = nil) {
        self.id = UUID()
        self.severity = severity
        self.message = message
        self.location = location
    }
}

struct YAMLValidator: Sendable {

    static func validateAll(
        workflow: WorkflowDefinition,
        catalog: AgentCatalog
    ) -> [ValidationIssue] {
        var issues: [ValidationIssue] = []
        issues += validateStateGraph(workflow)
        issues += validateAgentReferences(workflow: workflow, catalog: catalog)
        issues += validateBackendProfileRefs(catalog)
        issues += validatePermissionProfileRefs(catalog)
        issues += validateSkillRefs(catalog)
        issues += validateOutputContractRefs(catalog)
        issues += validateArtifactRefs(catalog)
        issues += validateProviderCoverage(workflow: workflow, catalog: catalog)
        issues += validateEnvPlaceholders(catalog)
        issues += validateRunBlockSemantics(workflow)
        return issues
    }

    // MARK: - State Graph

    static func validateStateGraph(_ workflow: WorkflowDefinition) -> [ValidationIssue] {
        var issues: [ValidationIssue] = []

        // initial_state exists
        if workflow.states[workflow.initialState] == nil {
            issues.append(ValidationIssue(severity: .error, message: "initial_state '\(workflow.initialState)' not found in states", location: "initial_state"))
        }

        // All transitions point to existing states
        for (stateID, state) in workflow.states {
            for (i, transition) in (state.transitions ?? []).enumerated() {
                if workflow.states[transition.to] == nil {
                    issues.append(ValidationIssue(severity: .error, message: "Transition to '\(transition.to)' references non-existent state", location: "states.\(stateID).transitions[\(i)]"))
                }
            }
        }

        // At least one end state
        let endStates = workflow.states.filter { $0.value.type == "end" }
        if endStates.isEmpty {
            issues.append(ValidationIssue(severity: .warning, message: "No state with type 'end' found", location: "states"))
        }

        // Orphan detection (reachable from initial_state)
        var visited = Set<String>()
        var queue = [workflow.initialState]
        while !queue.isEmpty {
            let current = queue.removeFirst()
            guard !visited.contains(current) else { continue }
            visited.insert(current)
            if let state = workflow.states[current] {
                for transition in state.transitions ?? [] {
                    queue.append(transition.to)
                }
            }
        }
        let orphans = Set(workflow.states.keys).subtracting(visited)
        for orphan in orphans {
            issues.append(ValidationIssue(severity: .warning, message: "State '\(orphan)' is unreachable from initial_state", location: "states.\(orphan)"))
        }

        return issues
    }

    // MARK: - Agent References

    static func validateAgentReferences(workflow: WorkflowDefinition, catalog: AgentCatalog) -> [ValidationIssue] {
        var issues: [ValidationIssue] = []
        let catalogIDs = Set(catalog.agents.map(\.id))

        for (stateID, state) in workflow.states {
            // owner
            if !catalogIDs.contains(state.owner) {
                issues.append(ValidationIssue(severity: .error, message: "State owner '\(state.owner)' not found in agent catalog", location: "states.\(stateID).owner"))
            }
            // run block agents
            for task in allTasks(in: state.run) + allTasks(in: state.runAfterApproval) {
                if !catalogIDs.contains(task.agent) {
                    issues.append(ValidationIssue(severity: .error, message: "Agent '\(task.agent)' not found in catalog", location: "states.\(stateID).run"))
                }
            }
        }
        return issues
    }

    private static func allTasks(in block: RunBlock?) -> [AgentTask] {
        guard let block else { return [] }
        return (block.sequence ?? []) + (block.parallel ?? []) + (block.then ?? [])
    }

    // MARK: - Catalog Internal Consistency

    static func validateBackendProfileRefs(_ catalog: AgentCatalog) -> [ValidationIssue] {
        catalog.agents.compactMap { agent in
            catalog.backendProfiles[agent.backendProfile] == nil
                ? ValidationIssue(severity: .error, message: "Agent '\(agent.id)' references non-existent backend profile '\(agent.backendProfile)'", location: "agents.\(agent.id).backend_profile")
                : nil
        }
    }

    static func validatePermissionProfileRefs(_ catalog: AgentCatalog) -> [ValidationIssue] {
        catalog.agents.compactMap { agent in
            catalog.permissionProfiles[agent.permissionProfile] == nil
                ? ValidationIssue(severity: .error, message: "Agent '\(agent.id)' references non-existent permission profile '\(agent.permissionProfile)'", location: "agents.\(agent.id).permission_profile")
                : nil
        }
    }

    static func validateSkillRefs(_ catalog: AgentCatalog) -> [ValidationIssue] {
        catalog.agents.compactMap { agent in
            catalog.skills[agent.skillRef] == nil
                ? ValidationIssue(severity: .error, message: "Agent '\(agent.id)' references non-existent skill '\(agent.skillRef)'", location: "agents.\(agent.id).skill_ref")
                : nil
        }
    }

    static func validateOutputContractRefs(_ catalog: AgentCatalog) -> [ValidationIssue] {
        catalog.agents.compactMap { agent in
            guard let contract = agent.outputContract else { return nil }
            return catalog.contracts[contract] == nil
                ? ValidationIssue(severity: .error, message: "Agent '\(agent.id)' references non-existent contract '\(contract)'", location: "agents.\(agent.id).output_contract")
                : nil
        }
    }

    static func validateArtifactRefs(_ catalog: AgentCatalog) -> [ValidationIssue] {
        var issues: [ValidationIssue] = []
        let artifactIDs = Set(catalog.artifacts.keys)
        for agent in catalog.agents {
            for input in agent.inputs where !artifactIDs.contains(input) {
                issues.append(ValidationIssue(severity: .warning, message: "Agent '\(agent.id)' input '\(input)' not found in artifacts", location: "agents.\(agent.id).inputs"))
            }
            for output in agent.outputs where !artifactIDs.contains(output) {
                issues.append(ValidationIssue(severity: .warning, message: "Agent '\(agent.id)' output '\(output)' not found in artifacts", location: "agents.\(agent.id).outputs"))
            }
        }
        return issues
    }

    // MARK: - Provider Coverage

    static func validateProviderCoverage(workflow: WorkflowDefinition, catalog: AgentCatalog) -> [ValidationIssue] {
        let availableProviders = Set(catalog.backendProfiles.values.map(\.provider))
        return workflow.workflow.requiredProviders.compactMap { provider in
            availableProviders.contains(provider) ? nil
                : ValidationIssue(severity: .error, message: "Required provider '\(provider)' not covered by any backend profile", location: "workflow.required_providers")
        }
    }

    // MARK: - Env Placeholders

    static func validateEnvPlaceholders(_ catalog: AgentCatalog) -> [ValidationIssue] {
        var issues: [ValidationIssue] = []
        let wellFormedPattern = /\$\{([^}]*)\}/

        // Collect all string surfaces that may contain env placeholders
        var surfaces: [(location: String, value: String)] = []

        // paths
        for (key, value) in catalog.paths {
            surfaces.append(("paths.\(key)", value))
        }

        // artifact paths
        for (key, value) in catalog.artifacts {
            surfaces.append(("artifacts.\(key)", value))
        }

        // worktree paths
        for agent in catalog.agents {
            if let wt = agent.worktreePolicy {
                surfaces.append(("agents.\(agent.id).worktree_policy.path", wt.path))
            }
        }

        for (location, value) in surfaces {
            // Well-formed placeholders without defaults → warning
            for match in value.matches(of: wellFormedPattern) {
                let content = String(match.1)
                if !content.contains(":-") && !content.contains(":+") {
                    issues.append(ValidationIssue(severity: .warning, message: "Env placeholder '\(content)' has no default value", location: location))
                }
            }

            // Malformed placeholders: unclosed ${ → error
            if value.contains("${") {
                let openCount = value.components(separatedBy: "${").count - 1
                let closeCount = value.matches(of: wellFormedPattern).count
                if openCount > closeCount {
                    issues.append(ValidationIssue(severity: .error, message: "Malformed env placeholder (unclosed '${') detected", location: location))
                }
            }
        }

        return issues
    }

    // MARK: - Steward Config Validation

    static func validateStewardConfig(_ config: StewardConfig) -> [ValidationIssue] {
        var issues: [ValidationIssue] = []

        // Schema version
        if config.schemaVersion != 1 {
            issues.append(ValidationIssue(severity: .error, message: "Unsupported steward config schema_version: \(config.schemaVersion)", location: "schema_version"))
        }

        // Window sizes
        if config.windows.observationWindowSize <= 0 {
            issues.append(ValidationIssue(severity: .error, message: "observation_window_size must be positive", location: "windows.observation_window_size"))
        }
        if config.windows.baselineWindowSize <= 0 {
            issues.append(ValidationIssue(severity: .error, message: "baseline_window_size must be positive", location: "windows.baseline_window_size"))
        }
        if config.windows.minimumWindowSize <= 0 {
            issues.append(ValidationIssue(severity: .error, message: "minimum_window_size must be positive", location: "windows.minimum_window_size"))
        }
        if config.windows.maximumWindowAgeDays <= 0 {
            issues.append(ValidationIssue(severity: .error, message: "maximum_window_age_days must be positive", location: "windows.maximum_window_age_days"))
        }
        if config.windows.minimumWindowSize > config.windows.observationWindowSize {
            issues.append(ValidationIssue(severity: .error, message: "minimum_window_size (\(config.windows.minimumWindowSize)) must not exceed observation_window_size (\(config.windows.observationWindowSize))", location: "windows"))
        }

        // Required threshold families
        let requiredFamilies: Set<String> = ["timing", "rework", "quality", "cost", "stability"]
        let validMethods: Set<String> = ["median_percentage", "mean_percentage", "ratio"]
        let presentFamilies = Set(config.thresholds.keys)
        let missingFamilies = requiredFamilies.subtracting(presentFamilies)
        for family in missingFamilies.sorted() {
            issues.append(ValidationIssue(severity: .error, message: "Required threshold family '\(family)' is missing", location: "thresholds"))
        }

        // Threshold entries
        for (family, entry) in config.thresholds {
            if !validMethods.contains(entry.method) {
                issues.append(ValidationIssue(severity: .error, message: "Invalid threshold method '\(entry.method)' for family '\(family)'", location: "thresholds.\(family).method"))
            }
            if entry.trigger <= 0 {
                issues.append(ValidationIssue(severity: .error, message: "Threshold trigger must be positive for family '\(family)'", location: "thresholds.\(family).trigger"))
            }
        }

        // Trigger config
        if config.triggers.postRunHook.enabled && config.triggers.postRunHook.runInterval < 1 {
            issues.append(ValidationIssue(severity: .error, message: "post_run_hook.run_interval must be >= 1 when enabled", location: "triggers.post_run_hook.run_interval"))
        }

        // Context strategy profiles
        if config.contextStrategyProfiles.isEmpty {
            issues.append(ValidationIssue(severity: .error, message: "context_strategy_profiles must contain at least one profile", location: "context_strategy_profiles"))
        } else if !config.contextStrategyProfiles.keys.contains("selective_compression_and_escalation") {
            issues.append(ValidationIssue(severity: .warning, message: "Recommended profile 'selective_compression_and_escalation' is missing", location: "context_strategy_profiles"))
        }

        for (profileID, profile) in config.contextStrategyProfiles {
            if profile.agents.isEmpty {
                issues.append(ValidationIssue(severity: .warning, message: "Profile '\(profileID)' has no agent entries", location: "context_strategy_profiles.\(profileID)"))
            }

            if profile.escalationModelTier != nil && profile.defaultModelTier == nil {
                issues.append(ValidationIssue(severity: .warning, message: "Profile '\(profileID)' defines escalation_model_tier without default_model_tier", location: "context_strategy_profiles.\(profileID)"))
            }

            for (agentID, agentProfile) in profile.agents {
                if agentID.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                    issues.append(ValidationIssue(severity: .error, message: "Profile '\(profileID)' contains an empty agent key", location: "context_strategy_profiles.\(profileID)"))
                }

                guard agentProfile.handoffPolicy != nil || agentProfile.continuityMode != nil else {
                    issues.append(ValidationIssue(
                        severity: .warning,
                        message: "Agent '\(agentID)' in profile '\(profileID)' has no handoff_policy or continuity_mode",
                        location: "context_strategy_profiles.\(profileID).\(agentID)"
                    ))
                    continue
                }

                guard let policy = agentProfile.handoffPolicy else { continue }

                for artifact in policy.mandatory where artifact.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                    issues.append(ValidationIssue(
                        severity: .error,
                        message: "Empty artifact reference in mandatory list",
                        location: "context_strategy_profiles.\(profileID).\(agentID).handoff_policy.mandatory"
                    ))
                }
                for artifact in policy.summarized where artifact.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                    issues.append(ValidationIssue(
                        severity: .error,
                        message: "Empty artifact reference in summarized list",
                        location: "context_strategy_profiles.\(profileID).\(agentID).handoff_policy.summarized"
                    ))
                }
                for artifact in policy.lazy where artifact.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                    issues.append(ValidationIssue(
                        severity: .error,
                        message: "Empty artifact reference in lazy list",
                        location: "context_strategy_profiles.\(profileID).\(agentID).handoff_policy.lazy"
                    ))
                }
            }
        }

        return issues
    }

    // MARK: - Run Block Semantics

    static func validateRunBlockSemantics(_ workflow: WorkflowDefinition) -> [ValidationIssue] {
        var issues: [ValidationIssue] = []
        for (stateID, state) in workflow.states {
            if let run = state.run {
                issues += validateSingleRunBlock(run, stateID: stateID, blockName: "run")
            }
            if let runAfter = state.runAfterApproval {
                issues += validateSingleRunBlock(runAfter, stateID: stateID, blockName: "run_after_approval")
            }
        }
        return issues
    }

    private static func validateSingleRunBlock(_ block: RunBlock, stateID: String, blockName: String) -> [ValidationIssue] {
        var issues: [ValidationIssue] = []

        if let seq = block.sequence, seq.isEmpty {
            issues.append(ValidationIssue(severity: .warning, message: "Empty sequence block", location: "states.\(stateID).\(blockName).sequence"))
        }
        if let par = block.parallel, par.isEmpty {
            issues.append(ValidationIssue(severity: .error, message: "Empty parallel block in fanout", location: "states.\(stateID).\(blockName).parallel"))
        }

        // Duplicate agent in then: agent appears in both parallel and then
        if let par = block.parallel, let then = block.then {
            let parallelAgents = Set(par.map(\.agent))
            for task in then {
                if parallelAgents.contains(task.agent) {
                    issues.append(ValidationIssue(severity: .warning, message: "Agent '\(task.agent)' appears in both parallel and then blocks", location: "states.\(stateID).\(blockName).then"))
                }
            }
        }

        return issues
    }
}
