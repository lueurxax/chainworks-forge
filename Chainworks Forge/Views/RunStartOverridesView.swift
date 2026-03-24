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
        VStack(alignment: .leading, spacing: 12) {
            if profileGroups.isEmpty {
                Text("No backend-profile overrides are available for this run.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            ForEach(profileGroups) { group in
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
