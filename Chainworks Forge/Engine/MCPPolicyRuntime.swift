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

struct RuntimeExtensionDefinition: Codable, Equatable, Sendable {
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

    init(
        enabled: Bool? = nil,
        type: String? = nil,
        name: String,
        description: String? = nil,
        displayName: String? = nil,
        cmd: String? = nil,
        args: [String]? = nil,
        envs: [String: String]? = nil,
        envKeys: [String]? = nil,
        timeout: Int? = nil,
        bundled: Bool? = nil,
        availableTools: [String]? = nil
    ) {
        self.enabled = enabled
        self.type = type
        self.name = name
        self.description = description
        self.displayName = displayName
        self.cmd = cmd
        self.args = args
        self.envs = envs
        self.envKeys = envKeys
        self.timeout = timeout
        self.bundled = bundled
        self.availableTools = availableTools
    }

    /// Initialise from a raw YAML dictionary (as returned by `Yams.load`).
    /// Used by extension registry reader `snapshot()` to avoid
    /// `Decodable` conformance actor-isolation issues in nonisolated contexts.
    nonisolated fileprivate init?(rawYAML dict: [String: Any]) {
        guard let name = dict["name"] as? String else { return nil }
        self.name = name
        self.enabled = dict["enabled"] as? Bool
        self.type = dict["type"] as? String
        self.description = dict["description"] as? String
        self.displayName = dict["display_name"] as? String
        self.cmd = dict["cmd"] as? String
        self.args = dict["args"] as? [String]
        self.envs = dict["envs"] as? [String: String]
        self.envKeys = dict["env_keys"] as? [String]
        self.timeout = (dict["timeout"] as? Int) ?? (dict["timeout"] as? Double).map(Int.init)
        self.bundled = dict["bundled"] as? Bool
        self.availableTools = dict["available_tools"] as? [String]
    }
}

struct RuntimeExtensionRegistrySnapshot: Equatable, Sendable {
    let configURL: URL
    let installedExtensionIDs: [String]
    let enabledExtensionIDs: [String]
    let configsByRuntimeID: [String: RuntimeExtensionDefinition]
}

private enum RuntimeExtensionRegistryConfigResolver {
    nonisolated static func fixtureURLIfPresent() -> URL? {
        let repoRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let candidate = repoRoot.appendingPathComponent("examples/mcp/mcp-config-fixture.yaml")
        return FileManager.default.isReadableFile(atPath: candidate.path) ? candidate : nil
    }

    nonisolated static func defaultConfigURL(
        primaryEnvironmentKey: String,
        secondaryEnvironmentKey: String? = nil,
        defaultPath: String
    ) -> URL {
        let environment = ProcessInfo.processInfo.environment
        let isTestHost = environment["XCTestConfigurationFilePath"] != nil
        let usesInMemoryStore = environment["CHAINWORKS_IN_MEMORY_STORE"] == "1"
        if (isTestHost || usesInMemoryStore), let fixtureURL = fixtureURLIfPresent() {
            return fixtureURL
        }

        if let explicitPath = environment[primaryEnvironmentKey]?
            .trimmingCharacters(in: .whitespacesAndNewlines),
           !explicitPath.isEmpty {
            return URL(fileURLWithPath: explicitPath)
        }

        if let secondaryEnvironmentKey,
           let fallbackPath = environment[secondaryEnvironmentKey]?
            .trimmingCharacters(in: .whitespacesAndNewlines),
           !fallbackPath.isEmpty {
            return URL(fileURLWithPath: fallbackPath)
        }

        return FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(defaultPath)
    }

    nonisolated static func loadSnapshot(configURL: URL) throws -> RuntimeExtensionRegistrySnapshot {
        let contents = try String(contentsOf: configURL, encoding: .utf8)
        let extensions: [String: RuntimeExtensionDefinition]
        if let root = try Yams.load(yaml: contents) as? [String: Any],
           let rawExtensions = root["extensions"] as? [String: Any] {
            extensions = rawExtensions.compactMapValues { value -> RuntimeExtensionDefinition? in
                guard let dict = value as? [String: Any] else { return nil }
                return RuntimeExtensionDefinition(rawYAML: dict)
            }
        } else {
            extensions = [:]
        }
        let installed = extensions.keys.sorted()
        let enabled = extensions
            .compactMap { key, value in value.enabled == true ? key : nil }
            .sorted()
        return RuntimeExtensionRegistrySnapshot(
            configURL: configURL,
            installedExtensionIDs: installed,
            enabledExtensionIDs: enabled,
            configsByRuntimeID: extensions
        )
    }
}

// MARK: - CodexExtensionRegistryReader (Proposal 029)

/// Codex-specific RuntimeExtensionRegistryProvider conformer.
/// Codex uses MCP natively, so it reads from a shared config format
/// and applies Codex-specific extension ID mappings. This ensures the MCP
/// policy resolver can validate extension availability against a Codex runtime.
struct CodexExtensionRegistryReader: RuntimeExtensionRegistryProvider, Sendable {
    static let environmentConfigPathKey = "CHAINWORKS_CODEX_CONFIG_PATH"
    let configURL: URL

    nonisolated init(configURL: URL? = nil) {
        if let configURL {
            self.configURL = configURL
            return
        }

        let preferred = RuntimeExtensionRegistryConfigResolver.defaultConfigURL(
            primaryEnvironmentKey: Self.environmentConfigPathKey,
            defaultPath: ".config/mcp/config.yaml"
        )
        if FileManager.default.isReadableFile(atPath: preferred.path) {
            self.configURL = preferred
            return
        }

        // ACP canonical path is ~/.config/mcp/config.yaml, but operator machines may
        // still keep the shared runtime registry at the historical filesystem
        // location while runtime execution is already ACP-only.
        let legacySharedStore = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".config/goose/config.yaml")
        self.configURL = legacySharedStore
    }

    nonisolated func snapshot() throws -> RuntimeExtensionRegistrySnapshot {
        try RuntimeExtensionRegistryConfigResolver.loadSnapshot(configURL: configURL)
    }

    /// RuntimeExtensionRegistryProvider conformance.
    nonisolated func registrySnapshot() throws -> RuntimeExtensionRegistrySnapshot {
        try snapshot()
    }
}

struct MCPPolicyResolver: Sendable {
    func resolve(
        agent: ResolvedAgent,
        catalog: AgentCatalog,
        providerBinding: ResolvedProviderBinding?,
        runtimeRegistry: RuntimeExtensionRegistrySnapshot?,
        runtimeNamespaceOverride: String? = nil
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
        let runtimeNamespace = runtimeNamespaceOverride ?? runtimeNamespace(for: providerBinding)

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
            if runtimeRegistry == nil {
                blockingIssues.append("Runtime extension registry is unavailable; cannot validate MCP profile '\(profileID)' for agent '\(agent.id)'.")
            }
        }

        for serverID in profile.requiredExtensions {
            switch resolveServer(
                serverID: serverID,
                runtimeNamespace: runtimeNamespace,
                registry: catalog.mcpServerRegistry,
                runtimeRegistry: runtimeRegistry
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
                runtimeRegistry: runtimeRegistry
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
        providerBinding?.effectiveRuntimeNamespace
    }

    private func resolveServer(
        serverID: String,
        runtimeNamespace: String?,
        registry: [String: MCPServerRegistryEntry],
        runtimeRegistry: RuntimeExtensionRegistrySnapshot?
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
        if let runtimeRegistry {
            guard runtimeRegistry.configsByRuntimeID[runtimeID] != nil else {
                return .missing("MCP server '\(serverID)' maps to runtime extension '\(runtimeID)', but that extension is not installed in the runtime registry.")
            }
        }
        return .available(runtimeID: runtimeID)
    }
}

private enum MCPServerResolution {
    case available(runtimeID: String)
    case missing(String)
}
