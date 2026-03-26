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
    case gooseEndpoint
    case gooseReachability
    case cliExecutable
    case credential
    case model
    case health
    case unknown

    var displayName: String {
        switch self {
        case .transport:
            return "Transport"
        case .gooseEndpoint:
            return "Goose Endpoint"
        case .gooseReachability:
            return "Goose Reachability"
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
    let gooseFirstGuidance: String?

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
        let isGooseFirstFamily = provider.family.gooseFirstPreferred
        let configuredEndpoint = provider.endpoint?.trimmingCharacters(in: .whitespacesAndNewlines)
        let configuredEndpointValue = configuredEndpoint?.isEmpty == false ? configuredEndpoint! : nil
        let cliExecutable = provider.family == .codex ? "codex" : provider.family == .claude ? "claude" : nil
        let cliPath = cliExecutable.flatMap { whichExecutable($0) }

        var evidence: [ProviderTroubleshootingEvidence] = [
            ProviderTroubleshootingEvidence(label: "Configured transport", value: provider.transport.displayName),
            ProviderTroubleshootingEvidence(label: "Preferred path", value: isGooseFirstFamily ? "Goose-backed" : "Direct transport"),
            ProviderTroubleshootingEvidence(label: "Active configuration source", value: appConfiguration.activeConfigurationSource.displayName)
        ]

        if let configuredEndpointValue {
            evidence.append(ProviderTroubleshootingEvidence(
                label: "Endpoint",
                value: configuredEndpointValue
            ))
        } else if provider.transport == .gooseServer {
            evidence.append(ProviderTroubleshootingEvidence(
                label: "Endpoint",
                value: "Goose base URL is missing",
                state: .blocked
            ))
        }

        if provider.transport == .gooseServer {
            if let health {
                if let reachabilityIssue = ProviderAdapterSupport.gooseServerReachabilityIssue(from: health.blockingIssues) {
                    evidence.append(ProviderTroubleshootingEvidence(
                        label: "Goose server reachability",
                        value: reachabilityIssue,
                        state: .blocked
                    ))
                } else if let configuredEndpointValue {
                    evidence.append(ProviderTroubleshootingEvidence(
                        label: "Goose server reachability",
                        value: "Reachable via \(ProviderAdapterSupport.gooseStatusURLString(for: configuredEndpointValue))",
                        state: .info
                    ))
                } else {
                    evidence.append(ProviderTroubleshootingEvidence(
                        label: "Goose server reachability",
                        value: "Reachability cannot be checked until an endpoint is configured",
                        state: .warning
                    ))
                }
            } else {
                evidence.append(ProviderTroubleshootingEvidence(
                    label: "Goose server reachability",
                    value: "Reachability has not been checked yet",
                    state: .warning
                ))
            }
        }

        if let cliExecutable {
            evidence.append(ProviderTroubleshootingEvidence(
                label: "CLI executable",
                value: cliPath ?? "\(cliExecutable) not on PATH",
                state: cliPath == nil ? .blocked : .info
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
                    value: health.blockingIssues.joined(separator: " • "),
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

        let gooseGuidance: String? = isGooseFirstFamily
            ? "Codex and Claude should prefer the Goose-backed path. Use Goose Server transport, then refresh diagnostics."
            : nil

        if provider.transport == .gooseServer {
            if configuredEndpointValue == nil {
                return ProviderTroubleshootingReport(
                    providerID: provider.id,
                    family: provider.family,
                    displayName: provider.displayName,
                    transport: provider.transport,
                    status: .blocked,
                    headline: "\(provider.displayName) needs a Goose endpoint",
                    explanation: "This provider is configured for Goose-backed execution, but the Goose base URL is missing.",
                    failureLayer: .gooseEndpoint,
                    remediation: [
                        "Enter the Goose server base URL in the provider endpoint field.",
                        "Use a local goosed endpoint such as https://127.0.0.1:51200.",
                        "Refresh diagnostics after saving."
                    ],
                    evidence: evidence,
                    availableModels: availableModels,
                    gooseFirstGuidance: gooseGuidance
                )
            }

            if let health, let reachabilityIssue = ProviderAdapterSupport.gooseServerReachabilityIssue(from: health.blockingIssues) {
                return ProviderTroubleshootingReport(
                    providerID: provider.id,
                    family: provider.family,
                    displayName: provider.displayName,
                    transport: provider.transport,
                    status: .blocked,
                    headline: "\(provider.displayName) cannot reach Goose server",
                    explanation: reachabilityIssue,
                    failureLayer: .gooseReachability,
                    remediation: [
                        "Make sure goosed is running at the configured base URL.",
                        "Verify the host, port, and local TLS trust for the Goose server.",
                        "Refresh diagnostics after the server becomes reachable."
                    ],
                    evidence: evidence,
                    availableModels: availableModels,
                    gooseFirstGuidance: gooseGuidance
                )
            }

            if let health, !health.blockingIssues.isEmpty {
                return ProviderTroubleshootingReport(
                    providerID: provider.id,
                    family: provider.family,
                    displayName: provider.displayName,
                    transport: provider.transport,
                    status: .blocked,
                    headline: "\(provider.displayName) Goose path needs attention",
                    explanation: health.summary,
                    failureLayer: .health,
                    remediation: health.blockingIssues.map { "Resolve: \($0)" },
                    evidence: evidence,
                    availableModels: availableModels,
                    gooseFirstGuidance: gooseGuidance
                )
            }

            return ProviderTroubleshootingReport(
                providerID: provider.id,
                family: provider.family,
                displayName: provider.displayName,
                transport: provider.transport,
                status: .healthy,
                headline: "\(provider.displayName) is Goose-backed",
                explanation: "The provider is configured for the Goose-backed runtime path and the latest diagnostics are clean.",
                failureLayer: .unknown,
                remediation: [
                    "Keep Goose Server transport as the primary path.",
                    "Use the CLI fallback only for break-glass debugging."
                ],
                evidence: evidence,
                availableModels: availableModels,
                gooseFirstGuidance: gooseGuidance
            )
        }

        if isGooseFirstFamily {
            if cliPath == nil {
                return ProviderTroubleshootingReport(
                    providerID: provider.id,
                    family: provider.family,
                    displayName: provider.displayName,
                    transport: provider.transport,
                    status: .blocked,
                    headline: "\(provider.displayName) is not ready for local CLI fallback",
                    explanation: "This family should prefer Goose-backed setup. The direct CLI executable is not available, so the fallback path is blocked.",
                    failureLayer: .cliExecutable,
                    remediation: [
                        "Switch this provider to Goose Server transport and set the Goose base URL.",
                        "Or install \(cliExecutable ?? provider.family.runtimeProviderIdentifier) on PATH if you intentionally want CLI fallback."
                    ],
                    evidence: evidence,
                    availableModels: availableModels,
                    gooseFirstGuidance: gooseGuidance
                )
            }

            return ProviderTroubleshootingReport(
                providerID: provider.id,
                family: provider.family,
                displayName: provider.displayName,
                transport: provider.transport,
                status: .warning,
                headline: "\(provider.displayName) is using direct CLI fallback",
                explanation: "Codex and Claude work more predictably through Goose-backed setup. The current direct CLI configuration is usable, but it is not the primary product path.",
                failureLayer: .transport,
                remediation: [
                    "Change this provider to Goose Server transport.",
                    "Set the Goose base URL in the endpoint field.",
                    "Refresh diagnostics to verify the Goose path."
                ],
                evidence: evidence,
                availableModels: availableModels,
                gooseFirstGuidance: gooseGuidance
            )
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
                availableModels: availableModels,
                gooseFirstGuidance: gooseGuidance
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
            availableModels: availableModels,
            gooseFirstGuidance: gooseGuidance
        )
    }
}
