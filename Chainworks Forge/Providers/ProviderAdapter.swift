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

enum GooseServerReachability: Equatable, Sendable {
    case reachable(statusCode: Int)
    case unreachable(reason: String)
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

    static func verifyGooseServerProvider(
        provider: ConfiguredProvider,
        summaryPrefix: String,
        secretStore: KeychainSecretStore,
        gooseProbe: @escaping @Sendable (URL) async -> GooseServerReachability = probeGooseServerStatus
    ) async -> ProviderHealthSnapshot {
        var issues: [String] = []
        var reachableStatusCode: Int?
        var baseURL: URL?

        if provider.endpoint?.isEmpty != false {
            issues.append("Goose server base URL is missing")
        } else if let endpoint = provider.endpoint, let parsedURL = URL(string: endpoint) {
            baseURL = parsedURL
        } else if let endpoint = provider.endpoint, URL(string: endpoint) == nil {
            issues.append("Goose server base URL is invalid")
        }

        if !hasRequiredCredential(provider: provider, secretStore: secretStore) {
            issues.append(credentialIssue(for: provider))
        }

        if let baseURL {
            switch await gooseProbe(baseURL) {
            case .reachable(let statusCode):
                reachableStatusCode = statusCode
            case .unreachable(let reason):
                issues.append(gooseServerReachabilityIssue(for: baseURL, reason: reason))
            }
        }

        let hasReachabilityIssue = gooseServerReachabilityIssue(from: issues) != nil
        let status: ProviderStatus
        if hasReachabilityIssue {
            status = .unavailable
        } else if issues.isEmpty {
            status = .healthy
        } else {
            status = .degraded
        }

        let summary: String
        switch status {
        case .healthy:
            let responseSuffix = reachableStatusCode.map { " (HTTP \($0))" } ?? ""
            summary = "\(summaryPrefix) Goose server is reachable\(responseSuffix)"
        case .unavailable:
            summary = "\(summaryPrefix) Goose server is unreachable"
        case .degraded, .unknown:
            if reachableStatusCode != nil {
                summary = "\(summaryPrefix) Goose server is reachable, but provider requires attention"
            } else {
                summary = "\(summaryPrefix) Goose path requires attention"
            }
        }

        return ProviderHealthSnapshot(
            providerID: provider.id,
            status: status,
            checkedAt: Date(),
            summary: summary,
            blockingIssues: issues
        )
    }

    static func gooseStatusURL(for baseURL: URL) -> URL {
        baseURL.appendingPathComponent("status")
    }

    static func gooseStatusURLString(for endpoint: String) -> String {
        guard let baseURL = URL(string: endpoint) else { return endpoint }
        return gooseStatusURL(for: baseURL).absoluteString
    }

    static func gooseServerReachabilityIssue(from issues: [String]) -> String? {
        issues.first { $0.hasPrefix(gooseReachabilityIssuePrefix) }
    }

    private static let gooseReachabilityIssuePrefix = "Goose server is unreachable at "

    private static func gooseServerReachabilityIssue(for baseURL: URL, reason: String) -> String {
        "\(gooseReachabilityIssuePrefix)\(gooseStatusURL(for: baseURL).absoluteString): \(reason)"
    }

    static func probeGooseServerStatus(at baseURL: URL) async -> GooseServerReachability {
        let sessionConfiguration = URLSessionConfiguration.ephemeral
        sessionConfiguration.timeoutIntervalForRequest = 5
        sessionConfiguration.timeoutIntervalForResource = 10
        let delegate = LocalhostTrustDelegate()
        let session = URLSession(configuration: sessionConfiguration, delegate: delegate, delegateQueue: nil)
        defer { session.invalidateAndCancel() }

        var request = URLRequest(url: gooseStatusURL(for: baseURL))
        request.httpMethod = "GET"

        do {
            let (_, response) = try await session.data(for: request)
            guard let httpResponse = response as? HTTPURLResponse else {
                return .unreachable(reason: "Received a non-HTTP response")
            }
            guard (200..<300).contains(httpResponse.statusCode) else {
                return .unreachable(reason: "Server returned HTTP \(httpResponse.statusCode)")
            }
            return .reachable(statusCode: httpResponse.statusCode)
        } catch {
            return .unreachable(reason: error.localizedDescription)
        }
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
    nonisolated static func which(_ executable: String) -> String? {
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
