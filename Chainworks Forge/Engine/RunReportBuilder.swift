import Foundation
import SwiftData

// MARK: - P005-OPS §6: Run Report Builder

/// Deterministic immutable reports plus latest summary view.
/// Every stable checkpoint emits immutable `run_report_v{n}` artifacts.
/// Latest summary artifacts exist separately from immutable history.
@MainActor
final class RunReportBuilder {
    static let immutableHistoryRetentionVersions = 10

    private let modelContext: ModelContext

    init(modelContext: ModelContext) {
        self.modelContext = modelContext
    }

    enum SummarySnapshotMode {
        case checkpoint
        case live(status: RunStatus)

        var effectiveStatus: RunStatus? {
            switch self {
            case .checkpoint:
                return nil
            case .live(let status):
                return status
            }
        }

        var suppressRecoveryNarrative: Bool {
            switch self {
            case .checkpoint:
                return false
            case .live(let status):
                switch status {
                case .pending, .ready, .running:
                    return true
                case .waitingApproval, .blocked, .completed, .failed, .cancelled, .cancelling:
                    return false
                }
            }
        }

        var completedAtOverride: Date?? {
            switch self {
            case .checkpoint:
                return nil
            case .live(let status):
                switch status {
                case .pending, .ready, .running, .waitingApproval, .blocked, .cancelling:
                    return .some(nil)
                case .completed, .failed, .cancelled:
                    return nil
                }
            }
        }
    }

    // MARK: - Report Generation (§6.3)

    /// Emit a new immutable report version when the run reaches a stable checkpoint.
    /// A new version is created on: terminal state, explicit recovery action, or approval re-arm.
    func emitReport(for run: Run) throws -> (markdownArtifact: Artifact, jsonArtifact: Artifact) {
        let nextVersion = run.latestReportVersion + 1
        let report = buildReportPayload(for: run, version: nextVersion)

        // Immutable markdown report
        let mdContent = renderMarkdown(from: report)
        let mdArtifact = Artifact(
            name: "run_report_v\(nextVersion).md",
            contractID: "run_report",
            format: .markdown,
            filePath: reportFilePath(run: run, name: "run_report_v\(nextVersion).md"),
            runID: run.id,
            stageID: cursorDerivedReportStageID(for: run),
            agentID: "system",
            provider: "system"
        )
        mdArtifact.reportKind = "immutable_history"
        mdArtifact.reportVersion = nextVersion
        if let previousID = run.latestImmutableReportArtifactID {
            mdArtifact.supersedesArtifactID = previousID
        }

        // Immutable JSON report
        let jsonContent = renderJSON(from: report)
        let jsonArtifact = Artifact(
            name: "run_report_v\(nextVersion).json",
            contractID: "run_report",
            format: .json,
            filePath: reportFilePath(run: run, name: "run_report_v\(nextVersion).json"),
            runID: run.id,
            stageID: cursorDerivedReportStageID(for: run),
            agentID: "system",
            provider: "system"
        )
        jsonArtifact.reportKind = "immutable_history"
        jsonArtifact.reportVersion = nextVersion

        // Write to disk
        try writeReportToDisk(content: mdContent, path: mdArtifact.filePath)
        try writeReportToDisk(content: jsonContent, path: jsonArtifact.filePath)

        // Persist metadata
        modelContext.insert(mdArtifact)
        modelContext.insert(jsonArtifact)

        // Update run pointers
        run.latestReportVersion = nextVersion
        run.latestImmutableReportArtifactID = mdArtifact.id

        // Emit latest summary
        try emitLatestSummary(for: run, basedOn: report)
        try pruneImmutableHistory(for: run, keepingLatestVersions: Self.immutableHistoryRetentionVersions)

        try modelContext.save()

        return (mdArtifact, jsonArtifact)
    }

    /// Emit or overwrite the mutable latest summary artifacts.
    func emitLatestSummary(for run: Run, basedOn report: RunReportPayload) throws {
        let mdContent = renderLatestSummaryMarkdown(from: report)
        let jsonContent = renderLatestSummaryJSON(from: report)

        let mdPath = reportFilePath(run: run, name: "run_summary_latest.md")
        let jsonPath = reportFilePath(run: run, name: "run_summary_latest.json")

        try writeReportToDisk(content: mdContent, path: mdPath)
        try writeReportToDisk(content: jsonContent, path: jsonPath)

        // If latest summary artifacts already exist, update them; otherwise create new
        if let existingID = run.latestSummaryArtifactID {
            let descriptor = FetchDescriptor<Artifact>(predicate: #Predicate { $0.id == existingID })
            if let existing = try? modelContext.fetch(descriptor).first {
                existing.reportKind = "latest_summary"
                existing.reportVersion = report.version
            }
        } else {
            let summaryArtifact = Artifact(
                name: "run_summary_latest.md",
                contractID: "run_summary",
                format: .markdown,
                filePath: mdPath,
                runID: run.id,
                stageID: cursorDerivedReportStageID(for: run),
                agentID: "system",
                provider: "system"
            )
            summaryArtifact.reportKind = "latest_summary"
            summaryArtifact.reportVersion = report.version
            modelContext.insert(summaryArtifact)
            run.latestSummaryArtifactID = summaryArtifact.id
        }
    }

    func emitLatestSummarySnapshot(for run: Run) throws {
        let version = max(run.latestReportVersion, 1)
        let mode = SummarySnapshotMode.live(status: run.status)
        let report = buildReportPayload(for: run, version: version, summarySnapshotMode: mode)
        try emitLatestSummary(for: run, basedOn: report)
        try modelContext.save()
    }

    func pruneImmutableHistoryForAllRuns() throws {
        let descriptor = FetchDescriptor<Run>()
        let runs = try modelContext.fetch(descriptor)
        for run in runs where run.latestReportVersion > Self.immutableHistoryRetentionVersions {
            try pruneImmutableHistory(for: run, keepingLatestVersions: Self.immutableHistoryRetentionVersions)
        }
        try modelContext.save()
    }

    // MARK: - Proposal 032: Cursor-Derived Report Stage ID

    /// Derive the stageID for report artifact metadata from the durable cursor.
    /// Falls back to `run.currentStageID` for pre-P032 runs without a cursor.
    private func cursorDerivedReportStageID(for run: Run) -> String {
        if let cursor = run.transitionCursor {
            switch cursor.settlementPhase {
            case .transitionSettled:
                return cursor.nextScheduledStateID ?? cursor.lastCompletedStateID ?? "unknown"
            case .transitionStarted:
                return cursor.nextScheduledStateID ?? "unknown"
            case .terminal:
                return cursor.lastCompletedStateID ?? "unknown"
            case .awaitingFirstState:
                break
            }
        }
        return run.currentStageID ?? "unknown"
    }

    // MARK: - Should Emit Check

    /// Determines if a report should be emitted based on current run state (§6.3).
    func shouldEmitReport(for run: Run) -> Bool {
        switch run.presentationStatus {
        case .completed, .failed, .cancelled, .blocked, .waitingApproval:
            return true
        default:
            return false
        }
    }

    // MARK: - Report Payload Construction (§6.4)

    func buildReportPayload(
        for run: Run,
        version: Int,
        summarySnapshotMode: SummarySnapshotMode = .checkpoint
    ) -> RunReportPayload {
        let historicalStages = PersistedRunGraph.stageExecutions(for: run).sorted { $0.startedAt < $1.startedAt }
        let canonicalStages = canonicalStages(from: historicalStages)
        let timelineStages = stageTimelineStages(from: canonicalStages)
        let allAgents = canonicalStages.flatMap { canonicalAgents(for: $0) }
        let approvals = PersistedRunGraph.approvals(for: run).sorted { $0.requestedAt < $1.requestedAt }
        let runStatus = summarySnapshotMode.effectiveStatus ?? run.presentationStatus

        // Stage timeline (canonical lineage only)
        let stageTimeline: [RunReportPayload.StageEntry] = timelineStages.map { stage in
            RunReportPayload.StageEntry(
                label: stage.label,
                status: stage.status.rawValue,
                iteration: stage.iteration,
                attempt: stage.attemptNumber,
                duration: stageDuration(stage)
            )
        }

        // Proposal 011 (REQ-008): Read model truth from frozen binding snapshot first.
        let frozenBindings = decodeFrozenBindings(from: run)
        let frozenProvenances = decodeFrozenProvenances(from: run)
        let resolvedSkills = decodeResolvedSkills(from: run)
        let catalogSkillRefs = decodeCatalogSkillRefs(from: run)
        let frozenMCPPolicies = decodeFrozenMCPPolicies(from: run)
        let kpiSummary = SessionReuseKPIExporter.decodeSummary(from: run.sessionKPIExportJSON)

        // Agents used
        let agentsUsed: [RunReportPayload.AgentEntry] = allAgents.map { agent in
            let frozenModel = frozenBindings[agent.agentID]?.model
            let provenanceLabel = frozenProvenances[agent.agentID].map { " (\($0.source.rawValue))" } ?? ""
            let skillRef = agent.skillRef ?? catalogSkillRefs[agent.agentID]
            let resolvedSkill = skillRef.flatMap { resolvedSkills[$0] }
            let mcpResolution = frozenMCPPolicies[agent.agentID]
            return RunReportPayload.AgentEntry(
                agentID: agent.agentID,
                provider: agent.provider,
                model: (frozenModel ?? agent.resolvedModel ?? agent.resolvedBackendProfileID ?? "unknown") + provenanceLabel,
                effort: agent.effort,
                costCents: agent.costCents,
                duration: agentDuration(agent),
                finalStatus: agent.status.rawValue,
                skillRef: skillRef,
                skillType: agent.skillType,
                skillRole: agent.skillRole,
                skillContentSummary: agent.skillContentSummary,
                skillSnapshotHash: agent.skillSnapshotHash,
                resolvedSkillContent: resolvedSkill?.resolvedContent,
                mcpProfileID: agent.mcpProfileID,
                requestedMCPExtensions: decodeStringArray(from: agent.requestedMCPExtensionsJSON),
                predictedMCPExtensions: mcpResolution?.predictedEffectiveRuntimeExtensionIDs ?? [],
                actualMCPExtensions: decodeStringArray(from: agent.effectiveMCPRuntimeExtensionIDsJSON),
                deniedMCPExtensions: decodeStringArray(from: agent.deniedMCPExtensionsJSON),
                runtimeProfileID: agent.runtimeProfileID,
                actualAdapterFamily: agent.actualAdapterFamily,
                actualCapabilityClass: agent.actualCapabilityClass
            )
        }

        // Approval entries
        let approvalEntries: [RunReportPayload.ApprovalEntry] = approvals.map { approval in
            RunReportPayload.ApprovalEntry(
                gateLabel: approval.stageID,
                decision: approval.decision.rawValue,
                comment: approval.comment,
                requestedAt: approval.requestedAt,
                decidedAt: approval.decidedAt
            )
        }

        // Execution summary
        let completedStages = canonicalStages.filter { $0.status == .completed }.count
        let skippedStages = canonicalStages.filter { $0.status == .skipped }.count
        let failedStages = canonicalStages.filter { $0.status == .failed }.count
        let loopsEntered = run.loopCounters.values.reduce(0, +)
        let approvalsRequested = approvals.count
        let approvalsGranted = approvals.filter { $0.decision == .granted }.count
        let approvalsRejected = approvals.filter { $0.decision == .rejected }.count

        // Key artifacts (canonical lineage only; pinned first, then report-worthy)
        let allArtifacts = allAgents
            .flatMap(\.artifacts)
            .sorted { $0.createdAt < $1.createdAt }
        let pinnedArtifacts = allArtifacts.filter { $0.isPinned }
        let reportArtifacts = allArtifacts.filter { !$0.isPinned && $0.reportKind == nil }

        let keyArtifactEntries: [RunReportPayload.ArtifactEntry] = (pinnedArtifacts + reportArtifacts).map { artifact in
            RunReportPayload.ArtifactEntry(
                name: artifact.name,
                format: artifact.format.rawValue,
                isPinned: artifact.isPinned,
                agentID: artifact.agentID,
                stageID: artifact.stageID
            )
        }

        let proposalLoopSummary = ProposalLoopFeedbackParser.parseSummary(from: allArtifacts)

        // §6.5: Retry/recovery narrative
        let stageRetriesPerformed = historicalStages.reduce(0) { $0 + max(0, $1.attemptNumber - 1) }
        let agentRetriesPerformed = historicalStages
            .flatMap(\.agentExecutions)
            .filter { $0.retryReason != nil }
            .count
        let retriesPerformed = stageRetriesPerformed + agentRetriesPerformed
        let recoveryActionsTaken: [String] = historicalStages
            .flatMap(\.agentExecutions)
            .compactMap { $0.retryReason }

        let interruptedContinuationStage = interruptedContinuationStage(for: run, historicalStages: historicalStages)
        let exhaustedWatchdogStage = exhaustedAutomaticWatchdogFailureStage(from: historicalStages)

        let unresolvedFailureStages = unresolvedFailureStages(
            from: canonicalStages,
            run: run,
            interruptedContinuationStage: interruptedContinuationStage
        )

        let failureEvidenceSummaries: [RunReportPayload.FailureEvidenceSummary] = unresolvedFailureStages
            .compactMap { stage in
            guard let packet = failureEvidencePacket(for: stage, run: run) else { return nil }
            return RunReportPayload.FailureEvidenceSummary(
                stageID: packet.stageID,
                stageLabel: packet.stageLabel,
                failureClass: packet.supervisionClassification?.rawValue ?? packet.failureClass.rawValue,
                failureSummary: packet.failureSummary,
                rawOutputsExist: packet.rawOutputsExist,
                receiptExists: packet.receiptExists,
                transcriptExists: packet.transcriptExists
            )
        }

        let currentRecoveryStage = exhaustedWatchdogStage
            ?? interruptedContinuationStage
            ?? unresolvedFailureStages.last
        let currentRecoverySnapshot = currentRecoveryStage.flatMap { recoverySnapshot(for: run, stage: $0) }
        let usesInterruptedContinuationTruth = exhaustedWatchdogStage == nil
            && interruptedContinuationStage?.id == currentRecoveryStage?.id
        let blockedReason = summarySnapshotMode.suppressRecoveryNarrative
            ? nil
            : currentRecoveryStage.flatMap { failureEvidencePacket(for: $0, run: run)?.failureSummary } ?? run.driftDetails
        let driftNote = summarySnapshotMode.suppressRecoveryNarrative ? nil : reportDriftNote(
            run: run,
            currentRecoveryStage: currentRecoveryStage,
            interruptedContinuationStage: usesInterruptedContinuationTruth ? interruptedContinuationStage : nil
        )
        let retryPath = summarySnapshotMode.suppressRecoveryNarrative
            ? nil
            : (usesInterruptedContinuationTruth ? nil : retryPathDescription(from: currentRecoverySnapshot))
        let resumePath = summarySnapshotMode.suppressRecoveryNarrative ? nil : resumePathDescription(
            run: run,
            waitingApprovalStage: historicalStages.last(where: { $0.status == .waitingApproval }),
            snapshot: currentRecoverySnapshot,
            interruptedContinuationStage: usesInterruptedContinuationTruth ? interruptedContinuationStage : nil
        )
        let contextStrategyProfileID = strategyProfileID(for: run)
        let strategyAssignmentMode = strategyAssignmentMode(for: run)
        let strategyRecommendationState = strategyRecommendationState(for: run)
        let strategyTelemetryComplete = hasCanonicalStrategyTelemetry(for: run)

        // Proposal 032: Read durable transition cursor for report truth.
        let cursor = run.transitionCursor

        return RunReportPayload(
            ideaTitle: run.idea?.title ?? "Unknown",
            workflowTitle: run.workflowTitle,
            runID: run.id,
            runStatus: runStatus.rawValue,
            version: version,
            startedAt: run.startedAt,
            completedAt: summarySnapshotMode.completedAtOverride ?? run.completedAt,
            elapsedSeconds: elapsedTime(for: run),
            totalCostCents: run.totalCostCents,
            workflowSnapshotHash: run.workflowSnapshotHash,
            catalogSnapshotHash: run.catalogSnapshotHash,
            runtimeTrustLevel: run.normalizedRuntimeTrustLevel ?? "unknown",
            driftNote: driftNote,
            completedStages: completedStages,
            skippedStages: skippedStages,
            failedStages: failedStages,
            loopsEntered: loopsEntered,
            approvalsRequested: approvalsRequested,
            approvalsGranted: approvalsGranted,
            approvalsRejected: approvalsRejected,
            stageTimeline: stageTimeline,
            agentsUsed: agentsUsed,
            approvalEntries: approvalEntries,
            keyArtifacts: keyArtifactEntries,
            contextStrategyProfileID: contextStrategyProfileID,
            strategyAssignmentMode: strategyAssignmentMode,
            strategyRecommendationState: strategyRecommendationState,
            strategyTelemetryComplete: strategyTelemetryComplete,
            blockedReason: blockedReason,
            retryPath: retryPath,
            resumePath: resumePath,
            driftDecision: run.driftDecision?.rawValue,
            retriesPerformed: retriesPerformed,
            recoveryActionsTaken: recoveryActionsTaken,
            failureEvidenceSummaries: failureEvidenceSummaries,
            proposalLoopSummary: proposalLoopSummary,
            mcpTelemetry: kpiSummary?.mcpTelemetry,
            transitionCursorLastCompletedStateID: cursor?.lastCompletedStateID,
            transitionCursorNextScheduledStateID: cursor?.nextScheduledStateID,
            transitionCursorSettlementPhase: cursor?.settlementPhase.rawValue,
            transitionCursorNextStarted: cursor?.settlementPhase == .transitionStarted
        )
    }

    private func canonicalStages(from historicalStages: [StageExecution]) -> [StageExecution] {
        let grouped = Dictionary(grouping: historicalStages) { stage in
            "\(stage.stageID)::\(stage.iteration)"
        }

        return grouped.values.compactMap { stages in
            stages.max { lhs, rhs in
                if lhs.attemptNumber != rhs.attemptNumber {
                    return lhs.attemptNumber < rhs.attemptNumber
                }
                if lhs.startedAt != rhs.startedAt {
                    return lhs.startedAt < rhs.startedAt
                }
                return lhs.id.uuidString < rhs.id.uuidString
            }
        }
        .sorted { $0.startedAt < $1.startedAt }
    }

    private func unresolvedFailureStages(
        from canonicalStages: [StageExecution],
        run: Run,
        interruptedContinuationStage: StageExecution?
    ) -> [StageExecution] {
        let latestByStateID = Dictionary(grouping: canonicalStages, by: \.stageID)
            .compactMapValues { stages in
                stages.max { lhs, rhs in
                    if lhs.startedAt != rhs.startedAt {
                        return lhs.startedAt < rhs.startedAt
                    }
                    if lhs.iteration != rhs.iteration {
                        return lhs.iteration < rhs.iteration
                    }
                    if lhs.attemptNumber != rhs.attemptNumber {
                        return lhs.attemptNumber < rhs.attemptNumber
                    }
                    return lhs.id.uuidString < rhs.id.uuidString
                }
            }

        var unresolved = latestByStateID.values.filter { stage in
            stage.status == .failed || stage.status == .blocked || stage.status == .waitingApproval
        }

        if let interruptedContinuationStage,
           !unresolved.contains(where: { $0.id == interruptedContinuationStage.id }),
           failureEvidencePacket(for: interruptedContinuationStage, run: run) != nil {
            unresolved.append(interruptedContinuationStage)
        }

        return unresolved.sorted { lhs, rhs in
            if lhs.startedAt != rhs.startedAt {
                return lhs.startedAt < rhs.startedAt
            }
            if lhs.iteration != rhs.iteration {
                return lhs.iteration < rhs.iteration
            }
            if lhs.attemptNumber != rhs.attemptNumber {
                return lhs.attemptNumber < rhs.attemptNumber
            }
            return lhs.id.uuidString < rhs.id.uuidString
        }
    }

    private func exhaustedAutomaticWatchdogFailureStage(from stages: [StageExecution]) -> StageExecution? {
        stages
            .filter { $0.status == .failed || $0.status == .blocked }
            .sorted { lhs, rhs in
                if lhs.startedAt != rhs.startedAt {
                    return lhs.startedAt < rhs.startedAt
                }
                if lhs.iteration != rhs.iteration {
                    return lhs.iteration < rhs.iteration
                }
                if lhs.attemptNumber != rhs.attemptNumber {
                    return lhs.attemptNumber < rhs.attemptNumber
                }
                return lhs.id.uuidString < rhs.id.uuidString
            }
            .last { stage in
                stage.agentExecutions.contains { agent in
                    agent.status == .failed
                        && agent.supervisionClassification != nil
                        && agent.retryReason == "automatic_watchdog_retry"
                }
            }
    }

    private func stageTimelineStages(from canonicalStages: [StageExecution]) -> [StageExecution] {
        let latestByStateID = Dictionary(grouping: canonicalStages, by: \.stageID)
            .compactMapValues { stages in
                stages.max { lhs, rhs in
                    if lhs.startedAt != rhs.startedAt {
                        return lhs.startedAt < rhs.startedAt
                    }
                    if lhs.iteration != rhs.iteration {
                        return lhs.iteration < rhs.iteration
                    }
                    if lhs.attemptNumber != rhs.attemptNumber {
                        return lhs.attemptNumber < rhs.attemptNumber
                    }
                    return lhs.id.uuidString < rhs.id.uuidString
                }
            }

        return canonicalStages.filter { stage in
            guard let latest = latestByStateID[stage.stageID] else {
                return true
            }

            switch stage.status {
            case .pending, .ready, .running, .waitingApproval:
                return latest.id == stage.id
            case .completed, .skipped, .failed, .blocked:
                return true
            }
        }
    }

    private func reportDriftNote(
        run: Run,
        currentRecoveryStage: StageExecution?,
        interruptedContinuationStage: StageExecution?
    ) -> String? {
        let trimmed = run.driftDetails?.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let driftNote = trimmed, !driftNote.isEmpty else {
            return nil
        }

        if interruptedContinuationStage != nil {
            return driftNote
        }

        guard currentRecoveryStage != nil else {
            return driftNote
        }

        let lowered = driftNote.lowercased()
        if lowered.contains("app restart")
            || lowered.contains("resume interrupted")
            || lowered.contains("interrupted") {
            return nil
        }
        return driftNote
    }

    private func canonicalAgents(for stage: StageExecution) -> [AgentExecution] {
        let grouped = Dictionary(grouping: stage.agentExecutions, by: \.agentID)

        return grouped.values.compactMap { agents in
            agents.max { lhs, rhs in
                let lhsAttempt = lhs.agentAttemptNumber ?? 1
                let rhsAttempt = rhs.agentAttemptNumber ?? 1
                if lhsAttempt != rhsAttempt {
                    return lhsAttempt < rhsAttempt
                }
                if lhs.startedAt != rhs.startedAt {
                    return lhs.startedAt < rhs.startedAt
                }
                return lhs.id.uuidString < rhs.id.uuidString
            }
        }
        .sorted { $0.startedAt < $1.startedAt }
    }

    private func failureEvidencePacket(for stage: StageExecution, run: Run) -> FailedStageEvidencePacket? {
        if let packetData = stage.evidencePacketJSON,
           let packet = try? JSONDecoder().decode(FailedStageEvidencePacket.self, from: packetData) {
            return packet
        }

        let validationFailure = validationFailureRecord(for: stage)
        let outputEnvelopes = stage.agentExecutions.flatMap { decodeOutputEnvelopes(from: $0) }
        let recoverySnapshot = recoverySnapshot(for: run, stage: stage)
        let failedAgent = failedAgent(for: stage)

        let hasFailureTruth = stage.status == .failed
            || stage.status == .blocked
            || validationFailure != nil
            || !outputEnvelopes.isEmpty
            || failedAgent != nil
        guard hasFailureTruth else { return nil }

        return FailedStageEvidenceBuilder.buildEvidencePacket(
            stageExecution: stage,
            failedAgent: failedAgent,
            validationFailure: validationFailure,
            outputEnvelopes: outputEnvelopes,
            recoverySnapshot: recoverySnapshot
        )
    }

    private func interruptedContinuationStage(for run: Run, historicalStages: [StageExecution]) -> StageExecution? {
        guard run.presentationStatus == .blocked || run.presentationStatus == .failed else { return nil }
        guard let cursor = run.transitionCursor,
              cursor.settlementPhase == .transitionStarted || cursor.settlementPhase == .transitionSettled,
              let nextStateID = cursor.nextScheduledStateID else { return nil }

        let matchingStages = historicalStages
            .filter { $0.stageID == nextStateID }
            .sorted { $0.startedAt < $1.startedAt }

        return matchingStages.last(where: { stage in
            switch stage.status {
            case .pending, .ready, .running, .waitingApproval, .blocked, .failed:
                return true
            case .completed, .skipped:
                return failureEvidencePacket(for: stage, run: run) != nil
            }
        })
    }

    private func recoverySnapshot(for run: Run, stage: StageExecution) -> RecoveryActionSnapshot? {
        if let data = stage.recoverySnapshotJSON,
           let snapshot = try? JSONDecoder().decode(RecoveryActionSnapshot.self, from: data) {
            return snapshot
        }

        if let packetData = stage.evidencePacketJSON,
           let packet = try? JSONDecoder().decode(FailedStageEvidencePacket.self, from: packetData),
           let snapshot = packet.recoverySnapshot {
            return snapshot
        }

        guard stage.status == .failed || stage.status == .blocked else { return nil }
        let retryCoordinator = StageRetryCoordinator(modelContext: modelContext)
        return retryCoordinator.narrowestRecoveryAction(
            for: run,
            failedStage: stage,
            failedAgent: failedAgent(for: stage),
            validationFailure: validationFailureRecord(for: stage)
        )
    }

    private func validationFailureRecord(for stage: StageExecution) -> ValidationFailureRecord? {
        if let data = stage.validationFailureJSON,
           let record = try? JSONDecoder().decode(ValidationFailureRecord.self, from: data) {
            return record
        }

        for agent in stage.agentExecutions.sorted(by: { $0.startedAt < $1.startedAt }).reversed() {
            if let data = agent.validationFailureJSON,
               let record = try? JSONDecoder().decode(ValidationFailureRecord.self, from: data) {
                return record
            }
        }

        return nil
    }

    private func decodeOutputEnvelopes(from agent: AgentExecution) -> [StructuredOutputEnvelope] {
        guard let data = agent.outputEnvelopesJSON else { return [] }
        return (try? JSONDecoder().decode([StructuredOutputEnvelope].self, from: data)) ?? []
    }

    private func failedAgent(for stage: StageExecution) -> AgentExecution? {
        stage.agentExecutions
            .filter { $0.status == .failed }
            .sorted { lhs, rhs in
                if lhs.startedAt != rhs.startedAt {
                    return lhs.startedAt < rhs.startedAt
                }
                let lhsAttempt = lhs.agentAttemptNumber ?? 1
                let rhsAttempt = rhs.agentAttemptNumber ?? 1
                return lhsAttempt < rhsAttempt
            }
            .last
    }

    private func retryPathDescription(from snapshot: RecoveryActionSnapshot?) -> String? {
        guard let snapshot else { return nil }
        let retryAction = snapshot.availableActions.first {
            $0.action == .retryFailedAgent
            || $0.action == .retryFailedAggregateStep
            || $0.action == .retryFailedStage
        } ?? snapshot.recommendedAction

        guard let action = retryAction else { return nil }
        switch action.action {
        case .retryFailedAgent:
            guard let agentID = action.agentID, let stageID = action.stageID else { return nil }
            return "Retry agent '\(agentID)' in stage '\(stageID)'"
        case .retryFailedAggregateStep:
            guard let agentID = action.agentID, let stageID = action.stageID else { return nil }
            return "Retry aggregate step '\(agentID)' in stage '\(stageID)'"
        case .retryFailedStage:
            guard let stageID = action.stageID else { return nil }
            return "Retry stage '\(stageID)'"
        default:
            return nil
        }
    }

    private func resumePathDescription(
        run: Run,
        waitingApprovalStage: StageExecution?,
        snapshot: RecoveryActionSnapshot?,
        interruptedContinuationStage: StageExecution?
    ) -> String? {
        let presentationStatus = run.presentationStatus

        if presentationStatus == .waitingApproval, let gateStage = waitingApprovalStage {
            return "Resume from approval gate '\(gateStage.label)'"
        }

        if interruptedContinuationStage != nil {
            return "Use Resume Interrupted to continue from the transition cursor continuation state; clone run is fallback only."
        }

        guard presentationStatus == .failed || presentationStatus == .blocked else { return nil }
        guard let snapshot else {
            return "Clone run (frozen snapshot or current config)"
        }

        if let recommended = snapshot.recommendedAction, recommended.action == .operatorInspection {
            return "Inspect failure evidence first; same-run retry remains available before clone-run."
        }

        let hasSameRunRecovery = snapshot.availableActions.contains { detail in
            detail.action == .retryFailedAgent
            || detail.action == .retryFailedAggregateStep
            || detail.action == .retryFailedStage
        }
        if hasSameRunRecovery {
            return "Use same-run recovery from the canonical recovery snapshot; clone run is fallback only."
        }

        return "Clone run (frozen snapshot or current config)"
    }

    // MARK: - Rendering

    private func renderMarkdown(from payload: RunReportPayload) -> String {
        var lines: [String] = []
        lines.append("# Run Report v\(payload.version)")
        lines.append("")
        lines.append("## 1. Header")
        lines.append("- **Idea:** \(payload.ideaTitle)")
        lines.append("- **Workflow:** \(payload.workflowTitle)")
        lines.append("- **Run ID:** \(payload.runID)")
        lines.append("- **Status:** \(payload.runStatus)")
        lines.append("- **Report Version:** \(payload.version)")
        lines.append("- **Started:** \(payload.startedAt)")
        if let completed = payload.completedAt {
            lines.append("- **Completed:** \(completed)")
        }
        lines.append("- **Elapsed:** \(formattedDuration(payload.elapsedSeconds))")
        if let cost = payload.totalCostCents {
            lines.append("- **Total Cost:** \(cost) cents")
        }
        lines.append("")
        lines.append("## 2. Snapshot and Runtime Provenance")
        lines.append("- **Workflow Hash:** \(payload.workflowSnapshotHash)")
        lines.append("- **Catalog Hash:** \(payload.catalogSnapshotHash)")
        lines.append("- **Runtime Trust:** \(payload.runtimeTrustLevel)")
        if let drift = payload.driftNote {
            lines.append("- **Drift Note:** \(drift)")
        }
        lines.append("")
        lines.append("## 3. Execution Summary")
        lines.append("- Stages completed: \(payload.completedStages)")
        lines.append("- Stages skipped: \(payload.skippedStages)")
        lines.append("- Stages failed: \(payload.failedStages)")
        lines.append("- Loops entered: \(payload.loopsEntered)")
        lines.append("- Approvals requested: \(payload.approvalsRequested)")
        lines.append("- Approvals granted: \(payload.approvalsGranted)")
        lines.append("- Approvals rejected: \(payload.approvalsRejected)")
        lines.append("")
        lines.append("## 4. Stage Timeline")
        for stage in payload.stageTimeline {
            lines.append("- **\(stage.label)** — \(stage.status) (iter \(stage.iteration), attempt \(stage.attempt), \(formattedDuration(stage.duration)))")
        }
        lines.append("")
        if let mcpTelemetry = payload.mcpTelemetry {
            lines.append("## 5A. MCP Telemetry")
            lines.append("- Executions with backend MCP requirements: \(mcpTelemetry.totalExecutionsWithMCPProfile)")
            lines.append("- Zero-MCP executions: \(mcpTelemetry.totalZeroMCPExecutions)")
            lines.append("- Requested runtime extensions: \(mcpTelemetry.totalRequestedExtensionCount)")
            lines.append("- Predicted runtime extensions: \(mcpTelemetry.totalPredictedExtensionCount)")
            lines.append("- Actual runtime extensions: \(mcpTelemetry.totalActualExtensionCount)")
            lines.append("- Denied runtime extensions: \(mcpTelemetry.totalDeniedExtensionCount)")
            lines.append("- Policy reduction executions: \(mcpTelemetry.totalPolicyReductionExecutions)")
            lines.append("- Prediction drift executions: \(mcpTelemetry.totalPredictionDriftExecutions)")
            lines.append("- Avg requested per execution: \(String(format: "%.2f", mcpTelemetry.averageRequestedExtensionsPerExecution))")
            lines.append("- Avg actual per execution: \(String(format: "%.2f", mcpTelemetry.averageActualExtensionsPerExecution))")
            lines.append("- Total startup latency (ms): \(mcpTelemetry.totalStartupLatencyMilliseconds)")
            lines.append("- Avg startup latency (ms): \(String(format: "%.2f", mcpTelemetry.averageStartupLatencyMilliseconds))")
            lines.append("- Prompt/context delta from MCP (bytes): \(mcpTelemetry.totalPromptContextDeltaBytes)")
            lines.append("- MCP-preflight blocked runs: \(mcpTelemetry.totalMCPPreflightBlockedRuns)")
            if !mcpTelemetry.startupLatencyByExtensionSet.isEmpty {
                lines.append("- Startup latency by extension set:")
                for bucket in mcpTelemetry.startupLatencyByExtensionSet {
                    lines.append("  - \(bucket.extensionSet): count=\(bucket.executionCount), total_ms=\(bucket.totalStartupLatencyMilliseconds), avg_ms=\(String(format: "%.2f", bucket.averageStartupLatencyMilliseconds))")
                }
            }
            if !mcpTelemetry.serverUsage.isEmpty {
                lines.append("- MCP server usage:")
                for usage in mcpTelemetry.serverUsage {
                    lines.append("  - \(usage.serverID): calls=\(usage.toolCallCount), request_bytes=\(usage.requestBytes), response_bytes=\(usage.responseBytes), prompt_delta_bytes=\(usage.promptContextDeltaBytes)")
                }
            }
            lines.append("")
        }
        lines.append("## 5. Agents Used")
        for agent in payload.agentsUsed {
            var line = "- **\(agent.agentID)** — \(agent.provider)"
            if let model = agent.model { line += " / \(model)" }
            line += " / \(agent.effort)"
            if let cost = agent.costCents { line += " (\(cost)c)" }
            line += " — \(agent.finalStatus)"
            lines.append(line)
            if let skillRef = agent.skillRef {
                var skillLine = "  - Skill: \(skillRef)"
                if let skillType = agent.skillType { skillLine += " [\(skillType)]" }
                if let skillRole = agent.skillRole { skillLine += " role=\(skillRole)" }
                lines.append(skillLine)
            }
            if let summary = agent.skillContentSummary, !summary.isEmpty {
                lines.append("  - Skill summary: \(summary)")
            }
            if let hash = agent.skillSnapshotHash {
                lines.append("  - Skill hash: \(hash)")
            }
            if let content = agent.resolvedSkillContent, !content.isEmpty {
                lines.append("  - Resolved skill content:")
                for line in content.split(separator: "\n", omittingEmptySubsequences: false) {
                    lines.append("    \(line)")
                }
            }
            if agent.mcpProfileID != nil
                || !agent.requestedMCPExtensions.isEmpty
                || !agent.predictedMCPExtensions.isEmpty
                || !agent.actualMCPExtensions.isEmpty
                || !agent.deniedMCPExtensions.isEmpty {
                lines.append("  - Backend MCP owner: \(agent.mcpProfileID ?? "none")")
                lines.append("  - MCP requested: \(joinedList(agent.requestedMCPExtensions))")
                lines.append("  - MCP predicted: \(joinedList(agent.predictedMCPExtensions))")
                lines.append("  - MCP actual: \(joinedList(agent.actualMCPExtensions))")
                lines.append("  - MCP denied: \(joinedList(agent.deniedMCPExtensions))")
            }
        }
        lines.append("")
        lines.append("## 6. Approvals")
        for approval in payload.approvalEntries {
            var line = "- **\(approval.gateLabel)** — \(approval.decision)"
            if let comment = approval.comment { line += " (\(comment))" }
            lines.append(line)
        }
        lines.append("")
        lines.append("## 7. Key Artifacts")
        for artifact in payload.keyArtifacts {
            let pinLabel = artifact.isPinned ? " [PINNED]" : ""
            lines.append("- \(artifact.name)\(pinLabel) (\(artifact.format)) — \(artifact.agentID) / \(artifact.stageID)")
        }
        lines.append("")
        lines.append("## 8. Strategy Context (Proposal 019)")
        if let profileID = payload.contextStrategyProfileID {
            lines.append("- Profile: \(profileID)")
        } else {
            lines.append("- Profile: not captured")
        }
        if let assignmentMode = payload.strategyAssignmentMode {
            lines.append("- Assignment mode: \(assignmentMode)")
        }
        if let recommendationState = payload.strategyRecommendationState {
            lines.append("- Recommendation state: \(recommendationState)")
        }
        lines.append("- Canonical strategy telemetry: \(payload.strategyTelemetryComplete ? "ready" : "incomplete")")
        lines.append("")
        lines.append("## 9. Recovery Notes")
        if let reason = payload.blockedReason { lines.append("- Blocked reason: \(reason)") }
        if let retry = payload.retryPath { lines.append("- Retry path: \(retry)") }
        if let resume = payload.resumePath { lines.append("- Resume path: \(resume)") }
        if let drift = payload.driftDecision { lines.append("- Drift decision: \(drift)") }
        lines.append("- Retries performed: \(payload.retriesPerformed)")
        if !payload.recoveryActionsTaken.isEmpty {
            lines.append("- Recovery actions taken:")
            for action in payload.recoveryActionsTaken {
                lines.append("  - \(action)")
            }
        }
        // Proposal 013: Failure evidence summaries
        if !payload.failureEvidenceSummaries.isEmpty {
            lines.append("## 10. Failure Evidence (Proposal 013)")
            for evidence in payload.failureEvidenceSummaries {
                lines.append("- **\(evidence.stageLabel)** (\(evidence.stageID))")
                lines.append("  - Failure class: \(evidence.failureClass)")
                lines.append("  - Summary: \(evidence.failureSummary)")
                lines.append("  - Raw output: \(evidence.rawOutputsExist ? "present" : "missing")")
                lines.append("  - Receipt: \(evidence.receiptExists ? "present" : "missing")")
                lines.append("  - Transcript: \(evidence.transcriptExists ? "present" : "missing")")
            }
        }
        lines.append("")
        if let summary = payload.proposalLoopSummary {
            lines.append("## 11. Proposal-loop Feedback (Proposal 022)")
            if summary.reviewCorpusBundlePresent {
                let rawCount = summary.reviewCorpusRawArtifactCount.map(String.init) ?? "unknown"
                lines.append("- Review corpus bundle: present (\(rawCount) raw reviews)")
            } else {
                lines.append("- Review corpus bundle: missing")
            }
            lines.append("- Backlog items: \(summary.backlogItemCount)")
            lines.append("- Unresolved items: \(summary.unresolvedItemCount)")
            lines.append("- Deferred items: \(summary.deferredItemCount)")
            lines.append("- Disputed items: \(summary.disputedItemCount)")
            if summary.partiallyResolvedItemCount > 0 {
                lines.append("- Partially resolved items: \(summary.partiallyResolvedItemCount)")
            }
            lines.append("- Merge provenance items: \(summary.mergeProvenanceItemCount)")
            lines.append("- Coverage: \(summary.coverageStatusSummary)")
            if let targeted = summary.targetedReviewerSummary {
                lines.append("- Targeted rereview: \(targeted)")
            }
            if let growthRatio = summary.proposalGrowthRatio {
                lines.append(String(format: "- Proposal growth ratio: %.2fx", growthRatio))
            }
            if let scoreDelta = summary.scoreDeltaSinceLastReview {
                lines.append(String(format: "- Score delta since last review: %.2f", scoreDelta))
            }
            if let closed = summary.backlogItemsClosedCount {
                lines.append("- Backlog items closed: \(closed)")
            }
            if let reopened = summary.reopenedItemCount, reopened > 0 {
                lines.append("- Backlog items reopened: \(reopened)")
            }
            if let recommendation = summary.growthGuardRecommendation {
                lines.append("- Growth guard: \(recommendation)")
            }
            if let nextAction = summary.boundedNextAction {
                lines.append("- Bounded next action: \(nextAction)")
            }
            lines.append("")
        }

        // Proposal 032: Transition cursor truth
        if payload.transitionCursorSettlementPhase != nil {
            lines.append("## 12. Transition Cursor (Proposal 032)")
            if let phase = payload.transitionCursorSettlementPhase {
                lines.append("- Settlement phase: \(phase)")
            }
            if let last = payload.transitionCursorLastCompletedStateID {
                lines.append("- Last completed state: \(last)")
            }
            if let next = payload.transitionCursorNextScheduledStateID {
                lines.append("- Next scheduled state: \(next)")
            }
            if let started = payload.transitionCursorNextStarted {
                lines.append("- Next-stage execution started: \(started ? "yes" : "no")")
            }
            lines.append("")
        }

        lines.append("## 13. Outcome")
        lines.append("- \(payload.runStatus)")
        lines.append("")
        return lines.joined(separator: "\n")
    }

    private func renderJSON(from payload: RunReportPayload) -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        encoder.dateEncodingStrategy = .iso8601
        guard let data = try? encoder.encode(payload) else { return "{}" }
        return String(data: data, encoding: .utf8) ?? "{}"
    }

    private func renderLatestSummaryMarkdown(from payload: RunReportPayload) -> String {
        var lines: [String] = []
        lines.append("# Latest Summary — \(payload.ideaTitle)")
        lines.append("")
        lines.append("**Status:** \(payload.runStatus) | **Trust:** \(payload.runtimeTrustLevel) | **Report v\(payload.version)**")
        lines.append("")
        lines.append("Stages: \(payload.completedStages) completed, \(payload.failedStages) failed, \(payload.skippedStages) skipped")
        lines.append("Elapsed: \(formattedDuration(payload.elapsedSeconds))")
        if let cost = payload.totalCostCents { lines.append("Cost: \(cost) cents") }
        if let drift = payload.driftNote { lines.append("Drift: \(drift)") }
        if let blockedReason = payload.blockedReason { lines.append("Blocked reason: \(blockedReason)") }
        if let retryPath = payload.retryPath { lines.append("Retry path: \(retryPath)") }
        if let resumePath = payload.resumePath { lines.append("Resume path: \(resumePath)") }
        if let profile = payload.contextStrategyProfileID { lines.append("Strategy: \(profile)") }
        if let mode = payload.strategyAssignmentMode { lines.append("Strategy mode: \(mode)") }
        if let state = payload.strategyRecommendationState { lines.append("Strategy recommendation state: \(state)") }
        if let feedback = payload.proposalLoopSummary {
            if feedback.reviewCorpusBundlePresent {
                let rawCount = feedback.reviewCorpusRawArtifactCount.map(String.init) ?? "unknown"
                lines.append("Proposal-loop review corpus bundle: present (\(rawCount) raw reviews)")
            } else {
                lines.append("Proposal-loop review corpus bundle: missing")
            }
            lines.append("Proposal-loop backlog items: \(feedback.backlogItemCount)")
            lines.append("Proposal-loop unresolved items: \(feedback.unresolvedItemCount)")
            lines.append("Proposal-loop merge provenance items: \(feedback.mergeProvenanceItemCount)")
            if let targeted = feedback.targetedReviewerSummary {
                lines.append("Proposal-loop targeted rereview: \(targeted)")
            }
            lines.append("Proposal-loop coverage: \(feedback.coverageStatusSummary)")
            if let growthRatio = feedback.proposalGrowthRatio {
                lines.append(String(format: "Proposal-loop growth ratio: %.2fx", growthRatio))
            }
            if let scoreDelta = feedback.scoreDeltaSinceLastReview {
                lines.append(String(format: "Proposal-loop score delta: %.2f", scoreDelta))
            }
            if let recommendation = feedback.growthGuardRecommendation {
                lines.append("Proposal-loop growth guard: \(recommendation)")
            }
            if let nextAction = feedback.boundedNextAction {
                lines.append("Proposal-loop bounded next action: \(nextAction)")
            }
        }
        lines.append("")
        if !payload.failureEvidenceSummaries.isEmpty {
            lines.append("Failure evidence summaries:")
            for evidence in payload.failureEvidenceSummaries {
                lines.append("- \(evidence.stageLabel): \(evidence.failureClass) — \(evidence.failureSummary)")
            }
            lines.append("")
        }
        return lines.joined(separator: "\n")
    }

    private func renderLatestSummaryJSON(from payload: RunReportPayload) -> String {
        // Emit a minimal summary subset
        let summary: [String: Any] = [
            "runID": payload.runID.uuidString,
            "status": payload.runStatus,
            "version": payload.version,
            "trust": payload.runtimeTrustLevel,
            "completedStages": payload.completedStages,
            "failedStages": payload.failedStages,
            "elapsedSeconds": payload.elapsedSeconds,
            "blockedReason": payload.blockedReason as Any,
            "retryPath": payload.retryPath as Any,
            "resumePath": payload.resumePath as Any,
            "failureEvidenceSummaries": payload.failureEvidenceSummaries.map {
                [
                    "stageID": $0.stageID,
                    "stageLabel": $0.stageLabel,
                    "failureClass": $0.failureClass,
                    "failureSummary": $0.failureSummary,
                    "rawOutputsExist": $0.rawOutputsExist,
                    "receiptExists": $0.receiptExists,
                    "transcriptExists": $0.transcriptExists
                ]
            },
            "proposalLoopSummary": payload.proposalLoopSummary.map { $0.backlogItemCount > -1 ? [
                "reviewCorpusBundlePresent": $0.reviewCorpusBundlePresent,
                "reviewCorpusRawArtifactCount": $0.reviewCorpusRawArtifactCount as Any,
                "backlogItemCount": $0.backlogItemCount,
                "unresolvedItemCount": $0.unresolvedItemCount,
                "deferredItemCount": $0.deferredItemCount,
                "disputedItemCount": $0.disputedItemCount,
                "partiallyResolvedItemCount": $0.partiallyResolvedItemCount,
                "addressedItemCount": $0.addressedItemCount,
                "mergeProvenanceItemCount": $0.mergeProvenanceItemCount,
                "coverageStatusSummary": $0.coverageStatusSummary,
                "targetedReviewerSummary": $0.targetedReviewerSummary as Any,
                "proposalByteSize": $0.proposalByteSize as Any,
                "previousProposalByteSize": $0.previousProposalByteSize as Any,
                "proposalGrowthRatio": $0.proposalGrowthRatio as Any,
                "scoreDeltaSinceLastReview": $0.scoreDeltaSinceLastReview as Any,
                "backlogItemsClosedCount": $0.backlogItemsClosedCount as Any,
                "reopenedItemCount": $0.reopenedItemCount as Any,
                "growthGuardRecommendation": $0.growthGuardRecommendation as Any,
                "boundedNextAction": $0.boundedNextAction as Any
            ] as [String: Any] : [:]
            } as Any,
            "contextStrategyProfileID": payload.contextStrategyProfileID as Any,
            "strategyAssignmentMode": payload.strategyAssignmentMode as Any,
            "strategyRecommendationState": payload.strategyRecommendationState as Any,
            "strategyTelemetryComplete": payload.strategyTelemetryComplete
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: summary, options: [.prettyPrinted, .sortedKeys]) else {
            return "{}"
        }
        return String(data: data, encoding: .utf8) ?? "{}"
    }

    // MARK: - Helpers

    private func elapsedTime(for run: Run) -> Double {
        let end = run.completedAt ?? Date()
        return end.timeIntervalSince(run.startedAt)
    }

    private func stageDuration(_ stage: StageExecution) -> Double {
        let end = stage.completedAt ?? Date()
        return end.timeIntervalSince(stage.startedAt)
    }

    private func agentDuration(_ agent: AgentExecution) -> Double {
        let end = agent.completedAt ?? Date()
        return end.timeIntervalSince(agent.startedAt)
    }

    // Proposal 011 (REQ-008): Decode frozen bindings from Run snapshot.
    private func decodeFrozenBindings(from run: Run) -> [String: ResolvedProviderBinding] {
        guard let data = run.providerBindingSnapshotJSON else { return [:] }
        return (try? JSONDecoder().decode([String: ResolvedProviderBinding].self, from: data)) ?? [:]
    }

    // Proposal 011 (REQ-009): Decode frozen provenances from Run snapshot.
    private func decodeFrozenProvenances(from run: Run) -> [String: FrozenBindingProvenance] {
        guard let data = run.bindingProvenanceJSON else { return [:] }
        return (try? JSONDecoder().decode([String: FrozenBindingProvenance].self, from: data)) ?? [:]
    }

    private func decodeResolvedSkills(from run: Run) -> [String: ResolvedSkill] {
        guard let data = run.resolvedSkillsJSON else { return [:] }
        return (try? JSONDecoder().decode([String: ResolvedSkill].self, from: data)) ?? [:]
    }

    private func decodeCatalogSkillRefs(from run: Run) -> [String: String] {
        guard let catalog = try? JSONDecoder().decode(AgentCatalog.self, from: run.catalogSnapshotJSON) else {
            return [:]
        }
        return Dictionary(uniqueKeysWithValues: catalog.agents.map { ($0.id, $0.skillRef) })
    }

    private func decodeFrozenMCPPolicies(from run: Run) -> [String: MCPPolicyResolutionReport] {
        guard let data = run.resolvedMCPPoliciesJSON else { return [:] }
        return (try? JSONDecoder().decode([String: MCPPolicyResolutionReport].self, from: data)) ?? [:]
    }

    private func decodeStringArray(from data: Data?) -> [String] {
        guard let data, let decoded = try? JSONDecoder().decode([String].self, from: data) else {
            return []
        }
        return decoded
    }

    private func strategyProfileID(for run: Run) -> String? {
        let value = run.contextStrategyProfileID.trimmingCharacters(in: .whitespacesAndNewlines)
        return value.isEmpty ? nil : value
    }

    private func strategyAssignmentMode(for run: Run) -> String? {
        let value = run.strategyAssignmentMode.trimmingCharacters(in: .whitespacesAndNewlines)
        return value.isEmpty ? nil : value
    }

    private func strategyRecommendationState(for run: Run) -> String? {
        let value = run.strategyRecommendationState.trimmingCharacters(in: .whitespacesAndNewlines)
        return value.isEmpty ? nil : value
    }

    private func hasCanonicalStrategyTelemetry(for run: Run) -> Bool {
        SessionReuseKPIExporter.hasCanonicalStrategyTelemetry(
            SessionReuseKPIExporter.decodeSummary(from: run.sessionKPIExportJSON)
        )
    }

    private func formattedDuration(_ seconds: Double) -> String {
        let mins = Int(seconds) / 60
        let secs = Int(seconds) % 60
        if mins > 0 { return "\(mins)m \(secs)s" }
        return "\(secs)s"
    }

    private func joinedList(_ values: [String]) -> String {
        values.isEmpty ? "none" : values.joined(separator: ", ")
    }

    private func reportFilePath(run: Run, name: String) -> String {
        let base = URL(fileURLWithPath: run.artifactRoot)
            .appendingPathComponent("reports", isDirectory: true)
        return base.appendingPathComponent(name).path
    }

    private func writeReportToDisk(content: String, path: String) throws {
        let url = URL(fileURLWithPath: path)
        try FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try content.write(to: url, atomically: true, encoding: .utf8)
    }

    private func pruneImmutableHistory(for run: Run, keepingLatestVersions retentionLimit: Int) throws {
        guard retentionLimit > 0 else { return }

        let minimumRetainedVersion = max(1, run.latestReportVersion - retentionLimit + 1)
        let runID = run.id
        let descriptor = FetchDescriptor<Artifact>(
            predicate: #Predicate<Artifact> { artifact in
                artifact.runID == runID && artifact.reportKind == "immutable_history"
            }
        )

        let staleArtifacts = try modelContext.fetch(descriptor).filter { artifact in
            guard let version = artifact.reportVersion else { return false }
            return version < minimumRetainedVersion
        }

        for artifact in staleArtifacts {
            let fileURL = URL(fileURLWithPath: artifact.filePath)
            if FileManager.default.fileExists(atPath: fileURL.path) {
                try FileManager.default.removeItem(at: fileURL)
            }
            modelContext.delete(artifact)
        }
    }
}

// MARK: - RunReportPayload (§6.4)

struct RunReportPayload: Codable, Sendable {
    let ideaTitle: String
    let workflowTitle: String
    let runID: UUID
    let runStatus: String
    let version: Int
    let startedAt: Date
    let completedAt: Date?
    let elapsedSeconds: Double
    let totalCostCents: Int64?

    // Snapshot and provenance
    let workflowSnapshotHash: String
    let catalogSnapshotHash: String
    let runtimeTrustLevel: String
    let driftNote: String?

    // Execution summary
    let completedStages: Int
    let skippedStages: Int
    let failedStages: Int
    let loopsEntered: Int
    let approvalsRequested: Int
    let approvalsGranted: Int
    let approvalsRejected: Int

    // Stage timeline
    let stageTimeline: [StageEntry]

    // Agents
    let agentsUsed: [AgentEntry]

    // Approvals
    let approvalEntries: [ApprovalEntry]

    // Key artifacts
    let keyArtifacts: [ArtifactEntry]

    // Proposal 019: Strategy context
    let contextStrategyProfileID: String?
    let strategyAssignmentMode: String?
    let strategyRecommendationState: String?
    let strategyTelemetryComplete: Bool

    // Recovery notes
    let blockedReason: String?
    let retryPath: String?
    let resumePath: String?
    let driftDecision: String?

    // §6.5: Retry/recovery narrative
    let retriesPerformed: Int
    let recoveryActionsTaken: [String]

    // Proposal 013: Failure evidence from canonical packet
    let failureEvidenceSummaries: [FailureEvidenceSummary]

    // Proposal 022: Feedback fidelity / review carry-forward summary
    let proposalLoopSummary: ProposalLoopFeedbackSummary?
    let mcpTelemetry: SessionReuseKPIExporter.MCPTelemetrySummary?

    // Proposal 032: Transition cursor truth for interrupted-transition reporting.
    let transitionCursorLastCompletedStateID: String?
    let transitionCursorNextScheduledStateID: String?
    let transitionCursorSettlementPhase: String?
    let transitionCursorNextStarted: Bool?

    struct FailureEvidenceSummary: Codable, Sendable {
        let stageID: String
        let stageLabel: String
        let failureClass: String
        let failureSummary: String
        let rawOutputsExist: Bool
        let receiptExists: Bool
        let transcriptExists: Bool
    }

    struct StageEntry: Codable, Sendable {
        let label: String
        let status: String
        let iteration: Int
        let attempt: Int
        let duration: Double
    }

    struct AgentEntry: Codable, Sendable {
        let agentID: String
        let provider: String
        let model: String?
        let effort: String
        let costCents: Int64?
        let duration: Double
        let finalStatus: String
        let skillRef: String?
        let skillType: String?
        let skillRole: String?
        let skillContentSummary: String?
        let skillSnapshotHash: String?
        let resolvedSkillContent: String?
        let mcpProfileID: String?
        let requestedMCPExtensions: [String]
        let predictedMCPExtensions: [String]
        let actualMCPExtensions: [String]
        let deniedMCPExtensions: [String]
        /// Proposal 026 ARCH-001: Actual runtime profile used for this execution.
        let runtimeProfileID: String?
        /// Proposal 026 ARCH-001: Actual adapter family of the runtime used.
        let actualAdapterFamily: String?
        /// Proposal 026 ARCH-001: Actual capability class of the runtime used.
        let actualCapabilityClass: String?
    }

    struct ApprovalEntry: Codable, Sendable {
        let gateLabel: String
        let decision: String
        let comment: String?
        let requestedAt: Date
        let decidedAt: Date?
    }

    struct ArtifactEntry: Codable, Sendable {
        let name: String
        let format: String
        let isPinned: Bool
        let agentID: String
        let stageID: String
    }
}
