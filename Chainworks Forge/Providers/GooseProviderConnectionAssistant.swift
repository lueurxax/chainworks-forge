import Foundation

enum GooseProviderJourneyState: String, Codable, Equatable, Sendable {
    case configuredUnverified = "configured_unverified"
    case probing
    case verified
    case degraded
    case failing

    var displayName: String {
        rawValue.replacingOccurrences(of: "_", with: " ").capitalized
    }
}

enum GooseProviderAssistantOrigin: String, Codable, Equatable, Sendable {
    case providerSettings = "provider_settings"
    case firstRunWizard = "first_run_wizard"
    case pilotReadiness = "pilot_readiness"

    var displayName: String {
        switch self {
        case .providerSettings:
            return "Provider Settings"
        case .firstRunWizard:
            return "First Run Wizard"
        case .pilotReadiness:
            return "Pilot Readiness"
        }
    }

    var canonicalReturnPath: String {
        switch self {
        case .providerSettings:
            return "Return to Provider Settings after verification."
        case .firstRunWizard:
            return "Return to First Run Wizard and refresh readiness."
        case .pilotReadiness:
            return "Return to Pilot Readiness and refresh the operator status."
        }
    }
}

struct GooseProviderAssistantSnapshot: Identifiable, Equatable, Sendable {
    let providerID: UUID
    let providerDisplayName: String
    let family: ProviderFamily
    let transport: ProviderTransport
    let endpoint: String?
    let authMode: ProviderAuthMode
    let providerIdentifier: String
    let configuredModel: String?
    let journeyState: GooseProviderJourneyState
    let report: ProviderTroubleshootingReport?
    let availableModels: [String]
    let handshakeSteps: [GooseHandshakeStep]
    let checkedAt: Date?
    let origin: GooseProviderAssistantOrigin

    var id: UUID { providerID }
}

enum GooseHandshakeStepState: String, Codable, Equatable, Sendable {
    case pending
    case passed
    case warning
    case failed

    var displayName: String {
        rawValue.capitalized
    }
}

struct GooseHandshakeStep: Identifiable, Codable, Equatable, Sendable {
    let id: UUID
    let label: String
    let value: String
    let state: GooseHandshakeStepState
    let detail: String?

    init(
        id: UUID = UUID(),
        label: String,
        value: String,
        state: GooseHandshakeStepState,
        detail: String? = nil
    ) {
        self.id = id
        self.label = label
        self.value = value
        self.state = state
        self.detail = detail
    }
}

@MainActor
struct GooseProviderHandshakeProbe {
    let providerRegistry: ProviderRegistry
    let appConfigurationStore: AppConfigurationStore

    func configuredSnapshot(
        for providerID: UUID,
        origin: GooseProviderAssistantOrigin
    ) -> GooseProviderAssistantSnapshot? {
        guard let provider = providerRegistry.configuredProvider(id: providerID) else { return nil }
        return GooseProviderAssistantSnapshot(
            providerID: provider.id,
            providerDisplayName: provider.displayName,
            family: provider.family,
            transport: provider.transport,
            endpoint: provider.endpoint,
            authMode: provider.authMode,
            providerIdentifier: provider.family.runtimeProviderIdentifier,
            configuredModel: provider.defaultModel,
            journeyState: .configuredUnverified,
            report: providerRegistry.troubleshootingReport(for: provider.id),
            availableModels: [],
            handshakeSteps: Self.handshakeSteps(
                for: provider,
                report: providerRegistry.troubleshootingReport(for: provider.id),
                availableModels: [],
                checkedAt: providerRegistry.lastRefreshedAt,
                isConfiguredOnly: true
            ),
            checkedAt: providerRegistry.lastRefreshedAt,
            origin: origin
        )
    }

    func probe(
        providerID: UUID,
        origin: GooseProviderAssistantOrigin
    ) async -> GooseProviderAssistantSnapshot? {
        guard let provider = providerRegistry.configuredProvider(id: providerID) else { return nil }
        await providerRegistry.refreshDiagnostics(appConfiguration: appConfigurationStore.configuration)
        let report = providerRegistry.troubleshootingReport(for: provider.id)
        let models = await providerRegistry.availableModels(for: provider)

        return GooseProviderAssistantSnapshot(
            providerID: provider.id,
            providerDisplayName: provider.displayName,
            family: provider.family,
            transport: provider.transport,
            endpoint: provider.endpoint,
            authMode: provider.authMode,
            providerIdentifier: provider.family.runtimeProviderIdentifier,
            configuredModel: provider.defaultModel,
            journeyState: Self.journeyState(for: report),
            report: report,
            availableModels: models,
            handshakeSteps: Self.handshakeSteps(
                for: provider,
                report: report,
                availableModels: models,
                checkedAt: providerRegistry.lastRefreshedAt,
                isConfiguredOnly: false
            ),
            checkedAt: providerRegistry.lastRefreshedAt,
            origin: origin
        )
    }

    private static func journeyState(for report: ProviderTroubleshootingReport?) -> GooseProviderJourneyState {
        guard let report else { return .configuredUnverified }
        switch report.status {
        case .healthy:
            return .verified
        case .warning:
            return .degraded
        case .blocked:
            return .failing
        }
    }

    private static func handshakeSteps(
        for provider: ConfiguredProvider,
        report: ProviderTroubleshootingReport?,
        availableModels: [String],
        checkedAt: Date?,
        isConfiguredOnly: Bool
    ) -> [GooseHandshakeStep] {
        let endpoint = provider.endpoint?.trimmingCharacters(in: .whitespacesAndNewlines)
        let endpointValue = endpoint?.isEmpty == false ? endpoint! : nil
        let transportState: GooseHandshakeStepState = provider.transport == .gooseServer ? .passed : .warning
        let endpointState: GooseHandshakeStepState
        let reachabilityState: GooseHandshakeStepState

        if isConfiguredOnly {
            endpointState = endpointValue == nil ? .failed : .pending
            reachabilityState = .pending
        } else {
            endpointState = endpointValue == nil ? .failed : .passed
            switch report?.failureLayer {
            case .gooseReachability:
                reachabilityState = .failed
            case .some:
                reachabilityState = report?.status == .healthy ? .passed : .warning
            case .none:
                reachabilityState = checkedAt == nil ? .pending : .warning
            }
        }

        let configuredModel = provider.defaultModel?.trimmingCharacters(in: .whitespacesAndNewlines)
        let modelState: GooseHandshakeStepState
        if isConfiguredOnly {
            modelState = configuredModel == nil || configuredModel?.isEmpty == true ? .pending : .passed
        } else if report?.failureLayer == .model {
            modelState = .failed
        } else if configuredModel == nil || configuredModel?.isEmpty == true {
            modelState = availableModels.isEmpty ? .warning : .passed
        } else {
            modelState = availableModels.isEmpty || availableModels.contains(configuredModel!) ? .passed : .warning
        }

        return [
            GooseHandshakeStep(
                label: "Transport",
                value: provider.transport.displayName,
                state: transportState,
                detail: provider.transport == .gooseServer ? "Goose-backed transport is configured." : "Switch to Goose Server to verify the runtime-backed path."
            ),
            GooseHandshakeStep(
                label: "Endpoint",
                value: endpointValue ?? "Missing Goose base URL",
                state: endpointState,
                detail: endpointValue == nil ? "Configure the Goose server base URL before verification." : "Goose requests will target this server."
            ),
            GooseHandshakeStep(
                label: "Auth Expectation",
                value: provider.authMode.displayName,
                state: .passed,
                detail: "The assistant will verify the Goose-backed path with this auth mode expectation."
            ),
            GooseHandshakeStep(
                label: "Provider Identifier",
                value: provider.family.runtimeProviderIdentifier,
                state: .passed,
                detail: "This is the runtime provider identifier sent to Goose for this family."
            ),
            GooseHandshakeStep(
                label: "Model Resolution",
                value: configuredModel ?? "Use provider/runtime default",
                state: modelState,
                detail: availableModels.isEmpty ? "Model availability will be confirmed after verification." : "Available models: \(availableModels.joined(separator: ", "))"
            ),
            GooseHandshakeStep(
                label: "Handshake Probe",
                value: checkedAt == nil ? "Not run yet" : (report?.headline ?? "Latest verification captured"),
                state: reachabilityState,
                detail: report?.explanation ?? "Run Goose verification to capture endpoint reachability and latest remediation."
            )
        ]
    }
}
