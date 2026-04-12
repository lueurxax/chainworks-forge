import Foundation

// MARK: - Proposal 019 Context Strategy Types

enum HandoffMode: String, Codable, Hashable, Sendable {
    case fullForward = "full_forward"
    case selective
    case full
    case none
}

enum ContextContinuityMode: String, Codable, Hashable, Sendable {
    case familyWithinRun = "family_within_run"
    case none
}

enum StrategyRecommendationStatus: String, Codable, Hashable, Sendable {
    case notEvaluated = "not_evaluated"
    case insufficientEvidence = "insufficient_evidence"
    case inconclusive
    case candidateWinner = "candidate_winner"
}

struct ArtifactPointer: Codable, Hashable, Sendable {
    let artifactName: String
    let absolutePath: String?
    let byteCount: Int
}

struct AgentHandoffPolicy: Codable, Hashable, Sendable {
    let mandatory: [String]
    let summarized: [String]
    let lazy: [String]

    init(
        mandatory: [String] = [],
        summarized: [String] = [],
        lazy: [String] = []
    ) {
        self.mandatory = mandatory
        self.summarized = summarized
        self.lazy = lazy
    }
}

struct ContextStrategyAgentRule: Codable, Hashable, Sendable {
    let handoffPolicy: AgentHandoffPolicy?
    let continuityMode: ContextContinuityMode?

    enum CodingKeys: String, CodingKey {
        case handoffPolicy = "handoff_policy"
        case continuityMode = "continuity_mode"
    }
}

struct ContextStrategyProfile: Codable, Hashable, Sendable {
    let profileID: String?
    let defaultHandoffMode: HandoffMode
    let defaultModelTier: String?
    let escalationModelTier: String?
    let agents: [String: ContextStrategyAgentRule]

    enum CodingKeys: String, CodingKey {
        case profileID = "profile_id"
        case defaultHandoffMode = "default_handoff_mode"
        case defaultModelTier = "default_model_tier"
        case escalationModelTier = "escalation_model_tier"
        case agents
    }
}

struct HandoffSummaryMetrics: Codable, Hashable, Sendable {
    let mandatoryArtifactCount: Int
    let summarizedArtifactCount: Int
    let lazyArtifactCount: Int
    let compactionCount: Int
    let payloadBytesBeforeStrategy: Int
    let payloadBytesAfterStrategy: Int

    var payloadReductionBytes: Int {
        max(0, payloadBytesBeforeStrategy - payloadBytesAfterStrategy)
    }
}

struct StrategyLimitPressureSignals: Codable, Hashable, Sendable {
    let inputPayloadBytes: Int
    let payloadBytesBeforeStrategy: Int
    let payloadBytesAfterStrategy: Int
    let payloadReductionBytes: Int
    let mandatoryArtifactCount: Int
    let summarizedArtifactCount: Int
    let lazyArtifactCount: Int
    let lazyEvidenceHitCount: Int?
    let lazyEvidenceHitRate: Double?
    let compactionCount: Int
    let cacheEffectiveness: Double?
    let compactionChurnCount: Int?
    let escalationCount: Int
    let retryableEscalationCount: Int
    let contractFailureCount: Int
    let operatorPromotedArtifactCount: Int
}

struct HandoffPacket: Codable, Sendable {
    let profileID: String
    let mode: HandoffMode
    let task: String
    let mandatoryArtifacts: [String: Data]
    let summaries: [String: String]
    let lazyArtifactRefs: [String: ArtifactPointer]
    let checkpoint: AgentSessionCheckpoint?
    let summaryMetrics: HandoffSummaryMetrics
    let promotedArtifacts: [String]

    var fingerprintMaterial: String {
        let summaryKeys = summaries.keys.sorted().joined(separator: ",")
        let lazyKeys = lazyArtifactRefs.keys.sorted().joined(separator: ",")
        let mandatoryKeys = mandatoryArtifacts.keys.sorted().joined(separator: ",")
        let promoted = promotedArtifacts.sorted().joined(separator: ",")
        return [
            profileID,
            mode.rawValue,
            mandatoryKeys,
            summaryKeys,
            lazyKeys,
            promoted
        ].joined(separator: "|")
    }
}

struct StrategyRecommendation: Codable, Hashable, Sendable {
    let status: StrategyRecommendationStatus
    let proofOwner: String
    let evaluationSetComplete: Bool
    let evaluationSetSummary: String
    let holdCriteria: [String]
    let recommendedProfileID: String?
    let rationale: String
}

struct ContextStrategySelection: Codable, Hashable, Sendable {
    let profileID: String
    let assignmentMode: String
    let recommendationState: String
    let profile: ContextStrategyProfile
}

enum ContextStrategyResolver {
    private static let baselineProfileID = "current_mixed_baseline"

    static func resolveSelection(
        selectedProfileID: String?,
        config: StewardConfig?
    ) -> ContextStrategySelection {
        let effectiveConfig = config ?? .defaultConfig
        let requestedProfileID = selectedProfileID?.trimmingCharacters(in: .whitespacesAndNewlines)
        let chosenProfileID: String
        let assignmentMode: String

        if let requestedProfileID,
           !requestedProfileID.isEmpty,
           let _ = effectiveConfig.contextStrategyProfiles[requestedProfileID] {
            chosenProfileID = requestedProfileID
            assignmentMode = requestedProfileID == baselineProfileID ? "default" : "manual_override"
        } else {
            chosenProfileID = baselineProfileID
            assignmentMode = "default"
        }

        let profile = (
            effectiveConfig.contextStrategyProfiles[chosenProfileID]
            ?? effectiveConfig.contextStrategyProfiles[baselineProfileID]
        )?.runtimeProfile(profileID: chosenProfileID) ?? ContextStrategyProfile(
            profileID: baselineProfileID,
            defaultHandoffMode: .fullForward,
            defaultModelTier: nil,
            escalationModelTier: nil,
            agents: [:]
        )

        return ContextStrategySelection(
            profileID: chosenProfileID,
            assignmentMode: assignmentMode,
            recommendationState: StrategyRecommendationStatus.notEvaluated.rawValue,
            profile: profile
        )
    }
}

struct HandoffCompiler: Sendable {
    func compile(
        profileID: String,
        profile: StewardContextStrategyProfile,
        agent: ResolvedAgent,
        task: AgentTask,
        context: ExecutionContext,
        promotedArtifacts: [String] = []
    ) -> HandoffPacket {
        compile(
            profileID: profileID,
            profile: profile.runtimeProfile(profileID: profileID),
            agent: agent,
            task: task,
            context: context,
            promotedArtifacts: promotedArtifacts
        )
    }

    func compile(
        profileID: String,
        profile: ContextStrategyProfile,
        agent: ResolvedAgent,
        task: AgentTask,
        context: ExecutionContext,
        promotedArtifacts: [String] = []
    ) -> HandoffPacket {
        let rule = profile.agents[agent.id] ?? profile.agents["*"]
        let policy = rule?.handoffPolicy
        let mode = profile.defaultHandoffMode
        let allArtifacts = context.inputArtifacts
        let allKeys = Set(allArtifacts.keys)
        let promotedKeys = Set(
            promotedArtifacts
                .filter { allKeys.contains($0) }
        )
        let semanticMandatoryKeys = semanticMandatoryArtifactKeys(
            agent: agent,
            task: task,
            available: allKeys,
            inputArtifacts: allArtifacts
        )

        var mandatoryKeys = Set(resolvedArtifactNames(
            requested: policy?.mandatory ?? [],
            available: allKeys
        ))
        mandatoryKeys.formUnion(promotedKeys)
        mandatoryKeys.formUnion(semanticMandatoryKeys)
        let summarizedKeys = Set(resolvedArtifactNames(
            requested: policy?.summarized ?? [],
            available: allKeys.subtracting(mandatoryKeys)
        ))
        let lazyKeys = Set(resolvedArtifactNames(
            requested: policy?.lazy ?? [],
            available: allKeys.subtracting(mandatoryKeys).subtracting(summarizedKeys)
        ))

        var mandatoryArtifacts: [String: Data] = [:]
        for key in mandatoryKeys.sorted() {
            if let data = allArtifacts[key] {
                mandatoryArtifacts[key] = data
            }
        }

        var summaries: [String: String] = [:]
        for key in summarizedKeys.sorted() {
            if let data = allArtifacts[key] {
                summaries[key] = summarize(data: data)
            }
        }

        var lazyRefs: [String: ArtifactPointer] = [:]
        for key in lazyKeys.sorted() {
            if let data = allArtifacts[key] {
                lazyRefs[key] = ArtifactPointer(
                    artifactName: key,
                    absolutePath: context.inputArtifactPaths[key],
                    byteCount: data.count
                )
            }
        }

        let bytesBefore = allArtifacts.values.reduce(0) { partial, data in
            partial + data.count
        }
        let mandatoryBytes = mandatoryArtifacts.values.reduce(0) { partial, data in
            partial + data.count
        }
        let summaryBytes = summaries.values.reduce(0) { partial, summary in
            partial + summary.utf8.count
        }
        let bytesAfter = mandatoryBytes + summaryBytes

        return HandoffPacket(
            profileID: profileID,
            mode: mode,
            task: task.task,
            mandatoryArtifacts: mandatoryArtifacts,
            summaries: summaries,
            lazyArtifactRefs: lazyRefs,
            checkpoint: nil,
            summaryMetrics: HandoffSummaryMetrics(
                mandatoryArtifactCount: mandatoryArtifacts.count,
                summarizedArtifactCount: summaries.count,
                lazyArtifactCount: lazyRefs.count,
                compactionCount: summaries.count,
                payloadBytesBeforeStrategy: bytesBefore,
                payloadBytesAfterStrategy: bytesAfter
            ),
            promotedArtifacts: promotedKeys.sorted()
        )
    }

    private func resolvedArtifactNames(
        requested: [String],
        available: Set<String>
    ) -> [String] {
        if requested.contains("*") {
            return Array(available).sorted()
        }

        return requested
            .filter { $0 != "current_task_description" }
            .filter { available.contains($0) }
            .sorted()
    }

    private func summarize(data: Data) -> String {
        let text = String(data: data, encoding: .utf8) ?? "<binary \(data.count) bytes>"
        let normalized = text.replacingOccurrences(of: "\n", with: " ").trimmingCharacters(in: .whitespacesAndNewlines)
        if normalized.count <= 240 {
            return normalized
        }
        let prefix = normalized.prefix(237)
        return "\(prefix)..."
    }

    private func semanticMandatoryArtifactKeys(
        agent: ResolvedAgent,
        task: AgentTask,
        available: Set<String>,
        inputArtifacts: [String: Data]
    ) -> Set<String> {
        let inlineLimitBytes = 64 * 1024

        if agent.mode.hasPrefix("proposal_review.") {
            let declaredInputs = Set((task.inputs ?? [])
                .filter { $0 != "current_task_description" }
                .filter { available.contains($0) })

            // Proposal reviewers compare current proposal intent against review evidence.
            // Their declared workflow inputs are core evidence, not opportunistic lazy context.
            return inlineEligibleArtifacts(
                declaredInputs,
                inputArtifacts: inputArtifacts,
                inlineLimitBytes: inlineLimitBytes
            )
        }

        if agent.id == "lead_orchestrator", task.task == "aggregate_proposal_reviews" {
            let declaredInputs = Set((task.inputs ?? [])
                .filter { $0 != "current_task_description" }
                .filter { available.contains($0) })

            // Review aggregation is not useful if the orchestrator spends its turn lazily
            // loading the core review corpus. Inline the declared review inputs so the
            // orchestrator can immediately synthesize the required aggregate outputs.
            return inlineEligibleArtifacts(
                declaredInputs,
                inputArtifacts: inputArtifacts,
                inlineLimitBytes: inlineLimitBytes
            )
        }

        if agent.id == "lead_orchestrator", task.task == "freeze_proposal_and_provision_worktree" {
            let declaredInputs = Set((task.inputs ?? [])
                .filter { $0 != "current_task_description" }
                .filter { available.contains($0) })

            // Implementation-start orchestration is fragile if proposal truth, review
            // verdict, and persisted run state are all left behind lazy reads. Inline the
            // declared planning inputs so the orchestrator can freeze and plan in one turn.
            return inlineEligibleArtifacts(
                declaredInputs,
                inputArtifacts: inputArtifacts,
                inlineLimitBytes: inlineLimitBytes
            )
        }

        if agent.id == "code_writer",
           task.task == "initial_implementation" || task.task == "continue_implementation" {
            let declaredInputs = Set((task.inputs ?? [])
                .filter { $0 != "current_task_description" }
                .filter { available.contains($0) })

            // Codex implementation turns are expensive when proposal truth and the
            // current plan/backlog are left behind lazy lookups. Inline the declared
            // implementation inputs so the agent starts from concrete files instead of
            // broad repo discovery.
            return inlineEligibleArtifacts(
                declaredInputs,
                inputArtifacts: inputArtifacts,
                inlineLimitBytes: inlineLimitBytes
            )
        }

        return []
    }

    private func inlineEligibleArtifacts(
        _ artifactNames: Set<String>,
        inputArtifacts: [String: Data],
        inlineLimitBytes: Int
    ) -> Set<String> {
        guard !artifactNames.isEmpty else {
            return []
        }
        return Set(
            artifactNames.filter { artifactName in
                guard let data = inputArtifacts[artifactName] else { return false }
                return data.count <= inlineLimitBytes
            }
        )
    }
}

extension StewardContextStrategyProfile {
    func runtimeProfile(profileID: String) -> ContextStrategyProfile {
        ContextStrategyProfile(
            profileID: profileID,
            defaultHandoffMode: {
                switch defaultHandoffMode {
                case .selective: return .selective
                case .fullForward: return .fullForward
                case .full: return .full
                }
            }(),
            defaultModelTier: defaultModelTier,
            escalationModelTier: escalationModelTier,
            agents: agents.mapValues { rule in
                ContextStrategyAgentRule(
                    handoffPolicy: rule.handoffPolicy.map {
                        AgentHandoffPolicy(
                            mandatory: $0.mandatory,
                            summarized: $0.summarized,
                            lazy: $0.lazy
                        )
                    },
                    continuityMode: rule.continuityMode.map {
                        switch $0 {
                        case .familyWithinRun: return .familyWithinRun
                        case .none: return .none
                        }
                    }
                )
            }
        )
    }
}
