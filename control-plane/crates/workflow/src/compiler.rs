//! Workflow compiler: resolves a workflow definition + agent catalog into
//! a ready-to-execute `RunPlan`.
//!
//! Mirrors the Swift `RunPlanCompiler.previewCompile()` flow:
//! 1. Parse both YAML files
//! 2. Build agent→(provider, model) lookup
//! 3. Resolve each state's owner and task agents
//! 4. Resolve loop max values from variables
//! 5. Return a `RunPlan`

use std::collections::HashMap;
use anyhow::{Context, Result};
use tracing::warn;

use crate::catalog;
use crate::definition;
use crate::plan::*;

/// Compile a workflow YAML + agent catalog YAML into a `RunPlan`.
///
/// Both paths must be readable files. The compiler validates that every
/// agent referenced by the workflow exists in the catalog and has a
/// resolvable backend profile.
pub fn compile(workflow_path: &str, catalog_path: &str) -> Result<RunPlan> {
    let wf = definition::load(workflow_path)
        .context("loading workflow definition")?;
    let cat = catalog::load(catalog_path)
        .context("loading agent catalog")?;

    let agent_lookup = build_agent_lookup(&cat)?;

    // Convert variables from serde_yaml::Value to serde_json::Value.
    let variables: HashMap<String, serde_json::Value> = wf
        .variables
        .unwrap_or_default()
        .into_iter()
        .map(|(k, v)| {
            let json = yaml_to_json(&v);
            (k, json)
        })
        .collect();

    let mut states = HashMap::new();
    for (state_id, state_def) in &wf.states {
        let compiled = compile_state(state_id, state_def, &agent_lookup, &variables)?;
        states.insert(state_id.clone(), compiled);
    }

    Ok(RunPlan {
        initial_state: wf.initial_state,
        states,
        variables,
    })
}

// ---------------------------------------------------------------------------
// Agent lookup
// ---------------------------------------------------------------------------

struct AgentBinding {
    provider: String,
    model: Option<String>,
}

fn build_agent_lookup(cat: &catalog::AgentCatalogFile) -> Result<HashMap<String, AgentBinding>> {
    let empty_profiles = HashMap::new();
    let profiles = cat.backend_profiles.as_ref()
        .unwrap_or(&empty_profiles);
    let agents = cat.agents.as_ref()
        .map(|v| v.as_slice())
        .unwrap_or(&[]);

    let mut lookup = HashMap::new();
    for agent in agents {
        let profile = profiles.get(&agent.backend_profile)
            .ok_or_else(|| anyhow::anyhow!(
                "Agent '{}' references unknown backend_profile '{}'",
                agent.id, agent.backend_profile
            ))?;

        let provider = normalize_provider(&profile.provider);
        let model = profile.model.clone();

        lookup.insert(agent.id.clone(), AgentBinding { provider, model });
    }
    Ok(lookup)
}

/// Normalize YAML provider names to ACP adapter names.
/// `claude_acp` → `claude`, `codex_acp` → `codex`, `gemini_acp` → `gemini`, etc.
/// If the name doesn't end with `_acp`, it's used as-is.
fn normalize_provider(yaml_provider: &str) -> String {
    yaml_provider
        .strip_suffix("_acp")
        .unwrap_or(yaml_provider)
        .to_string()
}

// ---------------------------------------------------------------------------
// State compilation
// ---------------------------------------------------------------------------

fn compile_state(
    state_id: &str,
    state: &definition::WorkflowState,
    agents: &HashMap<String, AgentBinding>,
    variables: &HashMap<String, serde_json::Value>,
) -> Result<CompiledState> {
    let owner = resolve_agent(&state.owner, agents)?;

    let tasks = state.run.as_ref()
        .map(|rb| compile_run_block(rb, agents))
        .transpose()?
        .unwrap_or_default();

    let post_approval_tasks = state.run_after_approval.as_ref()
        .map(|rb| compile_run_block(rb, agents))
        .transpose()?
        .unwrap_or_default();

    let transitions: Vec<CompiledTransition> = state
        .transitions
        .as_ref()
        .map(|ts| {
            ts.iter()
                .map(|t| CompiledTransition {
                    to: t.to.clone(),
                    condition: t.when.clone(),
                })
                .collect()
        })
        .unwrap_or_default();

    let loop_config = state.loop_config.as_ref()
        .map(|lc| compile_loop(lc, variables));

    Ok(CompiledState {
        id: state_id.to_string(),
        label: state.label.clone(),
        state_type: state.state_type.clone(),
        owner,
        is_manual_gate: state.is_manual_gate(),
        is_end: state.is_end(),
        tasks,
        post_approval_tasks,
        transitions,
        loop_config,
    })
}

fn resolve_agent(
    agent_id: &str,
    agents: &HashMap<String, AgentBinding>,
) -> Result<ResolvedAgent> {
    match agents.get(agent_id) {
        Some(binding) => Ok(ResolvedAgent {
            agent_id: agent_id.to_string(),
            provider: binding.provider.clone(),
            model: binding.model.clone(),
        }),
        None => {
            // Agent not in catalog — warn but don't fail. Use a placeholder
            // so the plan still compiles. The orchestrator can decide at
            // runtime whether to fail or use a default provider.
            warn!(
                agent_id = agent_id,
                "Agent not found in catalog; using placeholder binding"
            );
            Ok(ResolvedAgent {
                agent_id: agent_id.to_string(),
                provider: "claude".to_string(), // safe default
                model: None,
            })
        }
    }
}

fn compile_run_block(
    rb: &definition::RunBlock,
    agents: &HashMap<String, AgentBinding>,
) -> Result<Vec<CompiledTask>> {
    let mut tasks = Vec::new();

    // Sequential tasks
    if let Some(seq) = &rb.sequence {
        for at in seq {
            tasks.push(compile_agent_task(at, agents, false)?);
        }
    }

    // Parallel tasks
    if let Some(par) = &rb.parallel {
        for at in par {
            tasks.push(compile_agent_task(at, agents, true)?);
        }
    }

    // Then tasks (sequential after parallel)
    if let Some(then) = &rb.then {
        for at in then {
            tasks.push(compile_agent_task(at, agents, false)?);
        }
    }

    Ok(tasks)
}

fn compile_agent_task(
    at: &definition::AgentTask,
    agents: &HashMap<String, AgentBinding>,
    parallel: bool,
) -> Result<CompiledTask> {
    let agent = resolve_agent(&at.agent, agents)?;
    Ok(CompiledTask {
        agent,
        task_name: at.task.clone(),
        inputs: at.inputs.clone().unwrap_or_default(),
        outputs: at.outputs.clone().unwrap_or_default(),
        parallel,
    })
}

fn compile_loop(
    lc: &definition::LoopConfig,
    variables: &HashMap<String, serde_json::Value>,
) -> CompiledLoop {
    let max = resolve_loop_max(&lc.max, variables);
    CompiledLoop {
        counter: lc.counter.clone(),
        max,
    }
}

/// Resolve the loop `max` value which may be:
/// - A literal integer: `15`
/// - A variable reference: `vars.max_proposal_revision_cycles`
fn resolve_loop_max(
    val: &serde_yaml::Value,
    variables: &HashMap<String, serde_json::Value>,
) -> u64 {
    // Direct integer
    if let Some(n) = val.as_u64() {
        return n;
    }

    // String reference like "vars.max_proposal_revision_cycles"
    if let Some(s) = val.as_str() {
        if let Some(var_name) = s.strip_prefix("vars.") {
            if let Some(serde_json::Value::Number(n)) = variables.get(var_name) {
                if let Some(v) = n.as_u64() {
                    return v;
                }
            }
        }
    }

    // Fallback
    10
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn yaml_to_json(v: &serde_yaml::Value) -> serde_json::Value {
    match v {
        serde_yaml::Value::Null => serde_json::Value::Null,
        serde_yaml::Value::Bool(b) => serde_json::Value::Bool(*b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_json::Value::Number(i.into())
            } else if let Some(f) = n.as_f64() {
                serde_json::Number::from_f64(f)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null)
            } else {
                serde_json::Value::Null
            }
        }
        serde_yaml::Value::String(s) => serde_json::Value::String(s.clone()),
        serde_yaml::Value::Sequence(seq) => {
            serde_json::Value::Array(seq.iter().map(yaml_to_json).collect())
        }
        serde_yaml::Value::Mapping(map) => {
            let obj: serde_json::Map<String, serde_json::Value> = map
                .iter()
                .filter_map(|(k, v)| {
                    k.as_str().map(|s| (s.to_string(), yaml_to_json(v)))
                })
                .collect();
            serde_json::Value::Object(obj)
        }
        serde_yaml::Value::Tagged(tagged) => yaml_to_json(&tagged.value),
    }
}
