import Foundation

// MARK: - Proposal 013 Layer Q: Output Contract Declarative Bridge

/// Eliminates hardcoded outputName -> contractID fallback branches so
/// output-to-contract binding is fully catalog-driven and testable.
///
/// This bridge replaces the legacy OutputContractResolver with
/// OutputContractResolverV2 across all runtime consumers.
enum OutputContractDeclarativeBridge {

    /// Migrate all contract resolution calls from V1 to V2.
    /// Returns a diagnostic report of the migration.
    static func verifyDeclarativeBinding(catalog: AgentCatalog) -> DeclarativeBindingReport {
        var bindings: [ContractBinding] = []

        for agent in catalog.agents {
            let agentOutputs = agent.outputs
            for outputName in agentOutputs {
                // V1 resolution (legacy — for comparison only)
                let v1Contract = legacyResolveContractID(outputName: outputName, agent: agent, catalog: catalog)

                // V2 resolution (declarative — no hardcoded branches)
                let resolvedAgent = makeMinimalResolvedAgent(from: agent, catalog: catalog)
                let v2Contract = OutputContractResolverV2.resolveContractID(
                    for: outputName,
                    agent: resolvedAgent,
                    catalog: catalog
                )

                let binding = ContractBinding(
                    agentID: agent.id,
                    outputName: outputName,
                    v1ContractID: v1Contract,
                    v2ContractID: v2Contract,
                    bindingsMatch: v1Contract == v2Contract,
                    source: v2Contract != nil ? .catalogDriven : .unresolved
                )
                bindings.append(binding)
            }
        }

        return DeclarativeBindingReport(
            timestamp: Date(),
            bindings: bindings,
            totalBindings: bindings.count,
            matchingBindings: bindings.filter { $0.bindingsMatch }.count,
            catalogDrivenBindings: bindings.filter { $0.source == .catalogDriven }.count,
            unresolvedBindings: bindings.filter { $0.source == .unresolved }.count
        )
    }

    // MARK: - Legacy V1 Resolution (for comparison only)

    /// This is the old hardcoded resolution logic.
    /// Used only for migration verification — not for runtime.
    private static func legacyResolveContractID(
        outputName: String,
        agent: AgentDefinition,
        catalog: AgentCatalog
    ) -> String? {
        // These were the hardcoded branches in OutputContractResolver
        switch outputName {
        case "proposal_review_po", "proposal_review_ux", "proposal_review_ui", "proposal_review_architect":
            return "proposal_review_v1"
        case "proposal_review_summary":
            return "proposal_review_summary_v1"
        case "prepush_review_report":
            return "prepush_review_v1"
        case "final_feature_report":
            return "final_feature_report_v1"
        default:
            break
        }

        if catalog.contracts[outputName] != nil {
            return outputName
        }
        let versioned = "\(outputName)_v1"
        if catalog.contracts[versioned] != nil {
            return versioned
        }
        if let explicit = agent.outputContract {
            return explicit
        }
        return nil
    }

    // MARK: - Helper: Minimal ResolvedAgent

    private static func makeMinimalResolvedAgent(
        from definition: AgentDefinition,
        catalog: AgentCatalog
    ) -> ResolvedAgent {
        let profile = catalog.backendProfiles[definition.backendProfile]
        return ResolvedAgent(
            id: definition.id,
            title: definition.title,
            mode: definition.mode,
            backendProfileID: definition.backendProfile,
            provider: profile?.provider ?? "unknown",
            model: profile?.model ?? "unknown",
            effort: profile?.effort ?? "medium",
            maxTurns: profile?.maxTurns ?? 10,
            temperature: profile?.temperature ?? 0.1,
            permissionProfile: definition.permissionProfile,
            skillRef: definition.skillRef,
            skillRole: definition.skillRole,
            prompt: definition.prompt,
            outputContract: definition.outputContract,
            requiresHumanApproval: definition.requiresHumanApproval,
            inputs: definition.inputs,
            outputs: definition.outputs,
            worktreeWriteEnabled: definition.worktreePolicy?.writeEnabled ?? false
        )
    }
}

// MARK: - Declarative Binding Report

struct DeclarativeBindingReport: Codable, Sendable {
    let timestamp: Date
    let bindings: [ContractBinding]
    let totalBindings: Int
    let matchingBindings: Int
    let catalogDrivenBindings: Int
    let unresolvedBindings: Int

    var allBindingsMatch: Bool { matchingBindings == totalBindings }
    var allBindingsCatalogDriven: Bool { catalogDrivenBindings == totalBindings }
}

struct ContractBinding: Codable, Sendable {
    let agentID: String
    let outputName: String
    let v1ContractID: String?
    let v2ContractID: String?
    let bindingsMatch: Bool
    let source: BindingSource
}

enum BindingSource: String, Codable, Sendable {
    case catalogDriven = "catalog_driven"
    case unresolved
}
