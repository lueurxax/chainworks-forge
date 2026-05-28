//! Workflow compiler: resolves a workflow definition + agent catalog into
//! a ready-to-execute `RunPlan`.
//!
//! Mirrors the Swift `RunPlanCompiler.previewCompile()` flow:
//! 1. Parse both YAML files
//! 2. Build agent→(provider, model) lookup
//! 3. Resolve each state's owner and task agents
//! 4. Resolve loop max values from variables
//! 5. Return a `RunPlan`

use anyhow::{Context, Result};
use domain::provider::ProviderFamily;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::thread;
use tracing::{info, warn};

use crate::catalog;
use crate::definition;
use crate::direct_command::DirectCommandScan;
use crate::plan::*;

const WORKFLOW_COMPILE_STACK_BYTES: usize = 16 * 1024 * 1024;

/// Compile a workflow YAML + agent catalog YAML into a `RunPlan`.
///
/// Both paths must be readable files. The compiler validates that every
/// agent referenced by the workflow exists in the catalog and has a
/// resolvable backend profile.
pub fn compile(workflow_path: &str, catalog_path: &str) -> Result<RunPlan> {
    let workflow_path = workflow_path.to_string();
    let catalog_path = catalog_path.to_string();
    compile_on_dedicated_stack(move || compile_on_current_thread(&workflow_path, &catalog_path))
}

fn compile_on_current_thread(workflow_path: &str, catalog_path: &str) -> Result<RunPlan> {
    let wf = definition::load(workflow_path).context("loading workflow definition")?;
    let cat = catalog::load(catalog_path).context("loading agent catalog")?;
    let workflow_raw =
        load_raw_yaml_value(workflow_path).context("loading raw workflow YAML for P051 lint")?;
    let catalog_raw =
        load_raw_yaml_value(catalog_path).context("loading raw catalog YAML for P051 lint")?;
    let catalog_base = Path::new(catalog_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    compile_loaded(wf, cat, workflow_raw, catalog_raw, &catalog_base, None)
}

/// Compile a run plan from the immutable workflow/catalog snapshots captured
/// when the run was created.
pub fn compile_from_snapshot_json(
    workflow_snapshot_json: &str,
    catalog_snapshot_json: &str,
    catalog_path: &str,
) -> Result<RunPlan> {
    let workflow_snapshot_json = workflow_snapshot_json.to_string();
    let catalog_snapshot_json = catalog_snapshot_json.to_string();
    let catalog_path = catalog_path.to_string();
    compile_on_dedicated_stack(move || {
        compile_from_snapshot_json_on_current_thread(
            &workflow_snapshot_json,
            &catalog_snapshot_json,
            &catalog_path,
        )
    })
}

fn compile_from_snapshot_json_on_current_thread(
    workflow_snapshot_json: &str,
    catalog_snapshot_json: &str,
    catalog_path: &str,
) -> Result<RunPlan> {
    let wf: definition::WorkflowFile =
        serde_json::from_str(workflow_snapshot_json).context("parsing workflow snapshot JSON")?;
    let cat: catalog::AgentCatalogFile =
        serde_json::from_str(catalog_snapshot_json).context("parsing catalog snapshot JSON")?;
    let workflow_raw =
        serde_yaml::to_value(&wf).context("building workflow snapshot value for P051 lint")?;
    let catalog_raw =
        serde_yaml::to_value(&cat).context("building catalog snapshot value for P051 lint")?;
    let catalog_base = Path::new(catalog_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let snapshots = SnapshotJson {
        workflow_json: workflow_snapshot_json.to_string(),
        catalog_json: catalog_snapshot_json.to_string(),
    };

    compile_loaded(
        wf,
        cat,
        workflow_raw,
        catalog_raw,
        &catalog_base,
        Some(snapshots),
    )
}

fn compile_on_dedicated_stack<F>(compile: F) -> Result<RunPlan>
where
    F: FnOnce() -> Result<RunPlan> + Send + 'static,
{
    thread::Builder::new()
        .name("workflow-compile".to_string())
        .stack_size(WORKFLOW_COMPILE_STACK_BYTES)
        .spawn(compile)
        .context("spawning workflow compile thread")?
        .join()
        .map_err(|panic| {
            if let Some(message) = panic.downcast_ref::<&str>() {
                anyhow::anyhow!("workflow compile thread panicked: {message}")
            } else if let Some(message) = panic.downcast_ref::<String>() {
                anyhow::anyhow!("workflow compile thread panicked: {message}")
            } else {
                anyhow::anyhow!("workflow compile thread panicked")
            }
        })?
}

struct SnapshotJson {
    workflow_json: String,
    catalog_json: String,
}

fn compile_loaded(
    wf: definition::WorkflowFile,
    cat: catalog::AgentCatalogFile,
    workflow_raw: serde_yaml::Value,
    catalog_raw: serde_yaml::Value,
    catalog_base: &Path,
    snapshots: Option<SnapshotJson>,
) -> Result<RunPlan> {
    catalog::validate_catalog_has_exactly_one_system_lead(&cat)?;
    if snapshots.is_some() {
        catalog::validate_catalog_snapshot_format_version(&cat)?;
    }
    catalog::validate_toolchain_cache_policies(&cat)?;
    let direct_command_scan =
        crate::direct_command::scan_catalog(&cat, &wf, &workflow_raw, &catalog_raw);
    direct_command_scan.ensure_no_errors()?;
    let workflow_family = wf
        .workflow
        .as_ref()
        .and_then(|m| m.family.clone())
        .or_else(|| wf.workflow.as_ref().and_then(|m| m.id.clone()))
        .ok_or_else(|| anyhow::anyhow!("workflow.family or workflow.id is required"))?;
    let risk_class = wf
        .workflow
        .as_ref()
        .and_then(|m| m.risk_class.clone())
        .or_else(|| Some("standard".to_string()));
    let stack = wf
        .workflow
        .as_ref()
        .and_then(|m| m.stack.clone())
        .or_else(|| Some("unknown".to_string()));
    let legacy_broad_discovery_policy = wf
        .discovery
        .as_ref()
        .and_then(|discovery| discovery.legacy_broad_discovery_policy)
        .map(|policy| match policy {
            definition::LegacyBroadDiscoveryPolicyDef::Disabled => {
                LegacyBroadDiscoveryPolicy::Disabled
            }
            definition::LegacyBroadDiscoveryPolicyDef::WorkflowOptIn => {
                LegacyBroadDiscoveryPolicy::WorkflowOptIn
            }
        })
        .unwrap_or_default();
    // P077: Extract closeout_readiness_mode from workflow metadata.
    // Accepted values: "advisory" | "enforcement". Absent means advisory.
    let closeout_readiness_mode = wf
        .workflow
        .as_ref()
        .and_then(|m| m.closeout_readiness_mode.clone());
    let workflow_snapshot_json = match snapshots.as_ref() {
        Some(snapshot) => snapshot.workflow_json.clone(),
        None => canonical_json_string(&wf).context("serializing canonical workflow snapshot")?,
    };
    let catalog_snapshot_json = match snapshots.as_ref() {
        Some(snapshot) => snapshot.catalog_json.clone(),
        None => {
            canonical_json_string(&cat).context("serializing canonical agent catalog snapshot")?
        }
    };
    let workflow_snapshot_hash = sha256_string(&workflow_snapshot_json);
    let catalog_snapshot_hash = sha256_string(&catalog_snapshot_json);

    let agent_lookup = build_agent_lookup(&cat, catalog_base, &direct_command_scan)?;
    let contract_lookup = build_contract_lookup(&cat);

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
        let compiled = compile_state(
            state_id,
            state_def,
            &agent_lookup,
            &contract_lookup,
            &variables,
        )?;
        states.insert(state_id.clone(), compiled);
    }

    // P060: Compile dynamic candidate bindings from routing metadata.
    // Must happen before consuming cat.artifacts.
    let dynamic_candidate_bindings =
        compile_dynamic_candidate_bindings(&cat, &catalog_snapshot_hash);

    // P058: Compile escalation policies before any partial moves on `cat`.
    // Build the set of unsafe stage IDs for compile validation (SEC-P058-001).
    // is_unsafe_for_escalation() catches manual gates AND any non-compute stage types
    // (release, side_effect, publish, etc.) fail-closed before they exist in YAML.
    let unsafe_stage_ids: std::collections::HashSet<String> = wf
        .states
        .iter()
        .filter(|(_, state_def)| state_def.is_unsafe_for_escalation())
        .map(|(state_id, _)| state_id.clone())
        .collect();

    // SEC-001: also collect agent IDs that own or run tasks in unsafe stages, and the
    // backend_profile_ids of those agents, so agent_id/backend_profile_id bindings can
    // be validated fail-closed against side-effect stages.
    let mut unsafe_agent_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (_, state_def) in wf
        .states
        .iter()
        .filter(|(_, sd)| sd.is_unsafe_for_escalation())
    {
        unsafe_agent_ids.insert(state_def.owner.clone());
        let collect_tasks = |block: &crate::definition::RunBlock| {
            let mut ids = Vec::new();
            for task in block
                .sequence
                .iter()
                .flatten()
                .chain(block.parallel.iter().flatten())
                .chain(block.then.iter().flatten())
            {
                ids.push(task.agent.clone());
            }
            ids
        };
        if let Some(ref run_block) = state_def.run {
            unsafe_agent_ids.extend(collect_tasks(run_block));
        }
        if let Some(ref run_block) = state_def.run_after_approval {
            unsafe_agent_ids.extend(collect_tasks(run_block));
        }
    }
    // Map agent_id → backend_profile_id from catalog for the profile check.
    let unsafe_backend_profile_ids: std::collections::HashSet<String> = cat
        .agents
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter(|a| unsafe_agent_ids.contains(a.id.as_str()))
        .map(|a| a.backend_profile.clone())
        .collect();

    let all_stage_ids: std::collections::HashSet<String> = wf.states.keys().cloned().collect();
    let escalation_policies = compile_escalation_policies(
        &cat,
        &unsafe_stage_ids,
        &unsafe_agent_ids,
        &unsafe_backend_profile_ids,
        &all_stage_ids,
    )?;

    // Artifact name → path template from the catalog's `artifacts:` section.
    let artifact_paths: HashMap<String, String> =
        cat.artifacts.unwrap_or_default().into_iter().collect();

    // P066: Set run_plan_snapshot_format_version when any compiled agent carries
    // a toolchain_cache_policy.
    let has_toolchain_policy = states.values().any(|s| {
        s.owner.toolchain_cache_policy.is_some()
            || s.tasks
                .iter()
                .any(|t| t.agent.toolchain_cache_policy.is_some())
    });
    let run_plan_snapshot_format_version = if has_toolchain_policy {
        Some(crate::catalog::CATALOG_SNAPSHOT_FORMAT_VERSION)
    } else {
        None
    };

    Ok(RunPlan {
        initial_state: wf.initial_state,
        states,
        variables,
        artifact_paths,
        workflow_family: Some(workflow_family),
        risk_class,
        stack,
        legacy_broad_discovery_policy,
        workflow_snapshot_hash,
        catalog_snapshot_hash,
        workflow_snapshot_json,
        catalog_snapshot_json,
        dynamic_candidate_bindings,
        run_plan_snapshot_format_version,
        closeout_readiness_mode,
        escalation_policies,
    })
}

// ---------------------------------------------------------------------------
// Agent lookup
// ---------------------------------------------------------------------------

struct AgentBinding {
    backend_profile_id: String,
    provider: String,
    model: Option<String>,
    effort: Option<String>,
    max_turns: Option<u32>,
    temperature: Option<f64>,
    prompt: Option<String>,
    permission_profile: Option<String>,
    skill_ref: Option<String>,
    skill_role: Option<String>,
    skill_snapshot_hash: Option<String>,
    requested_mcp_server_ids: Vec<String>,
    output_contract: Option<String>,
    resolved_skill: Option<ResolvedSkill>,
    worktree_write_enabled: bool,
    worktree_strategy: Option<String>,
    session_reuse_scope: Option<String>,
    session_family_id: Option<String>,
    xcode_broker_required: bool,
    xcode_shim_injection_signal: bool,
    requires_xcode_host_execution: bool,
    xcode_prompt_lint_warnings: Vec<String>,
    /// P066: Toolchain cache policy from the catalog entry.
    toolchain_cache_policy: Option<crate::plan::ToolchainCachePolicySnapshot>,
}

/// Lookup from output artifact name or explicit contract ID → resolved schema.
/// Built once per compile from the catalog's `contracts:` section by indexing
/// the contract ID, normalized/raw artifact names, and versionless stem aliases.
struct ContractLookup {
    by_contract_id: HashMap<String, OutputSchema>,
    by_output: HashMap<String, OutputSchema>,
}

impl ContractLookup {
    fn resolve(
        &self,
        output_name: &str,
        explicit_contract: Option<&str>,
        output_count: usize,
    ) -> Option<OutputSchema> {
        if let Some(contract_id) = explicit_contract {
            if output_count == 1 || self.output_matches_contract(output_name, contract_id) {
                return self.by_contract_id.get(contract_id).cloned();
            }
        }

        self.by_output.get(output_name).cloned().or_else(|| {
            strip_version_suffix(output_name).and_then(|stem| self.by_output.get(&stem).cloned())
        })
    }

    fn output_matches_contract(&self, output_name: &str, contract_id: &str) -> bool {
        let Some(schema) = self.by_contract_id.get(contract_id) else {
            return false;
        };

        let output_stem = strip_version_suffix(output_name);
        let mut aliases = vec![contract_id];
        if let Some(alias) = schema.normalized_artifact_name.as_deref() {
            aliases.push(alias);
        }
        if let Some(alias) = schema.raw_artifact_name.as_deref() {
            aliases.push(alias);
        }

        aliases.into_iter().any(|alias| {
            output_name == alias
                || strip_version_suffix(alias).as_deref() == Some(output_name)
                || output_stem.as_deref() == Some(alias)
                || output_stem.as_deref() == strip_version_suffix(alias).as_deref()
        })
    }
}

fn build_contract_lookup(cat: &catalog::AgentCatalogFile) -> ContractLookup {
    let mut by_contract_id = HashMap::new();
    let mut by_output = HashMap::new();
    let Some(contracts) = cat.contracts.as_ref() else {
        return ContractLookup {
            by_contract_id,
            by_output,
        };
    };
    for (contract_id, def) in contracts.iter() {
        let schema = OutputSchema {
            contract_id: contract_id.clone(),
            format: def.format.clone().unwrap_or_else(|| "json".to_string()),
            human_format: def.human_format.clone().or_else(|| {
                (def.validation_mode.as_deref() == Some("structured_with_human_companion"))
                    .then_some("markdown".to_string())
            }),
            machine_format: def.machine_format.clone(),
            validation_mode: def.validation_mode.clone(),
            normalized_artifact_name: def.normalized_artifact_name.clone(),
            raw_artifact_name: def.raw_artifact_name.clone(),
            required_fields: def.required_fields.clone(),
        };
        by_contract_id.insert(contract_id.clone(), schema.clone());
        register_contract_alias(&mut by_output, contract_id, &schema);

        // Primary keys: normalized/raw artifact names (stable, machine-readable).
        if let Some(name) = &def.normalized_artifact_name {
            register_contract_alias(&mut by_output, name, &schema);
        }
        if let Some(name) = &def.raw_artifact_name {
            register_contract_alias(&mut by_output, name, &schema);
        }
    }
    ContractLookup {
        by_contract_id,
        by_output,
    }
}

fn register_contract_alias(
    lookup: &mut HashMap<String, OutputSchema>,
    alias: &str,
    schema: &OutputSchema,
) {
    lookup
        .entry(alias.to_string())
        .or_insert_with(|| schema.clone());
    if let Some(stem) = strip_version_suffix(alias) {
        lookup.entry(stem).or_insert_with(|| schema.clone());
    }
}

/// Strip a trailing version suffix from a contract identifier.
///
/// Accepted suffix families intentionally mirror the common stable forms used
/// in the proposal contract: `_vN`, `-vN`, `_VN`, `-VN`.
fn strip_version_suffix(id: &str) -> Option<String> {
    for marker in ["_v", "-v", "_V", "-V"] {
        let Some(idx) = id.rfind(marker) else {
            continue;
        };
        let tail = &id[idx + marker.len()..];
        if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) {
            return Some(id[..idx].to_string());
        }
    }
    None
}

fn sha256_string(content: &str) -> String {
    use sha2::{Digest, Sha256};

    let digest = Sha256::digest(content.as_bytes());
    format!("{digest:x}")
}

fn canonical_json_string<T: serde::Serialize>(value: &T) -> Result<String> {
    let value = serde_json::to_value(value).context("convert to canonical json value")?;
    serde_json::to_string(&sort_json_value(value)).context("serialize canonical json value")
}

fn load_raw_yaml_value(path: &str) -> Result<serde_yaml::Value> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("reading YAML at '{path}'"))?;
    serde_yaml::from_str(&content).with_context(|| format!("parsing YAML at '{path}'"))
}

fn sort_json_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(sort_json_value).collect())
        }
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(key, value)| (key, sort_json_value(value)))
                .collect(),
        ),
        scalar => scalar,
    }
}

fn build_agent_lookup(
    cat: &catalog::AgentCatalogFile,
    catalog_base: &Path,
    direct_command_scan: &DirectCommandScan,
) -> Result<HashMap<String, AgentBinding>> {
    let empty_profiles = HashMap::new();
    let profiles = cat.backend_profiles.as_ref().unwrap_or(&empty_profiles);
    let agents = cat.agents.as_ref().map(|v| v.as_slice()).unwrap_or(&[]);
    let empty_skills = HashMap::new();
    let skills = cat.skills.as_ref().unwrap_or(&empty_skills);

    let mut lookup = HashMap::new();
    for agent in agents {
        let profile = profiles.get(&agent.backend_profile).ok_or_else(|| {
            anyhow::anyhow!(
                "Agent '{}' references unknown backend_profile '{}'",
                agent.id,
                agent.backend_profile
            )
        })?;

        let provider = normalize_provider(&profile.provider).with_context(|| {
            format!(
                "Agent '{}' backend_profile '{}' has unknown provider '{}'",
                agent.id, agent.backend_profile, profile.provider
            )
        })?;
        let model = profile.model.clone();
        let effort = profile.effort.clone();
        let max_turns = profile.max_turns;
        let temperature = profile.temperature;
        let prompt = agent.prompt.clone();
        let output_contract = agent.output_contract.clone();
        let session_reuse_scope = agent.session_reuse_scope.clone();
        let session_family_id = agent.session_family_id.clone();
        let permission_profile = agent.permission_profile.clone();
        let skill_ref = agent.skill_ref.clone();
        let skill_role = agent.skill_role.clone();
        let mut requested_mcp_server_ids = profile.mcp.clone().unwrap_or_default();
        let xcode_signals =
            direct_command_scan.signals_for_agent(&agent.id, permission_profile.as_deref());
        let suppress_interactive_review_xcode_mcp =
            suppress_interactive_review_xcode_mcp(agent.mode.as_deref());
        if suppress_interactive_review_xcode_mcp {
            requested_mcp_server_ids.retain(|id| id != "xcode");
        }
        let xcode_mcp_requested = requested_mcp_server_ids.iter().any(|id| id == "xcode");

        // Resolve skill if referenced.
        let resolved_skill = if let Some(skill_ref) = &agent.skill_ref {
            match skills.get(skill_ref) {
                Some(skill_def) => {
                    match resolve_skill(
                        skill_ref,
                        skill_def,
                        agent.skill_role.as_deref(),
                        catalog_base,
                    ) {
                        Ok(rs) => {
                            info!(
                                agent_id = %agent.id,
                                skill_id = %rs.id,
                                skill_type = %rs.skill_type,
                                role = ?rs.role,
                                content_len = rs.injected_content.len(),
                                "Skill resolved"
                            );
                            Some(rs)
                        }
                        Err(e) => {
                            warn!(
                                agent_id = %agent.id,
                                skill_ref = %skill_ref,
                                "Failed to resolve skill: {e:#}"
                            );
                            None
                        }
                    }
                }
                None => {
                    warn!(
                        agent_id = %agent.id,
                        skill_ref = %skill_ref,
                        "skill_ref not found in catalog skills section"
                    );
                    None
                }
            }
        } else {
            None
        };
        let skill_snapshot_hash = resolved_skill
            .as_ref()
            .map(|skill| sha256_string(&skill.injected_content));

        // Extract worktree policy fields (matching Swift RunPlanCompiler).
        let (wt_write, wt_strategy) = agent
            .worktree_policy
            .as_ref()
            .map(|wp| (wp.write_enabled, Some(wp.strategy.clone())))
            .unwrap_or((false, None));

        lookup.insert(
            agent.id.clone(),
            AgentBinding {
                backend_profile_id: agent.backend_profile.clone(),
                provider,
                model,
                effort,
                max_turns,
                temperature,
                prompt,
                permission_profile,
                skill_ref,
                skill_role,
                skill_snapshot_hash,
                requested_mcp_server_ids,
                output_contract,
                resolved_skill,
                worktree_write_enabled: wt_write,
                worktree_strategy: wt_strategy,
                session_reuse_scope,
                session_family_id,
                xcode_broker_required: !suppress_interactive_review_xcode_mcp
                    && (agent.xcode_broker_required.unwrap_or(false) || xcode_mcp_requested),
                xcode_shim_injection_signal: agent.xcode_shim_injection_signal.unwrap_or(false)
                    || xcode_signals.xcode_shim_injection_signal,
                requires_xcode_host_execution: agent.requires_xcode_host_execution.unwrap_or(false)
                    || xcode_signals.requires_xcode_host_execution,
                xcode_prompt_lint_warnings: xcode_signals.xcode_prompt_lint_warnings,
                // P066: Copy catalog toolchain_cache_policy to snapshot.
                toolchain_cache_policy: agent.toolchain_cache_policy.as_ref().map(|p| {
                    crate::plan::ToolchainCachePolicySnapshot {
                        version: p.version,
                        enabled: p.enabled,
                        xcode_scope: p.xcode_scope.map(|s| match s {
                            crate::catalog::ToolchainCacheScope::Run => {
                                crate::plan::ToolchainCacheScopeSnapshot::Run
                            }
                            crate::catalog::ToolchainCacheScope::Session => {
                                crate::plan::ToolchainCacheScopeSnapshot::Session
                            }
                        }),
                        go_scope: p.go_scope.map(|s| match s {
                            crate::catalog::ToolchainCacheScope::Run => {
                                crate::plan::ToolchainCacheScopeSnapshot::Run
                            }
                            crate::catalog::ToolchainCacheScope::Session => {
                                crate::plan::ToolchainCacheScopeSnapshot::Session
                            }
                        }),
                    }
                }),
            },
        );
    }
    Ok(lookup)
}

fn suppress_interactive_review_xcode_mcp(mode: Option<&str>) -> bool {
    matches!(
        mode,
        Some("audit" | "prepush_review" | "proposal_authoring")
    )
}

/// Normalize YAML provider names to canonical provider families.
///
/// P061 requires catalog/workflow compilation to reuse the shared provider
/// resolver so unknown aliases fail before they can bypass scheduler caps.
fn normalize_provider(
    yaml_provider: &str,
) -> Result<String, domain::provider::UnknownProviderFamily> {
    ProviderFamily::canonicalize_alias(yaml_provider)
}

// ---------------------------------------------------------------------------
// State compilation
// ---------------------------------------------------------------------------

fn compile_state(
    state_id: &str,
    state: &definition::WorkflowState,
    agents: &HashMap<String, AgentBinding>,
    contracts: &ContractLookup,
    variables: &HashMap<String, serde_json::Value>,
) -> Result<CompiledState> {
    let owner = resolve_agent(&state.owner, agents)?;

    let tasks = state
        .run
        .as_ref()
        .map(|rb| compile_run_block(rb, agents, contracts))
        .transpose()?
        .unwrap_or_default();

    let post_approval_tasks = state
        .run_after_approval
        .as_ref()
        .map(|rb| compile_run_block(rb, agents, contracts))
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

    let loop_config = state
        .loop_config
        .as_ref()
        .map(|lc| compile_loop(lc, variables));
    let degraded_output_policy =
        compile_degraded_output_policy(state.degraded_output_policy.as_ref(), contracts)?;

    // P060: Compile dynamic_parallel and system_task definitions.
    let dynamic_parallel = state
        .run
        .as_ref()
        .and_then(|rb| rb.dynamic_parallel.as_ref())
        .map(|dp| CompiledDynamicParallel {
            selector_artifact: dp.selector_artifact.clone(),
            output_contract: dp.output_contract.clone(),
            inputs: dp.inputs.clone(),
        });

    let system_task = state
        .run
        .as_ref()
        .and_then(|rb| rb.system_task.as_ref())
        .map(|st| CompiledSystemTask {
            task_type: st.task_type.clone(),
            executor_mode: st.executor_mode.clone(),
        });

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
        degraded_output_policy,
        dynamic_parallel,
        system_task,
    })
}

fn compile_degraded_output_policy(
    policy: Option<&definition::DegradedOutputPolicyDef>,
    contracts: &ContractLookup,
) -> Result<DegradedOutputPolicy> {
    let Some(policy) = policy else {
        return Ok(DegradedOutputPolicy::default());
    };
    match policy.mode.as_str() {
        "deny" => Ok(DegradedOutputPolicy::default()),
        "allow_valid_contract_outputs" => {
            let contract_ids = policy.contracts.clone().unwrap_or_default();
            if contract_ids.is_empty() {
                anyhow::bail!(
                    "degraded_output_policy allow_valid_contract_outputs requires contracts"
                );
            }
            for contract_id in &contract_ids {
                if !contracts.by_contract_id.contains_key(contract_id) {
                    anyhow::bail!("unknown degraded_output_policy contract_id: {contract_id}");
                }
            }
            let failure_kinds = policy.failure_kinds.clone().unwrap_or_default();
            for failure_kind in &failure_kinds {
                if !matches!(
                    failure_kind.as_str(),
                    "provider_failure"
                        | "provider_quota"
                        | "provider_timeout"
                        | "transport_interrupted"
                ) {
                    anyhow::bail!("unknown degraded_output_policy failure_kind: {failure_kind}");
                }
            }
            let max_settlement = policy
                .max_settlement
                .clone()
                .unwrap_or_else(|| "valid_outputs_from_failed_execution".to_string());
            if max_settlement != "valid_outputs_from_failed_execution" {
                anyhow::bail!("unknown degraded_output_policy max_settlement: {max_settlement}");
            }
            Ok(DegradedOutputPolicy {
                mode: "allow_valid_contract_outputs".to_string(),
                contracts: contract_ids,
                failure_kinds,
                max_settlement,
            })
        }
        other => anyhow::bail!("unknown degraded_output_policy mode: {other}"),
    }
}

fn resolve_agent(agent_id: &str, agents: &HashMap<String, AgentBinding>) -> Result<ResolvedAgent> {
    match agents.get(agent_id) {
        Some(binding) => Ok(ResolvedAgent {
            agent_id: agent_id.to_string(),
            backend_profile_id: Some(binding.backend_profile_id.clone()),
            provider: binding.provider.clone(),
            model: binding.model.clone(),
            effort: binding.effort.clone(),
            max_turns: binding.max_turns,
            temperature: binding.temperature,
            prompt: binding.prompt.clone(),
            permission_profile: binding.permission_profile.clone(),
            skill_ref: binding.skill_ref.clone(),
            skill_role: binding.skill_role.clone(),
            skill_snapshot_hash: binding.skill_snapshot_hash.clone(),
            requested_mcp_server_ids: binding.requested_mcp_server_ids.clone(),
            resolved_skill: binding.resolved_skill.clone(),
            output_contract: binding.output_contract.clone(),
            worktree_write_enabled: binding.worktree_write_enabled,
            worktree_strategy: binding.worktree_strategy.clone(),
            session_reuse_scope: binding.session_reuse_scope.clone(),
            session_family_id: binding.session_family_id.clone(),
            xcode_broker_required: binding.xcode_broker_required,
            xcode_shim_injection_signal: binding.xcode_shim_injection_signal,
            requires_xcode_host_execution: binding.requires_xcode_host_execution,
            xcode_prompt_lint_warnings: binding.xcode_prompt_lint_warnings.clone(),
            toolchain_cache_policy: binding.toolchain_cache_policy.clone(),
        }),
        None => {
            // SEC-HIGH-002: unknown agent references must fail the compile rather than
            // silently resolving to a placeholder. A typo or injected agent_id must not
            // bypass catalog bindings (provider, model, permissions, output_contract, etc.).
            anyhow::bail!(
                "agent '{}' not found in catalog; workflow compile failed. \
                 Add the agent to the catalog or correct the reference.",
                agent_id
            )
        }
    }
}

fn compile_run_block(
    rb: &definition::RunBlock,
    agents: &HashMap<String, AgentBinding>,
    contracts: &ContractLookup,
) -> Result<Vec<CompiledTask>> {
    let mut tasks = Vec::new();

    // Sequential tasks — each gets its own phase for strict ordering (P044 §3a)
    if let Some(seq) = &rb.sequence {
        for (idx, at) in seq.iter().enumerate() {
            let mut t = compile_agent_task(at, agents, contracts, false)?;
            t.phase = idx as u32;
            tasks.push(t);
        }
    }

    // Parallel tasks (phase 0 — run concurrently)
    if let Some(par) = &rb.parallel {
        for at in par {
            let mut t = compile_agent_task(at, agents, contracts, true)?;
            t.phase = 0;
            tasks.push(t);
        }
    }

    // Then tasks — each gets its own incrementing phase after the max existing
    // phase, enforcing strict ordering (P044 §3a: auditor→prepush→aggregation)
    if let Some(then) = &rb.then {
        let mut next_phase = tasks.iter().map(|t| t.phase).max().unwrap_or(0) + 1;
        for at in then {
            let mut t = compile_agent_task(at, agents, contracts, false)?;
            t.phase = next_phase;
            next_phase += 1;
            tasks.push(t);
        }
    }

    Ok(tasks)
}

fn compile_agent_task(
    at: &definition::AgentTask,
    agents: &HashMap<String, AgentBinding>,
    contracts: &ContractLookup,
    parallel: bool,
) -> Result<CompiledTask> {
    let agent = resolve_agent(&at.agent, agents)?;
    let outputs = at.outputs.clone().unwrap_or_default();
    let explicit_contract = agent.output_contract.as_deref();
    let output_policies = compile_output_policies(&outputs, at.output_policies.as_ref())?;

    // Resolve output schemas. For single-output agents, an explicit
    // output_contract is authoritative. For multi-output agents, the explicit
    // contract only binds outputs whose alias/stem matches that contract; other
    // outputs resolve through their own artifact aliases.
    let mut output_schemas = HashMap::new();
    let output_count = outputs.len();
    for output_name in &outputs {
        if let Some(schema) = contracts.resolve(output_name, explicit_contract, output_count) {
            output_schemas.insert(output_name.clone(), schema);
        } else if let Some(contract_id) = explicit_contract {
            warn!(
                output_name = %output_name,
                contract_id = %contract_id,
                "explicit output_contract did not resolve to a contract"
            );
        }
    }

    // P060: propagate selected_outputs_from from DSL definition.
    let selected_outputs_from =
        at.selected_outputs_from
            .as_ref()
            .map(|sof| CompiledSelectedOutputsFrom {
                source_plan: sof.source_plan.clone(),
                output_contract: sof.output_contract.clone(),
            });

    Ok(CompiledTask {
        agent,
        task_name: at.task.clone(),
        inputs: at.inputs.clone().unwrap_or_default(),
        outputs,
        output_policies,
        output_schemas,
        parallel,
        phase: 0, // caller overrides for then-tasks
        selected_outputs_from,
    })
}

fn compile_output_policies(
    outputs: &[String],
    policies: Option<&HashMap<String, definition::OutputPolicyDef>>,
) -> Result<HashMap<String, OutputPolicy>> {
    let Some(policies) = policies else {
        return Ok(HashMap::new());
    };

    let output_names: HashSet<&str> = outputs.iter().map(String::as_str).collect();
    for output_name in policies.keys() {
        if !output_names.contains(output_name.as_str()) {
            anyhow::bail!(
                "output_policies key '{output_name}' does not match any declared task output"
            );
        }
    }

    Ok(policies
        .iter()
        .map(|(output_name, policy)| {
            let reuse_policy = match policy
                .reuse_policy
                .unwrap_or(definition::OutputReusePolicyDef::MustProduce)
            {
                definition::OutputReusePolicyDef::MustProduce => OutputReusePolicy::MustProduce,
                definition::OutputReusePolicyDef::AllowUnchangedExisting => {
                    OutputReusePolicy::AllowUnchangedExisting
                }
            };
            (output_name.clone(), OutputPolicy { reuse_policy })
        })
        .collect())
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

// ---------------------------------------------------------------------------
// Skill resolution (matches Swift SkillResolver + SkillRoleCustomizer + SkillInjector)
// ---------------------------------------------------------------------------

/// Resolve a skill definition into prompt-ready injected content.
///
/// Pipeline:
/// 1. Load base content (external → SKILL.md, inline → description, builtin → hardcoded)
/// SEC-002: Validate that a skill bundle relative path does not escape the catalog root.
/// Rejects absolute paths, `..` traversal components, null bytes, backslashes, and
/// URI-scheme prefixes. Works on the raw string before joining with catalog_base.
fn validate_skill_relative_path(skill_id: &str, field: &str, raw_path: &str) -> Result<()> {
    if raw_path.is_empty() {
        anyhow::bail!("skill '{skill_id}': field '{field}' is empty");
    }
    if raw_path.contains('\0') {
        anyhow::bail!("skill '{skill_id}': field '{field}' contains a null byte");
    }
    if raw_path.contains('\\') {
        anyhow::bail!("skill '{skill_id}': field '{field}' contains a backslash separator");
    }
    if raw_path.contains("://") {
        anyhow::bail!("skill '{skill_id}': field '{field}' contains a URI scheme separator");
    }
    // Reject absolute paths — all skill paths must be relative to catalog_base.
    if std::path::Path::new(raw_path).is_absolute() {
        anyhow::bail!(
            "skill '{skill_id}': field '{field}' must be a relative path, got '{raw_path}'"
        );
    }
    for component in raw_path.split('/') {
        if component == ".." {
            anyhow::bail!(
                "skill '{skill_id}': field '{field}' contains a path traversal component '..'"
            );
        }
    }
    Ok(())
}

/// SEC-002: Validate a role name so it cannot be used to escape the catalog via path separator
/// injection. Role names appear in `roles/{role}.md` joins and must be simple identifiers.
fn validate_skill_role_name(skill_id: &str, role: &str) -> Result<()> {
    if role.is_empty() {
        anyhow::bail!("skill '{skill_id}': role name is empty");
    }
    if role.contains('/')
        || role.contains('\\')
        || role.contains('\0')
        || role == ".."
        || role == "."
    {
        anyhow::bail!(
            "skill '{skill_id}': role name '{role}' contains unsafe characters or is a traversal token"
        );
    }
    Ok(())
}

/// 2. Apply role specialization (triad mode map, roles/{role}.md, or generic)
/// 3. Wrap with `## Skill: {id}\nType: {type}\n\n{content}` injection header
fn resolve_skill(
    skill_id: &str,
    skill_def: &catalog::SkillDef,
    skill_role: Option<&str>,
    catalog_base: &Path,
) -> Result<ResolvedSkill> {
    let skill_type_str = &skill_def.skill_type;

    // SEC-002: Validate role name before it is used in any path join.
    if let Some(role) = skill_role {
        validate_skill_role_name(skill_id, role)?;
    }

    // Step 1: Load base content by type
    let (base_content, type_label) = match skill_type_str.as_str() {
        "external_skill" => {
            let raw_path = skill_def
                .path
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("external_skill '{skill_id}' missing 'path'"))?;
            // SEC-002: Reject traversal and absolute paths before joining with catalog_base.
            validate_skill_relative_path(skill_id, "path", raw_path)?;
            let bundle_dir = catalog_base.join(raw_path);
            // SEC-002: After joining, verify the resolved path stays within the catalog base
            // to block symlink-based escapes that component-level checks cannot catch.
            if let (Ok(canon_base), Ok(canon_bundle)) = (
                std::fs::canonicalize(catalog_base),
                std::fs::canonicalize(&bundle_dir),
            ) {
                if !canon_bundle.starts_with(&canon_base) {
                    anyhow::bail!("skill '{skill_id}': path escapes catalog root via symlink");
                }
            }
            let skill_md = bundle_dir.join("SKILL.md");
            // Reject symlinks within the bundle to prevent escape via indirection.
            #[cfg(unix)]
            if std::fs::symlink_metadata(&skill_md)
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false)
            {
                anyhow::bail!("SKILL.md for external skill '{skill_id}' must not be a symlink");
            }
            let content = std::fs::read_to_string(&skill_md).with_context(|| {
                format!(
                    "reading SKILL.md for external skill '{skill_id}' at {}",
                    skill_md.display()
                )
            })?;
            if content.trim().is_empty() {
                anyhow::bail!("SKILL.md is empty for external skill '{skill_id}'");
            }
            (content, "external")
        }
        "inline_skill" => {
            let desc = skill_def.description.as_deref().ok_or_else(|| {
                anyhow::anyhow!("inline_skill '{skill_id}' missing 'description'")
            })?;
            (desc.to_string(), "inline")
        }
        "builtin_agent" => {
            let name = skill_def
                .name
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("builtin_agent '{skill_id}' missing 'name'"))?;
            let content = builtin_skill_content(name).ok_or_else(|| {
                anyhow::anyhow!("unknown builtin skill '{name}' (skill_id='{skill_id}')")
            })?;
            (content.to_string(), "builtin")
        }
        other => {
            anyhow::bail!("unsupported skill type '{other}' for skill '{skill_id}'");
        }
    };

    // Step 2: Apply role specialization
    let specialized =
        apply_role_specialization(skill_id, &base_content, skill_role, skill_def, catalog_base);

    // Step 3: Wrap with injection header (matches Swift SkillInjector)
    let injected_content = format!("## Skill: {skill_id}\nType: {type_label}\n\n{specialized}");

    Ok(ResolvedSkill {
        id: skill_id.to_string(),
        skill_type: type_label.to_string(),
        injected_content,
        role: skill_role.map(|s| s.to_string()),
    })
}

/// Apply role specialization to skill content.
/// Matches Swift `SkillRoleCustomizer.specialization()`.
fn apply_role_specialization(
    skill_id: &str,
    base_content: &str,
    skill_role: Option<&str>,
    skill_def: &catalog::SkillDef,
    catalog_base: &Path,
) -> String {
    let Some(role) = skill_role else {
        return base_content.to_string();
    };

    // Special case: proposal review skills have a hardcoded role→mode map.
    if skill_id == "proposal_review_triad" || skill_id == "proposal_review_router_skill" {
        if let Some((mode, instructions)) = proposal_review_role_mode(role) {
            return format!(
                "{base_content}\n\n## Active Role: {role}\n\nMode: {mode}\n\n{instructions}"
            );
        }
    }

    // Try loading roles/{role}.md from external bundle.
    if skill_def.skill_type == "external_skill" {
        if let Some(raw_path) = &skill_def.path {
            let role_file = catalog_base
                .join(raw_path)
                .join("roles")
                .join(format!("{role}.md"));
            // Reject symlinks to prevent escape via role file indirection.
            #[cfg(unix)]
            if std::fs::symlink_metadata(&role_file)
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false)
            {
                return base_content.to_string();
            }
            if let Ok(role_content) = std::fs::read_to_string(&role_file) {
                let trimmed = role_content.trim();
                if !trimmed.is_empty() {
                    return format!("{base_content}\n\n## Active Role: {role}\n\n{trimmed}");
                }
            }
        }
    }

    // Generic role block fallback.
    format!(
        "{base_content}\n\n## Active Role: {role}\n\n\
         You are operating in the \"{role}\" role for this skill. \
         Apply all skill instructions through the lens of this role."
    )
}

/// Hardcoded role→mode map for proposal review skills.
/// Matches Swift `SkillRoleCustomizer.proposalReviewModeMap`.
fn proposal_review_role_mode(role: &str) -> Option<(&'static str, &'static str)> {
    match role {
        "product_owner" => Some((
            "product-only",
            "As the Product Owner lens, focus on user problem clarity, business value, \
             scope discipline, acceptance criteria, rollout risk, metrics, and dependency realism.",
        )),
        "ux_designer" => Some((
            "ux-only",
            "As the UX lens, focus on information architecture, user flows, interaction patterns, \
             error handling, accessibility, and user mental model alignment.",
        )),
        "ui_designer" => Some((
            "ui-only",
            "As the UI lens, focus on visual hierarchy, component consistency, spacing, typography, \
             color, contrast, motion, responsive behavior, and design system compliance.",
        )),
        "architect" => Some((
            "architecture-only",
            "As the Architecture lens, focus on system boundaries, dependency management, data flow, \
             performance, scalability, security boundaries, and technical debt.",
        )),
        _ => None,
    }
}

/// Hardcoded builtin skill content registry.
/// Matches Swift `BuiltinSkillRegistry`.
fn builtin_skill_content(name: &str) -> Option<&'static str> {
    match name {
        "docs-quality-guardian" => Some(
            "You are the Docs Quality Guardian for Chainworks Forge.\n\
             Keep documentation aligned with approved behavior and implemented truth.\n\
             Prefer existing canonical reference and evidence lanes over duplicating proposal-era text.\n\
             Update only the documents that are genuinely affected, preserve source-of-truth boundaries, \
             and call out missing proof or stale references explicitly."
        ),
        _ => None,
    }
}

/// P060: Compile dynamic candidate bindings from a catalog for routing.
///
/// Extracts agents with `routing` metadata and builds frozen
/// `CompiledDynamicAgentBinding` entries for the proposal review router.
pub fn compile_dynamic_candidate_bindings(
    cat: &catalog::AgentCatalogFile,
    catalog_snapshot_hash: &str,
) -> Vec<domain::routing::CompiledDynamicAgentBinding> {
    let agents = cat.agents.as_ref().map(|v| v.as_slice()).unwrap_or(&[]);
    let empty_profiles = HashMap::new();
    let profiles = cat.backend_profiles.as_ref().unwrap_or(&empty_profiles);

    let mut bindings = Vec::new();
    for agent in agents {
        let Some(ref routing) = agent.routing else {
            continue;
        };

        // Build a minimal ResolvedAgent snapshot for the binding.
        let resolved_snapshot = serde_json::json!({
            "agent_id": agent.id,
            "backend_profile_id": agent.backend_profile,
            "provider": profiles.get(&agent.backend_profile).map(|p| &p.provider),
            "model": profiles.get(&agent.backend_profile).and_then(|p| p.model.as_ref()),
            "prompt": agent.prompt,
            "output_contract": agent.output_contract,
        });

        let resolved_agent_snapshot_json =
            serde_json::to_string(&resolved_snapshot).unwrap_or_else(|_| "{}".into());

        let output_contracts = agent
            .output_contract
            .as_ref()
            .map(|c| vec![c.clone()])
            .unwrap_or_default();

        let permission_hash =
            sha256_string(agent.permission_profile.as_deref().unwrap_or("default"));
        let worktree_hash = sha256_string(
            &agent
                .worktree_policy
                .as_ref()
                .map(|wp| wp.strategy.clone())
                .unwrap_or_default(),
        );
        let mcp_profile_hash = sha256_string(
            &profiles
                .get(&agent.backend_profile)
                .and_then(|p| p.mcp.as_ref())
                .map(|m| m.join(","))
                .unwrap_or_default(),
        );
        let skill_hash = sha256_string(agent.skill_ref.as_deref().unwrap_or("none"));
        let routing_metadata_json = serde_json::to_string(routing).unwrap_or_default();
        let routing_metadata_hash = sha256_string(&routing_metadata_json);

        let binding_id = format!("bind-{}", agent.id);

        bindings.push(domain::routing::CompiledDynamicAgentBinding {
            binding_id,
            agent_id: agent.id.clone(),
            resolved_agent_snapshot_json,
            output_contracts,
            permission_hash,
            worktree_hash,
            mcp_profile_hash,
            skill_hash,
            routing_metadata_hash,
            enabled_for_proposal_review: routing.enabled_for_proposal_review,
            rollout_wave: routing
                .rollout_wave
                .clone()
                .unwrap_or_else(|| "unknown".into()),
            catalog_snapshot_hash: catalog_snapshot_hash.into(),
            routing_metadata: domain::routing::RoutingMetadata {
                routing_id: routing.routing_id.clone(),
                family: routing.family.clone(),
                capabilities: routing.capabilities.clone(),
                stacks: routing.stacks.clone(),
                surfaces: routing.surfaces.clone(),
                risks: routing.risks.clone(),
                enabled_for_proposal_review: routing.enabled_for_proposal_review,
                rollout_wave: routing
                    .rollout_wave
                    .clone()
                    .unwrap_or_else(|| "unknown".into()),
                mandatory_when: routing.mandatory_when.clone(),
                usually_pair_with: routing.usually_pair_with.clone(),
                close_alternatives: routing.close_alternatives.clone(),
                strong_proposal_keywords: routing.strong_proposal_keywords.clone(),
                strong_repo_files: routing.strong_repo_files.clone(),
                strong_repo_symbols: routing.strong_repo_symbols.clone(),
                score_weights: routing.score_weights.clone(),
            },
        });
    }

    bindings
}

/// P060: Compile dynamic candidate bindings from already-parsed YAML paths.
pub fn compile_dynamic_candidate_bindings_from_paths(
    catalog_path: &str,
) -> Result<Vec<domain::routing::CompiledDynamicAgentBinding>> {
    let cat = catalog::load(catalog_path).context("loading agent catalog for routing bindings")?;
    let catalog_snapshot_json =
        canonical_json_string(&cat).context("serializing catalog snapshot for bindings")?;
    let catalog_snapshot_hash = sha256_string(&catalog_snapshot_json);
    Ok(compile_dynamic_candidate_bindings(
        &cat,
        &catalog_snapshot_hash,
    ))
}

// ── P058: Escalation policy compilation ───────────────────────────────────────

/// Compile escalation policies from the agent catalog into frozen RunPlan snapshots.
///
/// Validates backend_profile references, ambiguous bindings, and unsafe side-effect stage
/// bindings (by stage_id, agent_id, and backend_profile_id). Returns `Err` for any diagnostic.
fn compile_escalation_policies(
    cat: &catalog::AgentCatalogFile,
    unsafe_stage_ids: &std::collections::HashSet<String>,
    unsafe_agent_ids: &std::collections::HashSet<String>,
    unsafe_backend_profile_ids: &std::collections::HashSet<String>,
    all_stage_ids: &std::collections::HashSet<String>,
) -> Result<Vec<EscalationPolicySnapshot>> {
    use crate::escalation_policy::{
        compute_policy_hash, validate_applies_to_stage_selectors,
        validate_policies_against_catalog, validate_policies_for_ambiguous_bindings,
        validate_policies_for_unsafe_stage_bindings,
    };
    use crate::plan::EscalationTierSnapshot;

    // SEC-003: run structural validation FIRST so that malformed policy/stage/agent/
    // backend_profile identifiers containing control characters cannot be interpolated into
    // diagnostic messages before the safe-identifier check fires.
    // Policies deserialized from AgentCatalogFile bypass parse_policy and therefore bypass
    // the validate_policy_structure call there; re-run it here before any catalog validators
    // build error strings from catalog-controlled identifiers.
    if let Some(early_policies) = cat.escalation_policies.as_deref() {
        for policy in early_policies {
            crate::escalation_policy::validate_policy_structure(policy).map_err(|e| {
                anyhow::anyhow!(
                    "escalation_policy compile failed: [escalation_policy_compile_failed] \
                     policy structural validation: {e}"
                )
            })?;
        }
    }

    let mut all_diagnostics = validate_policies_against_catalog(cat);

    // Build agent_id → backend_profile_id map for cross-axis ambiguity detection.
    let agent_to_profile: std::collections::HashMap<&str, &str> = cat
        .agents
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|a| (a.id.as_str(), a.backend_profile.as_str()))
        .collect();

    // Check for ambiguous bindings, unsafe side-effect stage bindings (all three axes),
    // and unknown applies_to.stage_id selectors (HIGH-002 fail-closed).
    if let Some(policies) = cat.escalation_policies.as_deref() {
        all_diagnostics.extend(validate_policies_for_ambiguous_bindings(
            policies,
            &agent_to_profile,
        ));
        all_diagnostics.extend(validate_policies_for_unsafe_stage_bindings(
            policies,
            unsafe_stage_ids,
            unsafe_agent_ids,
            unsafe_backend_profile_ids,
        ));
        all_diagnostics.extend(validate_applies_to_stage_selectors(policies, all_stage_ids));
    }

    if !all_diagnostics.is_empty() {
        let msgs: Vec<String> = all_diagnostics
            .iter()
            .map(|d| format!("[{}] {}", d.pause_reason_code, d.detail))
            .collect();
        return Err(anyhow::anyhow!(
            "escalation_policy compile failed:\n{}",
            msgs.join("\n")
        ));
    }

    let policies = match cat.escalation_policies.as_deref() {
        Some(p) => p,
        None => return Ok(Vec::new()),
    };

    policies
        .iter()
        .map(|policy| {
            let policy_hash = compute_policy_hash(policy)
                .map_err(|e| anyhow::anyhow!("policy '{}' hash failed: {e}", policy.policy_id))?;
            let tiers = policy
                .tiers
                .iter()
                .map(|t| EscalationTierSnapshot {
                    tier_id: t.tier_id.clone(),
                    kind: t.kind.clone(),
                    backend_profile_id: t.backend_profile_id.clone(),
                    max_attempts: t.max_attempts,
                })
                .collect();
            let triggers = policy
                .triggers
                .iter()
                .map(|t| t.as_raw_str().to_string())
                .collect();
            Ok(EscalationPolicySnapshot {
                policy_id: policy.policy_id.clone(),
                schema_version: policy.schema_version.clone(),
                enabled_default: policy.enabled_default,
                applies_to_agent_id: policy.applies_to.agent_id.clone(),
                applies_to_backend_profile_id: policy.applies_to.backend_profile_id.clone(),
                applies_to_stage_id: policy.applies_to.stage_id.clone(),
                max_chain_attempts: policy.max_chain_attempts,
                max_chain_wall_clock_seconds: policy.max_chain_wall_clock_seconds,
                triggers,
                tiers,
                policy_hash,
                // Frozen at compile time — Phase 2+ digest computation reads this from the
                // snapshot to ensure algorithm stability across daemon upgrades mid-run.
                digest_version: Some("escalation_blocker_digest_v1".to_string()),
                // Phase 2+ runtime overrides (kill-switch, in_flight_toggle_behavior) will
                // populate this field when active at plan compile time. None for Phase 0-1.
                rollout_override_state: None,
            })
        })
        .collect()
}

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
                .filter_map(|(k, v)| k.as_str().map(|s| (s.to_string(), yaml_to_json(v))))
                .collect();
            serde_json::Value::Object(obj)
        }
        serde_yaml::Value::Tagged(tagged) => yaml_to_json(&tagged.value),
    }
}
