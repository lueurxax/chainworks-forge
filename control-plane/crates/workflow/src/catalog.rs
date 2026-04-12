//! Serde types for agent catalog YAML files.
//!
//! These types mirror the schema in `examples/agents/agents.yaml` and the Swift
//! `AgentCatalog.swift` structs. Only the subset needed for agent→provider
//! resolution is modeled.

use std::collections::HashMap;
use serde::Deserialize;

/// Root of an agent catalog YAML file.
#[derive(Debug, Deserialize)]
pub struct AgentCatalogFile {
    pub schema_version: Option<u32>,
    pub app: Option<serde_yaml::Value>,
    pub paths: Option<HashMap<String, String>>,
    pub artifacts: Option<HashMap<String, String>>,
    pub skills: Option<serde_yaml::Value>,
    pub contracts: Option<serde_yaml::Value>,
    pub runtime_profiles: Option<HashMap<String, RuntimeProfile>>,
    pub backend_profiles: Option<HashMap<String, BackendProfile>>,
    pub permission_profiles: Option<serde_yaml::Value>,
    pub agents: Option<Vec<AgentEntry>>,
}

/// A backend profile defining which ACP provider and model to use.
#[derive(Debug, Deserialize)]
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
#[derive(Debug, Deserialize)]
pub struct RuntimeProfile {
    pub capability_class: Option<String>,
    pub adapter_family: Option<String>,
    pub transport_kind: Option<String>,
    pub mcp_realization_path: Option<String>,
    pub requires: Option<Vec<String>>,
}

/// An agent definition in the catalog.
#[derive(Debug, Deserialize)]
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
    pub worktree_policy: Option<serde_yaml::Value>,
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
