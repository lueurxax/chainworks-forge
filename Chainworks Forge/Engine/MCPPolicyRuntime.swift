import Foundation
import Yams

enum MCPFallbackPolicy: String, Codable, Sendable {
    case failIfRequiredMissing = "fail_if_required_missing"
    case allowWithoutExtensions = "allow_without_extensions"
}

struct MCPPolicyResolutionReport: Codable, Equatable, Sendable {
    let profileID: String
    let requiredExtensions: [String]
    let optionalExtensions: [String]
    let requestedExtensions: [String]
    let requiredRuntimeExtensionIDs: [String]
    let optionalRuntimeExtensionIDs: [String]
    let predictedEffectiveExtensions: [String]
    let predictedEffectiveRuntimeExtensionIDs: [String]
    let deniedExtensions: [String]
    let warnings: [String]
    let blockingIssues: [String]

    static let none = MCPPolicyResolutionReport(
        profileID: "none",
        requiredExtensions: [],
        optionalExtensions: [],
        requestedExtensions: [],
        requiredRuntimeExtensionIDs: [],
        optionalRuntimeExtensionIDs: [],
        predictedEffectiveExtensions: [],
        predictedEffectiveRuntimeExtensionIDs: [],
        deniedExtensions: [],
        warnings: [],
        blockingIssues: []
    )
}

struct GooseExtensionDefinition: Codable, Equatable, Sendable {
    let enabled: Bool?
    let type: String?
    let name: String
    let description: String?
    let displayName: String?
    let cmd: String?
    let args: [String]?
    let envs: [String: String]?
    let envKeys: [String]?
    let timeout: Int?
    let bundled: Bool?
    let availableTools: [String]?

    enum CodingKeys: String, CodingKey {
        case enabled
        case type
        case name
        case description
        case displayName = "display_name"
        case cmd
        case args
        case envs
        case envKeys = "env_keys"
        case timeout
        case bundled
        case availableTools = "available_tools"
    }
}

private struct GooseExtensionConfigFile: Codable {
    let extensions: [String: GooseExtensionDefinition]
}

struct GooseExtensionRegistrySnapshot: Equatable, Sendable {
    let configURL: URL
    let installedExtensionIDs: [String]
    let enabledExtensionIDs: [String]
    let configsByRuntimeID: [String: GooseExtensionDefinition]
}

struct GooseExtensionRegistryReader: Sendable {
    let configURL: URL

    init(configURL: URL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".config/goose/config.yaml")) {
        self.configURL = configURL
    }

    func snapshot() throws -> GooseExtensionRegistrySnapshot {
        let contents = try String(contentsOf: configURL, encoding: .utf8)
        let decoded = try YAMLDecoder().decode(GooseExtensionConfigFile.self, from: contents)
        let installed = decoded.extensions.keys.sorted()
        let enabled = decoded.extensions
            .compactMap { key, value in value.enabled == true ? key : nil }
            .sorted()
        return GooseExtensionRegistrySnapshot(
            configURL: configURL,
            installedExtensionIDs: installed,
            enabledExtensionIDs: enabled,
            configsByRuntimeID: decoded.extensions
        )
    }
}

struct MCPPolicyResolver: Sendable {
    func resolve(
        agent: ResolvedAgent,
        catalog: AgentCatalog,
        providerBinding: ResolvedProviderBinding?,
        gooseRegistry: GooseExtensionRegistrySnapshot?
    ) -> MCPPolicyResolutionReport {
        let defaultProfile = catalog.mcpPolicy.defaultProfile
        let profileID = (agent.mcpProfileID?.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty == false)
            ? agent.mcpProfileID!
            : defaultProfile

        if profileID == "none" {
            return .none
        }

        guard let profile = catalog.mcpProfiles[profileID] else {
            return MCPPolicyResolutionReport(
                profileID: profileID,
                requiredExtensions: [],
                optionalExtensions: [],
                requestedExtensions: [],
                requiredRuntimeExtensionIDs: [],
                optionalRuntimeExtensionIDs: [],
                predictedEffectiveExtensions: [],
                predictedEffectiveRuntimeExtensionIDs: [],
                deniedExtensions: [],
                warnings: [],
                blockingIssues: ["Agent '\(agent.id)' references unknown MCP profile '\(profileID)'."]
            )
        }

        let fallback = MCPFallbackPolicy(rawValue: profile.fallbackPolicy) ?? .failIfRequiredMissing
        let runtimeNamespace = runtimeNamespace(for: providerBinding)

        var requiredRuntimeIDs: [String] = []
        var optionalRuntimeIDs: [String] = []
        var effectiveExtensions: [String] = []
        var effectiveRuntimeIDs: [String] = []
        var deniedExtensions: [String] = []
        var warnings: [String] = []
        var blockingIssues: [String] = []

        if !profile.allRequestedExtensions.isEmpty {
            if runtimeNamespace == nil {
                blockingIssues.append("Provider runtime cannot reconcile session-scoped MCP extensions for agent '\(agent.id)'.")
            }
            if gooseRegistry == nil {
                blockingIssues.append("Goose extension registry is unavailable; cannot validate MCP profile '\(profileID)' for agent '\(agent.id)'.")
            }
        }

        for serverID in profile.requiredExtensions {
            switch resolveServer(
                serverID: serverID,
                runtimeNamespace: runtimeNamespace,
                registry: catalog.mcpServerRegistry,
                gooseRegistry: gooseRegistry
            ) {
            case .available(let runtimeID):
                requiredRuntimeIDs.append(runtimeID)
                effectiveExtensions.append(serverID)
                effectiveRuntimeIDs.append(runtimeID)
            case .missing(let message):
                deniedExtensions.append(serverID)
                if fallback == .failIfRequiredMissing {
                    blockingIssues.append(message)
                } else {
                    warnings.append(message)
                }
            }
        }

        for serverID in profile.optionalExtensions {
            switch resolveServer(
                serverID: serverID,
                runtimeNamespace: runtimeNamespace,
                registry: catalog.mcpServerRegistry,
                gooseRegistry: gooseRegistry
            ) {
            case .available(let runtimeID):
                optionalRuntimeIDs.append(runtimeID)
                effectiveExtensions.append(serverID)
                effectiveRuntimeIDs.append(runtimeID)
            case .missing(let message):
                deniedExtensions.append(serverID)
                warnings.append(message)
            }
        }

        return MCPPolicyResolutionReport(
            profileID: profileID,
            requiredExtensions: profile.requiredExtensions,
            optionalExtensions: profile.optionalExtensions,
            requestedExtensions: profile.allRequestedExtensions,
            requiredRuntimeExtensionIDs: Array(Set(requiredRuntimeIDs)).sorted(),
            optionalRuntimeExtensionIDs: Array(Set(optionalRuntimeIDs)).sorted(),
            predictedEffectiveExtensions: Array(Set(effectiveExtensions)).sorted(),
            predictedEffectiveRuntimeExtensionIDs: Array(Set(effectiveRuntimeIDs)).sorted(),
            deniedExtensions: Array(Set(deniedExtensions)).sorted(),
            warnings: Array(Set(warnings)).sorted(),
            blockingIssues: Array(Set(blockingIssues)).sorted()
        )
    }

    private func runtimeNamespace(for providerBinding: ResolvedProviderBinding?) -> String? {
        guard let providerBinding else { return nil }
        return providerBinding.transport == ProviderTransport.gooseServer.rawValue ? "goose" : nil
    }

    private func resolveServer(
        serverID: String,
        runtimeNamespace: String?,
        registry: [String: MCPServerRegistryEntry],
        gooseRegistry: GooseExtensionRegistrySnapshot?
    ) -> MCPServerResolution {
        guard let entry = registry[serverID] else {
            return .missing("MCP server '\(serverID)' is not declared in mcp_server_registry.")
        }
        guard let runtimeNamespace else {
            return .missing("MCP server '\(serverID)' requires a runtime with session-scoped MCP reconciliation support.")
        }
        guard let runtimeID = entry.runtimeIDs[runtimeNamespace], !runtimeID.isEmpty else {
            return .missing("MCP server '\(serverID)' has no runtime mapping for '\(runtimeNamespace)'.")
        }
        guard let gooseRegistry else {
            return .missing("MCP server '\(serverID)' cannot be validated because Goose extension registry is unavailable.")
        }
        guard gooseRegistry.configsByRuntimeID[runtimeID] != nil else {
            return .missing("MCP server '\(serverID)' maps to runtime extension '\(runtimeID)', but that extension is not installed in Goose.")
        }
        return .available(runtimeID: runtimeID)
    }
}

private enum MCPServerResolution {
    case available(runtimeID: String)
    case missing(String)
}
