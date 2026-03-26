import SwiftUI

struct RunStartOverridesView: View {
    let plan: RunPlan
    let providerRegistry: ProviderRegistry
    @Binding var startOptions: RunStartOptions

    private var profileGroups: [BackendProfileGroup] {
        Dictionary(grouping: plan.agentBindings.values.compactMap { agent -> (String, ResolvedAgent)? in
            guard let backendProfileID = agent.backendProfileID else { return nil }
            return (backendProfileID, agent)
        }, by: \.0)
            .map { backendProfileID, pairs in
                let agents = pairs.map(\.1)
                return BackendProfileGroup(
                    backendProfileID: backendProfileID,
                    providerFamily: ProviderFamily.from(runtimeIdentifier: agents.first?.provider ?? ""),
                    agentTitles: agents.map(\.title).sorted()
                )
            }
            .sorted { $0.backendProfileID < $1.backendProfileID }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            if profileGroups.isEmpty {
                Text("No backend-profile overrides are available for this run.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            ForEach(Array(profileGroups.enumerated()), id: \.element.id) { index, group in
                if index > 0 {
                    Divider()
                        .padding(.vertical, 6)
                }
                OverrideRow(
                    group: group,
                    providerRegistry: providerRegistry,
                    overrideValue: Binding(
                        get: { startOptions.overridesByBackendProfileID[group.backendProfileID] ?? RunStartOverride() },
                        set: { newValue in
                            startOptions.overridesByBackendProfileID[group.backendProfileID] = newValue
                        }
                    )
                )
            }
        }
    }
}

private struct BackendProfileGroup: Identifiable {
    let backendProfileID: String
    let providerFamily: ProviderFamily?
    let agentTitles: [String]

    var id: String { backendProfileID }
}

// MARK: - Dividers between override groups

private struct OverrideRow: View {
    let group: BackendProfileGroup
    let providerRegistry: ProviderRegistry
    @Binding var overrideValue: RunStartOverride

    private var providerOptions: [ConfiguredProvider] {
        guard let family = group.providerFamily else { return [] }
        return providerRegistry.configuredProviders.filter { $0.family == family }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(group.backendProfileID)
                .font(.subheadline.bold())
            Text(group.agentTitles.joined(separator: ", "))
                .font(.caption)
                .foregroundStyle(.secondary)

            Picker("Provider", selection: Binding(
                get: { overrideValue.configuredProviderID },
                set: { overrideValue.configuredProviderID = $0 }
            )) {
                Text("Default").tag(UUID?.none)
                ForEach(providerOptions) { provider in
                    Text(provider.displayName).tag(Optional(provider.id))
                }
            }

            TextField("Model Override", text: Binding(
                get: { overrideValue.model ?? "" },
                set: { overrideValue.model = $0.isEmpty ? nil : $0 }
            ))

            TextField("Effort Override", text: Binding(
                get: { overrideValue.effort ?? "" },
                set: { overrideValue.effort = $0.isEmpty ? nil : $0 }
            ))
        }
        .padding(.vertical, 4)
    }
}

// MARK: - Preview

#Preview("Override List — 8 agents") {
    @Previewable @State var options = RunStartOptions.empty

    let agents: [String: ResolvedAgent] = {
        let ids = [
            ("claude_prepush_medium", "Pre-push Code Reviewer", "claude_code"),
            ("claude_product_high", "Proposal Reviewer / Product Owner", "claude_code"),
            ("claude_security_high", "Security Checker", "claude_code"),
            ("claude_writer_high", "Proposal Writer", "claude_code"),
            ("codex_architect_high", "Proposal Reviewer / Architect", "codex"),
            ("codex_audit_high", "Proposal vs Implementation Auditor", "codex"),
            ("codex_implementer_high", "Feature Implementer", "codex"),
            ("gemini_reviewer_medium", "Style & Convention Reviewer", "gemini"),
        ]
        return Dictionary(uniqueKeysWithValues: ids.map { id, title, provider in
            (id, ResolvedAgent(
                id: id, title: title, mode: "tool_use",
                backendProfileID: id,
                provider: provider, model: "default", effort: "high",
                maxTurns: 10, temperature: 0,
                permissionProfile: "ORCH", skillRef: "sk1", skillRole: nil,
                prompt: "preview", outputContract: nil,
                requiresHumanApproval: false, inputs: [], outputs: ["out"]
            ))
        })
    }()

    let plan = RunPlan(
        workflowID: "proposal_loop_live",
        workflowTitle: "Proposal Loop (Live)",
        states: [:], initialStateID: "s1",
        agentBindings: agents, variables: [:],
        scoring: nil, failurePolicy: nil,
        requiresProjectAccess: false,
        workflowSnapshotHash: "abc", catalogSnapshotHash: "def",
        workflowSnapshotJSON: Data(), catalogSnapshotJSON: Data(),
        planCompilerVersion: 1
    )

    let settingsStore = ProviderSettingsStore(
        fileURL: FileManager.default.temporaryDirectory.appendingPathComponent("preview-providers.json"),
        initialSettings: .empty
    )
    let registry = ProviderRegistry(
        settingsStore: settingsStore,
        secretStore: KeychainSecretStore(useInMemoryStore: true)
    )

    ScrollView {
        GroupBox("Run Start Overrides") {
            RunStartOverridesView(
                plan: plan,
                providerRegistry: registry,
                startOptions: $options
            )
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
    .frame(width: 520, height: 600)
    .padding()
}
