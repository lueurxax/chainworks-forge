// ProposalReviewRouting.swift
//
// P060 Phase 3 Swift parity: Codable mirror of `domain::routing` (Rust)
// and the `RoutingEvidenceProjectionAuthorizer` redaction primitive so
// the macOS app can decode `agent_selection_plan_v1` and routing
// receipt artifacts without leaking debug-tier evidence.
//
// What this file ships:
//   - `ReviewRoutingMode` (legacyFixed | shadowDynamic | dynamic)
//   - `RoutingEvidenceRef` with `redacted()`
//   - `ScoreTerms`, `SelectedAgent`, `RejectedAlternative`,
//     `IneligibleCandidate`, `InputSnapshotHashes`, `AgentSelectionPlanV1`
//   - `ReviewRoutingOptions`, `RoutingMetadata`
//   - `RoutingEvidenceProjectionAuthorizer` (default-deny, env-gated)
//   - `EffectiveRoutingModeResolution` + `resolveEffectiveRoutingMode`
//
// What this file does NOT ship:
//   - The deterministic scoring algorithm (`route_proposal_reviewers`
//     equivalent). That is a separate parity slice tracked in the P060
//     remaining_code_tasks; the Swift layer here only needs to safely
//     decode and project the Rust output, not reproduce the decision.
//
// JSON shape parity with Rust serde:
//   - All keys are camelCase on the Swift side, and the Codable layer
//     uses `CodingKeys` to map to/from snake_case wire JSON, matching
//     `#[serde(rename_all = "snake_case")]` and similar attributes on
//     the Rust types.
//   - Optional fields use `decodeIfPresent` / `encodeIfPresent` and
//     mirror Rust `Option<T>` with `#[serde(default, skip_serializing_if)]`.

import Foundation

// MARK: - ReviewRoutingMode

/// Routing mode for proposal review. Mirror of
/// `domain::routing::ReviewRoutingMode` — three variants:
///
///  - `legacyFixed`: route review to the hard-coded fixed quartet
///    (Product Owner, UX, UI, Architect).
///  - `shadowDynamic`: run the dynamic algorithm and persist the
///    plan as evidence, but the legacy fixed quartet still drives
///    the actual reviewer dispatch. Used for A/B comparison before
///    cutover.
///  - `dynamic`: dispatch reviewers selected by the algorithm. Default.
public enum ReviewRoutingMode: String, Codable, Hashable, CaseIterable {
    case legacyFixed = "legacy_fixed"
    case shadowDynamic = "shadow_dynamic"
    case dynamic = "dynamic"

    /// Default matches Rust `Default for ReviewRoutingMode`.
    public static var defaultMode: ReviewRoutingMode { .dynamic }
}

// MARK: - RoutingFailureKind

public enum RoutingFailureKind: String, Codable, Hashable, CaseIterable {
    case overrideConflict = "override_conflict"
    case mandatoryOverflow = "mandatory_overflow"
    case disabledRolloutWave = "disabled_rollout_wave"
    case unknownAgent = "unknown_agent"
    case placeholderResolvedAgent = "placeholder_resolved_agent"
    case malformedRoutingMetadata = "malformed_routing_metadata"
    case mixedVersionSnapshot = "mixed_version_snapshot"
    case missingOutputContract = "missing_output_contract"
}

// MARK: - RoutingEvidenceRef + redaction

/// Traceable evidence reference. Raw fields require the
/// `operator_debug_routing_evidence` capability for readback (see
/// `RoutingEvidenceProjectionAuthorizer`).
public struct RoutingEvidenceRef: Codable, Hashable {
    public let evidenceId: String
    public let evidenceType: String
    public let hash: String
    public var normalizedValue: String?
    public var path: String?
    public var symbol: String?
    public var span: String?

    public init(
        evidenceId: String,
        evidenceType: String,
        hash: String,
        normalizedValue: String? = nil,
        path: String? = nil,
        symbol: String? = nil,
        span: String? = nil
    ) {
        self.evidenceId = evidenceId
        self.evidenceType = evidenceType
        self.hash = hash
        self.normalizedValue = normalizedValue
        self.path = path
        self.symbol = symbol
        self.span = span
    }

    private enum CodingKeys: String, CodingKey {
        case evidenceId = "evidence_id"
        case evidenceType = "evidence_type"
        case hash
        case normalizedValue = "normalized_value"
        case path
        case symbol
        case span
    }

    /// Hash-only projection — drops every raw field.
    /// Mirror of `RoutingEvidenceRef::redacted()` in Rust.
    public func redacted() -> RoutingEvidenceRef {
        RoutingEvidenceRef(
            evidenceId: evidenceId,
            evidenceType: evidenceType,
            hash: hash
        )
    }
}

// MARK: - ScoreTerms

public struct ScoreTerms: Codable, Hashable {
    public var forceInclude: Int
    public var stackMatches: Int
    public var surfaceMatches: Int
    public var riskMatches: Int
    public var strongKeywordMatches: Int
    public var repoSignalMatches: Int
    public var crossStackDependencyMatches: Int
    public var baselineGapMatches: Int
    public var overlapPenalty: Int

    /// Total score per Proposal 060 §5 formula:
    ///   5*force_include + 4*stack + 3*surface + 3*risk + 2*strong_keyword +
    ///   2*repo_signal + 2*cross_stack_dep + 1*baseline_gap - 3*overlap
    public func total() -> Int {
        forceInclude * 5
            + stackMatches * 4
            + surfaceMatches * 3
            + riskMatches * 3
            + strongKeywordMatches * 2
            + repoSignalMatches * 2
            + crossStackDependencyMatches * 2
            + baselineGapMatches * 1
            - overlapPenalty * 3
    }

    private enum CodingKeys: String, CodingKey {
        case forceInclude = "force_include"
        case stackMatches = "stack_matches"
        case surfaceMatches = "surface_matches"
        case riskMatches = "risk_matches"
        case strongKeywordMatches = "strong_keyword_matches"
        case repoSignalMatches = "repo_signal_matches"
        case crossStackDependencyMatches = "cross_stack_dependency_matches"
        case baselineGapMatches = "baseline_gap_matches"
        case overlapPenalty = "overlap_penalty"
    }
}

public struct ScoreWeights: Codable, Hashable {
    public let forceInclude: Int
    public let stackMatches: Int
    public let surfaceMatches: Int
    public let riskMatches: Int
    public let strongKeywordMatches: Int
    public let repoSignalMatches: Int
    public let crossStackDependencyMatches: Int
    public let baselineGapMatches: Int
    public let overlapPenalty: Int

    private enum CodingKeys: String, CodingKey {
        case forceInclude = "force_include"
        case stackMatches = "stack_matches"
        case surfaceMatches = "surface_matches"
        case riskMatches = "risk_matches"
        case strongKeywordMatches = "strong_keyword_matches"
        case repoSignalMatches = "repo_signal_matches"
        case crossStackDependencyMatches = "cross_stack_dependency_matches"
        case baselineGapMatches = "baseline_gap_matches"
        case overlapPenalty = "overlap_penalty"
    }
}

// MARK: - Plan participant types

public struct SelectedAgent: Codable, Hashable {
    public let agentId: String
    public let routingId: String
    public let score: Int
    public let mandatory: Bool
    public let overrideSource: String?
    public let scoreTerms: ScoreTerms
    public let rationale: String
    public var evidenceRefs: [RoutingEvidenceRef]
    public let materializationBindingId: String

    private enum CodingKeys: String, CodingKey {
        case agentId = "agent_id"
        case routingId = "routing_id"
        case score
        case mandatory
        case overrideSource = "override_source"
        case scoreTerms = "score_terms"
        case rationale
        case evidenceRefs = "evidence_refs"
        case materializationBindingId = "materialization_binding_id"
    }
}

public struct RejectedAlternative: Codable, Hashable {
    public let agentId: String
    public let routingId: String
    public let score: Int
    public let reason: String
    public let scoreTerms: ScoreTerms

    private enum CodingKeys: String, CodingKey {
        case agentId = "agent_id"
        case routingId = "routing_id"
        case score
        case reason
        case scoreTerms = "score_terms"
    }
}

public struct IneligibleCandidate: Codable, Hashable {
    public let agentId: String
    public let routingId: String
    public let reason: String

    private enum CodingKeys: String, CodingKey {
        case agentId = "agent_id"
        case routingId = "routing_id"
        case reason
    }
}

// MARK: - InputSnapshotHashes

/// Frozen hashes of all routing inputs for determinism verification.
/// Mirrors Rust `domain::routing::InputSnapshotHashes`.
public struct InputSnapshotHashes: Codable, Hashable {
    public let workflowSnapshotHash: String
    public let catalogSnapshotHash: String
    public let routingMetadataHash: String
    public let candidateBindingHash: String
    public let evidenceHash: String
    public let overrideHash: String?

    private enum CodingKeys: String, CodingKey {
        case workflowSnapshotHash = "workflow_snapshot_hash"
        case catalogSnapshotHash = "catalog_snapshot_hash"
        case routingMetadataHash = "routing_metadata_hash"
        case candidateBindingHash = "candidate_binding_hash"
        case evidenceHash = "evidence_hash"
        case overrideHash = "override_hash"
    }
}

// MARK: - AgentSelectionPlanV1

/// Codable mirror of `domain::routing::AgentSelectionPlanV1`. The macOS
/// app receives this via the GraphQL `Run.workflowConflict` enrichment
/// or via reading the `agent_selection_plan_v1` artifact under
/// `<run_dir>/routing/agent-selection-plan.v1.json`.
public struct AgentSelectionPlanV1: Codable, Hashable {
    public let schemaVersion: String
    public let routingRulesVersion: String
    public let proposalMd5: String
    public let planHash: String
    public let mode: ReviewRoutingMode
    public let fingerprint: [String]
    public var selectedAgents: [SelectedAgent]
    public let rejectedAlternatives: [RejectedAlternative]
    public let ineligibleCandidates: [IneligibleCandidate]
    public let warnings: [String]
    public let inputSnapshotHashes: InputSnapshotHashes

    private enum CodingKeys: String, CodingKey {
        case schemaVersion = "schema_version"
        case routingRulesVersion = "routing_rules_version"
        case proposalMd5 = "proposal_md5"
        case planHash = "plan_hash"
        case mode
        case fingerprint
        case selectedAgents = "selected_agents"
        case rejectedAlternatives = "rejected_alternatives"
        case ineligibleCandidates = "ineligible_candidates"
        case warnings
        case inputSnapshotHashes = "input_snapshot_hashes"
    }
}

// MARK: - ReviewRoutingOptions + RoutingMetadata

public struct ReviewRoutingOptions: Codable, Hashable {
    public var mode: ReviewRoutingMode
    public var forceInclude: [String]
    public var forceExclude: [String]
    public var operatorOverrideRationale: String?
    public var operatorId: String?
    public var createdAt: Date?

    public init(
        mode: ReviewRoutingMode = .dynamic,
        forceInclude: [String] = [],
        forceExclude: [String] = [],
        operatorOverrideRationale: String? = nil,
        operatorId: String? = nil,
        createdAt: Date? = nil
    ) {
        self.mode = mode
        self.forceInclude = forceInclude
        self.forceExclude = forceExclude
        self.operatorOverrideRationale = operatorOverrideRationale
        self.operatorId = operatorId
        self.createdAt = createdAt
    }

    private enum CodingKeys: String, CodingKey {
        case mode
        case forceInclude = "force_include"
        case forceExclude = "force_exclude"
        case operatorOverrideRationale = "override_reason"
        case operatorId = "operator_id"
        case createdAt = "created_at"
    }
}

public struct RoutingMetadata: Codable, Hashable {
    public let routingId: String
    public let family: String
    public let capabilities: [String]
    public let stacks: [String]
    public let surfaces: [String]
    public let risks: [String]
    public let enabledForProposalReview: Bool
    public let rolloutWave: String
    public let strongProposalKeywords: [String]?
    public let strongRepoFiles: [String]?
    public let strongRepoSymbols: [String]?
    public let scoreWeights: ScoreWeights?

    private enum CodingKeys: String, CodingKey {
        case routingId = "routing_id"
        case family
        case capabilities
        case stacks
        case surfaces
        case risks
        case enabledForProposalReview = "enabled_for_proposal_review"
        case rolloutWave = "rollout_wave"
        case strongProposalKeywords = "strong_proposal_keywords"
        case strongRepoFiles = "strong_repo_files"
        case strongRepoSymbols = "strong_repo_symbols"
        case scoreWeights = "score_weights"
    }
}

public struct ReviewCorpusBundleV2: Codable, Hashable {
    public let selectedReviewArtifacts: [String]
    public let selectedReviewerIds: [String]
    public let reviewerCount: Int
    public let selectionPlanHash: String
    public let selectionPlan: AgentSelectionPlanV1
    public let legacyFixedMode: Bool

    private enum CodingKeys: String, CodingKey {
        case selectedReviewArtifacts = "selected_review_artifacts"
        case selectedReviewerIds = "selected_reviewer_ids"
        case reviewerCount = "reviewer_count"
        case selectionPlanHash = "selection_plan_hash"
        case selectionPlan = "selection_plan"
        case legacyFixedMode = "legacy_fixed_mode"
    }
}

// MARK: - RoutingEvidenceProjectionAuthorizer

/// Outcome of an evidence-projection authorization check.
public enum RoutingEvidenceProjection: Hashable {
    case full
    case redacted
}

/// P060 Phase 3 Swift mirror of the Rust authorizer. Default-deny.
///
/// The macOS UI is always operator-class; full evidence projection is
/// gated on the `CHAINWORKS_OPERATOR_DEBUG_ROUTING_EVIDENCE` env var on
/// the daemon side, so the Swift layer must rely on the daemon to
/// already redact wire payloads. This authorizer exists for cases where
/// the Swift code itself receives full payloads (e.g. from a local
/// shared fixture loaded directly from disk) and wants to apply the
/// same redaction policy before showing them to the operator.
public struct RoutingEvidenceProjectionAuthorizer {
    public let projection: RoutingEvidenceProjection

    public static let redactedOnly = RoutingEvidenceProjectionAuthorizer(projection: .redacted)
    public static let full = RoutingEvidenceProjectionAuthorizer(projection: .full)

    public init(projection: RoutingEvidenceProjection) {
        self.projection = projection
    }

    /// Resolve the projection level from environment.
    /// Operator-class is implicit on the macOS app side (it is the
    /// operator surface), so the Swift entry point is parameterised
    /// only by the env var.
    public static func fromEnvironment(
        environment: [String: String] = ProcessInfo.processInfo.environment
    ) -> RoutingEvidenceProjectionAuthorizer {
        guard let raw = environment["CHAINWORKS_OPERATOR_DEBUG_ROUTING_EVIDENCE"] else {
            return .redactedOnly
        }
        let normalized = raw.lowercased()
        if normalized == "1" || normalized == "true" {
            return .full
        }
        return .redactedOnly
    }

    public func project(_ evidence: RoutingEvidenceRef) -> RoutingEvidenceRef {
        switch projection {
        case .full: return evidence
        case .redacted: return evidence.redacted()
        }
    }

    public func project(_ refs: [RoutingEvidenceRef]) -> [RoutingEvidenceRef] {
        refs.map(project)
    }

    /// Project an entire selection plan — redacts selected-agent evidence_refs only.
    /// Other plan fields stay intact since they are not raw-evidence.
    public func project(_ plan: AgentSelectionPlanV1) -> AgentSelectionPlanV1 {
        var copy = plan
        copy.selectedAgents = plan.selectedAgents.map { agent in
            var projectedAgent = agent
            projectedAgent.evidenceRefs = project(agent.evidenceRefs)
            return projectedAgent
        }
        return copy
    }
}

// MARK: - Feature-flag cutover resolver

/// Env var name parity with Rust `domain::routing::ROUTING_MODE_OVERRIDE_ENV`.
public let routingModeOverrideEnvName = "CHAINWORKS_P060_ROUTING_MODE_OVERRIDE"

/// Mirror of Rust `EffectiveRoutingModeResolution`.
public enum EffectiveRoutingModeResolution: Hashable {
    case usedPerRunMode(ReviewRoutingMode)
    case overriddenByEnv(from: ReviewRoutingMode, to: ReviewRoutingMode)
    case overrideUnrecognized(raw: String, perRun: ReviewRoutingMode)

    public func effective() -> ReviewRoutingMode {
        switch self {
        case .usedPerRunMode(let mode):
            return mode
        case .overriddenByEnv(_, let to):
            return to
        case .overrideUnrecognized(_, let perRun):
            return perRun
        }
    }
}

/// Resolve the effective routing mode, considering the per-run YAML
/// setting and the `CHAINWORKS_P060_ROUTING_MODE_OVERRIDE` env var.
///
/// This Swift-side helper exists so test fixtures and developer tooling
/// can reproduce the daemon's mode-resolution decision without an IPC
/// roundtrip. The runtime cutover decision still happens daemon-side.
public func resolveEffectiveRoutingMode(
    perRunMode: ReviewRoutingMode,
    environment: [String: String] = ProcessInfo.processInfo.environment
) -> EffectiveRoutingModeResolution {
    guard let raw = environment[routingModeOverrideEnvName] else {
        return .usedPerRunMode(perRunMode)
    }
    if let overrideMode = ReviewRoutingMode(rawValue: raw) {
        return .overriddenByEnv(from: perRunMode, to: overrideMode)
    }
    return .overrideUnrecognized(raw: raw, perRun: perRunMode)
}
