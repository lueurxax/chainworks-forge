import Foundation

// MARK: - Proposal 013 Layer Q: Declarative Coverage Report

/// Emits testable evidence of which YAML fields are executable truth
/// versus intentionally non-runtime metadata, including tier classification
/// for every Appendix B row.
struct DeclarativeCoverageReport: Codable, Sendable {

    let reportVersion: Int
    let generatedAt: Date
    let agentCatalogEntries: [CatalogCoverageEntry]
    let workflowEntries: [WorkflowCoverageEntry]

    init(generatedAt: Date = Date()) {
        self.reportVersion = 1
        self.generatedAt = generatedAt
        self.agentCatalogEntries = Self.buildCatalogEntries()
        self.workflowEntries = Self.buildWorkflowEntries()
    }

    // MARK: - Catalog Coverage Entries (Appendix B — agents.yaml)

    private static func buildCatalogEntries() -> [CatalogCoverageEntry] {
        [
            // Tier 1 — 013 mandatory hardening
            CatalogCoverageEntry(
                surface: "contracts.*",
                tier: .tier1Mandatory,
                status: .enforced,
                truth: "Output-to-contract binding is fully catalog-driven via OutputContractResolverV2. No hardcoded fallback branches."
            ),
            CatalogCoverageEntry(
                surface: "backend_profiles.*.structured_output",
                tier: .tier1Mandatory,
                status: .enforced,
                truth: "Reaches transport or triggers preflight failure via StructuredOutputSchemaGate."
            ),

            // Tier 2 — metadata-only by design
            CatalogCoverageEntry(
                surface: "app.*",
                tier: .tier2MetadataOnly,
                status: .metadataOnly,
                truth: "Decoded from YAML but no runtime component reads catalog-level app settings."
            ),
            CatalogCoverageEntry(
                surface: "paths.*",
                tier: .tier2MetadataOnly,
                status: .metadataOnly,
                truth: "Only scanned by env-placeholder validation; does not drive runtime path resolution."
            ),
            CatalogCoverageEntry(
                surface: "artifacts.*",
                tier: .tier2MetadataOnly,
                status: .metadataOnly,
                truth: "Artifact names validated and declared paths can hint format detection. Persistence layout is engine-owned."
            ),
            CatalogCoverageEntry(
                surface: "agents.*.worktree_policy.strategy/path/base_branch",
                tier: .tier2MetadataOnly,
                status: .metadataOnly,
                truth: "Parsed and validated as strings only; worktree provisioning uses delivery configuration."
            ),
            CatalogCoverageEntry(
                surface: "agents.*.notes",
                tier: .tier2MetadataOnly,
                status: .metadataOnly,
                truth: "Present in schema but not consumed by runtime or UI. Purely descriptive."
            ),

            // Tier 3 — later proposal / later platform work
            CatalogCoverageEntry(
                surface: "skills.*",
                tier: .tier3LaterProposal,
                status: .partial,
                truth: "Skill definitions exist in YAML but skill content is not resolved into live execution. Tracked for Proposal 015."
            ),
            CatalogCoverageEntry(
                surface: "agents.*.skill_ref / skill_role",
                tier: .tier3LaterProposal,
                status: .partial,
                truth: "Parsed, validated, displayed, hashed into provenance; NOT injected into Goose prompts. Tracked for Proposal 015."
            ),
            CatalogCoverageEntry(
                surface: "backend_profiles.*.effort",
                tier: .tier3LaterProposal,
                status: .partial,
                truth: "Persisted in provenance/receipts and provider binding but not sent as transport control."
            ),
            CatalogCoverageEntry(
                surface: "backend_profiles.*.max_turns / temperature",
                tier: .tier3LaterProposal,
                status: .partial,
                truth: "Carried into ResolvedAgent and hashes but not enforced by live Goose transport."
            ),
            CatalogCoverageEntry(
                surface: "permission_profiles.*",
                tier: .tier3LaterProposal,
                status: .partial,
                truth: "Profile existence validated, profile ID sent to Goose, some profile names drive side-effect heuristics. Detailed allowlists not enforced."
            ),
            CatalogCoverageEntry(
                surface: "agents.*.required_tools",
                tier: .tier3LaterProposal,
                status: .unused,
                truth: "Declared in YAML but not checked before or during execution."
            ),
            CatalogCoverageEntry(
                surface: "agents.*.requires_human_approval",
                tier: .tier3LaterProposal,
                status: .partial,
                truth: "Used by resume-side-effect heuristics but actual gate behavior owned by workflow approval states."
            ),

            // Already used — no action needed
            CatalogCoverageEntry(
                surface: "backend_profiles.*.provider / model",
                tier: .alreadyUsed,
                status: .enforced,
                truth: "Drive provider-family resolution and live model selection."
            ),
            CatalogCoverageEntry(
                surface: "agents.*.worktree_policy.write_enabled",
                tier: .alreadyUsed,
                status: .enforced,
                truth: "Only worktree-policy field that changes runtime behavior."
            ),
        ]
    }

    // MARK: - Workflow Coverage Entries (Appendix B — workflow.yaml)

    private static func buildWorkflowEntries() -> [WorkflowCoverageEntry] {
        [
            // Tier 2 — metadata-only by design
            WorkflowCoverageEntry(
                surface: "workflow.uses_agent_catalog",
                tier: .tier2MetadataOnly,
                status: .unused,
                truth: "App receives workflow and catalog URLs externally; not dereferenced at runtime."
            ),
            WorkflowCoverageEntry(
                surface: "workflow.description",
                tier: .tier2MetadataOnly,
                status: .unused,
                truth: "Stored in YAML but not consumed by current runtime surfaces."
            ),
            WorkflowCoverageEntry(
                surface: "workflow.idea_input.mode",
                tier: .tier2MetadataOnly,
                status: .unused,
                truth: "Idea intake behavior is app-owned; this field does not drive UI or execution."
            ),
            WorkflowCoverageEntry(
                surface: "scoring.*",
                tier: .tier2MetadataOnly,
                status: .unused,
                truth: "Parsed and persisted but pass/fail behavior encoded directly in state transition expressions."
            ),

            // Tier 3 — later proposal
            WorkflowCoverageEntry(
                surface: "workflow.execution.single_active_run_per_idea / resume_policy",
                tier: .tier3LaterProposal,
                status: .unused,
                truth: "Current behavior exists in app/runtime code but not because these YAML values are read."
            ),
            WorkflowCoverageEntry(
                surface: "workflow.required_providers",
                tier: .tier3LaterProposal,
                status: .partial,
                truth: "Used for validation and preflight but not as separate runtime gate beyond agent bindings."
            ),
            WorkflowCoverageEntry(
                surface: "failure_policy.preserve_artifacts",
                tier: .tier3LaterProposal,
                status: .unused,
                truth: "Parsed and persisted in RunPlan but no runtime branch reads it."
            ),

            // Already used — no action needed
            WorkflowCoverageEntry(
                surface: "variables.*",
                tier: .alreadyUsed,
                status: .enforced,
                truth: "Loop budgets and transition expressions read runtime variables."
            ),
            WorkflowCoverageEntry(
                surface: "failure_policy.on_error / on_loop_budget_exhausted",
                tier: .alreadyUsed,
                status: .enforced,
                truth: "Orchestration failure handling reads these values."
            ),
            WorkflowCoverageEntry(
                surface: "states.*.label/type/owner/approval/run/run_after_approval/loop/transitions",
                tier: .alreadyUsed,
                status: .enforced,
                truth: "Actively shape compilation, orchestration, approval handling, workflow-map projection, and loop control."
            ),
        ]
    }

    // MARK: - Summary

    var mandatoryTierCount: Int {
        agentCatalogEntries.filter { $0.tier == .tier1Mandatory }.count +
        workflowEntries.filter { $0.tier == .tier1Mandatory }.count
    }

    var mandatoryTierEnforced: Int {
        agentCatalogEntries.filter { $0.tier == .tier1Mandatory && $0.status == .enforced }.count +
        workflowEntries.filter { $0.tier == .tier1Mandatory && $0.status == .enforced }.count
    }

    var allMandatoryEnforced: Bool {
        mandatoryTierCount == mandatoryTierEnforced
    }
}

// MARK: - Coverage Entry Types

struct CatalogCoverageEntry: Codable, Sendable {
    let surface: String
    let tier: CoverageTier
    let status: CoverageStatus
    let truth: String
}

struct WorkflowCoverageEntry: Codable, Sendable {
    let surface: String
    let tier: CoverageTier
    let status: CoverageStatus
    let truth: String
}

// MARK: - Coverage Tier (Appendix B)

enum CoverageTier: String, Codable, Sendable {
    /// Must gain runtime enforcement or fail-closed in Proposal 013.
    case tier1Mandatory = "tier_1_mandatory"
    /// Intentionally non-runtime; schema and docs must say so explicitly.
    case tier2MetadataOnly = "tier_2_metadata_only"
    /// Execution-relevant but out of scope for this slice.
    case tier3LaterProposal = "tier_3_later_proposal"
    /// Already used by runtime — no action needed.
    case alreadyUsed = "already_used"
}

// MARK: - Coverage Status

enum CoverageStatus: String, Codable, Sendable {
    case enforced
    case partial
    case unused
    case metadataOnly = "metadata_only"
}
