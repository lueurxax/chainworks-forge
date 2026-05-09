//! Compiled run plan — the output of the workflow compiler.
//!
//! A `RunPlan` is a fully-resolved, ready-to-execute representation of a
//! workflow definition + agent catalog pair. It contains all the information
//! the orchestrator needs to drive a run through the state machine without
//! re-reading the YAML files.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── P066: Toolchain cache policy snapshot types ───────────────────────────────

/// Toolchain scope in frozen snapshots: run or session.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolchainCacheScopeSnapshot {
    Run,
    Session,
}

/// P066: Toolchain cache policy as stored in a frozen ResolvedAgent snapshot.
/// Field names and shape match the YAML catalog block exactly.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolchainCachePolicySnapshot {
    pub version: u32,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xcode_scope: Option<ToolchainCacheScopeSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub go_scope: Option<ToolchainCacheScopeSnapshot>,
}

impl ToolchainCachePolicySnapshot {
    /// Effective xcode scope: explicit value, or `run` when enabled and absent.
    pub fn effective_xcode_scope(&self) -> Option<ToolchainCacheScopeSnapshot> {
        if !self.enabled {
            return None;
        }
        Some(self.xcode_scope.unwrap_or(ToolchainCacheScopeSnapshot::Run))
    }

    /// Effective go scope: explicit value, or `session` when enabled and absent.
    pub fn effective_go_scope(&self) -> Option<ToolchainCacheScopeSnapshot> {
        if !self.enabled {
            return None;
        }
        Some(
            self.go_scope
                .unwrap_or(ToolchainCacheScopeSnapshot::Session),
        )
    }
}

/// A fully compiled run plan ready for execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunPlan {
    pub initial_state: String,
    pub states: HashMap<String, CompiledState>,
    pub variables: HashMap<String, serde_json::Value>,
    /// Artifact name → file path template from the agent catalog's `artifacts:` section.
    /// Used by `exists('artifact_name')` transition conditions to check if an artifact
    /// has been produced on the filesystem.
    pub artifact_paths: HashMap<String, String>,
    /// Frozen workflow cohort family from parsed workflow metadata.
    pub workflow_family: Option<String>,
    /// Frozen workflow risk class from parsed workflow metadata.
    pub risk_class: Option<String>,
    /// Frozen stack identifier from parsed workflow metadata.
    pub stack: Option<String>,
    /// Frozen P053 policy for compatibility-only broad discovery.
    #[serde(default)]
    pub legacy_broad_discovery_policy: LegacyBroadDiscoveryPolicy,
    /// SHA-256 over canonical parsed workflow snapshot JSON.
    pub workflow_snapshot_hash: String,
    /// SHA-256 over canonical parsed agent-catalog snapshot JSON.
    pub catalog_snapshot_hash: String,
    /// Canonical parsed workflow snapshot JSON.
    pub workflow_snapshot_json: String,
    /// Canonical parsed agent-catalog snapshot JSON.
    pub catalog_snapshot_json: String,
    /// P060: Dynamic candidate bindings for proposal review routing.
    /// Compiled from catalog entries with `routing` metadata.
    #[serde(default)]
    pub dynamic_candidate_bindings: Vec<domain::routing::CompiledDynamicAgentBinding>,
    /// P066: Frozen-snapshot format version. Set to 1 when any resolved agent
    /// carries a toolchain_cache_policy. Absent on pre-P066 (legacy_v0) snapshots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_plan_snapshot_format_version: Option<u32>,
    /// P077: Closeout readiness enforcement mode frozen from workflow metadata at compile time.
    /// Accepted values: "advisory" | "enforcement". Absent means advisory (legacy fallback).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closeout_readiness_mode: Option<String>,
    /// P058: Frozen escalation policies compiled from the agent catalog at run start.
    /// Absent for pre-P058 runs and catalogs without escalation_policies declarations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub escalation_policies: Vec<EscalationPolicySnapshot>,
}

// ── P058: Escalation policy snapshot types ────────────────────────────────────

/// A single tier as frozen into a RunPlan escalation policy snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EscalationTierSnapshot {
    pub tier_id: String,
    /// Raw tier kind string: same_backend_retry | backend_profile | lead_mediation | pause
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend_profile_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_attempts: Option<u32>,
}

/// A frozen escalation policy as stored in a RunPlan snapshot.
///
/// Captured at run compile time from the agent catalog's `escalation_policies:` section.
/// `policy_hash` is the SHA-256 of the canonical JSON of the source `EscalationPolicyYaml`,
/// enabling drift detection on resume.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationPolicySnapshot {
    pub policy_id: String,
    pub schema_version: String,
    pub enabled_default: bool,
    pub applies_to_agent_id: Option<String>,
    pub applies_to_backend_profile_id: Option<String>,
    pub applies_to_stage_id: Option<String>,
    pub max_chain_attempts: u32,
    pub max_chain_wall_clock_seconds: u64,
    /// Raw trigger strings — forward-compatible with future values.
    pub triggers: Vec<String>,
    pub tiers: Vec<EscalationTierSnapshot>,
    /// SHA-256 of the canonical JSON of the source policy at compile time.
    pub policy_hash: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyBroadDiscoveryPolicy {
    #[default]
    Disabled,
    WorkflowOptIn,
}

/// A single compiled state in the workflow state machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledState {
    pub id: String,
    pub label: String,
    pub state_type: Option<String>,
    pub owner: ResolvedAgent,
    pub is_manual_gate: bool,
    pub is_end: bool,
    pub tasks: Vec<CompiledTask>,
    /// Tasks to execute after approval is granted (manual_release gates).
    pub post_approval_tasks: Vec<CompiledTask>,
    pub transitions: Vec<CompiledTransition>,
    pub loop_config: Option<CompiledLoop>,
    #[serde(default)]
    pub degraded_output_policy: DegradedOutputPolicy,
    /// P060: Dynamic parallel configuration (materializes from selector artifact).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic_parallel: Option<CompiledDynamicParallel>,
    /// P060: System task configuration (no provider invocation).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_task: Option<CompiledSystemTask>,
}

/// P060: Compiled dynamic_parallel block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledDynamicParallel {
    pub selector_artifact: String,
    pub output_contract: String,
    pub inputs: Vec<String>,
}

/// P060: Compiled system task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledSystemTask {
    pub task_type: String,
    pub executor_mode: String,
}

/// A resolved agent binding: agent ID → provider + model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedAgent {
    pub agent_id: String,
    pub backend_profile_id: Option<String>,
    /// Bare ACP provider name: "claude", "codex", "gemini", "auggie", "junie".
    pub provider: String,
    pub model: Option<String>,
    /// Effort level from backend_profile (e.g. "high", "medium", "low").
    pub effort: Option<String>,
    pub max_turns: Option<u32>,
    pub temperature: Option<f64>,
    /// System prompt from the agent catalog (agents[].prompt).
    pub prompt: Option<String>,
    pub permission_profile: Option<String>,
    pub skill_ref: Option<String>,
    pub skill_role: Option<String>,
    pub skill_snapshot_hash: Option<String>,
    #[serde(default)]
    pub requested_mcp_server_ids: Vec<String>,
    /// Resolved skill content for prompt injection.
    /// Populated during compilation when `skill_ref` is set on the agent.
    pub resolved_skill: Option<ResolvedSkill>,
    /// Explicit output contract requested by the agent catalog entry.
    pub output_contract: Option<String>,
    /// Whether this agent has write access to a worktree (Proposal 007).
    /// Derived from `worktree_policy.write_enabled` in the agent catalog.
    #[serde(default)]
    pub worktree_write_enabled: bool,
    /// Worktree strategy from the catalog: "dedicated", "meta_only",
    /// "shared_implementation_worktree". `None` = no worktree policy.
    pub worktree_strategy: Option<String>,
    /// Session reuse scope declared in the agent catalog.
    pub session_reuse_scope: Option<String>,
    /// Session family ID declared in the agent catalog.
    pub session_family_id: Option<String>,
    /// P051: whether Xcode MCP must use the brokered HTTP path.
    #[serde(default)]
    pub xcode_broker_required: bool,
    /// P051: whether PATH shim injection is expected for the agent.
    #[serde(default)]
    pub xcode_shim_injection_signal: bool,
    /// P051: whether direct Xcode commands must route through host execution.
    #[serde(default)]
    pub requires_xcode_host_execution: bool,
    /// P051: compile-time advisory warnings carried to execution-time observation.
    #[serde(default)]
    pub xcode_prompt_lint_warnings: Vec<String>,
    /// P066: Frozen toolchain cache policy for this agent. None = policy_absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toolchain_cache_policy: Option<ToolchainCachePolicySnapshot>,
}

/// A resolved skill, ready for prompt injection.
///
/// Matches Swift `ResolvedSkill`. The `injected_content` field is the final
/// prompt fragment including the `## Skill: ...` header and role specialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedSkill {
    /// Skill ID from the catalog (e.g. "proposal_review_triad").
    pub id: String,
    /// Skill type: "external", "inline", "builtin".
    pub skill_type: String,
    /// Final prompt fragment for injection. Includes the header and role block.
    pub injected_content: String,
    /// The role requested by the agent (e.g. "product_owner").
    pub role: Option<String>,
}

/// A compiled agent task invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledTask {
    pub agent: ResolvedAgent,
    pub task_name: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub output_policies: HashMap<String, OutputPolicy>,
    /// Resolved output schemas keyed by output artifact name.
    /// Populated from an explicit `output_contract` for single-output agents
    /// or matching multi-output aliases, then by exact normalized/raw artifact
    /// names and version stem fallbacks. Consumed by the prompt builder to
    /// embed required field lists and contract metadata in task directives.
    #[serde(default)]
    pub output_schemas: HashMap<String, OutputSchema>,
    /// Whether this task runs in parallel with siblings.
    pub parallel: bool,
    /// Phase within the run block. Parallel tasks are phase 0; `then`
    /// (sequential-after-parallel) tasks are phase 1. Phase 1 tasks are
    /// only enqueued after all phase 0 tasks complete.
    #[serde(default)]
    pub phase: u32,
    /// P060: selected_outputs_from declaration for aggregation tasks.
    /// When present, the orchestrator resolves selected reviewer artifacts
    /// and injects them into the task prompt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_outputs_from: Option<CompiledSelectedOutputsFrom>,
}

/// P060: Compiled selected_outputs_from configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledSelectedOutputsFrom {
    pub source_plan: String,
    pub output_contract: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputReusePolicy {
    MustProduce,
    AllowUnchangedExisting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputPolicy {
    pub reuse_policy: OutputReusePolicy,
}

/// Resolved output schema from a catalog contract.
/// Flows into the prompt builder so agents receive a "must produce JSON
/// with these fields" directive alongside the task description.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputSchema {
    pub contract_id: String,
    pub format: String,
    pub human_format: Option<String>,
    pub machine_format: Option<String>,
    pub validation_mode: Option<String>,
    pub normalized_artifact_name: Option<String>,
    pub raw_artifact_name: Option<String>,
    pub required_fields: Vec<String>,
}

/// A compiled transition with its condition string.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledTransition {
    pub to: String,
    pub condition: String,
}

/// A compiled loop configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledLoop {
    pub counter: String,
    pub max: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct DegradedOutputPolicy {
    pub mode: String,
    pub contracts: Vec<String>,
    pub failure_kinds: Vec<String>,
    pub max_settlement: String,
}

impl Default for DegradedOutputPolicy {
    fn default() -> Self {
        Self {
            mode: "deny".to_string(),
            contracts: Vec::new(),
            failure_kinds: Vec::new(),
            max_settlement: "valid_outputs_from_failed_execution".to_string(),
        }
    }
}
