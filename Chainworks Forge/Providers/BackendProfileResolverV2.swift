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

    /// Proposal 026: Resolved runtime profile identifier.
    let runtimeProfileID: String?
    /// Proposal 026: Adapter family from the resolved runtime profile.
    let adapterFamily: String?
    /// Proposal 026: Capability class from the resolved runtime profile.
    let capabilityClass: RuntimeCapabilityClass?

    init(
        agentID: String,
        backendProfileID: String?,
        configuredProviderID: UUID,
        providerFamily: String,
        providerIdentifier: String,
        model: String,
        effort: String,
        transport: String,
        adapterVersion: String,
        runtimeProfileID: String? = nil,
        adapterFamily: String? = nil,
        capabilityClass: RuntimeCapabilityClass? = nil
    ) {
        self.agentID = agentID
        self.backendProfileID = backendProfileID
        self.configuredProviderID = configuredProviderID
        self.providerFamily = providerFamily
        self.providerIdentifier = providerIdentifier
        self.model = model
        self.effort = effort
        self.transport = transport
        self.adapterVersion = adapterVersion
        self.runtimeProfileID = runtimeProfileID
        self.adapterFamily = adapterFamily
        self.capabilityClass = capabilityClass
    }

    var effectiveRuntimeNamespace: String? {
        switch adapterFamily {
        case nil, "", "goose":
            return transport == ProviderTransport.gooseServer.rawValue ? "goose" : nil
        case "claude_agent_acp":
            return "claude_agent"
        case "gemini_cli_acp":
            return "gemini_cli"
        case "codex_acp":
            return "codex"
        default:
            return nil
        }
    }

    var usesGooseExecutionPath: Bool {
        effectiveRuntimeNamespace == "goose"
    }

    // MARK: - Proposal 011 (REQ-010): Cross-family coherence check

    /// Heuristic check for obvious cross-family provider/model mismatches.
    /// Returns `true` when the resolved model name appears to belong to a different
    /// provider family than the one actually serving the request.
    var hasCrossFamilyMismatch: Bool {
        let lowerModel = model.lowercased()
        let lowerFamily = providerFamily.lowercased()
        let familyModelPrefixes: [([String], [String])] = [
            (["claude"], ["claude", "anthropic"]),
            (["openai", "codex"], ["gpt", "o1", "o3", "chatgpt"]),
            (["gemini"], ["gemini", "palm"]),
        ]
        for (familyAliases, prefixes) in familyModelPrefixes {
            let modelBelongsToFamily = prefixes.contains(where: { lowerModel.hasPrefix($0) })
            let familyMatches = familyAliases.contains(where: { lowerFamily.contains($0) })
            if modelBelongsToFamily && !familyMatches {
                return true
            }
        }
        return false
    }
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
        startOptions: RunStartOptions,
        runtimeProfiles: [String: RuntimeProfile] = [:]
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

            let resolvedModel = override?.model ?? agent.model.ifNotEmpty ?? configuredProvider.defaultModel ?? ""
            guard !resolvedModel.isEmpty else {
                throw BackendProfileResolverError.missingModel(agentID: agentID)
            }

            // Proposal 026: Resolve runtime profile from agent's runtimeProfileID
            let resolvedRuntimeProfileID: String?
            let resolvedAdapterFamily: String
            let resolvedCapabilityClass: RuntimeCapabilityClass
            let resolvedTransportKind: String
            if let profileKey = agent.runtimeProfileID,
               let profile = runtimeProfiles[profileKey] {
                resolvedRuntimeProfileID = profileKey
                resolvedAdapterFamily = profile.adapterFamily
                resolvedCapabilityClass = profile.capabilityClass
                resolvedTransportKind = profile.transportKind
            } else {
                // Default: legacy Goose runtime
                resolvedRuntimeProfileID = agent.runtimeProfileID
                resolvedAdapterFamily = "goose"
                resolvedCapabilityClass = .legacyOperatorGrade
                resolvedTransportKind = configuredProvider.transport.rawValue
            }

            bindings[agentID] = ResolvedProviderBinding(
                agentID: agentID,
                backendProfileID: agent.backendProfileID,
                configuredProviderID: configuredProvider.id,
                providerFamily: configuredProvider.family.rawValue,
                providerIdentifier: configuredProvider.family.runtimeProviderIdentifier,
                model: resolvedModel,
                effort: override?.effort ?? agent.effort,
                transport: resolvedTransportKind,
                adapterVersion: configuredProvider.adapterVersion,
                runtimeProfileID: resolvedRuntimeProfileID,
                adapterFamily: resolvedAdapterFamily,
                capabilityClass: resolvedCapabilityClass
            )
        }

        return bindings
    }

    /// Proposal 011 (REQ-009): Build frozen provenance for each agent binding.
    /// The three frozen inputs (backend profile, configured provider, run override)
    /// guarantee that provenance is always determinable.
    func resolveProvenances(
        plan: RunPlan,
        startOptions: RunStartOptions
    ) -> [String: FrozenBindingProvenance] {
        var provenances: [String: FrozenBindingProvenance] = [:]

        for (agentID, agent) in plan.agentBindings {
            guard let family = ProviderFamily.from(runtimeIdentifier: agent.provider) else { continue }

            let override = agent.backendProfileID.flatMap { startOptions.overridesByBackendProfileID[$0] }
            let configuredProvider = override?.configuredProviderID
                .flatMap { providerRegistry.configuredProvider(id: $0) }
                ?? providerRegistry.preferredProvider(for: family)

            let overrideModel = override?.model
            let providerDefaultModel = configuredProvider?.defaultModel
            let backendProfileModel = agent.model

            // Determine provenance source from the three frozen inputs.
            let source: BindingProvenanceSource
            let resolvedModel: String

            if let explicitOverride = overrideModel, !explicitOverride.isEmpty {
                source = .runOverride
                resolvedModel = explicitOverride
            } else if !backendProfileModel.isEmpty {
                source = .backendProfileDefault
                resolvedModel = backendProfileModel
            } else if let provDefault = providerDefaultModel, !provDefault.isEmpty {
                source = .configuredProviderDefault
                resolvedModel = provDefault
            } else {
                // Contract: this should not happen because resolveBindings() already
                // validates model presence. Record as unverifiable — never a false source.
                source = .unverifiable
                resolvedModel = "unknown"
            }

            provenances[agentID] = FrozenBindingProvenance(
                source: source,
                backendProfileID: agent.backendProfileID ?? agentID,
                backendProfileModel: backendProfileModel,
                configuredProviderID: configuredProvider?.id,
                configuredProviderDefaultModel: providerDefaultModel,
                runOverrideModel: overrideModel,
                resolvedModel: resolvedModel,
                resolvedProviderFamily: configuredProvider?.family.rawValue ?? agent.provider
            )
        }

        return provenances
    }
}

private extension String {
    var ifNotEmpty: String? {
        let trimmed = trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : self
    }
}
