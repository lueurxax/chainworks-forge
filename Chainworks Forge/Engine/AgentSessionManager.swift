import Foundation
import SwiftData

struct ProviderSessionConflict: Sendable, Equatable {
    let lineageID: UUID
    let generationID: UUID
    let agentID: String
    let invocationOwnerKey: String
    let runtimeProvider: String
    let runtimeModel: String
    let status: AgentSessionStatus
}

actor AgentSessionManager {
    private let container: ModelContainer
    
    init(container: ModelContainer) {
        self.container = container
    }

    // MARK: - Lineage Management

    func getOrCreateLineage(
        runID: UUID,
        agentID: String,
        scope: SessionReuseScope,
        familyID: String?
    ) async throws -> UUID {
        let fetchContext = ModelContext(container)
        let predicate = #Predicate<AgentSessionLineage> {
            $0.runID == runID && $0.agentID == agentID && $0.sessionFamilyID == familyID
        }
        
        let descriptor = FetchDescriptor<AgentSessionLineage>(predicate: predicate)
        let results = try fetchContext.fetch(descriptor)
        
        if let existing = results.first {
            // Update scope if it changed (e.g. from none to same_invocation_owner)
            if existing.sessionReuseScope != scope {
                existing.sessionReuseScope = scope
                try fetchContext.save()
            }
            return existing.id
        }
        
        let lineage = AgentSessionLineage(
            runID: runID,
            agentID: agentID,
            lineageID: UUID().uuidString,
            sessionReuseScope: scope,
            sessionFamilyID: familyID
        )
        fetchContext.insert(lineage)
        try fetchContext.save()
        return lineage.id
    }
    
    func getLineage(id: UUID) async throws -> AgentSessionLineage? {
        let fetchContext = ModelContext(container)
        let predicate = #Predicate<AgentSessionLineage> { $0.id == id }
        let descriptor = FetchDescriptor<AgentSessionLineage>(predicate: predicate)
        return try fetchContext.fetch(descriptor).first
    }

    func providerSessionConflicts(
        runID: UUID,
        providerSessionID: String,
        excludingLineageID: UUID? = nil
    ) async throws -> [ProviderSessionConflict] {
        let fetchContext = ModelContext(container)
        let predicate = #Predicate<AgentSessionLineage> { $0.runID == runID }
        let descriptor = FetchDescriptor<AgentSessionLineage>(predicate: predicate)
        let lineages = try fetchContext.fetch(descriptor)

        return lineages.compactMap { lineage in
            guard lineage.id != excludingLineageID else { return nil }
            guard let generation = lineage.generations.first(where: {
                $0.providerSessionID == providerSessionID && $0.status == .active
            }) else {
                return nil
            }
            return ProviderSessionConflict(
                lineageID: lineage.id,
                generationID: generation.id,
                agentID: lineage.agentID,
                invocationOwnerKey: generation.invocationOwnerKey,
                runtimeProvider: generation.runtimeProvider,
                runtimeModel: generation.runtimeModel,
                status: generation.status
            )
        }
    }

    func recordEvent(lineageID: UUID, generationID: UUID, type: AgentSessionEventType, detailsJSON: Data? = nil) async throws {
        let recordContext = ModelContext(container)
        let predicate = #Predicate<AgentSessionLineage> { $0.id == lineageID }
        guard let lineage = try recordContext.fetch(FetchDescriptor<AgentSessionLineage>(predicate: predicate)).first else {
            return
        }
        
        let event = AgentSessionEvent(generationID: generationID, eventType: type, detailsJSON: detailsJSON)
        event.lineage = lineage
        recordContext.insert(event)
        try recordContext.save()
    }
    
    func createGeneration(
        lineageID: UUID,
        invocationOwnerKey: String,
        providerSessionID: String?,
        bindingFingerprint: String,
        workingDirectory: String,
        workspaceMode: String,
        runtimeProvider: String,
        runtimeModel: String,
        rehydratedFromCheckpointArtifactID: UUID? = nil
    ) async throws -> UUID {
        let genContext = ModelContext(container)
        let predicate = #Predicate<AgentSessionLineage> { $0.id == lineageID }
        guard let lineage = try genContext.fetch(FetchDescriptor<AgentSessionLineage>(predicate: predicate)).first else {
            throw NSError(domain: "AgentSessionManager", code: 404, userInfo: [NSLocalizedDescriptionKey: "Lineage not found"])
        }

        if let activeGenerationID = lineage.activeGenerationID,
           let previousActiveGeneration = lineage.generations.first(where: { $0.id == activeGenerationID && $0.status == .active }) {
            previousActiveGeneration.status = .invalidated
            previousActiveGeneration.endedAt = Date()
            previousActiveGeneration.endReason = "Superseded by new generation"

            let event = AgentSessionEvent(
                generationID: previousActiveGeneration.id,
                eventType: .invalidated,
                detailsJSON: nil
            )
            event.lineage = lineage
            genContext.insert(event)
        }
        
        let generationCount = lineage.generations.count + 1
        let generation = AgentSessionGeneration(
            generation: generationCount,
            invocationOwnerKey: invocationOwnerKey,
            providerSessionID: providerSessionID,
            bindingFingerprint: bindingFingerprint,
            workingDirectory: workingDirectory,
            workspaceMode: workspaceMode,
            runtimeProvider: runtimeProvider,
            runtimeModel: runtimeModel
        )
        generation.rehydratedFromCheckpointArtifactID = rehydratedFromCheckpointArtifactID
        generation.lineage = lineage
        lineage.activeGenerationID = generation.id
        genContext.insert(generation)
        try genContext.save()
        return generation.id
    }
    
    func updateGenerationUsage(
        generationID: UUID,
        turnIncrement: Int,
        promptTokensIncrement: Int64,
        costCentsIncrement: Int64,
        estimatedInputTokens: Int64?
    ) async throws {
        let updateContext = ModelContext(container)
        let predicate = #Predicate<AgentSessionGeneration> { $0.id == generationID }
        guard let generation = try updateContext.fetch(FetchDescriptor<AgentSessionGeneration>(predicate: predicate)).first else {
            return
        }
        
        generation.turnCount += turnIncrement
        generation.cumulativePromptTokens += promptTokensIncrement
        generation.cumulativeCostCents += costCentsIncrement
        if let tokens = estimatedInputTokens {
            generation.estimatedInputTokens = tokens
        }
        try updateContext.save()
    }

    func invalidateGeneration(generationID: UUID, reason: String) async throws {
        let updateContext = ModelContext(container)
        let predicate = #Predicate<AgentSessionGeneration> { $0.id == generationID }
        guard let generation = try updateContext.fetch(FetchDescriptor<AgentSessionGeneration>(predicate: predicate)).first else {
            return
        }

        generation.status = .invalidated
        generation.endedAt = Date()
        generation.endReason = reason
        generation.lineage?.activeGenerationID = nil
        try updateContext.save()
    }

    func closeGeneration(generationID: UUID, reason: String) async throws {
        let updateContext = ModelContext(container)
        let predicate = #Predicate<AgentSessionGeneration> { $0.id == generationID }
        guard let generation = try updateContext.fetch(FetchDescriptor<AgentSessionGeneration>(predicate: predicate)).first else {
            return
        }

        guard generation.status == .active else {
            if generation.lineage?.activeGenerationID == generation.id {
                generation.lineage?.activeGenerationID = nil
                try updateContext.save()
            }
            return
        }

        generation.status = .closed
        generation.endedAt = Date()
        generation.endReason = reason
        generation.lineage?.activeGenerationID = nil
        try updateContext.save()
    }

    /// Record a `checkpoint_created` event on the lineage (§6.4).
    func recordCheckpointCreated(lineageID: UUID, generationID: UUID, checkpointData: Data?) async throws {
        let ctx = ModelContext(container)
        let predicate = #Predicate<AgentSessionLineage> { $0.id == lineageID }
        guard let lineage = try ctx.fetch(FetchDescriptor<AgentSessionLineage>(predicate: predicate)).first else {
            return
        }

        let event = AgentSessionEvent(generationID: generationID, eventType: .checkpoint_created, detailsJSON: checkpointData)
        event.lineage = lineage
        ctx.insert(event)

        // Update generation's lastCheckpointArtifactID marker
        if let generation = lineage.generations.first(where: { $0.id == generationID }) {
            // For inline checkpoints, we don't have an artifact UUID yet — the caller can set it later.
            // Mark that a checkpoint was emitted.
            generation.lastCheckpointArtifactID = generation.lastCheckpointArtifactID ?? generationID
        }

        try ctx.save()
    }

    /// Reset session lineage (§6.9).
    ///
    /// 1. Mark the current `AgentSessionGeneration` as ended with `endReason = operator_reset`
    /// 2. Append `operator_reset` to events
    /// 3. Set `activeGenerationID` to nil
    /// 4. Next invocation for this lineage will produce `.fresh_after_reset` via `SessionReusePolicy`
    func resetSession(lineageID: UUID, reason: String? = nil) async throws {
        let updateContext = ModelContext(container)
        let predicate = #Predicate<AgentSessionLineage> { $0.id == lineageID }
        guard let lineage = try updateContext.fetch(FetchDescriptor<AgentSessionLineage>(predicate: predicate)).first else {
            return
        }

        let resetReason = reason ?? "operator_reset"

        if let activeID = lineage.activeGenerationID,
           let activeGen = lineage.generations.first(where: { $0.id == activeID }) {
            activeGen.status = .reset
            activeGen.endedAt = Date()
            activeGen.endReason = resetReason

            let detailsJSON = try? JSONEncoder().encode(["reason": resetReason])
            let event = AgentSessionEvent(generationID: activeID, eventType: .operator_reset, detailsJSON: detailsJSON)
            event.lineage = lineage
            updateContext.insert(event)
        }

        lineage.activeGenerationID = nil
        try updateContext.save()
    }
}
