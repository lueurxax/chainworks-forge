import Foundation

protocol ProviderAdapter: Sendable {
    var family: ProviderFamily { get }
    var adapterVersion: String { get }

    func verify(
        provider: ConfiguredProvider,
        secretStore: KeychainSecretStore
    ) async -> ProviderHealthSnapshot

    func availableModels(
        provider: ConfiguredProvider,
        secretStore: KeychainSecretStore
    ) async -> [String]
}

enum ProviderAdapterFactory {
    static func makeAdapters() -> [ProviderFamily: any ProviderAdapter] {
        [
            .codex: CodexProviderAdapter(),
            .claude: ClaudeProviderAdapter(),
            .gemini: GeminiProviderAdapter()
        ]
    }
}

enum ProviderAdapterSupport {
    static func verifyCLIProvider(
        executable: String,
        provider: ConfiguredProvider,
        summaryPrefix: String,
        secretStore: KeychainSecretStore
    ) async -> ProviderHealthSnapshot {
        let hasExecutable = ProcessSupport.which(executable) != nil
        let hasCredential = hasRequiredCredential(provider: provider, secretStore: secretStore)
        let issues = [
            hasExecutable ? nil : "Executable '\(executable)' is not available on PATH",
            hasCredential ? nil : credentialIssue(for: provider)
        ].compactMap { $0 }

        return ProviderHealthSnapshot(
            providerID: provider.id,
            status: issues.isEmpty ? .healthy : .degraded,
            checkedAt: Date(),
            summary: issues.isEmpty ? "\(summaryPrefix) CLI is available" : "\(summaryPrefix) requires attention",
            blockingIssues: issues
        )
    }

    static func verifyEndpointProvider(
        provider: ConfiguredProvider,
        summaryPrefix: String,
        secretStore: KeychainSecretStore
    ) async -> ProviderHealthSnapshot {
        var issues: [String] = []
        if provider.endpoint?.isEmpty != false {
            issues.append("Endpoint is missing")
        }
        if !hasRequiredCredential(provider: provider, secretStore: secretStore) {
            issues.append(credentialIssue(for: provider))
        }
        return ProviderHealthSnapshot(
            providerID: provider.id,
            status: issues.isEmpty ? .healthy : .degraded,
            checkedAt: Date(),
            summary: issues.isEmpty ? "\(summaryPrefix) endpoint is configured" : "\(summaryPrefix) requires attention",
            blockingIssues: issues
        )
    }

    static func availableModels(for provider: ConfiguredProvider) -> [String] {
        if let defaultModel = provider.defaultModel, !defaultModel.isEmpty {
            return [defaultModel]
        }

        switch provider.family {
        case .codex:
            return ["gpt-5-codex", "gpt-5.4"]
        case .claude:
            return ["claude-sonnet-4", "claude-opus-4"]
        case .gemini:
            return ["gemini-2.5-pro", "gemini-2.5-flash"]
        }
    }

    private static func hasRequiredCredential(
        provider: ConfiguredProvider,
        secretStore: KeychainSecretStore
    ) -> Bool {
        switch provider.authMode {
        case .none:
            return true
        case .apiKey, .sessionToken:
            return (try? secretStore.secret(for: secretKey(for: provider)))?.isEmpty == false
        }
    }

    static func secretKey(for provider: ConfiguredProvider) -> String {
        "provider.\(provider.id.uuidString)"
    }

    private static func credentialIssue(for provider: ConfiguredProvider) -> String {
        switch provider.authMode {
        case .none:
            return ""
        case .apiKey:
            return "API key is missing"
        case .sessionToken:
            return "Session token is missing"
        }
    }
}

enum ProcessSupport {
    static func which(_ executable: String) -> String? {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/which")
        process.arguments = [executable]
        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = Pipe()
        do {
            try process.run()
            process.waitUntilExit()
        } catch {
            return nil
        }

        guard process.terminationStatus == 0 else { return nil }
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        let output = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines)
        return output?.isEmpty == false ? output : nil
    }
}
