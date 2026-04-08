import Foundation
import SwiftData

@MainActor
struct RunArtifactHierarchyBuilder {
    func build(for run: Run) -> RunArtifactHierarchy {
        let artifacts = persistedArtifacts(for: run)
        let promotedNames = decodePromotedArtifactNames(from: run.promotedHandoffArtifactsJSON)
        let stageExecutions = RunStageSnapshotLoader.load(for: run).sorted { lhs, rhs in
            if lhs.startedAt == rhs.startedAt {
                return lhs.attemptNumber > rhs.attemptNumber
            }
            return lhs.startedAt > rhs.startedAt
        }
        let stageLookup = StageExecutionLookup(stageExecutions: stageExecutions)

        var promotedArtifacts: [RunArtifactLeaf] = []
        var groupedLeaves: [StageGroupKey: StageGroupAccumulator] = [:]

        for artifact in artifacts.sorted(by: artifactSort(lhs:rhs:)) {
            let resolvedStage = stageLookup.resolve(for: artifact)
            let leaf = makeLeaf(
                artifact: artifact,
                resolvedStage: resolvedStage,
                run: run,
                promotedNames: promotedNames
            )

            if leaf.isPromoted {
                promotedArtifacts.append(leaf)
            }

            let groupKey = StageGroupKey(
                stageExecutionID: leaf.stageExecutionID,
                stageID: leaf.stageID,
                iteration: leaf.iteration,
                attemptNumber: leaf.attemptNumber
            )

            var accumulator = groupedLeaves[groupKey] ?? StageGroupAccumulator(
                stageExecutionID: leaf.stageExecutionID,
                stageID: leaf.stageID,
                stageLabel: leaf.stageLabel,
                iteration: leaf.iteration,
                attemptNumber: leaf.attemptNumber
            )

            accumulator.absorb(leaf, into: classify(artifact))
            groupedLeaves[groupKey] = accumulator
        }

        let stageGroups = groupedLeaves.values
            .map(\.stageGroup)
            .sorted(by: stageGroupSort(lhs:rhs:))

        return RunArtifactHierarchy(
            runID: run.id,
            latestSummaryArtifactID: run.latestSummaryArtifactID,
            latestImmutableReportArtifactID: run.latestImmutableReportArtifactID,
            latestReportVersion: run.latestReportVersion,
            promotedArtifacts: promotedArtifacts.sorted(by: leafSort(lhs:rhs:)),
            stageGroups: stageGroups
        )
    }

    private func persistedArtifacts(for run: Run) -> [Artifact] {
        guard let modelContext = run.modelContext else {
            return PersistedRunGraph.stageExecutions(for: run)
                .flatMap(\.agentExecutions)
                .flatMap(\.artifacts)
        }

        let runID = run.id
        let descriptor = FetchDescriptor<Artifact>(
            predicate: #Predicate<Artifact> { artifact in
                artifact.runID == runID
            },
            sortBy: [SortDescriptor(\.createdAt, order: .reverse)]
        )
        return (try? modelContext.fetch(descriptor)) ?? []
    }

    private func decodePromotedArtifactNames(from data: Data?) -> Set<String> {
        guard
            let data,
            let names = try? JSONDecoder().decode([String].self, from: data)
        else {
            return []
        }

        return Set(names)
    }

    private func makeLeaf(
        artifact: Artifact,
        resolvedStage: ResolvedStageExecution?,
        run: Run,
        promotedNames: Set<String>
    ) -> RunArtifactLeaf {
        let agentExecution = artifact.agentExecution
        let fileURL: URL? = artifact.filePath.isEmpty ? nil : URL(fileURLWithPath: artifact.filePath)
        let agentTitle = agentExecution?.agentTitle ?? artifact.agentID
        let isPromoted = artifact.isPinned || promotedNames.contains(artifact.name)

        return RunArtifactLeaf(
            artifactID: artifact.id,
            name: artifact.name,
            contractID: artifact.contractID,
            format: artifact.format,
            createdAt: artifact.createdAt,
            fileURL: fileURL,
            sizeBytes: artifact.sizeBytes,
            stageID: artifact.stageID,
            stageExecutionID: resolvedStage?.id,
            stageLabel: resolvedStage?.label ?? artifact.stageID,
            iteration: resolvedStage?.iteration ?? 0,
            attemptNumber: resolvedStage?.attemptNumber ?? artifact.attemptNumber,
            agentID: artifact.agentID,
            agentExecutionID: agentExecution?.id,
            agentTitle: agentTitle,
            provider: artifact.provider,
            model: artifact.model ?? agentExecution?.resolvedModel,
            effort: artifact.effort ?? agentExecution?.effort,
            agentAttemptNumber: artifact.agentAttemptNumber ?? agentExecution?.agentAttemptNumber,
            artifactLineageKind: artifact.artifactLineageKind,
            supersedesArtifactID: artifact.supersedesArtifactID,
            supersedesAgentArtifactID: artifact.supersedesAgentArtifactID,
            reportKind: artifact.reportKind,
            reportVersion: artifact.reportVersion,
            isPinned: artifact.isPinned,
            isPromoted: isPromoted,
            isLatestSummaryReport: artifact.id == run.latestSummaryArtifactID,
            isLatestImmutableReport: artifact.id == run.latestImmutableReportArtifactID
        )
    }

    private func classify(_ artifact: Artifact) -> RunArtifactBucketKind {
        let name = artifact.name.lowercased()
        let contractID = artifact.contractID.lowercased()
        let displayRole = artifact.displayRole?.lowercased() ?? ""

        if artifact.reportKind == "latest_summary" || name.contains("summary") || displayRole.contains("summary") {
            return .summary
        }
        if artifact.reportKind != nil || contractID.contains("run_report") {
            return .report
        }
        if artifact.format == .diff || name.contains("diff") || name.contains("patch") {
            return .diff
        }
        if name.contains("receipt") || contractID.contains("receipt") {
            return .receipt
        }
        if name.contains("transcript") {
            return .transcript
        }
        if name.contains("approval") {
            return .approvalContext
        }
        if name.contains("diagnostic") || name.contains("trace") || name.contains("debug") || name.contains("log") {
            return .diagnostic
        }
        if name.contains("review") {
            return .review
        }
        if name.contains("release") || name.contains("manifest") || contractID.contains("release") {
            return .release
        }
        if name.contains("delivery") || name.contains("publish") || name.contains("upload") {
            return .delivery
        }
        if name.contains("test") || contractID.contains("test") {
            return .test
        }
        return .other
    }

    private func artifactSort(lhs: Artifact, rhs: Artifact) -> Bool {
        if lhs.createdAt == rhs.createdAt {
            return lhs.name.localizedStandardCompare(rhs.name) == .orderedAscending
        }
        return lhs.createdAt > rhs.createdAt
    }

    private func leafSort(lhs: RunArtifactLeaf, rhs: RunArtifactLeaf) -> Bool {
        if lhs.createdAt == rhs.createdAt {
            return lhs.name.localizedStandardCompare(rhs.name) == .orderedAscending
        }
        return lhs.createdAt > rhs.createdAt
    }

    private func stageGroupSort(lhs: RunArtifactStageGroup, rhs: RunArtifactStageGroup) -> Bool {
        let lhsLatest = lhs.allArtifacts.map(\.createdAt).max() ?? .distantPast
        let rhsLatest = rhs.allArtifacts.map(\.createdAt).max() ?? .distantPast

        if lhsLatest == rhsLatest {
            if lhs.stageID == rhs.stageID {
                return lhs.attemptNumber > rhs.attemptNumber
            }
            return lhs.stageLabel.localizedStandardCompare(rhs.stageLabel) == .orderedAscending
        }

        return lhsLatest > rhsLatest
    }
}

private struct StageExecutionLookup {
    let stageExecutions: [RunStageSnapshot]

    func resolve(for artifact: Artifact) -> ResolvedStageExecution? {
        if let exact = stageExecutions.first(where: {
            $0.stageID == artifact.stageID && $0.attemptNumber == artifact.attemptNumber
        }) {
            return ResolvedStageExecution(exact)
        }

        if let fallback = stageExecutions.first(where: { $0.stageID == artifact.stageID }) {
            return ResolvedStageExecution(fallback)
        }

        return nil
    }
}

private struct ResolvedStageExecution {
    let id: UUID
    let label: String
    let iteration: Int
    let attemptNumber: Int

    init(_ stageExecution: RunStageSnapshot) {
        id = stageExecution.id
        label = stageExecution.label
        iteration = stageExecution.iteration
        attemptNumber = stageExecution.attemptNumber
    }
}

private struct StageGroupKey: Hashable {
    let stageExecutionID: UUID?
    let stageID: String
    let iteration: Int
    let attemptNumber: Int
}

private struct StageGroupAccumulator {
    let stageExecutionID: UUID?
    let stageID: String
    let stageLabel: String
    let iteration: Int
    let attemptNumber: Int

    private(set) var stageBuckets: [RunArtifactBucketKind: [RunArtifactLeaf]] = [:]
    private(set) var agentGroups: [AgentGroupKey: [RunArtifactBucketKind: [RunArtifactLeaf]]] = [:]
    private(set) var agentTitles: [AgentGroupKey: String] = [:]

    mutating func absorb(_ leaf: RunArtifactLeaf, into bucket: RunArtifactBucketKind) {
        if let agentExecutionID = leaf.agentExecutionID {
            let key = AgentGroupKey(agentExecutionID: agentExecutionID, agentID: leaf.agentID)
            var buckets = agentGroups[key] ?? [:]
            buckets[bucket, default: []].append(leaf)
            agentGroups[key] = buckets
            agentTitles[key] = leaf.agentTitle
        } else if leaf.agentID != "system", leaf.agentID.isEmpty == false {
            let key = AgentGroupKey(agentExecutionID: nil, agentID: leaf.agentID)
            var buckets = agentGroups[key] ?? [:]
            buckets[bucket, default: []].append(leaf)
            agentGroups[key] = buckets
            agentTitles[key] = leaf.agentTitle
        } else {
            stageBuckets[bucket, default: []].append(leaf)
        }
    }

    var stageGroup: RunArtifactStageGroup {
        let materializedStageBuckets = stageBuckets
            .map { bucket, artifacts in
                RunArtifactSemanticBucket(
                    bucket: bucket,
                    artifacts: artifacts.sorted(by: leafSort(lhs:rhs:))
                )
            }
            .sorted(by: bucketSort(lhs:rhs:))

        let materializedAgentGroups = agentGroups
            .map { key, buckets in
                let semanticBuckets = buckets
                    .map { bucket, artifacts in
                        RunArtifactSemanticBucket(
                            bucket: bucket,
                            artifacts: artifacts.sorted(by: leafSort(lhs:rhs:))
                        )
                    }
                    .sorted(by: bucketSort(lhs:rhs:))

                return RunArtifactAgentGroup(
                    agentExecutionID: key.agentExecutionID,
                    agentID: key.agentID,
                    agentTitle: agentTitles[key] ?? key.agentID,
                    semanticBuckets: semanticBuckets
                )
            }
            .sorted(by: agentGroupSort(lhs:rhs:))

        return RunArtifactStageGroup(
            stageExecutionID: stageExecutionID,
            stageID: stageID,
            stageLabel: stageLabel,
            iteration: iteration,
            attemptNumber: attemptNumber,
            stageBuckets: materializedStageBuckets,
            agentGroups: materializedAgentGroups
        )
    }

    private func bucketSort(lhs: RunArtifactSemanticBucket, rhs: RunArtifactSemanticBucket) -> Bool {
        lhs.bucketIndex < rhs.bucketIndex
    }

    private func agentGroupSort(lhs: RunArtifactAgentGroup, rhs: RunArtifactAgentGroup) -> Bool {
        let lhsLatest = lhs.allArtifacts.map(\.createdAt).max() ?? .distantPast
        let rhsLatest = rhs.allArtifacts.map(\.createdAt).max() ?? .distantPast

        if lhsLatest == rhsLatest {
            return lhs.agentTitle.localizedStandardCompare(rhs.agentTitle) == .orderedAscending
        }
        return lhsLatest > rhsLatest
    }

    private func leafSort(lhs: RunArtifactLeaf, rhs: RunArtifactLeaf) -> Bool {
        if lhs.createdAt == rhs.createdAt {
            return lhs.name.localizedStandardCompare(rhs.name) == .orderedAscending
        }
        return lhs.createdAt > rhs.createdAt
    }
}

private struct AgentGroupKey: Hashable {
    let agentExecutionID: UUID?
    let agentID: String
}

private extension RunArtifactSemanticBucket {
    var bucketIndex: Int {
        RunArtifactBucketKind.allCases.firstIndex(of: bucket) ?? .max
    }
}
