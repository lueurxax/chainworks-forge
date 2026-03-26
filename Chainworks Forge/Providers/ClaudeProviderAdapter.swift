import Foundation

struct ClaudeProviderAdapter: ProviderAdapter {
    let family: ProviderFamily = .claude
    let adapterVersion = "claude-v1"
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
                executable: "claude",
                provider: provider,
                summaryPrefix: "Claude",
                secretStore: secretStore
            )
        case .httpAPI, .localBridge:
            return await ProviderAdapterSupport.verifyEndpointProvider(
                provider: provider,
                summaryPrefix: "Claude",
                secretStore: secretStore
            )
        case .gooseServer:
            return await ProviderAdapterSupport.verifyGooseServerProvider(
                provider: provider,
                summaryPrefix: "Claude",
                secretStore: secretStore,
                gooseProbe: gooseProbe
            )
        }
    }

    func availableModels(provider: ConfiguredProvider, secretStore: KeychainSecretStore) async -> [String] {
        ProviderAdapterSupport.availableModels(for: provider)
    }
}
