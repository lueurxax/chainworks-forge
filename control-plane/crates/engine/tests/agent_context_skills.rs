use chrono::Utc;
use db::pool::create_pool;
use db::repos::{ideas, runs};
use domain::commands::{CallerContext, Command, PrincipalClass, StartRunCmd};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{IdeaId, RunId};
use domain::run::{Run, RunStatus};
use engine::agent_mission_context::{
    finalize_mediation_prompt_v1, finalize_owner_prompt_v1, finalize_task_prompt_v1,
    preflight_run_mission_context, validate_legacy_flat_invoke_agent,
    validate_persisted_v1_payload_prompt, validate_persisted_v1_prompt, MAX_IDEA_CONTEXT_BYTES,
};
use engine::command_handler::{compile_run_plan_for_run, CommandHandler};
use engine::event_bus;
use engine::work_queue::WorkQueue;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::collections::{BTreeSet, HashMap};
use workflow::plan::{
    CompiledState, CompiledTask, CompiledTransition, DegradedOutputPolicy, ResolvedAgent,
    ResolvedSkill, RunPlan,
};

fn compile_plan() -> RunPlan {
    compile_workflow("full-mvp-live.yaml")
}

fn compile_workflow(workflow_file: &str) -> RunPlan {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    workflow::compiler::compile(
        root.join("examples/workflows")
            .join(workflow_file)
            .to_str()
            .unwrap(),
        root.join("examples/agents/agents.yaml").to_str().unwrap(),
    )
    .expect("active workflow should compile")
}

fn task_for_name<'a>(
    plan: &'a RunPlan,
    state_id: &str,
    task_name: &str,
) -> (&'a CompiledState, &'a CompiledTask) {
    let state = plan.states.get(state_id).expect("state should exist");
    let task = state
        .tasks
        .iter()
        .find(|task| task.task_name == task_name)
        .expect("task should exist");
    (state, task)
}

fn idea(title: String, body: String) -> Idea {
    Idea {
        id: IdeaId::new(),
        title,
        body,
        workspace_root_path: Some("/workspace".into()),
        project_key: None,
        status: IdeaStatus::Draft,
        created_at: Utc::now(),
        archived_at: None,
    }
}

fn run(plan: &RunPlan, idea: &Idea, state_id: &str) -> Run {
    Run {
        id: RunId::new(),
        idea_id: idea.id,
        status: RunStatus::Running,
        workflow_id: "full-mvp-live".into(),
        workflow_title: "Full MVP".into(),
        workspace_root: "/workspace".into(),
        artifact_root: "/workspace/.chainworks".into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        cancellation_settlement_log: None,
        current_state: Some(state_id.into()),
        workflow_yaml_path: None,
        agent_catalog_yaml_path: None,
        worktree_root: Some("/workspace/.chainworks/worktrees/implementation".into()),
        base_branch: None,
        base_revision: None,
        target_branch: None,
        delivery_configuration_json: None,
        delivery_preflight_json: None,
        workflow_family: plan.workflow_family.clone(),
        project_key: None,
        risk_class: plan.risk_class.clone(),
        stack: plan.stack.clone(),
        workflow_snapshot_hash: Some(plan.workflow_snapshot_hash.clone()),
        catalog_snapshot_hash: Some(plan.catalog_snapshot_hash.clone()),
        workflow_snapshot_json: Some(plan.workflow_snapshot_json.clone()),
        catalog_snapshot_json: Some(plan.catalog_snapshot_json.clone()),
        drift_detected_at: None,
        drift_details_json: None,
        chainworks_meta_root: Some("/workspace/.chainworks/runs/test".into()),
        review_routing_json: None,
        closeout_readiness_mode: None,
    }
}

fn task_for_agent<'a>(plan: &'a RunPlan, agent_id: &str) -> (&'a CompiledState, &'a CompiledTask) {
    plan.states
        .values()
        .find_map(|state| {
            state
                .tasks
                .iter()
                .find(|task| task.agent.agent_id == agent_id)
                .map(|task| (state, task))
        })
        .expect("agent task should exist")
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextEvalCase {
    case_id: String,
    run_id: String,
    idea_id: String,
    idea: ContextEvalIdea,
    frozen_plan: ContextEvalPlan,
    dispatch_task: ContextEvalTask,
    task_body: String,
    expected_context: serde_json::Value,
    #[serde(default)]
    required_prompt_substrings: Vec<String>,
    #[serde(default)]
    prohibited_mission_substrings: Vec<String>,
    negative_mutations: Vec<ContextEvalMutation>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextEvalIdea {
    title: String,
    body: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextEvalPlan {
    workflow_family: String,
    state_id: String,
    state_label: String,
    owner: ContextEvalAgent,
    #[serde(default)]
    declared_tasks: Vec<ContextEvalTask>,
    #[serde(default)]
    transitions: Vec<ContextEvalTransition>,
    #[serde(default)]
    target_owners: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextEvalTask {
    task_name: String,
    agent: ContextEvalAgent,
    #[serde(default)]
    outputs: Vec<String>,
    #[serde(default)]
    parallel: bool,
    #[serde(default)]
    phase: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextEvalAgent {
    agent_id: String,
    provider: String,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    system_prompt: Option<String>,
    #[serde(default)]
    permission_profile: Option<String>,
    #[serde(default)]
    worktree_write_enabled: bool,
    #[serde(default)]
    output_contract: Option<String>,
    #[serde(default)]
    procedure: Option<ContextEvalProcedure>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextEvalProcedure {
    id: String,
    source_kind: String,
    hash: String,
    content: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextEvalTransition {
    to: String,
    when: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextEvalMutation {
    json_pointer: String,
    replacement: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActiveReviewContextCase {
    case_id: String,
    workflow_path: String,
    state_id: String,
    task_name: String,
    idea: ContextEvalIdea,
    task_body: String,
    expected_inputs: Vec<String>,
    expected_output: String,
    expected_contract: String,
    expected_permission_profile: String,
    expected_skill_ref: String,
    expected_system_prompt_clause: String,
    expected_consumer_task: String,
    expected_consumer_agent: String,
    expected_claim_ids: BTreeSet<String>,
    negative_mutations: Vec<PromptMutation>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum PromptMutation {
    MissionJsonReplace {
        claim_id: String,
        json_pointer: String,
        replacement: serde_json::Value,
    },
    SystemPromptRemove {
        claim_id: String,
        needle: String,
    },
    ProcedureRemove {
        claim_id: String,
        needle: String,
    },
    TaskInputRemove {
        claim_id: String,
        input: String,
    },
    TaskInputAdd {
        claim_id: String,
        input: String,
    },
    TaskBodyRemove {
        claim_id: String,
        needle: String,
    },
}

impl PromptMutation {
    fn claim_id(&self) -> &str {
        match self {
            Self::MissionJsonReplace { claim_id, .. }
            | Self::SystemPromptRemove { claim_id, .. }
            | Self::ProcedureRemove { claim_id, .. }
            | Self::TaskInputRemove { claim_id, .. }
            | Self::TaskInputAdd { claim_id, .. }
            | Self::TaskBodyRemove { claim_id, .. } => claim_id,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InvokeAgentProducerFixture {
    producer_id: String,
    source_file: String,
    function: String,
    classification: String,
    guard: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SecurityPrepushSnapshotFixture {
    schema_version: String,
    source_commit: String,
    workflow_snapshot: serde_json::Value,
    catalog_snapshot: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SecurityPrepushGoldenPrompts {
    schema_version: String,
    security_prompt: String,
    prepush_prompt: String,
}

fn context_eval_agent(fixture: &ContextEvalAgent) -> ResolvedAgent {
    let (skill_ref, skill_snapshot_hash, resolved_skill) = match &fixture.procedure {
        Some(procedure) => (
            Some(procedure.id.clone()),
            Some(procedure.hash.clone()),
            Some(ResolvedSkill {
                id: procedure.id.clone(),
                skill_type: procedure.source_kind.clone(),
                injected_content: procedure.content.clone(),
                role: None,
            }),
        ),
        None => (None, None, None),
    };
    ResolvedAgent {
        agent_id: fixture.agent_id.clone(),
        backend_profile_id: Some(format!("{}_profile", fixture.agent_id)),
        provider: fixture.provider.clone(),
        model: fixture.model.clone(),
        effort: None,
        max_turns: Some(8),
        temperature: None,
        prompt: fixture.system_prompt.clone(),
        permission_profile: fixture.permission_profile.clone(),
        skill_ref,
        skill_role: None,
        skill_snapshot_hash,
        requested_mcp_server_ids: Vec::new(),
        resolved_skill,
        output_contract: fixture.output_contract.clone(),
        worktree_write_enabled: fixture.worktree_write_enabled,
        worktree_strategy: None,
        session_reuse_scope: None,
        session_family_id: None,
        xcode_broker_required: false,
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        xcode_prompt_lint_warnings: Vec::new(),
        toolchain_cache_policy: None,
    }
}

fn context_eval_task(fixture: &ContextEvalTask) -> CompiledTask {
    CompiledTask {
        agent: context_eval_agent(&fixture.agent),
        task_name: fixture.task_name.clone(),
        inputs: Vec::new(),
        outputs: fixture.outputs.clone(),
        output_policies: HashMap::new(),
        output_schemas: HashMap::new(),
        parallel: fixture.parallel,
        phase: fixture.phase,
        selected_outputs_from: None,
    }
}

fn context_eval_plan(fixture: &ContextEvalPlan) -> RunPlan {
    let transitions = fixture
        .transitions
        .iter()
        .map(|transition| CompiledTransition {
            to: transition.to.clone(),
            condition: transition.when.clone(),
        })
        .collect();
    let mut states = HashMap::new();
    states.insert(
        fixture.state_id.clone(),
        CompiledState {
            id: fixture.state_id.clone(),
            label: fixture.state_label.clone(),
            state_type: None,
            owner: context_eval_agent(&fixture.owner),
            is_manual_gate: false,
            is_end: false,
            tasks: fixture
                .declared_tasks
                .iter()
                .map(context_eval_task)
                .collect(),
            post_approval_tasks: Vec::new(),
            transitions,
            loop_config: None,
            degraded_output_policy: DegradedOutputPolicy::default(),
            dynamic_parallel: None,
            system_task: None,
        },
    );
    for (state_id, owner_id) in &fixture.target_owners {
        states.insert(
            state_id.clone(),
            CompiledState {
                id: state_id.clone(),
                label: state_id.clone(),
                state_type: None,
                owner: context_eval_agent(&ContextEvalAgent {
                    agent_id: owner_id.clone(),
                    provider: "fixture".into(),
                    model: None,
                    system_prompt: None,
                    permission_profile: None,
                    worktree_write_enabled: false,
                    output_contract: None,
                    procedure: None,
                }),
                is_manual_gate: false,
                is_end: true,
                tasks: Vec::new(),
                post_approval_tasks: Vec::new(),
                transitions: Vec::new(),
                loop_config: None,
                degraded_output_policy: DegradedOutputPolicy::default(),
                dynamic_parallel: None,
                system_task: None,
            },
        );
    }
    RunPlan {
        initial_state: fixture.state_id.clone(),
        states,
        variables: HashMap::new(),
        artifact_paths: HashMap::new(),
        workflow_family: Some(fixture.workflow_family.clone()),
        risk_class: None,
        stack: None,
        legacy_broad_discovery_policy: Default::default(),
        workflow_snapshot_hash: "1".repeat(64),
        catalog_snapshot_hash: "2".repeat(64),
        workflow_snapshot_json: "{}".into(),
        catalog_snapshot_json: "{}".into(),
        mission_context_version: Some("agent_mission_context_v1".into()),
        dynamic_candidate_bindings: Vec::new(),
        run_plan_snapshot_format_version: None,
        closeout_readiness_mode: None,
        escalation_policies: Vec::new(),
    }
}

fn extract_mission_context(prompt: &str) -> serde_json::Value {
    let raw = prompt
        .split_once("## Mission Context\n")
        .and_then(|(_, rest)| rest.split_once("\n\nFrozen precedence rules:"))
        .map(|(context, _)| context)
        .expect("prompt must contain one bounded mission context block");
    serde_json::from_str(raw).expect("mission context must be JSON")
}

fn score_context_case(
    fixture: &ContextEvalCase,
    prompt: &str,
    actual_context: &serde_json::Value,
) -> Result<(), String> {
    if prompt.matches("## Mission Context").count() != 1 {
        return Err(format!(
            "{} mission block cardinality changed",
            fixture.case_id
        ));
    }
    if actual_context != &fixture.expected_context {
        return Err(format!("{} mission context differs", fixture.case_id));
    }
    let mission_offset = prompt
        .find("## Mission Context")
        .ok_or_else(|| format!("{} mission block missing", fixture.case_id))?;
    let precedence_offset = prompt
        .find("Frozen precedence rules:")
        .ok_or_else(|| format!("{} precedence missing", fixture.case_id))?;
    let body_offset = prompt
        .rfind(&fixture.task_body)
        .ok_or_else(|| format!("{} task body missing", fixture.case_id))?;
    if !(mission_offset < precedence_offset && precedence_offset < body_offset) {
        return Err(format!("{} prompt ordering changed", fixture.case_id));
    }
    for required in &fixture.required_prompt_substrings {
        if !prompt.contains(required) {
            return Err(format!(
                "{} required prompt evidence missing",
                fixture.case_id
            ));
        }
    }
    let mission_bytes = serde_json::to_string(actual_context).map_err(|error| error.to_string())?;
    for prohibited in &fixture.prohibited_mission_substrings {
        if mission_bytes.contains(prohibited) {
            return Err(format!(
                "{} prohibited mission content present",
                fixture.case_id
            ));
        }
    }
    Ok(())
}

fn materialized_review_task_body(fixture: &ActiveReviewContextCase, task: &CompiledTask) -> String {
    let direct_test_evidence = if task.inputs.iter().any(|input| input == "tests_result") {
        "Direct tests_result evidence: declared by the compiled task; assess it directly."
    } else {
        "Direct tests_result evidence: not declared by the compiled task; do not invent or fetch it."
    };
    format!(
        "{}\n\nDeclared task inputs: {}\nLogical output: {}\nOutput contract: {}.\n{}",
        fixture.task_body,
        serde_json::to_string(&task.inputs).unwrap(),
        task.outputs.join(","),
        task.agent.output_contract.as_deref().unwrap_or("none"),
        direct_test_evidence,
    )
}

fn replacement_string<'a>(
    fixture: &ActiveReviewContextCase,
    pointer: &str,
    replacement: &'a serde_json::Value,
) -> &'a str {
    replacement.as_str().unwrap_or_else(|| {
        panic!(
            "{} mutation {} replacement must be a string",
            fixture.case_id, pointer
        )
    })
}

fn apply_mission_source_mutation(
    fixture: &ActiveReviewContextCase,
    plan: &mut RunPlan,
    task: &mut CompiledTask,
    idea: &mut Idea,
    json_pointer: &str,
    replacement: &serde_json::Value,
) {
    let replacement = replacement_string(fixture, json_pointer, replacement).to_string();
    match json_pointer {
        "/mission/operator_request_title" => idea.title = replacement,
        "/runtime/permission_profile" => task.agent.permission_profile = Some(replacement),
        "/runtime/procedure/source_kind" => {
            task.agent
                .resolved_skill
                .as_mut()
                .expect("review task should resolve a procedure")
                .skill_type = replacement;
        }
        "/assignment/declared_outputs/0" => {
            *task
                .outputs
                .first_mut()
                .expect("review task should declare one output") = replacement;
        }
        "/assignment/consumers/0/task" => {
            let state = plan
                .states
                .get_mut(&fixture.state_id)
                .expect("review state should exist");
            let next_phase = state
                .tasks
                .iter()
                .map(|candidate| candidate.phase)
                .filter(|phase| *phase > task.phase)
                .min()
                .expect("review task should have a next execution phase");
            state
                .tasks
                .iter_mut()
                .find(|candidate| candidate.phase == next_phase)
                .expect("next execution phase should contain a task")
                .task_name = replacement;
        }
        other => panic!(
            "{} has unsupported source mutation pointer {other}",
            fixture.case_id
        ),
    }
}

fn finalized_active_review_case(
    fixture: &ActiveReviewContextCase,
    mutation: Option<&PromptMutation>,
) -> (CompiledTask, String, serde_json::Value) {
    let workflow_file = std::path::Path::new(&fixture.workflow_path)
        .file_name()
        .and_then(|value| value.to_str())
        .expect("fixture workflow path should have a file name");
    let mut plan = compile_workflow(workflow_file);
    let (_, compiled_task) = task_for_name(&plan, &fixture.state_id, &fixture.task_name);
    let mut task = compiled_task.clone();
    let mut idea = idea(fixture.idea.title.clone(), fixture.idea.body.clone());
    let mut body_removal = None;

    if let Some(mutation) = mutation {
        match mutation {
            PromptMutation::MissionJsonReplace {
                json_pointer,
                replacement,
                ..
            } => apply_mission_source_mutation(
                fixture,
                &mut plan,
                &mut task,
                &mut idea,
                json_pointer,
                replacement,
            ),
            PromptMutation::SystemPromptRemove { needle, .. } => {
                let prompt = task
                    .agent
                    .prompt
                    .as_mut()
                    .expect("review task should have a system prompt");
                assert!(
                    prompt.contains(needle),
                    "{} system mutation needle must exist",
                    fixture.case_id
                );
                *prompt = prompt.replacen(needle, "", 1);
            }
            PromptMutation::ProcedureRemove { needle, .. } => {
                let procedure = &mut task
                    .agent
                    .resolved_skill
                    .as_mut()
                    .expect("review task should resolve a procedure")
                    .injected_content;
                assert!(
                    procedure.contains(needle),
                    "{} procedure mutation needle must exist",
                    fixture.case_id
                );
                *procedure = procedure.replacen(needle, "", 1);
            }
            PromptMutation::TaskInputRemove { input, .. } => {
                let original_len = task.inputs.len();
                task.inputs.retain(|candidate| candidate != input);
                assert_eq!(
                    task.inputs.len() + 1,
                    original_len,
                    "{} input-removal mutation must remove one input",
                    fixture.case_id
                );
            }
            PromptMutation::TaskInputAdd { input, .. } => {
                assert!(
                    !task.inputs.contains(input),
                    "{} input-add mutation must add an undeclared input",
                    fixture.case_id
                );
                task.inputs.push(input.clone());
            }
            PromptMutation::TaskBodyRemove { needle, .. } => {
                body_removal = Some(needle.as_str());
            }
        }
    }

    let mut body = materialized_review_task_body(fixture, &task);
    if let Some(needle) = body_removal {
        assert!(
            body.contains(needle),
            "{} task-body mutation needle must exist",
            fixture.case_id
        );
        body = body.replacen(needle, "", 1);
    }
    let state = plan
        .states
        .get(&fixture.state_id)
        .expect("review state should exist");
    let run = run(&plan, &idea, &state.id);
    let prompt = finalize_task_prompt_v1(&plan, &run, state, &task, &idea, &body)
        .expect("active review prompt should finalize");
    let mission = extract_mission_context(&prompt);
    (task, prompt, mission)
}

fn string_array(value: Option<&serde_json::Value>) -> Vec<&str> {
    value
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .collect()
}

fn score_active_review_case(
    fixture: &ActiveReviewContextCase,
    task: &CompiledTask,
    prompt: &str,
    mission: &serde_json::Value,
) -> BTreeSet<String> {
    let mut claims = BTreeSet::new();
    let body = prompt
        .rsplit_once("## Security Assignment")
        .or_else(|| prompt.rsplit_once("## Pre-Push Assignment"))
        .map(|(_, body)| body)
        .unwrap_or_default();

    if mission
        .pointer("/mission/operator_request_title")
        .and_then(|v| v.as_str())
        == Some(fixture.idea.title.as_str())
        && mission
            .pointer("/mission/operator_request_body")
            .and_then(|value| value.as_str())
            == Some(fixture.idea.body.as_str())
    {
        claims.insert("operator_objective".to_string());
    }
    if prompt.contains(&fixture.expected_system_prompt_clause) {
        claims.insert("review_assignment".to_string());
    }
    if mission
        .pointer("/runtime/procedure/kind")
        .and_then(|value| value.as_str())
        == Some("resolved")
        && mission
            .pointer("/runtime/procedure/id")
            .and_then(|value| value.as_str())
            == Some(fixture.expected_skill_ref.as_str())
        && mission
            .pointer("/runtime/procedure/source_kind")
            .and_then(|value| value.as_str())
            == Some("external")
    {
        claims.insert("external_procedure".to_string());
    }
    if mission
        .pointer("/runtime/permission_profile")
        .and_then(|value| value.as_str())
        == Some(fixture.expected_permission_profile.as_str())
    {
        claims.insert("permission_profile".to_string());
    }
    if string_array(mission.pointer("/assignment/declared_outputs"))
        == [fixture.expected_output.as_str()]
        && string_array(mission.pointer("/assignment/provider_outputs"))
            == [fixture.expected_output.as_str()]
        && body.contains(&format!("Logical output: {}", fixture.expected_output))
    {
        claims.insert("logical_output".to_string());
    }
    if task.agent.output_contract.as_deref() == Some(fixture.expected_contract.as_str())
        && body.contains(&format!("Output contract: {}.", fixture.expected_contract))
    {
        claims.insert("output_contract".to_string());
    }
    let actual_non_test_inputs = task
        .inputs
        .iter()
        .filter(|input| input.as_str() != "tests_result")
        .cloned()
        .collect::<Vec<_>>();
    let expected_non_test_inputs = fixture
        .expected_inputs
        .iter()
        .filter(|input| input.as_str() != "tests_result")
        .cloned()
        .collect::<Vec<_>>();
    if actual_non_test_inputs == expected_non_test_inputs {
        claims.insert("required_evidence_inputs".to_string());
    }
    let procedure_has_conditional_test_rule = prompt.contains(
        "Inspect `tests_result` directly only when it is declared by the compiled task; otherwise do not invent or fetch it.",
    );
    if fixture.expected_inputs.iter().any(|input| input == "tests_result") {
        if task.inputs.iter().any(|input| input == "tests_result")
            && body.contains(
                "Direct tests_result evidence: declared by the compiled task; assess it directly.",
            )
            && procedure_has_conditional_test_rule
        {
            claims.insert("declared_test_evidence_available".to_string());
        }
    } else if !task.inputs.iter().any(|input| input == "tests_result")
        && body.contains(
            "Direct tests_result evidence: not declared by the compiled task; do not invent or fetch it.",
        )
        && procedure_has_conditional_test_rule
    {
        claims.insert("no_undeclared_test_evidence".to_string());
    }
    if prompt.contains(
        "only the declared control-plane-generated `changed_files_manifest` as canonical Git evidence",
    ) {
        claims.insert("control_plane_manifest_provenance".to_string());
    }
    if prompt
        .contains("read-only scanner results as evidence rather than as a substitute for reasoning")
    {
        claims.insert("scanner_as_evidence".to_string());
    }
    if prompt.contains("Keep discovery bounded to changed and implicated paths.") {
        claims.insert("bounded_discovery".to_string());
    }
    if prompt.contains(
        "Never invoke `git status`, `git diff`, or `git rev-parse`, and never read `.git`.",
    ) {
        claims.insert("no_direct_git".to_string());
    }
    if prompt.contains(
        "Publish only the logical output `security_report` under `security_report_v1`; do not mutate source, proposal, approval, release, or external state.",
    ) {
        claims.insert("no_mutation_authority".to_string());
    }
    if prompt.contains(
        "Return `block` when required evidence is missing, invalid, red, or contains an unresolved blocking finding.",
    ) {
        claims.insert("fail_closed".to_string());
    }
    if prompt.contains(
        "Publish only the logical output `prepush_review_report` under `prepush_review_v1`; do not edit source, commit, push, approve, release, or cause external effects.",
    ) {
        claims.insert("no_release_authority".to_string());
    }
    let consumers = mission
        .pointer("/assignment/consumers")
        .and_then(serde_json::Value::as_array);
    if consumers.is_some_and(|values| {
        values.len() == 1
            && values[0].get("kind").and_then(|value| value.as_str()) == Some("task")
            && values[0].get("task").and_then(|value| value.as_str())
                == Some(fixture.expected_consumer_task.as_str())
            && values[0].get("agent_id").and_then(|value| value.as_str())
                == Some(fixture.expected_consumer_agent.as_str())
    }) {
        claims.insert("next_phase_consumer".to_string());
    }
    if body.contains(
        "audit_report is invalid and security_report contains an unresolved blocking finding",
    ) {
        claims.insert("blocking_upstream_evidence".to_string());
    }
    claims
}

#[test]
fn ctx_007_and_008_compile_active_tasks_and_reject_each_prompt_mutation() {
    let fixture_dir =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/agent_context");
    for case_id in ["CTX-007", "CTX-008"] {
        let path = fixture_dir.join(format!("{case_id}.json"));
        let fixture: ActiveReviewContextCase =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(fixture.case_id, case_id);
        let mutation_claims = fixture
            .negative_mutations
            .iter()
            .map(|mutation| mutation.claim_id().to_string())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            mutation_claims, fixture.expected_claim_ids,
            "{case_id} must target every expected claim exactly once"
        );
        assert_eq!(
            fixture.negative_mutations.len(),
            fixture.expected_claim_ids.len(),
            "{case_id} must have one mutation per expected claim"
        );

        let (task, prompt, mission) = finalized_active_review_case(&fixture, None);
        let baseline = score_active_review_case(&fixture, &task, &prompt, &mission);
        assert_eq!(
            baseline, fixture.expected_claim_ids,
            "{case_id} baseline claim set drifted"
        );

        for mutation in &fixture.negative_mutations {
            let (task, prompt, mission) = finalized_active_review_case(&fixture, Some(mutation));
            let actual = score_active_review_case(&fixture, &task, &prompt, &mission);
            let mut expected = fixture.expected_claim_ids.clone();
            expected.remove(mutation.claim_id());
            assert_eq!(
                actual,
                expected,
                "{case_id} mutation for {} must remove only its named claim",
                mutation.claim_id()
            );
        }
    }
}

#[test]
fn active_review_tasks_cover_conditional_test_evidence_branches() {
    struct Case<'a> {
        workflow: &'a str,
        task: &'a str,
        inputs: &'a [&'a str],
        output: &'a str,
        contract: &'a str,
        consumer_task: &'a str,
        consumer_agent: &'a str,
    }
    let cases = [
        Case {
            workflow: "full-mvp-live.yaml",
            task: "check_implementation_security",
            inputs: &["approved_proposal", "changed_files_manifest"],
            output: "security_report",
            contract: "security_report_v1",
            consumer_task: "audit_implementation_against_proposal",
            consumer_agent: "proposal_implementation_auditor",
        },
        Case {
            workflow: "full-mvp-live.yaml",
            task: "prepush_review",
            inputs: &[
                "approved_proposal",
                "changed_files_manifest",
                "audit_report",
                "security_report",
            ],
            output: "prepush_review_report",
            contract: "prepush_review_v1",
            consumer_task: "aggregate_implementation_reviews",
            consumer_agent: "lead_orchestrator",
        },
        Case {
            workflow: "workflow.yaml",
            task: "review_security",
            inputs: &[
                "approved_proposal",
                "implementation_progress",
                "changed_files_manifest",
                "tests_result",
            ],
            output: "security_report",
            contract: "security_report_v1",
            consumer_task: "sync_docs_for_review",
            consumer_agent: "docs_guardian",
        },
        Case {
            workflow: "workflow.yaml",
            task: "review_before_push",
            inputs: &[
                "approved_proposal",
                "implementation_progress",
                "changed_files_manifest",
                "tests_result",
                "audit_report",
                "security_report",
            ],
            output: "prepush_review_report",
            contract: "prepush_review_v1",
            consumer_task: "aggregate_implementation_reviews",
            consumer_agent: "lead_orchestrator",
        },
    ];

    for case in cases {
        let plan = compile_workflow(case.workflow);
        let (state, task) = task_for_name(&plan, "state_9_implementation_reviewed", case.task);
        assert_eq!(
            task.inputs,
            case.inputs
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
        );
        assert_eq!(task.outputs, [case.output]);
        assert_eq!(task.agent.output_contract.as_deref(), Some(case.contract));
        assert_eq!(
            task.agent
                .resolved_skill
                .as_ref()
                .map(|skill| skill.skill_type.as_str()),
            Some("external")
        );

        let fixture = ActiveReviewContextCase {
            case_id: format!("{}:{}", case.workflow, case.task),
            workflow_path: format!("examples/workflows/{}", case.workflow),
            state_id: state.id.clone(),
            task_name: task.task_name.clone(),
            idea: ContextEvalIdea {
                title: "Conditional evidence compatibility".into(),
                body: "Use exactly the evidence declared by the compiled task.".into(),
            },
            task_body: if case.contract == "security_report_v1" {
                "## Security Assignment\nReview the declared evidence.".into()
            } else {
                "## Pre-Push Assignment\nReview the declared evidence.".into()
            },
            expected_inputs: case.inputs.iter().map(|value| value.to_string()).collect(),
            expected_output: case.output.into(),
            expected_contract: case.contract.into(),
            expected_permission_profile: task
                .agent
                .permission_profile
                .clone()
                .expect("review permission should be declared"),
            expected_skill_ref: task
                .agent
                .skill_ref
                .clone()
                .expect("review skill should be declared"),
            expected_system_prompt_clause: task.agent.prompt.clone().unwrap_or_default(),
            expected_consumer_task: case.consumer_task.into(),
            expected_consumer_agent: case.consumer_agent.into(),
            expected_claim_ids: BTreeSet::new(),
            negative_mutations: Vec::new(),
        };
        let (baseline_task, baseline_prompt, baseline_mission) =
            finalized_active_review_case(&fixture, None);
        let baseline = score_active_review_case(
            &fixture,
            &baseline_task,
            &baseline_prompt,
            &baseline_mission,
        );
        let expected_test_claim = if case.inputs.contains(&"tests_result") {
            "declared_test_evidence_available"
        } else {
            "no_undeclared_test_evidence"
        };
        assert!(
            baseline.contains(expected_test_claim),
            "{} must take its declared test-evidence branch",
            fixture.case_id
        );
        assert!(baseline.contains("logical_output"));
        assert!(baseline.contains("output_contract"));
        assert!(baseline.contains("required_evidence_inputs"));
        assert!(baseline.contains("next_phase_consumer"));

        let mutation = if case.inputs.contains(&"tests_result") {
            PromptMutation::TaskInputRemove {
                claim_id: expected_test_claim.into(),
                input: "tests_result".into(),
            }
        } else {
            PromptMutation::TaskInputAdd {
                claim_id: expected_test_claim.into(),
                input: "tests_result".into(),
            }
        };
        let (mutated_task, mutated_prompt, mutated_mission) =
            finalized_active_review_case(&fixture, Some(&mutation));
        let mutated =
            score_active_review_case(&fixture, &mutated_task, &mutated_prompt, &mutated_mission);
        assert!(
            !mutated.contains(expected_test_claim),
            "{} test-evidence mutation must fail its branch claim",
            fixture.case_id
        );
    }
}

fn security_prepush_compatibility_prompts(plan: &RunPlan) -> (String, String) {
    let state = &plan.states["state_9_implementation_reviewed"];
    let mut compatibility_idea = idea(
        "Preserve pre-migration review prompts".into(),
        "Existing frozen runs must retain their exact inline security and pre-push procedures."
            .into(),
    );
    compatibility_idea.id = "00000000-0000-4000-8000-000000002829".parse().unwrap();
    let mut compatibility_run = run(plan, &compatibility_idea, &state.id);
    compatibility_run.id = "00000000-0000-4000-8000-000000002830".parse().unwrap();

    let security_task = state
        .tasks
        .iter()
        .find(|task| task.task_name == "check_implementation_security")
        .unwrap();
    let security_prompt = finalize_task_prompt_v1(
        plan,
        &compatibility_run,
        state,
        security_task,
        &compatibility_idea,
        "## Security Assignment\nReview the historical inline security procedure.",
    )
    .unwrap();

    let prepush_task = state
        .tasks
        .iter()
        .find(|task| task.task_name == "prepush_review")
        .unwrap();
    let prepush_prompt = finalize_task_prompt_v1(
        plan,
        &compatibility_run,
        state,
        prepush_task,
        &compatibility_idea,
        "## Pre-Push Assignment\nReview the historical inline pre-push procedure.",
    )
    .unwrap();
    (security_prompt, prepush_prompt)
}

#[test]
fn pre_migration_v2_inline_snapshot_survives_external_bundle_migration() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let fixture: SecurityPrepushSnapshotFixture = serde_json::from_slice(
        &std::fs::read(root.join(
            "control-plane/crates/workflow/tests/fixtures/agent_context/\
             security_prepush_catalog_v2.json",
        ))
        .unwrap(),
    )
    .unwrap();
    let golden: SecurityPrepushGoldenPrompts = serde_json::from_slice(
        &std::fs::read(root.join(
            "control-plane/crates/workflow/tests/fixtures/agent_context/\
             security_prepush_golden_prompts.json",
        ))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        fixture.schema_version,
        "security_prepush_catalog_v2_fixture_v1"
    );
    assert_eq!(
        fixture.source_commit,
        "465fa72a880333347fbc0988f788f0f82d8b2523"
    );
    assert_eq!(golden.schema_version, "security_prepush_golden_prompts_v1");
    assert_eq!(
        fixture.catalog_snapshot["catalog_snapshot_format_version"],
        2
    );
    assert!(
        fixture.catalog_snapshot["chainworks_compiled"]["skill_bundles"]
            .get("security_checker_core")
            .is_none()
    );
    assert!(
        fixture.catalog_snapshot["chainworks_compiled"]["skill_bundles"]
            .get("prepush_review_core")
            .is_none()
    );

    let plan = workflow::compiler::compile_from_snapshot_json(
        &serde_json::to_string(&fixture.workflow_snapshot).unwrap(),
        &serde_json::to_string(&fixture.catalog_snapshot).unwrap(),
        "/tmp/chainworks-agent-context-missing-live-catalog/catalog.yaml",
    )
    .expect("pre-migration V2 snapshot must not consult the live catalog or new bundles");
    let state = &plan.states["state_9_implementation_reviewed"];
    for task_name in ["check_implementation_security", "prepush_review"] {
        let procedure = state
            .tasks
            .iter()
            .find(|task| task.task_name == task_name)
            .and_then(|task| task.agent.resolved_skill.as_ref())
            .expect("pre-migration review procedure should resolve from snapshot");
        assert_eq!(procedure.skill_type, "inline");
    }
    let (security_prompt, prepush_prompt) = security_prepush_compatibility_prompts(&plan);
    assert_eq!(
        security_prompt.as_bytes(),
        golden.security_prompt.as_bytes()
    );
    assert_eq!(prepush_prompt.as_bytes(), golden.prepush_prompt.as_bytes());
}

#[test]
#[ignore = "fixture regeneration requires explicit historical source paths"]
fn regenerate_security_prepush_compatibility_fixtures() {
    let workflow_path = std::env::var("CHAINWORKS_PREMIGRATION_WORKFLOW_PATH")
        .expect("set CHAINWORKS_PREMIGRATION_WORKFLOW_PATH");
    let catalog_path = std::env::var("CHAINWORKS_PREMIGRATION_CATALOG_PATH")
        .expect("set CHAINWORKS_PREMIGRATION_CATALOG_PATH");
    let plan = workflow::compiler::compile(&workflow_path, &catalog_path)
        .expect("historical workflow and catalog should compile");
    let (security_prompt, prepush_prompt) = security_prepush_compatibility_prompts(&plan);
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let fixture_dir = root.join("control-plane/crates/workflow/tests/fixtures/agent_context");
    let snapshot = serde_json::json!({
        "schema_version": "security_prepush_catalog_v2_fixture_v1",
        "source_commit": "465fa72a880333347fbc0988f788f0f82d8b2523",
        "workflow_snapshot": serde_json::from_str::<serde_json::Value>(&plan.workflow_snapshot_json).unwrap(),
        "catalog_snapshot": serde_json::from_str::<serde_json::Value>(&plan.catalog_snapshot_json).unwrap(),
    });
    let golden = serde_json::json!({
        "schema_version": "security_prepush_golden_prompts_v1",
        "security_prompt": security_prompt,
        "prepush_prompt": prepush_prompt,
    });
    std::fs::write(
        fixture_dir.join("security_prepush_catalog_v2.json"),
        serde_json::to_vec_pretty(&snapshot).unwrap(),
    )
    .unwrap();
    std::fs::write(
        fixture_dir.join("security_prepush_golden_prompts.json"),
        serde_json::to_vec_pretty(&golden).unwrap(),
    )
    .unwrap();
}

#[test]
fn ctx_001_through_ctx_006_have_exact_positive_and_mutation_negative_scores() {
    let fixture_dir =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/agent_context");
    let paths = (1..=6)
        .map(|index| fixture_dir.join(format!("CTX-{index:03}.json")))
        .collect::<Vec<_>>();
    let expected_names = (1..=6)
        .map(|index| format!("CTX-{index:03}.json"))
        .collect::<Vec<_>>();
    let actual_names = paths
        .iter()
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        actual_names, expected_names,
        "CTX corpus must be an exact set"
    );

    for path in paths {
        let fixture: ContextEvalCase =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(path.file_stem().unwrap().to_string_lossy(), fixture.case_id);
        assert!(!fixture.negative_mutations.is_empty());
        let plan = context_eval_plan(&fixture.frozen_plan);
        let mut idea = idea(fixture.idea.title.clone(), fixture.idea.body.clone());
        idea.id = fixture.idea_id.parse().unwrap();
        let state = &plan.states[&fixture.frozen_plan.state_id];
        let task = context_eval_task(&fixture.dispatch_task);
        let mut run = run(&plan, &idea, &state.id);
        run.id = fixture.run_id.parse().unwrap();
        let prompt =
            finalize_task_prompt_v1(&plan, &run, state, &task, &idea, &fixture.task_body).unwrap();
        let actual_context = extract_mission_context(&prompt);
        score_context_case(&fixture, &prompt, &actual_context).unwrap();

        for mutation in &fixture.negative_mutations {
            let mut mutated = actual_context.clone();
            *mutated
                .pointer_mut(&mutation.json_pointer)
                .unwrap_or_else(|| panic!("{} invalid mutation pointer", fixture.case_id)) =
                mutation.replacement.clone();
            assert!(
                score_context_case(&fixture, &prompt, &mutated).is_err(),
                "{} mutation {} must be rejected",
                fixture.case_id,
                mutation.json_pointer
            );
        }
    }
}

fn producer_function_span(source: &str, function: &str) -> Result<(usize, usize), String> {
    let signatures = [
        format!("    async fn {function}"),
        format!("    pub async fn {function}"),
        format!("    fn {function}"),
        format!("    pub fn {function}"),
    ];
    let starts = signatures
        .iter()
        .filter_map(|signature| source.find(signature))
        .collect::<Vec<_>>();
    if starts.len() != 1 {
        return Err(format!(
            "function {function} must have one production definition; found {}",
            starts.len()
        ));
    }
    let start = starts[0];
    let tail = &source[start + 1..];
    let end = [
        "\n    async fn ",
        "\n    pub async fn ",
        "\n    fn ",
        "\n    pub fn ",
    ]
    .iter()
    .filter_map(|marker| tail.find(marker))
    .min()
    .map(|offset| start + 1 + offset)
    .unwrap_or(source.len());
    Ok((start, end))
}

fn validate_producer_inventory(
    manifest: &[InvokeAgentProducerFixture],
    sources: &HashMap<String, String>,
) -> Result<(), String> {
    let allowed = ["fresh_finalized", "copy_validated", "legacy_non_v1"];
    let mut ids = std::collections::BTreeSet::new();
    let mut classified_sites = Vec::new();
    for entry in manifest {
        if !ids.insert(entry.producer_id.clone()) {
            return Err(format!("duplicate producer_id {}", entry.producer_id));
        }
        if !allowed.contains(&entry.classification.as_str()) {
            return Err(format!(
                "{} has unsupported classification {}",
                entry.producer_id, entry.classification
            ));
        }
        let source = sources
            .get(&entry.source_file)
            .ok_or_else(|| format!("{} source missing", entry.producer_id))?;
        let (start, end) = producer_function_span(source, &entry.function)?;
        let function_source = &source[start..end];
        let raw_sites = function_source
            .match_indices("WorkItemKind::InvokeAgent,")
            .map(|(offset, _)| start + offset)
            .collect::<Vec<_>>();
        if raw_sites.len() != 1 {
            return Err(format!(
                "{} must own exactly one InvokeAgent producer; found {}",
                entry.producer_id,
                raw_sites.len()
            ));
        }
        let guard_offset = function_source.find(&entry.guard).ok_or_else(|| {
            format!(
                "{} missing {} guard {}",
                entry.producer_id, entry.classification, entry.guard
            )
        })?;
        if start + guard_offset >= raw_sites[0] {
            return Err(format!(
                "{} guard must execute before its producer",
                entry.producer_id
            ));
        }
        classified_sites.push((entry.source_file.clone(), raw_sites[0]));
    }

    for (source_file, source) in sources {
        for (offset, _) in source.match_indices("WorkItemKind::InvokeAgent,") {
            let owners = classified_sites
                .iter()
                .filter(|(classified_file, classified_offset)| {
                    classified_file == source_file && *classified_offset == offset
                })
                .count();
            if owners != 1 {
                return Err(format!(
                    "unclassified InvokeAgent producer in {source_file} at byte {offset}"
                ));
            }
        }
    }
    Ok(())
}

#[test]
fn invoke_agent_producer_manifest_is_closed_and_each_guard_is_mutation_sensitive() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let manifest_path = root.join(
        "control-plane/crates/engine/tests/fixtures/agent_context/invoke_agent_producers.json",
    );
    let manifest: Vec<InvokeAgentProducerFixture> = serde_json::from_slice(
        &std::fs::read(&manifest_path).expect("producer manifest must exist"),
    )
    .unwrap();
    let expected_ids = [
        "command_handler.targeted_retry",
        "orchestrator.auto_contract_retry",
        "orchestrator.dynamic_parallel",
        "orchestrator.legacy_flat",
        "orchestrator.owner_only",
        "orchestrator.p017_mediation",
        "orchestrator.p058_escalation_retry",
        "orchestrator.standard_task",
    ];
    let actual_ids = manifest
        .iter()
        .map(|entry| entry.producer_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(actual_ids, expected_ids.into_iter().collect());

    let mut sources = HashMap::new();
    for source_file in ["orchestrator.rs", "command_handler.rs"] {
        let source = std::fs::read_to_string(
            root.join("control-plane/crates/engine/src")
                .join(source_file),
        )
        .unwrap();
        sources.insert(
            source_file.to_string(),
            source.split("#[cfg(test)]").next().unwrap().to_string(),
        );
    }
    validate_producer_inventory(&manifest, &sources).unwrap();

    for entry in &manifest {
        let mut mutated = sources.clone();
        let source = mutated.get_mut(&entry.source_file).unwrap();
        let (start, end) = producer_function_span(source, &entry.function).unwrap();
        let relative = source[start..end].find(&entry.guard).unwrap();
        let guard_start = start + relative;
        source.replace_range(
            guard_start..guard_start + entry.guard.len(),
            "removed_guard_for_mutation",
        );
        let error = validate_producer_inventory(&manifest, &mutated)
            .expect_err("removing any producer guard must fail inventory");
        assert!(
            error.contains(&entry.producer_id),
            "mutation failure must identify {}: {error}",
            entry.producer_id
        );
    }

    let mut unclassified = sources.clone();
    unclassified.get_mut("orchestrator.rs").unwrap().push_str(
        "\n    async fn mutation_only_unknown_producer() { WorkItemKind::InvokeAgent, }\n",
    );
    let error = validate_producer_inventory(&manifest, &unclassified)
        .expect_err("an unknown InvokeAgent producer must fail inventory");
    assert!(error.contains("unclassified InvokeAgent producer"));
}

#[test]
fn v1_task_prompt_orders_mission_before_skill_and_projects_authoritative_outputs() {
    let plan = compile_plan();
    let (state, task) = task_for_agent(&plan, "code_writer");
    let idea = idea(
        "Implement the approved request".into(),
        "The feature works by default; do not add a flag.".into(),
    );
    let run = run(&plan, &idea, &state.id);

    let prompt = finalize_task_prompt_v1(
        &plan,
        &run,
        state,
        task,
        &idea,
        "## Task Body\nPerform the implementation.",
    )
    .expect("V1 task prompt should finalize");

    assert_eq!(prompt.matches("## Mission Context").count(), 1);
    let system = prompt.find("## System Instructions").unwrap();
    let mission = prompt.find("## Mission Context").unwrap();
    let skill = prompt.find("## Skill: code_writer_core").unwrap();
    let body = prompt.find("## Task Body").unwrap();
    assert!(system < mission && mission < skill && skill < body);
    assert!(prompt.contains("\"operator_request_title\":\"Implement the approved request\""));
    assert!(prompt.contains("\"permission_profile\":\"CODE_WRITE\""));
    assert!(prompt.contains("\"engine_owned_outputs\":[\"changed_files_manifest\"]"));
    assert!(prompt.contains(
        "\"provider_outputs\":[\"implementation_progress\",\"implementation_self_assessment\",\"tests_result\"]"
    ));
    assert!(prompt.contains("\"kind\":\"resolved\""));
    validate_persisted_v1_prompt(&plan, &prompt).expect("fresh prompt should validate for copy");
}

#[test]
fn implementation_auditor_uses_external_procedure_without_prompt_duplication() {
    let plan = compile_plan();
    let (state, task) = task_for_agent(&plan, "proposal_implementation_auditor");
    let skill = task
        .agent
        .resolved_skill
        .as_ref()
        .expect("implementation auditor should resolve a procedure");

    assert_eq!(skill.skill_type, "external");
    assert!(skill
        .injected_content
        .contains("Use changed_files_manifest as canonical Git evidence"));
    assert!(!task
        .agent
        .prompt
        .as_deref()
        .unwrap_or_default()
        .contains("Use changed_files_manifest as canonical Git evidence"));

    let idea = idea(
        "Audit the approved implementation".into(),
        "Keep conformance and readiness evidence exact.".into(),
    );
    let run = run(&plan, &idea, &state.id);
    let prompt = finalize_task_prompt_v1(
        &plan,
        &run,
        state,
        task,
        &idea,
        "## Task Body\nAudit this implementation.",
    )
    .expect("implementation auditor prompt should finalize");

    let mission = prompt.find("## Mission Context").unwrap();
    let procedure = prompt
        .find("## Skill: proposal_implementation_audit")
        .unwrap();
    let body = prompt.find("## Task Body").unwrap();
    assert!(mission < procedure && procedure < body);
    assert_eq!(
        prompt
            .matches("Use changed_files_manifest as canonical Git evidence")
            .count(),
        1
    );
    assert!(prompt.contains("\"permission_profile\":\"RO_VERIFY\""));
    assert!(prompt.contains("\"provider_outputs\":[\"audit_report\"]"));
}

#[test]
fn security_and_prepush_external_procedures_inject_once_after_mission() {
    let plan = compile_plan();
    let idea = idea(
        "Review the implementation safely".into(),
        "Preserve the frozen evidence and authority boundaries.".into(),
    );
    for (agent_id, skill_id, unique_clause, body_header) in [
        (
            "security_checker",
            "security_checker_core",
            "read-only scanner results as evidence rather than as a substitute for reasoning",
            "## Security Assignment",
        ),
        (
            "prepush_code_reviewer",
            "prepush_review_core",
            "Never reinterpret a blocking security or audit result as `pass`.",
            "## Pre-Push Assignment",
        ),
    ] {
        let (state, task) = task_for_agent(&plan, agent_id);
        let procedure = task
            .agent
            .resolved_skill
            .as_ref()
            .expect("review procedure should resolve");
        assert_eq!(procedure.skill_type, "external");
        assert!(!task
            .agent
            .prompt
            .as_deref()
            .unwrap_or_default()
            .contains(unique_clause));
        let run = run(&plan, &idea, &state.id);
        let prompt = finalize_task_prompt_v1(
            &plan,
            &run,
            state,
            task,
            &idea,
            &format!("{body_header}\nReview only the declared evidence."),
        )
        .unwrap();
        let mission = prompt.find("## Mission Context").unwrap();
        let procedure = prompt.find(&format!("## Skill: {skill_id}")).unwrap();
        let body = prompt.find(body_header).unwrap();
        assert!(mission < procedure && procedure < body);
        assert_eq!(prompt.matches(unique_clause).count(), 1);
    }
}

#[test]
fn mission_consumers_cover_next_phase_and_transitions() {
    let plan = compile_plan();
    let idea = idea(
        "Consumer test".into(),
        "Keep downstream work explicit.".into(),
    );

    let state_with_next_phase = &plan.states["state_10_implementation_refined"];
    let first = &state_with_next_phase.tasks[0];
    let first_run = run(&plan, &idea, &state_with_next_phase.id);
    let phase_prompt = finalize_task_prompt_v1(
        &plan,
        &first_run,
        state_with_next_phase,
        first,
        &idea,
        "body",
    )
    .unwrap();
    assert!(phase_prompt.contains("\"kind\":\"task\",\"task\":\"sync_docs_after_refinement\""));

    let transition_state = &plan.states["state_7_implementation_started"];
    let transition_task = transition_state
        .tasks
        .iter()
        .max_by_key(|task| task.phase)
        .expect("transition state should have tasks");
    let transition_run = run(&plan, &idea, &transition_state.id);
    let transition_prompt = finalize_task_prompt_v1(
        &plan,
        &transition_run,
        transition_state,
        transition_task,
        &idea,
        "body",
    )
    .unwrap();
    assert!(transition_prompt.contains("\"kind\":\"transition\""));
    assert!(transition_prompt.contains("\"target_state_id\":\"state_8_implementation_continued\""));
}

#[test]
fn consumer_grammar_covers_multi_transition_owner_and_terminal_shapes() {
    let plan = compile_plan();
    let idea = idea(
        "Consumer grammar".into(),
        "Preserve every downstream consumer shape.".into(),
    );

    let multi = plan
        .states
        .values()
        .find(|state| state.transitions.len() > 1)
        .expect("active workflow should contain a multi-transition state");
    let multi_run = run(&plan, &idea, &multi.id);
    let multi_prompt = if let Some(task) = multi.tasks.iter().max_by_key(|task| task.phase) {
        finalize_task_prompt_v1(&plan, &multi_run, multi, task, &idea, "multi body").unwrap()
    } else {
        finalize_owner_prompt_v1(&plan, &multi_run, multi, &idea, "multi body").unwrap()
    };
    let multi_context = extract_mission_context(&multi_prompt);
    let consumers = multi_context["assignment"]["consumers"]
        .as_array()
        .expect("multi-transition consumers must be an array");
    assert_eq!(consumers.len(), multi.transitions.len());
    for transition in &multi.transitions {
        assert!(consumers.iter().any(|consumer| {
            consumer["kind"] == "transition"
                && consumer["target_state_id"] == transition.to
                && consumer["when"] == transition.condition
        }));
    }

    let owner_only = plan
        .states
        .values()
        .find(|state| state.tasks.is_empty() && !state.is_end)
        .expect("active workflow should contain owner-only work");
    let owner_run = run(&plan, &idea, &owner_only.id);
    let owner_context = extract_mission_context(
        &finalize_owner_prompt_v1(&plan, &owner_run, owner_only, &idea, "owner body").unwrap(),
    );
    assert_eq!(owner_context["assignment"]["kind"], "state_owner");
    assert_eq!(
        owner_context["assignment"]["consumers"]
            .as_array()
            .unwrap()
            .len(),
        owner_only.transitions.len()
    );

    let mut terminal = plan
        .states
        .values()
        .find(|state| state.is_end)
        .expect("active workflow should contain a terminal state")
        .clone();
    terminal.transitions.clear();
    let terminal_run = run(&plan, &idea, &terminal.id);
    let terminal_context = extract_mission_context(
        &finalize_owner_prompt_v1(&plan, &terminal_run, &terminal, &idea, "terminal body").unwrap(),
    );
    assert_eq!(terminal_context["assignment"]["kind"], "state_owner");
    assert_eq!(
        terminal_context["assignment"]["consumers"],
        serde_json::json!([])
    );
}

#[test]
fn owner_prompt_uses_state_owner_assignment_and_copy_validation_rejects_duplicates() {
    let plan = compile_plan();
    let state = plan
        .states
        .values()
        .find(|state| state.tasks.is_empty() && !state.is_end)
        .expect("owner-only state should exist");
    let idea = idea(
        "Owner test".into(),
        "Preserve exact mission context.".into(),
    );
    let run = run(&plan, &idea, &state.id);

    let prompt = finalize_owner_prompt_v1(&plan, &run, state, &idea, "owner body").unwrap();
    assert!(prompt.contains("\"kind\":\"state_owner\""));
    assert!(!prompt.contains("\"declared_outputs\""));

    let duplicate = format!("{prompt}\n\n{prompt}");
    let error = validate_persisted_v1_prompt(&plan, &duplicate)
        .expect_err("copy prompt with duplicate mission blocks must fail")
        .to_string();
    assert!(error.contains("exactly one"));

    let missing_prompt = serde_json::json!({});
    let error = validate_persisted_v1_payload_prompt(&plan, &missing_prompt)
        .expect_err("V1 copied payload without prompt must fail")
        .to_string();
    assert!(error.contains("no persisted prompt"));
}

#[test]
fn dynamic_post_approval_and_mediation_assignments_use_the_common_finalizer() {
    let plan = compile_plan();
    let idea = idea(
        "Assignment coverage".into(),
        "Keep every fresh provider prompt on one finalizer.".into(),
    );

    let dynamic_state = plan
        .states
        .values()
        .find(|state| state.dynamic_parallel.is_some())
        .expect("active workflow should contain dynamic parallel review");
    let binding = plan
        .dynamic_candidate_bindings
        .first()
        .expect("active catalog should freeze dynamic candidates");
    let dynamic_agent: workflow::plan::ResolvedAgent =
        serde_json::from_str(&binding.resolved_agent_snapshot_json).unwrap();
    assert!(dynamic_agent.permission_profile.is_some());
    assert!(dynamic_agent.skill_ref.is_some());
    assert!(dynamic_agent.skill_snapshot_hash.is_some());
    let dynamic_task = CompiledTask {
        agent: dynamic_agent,
        task_name: "dynamic_review_fixture".into(),
        inputs: vec!["proposal_current".into()],
        outputs: vec!["proposal_review_dynamic".into()],
        output_policies: std::collections::HashMap::new(),
        output_schemas: std::collections::HashMap::new(),
        parallel: true,
        phase: 0,
        selected_outputs_from: None,
    };
    let dynamic_run = run(&plan, &idea, &dynamic_state.id);
    let dynamic_prompt = finalize_task_prompt_v1(
        &plan,
        &dynamic_run,
        dynamic_state,
        &dynamic_task,
        &idea,
        "dynamic body",
    )
    .unwrap();
    assert!(dynamic_prompt.contains("\"origin\":\"dynamic_parallel\""));
    assert_eq!(dynamic_prompt.matches("## Mission Context").count(), 1);
    let dynamic_context = extract_mission_context(&dynamic_prompt);
    let next_phase = dynamic_state
        .tasks
        .iter()
        .map(|task| task.phase)
        .filter(|phase| *phase > dynamic_task.phase)
        .min()
        .expect("dynamic dispatch should hand off to a declared next-phase task");
    let expected_next_tasks = dynamic_state
        .tasks
        .iter()
        .filter(|task| task.phase == next_phase)
        .collect::<Vec<_>>();
    let dynamic_consumers = dynamic_context["assignment"]["consumers"]
        .as_array()
        .unwrap();
    assert_eq!(
        dynamic_consumers.len(),
        expected_next_tasks.len(),
        "dynamic work must name the immediate declared consumers"
    );
    for task in expected_next_tasks {
        assert!(dynamic_consumers.iter().any(|consumer| {
            consumer["kind"] == "task"
                && consumer["task"] == task.task_name
                && consumer["agent_id"] == task.agent.agent_id
        }));
    }

    let (post_state, post_task) = plan
        .states
        .values()
        .find_map(|state| state.post_approval_tasks.first().map(|task| (state, task)))
        .expect("active workflow should contain a post-approval task");
    let post_run = run(&plan, &idea, &post_state.id);
    let post_prompt =
        finalize_task_prompt_v1(&plan, &post_run, post_state, post_task, &idea, "post body")
            .unwrap();
    assert!(post_prompt.contains("\"origin\":\"static\""));

    let lead_state = plan
        .states
        .values()
        .find(|state| state.owner.agent_id == "lead_orchestrator")
        .expect("system lead should own a state");
    let mediation_run = run(&plan, &idea, &lead_state.id);
    for origin in ["p017_conflict_mediation", "p058_lead_mediation"] {
        let prompt = finalize_mediation_prompt_v1(
            &plan,
            &mediation_run,
            lead_state,
            &lead_state.owner,
            &idea,
            origin,
            "conflict-or-ledger-id",
            "LeadResolutionContract",
            "mediate the frozen evidence",
        )
        .unwrap();
        assert!(prompt.contains(&format!("\"origin\":\"{origin}\"")));
        assert!(prompt.contains("\"kind\":\"mediation\""));
        assert_eq!(prompt.matches("## Mission Context").count(), 1);
    }
}

#[test]
fn active_catalog_preserves_procedure_kinds_and_does_not_duplicate_bundle_body_in_prompts() {
    let plan = compile_plan();
    let mut kinds = std::collections::BTreeSet::new();
    for state in plan.states.values() {
        for agent in std::iter::once(&state.owner).chain(
            state
                .tasks
                .iter()
                .chain(&state.post_approval_tasks)
                .map(|task| &task.agent),
        ) {
            kinds.insert(
                agent
                    .resolved_skill
                    .as_ref()
                    .map(|skill| skill.skill_type.as_str())
                    .unwrap_or("none"),
            );
            if matches!(
                agent.skill_ref.as_deref(),
                Some(
                    "proposal_review_router_skill"
                        | "code_writer_core"
                        | "proposal_implementation_audit"
                        | "security_checker_core"
                        | "prepush_review_core"
                )
            ) {
                let skill = agent.resolved_skill.as_ref().unwrap();
                assert_eq!(skill.skill_type, "external");
                assert!(!agent
                    .prompt
                    .as_deref()
                    .unwrap_or_default()
                    .contains(&skill.injected_content));
            }
        }
    }
    assert!(kinds.contains("external"));
    assert!(kinds.contains("inline"));
    assert!(kinds.contains("builtin"));

    let (state, task) = task_for_agent(&plan, "code_writer");
    let mut no_skill_task = task.clone();
    no_skill_task.agent.skill_ref = None;
    no_skill_task.agent.skill_role = None;
    no_skill_task.agent.skill_snapshot_hash = None;
    no_skill_task.agent.resolved_skill = None;
    let idea = idea("No skill".into(), "Exercise the closed none arm.".into());
    let run = run(&plan, &idea, &state.id);
    let prompt =
        finalize_task_prompt_v1(&plan, &run, state, &no_skill_task, &idea, "body").unwrap();
    assert!(prompt.contains("\"procedure\":{\"kind\":\"none\"}"));
}

#[test]
fn run_preflight_enforces_exact_idea_bound_without_writes() {
    let plan = compile_plan();
    let exact = idea("".into(), "x".repeat(MAX_IDEA_CONTEXT_BYTES));
    preflight_run_mission_context(&plan, RunId::new(), &exact)
        .expect("exact Idea limit should pass");

    let oversized = idea("".into(), "x".repeat(MAX_IDEA_CONTEXT_BYTES + 1));
    let error = preflight_run_mission_context(&plan, RunId::new(), &oversized)
        .expect_err("Idea above the limit must fail")
        .to_string();
    assert!(error.contains("mission_context_input_too_large"));
}

async fn test_pool() -> SqlitePool {
    let pool = create_pool("sqlite::memory:")
        .await
        .expect("in-memory pool should open");
    let writer = std::sync::Arc::new(db::writer::DbWriter::new(pool.clone()));
    db::writer::register_shared_writer(&pool, writer)
        .await
        .expect("shared test writer should register");
    pool
}

fn start_run_fixture(root: &std::path::Path, idea_id: IdeaId) -> Command {
    let workflow = root.join("workflow.yaml");
    let catalog = root.join("catalog.yaml");
    std::fs::write(
        &workflow,
        r#"
workflow:
  id: mission-start-test
  family: mission_start_test
initial_state: done
states:
  done:
    label: Done
    type: end
    owner: lead
"#,
    )
    .unwrap();
    std::fs::write(
        &catalog,
        r#"
schema_version: 1
permission_profiles:
  ORCH: {}
contracts:
  LeadResolutionContract:
    format: json
    required_fields: [resolution_mode]
backend_profiles:
  test:
    provider: codex
    model: test-model
agents:
  - id: lead
    system_role: lead
    backend_profile: test
    permission_profile: ORCH
    lead_resolution_contract: LeadResolutionContract
    prompt: Lead the run.
"#,
    )
    .unwrap();
    Command::StartRun(StartRunCmd {
        idea_id,
        workflow_id: "mission-start-test".into(),
        workflow_title: "Mission start test".into(),
        workspace_root: root.to_string_lossy().into_owned(),
        artifact_root: root.join("artifacts").to_string_lossy().into_owned(),
        delivery_configuration_json: None,
        workflow_yaml_path: workflow.to_string_lossy().into_owned(),
        agent_catalog_yaml_path: catalog.to_string_lossy().into_owned(),
        review_routing_json: None,
        rollout_contract_preflight_policy_json: None,
        closeout_readiness_mode: None,
    })
}

#[tokio::test]
async fn production_start_run_rejects_missing_or_oversized_idea_before_run_and_work_insert() {
    let pool = test_pool().await;
    let handler = CommandHandler::new(
        pool.clone(),
        event_bus::new_bus(32),
        WorkQueue::new(pool.clone()),
    );
    let root = tempfile::tempdir().unwrap();
    let oversized = idea("".into(), "x".repeat(MAX_IDEA_CONTEXT_BYTES + 1));
    ideas::insert(&pool, &oversized).await.unwrap();

    let error = handler
        .handle(
            start_run_fixture(root.path(), oversized.id),
            CallerContext::mcp("operator", &PrincipalClass::Operator, "runs.start"),
        )
        .await
        .err()
        .expect("oversized Idea must fail StartRun")
        .to_string();
    assert!(error.contains("mission_context_input_too_large"));
    assert!(runs::list_by_idea(&pool, oversized.id)
        .await
        .unwrap()
        .is_empty());

    let missing_id = IdeaId::new();
    let error = handler
        .handle(
            start_run_fixture(root.path(), missing_id),
            CallerContext::mcp("operator", &PrincipalClass::Operator, "runs.start"),
        )
        .await
        .err()
        .expect("missing Idea must fail StartRun")
        .to_string();
    assert!(error.contains("not found"));
    assert!(runs::list_by_idea(&pool, missing_id)
        .await
        .unwrap()
        .is_empty());

    let work_items: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM work_items")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(work_items, 0, "failed preflight must enqueue no work");
}

#[test]
fn persisted_snapshot_quartet_fails_closed_on_hash_mismatch_or_partial_presence() {
    let plan = compile_plan();
    let idea = idea("Snapshot test".into(), "Use only frozen bytes.".into());
    let mut persisted = run(&plan, &idea, "state_0_idea_submitted");
    persisted.catalog_snapshot_hash = Some("0".repeat(64));
    let error = compile_run_plan_for_run(&persisted)
        .expect_err("catalog hash mismatch must fail closed")
        .to_string();
    assert!(
        error.contains("stored snapshot digest mismatch"),
        "unexpected error: {error}"
    );

    persisted.catalog_snapshot_hash = None;
    let error = compile_run_plan_for_run(&persisted)
        .expect_err("partial snapshot quartet must fail closed")
        .to_string();
    assert!(
        error.contains("complete JSON/hash quartet"),
        "unexpected error: {error}"
    );
}

#[test]
fn persisted_snapshot_quartet_exhausts_all_presence_states_and_never_falls_back() {
    let plan = compile_plan();
    let idea = idea(
        "Snapshot matrix".into(),
        "Use authenticated bytes only.".into(),
    );
    let base = run(&plan, &idea, "state_0_idea_submitted");

    for mask in 0u8..16 {
        let mut persisted = base.clone();
        persisted.workflow_snapshot_json =
            (mask & 0b0001 != 0).then(|| plan.workflow_snapshot_json.clone());
        persisted.workflow_snapshot_hash =
            (mask & 0b0010 != 0).then(|| plan.workflow_snapshot_hash.clone());
        persisted.catalog_snapshot_json =
            (mask & 0b0100 != 0).then(|| plan.catalog_snapshot_json.clone());
        persisted.catalog_snapshot_hash =
            (mask & 0b1000 != 0).then(|| plan.catalog_snapshot_hash.clone());

        match mask {
            0 => assert!(
                compile_run_plan_for_run(&persisted).unwrap().is_none(),
                "all-absent legacy state should not synthesize a plan"
            ),
            15 => assert!(
                compile_run_plan_for_run(&persisted).unwrap().is_some(),
                "complete authenticated quartet should compile"
            ),
            _ => {
                persisted.workflow_yaml_path = Some("/must/not/be-read/workflow.yaml".into());
                persisted.agent_catalog_yaml_path = Some("/must/not-be-read/catalog.yaml".into());
                let error = compile_run_plan_for_run(&persisted)
                    .expect_err("every partial quartet must fail closed")
                    .to_string();
                assert!(
                    error.contains("complete JSON/hash quartet"),
                    "mask {mask:04b} unexpectedly reached live files: {error}"
                );
            }
        }
    }

    let mut malformed = base;
    malformed.workflow_snapshot_json = Some("{".into());
    malformed.workflow_snapshot_hash = Some(format!("{:x}", Sha256::digest(b"{")));
    let error = compile_run_plan_for_run(&malformed)
        .expect_err("hash-valid malformed snapshot JSON must fail")
        .to_string();
    assert!(
        error.contains("snapshot") || error.contains("JSON") || error.contains("EOF"),
        "unexpected malformed snapshot error: {error}"
    );
}

#[test]
fn legacy_prompt_validation_preserves_pre_v1_bytes_exactly() {
    let mut plan = compile_plan();
    plan.mission_context_version = None;
    let legacy_prompt = "legacy prompt bytes\nwithout a mission block";

    validate_persisted_v1_prompt(&plan, legacy_prompt)
        .expect("legacy prompt bytes must remain valid without V1 injection");
    let payload = serde_json::json!({"prompt": legacy_prompt});
    validate_persisted_v1_payload_prompt(&plan, &payload)
        .expect("legacy copy validation must not rewrite prompt bytes");
    assert_eq!(payload["prompt"], legacy_prompt);
}

#[test]
fn legacy_flat_producer_rejects_workflow_or_snapshot_backed_runs() {
    let plan = compile_plan();
    let idea = idea(
        "Legacy boundary".into(),
        "Keep V1 out of flat orchestration.".into(),
    );
    let mut legacy = run(&plan, &idea, "legacy");
    legacy.workflow_snapshot_json = None;
    legacy.workflow_snapshot_hash = None;
    legacy.catalog_snapshot_json = None;
    legacy.catalog_snapshot_hash = None;
    validate_legacy_flat_invoke_agent(&legacy).expect("fully legacy flat run should pass");

    legacy.workflow_yaml_path = Some("workflow.yaml".into());
    assert!(validate_legacy_flat_invoke_agent(&legacy).is_err());
    legacy.workflow_yaml_path = None;
    legacy.catalog_snapshot_json = Some("{}".into());
    assert!(validate_legacy_flat_invoke_agent(&legacy).is_err());
}
