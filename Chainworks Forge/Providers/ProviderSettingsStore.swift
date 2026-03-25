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
            self.settings = initialSettings
            try? persist()
            return
        }

        if let loaded = try? Self.load(from: resolvedURL) {
            self.settings = loaded
        } else {
            self.settings = Self.seededDefault()
            try? persist()
        }
    }

    @MainActor
    func replace(with settings: ProviderSettings) {
        self.settings = settings
        try? persist()
    }

    @MainActor
    func upsert(provider: ConfiguredProvider) {
        var providers = settings.configuredProviders
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

    static func defaultFileURL() -> URL {
        AppConfiguration.defaultSupportRoot()
            .appendingPathComponent("provider-settings.json")
    }

    private static func seededDefault() -> ProviderSettings {
        let environment = ProcessInfo.processInfo.environment
        let seedsInMemory = environment["CHAINWORKS_IN_MEMORY_STORE"] == "1"
        var providers: [ConfiguredProvider] = seedsInMemory ? [
            ConfiguredProvider(
                family: .codex,
                displayName: "Codex Goose",
                transport: .gooseServer,
                endpoint: environment["CHAINWORKS_GOOSE_BASE_URL"],
                authMode: environment["CHAINWORKS_GOOSE_API_KEY"] == nil ? .none : .apiKey,
                defaultModel: "gpt-5-codex"
            ),
            ConfiguredProvider(
                family: .claude,
                displayName: "Claude Goose",
                transport: .gooseServer,
                endpoint: environment["CHAINWORKS_GOOSE_BASE_URL"],
                authMode: environment["CHAINWORKS_GOOSE_API_KEY"] == nil ? .none : .apiKey,
                defaultModel: "claude-sonnet-4"
            ),
            ConfiguredProvider(
                family: .gemini,
                displayName: "Gemini CLI",
                transport: .cli,
                authMode: .none,
                defaultModel: "gemini-2.5-pro"
            )
        ] : []

        if let liveProvider = environment["CHAINWORKS_LIVE_PROVIDER"],
           let family = ProviderFamily.from(runtimeIdentifier: liveProvider) {
            let gooseBaseURL = environment["CHAINWORKS_GOOSE_BASE_URL"]
            let transport: ProviderTransport = gooseBaseURL == nil ? .cli : .gooseServer
            let authMode: ProviderAuthMode = environment["CHAINWORKS_GOOSE_API_KEY"] == nil ? .none : .apiKey
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
