import SwiftUI
import SwiftData

struct GooseProviderConnectionAssistantView: View {
    @Environment(AppConfigurationStore.self) private var appConfigurationStore
    @Environment(ProviderSettingsStore.self) private var providerSettingsStore
    @Environment(ProviderRegistry.self) private var providerRegistry
    @Environment(GooseServerManager.self) private var gooseServerManager
    @Environment(\.dismiss) private var dismiss

    let providerID: UUID
    let origin: GooseProviderAssistantOrigin

    @State private var snapshot: GooseProviderAssistantSnapshot?
    @State private var draftProvider: ConfiguredProvider?
    @State private var isProbing = false
    @State private var lastSavedMessage: String?

    private var probe: GooseProviderHandshakeProbe {
        GooseProviderHandshakeProbe(
            providerRegistry: providerRegistry,
            appConfigurationStore: appConfigurationStore
        )
    }

    var body: some View {
        NavigationStack {
            List {
                // Proposal 012 (L-04): Journey visualization with 3-step progress
                Section("Journey") {
                    Text("Goose Connection Assistant")
                        .font(.title2.bold())
                        .accessibilityIdentifier("goose-assistant-title")

                    // 3-step progress indicator: Configure → Verify → Connected
                    journeyProgressIndicator

                    if let snapshot {
                        LabeledContent("Provider", value: snapshot.providerDisplayName)
                            .accessibilityIdentifier("goose-assistant-provider-name")
                        LabeledContent("Family", value: snapshot.family.displayName)
                            .accessibilityIdentifier("goose-assistant-provider-family")
                    } else if let draftProvider {
                        LabeledContent("Provider", value: draftProvider.displayName)
                            .accessibilityIdentifier("goose-assistant-provider-name")
                        LabeledContent("Family", value: draftProvider.family.displayName)
                            .accessibilityIdentifier("goose-assistant-provider-family")
                    }
                    LabeledContent("Origin", value: origin.displayName)

                    // Proposal 012 (L-12): Journey state with spinner during probing
                    HStack {
                        Text("State")
                        Spacer()
                        if isProbing {
                            ProgressView()
                                .controlSize(.small)
                            Text("Verifying…")
                                .font(DesignTokens.Typography.supporting)
                                .foregroundStyle(.secondary)
                        } else {
                            StatusCapsule(
                                text: journeyState.displayName,
                                color: journeyStateColor,
                                icon: journeyStateIcon,
                                size: .small
                            )
                        }
                    }
                    .accessibilityIdentifier("goose-assistant-state")
                    Text(origin.canonicalReturnPath)
                        .font(DesignTokens.Typography.supporting)
                        .foregroundStyle(.secondary)
                }

                if snapshot == nil && draftProvider == nil {
                    Section("Loading") {
                        ProgressView("Loading provider journey…")
                            .accessibilityIdentifier("goose-assistant-loading")
                    }
                } else {
                    configurationSection
                }

                Section("Guided Verification") {
                    Text("Use this assistant to verify the Goose-backed path the live runtime actually depends on. The assistant owns verification; raw evidence stays in the evidence panel.")
                        .font(.caption)
                        .foregroundStyle(.secondary)

                    Button(isProbing ? "Saving and Verifying…" : "Save and Verify Goose Path") {
                        Task { await saveAndVerify() }
                    }
                    .disabled(isProbing || draftProvider == nil)
                    .accessibilityIdentifier("goose-assistant-save-and-verify")

                    Button(isProbing ? "Running Goose Verification…" : "Run Goose Verification") {
                        Task { await runProbe() }
                    }
                    .disabled(isProbing)
                    .accessibilityIdentifier("goose-assistant-run-probe")

                    Button("Start Managed Goose Server") {
                        Task {
                            await gooseServerManager.ensureRunning()
                            await runProbe()
                        }
                    }
                    .accessibilityIdentifier("goose-assistant-start-server")

                    Button("Refresh and Return to \(origin.displayName)") {
                        Task {
                            await providerRegistry.refreshDiagnostics(appConfiguration: appConfigurationStore.configuration)
                            dismiss()
                        }
                    }
                    .disabled(isProbing)
                    .accessibilityIdentifier("goose-assistant-return")

                    if let lastSavedMessage {
                        Text(lastSavedMessage)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .accessibilityIdentifier("goose-assistant-save-message")
                    }
                }

                if let snapshot {
                    if let report = snapshot.report {
                        Section("Current Guidance") {
                            ProviderTroubleshootingPanel(report: report)
                        }

                        Section("Evidence") {
                            ProviderSetupEvidencePanel(snapshot: snapshot)
                        }
                    } else {
                        Section("Current Guidance") {
                            Text("No diagnostics captured yet. Run Goose verification to populate handshake facts and remediation.")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
            }
            .navigationTitle("Goose Assistant")
            .accessibilityIdentifier("goose-connection-assistant-view")
            .frame(minWidth: 760, idealWidth: 840, minHeight: 680, idealHeight: 780)
            .task(id: providerID) {
                let provider = providerRegistry.configuredProvider(id: providerID)
                draftProvider = provider
                snapshot = probe.configuredSnapshot(for: providerID, origin: origin)
            }
        }
    }

    private var configurationSection: some View {
        Section("Goose-backed Setup") {
            Text("Edit the runtime-backed path here, then verify it. Settings and readiness should hand off into this assistant instead of splitting remediation across separate forms.")
                .font(.caption)
                .foregroundStyle(.secondary)

            if let binding = draftProviderBinding {
                Picker("Transport", selection: binding.transport) {
                    ForEach(ProviderTransport.allCases, id: \.self) { transport in
                        Text(transport.displayName).tag(transport)
                    }
                }
                .accessibilityIdentifier("goose-assistant-transport")

                TextField("Goose Endpoint", text: binding.endpoint)
                    .accessibilityIdentifier("goose-assistant-endpoint")

                Picker("Auth Mode", selection: binding.authMode) {
                    ForEach(ProviderAuthMode.allCases, id: \.self) { mode in
                        Text(mode.displayName).tag(mode)
                    }
                }
                .accessibilityIdentifier("goose-assistant-auth-mode")

                TextField("Default Model", text: binding.defaultModel)
                    .accessibilityIdentifier("goose-assistant-default-model")

                LabeledContent("Goose Provider", value: binding.providerIdentifier.wrappedValue)
                    .accessibilityIdentifier("goose-assistant-provider-identifier")
            } else {
                Text("This provider is no longer available.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private var journeyState: GooseProviderJourneyState {
        if isProbing {
            return .probing
        }
        return snapshot?.journeyState ?? .configuredUnverified
    }

    private var draftProviderBinding: (
        transport: Binding<ProviderTransport>,
        endpoint: Binding<String>,
        authMode: Binding<ProviderAuthMode>,
        defaultModel: Binding<String>,
        providerIdentifier: Binding<String>
    )? {
        guard draftProvider != nil else { return nil }
        return (
            transport: Binding(
                get: { draftProvider?.transport ?? .gooseServer },
                set: { newValue in draftProvider?.transport = newValue }
            ),
            endpoint: Binding(
                get: { draftProvider?.endpoint ?? "" },
                set: { value in draftProvider?.endpoint = value.isEmpty ? nil : value }
            ),
            authMode: Binding(
                get: { draftProvider?.authMode ?? .none },
                set: { newValue in draftProvider?.authMode = newValue }
            ),
            defaultModel: Binding(
                get: { draftProvider?.defaultModel ?? "" },
                set: { value in draftProvider?.defaultModel = value.isEmpty ? nil : value }
            ),
            providerIdentifier: Binding(
                get: { draftProvider?.family.runtimeProviderIdentifier ?? "" },
                set: { _ in }
            )
        )
    }

    private func saveAndVerify() async {
        guard let draftProvider else { return }
        providerSettingsStore.upsert(provider: draftProvider)
        lastSavedMessage = "Saved provider settings into the canonical provider store. Running verification next."
        await runProbe()
    }

    private func runProbe() async {
        isProbing = true
        defer { isProbing = false }
        if let refreshedProvider = providerRegistry.configuredProvider(id: providerID) {
            draftProvider = refreshedProvider
        }
        snapshot = await probe.probe(providerID: providerID, origin: origin)
    }

    // MARK: - Proposal 012 (L-04): Journey Progress Indicator

    private var journeyProgressIndicator: some View {
        HStack(spacing: 0) {
            journeyStep(title: "Configure", icon: "gearshape", isComplete: draftProvider != nil, isActive: journeyState == .configuredUnverified)
            journeyConnector(isComplete: draftProvider != nil)
            journeyStep(title: "Verify", icon: "checkmark.shield", isComplete: journeyState == .verified || journeyState == .probing, isActive: journeyState == .probing)
            journeyConnector(isComplete: journeyState == .verified)
            journeyStep(title: "Connected", icon: "link.circle.fill", isComplete: journeyState == .verified, isActive: false)
        }
        .padding(.vertical, DesignTokens.Spacing.small)
    }

    private func journeyStep(title: String, icon: String, isComplete: Bool, isActive: Bool) -> some View {
        VStack(spacing: DesignTokens.Spacing.compact) {
            ZStack {
                Circle()
                    .fill(isComplete ? DesignTokens.Status.success.opacity(0.15) : isActive ? DesignTokens.Action.primary.opacity(0.15) : Color.secondary.opacity(0.1))
                    .frame(width: 32, height: 32)
                if isActive && !isComplete {
                    ProgressView()
                        .controlSize(.small)
                } else {
                    Image(systemName: isComplete ? "checkmark" : icon)
                        .font(.caption.bold())
                        .foregroundStyle(isComplete ? DesignTokens.Status.success : isActive ? DesignTokens.Action.primary : .secondary)
                }
            }
            Text(title)
                .font(DesignTokens.Typography.micro)
                .foregroundStyle(isComplete || isActive ? .primary : .secondary)
        }
    }

    private func journeyConnector(isComplete: Bool) -> some View {
        Rectangle()
            .fill(isComplete ? DesignTokens.Status.success : Color.secondary.opacity(0.3))
            .frame(height: 2)
            .frame(maxWidth: 40)
    }

    private var journeyStateColor: Color {
        switch journeyState {
        case .configuredUnverified: return DesignTokens.Status.neutral
        case .probing: return DesignTokens.Status.running
        case .verified: return DesignTokens.Status.success
        case .degraded: return DesignTokens.Status.warning
        case .failing: return DesignTokens.Status.error
        }
    }

    private var journeyStateIcon: String {
        switch journeyState {
        case .configuredUnverified: return "circle.dashed"
        case .probing: return "arrow.clockwise"
        case .verified: return "checkmark.circle.fill"
        case .degraded: return "exclamationmark.triangle"
        case .failing: return "xmark.circle"
        }
    }
}

#Preview("Goose Assistant") {
    let container = PreviewSupport.makeModelContainer(seed: PreviewSupport.seedOperatorData)
    let appConfigurationStore = PreviewSupport.makeAppConfigurationStore()
    let providerSettingsStore = PreviewSupport.makeProviderSettingsStore()
    let providerRegistry = PreviewSupport.makeProviderRegistry(settingsStore: providerSettingsStore)
    let gooseServerManager = GooseServerManager(appConfigurationStore: appConfigurationStore)
    let providerID = providerSettingsStore.settings.configuredProviders.first?.id ?? UUID()

    return GooseProviderConnectionAssistantView(
        providerID: providerID,
        origin: .providerSettings
    )
    .modelContainer(container)
    .environment(appConfigurationStore)
    .environment(providerSettingsStore)
    .environment(providerRegistry)
    .environment(gooseServerManager)
    .frame(width: 780, height: 720)
}
