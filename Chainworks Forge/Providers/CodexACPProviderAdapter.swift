import Foundation

struct CodexACPProviderAdapter: ProviderAdapter {
    let family: ProviderFamily = .codexACP
    let adapterVersion = "codex-acp-v1"

    func verify(provider: ConfiguredProvider, secretStore: KeychainSecretStore) async -> ProviderHealthSnapshot {
        switch provider.transport {
        case .cli:
            return await ProviderAdapterSupport.verifyCLIProvider(
                executable: "codex-acp",
                provider: provider,
                summaryPrefix: "Codex ACP",
                secretStore: secretStore
            )
        case .httpAPI, .localBridge:
            return await ProviderAdapterSupport.verifyEndpointProvider(
                provider: provider,
                summaryPrefix: "Codex ACP",
                secretStore: secretStore
            )
        }
    }

    func availableModels(provider: ConfiguredProvider, secretStore: KeychainSecretStore) async -> [String] {
        ProviderAdapterSupport.availableModels(for: provider)
    }
}
