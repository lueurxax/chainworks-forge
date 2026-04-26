import Foundation

struct ClaudeACPProviderAdapter: ProviderAdapter {
    let family: ProviderFamily = .claudeACP
    let adapterVersion = "claude-acp-v1"

    func verify(provider: ConfiguredProvider, secretStore: KeychainSecretStore) async -> ProviderHealthSnapshot {
        await ProviderAdapterSupport.verifyCLIProvider(
            executable: "claude-agent-acp",
            provider: provider,
            summaryPrefix: "Claude ACP",
            secretStore: secretStore
        )
    }

    func availableModels(provider: ConfiguredProvider, secretStore: KeychainSecretStore) async -> [String] {
        ProviderAdapterSupport.availableModels(for: provider)
    }
}
