//! Compiled run plan — the output of the workflow compiler.
//!
//! A `RunPlan` is a fully-resolved, ready-to-execute representation of a
//! workflow definition + agent catalog pair. It contains all the information
//! the orchestrator needs to drive a run through the state machine without
//! re-reading the YAML files.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// A fully compiled run plan ready for execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunPlan {
    pub initial_state: String,
    pub states: HashMap<String, CompiledState>,
    pub variables: HashMap<String, serde_json::Value>,
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
}

/// A resolved agent binding: agent ID → provider + model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedAgent {
    pub agent_id: String,
    /// Bare ACP provider name: "claude", "codex", "gemini", "auggie", "junie".
    pub provider: String,
    pub model: Option<String>,
}

/// A compiled agent task invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledTask {
    pub agent: ResolvedAgent,
    pub task_name: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    /// Whether this task runs in parallel with siblings.
    pub parallel: bool,
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
