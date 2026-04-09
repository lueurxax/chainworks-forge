import Foundation

struct AuggieProviderAdapter: ProviderAdapter {
    let family: ProviderFamily = .auggie
    let adapterVersion = "auggie-v1"
    private let gooseProbe: @Sendable (URL) async -> GooseServerReachability

    init(
        gooseProbe: @escaping @Sendable (URL) async -> GooseServerReachability = ProviderAdapterSupport.probeGooseServerStatus
    ) {
        self.gooseProbe = gooseProbe
    }

    func verify(provider: ConfiguredProvider, secretStore: KeychainSecretStore) async -> ProviderHealthSnapshot {
        switch provider.transport {
        case .cli:
            return await ProviderAdapterSupport.verifyCLIProvider(
                executable: "auggie",
                provider: provider,
                summaryPrefix: "Auggie",
                secretStore: secretStore
            )
        case .httpAPI, .localBridge:
            return await ProviderAdapterSupport.verifyEndpointProvider(
                provider: provider,
                summaryPrefix: "Auggie",
                secretStore: secretStore
            )
        case .gooseServer:
            return await ProviderAdapterSupport.verifyGooseServerProvider(
                provider: provider,
                summaryPrefix: "Auggie",
                secretStore: secretStore,
                gooseProbe: gooseProbe
            )
        }
    }

    func availableModels(provider: ConfiguredProvider, secretStore: KeychainSecretStore) async -> [String] {
        ProviderAdapterSupport.availableModels(for: provider)
    }
}
