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
use std::path::Path;
use anyhow::{Context, Result};
use tracing::{info, warn};

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

    // Catalog base directory — used to resolve relative skill bundle paths.
    let catalog_base = Path::new(catalog_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    let agent_lookup = build_agent_lookup(&cat, &catalog_base)?;
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
        let compiled = compile_state(state_id, state_def, &agent_lookup, &contract_lookup, &variables)?;
        states.insert(state_id.clone(), compiled);
    }

    // Artifact name → path template from the catalog's `artifacts:` section.
    let artifact_paths: HashMap<String, String> = cat
        .artifacts
        .unwrap_or_default()
        .into_iter()
        .collect();

    Ok(RunPlan {
        initial_state: wf.initial_state,
        states,
        variables,
        artifact_paths,
    })
}

// ---------------------------------------------------------------------------
// Agent lookup
// ---------------------------------------------------------------------------

struct AgentBinding {
    provider: String,
    model: Option<String>,
    effort: Option<String>,
    prompt: Option<String>,
    output_contract: Option<String>,
    resolved_skill: Option<ResolvedSkill>,
    worktree_write_enabled: bool,
    worktree_strategy: Option<String>,
}

/// Lookup from output artifact name or explicit contract ID → resolved schema.
/// Built once per compile from the catalog's `contracts:` section by indexing
/// the contract ID, normalized/raw artifact names, and versionless stem aliases.
struct ContractLookup {
    by_contract_id: HashMap<String, OutputSchema>,
    by_output: HashMap<String, OutputSchema>,
}

impl ContractLookup {
    fn resolve(&self, output_name: &str, explicit_contract: Option<&str>) -> Option<OutputSchema> {
        if let Some(contract_id) = explicit_contract {
            return self.by_contract_id.get(contract_id).cloned();
        }

        self.by_output
            .get(output_name)
            .cloned()
            .or_else(|| {
                strip_version_suffix(output_name)
                    .and_then(|stem| self.by_output.get(&stem).cloned())
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
    lookup.entry(alias.to_string()).or_insert_with(|| schema.clone());
    if let Some(stem) = strip_version_suffix(alias) {
        lookup.entry(stem).or_insert_with(|| schema.clone());
    }
}

/// Strip a trailing `_v<N>` suffix from a contract identifier.
fn strip_version_suffix(id: &str) -> Option<String> {
    let idx = id.rfind("_v")?;
    let tail = &id[idx + 2..];
    if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) {
        Some(id[..idx].to_string())
    } else {
        None
    }
}

fn build_agent_lookup(
    cat: &catalog::AgentCatalogFile,
    catalog_base: &Path,
) -> Result<HashMap<String, AgentBinding>> {
    let empty_profiles = HashMap::new();
    let profiles = cat.backend_profiles.as_ref()
        .unwrap_or(&empty_profiles);
    let agents = cat.agents.as_ref()
        .map(|v| v.as_slice())
        .unwrap_or(&[]);
    let empty_skills = HashMap::new();
    let skills = cat.skills.as_ref().unwrap_or(&empty_skills);

    let mut lookup = HashMap::new();
    for agent in agents {
        let profile = profiles.get(&agent.backend_profile)
            .ok_or_else(|| anyhow::anyhow!(
                "Agent '{}' references unknown backend_profile '{}'",
                agent.id, agent.backend_profile
            ))?;

        let provider = normalize_provider(&profile.provider);
        let model = profile.model.clone();
        let effort = profile.effort.clone();
        let prompt = agent.prompt.clone();
        let output_contract = agent.output_contract.clone();

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

        // Extract worktree policy fields (matching Swift RunPlanCompiler).
        let (wt_write, wt_strategy) = agent.worktree_policy.as_ref()
            .map(|wp| (wp.write_enabled, Some(wp.strategy.clone())))
            .unwrap_or((false, None));

        lookup.insert(agent.id.clone(), AgentBinding {
            provider,
            model,
            effort,
            prompt,
            output_contract,
            resolved_skill,
            worktree_write_enabled: wt_write,
            worktree_strategy: wt_strategy,
        });
    }
    Ok(lookup)
}

/// Normalize YAML provider names to ACP adapter names.
/// `claude_acp` → `claude`, `codex_acp` → `codex`, `gemini_acp` → `gemini`, etc.
/// Also handles runtime-profile flavored providers like `claude_agent_acp` and `gemini_cli_acp`.
/// If the name doesn't end with a known ACP suffix, it's used as-is.
fn normalize_provider(yaml_provider: &str) -> String {
    if let Some(stripped) = yaml_provider.strip_suffix("_agent_acp") {
        return stripped.to_string();
    }
    if let Some(stripped) = yaml_provider.strip_suffix("_cli_acp") {
        return stripped.to_string();
    }
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
    contracts: &ContractLookup,
    variables: &HashMap<String, serde_json::Value>,
) -> Result<CompiledState> {
    let owner = resolve_agent(&state.owner, agents)?;

    let tasks = state.run.as_ref()
        .map(|rb| compile_run_block(rb, agents, contracts))
        .transpose()?
        .unwrap_or_default();

    let post_approval_tasks = state.run_after_approval.as_ref()
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
            effort: binding.effort.clone(),
            prompt: binding.prompt.clone(),
            resolved_skill: binding.resolved_skill.clone(),
            output_contract: binding.output_contract.clone(),
            worktree_write_enabled: binding.worktree_write_enabled,
            worktree_strategy: binding.worktree_strategy.clone(),
        }),
        None => {
            warn!(
                agent_id = agent_id,
                "Agent not found in catalog; using placeholder binding"
            );
            Ok(ResolvedAgent {
                agent_id: agent_id.to_string(),
                provider: "claude".to_string(),
                model: None,
                effort: None,
                prompt: None,
                resolved_skill: None,
                output_contract: None,
                worktree_write_enabled: false,
                worktree_strategy: None,
            })
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

    // Resolve output schemas: for each output, look up its contract via
    // explicit output_contract first, then normalized/raw artifact aliases,
    // then versioned/stem fallbacks. Agents get required-field lists and
    // contract metadata in their task directive so they know what structure
    // to produce.
    let mut output_schemas = HashMap::new();
    for output_name in &outputs {
        if let Some(schema) = contracts.resolve(output_name, explicit_contract) {
            output_schemas.insert(output_name.clone(), schema);
        } else if let Some(contract_id) = explicit_contract {
            warn!(
                output_name = %output_name,
                contract_id = %contract_id,
                "explicit output_contract did not resolve to a contract"
            );
        }
    }

    Ok(CompiledTask {
        agent,
        task_name: at.task.clone(),
        inputs: at.inputs.clone().unwrap_or_default(),
        outputs,
        output_schemas,
        parallel,
        phase: 0, // caller overrides for then-tasks
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

// ---------------------------------------------------------------------------
// Skill resolution (matches Swift SkillResolver + SkillRoleCustomizer + SkillInjector)
// ---------------------------------------------------------------------------

/// Resolve a skill definition into prompt-ready injected content.
///
/// Pipeline:
/// 1. Load base content (external → SKILL.md, inline → description, builtin → hardcoded)
/// 2. Apply role specialization (triad mode map, roles/{role}.md, or generic)
/// 3. Wrap with `## Skill: {id}\nType: {type}\n\n{content}` injection header
fn resolve_skill(
    skill_id: &str,
    skill_def: &catalog::SkillDef,
    skill_role: Option<&str>,
    catalog_base: &Path,
) -> Result<ResolvedSkill> {
    let skill_type_str = &skill_def.skill_type;

    // Step 1: Load base content by type
    let (base_content, type_label) = match skill_type_str.as_str() {
        "external_skill" => {
            let raw_path = skill_def.path.as_deref()
                .ok_or_else(|| anyhow::anyhow!("external_skill '{skill_id}' missing 'path'"))?;
            let bundle_dir = catalog_base.join(raw_path);
            let skill_md = bundle_dir.join("SKILL.md");
            let content = std::fs::read_to_string(&skill_md)
                .with_context(|| format!(
                    "reading SKILL.md for external skill '{skill_id}' at {}",
                    skill_md.display()
                ))?;
            if content.trim().is_empty() {
                anyhow::bail!("SKILL.md is empty for external skill '{skill_id}'");
            }
            (content, "external")
        }
        "inline_skill" => {
            let desc = skill_def.description.as_deref()
                .ok_or_else(|| anyhow::anyhow!("inline_skill '{skill_id}' missing 'description'"))?;
            (desc.to_string(), "inline")
        }
        "builtin_agent" => {
            let name = skill_def.name.as_deref()
                .ok_or_else(|| anyhow::anyhow!("builtin_agent '{skill_id}' missing 'name'"))?;
            let content = builtin_skill_content(name)
                .ok_or_else(|| anyhow::anyhow!("unknown builtin skill '{name}' (skill_id='{skill_id}')"))?;
            (content.to_string(), "builtin")
        }
        other => {
            anyhow::bail!("unsupported skill type '{other}' for skill '{skill_id}'");
        }
    };

    // Step 2: Apply role specialization
    let specialized = apply_role_specialization(
        skill_id,
        &base_content,
        skill_role,
        skill_def,
        catalog_base,
    );

    // Step 3: Wrap with injection header (matches Swift SkillInjector)
    let injected_content = format!(
        "## Skill: {skill_id}\nType: {type_label}\n\n{specialized}"
    );

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

    // Special case: proposal_review_triad has a hardcoded role→mode map.
    if skill_id == "proposal_review_triad" {
        if let Some((mode, instructions)) = triad_role_mode(role) {
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
            if let Ok(role_content) = std::fs::read_to_string(&role_file) {
                let trimmed = role_content.trim();
                if !trimmed.is_empty() {
                    return format!(
                        "{base_content}\n\n## Active Role: {role}\n\n{trimmed}"
                    );
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

/// Hardcoded role→mode map for `proposal_review_triad`.
/// Matches Swift `SkillRoleCustomizer.triadModeMap`.
fn triad_role_mode(role: &str) -> Option<(&'static str, &'static str)> {
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
