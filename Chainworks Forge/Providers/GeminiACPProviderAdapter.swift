import Foundation

struct GeminiACPProviderAdapter: ProviderAdapter {
    let family: ProviderFamily = .geminiACP
    let adapterVersion = "gemini-acp-v1"

    func verify(provider: ConfiguredProvider, secretStore: KeychainSecretStore) async -> ProviderHealthSnapshot {
        await ProviderAdapterSupport.verifyCLIProvider(
            executable: "gemini",
            provider: provider,
            summaryPrefix: "Gemini ACP",
            secretStore: secretStore
        )
    }

    func availableModels(provider: ConfiguredProvider, secretStore: KeychainSecretStore) async -> [String] {
        ProviderAdapterSupport.availableModels(for: provider)
    }
}
