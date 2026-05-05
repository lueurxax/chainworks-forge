//! Serde types for agent catalog YAML files.
//!
//! These types mirror the schema in `examples/agents/agents.yaml` and the Swift
//! `AgentCatalog.swift` structs. Only the subset needed for agent→provider
//! resolution is modeled.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── P066: Toolchain cache policy ──────────────────────────────────────────────

/// Toolchain scope for xcode or go families.
/// Only `run` and `session` are valid; unknown values fail YAML compilation.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolchainCacheScope {
    Run,
    Session,
}

/// P066: Per-agent toolchain cache mapping policy from agents[].toolchain_cache_policy.
///
/// `deny_unknown_fields` ensures unknown keys fail compilation rather than being
/// silently dropped.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainCachePolicyYaml {
    /// Format version — must be 1. Required field.
    pub version: u32,
    /// Whether toolchain cache mapping is enabled.
    pub enabled: bool,
    /// Xcode cache scope: run | session. Defaults to run when enabled.
    pub xcode_scope: Option<ToolchainCacheScope>,
    /// Go cache scope: run | session. Defaults to session when enabled.
    pub go_scope: Option<ToolchainCacheScope>,
}

impl ToolchainCachePolicyYaml {
    /// Validate that `version` is exactly 1 (the only supported value).
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.version != 1 {
            anyhow::bail!(
                "toolchain_cache_policy.version={} is not supported; only version 1 is valid",
                self.version
            );
        }
        Ok(())
    }
}

/// P066: Format version written into catalog snapshots that contain
/// toolchain_cache_policy entries. Value 1 is the only supported version.
/// Pre-P066 snapshots that omit this field and have no toolchain_cache_policy
/// entries decode as legacy_v0 (policy_absent). Snapshots that contain
/// toolchain_cache_policy but omit the version, or carry an unsupported version,
/// must be rejected as frozen_snapshot_contract_incompatible.
pub const CATALOG_SNAPSHOT_FORMAT_VERSION: u32 = 1;

/// Root of an agent catalog YAML file.
#[derive(Debug, Deserialize, Serialize)]
pub struct AgentCatalogFile {
    pub schema_version: Option<u32>,
    /// P066: Frozen-snapshot compatibility gate for toolchain_cache_policy.
    /// Set to 1 when any agent entry declares toolchain_cache_policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_snapshot_format_version: Option<u32>,
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

/// System-level role attached to an agent catalog entry.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SystemRole {
    Lead,
}

/// An agent definition in the catalog.
#[derive(Debug, Deserialize, Serialize)]
pub struct AgentEntry {
    pub id: String,
    pub title: Option<String>,
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_role: Option<SystemRole>,
    pub backend_profile: String,
    pub permission_profile: Option<String>,
    pub skill_ref: Option<String>,
    pub skill_role: Option<String>,
    pub session_reuse_scope: Option<String>,
    pub session_family_id: Option<String>,
    pub inputs: Option<Vec<String>>,
    pub outputs: Option<Vec<String>>,
    pub output_contract: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lead_resolution_contract: Option<String>,
    pub requires_human_approval: Option<bool>,
    pub prompt: Option<String>,
    pub notes: Option<String>,
    pub worktree_policy: Option<WorktreePolicy>,
    pub required_tools: Option<Vec<String>>,
    #[serde(default)]
    pub xcode_broker_required: Option<bool>,
    #[serde(default)]
    pub xcode_shim_injection_signal: Option<bool>,
    #[serde(default)]
    pub requires_xcode_host_execution: Option<bool>,
    /// P060: Routing metadata for deterministic reviewer selection.
    #[serde(default)]
    pub routing: Option<RoutingMetadataYaml>,
    /// P066: Toolchain cache mapping policy. Absent = policy_absent (disabled).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toolchain_cache_policy: Option<ToolchainCachePolicyYaml>,
}

/// P060: Routing metadata block on an agent catalog entry.
/// Tags are lowercase ASCII kebab-case.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RoutingMetadataYaml {
    pub routing_id: String,
    pub family: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub stacks: Vec<String>,
    #[serde(default)]
    pub surfaces: Vec<String>,
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub enabled_for_proposal_review: bool,
    pub rollout_wave: Option<String>,
    #[serde(default)]
    pub mandatory_when: Vec<String>,
    #[serde(default)]
    pub usually_pair_with: Vec<String>,
    #[serde(default)]
    pub close_alternatives: Vec<String>,
    #[serde(default)]
    pub strong_proposal_keywords: Vec<String>,
    #[serde(default)]
    pub strong_repo_files: Vec<String>,
    #[serde(default)]
    pub strong_repo_symbols: Vec<String>,
    #[serde(default)]
    pub score_weights: domain::routing::ScoreWeights,
}

/// Validate that an executable catalog has exactly one system lead with the
/// provider, permission, and LeadResolutionContract coverage required for P017
/// Phase B/C mediation.
pub fn validate_catalog_has_exactly_one_system_lead(
    catalog: &AgentCatalogFile,
) -> anyhow::Result<&AgentEntry> {
    let agents = catalog.agents.as_deref().unwrap_or(&[]);
    let leads: Vec<&AgentEntry> = agents
        .iter()
        .filter(|agent| agent.system_role == Some(SystemRole::Lead))
        .collect();

    match leads.as_slice() {
        [lead] => {
            validate_system_lead_runtime_coverage(catalog, lead)?;
            Ok(*lead)
        }
        [] => anyhow::bail!(
            "lead_missing: executable agent catalog must declare exactly one agents[].system_role=lead"
        ),
        _ => {
            let ids = leads
                .iter()
                .map(|agent| agent.id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            anyhow::bail!(
                "lead_ambiguous: executable agent catalog declares multiple system leads: {ids}"
            )
        }
    }
}

fn validate_system_lead_runtime_coverage(
    catalog: &AgentCatalogFile,
    lead: &AgentEntry,
) -> anyhow::Result<()> {
    let backend_profiles = catalog.backend_profiles.as_ref().ok_or_else(|| {
        anyhow::anyhow!("lead_backend_profile_missing: backend_profiles is empty")
    })?;
    if !backend_profiles.contains_key(&lead.backend_profile) {
        anyhow::bail!(
            "lead_backend_profile_missing: system lead '{}' references unknown backend_profile '{}'",
            lead.id,
            lead.backend_profile
        );
    }

    let permission_profile = lead.permission_profile.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
            "lead_permission_profile_missing: system lead '{}' must declare permission_profile",
            lead.id
        )
    })?;
    if !permission_profile_exists(catalog.permission_profiles.as_ref(), permission_profile) {
        anyhow::bail!(
            "lead_permission_profile_missing: system lead '{}' references unknown permission_profile '{}'",
            lead.id,
            permission_profile
        );
    }

    let contract_id = lead.lead_resolution_contract.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
            "lead_resolution_contract_missing: system lead '{}' must declare lead_resolution_contract",
            lead.id
        )
    })?;
    let contracts = catalog
        .contracts
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("lead_resolution_contract_missing: contracts is empty"))?;
    if !contracts.contains_key(contract_id) {
        anyhow::bail!(
            "lead_resolution_contract_missing: system lead '{}' references unknown lead_resolution_contract '{}'",
            lead.id,
            contract_id
        );
    }

    Ok(())
}

fn permission_profile_exists(permission_profiles: Option<&serde_yaml::Value>, name: &str) -> bool {
    let Some(serde_yaml::Value::Mapping(mapping)) = permission_profiles else {
        return false;
    };
    mapping.contains_key(serde_yaml::Value::String(name.to_string()))
}

/// P066: Validate frozen-snapshot format version compatibility for a catalog snapshot.
///
/// Rules:
/// - Absent version + no toolchain_cache_policy entries → legacy_v0, returns Ok(false) (policy_absent).
/// - Absent version + any toolchain_cache_policy present → incompatible, returns Err.
/// - Version present but unsupported (> 1) → incompatible, returns Err.
/// - Version = 1 → Ok(true) (P066-aware snapshot).
///
/// Returns `Ok(true)` for P066-aware snapshots, `Ok(false)` for legacy_v0.
pub fn validate_catalog_snapshot_format_version(
    catalog: &AgentCatalogFile,
) -> anyhow::Result<bool> {
    let has_policy = catalog
        .agents
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .any(|a| a.toolchain_cache_policy.is_some());

    match catalog.catalog_snapshot_format_version {
        None if !has_policy => Ok(false),
        None => anyhow::bail!(
            "frozen_snapshot_contract_incompatible: catalog snapshot contains toolchain_cache_policy \
             entries but omits catalog_snapshot_format_version; mixed-version snapshot is not supported"
        ),
        Some(CATALOG_SNAPSHOT_FORMAT_VERSION) => Ok(true),
        Some(v) => anyhow::bail!(
            "frozen_snapshot_contract_incompatible: catalog snapshot requires format version {v} \
             but this reader only supports version {CATALOG_SNAPSHOT_FORMAT_VERSION}"
        ),
    }
}

/// P066: Validate all toolchain_cache_policy entries in the catalog.
pub fn validate_toolchain_cache_policies(catalog: &AgentCatalogFile) -> anyhow::Result<()> {
    for agent in catalog.agents.as_deref().unwrap_or(&[]) {
        if let Some(policy) = &agent.toolchain_cache_policy {
            policy
                .validate()
                .map_err(|e| anyhow::anyhow!("agent '{}': {}", agent.id, e))?;
        }
    }
    Ok(())
}

/// Load and parse an agent catalog YAML file.
pub fn load(path: &str) -> anyhow::Result<AgentCatalogFile> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Failed to read agent catalog YAML at '{}': {}", path, e))?;
    let file: AgentCatalogFile = serde_yaml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse agent catalog YAML at '{}': {}", path, e))?;
    Ok(file)
}
