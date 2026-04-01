import Foundation

/// Frozen run-start snapshot persisted atomically with Run creation.
/// This is the shared boundary between editable UI drafts and immutable run truth.
struct RunStartSnapshot: Sendable {
    let providerBindingSnapshotJSON: Data?
    let bindingProvenanceJSON: Data?
    let startOptionsJSON: Data?
    let frozenWorkspaceRootPath: String?
    let deliveryConfiguration: DeliveryConfiguration?
    let deliveryPreflightJSON: Data?
    let contextStrategyProfileID: String?
    let strategyAssignmentMode: String?
    let strategyRecommendationState: String?
    let contextStrategySnapshotJSON: Data?
    let promotedHandoffArtifactsJSON: Data?

    init(
        providerBindingSnapshotJSON: Data? = nil,
        bindingProvenanceJSON: Data? = nil,
        startOptionsJSON: Data? = nil,
        frozenWorkspaceRootPath: String? = nil,
        deliveryConfiguration: DeliveryConfiguration? = nil,
        deliveryPreflightJSON: Data? = nil,
        contextStrategyProfileID: String? = nil,
        strategyAssignmentMode: String? = nil,
        strategyRecommendationState: String? = nil,
        contextStrategySnapshotJSON: Data? = nil,
        promotedHandoffArtifactsJSON: Data? = nil
    ) {
        self.providerBindingSnapshotJSON = providerBindingSnapshotJSON
        self.bindingProvenanceJSON = bindingProvenanceJSON
        self.startOptionsJSON = startOptionsJSON
        self.frozenWorkspaceRootPath = frozenWorkspaceRootPath
        self.deliveryConfiguration = deliveryConfiguration
        self.deliveryPreflightJSON = deliveryPreflightJSON
        self.contextStrategyProfileID = contextStrategyProfileID
        self.strategyAssignmentMode = strategyAssignmentMode
        self.strategyRecommendationState = strategyRecommendationState
        self.contextStrategySnapshotJSON = contextStrategySnapshotJSON
        self.promotedHandoffArtifactsJSON = promotedHandoffArtifactsJSON
    }

    static let empty = RunStartSnapshot(
        providerBindingSnapshotJSON: nil,
        bindingProvenanceJSON: nil,
        startOptionsJSON: nil,
        frozenWorkspaceRootPath: nil,
        deliveryConfiguration: nil,
        deliveryPreflightJSON: nil,
        contextStrategyProfileID: nil,
        strategyAssignmentMode: nil,
        strategyRecommendationState: nil,
        contextStrategySnapshotJSON: nil,
        promotedHandoffArtifactsJSON: nil
    )

    func apply(to run: Run) {
        run.providerBindingSnapshotJSON = providerBindingSnapshotJSON
        run.bindingProvenanceJSON = bindingProvenanceJSON
        run.startOptionsJSON = startOptionsJSON
        run.frozenWorkspaceRootPath = frozenWorkspaceRootPath
        run.deliveryPreflightJSON = deliveryPreflightJSON
        run.contextStrategyProfileID = contextStrategyProfileID ?? run.contextStrategyProfileID
        run.strategyAssignmentMode = strategyAssignmentMode ?? run.strategyAssignmentMode
        run.strategyRecommendationState = strategyRecommendationState ?? run.strategyRecommendationState
        run.contextStrategySnapshotJSON = contextStrategySnapshotJSON
        run.promotedHandoffArtifactsJSON = promotedHandoffArtifactsJSON ?? run.promotedHandoffArtifactsJSON

        guard let deliveryConfiguration else { return }
        let encoder = JSONEncoder()
        run.deliveryConfigurationJSON = try? encoder.encode(deliveryConfiguration)
        run.repoIdentifier = deliveryConfiguration.repoIdentifier
        run.repoRoot = deliveryConfiguration.repoRoot
        run.baseBranch = deliveryConfiguration.baseBranch
        run.targetBranch = deliveryConfiguration.targetBranch
        run.releaseTargetID = deliveryConfiguration.releaseTargetID
        run.releaseMode = deliveryConfiguration.releaseMode.rawValue
    }

    static func from(run: Run) -> RunStartSnapshot {
        let deliveryConfiguration: DeliveryConfiguration?
        if let data = run.deliveryConfigurationJSON {
            deliveryConfiguration = try? JSONDecoder().decode(DeliveryConfiguration.self, from: data)
        } else {
            deliveryConfiguration = nil
        }

        return RunStartSnapshot(
            providerBindingSnapshotJSON: run.providerBindingSnapshotJSON,
            bindingProvenanceJSON: run.bindingProvenanceJSON,
            startOptionsJSON: run.startOptionsJSON,
            frozenWorkspaceRootPath: run.frozenWorkspaceRootPath,
            deliveryConfiguration: deliveryConfiguration,
            deliveryPreflightJSON: run.deliveryPreflightJSON,
            contextStrategyProfileID: run.contextStrategyProfileID,
            strategyAssignmentMode: run.strategyAssignmentMode,
            strategyRecommendationState: run.strategyRecommendationState,
            contextStrategySnapshotJSON: run.contextStrategySnapshotJSON,
            promotedHandoffArtifactsJSON: run.promotedHandoffArtifactsJSON
        )
    }
}
