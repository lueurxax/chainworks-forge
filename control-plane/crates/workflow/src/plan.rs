//! Compiled run plan — the output of the workflow compiler.
//!
//! A `RunPlan` is a fully-resolved, ready-to-execute representation of a
//! workflow definition + agent catalog pair. It contains all the information
//! the orchestrator needs to drive a run through the state machine without
//! re-reading the YAML files.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    /// SHA-256 over canonical parsed workflow snapshot JSON.
    pub workflow_snapshot_hash: String,
    /// SHA-256 over canonical parsed agent-catalog snapshot JSON.
    pub catalog_snapshot_hash: String,
    /// Canonical parsed workflow snapshot JSON.
    pub workflow_snapshot_json: String,
    /// Canonical parsed agent-catalog snapshot JSON.
    pub catalog_snapshot_json: String,
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
    /// Resolved output schemas keyed by output artifact name.
    /// Populated from the agent's explicit `output_contract` first, then by
    /// exact normalized/raw artifact-name matches, and finally by version
    /// stem fallbacks. Consumed by the prompt builder to embed required field
    /// lists and contract metadata in task directives.
    #[serde(default)]
    pub output_schemas: HashMap<String, OutputSchema>,
    /// Whether this task runs in parallel with siblings.
    pub parallel: bool,
    /// Phase within the run block. Parallel tasks are phase 0; `then`
    /// (sequential-after-parallel) tasks are phase 1. Phase 1 tasks are
    /// only enqueued after all phase 0 tasks complete.
    #[serde(default)]
    pub phase: u32,
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
