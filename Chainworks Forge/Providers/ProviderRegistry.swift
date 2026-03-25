import Foundation
import Observation

@Observable
final class ProviderRegistry {
    let settingsStore: ProviderSettingsStore
    let secretStore: KeychainSecretStore
    private let diagnosticService: ProviderDiagnosticService

    @MainActor private(set) var latestHealthByProviderID: [UUID: ProviderHealthSnapshot] = [:]
    @MainActor private(set) var lastRefreshedAt: Date?

    @MainActor
    init(
        settingsStore: ProviderSettingsStore,
        secretStore: KeychainSecretStore? = nil,
        adapters: [ProviderFamily: any ProviderAdapter]? = nil
    ) {
        let resolvedSecretStore = secretStore ?? KeychainSecretStore()
        let resolvedAdapters = adapters ?? ProviderAdapterFactory.makeAdapters()
        self.settingsStore = settingsStore
        self.secretStore = resolvedSecretStore
        self.diagnosticService = ProviderDiagnosticService(secretStore: resolvedSecretStore, adapters: resolvedAdapters)
    }

    @MainActor
    var configuredProviders: [ConfiguredProvider] {
        settingsStore.settings.configuredProviders
    }

    @MainActor
    func preferredProvider(for family: ProviderFamily) -> ConfiguredProvider? {
        let preferredID = settingsStore.settings.preferredProviderIDsByFamily[family.rawValue]
        return configuredProviders.first(where: { $0.id == preferredID })
            ?? configuredProviders.first(where: { $0.family == family })
    }

    @MainActor
    func configuredProvider(id: UUID) -> ConfiguredProvider? {
        configuredProviders.first(where: { $0.id == id })
    }

    @MainActor
    func availableModels(for provider: ConfiguredProvider) async -> [String] {
        await diagnosticService.availableModels(for: provider)
    }

    @MainActor
    func refreshHealth() async {
        var snapshots: [UUID: ProviderHealthSnapshot] = [:]
        for provider in configuredProviders {
            snapshots[provider.id] = await diagnosticService.healthSnapshot(for: provider)
        }
        latestHealthByProviderID = snapshots
        lastRefreshedAt = Date()
    }

    @MainActor
    func healthSnapshot(for providerID: UUID) -> ProviderHealthSnapshot? {
        latestHealthByProviderID[providerID]
    }
}
