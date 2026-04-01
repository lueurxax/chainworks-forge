import Foundation

struct StrategyExperimentCoordinator: Sendable {
    private let config: StewardConfig

    init(config: StewardConfig?) {
        self.config = config ?? .defaultConfig
    }

    func resolveSelection(
        selectedProfileID: String?,
        cohortID: UUID? = nil
    ) -> ContextStrategySelection {
        if let selectedProfileID,
           !selectedProfileID.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return ContextStrategyResolver.resolveSelection(
                selectedProfileID: selectedProfileID,
                config: config
            )
        }

        if let cohortID {
            let profileIDs = config.contextStrategyProfiles.keys.sorted()
            if let assignedProfileID = profileIDs[safe: abs(cohortID.hashValue) % max(1, profileIDs.count)] {
                let selection = ContextStrategyResolver.resolveSelection(
                    selectedProfileID: assignedProfileID,
                    config: config
                )
                return ContextStrategySelection(
                    profileID: selection.profileID,
                    assignmentMode: "experiment_cohort",
                    recommendationState: selection.recommendationState,
                    profile: selection.profile
                )
            }
        }

        return ContextStrategyResolver.resolveSelection(
            selectedProfileID: nil,
            config: config
        )
    }
}

private extension Array {
    subscript(safe index: Int) -> Element? {
        guard indices.contains(index) else { return nil }
        return self[index]
    }
}
