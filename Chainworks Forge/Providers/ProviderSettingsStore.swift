import Foundation
import Observation

@Observable
final class ProviderSettingsStore {
    private let fileURL: URL
    @MainActor private(set) var settings: ProviderSettings
    @MainActor private(set) var diagnosticsMessage: String?

    @MainActor
    init(fileURL: URL? = nil, initialSettings: ProviderSettings? = nil) {
        let resolvedURL = fileURL ?? Self.defaultFileURL()
        self.fileURL = resolvedURL
        self.diagnosticsMessage = nil

        if let initialSettings {
            self.settings = Self.sanitized(initialSettings)
            persistOrRecordFailure(
                summary: "Failed to persist provider settings during initialization"
            )
            return
        }

        let fileExists = FileManager.default.fileExists(atPath: resolvedURL.path)
        if let loaded = try? Self.load(from: resolvedURL) {
            let migrated = Self.migratedIfNeeded(loaded)
            self.settings = Self.sanitized(migrated)
            if self.settings != loaded {
                persistOrRecordFailure(
                    summary: "Failed to persist normalized provider settings"
                )
            } else {
                clearDiagnostics()
            }
        } else {
            self.settings = Self.sanitized(Self.seededDefault())
            if fileExists {
                recordFailure(
                    summary: "Failed to load persisted provider settings; using defaults instead",
                    error: ProviderSettingsStoreError.loadFailed(resolvedURL.path)
                )
                do {
                    try persist()
                } catch {
                    recordFailure(summary: "Failed to persist fallback provider settings", error: error)
                }
                return
            }
            persistOrRecordFailure(summary: "Failed to create default provider settings store")
        }
    }

    @MainActor
    func replace(with settings: ProviderSettings) {
        self.settings = Self.sanitized(settings)
        persistOrRecordFailure(summary: "Failed to save provider settings")
    }

    @MainActor
    func upsert(provider: ConfiguredProvider) {
        var providers = settings.configuredProviders
        let provider = Self.sanitized(provider)
        if let index = providers.firstIndex(where: { $0.id == provider.id }) {
            providers[index] = provider
        } else {
            providers.append(provider)
        }
        settings.configuredProviders = providers.sorted { $0.displayName < $1.displayName }

        if settings.preferredProviderIDsByFamily[provider.family.rawValue] == nil {
            settings.preferredProviderIDsByFamily[provider.family.rawValue] = provider.id
        }
        persistOrRecordFailure(summary: "Failed to save provider settings")
    }

    @MainActor
    func removeProvider(id: UUID) {
        guard let provider = settings.configuredProviders.first(where: { $0.id == id }) else { return }
        settings.configuredProviders.removeAll { $0.id == id }
        if settings.preferredProviderIDsByFamily[provider.family.rawValue] == id {
            settings.preferredProviderIDsByFamily[provider.family.rawValue] =
                settings.configuredProviders.first(where: { $0.family == provider.family && $0.isEnabled })?.id
        }
        persistOrRecordFailure(summary: "Failed to save provider settings")
    }

    @MainActor
    func setPreferredProvider(id: UUID, for family: ProviderFamily) {
        settings.preferredProviderIDsByFamily[family.rawValue] = id
        persistOrRecordFailure(summary: "Failed to save provider settings")
    }

    private func persist() throws {
        let directory = fileURL.deletingLastPathComponent()
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        let data = try encoder.encode(settings)
        try data.write(to: fileURL, options: .atomic)
    }

    private static func load(from fileURL: URL) throws -> ProviderSettings {
        let raw = try Data(contentsOf: fileURL)
        let migrated: Data
        do {
            migrated = try migrateRawProviderSettings(raw)
        } catch {
            ForgeLogger.app.error("Provider settings raw migration failed for \(fileURL.path): \(error.localizedDescription)")
            migrated = raw
        }
        return try JSONDecoder().decode(ProviderSettings.self, from: migrated)
    }

    @MainActor
    private func persistOrRecordFailure(summary: String) {
        do {
            try persist()
            clearDiagnostics()
        } catch {
            recordFailure(summary: summary, error: error)
        }
    }

    @MainActor
    private func recordFailure(summary: String, error: Error) {
        let message = "\(summary): \(error.localizedDescription)"
        diagnosticsMessage = message
        ForgeLogger.app.error(message)
    }

    @MainActor
    private func clearDiagnostics() {
        diagnosticsMessage = nil
    }

    // MARK: - Raw Goose-era Migration (P033)

    /// Pre-decode migration for local `provider-settings.json`.
    /// Rewrites Goose-era `ProviderFamily` and `ProviderTransport` raw values before
    /// `JSONDecoder` touches the payload (deleted enum cases cause decode failures).
    static func migrateRawProviderSettings(_ data: Data) throws -> Data {
        guard var json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { return data }
        guard (json["migration_version"] as? Int ?? 0) < 1 else { return data }
        migrateProviderSettingsFields(&json)
        json["migration_version"] = 1
        return try JSONSerialization.data(withJSONObject: json)
    }

    /// Pre-decode migration for imported `chainworks-settings.json` (ExportableSettingsPackage shape).
    /// Rewrites the nested `providerSettings` dict and trims deleted-UUID placeholders.
    static func migrateRawTransferPackage(_ data: Data) throws -> Data {
        guard var json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { return data }
        guard var nested = json["providerSettings"] as? [String: Any] else { return data }
        guard (nested["migration_version"] as? Int ?? 0) < 1 else { return data }
        let deletedIDs = gooseCodexUUIDs(from: nested)
        migrateProviderSettingsFields(&nested)
        nested["migration_version"] = 1
        json["providerSettings"] = nested
        if var placeholders = json["secretPlaceholders"] as? [String] {
            placeholders.removeAll { key in deletedIDs.contains { key.contains($0) } }
            json["secretPlaceholders"] = placeholders
        }
        return try JSONSerialization.data(withJSONObject: json)
    }

    /// Shared raw field rewriter — operates on a `ProviderSettings`-shaped dict.
    private static func migrateProviderSettingsFields(_ json: inout [String: Any]) {
        var providers = json["configuredProviders"] as? [[String: Any]] ?? []

        providers = providers.compactMap { provider -> [String: Any]? in
            var p = provider
            guard let family = p["family"] as? String else { return p }

            switch family {
            case "codex":
                return nil  // Goose-backed Codex deleted — no ACP continuation
            case "claude":
                p["family"] = "claudeACP"
                p["transport"] = "cli"
                p["endpoint"] = NSNull()
                if (p["displayName"] as? String) == "Claude Goose" { p["displayName"] = "Claude ACP" }
                p["capabilities"] = encodedCapabilitiesJSON(.default(for: .claudeACP))
                p["adapterVersion"] = "acp-v1"
                p["isEnabled"] = true
            case "gemini":
                p["family"] = "geminiACP"
                p["transport"] = "cli"
                p["endpoint"] = NSNull()
                if (p["displayName"] as? String) == "Gemini Goose" { p["displayName"] = "Gemini ACP" }
                p["capabilities"] = encodedCapabilitiesJSON(.default(for: .geminiACP))
                p["adapterVersion"] = "acp-v1"
                p["isEnabled"] = true
            default:
                if (p["transport"] as? String) == "goose_server" { p["transport"] = "cli" }
            }
            return p
        }

        json["configuredProviders"] = providers
        if json["notificationOnProviderFailure"] == nil {
            json["notificationOnProviderFailure"] = true
        }
        if json["runStartRequiresCleanPreflight"] == nil {
            json["runStartRequiresCleanPreflight"] = true
        }

        if var preferred = json["preferredProviderIDsByFamily"] as? [String: Any] {
            preferred.removeValue(forKey: "codex")
            if let v = preferred.removeValue(forKey: "claude") { preferred["claudeACP"] = v }
            if let v = preferred.removeValue(forKey: "gemini") { preferred["geminiACP"] = v }
            json["preferredProviderIDsByFamily"] = preferred
        } else {
            json["preferredProviderIDsByFamily"] = [:]
        }
    }

    /// Collects UUID strings from Goose-era Codex rows (for transfer placeholder cleanup).
    private static func gooseCodexUUIDs(from providerSettingsJSON: [String: Any]) -> [String] {
        guard let providers = providerSettingsJSON["configuredProviders"] as? [[String: Any]] else { return [] }
        return providers.compactMap { p -> String? in
            guard (p["family"] as? String) == "codex", let id = p["id"] as? String else { return nil }
            return id
        }
    }

    private static func encodedCapabilitiesJSON(_ capabilities: ProviderCapabilities) -> [String: Any] {
        let encoder = JSONEncoder()
        guard
            let data = try? encoder.encode(capabilities),
            let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else {
            return [:]
        }
        return object
    }

    private static func sanitized(_ settings: ProviderSettings) -> ProviderSettings {
        ProviderSettings(
            configuredProviders: settings.configuredProviders.map { sanitized($0) },
            preferredProviderIDsByFamily: settings.preferredProviderIDsByFamily,
            notificationOnProviderFailure: settings.notificationOnProviderFailure,
            runStartRequiresCleanPreflight: settings.runStartRequiresCleanPreflight
        )
    }

    private static func migratedIfNeeded(_ settings: ProviderSettings) -> ProviderSettings {
        guard settings.configuredProviders.isEmpty,
              settings.preferredProviderIDsByFamily.isEmpty else {
            return settings
        }
        return seededDefault()
    }

    private static func sanitized(_ provider: ConfiguredProvider) -> ConfiguredProvider {
        var provider = provider
        provider.defaultModel = ProviderDefaults.canonicalModel(
            provider.defaultModel,
            for: provider.family,
            transport: provider.transport
        )

        if let defaultModel = provider.defaultModel,
           !ProviderDefaults.model(defaultModel, isCompatibleWith: provider.family) {
            provider.defaultModel = ProviderDefaults.defaultModel(for: provider.family)
        }

        let generatedNamesForOtherFamilies = ProviderFamily.allCases
            .filter { $0 != provider.family }
            .map { ProviderDefaults.generatedDisplayName(for: $0, transport: provider.transport) }
        if generatedNamesForOtherFamilies.contains(provider.displayName) {
            provider.displayName = ProviderDefaults.generatedDisplayName(for: provider.family, transport: provider.transport)
        }
        return provider
    }

    static func defaultFileURL() -> URL {
        AppConfiguration.defaultSupportRoot()
            .appendingPathComponent("provider-settings.json")
    }

    private static func seededDefault() -> ProviderSettings {
        let environment = ProcessInfo.processInfo.environment
        var providers: [ConfiguredProvider] = [
            ConfiguredProvider(
                family: .codexACP,
                displayName: "Codex ACP",
                transport: .cli,
                authMode: .apiKey,
                defaultModel: "gpt-5"
            ),
            ConfiguredProvider(
                family: .claudeACP,
                displayName: "Claude ACP",
                transport: .cli,
                authMode: .apiKey,
                defaultModel: "sonnet"
            ),
            ConfiguredProvider(
                family: .geminiACP,
                displayName: "Gemini ACP",
                transport: .cli,
                authMode: .apiKey,
                defaultModel: "gemini-3.1-pro-preview"
            ),
            ConfiguredProvider(
                family: .auggie,
                displayName: "Auggie CLI",
                transport: .cli,
                authMode: .apiKey,
                defaultModel: "auggie-default",
                isEnabled: false
            ),
            ConfiguredProvider(
                family: .junie,
                displayName: "Junie CLI",
                transport: .cli,
                authMode: .apiKey,
                defaultModel: "junie-default",
                isEnabled: false
            )
        ]

        if let liveProvider = environment["CHAINWORKS_LIVE_PROVIDER"],
           let family = ProviderFamily.from(runtimeIdentifier: liveProvider) {
            providers.removeAll { $0.family == family }
            providers.append(ConfiguredProvider(
                family: family,
                displayName: "\(family.displayName) Seeded",
                transport: .cli,
                authMode: .none,
                defaultModel: environment["CHAINWORKS_LIVE_MODEL"],
                capabilities: .default(for: family)
            ))
        }

        let preferred = Dictionary(uniqueKeysWithValues: providers.map { ($0.family.rawValue, $0.id) })

        return ProviderSettings(
            configuredProviders: providers,
            preferredProviderIDsByFamily: preferred,
            notificationOnProviderFailure: true,
            runStartRequiresCleanPreflight: true
        )
    }
}

private enum ProviderSettingsStoreError: LocalizedError {
    case loadFailed(String)

    var errorDescription: String? {
        switch self {
        case .loadFailed(let path):
            return "Persisted settings at \(path) could not be decoded"
        }
    }
}
