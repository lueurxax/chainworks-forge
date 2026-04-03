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
            self.settings = Self.sanitized(loaded)
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
                settings.configuredProviders.first(where: { $0.family == provider.family })?.id
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
            configuredProviders: settings.configuredProviders.map(sanitized),
            preferredProviderIDsByFamily: settings.preferredProviderIDsByFamily,
            notificationOnProviderFailure: settings.notificationOnProviderFailure,
            runStartRequiresCleanPreflight: settings.runStartRequiresCleanPreflight
        )
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
        let seedsInMemory = environment["CHAINWORKS_IN_MEMORY_STORE"] == "1"
        let fixtureEndpoint = environment["CHAINWORKS_GOOSE_FIXTURE_MODE"] == nil ? nil : "http://fixture.local"
        let gooseBaseURL = environment["CHAINWORKS_GOOSE_BASE_URL"] ?? fixtureEndpoint
        var providers: [ConfiguredProvider] = seedsInMemory ? [
            ConfiguredProvider(
                family: .codex,
                displayName: "Codex Goose",
                transport: .gooseServer,
                endpoint: gooseBaseURL,
                authMode: gooseBaseURL == fixtureEndpoint ? .none : (environment["CHAINWORKS_GOOSE_API_KEY"] == nil ? .none : .apiKey),
                defaultModel: "gpt-5-codex"
            ),
            ConfiguredProvider(
                family: .claude,
                displayName: "Claude Goose",
                transport: .gooseServer,
                endpoint: gooseBaseURL,
                authMode: gooseBaseURL == fixtureEndpoint ? .none : (environment["CHAINWORKS_GOOSE_API_KEY"] == nil ? .none : .apiKey),
                defaultModel: "sonnet"
            ),
            ConfiguredProvider(
                family: .gemini,
                displayName: gooseBaseURL == nil ? "Gemini CLI" : "Gemini Goose",
                transport: gooseBaseURL == nil ? .cli : .gooseServer,
                endpoint: gooseBaseURL,
                authMode: gooseBaseURL == nil || gooseBaseURL == fixtureEndpoint ? .none : (environment["CHAINWORKS_GOOSE_API_KEY"] == nil ? .none : .apiKey),
                defaultModel: "gemini-2.5-pro"
            )
        ] : []

        if let liveProvider = environment["CHAINWORKS_LIVE_PROVIDER"],
           let family = ProviderFamily.from(runtimeIdentifier: liveProvider) {
            let transport: ProviderTransport = gooseBaseURL == nil ? .cli : .gooseServer
            let authMode: ProviderAuthMode = gooseBaseURL == nil || gooseBaseURL == fixtureEndpoint
                ? .none
                : (environment["CHAINWORKS_GOOSE_API_KEY"] == nil ? .none : .apiKey)
            let endpoint = gooseBaseURL
            providers.removeAll { $0.family == family }
            providers.append(ConfiguredProvider(
                family: family,
                displayName: transport == .gooseServer ? "\(family.displayName) Goose" : "\(family.displayName) Seeded",
                transport: transport,
                endpoint: endpoint,
                authMode: authMode,
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
