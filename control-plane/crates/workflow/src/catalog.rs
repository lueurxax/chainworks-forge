//! Serde types for agent catalog YAML files.
//!
//! These types mirror the schema in `examples/agents/agents.yaml` and the Swift
//! `AgentCatalog.swift` structs. Only the subset needed for agent→provider
//! resolution is modeled.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Root of an agent catalog YAML file.
#[derive(Debug, Deserialize, Serialize)]
pub struct AgentCatalogFile {
    pub schema_version: Option<u32>,
    pub app: Option<serde_yaml::Value>,
    pub paths: Option<HashMap<String, String>>,
    pub artifacts: Option<HashMap<String, String>>,
    pub skills: Option<HashMap<String, SkillDef>>,
    pub contracts: Option<HashMap<String, ContractDef>>,
    pub runtime_profiles: Option<HashMap<String, RuntimeProfile>>,
    pub backend_profiles: Option<HashMap<String, BackendProfile>>,
    pub permission_profiles: Option<serde_yaml::Value>,
    pub agents: Option<Vec<AgentEntry>>,
}

/// A skill definition from the catalog's `skills:` section.
///
/// Matches Swift `SkillRef` — three types:
/// - `external_skill`: disk bundle with `SKILL.md` at `path`
/// - `inline_skill`: uses `description` directly
/// - `builtin_agent`: looked up by `name` in a hardcoded registry
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SkillDef {
    /// Skill type: `"external_skill"` | `"inline_skill"` | `"builtin_agent"`
    #[serde(rename = "type")]
    pub skill_type: String,
    /// Path to external skill bundle dir (relative to catalog YAML).
    pub path: Option<String>,
    /// Builtin skill name (e.g. `"docs-quality-guardian"`).
    pub name: Option<String>,
    /// Inline skill description (raw prompt text).
    pub description: Option<String>,
    /// Informational notes (not used at runtime).
    pub notes: Option<String>,
}

/// An output contract definition from the catalog's `contracts:` section.
/// Defines the schema an agent's structured output must conform to.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ContractDef {
    pub format: Option<String>,
    pub human_format: Option<String>,
    pub machine_format: Option<String>,
    pub validation_mode: Option<String>,
    /// Stable artifact name — when an output artifact matches this,
    /// the contract applies. Example: `proposal_review_summary`.
    pub normalized_artifact_name: Option<String>,
    pub raw_artifact_name: Option<String>,
    #[serde(default)]
    pub required_fields: Vec<String>,
}

/// A backend profile defining which ACP provider and model to use.
#[derive(Debug, Deserialize, Serialize)]
pub struct BackendProfile {
    pub provider: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub temperature: Option<f64>,
    pub max_turns: Option<u32>,
    pub structured_output: Option<String>,
    pub mcp: Option<Vec<String>>,
    pub runtime_profile: Option<String>,
}

/// A runtime profile defining ACP transport capabilities.
#[derive(Debug, Deserialize, Serialize)]
pub struct RuntimeProfile {
    pub capability_class: Option<String>,
    pub adapter_family: Option<String>,
    pub transport_kind: Option<String>,
    pub mcp_realization_path: Option<String>,
    pub requires: Option<Vec<String>>,
}

/// A worktree policy from the catalog's agent `worktree_policy:` section.
/// Matches Swift `WorktreePolicy` — determines how an agent interacts with
/// the filesystem during implementation stages.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorktreePolicy {
    /// Strategy: `"dedicated"` | `"meta_only"` | `"shared_implementation_worktree"`
    pub strategy: String,
    /// Path template for the worktree (e.g. `${CHAINWORKS_IMPLEMENTATION_WORKTREE:-.chainworks/worktrees/implementation}`)
    pub path: Option<String>,
    /// Base branch to create the worktree from (e.g. `${CHAINWORKS_BASE_BRANCH:-main}`)
    #[serde(default)]
    pub base_branch: Option<String>,
    /// Whether the agent has write access.
    #[serde(default)]
    pub write_enabled: bool,
}

/// An agent definition in the catalog.
#[derive(Debug, Deserialize, Serialize)]
pub struct AgentEntry {
    pub id: String,
    pub title: Option<String>,
    pub mode: Option<String>,
    pub backend_profile: String,
    pub permission_profile: Option<String>,
    pub skill_ref: Option<String>,
    pub skill_role: Option<String>,
    pub session_reuse_scope: Option<String>,
    pub session_family_id: Option<String>,
    pub inputs: Option<Vec<String>>,
    pub outputs: Option<Vec<String>>,
    pub output_contract: Option<String>,
    pub requires_human_approval: Option<bool>,
    pub prompt: Option<String>,
    pub notes: Option<String>,
    pub worktree_policy: Option<WorktreePolicy>,
    pub required_tools: Option<Vec<String>>,
}

/// Load and parse an agent catalog YAML file.
pub fn load(path: &str) -> anyhow::Result<AgentCatalogFile> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Failed to read agent catalog YAML at '{}': {}", path, e))?;
    let file: AgentCatalogFile = serde_yaml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse agent catalog YAML at '{}': {}", path, e))?;
    Ok(file)
}
