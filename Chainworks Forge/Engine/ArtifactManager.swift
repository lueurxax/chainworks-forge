import Foundation
import SwiftData

// MARK: - ArtifactManager (@MainActor SwiftData metadata adapter — ARCH-023)

/// Bridges nonisolated ArtifactStorage (disk I/O) with @MainActor SwiftData metadata.
/// Uses ArtifactContract.format for format detection, not hardcoded assumptions.
final class ArtifactManager {
    private let modelContext: ModelContext

    @MainActor
    init(modelContext: ModelContext) {
        self.modelContext = modelContext
    }

    /// Persist agent outputs: write data to disk via ArtifactStorage, then create SwiftData Artifact records.
    /// - Parameters:
    ///   - outputs: The agent's output data (name -> Data), from AgentResult.outputs.
    ///   - agent: The resolved agent that produced the outputs.
    ///   - agentExecution: The AgentExecution SwiftData record to attach artifacts to.
    ///   - workspace: The run's workspace (for path boundaries).
    ///   - stageID: Current stage identifier.
    ///   - iteration: Current iteration within the stage.
    ///   - attemptNumber: Current attempt number.
    ///   - catalog: Optional agent catalog for contract-based format detection.
    /// - Returns: The created Artifact records.
    @discardableResult
    @MainActor
    func persistOutputs(
        outputs: [String: Data],
        agent: ResolvedAgent,
        agentExecution: AgentExecution,
        workspace: RunWorkspace,
        stageID: String,
        iteration: Int,
        attemptNumber: Int,
        catalog: AgentCatalog? = nil
    ) throws -> [Artifact] {
        var artifacts: [Artifact] = []

        for (name, data) in outputs {
            // Write to disk (nonisolated)
            let storageResult = try ArtifactStorage.write(
                data: data,
                name: name,
                stageID: stageID,
                iteration: iteration,
                agentID: agent.id,
                attemptNumber: attemptNumber,
                artifactRoot: workspace.artifactRoot,
                workspaceRoot: workspace.workspaceRoot
            )

            // Determine format from catalog contract, not hardcoded
            let format = resolveFormat(
                outputName: name,
                agent: agent,
                catalog: catalog
            )
            let contractID = OutputContractResolver.contractID(
                for: name,
                agent: agent,
                catalog: catalog
            ) ?? "none"

            // Create SwiftData record
            let artifact = Artifact(
                name: name,
                contractID: contractID,
                format: format,
                filePath: storageResult.filePath,
                runID: workspace.runID,
                stageID: stageID,
                agentID: agent.id,
                provider: agent.provider,
                attemptNumber: attemptNumber
            )
            artifact.checksumSHA256 = storageResult.checksumSHA256
            artifact.sizeBytes = storageResult.sizeBytes
            artifact.model = agent.model
            artifact.effort = agent.effort
            artifact.agentExecution = agentExecution

            modelContext.insert(artifact)
            artifacts.append(artifact)
        }

        return artifacts
    }

    /// Read artifact data from disk, validating path boundaries.
    @MainActor
    func readArtifact(_ artifact: Artifact, workspace: RunWorkspace) throws -> Data {
        try ArtifactStorage.read(
            filePath: artifact.filePath,
            workspaceRoot: workspace.workspaceRoot
        )
    }

    /// Query all artifacts for a run.
    @MainActor
    func artifacts(forRunID runID: UUID) throws -> [Artifact] {
        let descriptor = FetchDescriptor<Artifact>(
            sortBy: [SortDescriptor(\.createdAt)]
        )
        return try modelContext.fetch(descriptor)
            .filter { $0.runID == runID }
    }

    /// Query artifacts by stage.
    @MainActor
    func artifacts(forRunID runID: UUID, stageID: String) throws -> [Artifact] {
        let descriptor = FetchDescriptor<Artifact>(
            sortBy: [SortDescriptor(\.createdAt)]
        )
        return try modelContext.fetch(descriptor)
            .filter { $0.runID == runID && $0.stageID == stageID }
    }

    /// Get the set of produced artifact names for a run (for TransitionEvaluator).
    @MainActor
    func producedArtifactNames(forRunID runID: UUID) throws -> Set<String> {
        let artifacts = try artifacts(forRunID: runID)
        return Set(artifacts.map(\.name))
    }

    /// Persist a system-generated artifact that is not attached to a specific agent execution.
    @discardableResult
    @MainActor
    func persistSystemArtifact(
        name: String,
        data: Data,
        contractID: String,
        format: ArtifactFormat,
        workspace: RunWorkspace,
        stageID: String,
        agentID: String,
        provider: String,
        model: String?,
        effort: String?,
        attemptNumber: Int
    ) throws -> Artifact {
        let storageResult = try ArtifactStorage.write(
            data: data,
            name: name,
            stageID: stageID,
            iteration: 1,
            agentID: agentID,
            attemptNumber: attemptNumber,
            artifactRoot: workspace.artifactRoot,
            workspaceRoot: workspace.workspaceRoot
        )

        let artifact = Artifact(
            name: name,
            contractID: contractID,
            format: format,
            filePath: storageResult.filePath,
            runID: workspace.runID,
            stageID: stageID,
            agentID: agentID,
            provider: provider,
            attemptNumber: attemptNumber
        )
        artifact.checksumSHA256 = storageResult.checksumSHA256
        artifact.sizeBytes = storageResult.sizeBytes
        artifact.model = model
        artifact.effort = effort

        modelContext.insert(artifact)
        return artifact
    }

    // MARK: - Format Resolution

    /// Resolve artifact format using the proposal §7.3 contract:
    /// Priority: file extension > contract.format > .report fallback.
    /// Delegates to `ArtifactFormat.detect(from:contract:)`.
    private func resolveFormat(
        outputName: String,
        agent: ResolvedAgent,
        catalog: AgentCatalog?
    ) -> ArtifactFormat {
        let contract = OutputContractResolver.contract(
            for: outputName,
            agent: agent,
            catalog: catalog
        )

        if contract == nil,
           let hintedPath = catalog?.artifacts[outputName] {
            return ArtifactFormat.detect(from: hintedPath, contract: nil)
        }

        return ArtifactFormat.detect(from: outputName, contract: contract)
    }
}
