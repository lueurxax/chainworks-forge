import Foundation

struct GeminiProviderAdapter: ProviderAdapter {
    let family: ProviderFamily = .gemini
    let adapterVersion = "gemini-v1"
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
                executable: "gemini",
                provider: provider,
                summaryPrefix: "Gemini",
                secretStore: secretStore
            )
        case .httpAPI, .localBridge:
            return await ProviderAdapterSupport.verifyEndpointProvider(
                provider: provider,
                summaryPrefix: "Gemini",
                secretStore: secretStore
            )
        case .gooseServer:
            return await ProviderAdapterSupport.verifyGooseServerProvider(
                provider: provider,
                summaryPrefix: "Gemini",
                secretStore: secretStore,
                gooseProbe: gooseProbe
            )
        }
    }

    func availableModels(provider: ConfiguredProvider, secretStore: KeychainSecretStore) async -> [String] {
        ProviderAdapterSupport.availableModels(for: provider)
    }
}
