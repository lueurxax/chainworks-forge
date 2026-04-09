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
    /// Used by `GooseExtensionRegistryReader.snapshot()` to avoid
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

struct GooseExtensionRegistryReader: RuntimeExtensionRegistryProvider, Sendable {
    static let environmentConfigPathKey = "CHAINWORKS_GOOSE_CONFIG_PATH"
    let configURL: URL

    nonisolated init(configURL: URL? = nil) {
        if let configURL {
            self.configURL = configURL
            return
        }

        let environment = ProcessInfo.processInfo.environment
        let isTestHost = environment["XCTestConfigurationFilePath"] != nil
        let usesInMemoryStore = environment["CHAINWORKS_IN_MEMORY_STORE"] == "1"
        if isTestHost || usesInMemoryStore {
            // Nonisolated fixture URL resolution. Navigates 3 levels up from this
            // source file to the repo root (Engine -> app target -> project -> repo).
            let fixtureURL = URL(fileURLWithPath: #filePath)
                .deletingLastPathComponent()
                .deletingLastPathComponent()
                .deletingLastPathComponent()
                .appendingPathComponent("examples/goose/goose-config-fixture.yaml")
            if FileManager.default.isReadableFile(atPath: fixtureURL.path) {
                self.configURL = fixtureURL
                return
            }
        }

        if let environmentPath = ProcessInfo.processInfo.environment["CHAINWORKS_GOOSE_CONFIG_PATH"]?
            .trimmingCharacters(in: .whitespacesAndNewlines),
           !environmentPath.isEmpty {
            self.configURL = URL(fileURLWithPath: environmentPath)
            return
        }

        self.configURL = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".config/goose/config.yaml")
    }

    nonisolated func snapshot() throws -> RuntimeExtensionRegistrySnapshot {
        let contents = try String(contentsOf: configURL, encoding: .utf8)
        // Use raw YAML load to avoid @MainActor-inferred Decodable conformance issues.
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
            if runtimeNamespace == "goose", runtimeRegistry == nil {
                blockingIssues.append("Goose extension registry is unavailable; cannot validate MCP profile '\(profileID)' for agent '\(agent.id)'.")
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
        if runtimeNamespace == "goose" {
            guard let runtimeRegistry else {
                return .missing("MCP server '\(serverID)' cannot be validated because Goose extension registry is unavailable.")
            }
            guard runtimeRegistry.configsByRuntimeID[runtimeID] != nil else {
                return .missing("MCP server '\(serverID)' maps to runtime extension '\(runtimeID)', but that extension is not installed in Goose.")
            }
        }
        return .available(runtimeID: runtimeID)
    }
}

private enum MCPServerResolution {
    case available(runtimeID: String)
    case missing(String)
}
