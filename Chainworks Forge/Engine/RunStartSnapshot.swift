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

    nonisolated static let empty = RunStartSnapshot(
        providerBindingSnapshotJSON: nil,
        bindingProvenanceJSON: nil,
        startOptionsJSON: nil,
        frozenWorkspaceRootPath: nil,
        deliveryConfiguration: nil,
        deliveryPreflightJSON: nil
    )

    func apply(to run: Run) {
        run.providerBindingSnapshotJSON = providerBindingSnapshotJSON
        run.bindingProvenanceJSON = bindingProvenanceJSON
        run.startOptionsJSON = startOptionsJSON
        run.frozenWorkspaceRootPath = frozenWorkspaceRootPath
        run.deliveryPreflightJSON = deliveryPreflightJSON

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
            deliveryPreflightJSON: run.deliveryPreflightJSON
        )
    }
}
