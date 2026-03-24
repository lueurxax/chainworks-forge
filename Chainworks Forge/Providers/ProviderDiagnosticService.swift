import Foundation

struct ProviderDiagnosticService {
    let secretStore: KeychainSecretStore
    let adapters: [ProviderFamily: any ProviderAdapter]

    init(
        secretStore: KeychainSecretStore,
        adapters: [ProviderFamily: any ProviderAdapter] = ProviderAdapterFactory.makeAdapters()
    ) {
        self.secretStore = secretStore
        self.adapters = adapters
    }

    func healthSnapshot(for provider: ConfiguredProvider) async -> ProviderHealthSnapshot {
        guard let adapter = adapters[provider.family] else {
            return ProviderHealthSnapshot(
                providerID: provider.id,
                status: .unavailable,
                checkedAt: Date(),
                summary: "No adapter is registered for \(provider.family.displayName)",
                blockingIssues: ["No adapter registered"]
            )
        }
        return await adapter.verify(provider: provider, secretStore: secretStore)
    }

    func availableModels(for provider: ConfiguredProvider) async -> [String] {
        guard let adapter = adapters[provider.family] else { return [] }
        return await adapter.availableModels(provider: provider, secretStore: secretStore)
    }
}
