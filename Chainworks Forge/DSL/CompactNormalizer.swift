import Foundation

/// Converts CompactWorkflowDefinition into WorkflowDefinition (ARCH-010, ARCH-012).
/// Agent alias resolution: Strategy 1 (hyphens → underscores), Strategy 2 (explicit agent_aliases map).
/// No automatic guessing. Unresolvable IDs are compilation errors.
struct CompactNormalizer {

    /// Normalize a compact workflow into a full WorkflowDefinition.
    static func normalize(
        _ compact: CompactWorkflowDefinition,
        catalog: AgentCatalog
    ) throws -> WorkflowDefinition {
        let catalogAgentIDs = Set(catalog.agents.map(\.id))
        let aliasMap = compact.agentAliases ?? [:]

        // Resolve all agent IDs in stages
        var states: [String: WorkflowState] = [:]
        let stages = compact.workflow.stages

        // Build the needs→transitions graph: for each stage, create transitions
        // FROM each needed stage TO this stage.
        var transitionsBySource: [String: [Transition]] = [:]

        for stage in stages {
            // Build transitions from this stage's needs
            if let needs = stage.needs, !needs.isEmpty {
                for needed in needs {
                    let transition = Transition(to: stage.id, when: "'true'")
                    transitionsBySource[needed, default: []].append(transition)
                }
            }

            // Add gate conditions as transitions from this stage
            if let gate = stage.gate {
                for req in gate.require {
                    // Gate requirements become transition conditions on outgoing edges
                    // (handled when the stage that needs this one resolves transitions)
                    _ = req // gate requirements are validated but not turned into transitions in compact
                }
            }
        }

        // First stage with no needs is the entry point
        let entryStage = stages.first { ($0.needs ?? []).isEmpty }
        let initialStateID = entryStage?.id ?? stages.first!.id

        // Last stage (no stage needs it) is the end
        let neededIDs = Set(stages.flatMap { $0.needs ?? [] })
        let endStageID = stages.last { !neededIDs.contains($0.id) && $0.type != "approval" }?.id

        for stage in stages {
            let stateType: String?
            let approval: String?
            var runBlock: RunBlock?

            switch stage.type {
            case "approval":
                stateType = "manual_gate"
                approval = "required"
                runBlock = nil

            case "single":
                stateType = nil
                approval = nil
                if let agentAlias = stage.agent {
                    let resolvedID = try resolveAgentID(agentAlias, aliasMap: aliasMap, catalogIDs: catalogAgentIDs, stageID: stage.id)
                    runBlock = RunBlock(
                        sequence: [AgentTask(agent: resolvedID, task: stage.id, inputs: nil, outputs: nil)],
                        parallel: nil,
                        then: nil
                    )
                }

            case "fanout":
                stateType = nil
                approval = nil
                if let agentAliases = stage.agents {
                    let resolvedIDs = try agentAliases.map {
                        try resolveAgentID($0, aliasMap: aliasMap, catalogIDs: catalogAgentIDs, stageID: stage.id)
                    }
                    let parallelTasks = resolvedIDs.map {
                        AgentTask(agent: $0, task: stage.id, inputs: nil, outputs: nil)
                    }
                    runBlock = RunBlock(sequence: nil, parallel: parallelTasks, then: nil)
                }

            default:
                stateType = nil
                approval = nil
            }

            // Determine owner: first agent in the stage, or "lead_orchestrator" for approval gates
            let owner: String
            if stage.type == "approval" {
                owner = resolveAgentIDSafe("orchestrator", aliasMap: aliasMap, catalogIDs: catalogAgentIDs) ?? "lead_orchestrator"
            } else if let agentAlias = stage.agent {
                owner = try resolveAgentID(agentAlias, aliasMap: aliasMap, catalogIDs: catalogAgentIDs, stageID: stage.id)
            } else if let firstAgent = stage.agents?.first {
                if let orchestratorID = resolveAgentIDSafe("orchestrator", aliasMap: aliasMap, catalogIDs: catalogAgentIDs) {
                    owner = orchestratorID
                } else {
                    owner = try resolveAgentID(firstAgent, aliasMap: aliasMap, catalogIDs: catalogAgentIDs, stageID: stage.id)
                }
            } else {
                owner = "lead_orchestrator"
            }

            // Mark entry and end states
            var finalType = stateType
            if stage.id == initialStateID { finalType = "start" }
            if stage.id == endStageID { finalType = "end" }

            let transitions = transitionsBySource[stage.id] ?? []
            // For approval gates, add approval.granted transition
            let finalTransitions: [Transition]
            if stage.type == "approval" {
                finalTransitions = transitions.map { t in
                    Transition(to: t.to, when: "approval.granted == true")
                }
            } else {
                finalTransitions = transitions
            }

            states[stage.id] = WorkflowState(
                label: stage.id.replacingOccurrences(of: "_", with: " ").capitalized,
                type: finalType,
                owner: owner,
                approval: approval,
                run: runBlock,
                runAfterApproval: nil,
                loop: nil,
                transitions: finalTransitions.isEmpty ? nil : finalTransitions
            )
        }

        return WorkflowDefinition(
            schemaVersion: compact.version,
            workflow: WorkflowMeta(
                id: compact.workflow.id,
                name: compact.workflow.title,
                usesAgentCatalog: nil,
                description: "Normalized from compact format",
                ideaInput: nil,
                execution: compact.workflow.execution,
                requiredProviders: compact.workflow.requiredProviders
            ),
            variables: nil,
            failurePolicy: nil,
            scoring: nil,
            initialState: initialStateID,
            states: states
        )
    }

    // MARK: - Agent ID Resolution (ARCH-012)

    /// Resolve a compact agent alias to a canonical catalog ID.
    /// Strategy 1: hyphens → underscores. Strategy 2: explicit alias map.
    /// No guessing. Throws if unresolvable.
    private static func resolveAgentID(
        _ alias: String,
        aliasMap: [String: String],
        catalogIDs: Set<String>,
        stageID: String
    ) throws -> String {
        // Strategy 1: mechanical transform
        let mechanical = alias.replacingOccurrences(of: "-", with: "_")
        if catalogIDs.contains(mechanical) {
            return mechanical
        }

        // Strategy 2: explicit alias map
        if let mapped = aliasMap[alias], catalogIDs.contains(mapped) {
            return mapped
        }

        // Also check if alias itself is already a canonical ID
        if catalogIDs.contains(alias) {
            return alias
        }

        throw CompilationError.agentNotFound(agentID: alias, stateID: stageID)
    }

    /// Non-throwing variant for optional resolution (e.g., owner fallback).
    private static func resolveAgentIDSafe(
        _ alias: String,
        aliasMap: [String: String],
        catalogIDs: Set<String>
    ) -> String? {
        let mechanical = alias.replacingOccurrences(of: "-", with: "_")
        if catalogIDs.contains(mechanical) { return mechanical }
        if let mapped = aliasMap[alias], catalogIDs.contains(mapped) { return mapped }
        if catalogIDs.contains(alias) { return alias }
        return nil
    }
}
