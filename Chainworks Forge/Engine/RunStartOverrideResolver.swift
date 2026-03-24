import Foundation

struct RunStartOverride: Codable, Equatable, Sendable {
    var configuredProviderID: UUID?
    var model: String?
    var effort: String?
}

struct RunStartOptions: Codable, Equatable, Sendable {
    var overridesByBackendProfileID: [String: RunStartOverride] = [:]

    static let empty = RunStartOptions()
}

enum RunStartOverrideResolver {
    static func applying(
        bindings: [String: ResolvedProviderBinding],
        to plan: RunPlan
    ) -> RunPlan {
        let adjustedBindings = Dictionary(uniqueKeysWithValues: plan.agentBindings.map { key, agent in
            guard let binding = bindings[key] else { return (key, agent) }
            return (key, ResolvedAgent(
                id: agent.id,
                title: agent.title,
                mode: agent.mode,
                backendProfileID: agent.backendProfileID,
                provider: binding.providerIdentifier,
                model: binding.model,
                effort: binding.effort,
                maxTurns: agent.maxTurns,
                temperature: agent.temperature,
                permissionProfile: agent.permissionProfile,
                skillRef: agent.skillRef,
                skillRole: agent.skillRole,
                prompt: agent.prompt,
                outputContract: agent.outputContract,
                requiresHumanApproval: agent.requiresHumanApproval,
                inputs: agent.inputs,
                outputs: agent.outputs
            ))
        })

        return RunPlan(
            workflowID: plan.workflowID,
            workflowTitle: plan.workflowTitle,
            states: plan.states,
            initialStateID: plan.initialStateID,
            agentBindings: adjustedBindings,
            variables: plan.variables,
            scoring: plan.scoring,
            failurePolicy: plan.failurePolicy,
            workflowSnapshotHash: plan.workflowSnapshotHash,
            catalogSnapshotHash: plan.catalogSnapshotHash,
            workflowSnapshotJSON: plan.workflowSnapshotJSON,
            catalogSnapshotJSON: plan.catalogSnapshotJSON,
            planCompilerVersion: plan.planCompilerVersion
        )
    }
}
