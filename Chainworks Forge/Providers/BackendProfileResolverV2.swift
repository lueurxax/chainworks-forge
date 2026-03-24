import Foundation

struct ResolvedProviderBinding: Codable, Equatable, Sendable {
    let agentID: String
    let backendProfileID: String?
    let configuredProviderID: UUID
    let providerFamily: String
    let providerIdentifier: String
    let model: String
    let effort: String
    let transport: String
    let adapterVersion: String
}

enum BackendProfileResolverError: Error, LocalizedError {
    case unknownProviderFamily(String)
    case noConfiguredProvider(ProviderFamily)
    case configuredProviderNotFound(UUID)
    case missingModel(agentID: String)

    var errorDescription: String? {
        switch self {
        case .unknownProviderFamily(let provider):
            return "Provider family '\(provider)' is not configured for Proposal 006"
        case .noConfiguredProvider(let family):
            return "No configured provider available for \(family.displayName)"
        case .configuredProviderNotFound(let id):
            return "Configured provider \(id.uuidString) could not be found"
        case .missingModel(let agentID):
            return "Resolved model is missing for agent \(agentID)"
        }
    }
}

struct BackendProfileResolverV2 {
    let providerRegistry: ProviderRegistry

    func resolveBindings(
        plan: RunPlan,
        startOptions: RunStartOptions
    ) throws -> [String: ResolvedProviderBinding] {
        var bindings: [String: ResolvedProviderBinding] = [:]

        for (agentID, agent) in plan.agentBindings {
            guard let family = ProviderFamily.from(runtimeIdentifier: agent.provider) else {
                throw BackendProfileResolverError.unknownProviderFamily(agent.provider)
            }

            let override = agent.backendProfileID.flatMap { startOptions.overridesByBackendProfileID[$0] }
            let configuredProvider: ConfiguredProvider
            if let configuredProviderID = override?.configuredProviderID {
                guard let resolved = providerRegistry.configuredProvider(id: configuredProviderID) else {
                    throw BackendProfileResolverError.configuredProviderNotFound(configuredProviderID)
                }
                configuredProvider = resolved
            } else if let preferred = providerRegistry.preferredProvider(for: family) {
                configuredProvider = preferred
            } else {
                throw BackendProfileResolverError.noConfiguredProvider(family)
            }

            let resolvedModel = override?.model ?? configuredProvider.defaultModel ?? agent.model
            guard !resolvedModel.isEmpty else {
                throw BackendProfileResolverError.missingModel(agentID: agentID)
            }

            bindings[agentID] = ResolvedProviderBinding(
                agentID: agentID,
                backendProfileID: agent.backendProfileID,
                configuredProviderID: configuredProvider.id,
                providerFamily: configuredProvider.family.rawValue,
                providerIdentifier: configuredProvider.family.runtimeProviderIdentifier,
                model: resolvedModel,
                effort: override?.effort ?? agent.effort,
                transport: configuredProvider.transport.rawValue,
                adapterVersion: configuredProvider.adapterVersion
            )
        }

        return bindings
    }
}
