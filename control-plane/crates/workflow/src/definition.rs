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
    /// P077: closeout readiness enforcement mode for this workflow.
    /// Accepted values: "advisory" | "enforcement". Absent means advisory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closeout_readiness_mode: Option<String>,
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
    /// P060: Dynamic parallel — materializes selected reviewers from a typed
    /// selector artifact (AgentSelectionPlanV1). Additive sibling to
    /// sequence/parallel/then.
    #[serde(default)]
    pub dynamic_parallel: Option<DynamicParallelDef>,
    /// P060: System task — executed in-process without a provider.
    /// Used for system.routing (proposal_review_router).
    #[serde(default)]
    pub system_task: Option<SystemTaskDef>,
}

/// P060: Definition for a dynamic_parallel block.
#[derive(Debug, Deserialize, Serialize)]
pub struct DynamicParallelDef {
    /// The selector artifact to read (e.g. "agent_selection_plan_v1").
    pub selector_artifact: String,
    /// The output contract each materialized reviewer must produce.
    pub output_contract: String,
    /// Inputs passed to each materialized reviewer.
    #[serde(default)]
    pub inputs: Vec<String>,
}

/// P060: Definition for a system task block.
#[derive(Debug, Deserialize, Serialize)]
pub struct SystemTaskDef {
    /// Task type identifier (e.g. "proposal_review_router").
    pub task_type: String,
    /// Executor mode (e.g. "system.routing").
    pub executor_mode: String,
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
    /// P060: selected_outputs_from declaration for aggregation tasks.
    /// When present, the task receives only artifacts from dynamically
    /// selected reviewers rather than all artifacts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_outputs_from: Option<SelectedOutputsFromDef>,
}

/// P060: Declaration for selected_outputs_from on aggregation tasks.
#[derive(Debug, Deserialize, Serialize)]
pub struct SelectedOutputsFromDef {
    /// The selector plan type (e.g. "agent_selection_plan_v1").
    pub source_plan: String,
    /// The output contract to filter by (e.g. "proposal_review_v1").
    pub output_contract: String,
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
