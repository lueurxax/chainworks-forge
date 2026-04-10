import Foundation
import Observation

@Observable
final class ProviderSettingsStore {
    private let fileURL: URL
    @MainActor private(set) var settings: ProviderSettings

    @MainActor
    init(fileURL: URL? = nil, initialSettings: ProviderSettings? = nil) {
        let resolvedURL = fileURL ?? Self.defaultFileURL()
        self.fileURL = resolvedURL

        if let initialSettings {
            self.settings = Self.sanitized(initialSettings)
            try? persist()
            return
        }

        if let loaded = try? Self.load(from: resolvedURL) {
            let migrated = Self.migratedIfNeeded(loaded)
            self.settings = Self.sanitized(migrated)
            if self.settings != loaded {
                try? persist()
            }
        } else {
            self.settings = Self.sanitized(Self.seededDefault())
            try? persist()
        }
    }

    @MainActor
    func replace(with settings: ProviderSettings) {
        self.settings = Self.sanitized(settings)
        try? persist()
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
        try? persist()
    }

    @MainActor
    func removeProvider(id: UUID) {
        guard let provider = settings.configuredProviders.first(where: { $0.id == id }) else { return }
        settings.configuredProviders.removeAll { $0.id == id }
        if settings.preferredProviderIDsByFamily[provider.family.rawValue] == id {
            settings.preferredProviderIDsByFamily[provider.family.rawValue] =
                settings.configuredProviders.first(where: { $0.family == provider.family && $0.isEnabled })?.id
        }
        try? persist()
    }

    @MainActor
    func setPreferredProvider(id: UUID, for family: ProviderFamily) {
        settings.preferredProviderIDsByFamily[family.rawValue] = id
        try? persist()
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
        try JSONDecoder().decode(ProviderSettings.self, from: Data(contentsOf: fileURL))
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
                defaultModel: "gemini-2.5-pro"
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
