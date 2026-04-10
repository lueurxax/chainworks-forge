import Foundation

enum ProviderTroubleshootingStatus: String, Codable, Sendable {
    case healthy
    case warning
    case blocked

    var displayName: String {
        rawValue.capitalized
    }
}

enum ProviderTroubleshootingLayer: String, Codable, Sendable {
    case transport
    case cliExecutable
    case credential
    case model
    case health
    case unknown

    var displayName: String {
        switch self {
        case .transport:
            return "Transport"
        case .cliExecutable:
            return "CLI Executable"
        case .credential:
            return "Credential"
        case .model:
            return "Model"
        case .health:
            return "Health"
        case .unknown:
            return "Unknown"
        }
    }
}

enum ProviderTroubleshootingEvidenceState: String, Codable, Sendable {
    case info
    case warning
    case blocked

    var displayName: String {
        rawValue.capitalized
    }
}

struct ProviderTroubleshootingEvidence: Identifiable, Codable, Equatable, Sendable {
    let id: UUID
    let label: String
    let value: String
    let state: ProviderTroubleshootingEvidenceState

    init(
        id: UUID = UUID(),
        label: String,
        value: String,
        state: ProviderTroubleshootingEvidenceState = .info
    ) {
        self.id = id
        self.label = label
        self.value = value
        self.state = state
    }
}

struct ProviderTroubleshootingReport: Identifiable, Codable, Equatable, Sendable {
    let providerID: UUID
    let family: ProviderFamily
    let displayName: String
    let transport: ProviderTransport
    let status: ProviderTroubleshootingStatus
    let headline: String
    let explanation: String
    let failureLayer: ProviderTroubleshootingLayer
    let remediation: [String]
    let evidence: [ProviderTroubleshootingEvidence]
    let availableModels: [String]

    var id: UUID { providerID }
}

@MainActor
struct ProviderTroubleshootingService {
    private let whichExecutable: @Sendable (String) -> String?

    init(whichExecutable: @escaping @Sendable (String) -> String? = ProcessSupport.which) {
        self.whichExecutable = whichExecutable
    }

    func report(
        for provider: ConfiguredProvider,
        providerRegistry: ProviderRegistry,
        appConfiguration: AppConfiguration
    ) async -> ProviderTroubleshootingReport {
        let health = providerRegistry.healthSnapshot(for: provider.id)
        let availableModels = await providerRegistry.availableModels(for: provider)
        let configuredEndpoint = provider.endpoint?.trimmingCharacters(in: .whitespacesAndNewlines)
        let configuredEndpointValue = configuredEndpoint?.isEmpty == false ? configuredEndpoint! : nil

        var evidence: [ProviderTroubleshootingEvidence] = [
            ProviderTroubleshootingEvidence(label: "Configured transport", value: provider.transport.displayName),
            ProviderTroubleshootingEvidence(label: "Active configuration source", value: appConfiguration.activeConfigurationSource.displayName)
        ]

        if let configuredEndpointValue {
            evidence.append(ProviderTroubleshootingEvidence(
                label: "Endpoint",
                value: configuredEndpointValue
            ))
        }

        if let health {
            evidence.append(ProviderTroubleshootingEvidence(
                label: "Health summary",
                value: health.summary,
                state: health.blockingIssues.isEmpty ? .info : .warning
            ))
            if !health.blockingIssues.isEmpty {
                evidence.append(ProviderTroubleshootingEvidence(
                    label: "Blocking issues",
                    value: health.blockingIssues.joined(separator: " * "),
                    state: .blocked
                ))
            }
        } else {
            evidence.append(ProviderTroubleshootingEvidence(
                label: "Health summary",
                value: "Health has not been refreshed yet",
                state: .warning
            ))
        }

        if !availableModels.isEmpty {
            evidence.append(ProviderTroubleshootingEvidence(
                label: "Available models",
                value: availableModels.joined(separator: ", ")
            ))
        }

        if let health, health.status != .healthy {
            let status: ProviderTroubleshootingStatus = health.status == .unavailable ? .blocked : .warning
            return ProviderTroubleshootingReport(
                providerID: provider.id,
                family: provider.family,
                displayName: provider.displayName,
                transport: provider.transport,
                status: status,
                headline: "\(provider.displayName) needs attention",
                explanation: health.summary,
                failureLayer: .health,
                remediation: health.blockingIssues.isEmpty
                    ? ["Refresh diagnostics or verify the configured provider."]
                    : health.blockingIssues.map { "Resolve: \($0)" },
                evidence: evidence,
                availableModels: availableModels
            )
        }

        return ProviderTroubleshootingReport(
            providerID: provider.id,
            family: provider.family,
            displayName: provider.displayName,
            transport: provider.transport,
            status: .healthy,
            headline: "\(provider.displayName) is ready",
            explanation: health?.summary ?? "\(provider.displayName) is configured and ready.",
            failureLayer: .unknown,
            remediation: [
                "Keep this provider configuration as the persisted source of truth.",
                "Refresh diagnostics if the external environment changes."
            ],
            evidence: evidence,
            availableModels: availableModels
        )
    }
}
