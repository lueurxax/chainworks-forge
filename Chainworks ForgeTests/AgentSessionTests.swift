import Testing
import SwiftData
import Foundation
@testable import Chainworks_Forge

@MainActor
@Suite("Agent Session Lineage (P018)", .serialized, .tags(.fast))
struct AgentSessionTests {
    let container: ModelContainer
    let context: ModelContext

    init() throws {
        let schema = Schema([
            Idea.self, Run.self, StageExecution.self, AgentExecution.self, 
            Artifact.self, AgentSessionLineage.self, AgentSessionGeneration.self, 
            AgentSessionEvent.self
        ])
        let config = ModelConfiguration("AgentSessionTests-\(UUID().uuidString)", schema: schema, isStoredInMemoryOnly: true)
        container = try ModelContainer(for: schema, configurations: [config])
        TestModelContainerRetainer.retain(container)
        context = container.mainContext
    }

    @Test("Sanity check for model insertion")
    func sanityCheck() throws {
        let runID = UUID()
        let lineage = AgentSessionLineage(runID: runID, agentID: "a", lineageID: "l")
        context.insert(lineage)
        
        let gen = AgentSessionGeneration(generation: 1, invocationOwnerKey: "o", bindingFingerprint: "f", workingDirectory: "w", workspaceMode: "r", runtimeProvider: "p", runtimeModel: "m")
        gen.lineage = lineage
        context.insert(gen)
        
        try context.save()
        
        #expect(lineage.generations.count == 1)
        #expect(lineage.generations.first?.id == gen.id)
    }

    @Test("SessionReusePolicy correctly evaluates same_invocation_owner reuse")
    func evaluateSameInvocationOwner() throws {
        let runID = UUID()
        let lineage = AgentSessionLineage(runID: runID, agentID: "agent_1", lineageID: "lin_1", sessionReuseScope: .same_invocation_owner)
        context.insert(lineage)
        
        let generation = AgentSessionGeneration(
            generation: 1, 
            invocationOwnerKey: "owner_1", 
            providerSessionID: "sess_1", 
            bindingFingerprint: "fp_1", 
            workingDirectory: "/tmp", 
            workspaceMode: "read_only", 
            runtimeProvider: "p1", 
            runtimeModel: "m1"
        )
        generation.lineage = lineage
        context.insert(generation)
        
        lineage.activeGenerationID = generation.id
        try context.save()
        
        ForgeLogger.test.debug("Lineage count: \(lineage.generations.count)")
        
        // 1. Success case: same owner, same fingerprint
        let decision1 = SessionReusePolicy.evaluate(
            lineage: lineage, 
            currentInvocationOwnerKey: "owner_1", 
            currentBindingFingerprint: "fp_1", 
            currentRecoveryBranchID: nil
        )
        ForgeLogger.test.debug("Decision 1: \(decision1)")
        if case .reuse(let gen) = decision1 {
            #expect(gen.id == generation.id)
        } else {
            Issue.record("Expected reuse decision")
        }
        
        // 2. Failure case: different owner
        let decision2 = SessionReusePolicy.evaluate(
            lineage: lineage, 
            currentInvocationOwnerKey: "owner_2", 
            currentBindingFingerprint: "fp_1", 
            currentRecoveryBranchID: nil
        )
        if case .createFresh(let disposition, _) = decision2 {
            #expect(disposition == .fresh)
        } else {
            Issue.record("Expected fresh session due to owner mismatch")
        }
        
        // 3. Failure case: different fingerprint
        let decision3 = SessionReusePolicy.evaluate(
            lineage: lineage, 
            currentInvocationOwnerKey: "owner_1", 
            currentBindingFingerprint: "fp_2", 
            currentRecoveryBranchID: nil
        )
        if case .createFresh(let disposition, _) = decision3 {
            #expect(disposition == .fresh_session_required)
        } else {
            Issue.record("Expected fresh session due to fingerprint mismatch")
        }
        
        // 4. Unverifiable history case: lineage exists but active generation not found in generations list
        lineage.activeGenerationID = UUID() // Non-existent ID
        let decision4 = SessionReusePolicy.evaluate(
            lineage: lineage, 
            currentInvocationOwnerKey: "owner_1", 
            currentBindingFingerprint: "fp_1", 
            currentRecoveryBranchID: nil
        )
        if case .createFresh(let disposition, _) = decision4 {
            #expect(disposition == .unverifiable_session_history)
        } else {
            Issue.record("Expected unverifiable_session_history decision")
        }

        // 5. Invalidated session case
        generation.status = .invalidated
        lineage.activeGenerationID = generation.id
        let decision5 = SessionReusePolicy.evaluate(lineage: lineage, currentInvocationOwnerKey: "owner_1", currentBindingFingerprint: "fp_1", currentRecoveryBranchID: nil)
        if case .createFresh(let disposition, _) = decision5 {
            #expect(disposition == .fresh_after_invalidation)
        } else {
            Issue.record("Expected fresh_after_invalidation")
        }

        // 6. Reset session case
        generation.status = .reset
        let decision6 = SessionReusePolicy.evaluate(lineage: lineage, currentInvocationOwnerKey: "owner_1", currentBindingFingerprint: "fp_1", currentRecoveryBranchID: nil)
        if case .createFresh(let disposition, _) = decision6 {
            #expect(disposition == .fresh_after_reset)
        } else {
            Issue.record("Expected fresh_after_reset")
        }
    }

    @Test("SessionReusePolicy correctly evaluates same_agent_family_within_run reuse")
    func evaluateFamilyReuse() throws {
        let runID = UUID()
        let lineage = AgentSessionLineage(
            runID: runID, 
            agentID: "agent_1", 
            lineageID: "lin_1", 
            sessionReuseScope: .same_agent_family_within_run,
            sessionFamilyID: "fam_1"
        )
        context.insert(lineage)
        
        let generation = AgentSessionGeneration(
            generation: 1, 
            invocationOwnerKey: "owner_1", 
            providerSessionID: "sess_1", 
            bindingFingerprint: "fp_1", 
            workingDirectory: "/tmp", 
            workspaceMode: "read_only", 
            runtimeProvider: "p1", 
            runtimeModel: "m1"
        )
        generation.lineage = lineage
        context.insert(generation)
        
        lineage.activeGenerationID = generation.id
        try context.save()
        
        // Success case: different owner, same family, same fingerprint
        let decision = SessionReusePolicy.evaluate(
            lineage: lineage, 
            currentInvocationOwnerKey: "owner_2", 
            currentBindingFingerprint: "fp_1", 
            currentRecoveryBranchID: nil
        )
        if case .reuse(let gen) = decision {
            #expect(gen.id == generation.id)
        } else {
            Issue.record("Expected family reuse decision")
        }
    }

    @Test("ContextBudgetGuard invalidates session on threshold exceeded")
    func budgetGuardThresholds() throws {
        let generation = AgentSessionGeneration(
            generation: 1, 
            invocationOwnerKey: "o1", 
            bindingFingerprint: "f1", 
            workingDirectory: "w1", 
            workspaceMode: "rw", 
            runtimeProvider: "p1", 
            runtimeModel: "m1"
        )
        
        let config = ContextBudgetGuard.BudgetConfig(
            maxTurns: 2,
            maxEstimatedInputTokens: 1000,
            maxCumulativePromptTokens: 5000,
            maxCumulativeCostCents: 10,
            maxIdleAgeSeconds: 3600,
            maxTranscriptGrowthRatio: 2.0,
            minCachedTokenShare: 0.2,
            maxReuseCostPenaltyCents: 5.0,
            maxEffectivePromptSizeFraction: 0.5
        )

        // 1. Within budget
        let d1 = ContextBudgetGuard.evaluate(generation: generation, config: config)
        if case .continueReuse = d1 {} else { Issue.record("Expected continue") }
        
        // 2. Turns exceeded
        generation.turnCount = 3
        let d2 = ContextBudgetGuard.evaluate(generation: generation, config: config)
        if case .compact = d2 {} else { Issue.record("Expected compact due to turns") }
        
        // 3. Cost exceeded
        generation.turnCount = 1
        generation.cumulativeCostCents = 11
        let d3 = ContextBudgetGuard.evaluate(generation: generation, config: config)
        if case .invalidate = d3 {} else { Issue.record("Expected invalidate due to cost") }
    }

    @Test("ContextBudgetGuard uses economic signals for decisions")
    func budgetGuardEconomicSignals() throws {
        let generation = AgentSessionGeneration(
            generation: 1, 
            invocationOwnerKey: "o1", 
            bindingFingerprint: "f1", 
            workingDirectory: "w1", 
            workspaceMode: "ro", 
            runtimeProvider: "p1", 
            runtimeModel: "m1"
        )
        generation.estimatedInputTokens = 60_000
        
        // 1. Low cache hit rate triggers compaction
        let signals1 = ContextBudgetGuard.EconomicSignals(
            cachedTokenShare: 0.1,
            normalizedSavingsVersusFresh: 0,
            transcriptGrowthRatio: 1.0,
            effectivePromptSizeFraction: nil,
            compactionChurnCount: nil
        )
        let d1 = ContextBudgetGuard.evaluate(generation: generation, signals: signals1)
        if case .compact(let reason) = d1 {
            #expect(reason.contains("cache hit rate"))
        } else {
            Issue.record("Expected compact due to low cache hit")
        }

        // 2. High growth triggers compaction
        let signals2 = ContextBudgetGuard.EconomicSignals(
            cachedTokenShare: 0.8,
            normalizedSavingsVersusFresh: 0,
            transcriptGrowthRatio: 2.1,
            effectivePromptSizeFraction: nil,
            compactionChurnCount: nil
        )
        let d2 = ContextBudgetGuard.evaluate(generation: generation, signals: signals2)
        if case .compact(let reason) = d2 {
            #expect(reason.contains("Transcript growth"))
        } else {
            Issue.record("Expected compact due to growth")
        }

        // 3. High cost penalty triggers invalidation
        let signals3 = ContextBudgetGuard.EconomicSignals(
            cachedTokenShare: 0.5,
            normalizedSavingsVersusFresh: -6.0,
            transcriptGrowthRatio: 1.0,
            effectivePromptSizeFraction: nil,
            compactionChurnCount: nil
        )
        let d3 = ContextBudgetGuard.evaluate(generation: generation, signals: signals3)
        if case .invalidate(let reason) = d3 {
            #expect(reason.contains("Reuse cost penalty"))
        } else {
            Issue.record("Expected invalidate due to cost penalty")
        }

        // 4. Effective prompt size exceeds context window fraction
        generation.turnCount = 1
        generation.cumulativeCostCents = 0
        let signals4 = ContextBudgetGuard.EconomicSignals(
            cachedTokenShare: 0.8,
            normalizedSavingsVersusFresh: 0,
            transcriptGrowthRatio: 1.0,
            effectivePromptSizeFraction: 0.6,
            compactionChurnCount: nil
        )
        let d4 = ContextBudgetGuard.evaluate(generation: generation, signals: signals4)
        if case .compact(let reason) = d4 {
            #expect(reason.contains("Effective prompt size"))
        } else {
            Issue.record("Expected compact due to effective prompt size")
        }

        // 5. High compaction churn triggers invalidation
        let signals5 = ContextBudgetGuard.EconomicSignals(
            cachedTokenShare: 0.8,
            normalizedSavingsVersusFresh: 0,
            transcriptGrowthRatio: 1.0,
            effectivePromptSizeFraction: 0.3,
            compactionChurnCount: 3
        )
        let d5 = ContextBudgetGuard.evaluate(generation: generation, signals: signals5)
        if case .invalidate(let reason) = d5 {
            #expect(reason.contains("Compaction churn"))
        } else {
            Issue.record("Expected invalidate due to compaction churn")
        }
    }

    @Test("AgentSessionManager manages lineage and generations")
    func managerLifecycle() async throws {
        let manager = AgentSessionManager(container: container)
        let runID = UUID()

        // 1. Create lineage
        let lid = try await manager.getOrCreateLineage(
            runID: runID,
            agentID: "a1",
            scope: .same_invocation_owner,
            familyID: nil
        )

        let lineage = try await manager.getLineage(id: lid)
        #expect(lineage?.agentID == "a1")
        #expect(lineage?.activeGenerationID == nil)

        // 2. Create generation
        let gid = try await manager.createGeneration(
            lineageID: lid,
            invocationOwnerKey: "o1",
            providerSessionID: "s1",
            bindingFingerprint: "f1",
            workingDirectory: "/tmp",
            workspaceMode: "ro",
            runtimeProvider: "p1",
            runtimeModel: "m1"
        )

        // Re-fetch to see actor-isolated changes
        let afterGen = try await manager.getLineage(id: lid)
        #expect(afterGen?.activeGenerationID == gid)

        // 3. Record event
        try await manager.recordEvent(lineageID: lid, generationID: gid, type: .reused)
        let afterEvent = try await manager.getLineage(id: lid)
        #expect(afterEvent?.events.count == 1)
        #expect(afterEvent?.events.first?.eventType == .reused)

        // 4. Invalidate
        try await manager.invalidateGeneration(generationID: gid, reason: "test")
        let afterInvalidate = try await manager.getLineage(id: lid)
        #expect(afterInvalidate?.activeGenerationID == nil)
        #expect(afterInvalidate?.generations.first?.status == .invalidated)

        // 5. Reset
        let _ = try await manager.createGeneration(lineageID: lid, invocationOwnerKey: "o2", providerSessionID: "s2", bindingFingerprint: "f1", workingDirectory: "/tmp", workspaceMode: "ro", runtimeProvider: "p1", runtimeModel: "m1")
        try await manager.resetSession(lineageID: lid)
        let lineageAfterReset = try await manager.getLineage(id: lid)
        #expect(lineageAfterReset?.activeGenerationID == nil)
        #expect(lineageAfterReset?.generations.last?.status == .reset)
        #expect(lineageAfterReset?.events.contains { $0.eventType == .operator_reset } == true)
    }

    @Test("AgentSessionManager distinguishes lineages by familyID")
    func managerFamilyIDDistinction() async throws {
        let manager = AgentSessionManager(container: container)
        let runID = UUID()
        
        let lid1 = try await manager.getOrCreateLineage(runID: runID, agentID: "a1", scope: .same_agent_family_within_run, familyID: "f1")
        let lid2 = try await manager.getOrCreateLineage(runID: runID, agentID: "a1", scope: .same_agent_family_within_run, familyID: "f2")
        let lid3 = try await manager.getOrCreateLineage(runID: runID, agentID: "a1", scope: .same_invocation_owner, familyID: nil)
        
        #expect(lid1 != lid2)
        #expect(lid1 != lid3)
        #expect(lid2 != lid3)
    }

    @Test("AgentSessionManager reports cross-lineage provider session collisions within a run")
    func managerReportsProviderSessionConflicts() async throws {
        let manager = AgentSessionManager(container: container)
        let runID = UUID()

        let leadLineageID = try await manager.getOrCreateLineage(
            runID: runID,
            agentID: "lead_orchestrator",
            scope: .same_agent_family_within_run,
            familyID: "orchestration_loop"
        )
        let reviewerLineageID = try await manager.getOrCreateLineage(
            runID: runID,
            agentID: "proposal_reviewer_ui",
            scope: .same_invocation_owner,
            familyID: nil
        )

        _ = try await manager.createGeneration(
            lineageID: leadLineageID,
            invocationOwnerKey: "lead-owner",
            providerSessionID: "dup-session",
            bindingFingerprint: "lead-fp",
            workingDirectory: "/tmp/lead",
            workspaceMode: "ro",
            runtimeProvider: "claude_code",
            runtimeModel: "opus"
        )
        _ = try await manager.createGeneration(
            lineageID: reviewerLineageID,
            invocationOwnerKey: "review-owner",
            providerSessionID: "dup-session",
            bindingFingerprint: "review-fp",
            workingDirectory: "/tmp/review",
            workspaceMode: "ro",
            runtimeProvider: "gemini",
            runtimeModel: "gemini-2.5-pro"
        )

        let conflicts = try await manager.providerSessionConflicts(
            runID: runID,
            providerSessionID: "dup-session",
            excludingLineageID: leadLineageID
        )

        #expect(conflicts.count == 1)
        #expect(conflicts.first?.agentID == "proposal_reviewer_ui")
        #expect(conflicts.first?.runtimeProvider == "gemini")
        #expect(conflicts.first?.runtimeModel == "gemini-2.5-pro")
    }

    @Test("AgentSessionManager ignores closed generations when checking provider session collisions")
    func managerIgnoresClosedGenerationsInProviderSessionConflicts() async throws {
        let manager = AgentSessionManager(container: container)
        let runID = UUID()

        let leadLineageID = try await manager.getOrCreateLineage(
            runID: runID,
            agentID: "lead_orchestrator",
            scope: .same_agent_family_within_run,
            familyID: "orchestration_loop"
        )
        let writerLineageID = try await manager.getOrCreateLineage(
            runID: runID,
            agentID: "proposal_writer",
            scope: .same_invocation_owner,
            familyID: nil
        )

        let generationID = try await manager.createGeneration(
            lineageID: writerLineageID,
            invocationOwnerKey: "writer-owner",
            providerSessionID: "recycled-session",
            bindingFingerprint: "writer-fp",
            workingDirectory: "/tmp/writer",
            workspaceMode: "ro",
            runtimeProvider: "codex",
            runtimeModel: "gpt-5.4"
        )
        try await manager.closeGeneration(generationID: generationID, reason: "completed")

        let conflicts = try await manager.providerSessionConflicts(
            runID: runID,
            providerSessionID: "recycled-session",
            excludingLineageID: leadLineageID
        )

        #expect(conflicts.isEmpty)
    }

    @Test("AgentSessionGeneration preserves checkpoint traceability")
    func checkpointTraceability() async throws {
        let manager = AgentSessionManager(container: container)
        let runID = UUID()
        let checkpointID = UUID()
        
        let lid = try await manager.getOrCreateLineage(runID: runID, agentID: "a1", scope: .same_invocation_owner, familyID: nil)
        
        let gid = try await manager.createGeneration(
            lineageID: lid, 
            invocationOwnerKey: "o1", 
            providerSessionID: "s1", 
            bindingFingerprint: "f1", 
            workingDirectory: "/tmp", 
            workspaceMode: "ro", 
            runtimeProvider: "p1", 
            runtimeModel: "m1",
            rehydratedFromCheckpointArtifactID: checkpointID
        )
        
        let lineage = try await manager.getLineage(id: lid)
        let generation = lineage?.generations.first(where: { $0.id == gid })
        #expect(generation?.rehydratedFromCheckpointArtifactID == checkpointID)
    }

    @Test("AgentSessionCheckpointBuilder extracts blockers and questions")
    func checkpointExtraction() throws {
        let result = AgentResult.minimal(.completed, sessionID: "s1", duration: 1.0)
        let now = Date()
        let events: [ExecutionEvent] = [
            ExecutionEvent(type: .textChunk, timestamp: now, detail: "Some normal output."),
            ExecutionEvent(type: .textChunk, timestamp: now, detail: "BLOCKER: API key missing."),
            ExecutionEvent(type: .textChunk, timestamp: now, detail: "QUESTION: What is the target color?")
        ]

        let checkpoint = AgentSessionCheckpointBuilder.build(
            executionResult: result,
            eventLog: events,
            ownerKey: "o1",
            scope: .same_invocation_owner
        )

        #expect(checkpoint.unresolvedBlockers.count == 1)
        #expect(checkpoint.unresolvedBlockers.first?.contains("API key missing") == true)
        #expect(checkpoint.openQuestions.count == 1)
        #expect(checkpoint.openQuestions.first?.contains("target color") == true)

        let contextData = try #require(checkpoint.ownerAndBindingContextJSON)
        let context = try JSONDecoder().decode([String: String].self, from: contextData)
        #expect(context["ownerKey"] == "o1")

        // Verify scopeContextJSON is also populated
        #expect(checkpoint.scopeContextJSON != nil)
    }

    // MARK: - Recovery Branch Truth (ARCH-001)

    @Test("SessionReusePolicy fails closed on mismatched recovery branch ID")
    func recoveryBranchMismatch() throws {
        let runID = UUID()
        let branchA = UUID()
        let branchB = UUID()

        let ownerKeyA = "run:\(runID.uuidString):agent_1:stage_1:task:write:\(branchA.uuidString)"

        let lineage = AgentSessionLineage(runID: runID, agentID: "agent_1", lineageID: "lin_1", sessionReuseScope: .same_invocation_owner)
        context.insert(lineage)

        let generation = AgentSessionGeneration(
            generation: 1,
            invocationOwnerKey: ownerKeyA,
            providerSessionID: "sess_1",
            bindingFingerprint: "fp_1",
            workingDirectory: "/tmp",
            workspaceMode: "read_only",
            runtimeProvider: "p1",
            runtimeModel: "m1"
        )
        generation.lineage = lineage
        context.insert(generation)
        lineage.activeGenerationID = generation.id
        try context.save()

        // Same owner key, same branch: should reuse
        let decision1 = SessionReusePolicy.evaluate(
            lineage: lineage,
            currentInvocationOwnerKey: ownerKeyA,
            currentBindingFingerprint: "fp_1",
            currentRecoveryBranchID: branchA
        )
        if case .reuse = decision1 {} else {
            Issue.record("Expected reuse when recovery branch matches")
        }

        // Different owner key (different branch): should create fresh
        let ownerKeyB = "run:\(runID.uuidString):agent_1:stage_1:task:write:\(branchB.uuidString)"
        let decision2 = SessionReusePolicy.evaluate(
            lineage: lineage,
            currentInvocationOwnerKey: ownerKeyB,
            currentBindingFingerprint: "fp_1",
            currentRecoveryBranchID: branchB
        )
        if case .createFresh = decision2 {} else {
            Issue.record("Expected fresh session when recovery branch differs")
        }
    }

    // MARK: - Reset + Checkpoint Flow (REQ-005, REQ-010)

    @Test("Reset flow produces fresh_after_reset via SessionReusePolicy after real reset")
    func resetProducesFreshAfterReset() async throws {
        let manager = AgentSessionManager(container: container)
        let runID = UUID()

        let lid = try await manager.getOrCreateLineage(runID: runID, agentID: "a1", scope: .same_invocation_owner, familyID: nil)
        let gid = try await manager.createGeneration(
            lineageID: lid, invocationOwnerKey: "o1", providerSessionID: "s1",
            bindingFingerprint: "f1", workingDirectory: "/tmp", workspaceMode: "ro",
            runtimeProvider: "p1", runtimeModel: "m1"
        )
        try await manager.recordEvent(lineageID: lid, generationID: gid, type: .created)

        // Record checkpoint before reset (§6.4 rule 1)
        try await manager.recordCheckpointCreated(lineageID: lid, generationID: gid, checkpointData: nil)

        // Perform reset
        try await manager.resetSession(lineageID: lid, reason: "test operator reset")

        // After real reset, policy should produce fresh_after_reset
        let lineage = try await manager.getLineage(id: lid)
        let decision = SessionReusePolicy.evaluate(
            lineage: lineage,
            currentInvocationOwnerKey: "o1",
            currentBindingFingerprint: "f1",
            currentRecoveryBranchID: nil
        )
        if case .createFresh(let disposition, _) = decision {
            #expect(disposition == .fresh_after_reset, "Expected fresh_after_reset but got \(disposition)")
        } else {
            Issue.record("Expected createFresh after reset, got \(decision)")
        }

        // Verify checkpoint_created event was recorded
        let checkpointEvents = lineage?.events.filter { $0.eventType == .checkpoint_created }
        #expect(checkpointEvents?.isEmpty == false, "Checkpoint event should have been recorded before reset")

        // Verify operator_reset event was recorded with details
        let resetEvents = lineage?.events.filter { $0.eventType == .operator_reset }
        #expect(resetEvents?.isEmpty == false, "operator_reset event should have been recorded")
        if let detailsJSON = resetEvents?.first?.detailsJSON {
            let details = try JSONDecoder().decode([String: String].self, from: detailsJSON)
            #expect(details["reason"] == "test operator reset")
        }
    }

    @Test("AgentSessionCheckpointBuilder.buildForReset produces continuation-safe checkpoint")
    func checkpointBuilderForReset() throws {
        let runID = UUID()
        let lineage = AgentSessionLineage(runID: runID, agentID: "a1", lineageID: "lin_1", sessionReuseScope: .same_invocation_owner)
        context.insert(lineage)

        let generation = AgentSessionGeneration(
            generation: 2, invocationOwnerKey: "o1", providerSessionID: "s1",
            bindingFingerprint: "f1", workingDirectory: "/tmp/work",
            workspaceMode: "read_write", runtimeProvider: "claude", runtimeModel: "sonnet"
        )
        generation.turnCount = 5
        generation.cumulativePromptTokens = 50_000
        generation.cumulativeCostCents = 12
        generation.lineage = lineage
        context.insert(generation)
        try context.save()

        let checkpoint = AgentSessionCheckpointBuilder.buildForReset(
            generation: generation, lineage: lineage, resetReason: "manual reset"
        )

        #expect(checkpoint.machineSummary.contains("generation #2"))
        #expect(checkpoint.machineSummary.contains("Turns: 5"))
        #expect(checkpoint.machineSummary.contains("manual reset"))
        #expect(checkpoint.durableLearnings.contains(where: { $0.contains("claude/sonnet") }))
        #expect(checkpoint.nextSteps.contains(where: { $0.contains("Fresh session") }))

        // Verify owner context is populated
        let contextData = try #require(checkpoint.ownerAndBindingContextJSON)
        let ctx = try JSONDecoder().decode([String: String].self, from: contextData)
        #expect(ctx["ownerKey"] == "o1")
        #expect(ctx["scope"] == "same_invocation_owner")
    }

    // MARK: - KPI Export (REQ-013)

    @Test("SessionReuseKPIExporter generates per-agent and run-level KPIs")
    func kpiExporter() async throws {
        let manager = AgentSessionManager(container: container)
        let runID = UUID()

        // Create lineage with generations and events
        let lid = try await manager.getOrCreateLineage(runID: runID, agentID: "writer", scope: .same_agent_family_within_run, familyID: "fam")
        let gid1 = try await manager.createGeneration(
            lineageID: lid, invocationOwnerKey: "o1", providerSessionID: "s1",
            bindingFingerprint: "f1", workingDirectory: "/tmp", workspaceMode: "ro",
            runtimeProvider: "p1", runtimeModel: "m1"
        )
        try await manager.recordEvent(lineageID: lid, generationID: gid1, type: .created)
        try await manager.updateGenerationUsage(generationID: gid1, turnIncrement: 3, promptTokensIncrement: 30_000, costCentsIncrement: 5, estimatedInputTokens: 10_000)
        try await manager.recordEvent(lineageID: lid, generationID: gid1, type: .reused)
        try await manager.recordEvent(lineageID: lid, generationID: gid1, type: .reused)

        let summary = SessionReuseKPIExporter.exportKPIs(for: runID, context: context)
        #expect(summary.runID == runID)
        #expect(summary.totalExecutions > 0)
        #expect(summary.perAgentKPIs.count == 1)

        let agentKPI = try #require(summary.perAgentKPIs.first)
        #expect(agentKPI.agentID == "writer")
        #expect(agentKPI.reusedExecutions == 2)
        #expect(agentKPI.freshExecutions == 1)
        #expect(agentKPI.reusePercentage > 0)
        #expect(agentKPI.totalExecutions == 3)

        // Verify JSON export works
        let json = SessionReuseKPIExporter.exportJSON(for: runID, context: context)
        #expect(json != nil)
    }

    // MARK: - Report Bridge (REQ-006)

    @Test("SessionLineageReportBridge generates structured reports")
    func reportBridge() async throws {
        let manager = AgentSessionManager(container: container)
        let runID = UUID()

        let lid = try await manager.getOrCreateLineage(runID: runID, agentID: "agent_x", scope: .same_invocation_owner, familyID: nil)
        let gid = try await manager.createGeneration(
            lineageID: lid, invocationOwnerKey: "o1", providerSessionID: "s1",
            bindingFingerprint: "f1", workingDirectory: "/tmp", workspaceMode: "ro",
            runtimeProvider: "p1", runtimeModel: "m1"
        )
        try await manager.recordEvent(lineageID: lid, generationID: gid, type: .created)
        try await manager.recordEvent(lineageID: lid, generationID: gid, type: .reused)
        try await manager.updateGenerationUsage(generationID: gid, turnIncrement: 2, promptTokensIncrement: 20_000, costCentsIncrement: 3, estimatedInputTokens: 15_000)

        let reports = SessionLineageReportBridge.generateReports(for: runID, context: context)
        #expect(reports.count == 1)

        let report = try #require(reports.first)
        #expect(report.agentID == "agent_x")
        #expect(report.reuseScope == "same_invocation_owner")
        #expect(report.totalGenerations == 1)
        #expect(report.totalEvents == 2)
        #expect(report.totalTurns == 2)
        #expect(report.totalPromptTokens == 20_000)
        #expect(report.dispositionHistory.contains("created"))
        #expect(report.dispositionHistory.contains("reused"))

        // Verify JSON export
        let json = SessionLineageReportBridge.generateReportJSON(for: runID, context: context)
        #expect(json != nil)
    }

    @Test("SessionLineageReportBridge surfaces fetch failures in the envelope")
    func reportBridgeSurfacesFetchFailure() throws {
        enum TestError: LocalizedError {
            case fetchFailed

            var errorDescription: String? {
                "fixture fetch failed"
            }
        }

        let runID = UUID()
        let envelope = SessionLineageReportBridge.generateEnvelope(
            for: runID,
            fetchLineages: {
                throw TestError.fetchFailed
            }
        )

        #expect(envelope.reports.isEmpty)
        let errorMessage = try #require(envelope.errorMessage)
        #expect(errorMessage.contains("fixture fetch failed"))
    }

    // MARK: - Receipt Fields (REQ-006)

    @Test("SessionReuseReceiptFields builds from AgentExecution")
    func receiptFields() throws {
        let agentExec = AgentExecution(agentID: "a1", agentTitle: "Agent 1", taskName: "task1", provider: "p1", effort: "high")
        agentExec.sessionLineageID = UUID()
        agentExec.sessionGenerationID = UUID()
        agentExec.invocationOwnerKey = "owner_key_1"
        agentExec.sessionReuseScope = .same_invocation_owner
        agentExec.sessionReuseDisposition = .reused
        agentExec.sessionResetReason = nil

        let fields = SessionReuseReceiptFields.from(execution: agentExec)
        #expect(fields.sessionLineageID == agentExec.sessionLineageID)
        #expect(fields.sessionGenerationID == agentExec.sessionGenerationID)
        #expect(fields.invocationOwnerKey == "owner_key_1")
        #expect(fields.sessionReuseScope == "same_invocation_owner")
        #expect(fields.sessionReuseDisposition == "reused")
        #expect(fields.sessionResetReason == nil)
    }

    // MARK: - Audit Trail (REQ-006)

    @Test("SessionResetAuditTrail captures reset history with checkpoint truth")
    func resetAuditTrail() async throws {
        let manager = AgentSessionManager(container: container)
        let runID = UUID()

        let lid = try await manager.getOrCreateLineage(runID: runID, agentID: "a1", scope: .same_invocation_owner, familyID: nil)
        let gid = try await manager.createGeneration(
            lineageID: lid, invocationOwnerKey: "o1", providerSessionID: "s1",
            bindingFingerprint: "f1", workingDirectory: "/tmp", workspaceMode: "ro",
            runtimeProvider: "p1", runtimeModel: "m1"
        )
        try await manager.updateGenerationUsage(generationID: gid, turnIncrement: 5, promptTokensIncrement: 50_000, costCentsIncrement: 10, estimatedInputTokens: 40_000)

        // Emit checkpoint then reset
        try await manager.recordCheckpointCreated(lineageID: lid, generationID: gid, checkpointData: nil)
        try await manager.resetSession(lineageID: lid, reason: "operator test reset")

        let history = SessionResetAuditTrail.fetchResetHistory(for: runID, context: context)
        #expect(history.count == 1)

        let entry = try #require(history.first)
        #expect(entry.agentID == "a1")
        #expect(entry.runID == runID)
        #expect(entry.resetReason == "operator test reset")
        #expect(entry.priorTurnCount == 5)
        #expect(entry.priorCumulativeTokens == 50_000)
        #expect(entry.checkpointEmitted == true)

        // Verify JSON export
        let json = SessionResetAuditTrail.exportJSON(for: runID, context: context)
        #expect(json != nil)
    }
}
