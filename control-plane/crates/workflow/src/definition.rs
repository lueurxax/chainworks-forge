//! Serde types for workflow YAML definitions.
//!
//! These types mirror the schema in `examples/workflows/*.yaml` and the Swift
//! `WorkflowDefinition.swift` structs. Only the subset needed for run-plan
//! compilation is modeled; presentation-only fields are ignored via
//! `deny_unknown_fields` being OFF.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Root of a workflow YAML file.
#[derive(Debug, Deserialize, Serialize)]
pub struct WorkflowFile {
    pub schema_version: Option<u32>,
    pub workflow: Option<WorkflowMeta>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovery: Option<DiscoveryDef>,
    pub variables: Option<HashMap<String, serde_yaml::Value>>,
    pub initial_state: String,
    pub states: HashMap<String, WorkflowState>,
    /// Scoring config (not used for compilation, but preserved for later).
    pub scoring: Option<serde_yaml::Value>,
    pub failure_policy: Option<serde_yaml::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WorkflowMeta {
    pub id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub family: Option<String>,
    pub risk_class: Option<String>,
    pub stack: Option<String>,
    pub uses_agent_catalog: Option<String>,
    pub required_providers: Option<Vec<String>>,
    pub execution: Option<serde_yaml::Value>,
    pub idea_input: Option<serde_yaml::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DiscoveryDef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_broad_discovery_policy: Option<LegacyBroadDiscoveryPolicyDef>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LegacyBroadDiscoveryPolicyDef {
    Disabled,
    WorkflowOptIn,
}

/// A single state in the workflow state machine.
#[derive(Debug, Deserialize, Serialize)]
pub struct WorkflowState {
    pub label: String,
    /// "start", "end", "manual_gate", or absent (regular compute state).
    #[serde(rename = "type")]
    pub state_type: Option<String>,
    /// Agent ID that owns this state's execution.
    pub owner: String,
    /// "required" for manual gates.
    pub approval: Option<String>,
    pub approval_policy: Option<String>,
    /// Execution block: sequence / parallel / then.
    pub run: Option<RunBlock>,
    /// Execution after approval is granted (used by manual_release gates).
    pub run_after_approval: Option<RunBlock>,
    /// Loop configuration for revision cycles.
    #[serde(rename = "loop")]
    pub loop_config: Option<LoopConfig>,
    /// Transition rules evaluated after the state completes.
    pub transitions: Option<Vec<Transition>>,
    /// P057: policy for whether valid contract outputs from failed executions
    /// can satisfy transition truth. Missing means default deny.
    pub degraded_output_policy: Option<DegradedOutputPolicyDef>,
}

impl WorkflowState {
    /// Whether this state is a manual gate (requires human approval).
    pub fn is_manual_gate(&self) -> bool {
        self.state_type.as_deref() == Some("manual_gate")
            || self.approval.as_deref() == Some("required")
    }

    /// Whether this state is an end state.
    pub fn is_end(&self) -> bool {
        self.state_type.as_deref() == Some("end")
    }

    /// Whether this state is a start state.
    pub fn is_start(&self) -> bool {
        self.state_type.as_deref() == Some("start")
    }
}

/// The execution block within a state: sequential, parallel, or fan-out/fan-in.
#[derive(Debug, Deserialize, Serialize)]
pub struct RunBlock {
    pub sequence: Option<Vec<AgentTask>>,
    pub parallel: Option<Vec<AgentTask>>,
    /// Tasks to run sequentially after parallel tasks complete.
    pub then: Option<Vec<AgentTask>>,
}

/// A single agent invocation within a run block.
#[derive(Debug, Deserialize, Serialize)]
pub struct AgentTask {
    pub agent: String,
    pub task: String,
    pub inputs: Option<Vec<String>>,
    pub outputs: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_policies: Option<HashMap<String, OutputPolicyDef>>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct OutputPolicyDef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reuse_policy: Option<OutputReusePolicyDef>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutputReusePolicyDef {
    MustProduce,
    AllowUnchangedExisting,
}

/// A transition to another state, guarded by a condition.
#[derive(Debug, Deserialize, Serialize)]
pub struct Transition {
    pub to: String,
    /// Condition expression: `"true"`, `exists('artifact')`, `field == value`, etc.
    pub when: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DegradedOutputPolicyDef {
    pub mode: String,
    pub contracts: Option<Vec<String>>,
    pub failure_kinds: Option<Vec<String>>,
    pub max_settlement: Option<String>,
}

/// Loop configuration for revision cycles.
#[derive(Debug, Deserialize, Serialize)]
pub struct LoopConfig {
    /// Name of the counter variable (e.g. `proposal_revision_count`).
    pub counter: String,
    /// Maximum number of iterations; may reference `vars.max_*`.
    pub max: serde_yaml::Value,
}

/// Load and parse a workflow YAML file.
pub fn load(path: &str) -> anyhow::Result<WorkflowFile> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Failed to read workflow YAML at '{}': {}", path, e))?;
    let file: WorkflowFile = serde_yaml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse workflow YAML at '{}': {}", path, e))?;
    Ok(file)
}
