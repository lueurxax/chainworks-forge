use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use workflow::compiler;

static TEMP_FIXTURE_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn fixtures_dir() -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    format!("{manifest}/../../../examples")
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn write_temp_fixture(filename: &str, content: &str) -> String {
    let unique = TEMP_FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "workflow_contract_slice_{}_{}",
        std::process::id(),
        unique
    ));
    fs::create_dir_all(&dir).expect("should create temp fixture directory");
    let path = PathBuf::from(&dir).join(filename);
    fs::write(&path, content).expect("should write temp fixture");
    path.to_string_lossy().into_owned()
}

fn compile_result_from_strings(
    workflow_yaml: &str,
    catalog_yaml: &str,
) -> anyhow::Result<workflow::plan::RunPlan> {
    let workflow_yaml = if workflow_yaml.trim_start().starts_with("workflow:") {
        workflow_yaml.to_string()
    } else {
        format!("workflow:\n  id: contract-fixture\n  family: contract_fixture\n{workflow_yaml}")
    };
    let wf_path = write_temp_fixture("workflow.yaml", &workflow_yaml);
    let cat_path = write_temp_fixture("catalog.yaml", catalog_yaml);
    compiler::compile(&wf_path, &cat_path)
}

fn compile_from_strings(workflow_yaml: &str, catalog_yaml: &str) -> workflow::plan::RunPlan {
    compile_result_from_strings(workflow_yaml, catalog_yaml).expect("should compile plan")
}

fn compile_error_from_strings(workflow_yaml: &str, catalog_yaml: &str) -> String {
    let workflow_yaml = if workflow_yaml.trim_start().starts_with("workflow:") {
        workflow_yaml.to_string()
    } else {
        format!("workflow:\n  id: contract-fixture\n  family: contract_fixture\n{workflow_yaml}")
    };
    let wf_path = write_temp_fixture("workflow.yaml", &workflow_yaml);
    let cat_path = write_temp_fixture("catalog.yaml", catalog_yaml);
    compiler::compile(&wf_path, &cat_path)
        .expect_err("plan should fail direct-command lint")
        .to_string()
}

#[test]
fn test_parse_full_mvp_live_workflow() {
    let wf_path = format!("{}/workflows/full-mvp-live.yaml", fixtures_dir());
    let wf = workflow::definition::load(&wf_path).expect("should parse workflow YAML");

    assert_eq!(wf.initial_state, "state_1_idea_received");
    assert_eq!(wf.states.len(), 12, "full-mvp-live has 12 states");

    // Verify state types
    let s1 = &wf.states["state_1_idea_received"];
    assert!(s1.is_start());
    assert_eq!(s1.owner, "lead_orchestrator");

    let s3 = &wf.states["state_3_initial_proposal_approval"];
    assert!(s3.is_manual_gate());

    let s12 = &wf.states["state_12_workflow_complete"];
    assert!(s12.is_end());

    // Verify a state with parallel tasks
    let s4 = &wf.states["state_4_proposal_reviewed"];
    let run = s4.run.as_ref().expect("state_4 has a run block");
    assert!(run.parallel.is_some(), "state_4 has parallel reviewers");
    let parallel = run.parallel.as_ref().unwrap();
    assert_eq!(parallel.len(), 4, "4 parallel reviewers");

    // Verify loop config
    let s5 = &wf.states["state_5_proposal_refined"];
    let lc = s5.loop_config.as_ref().expect("state_5 has loop config");
    assert_eq!(lc.counter, "proposal_revision_count");
}

#[test]
fn test_parse_agent_catalog() {
    let cat_path = format!("{}/agents/agents.yaml", fixtures_dir());
    let cat = workflow::catalog::load(&cat_path).expect("should parse agent catalog YAML");

    let profiles = cat.backend_profiles.as_ref().expect("has backend_profiles");
    assert!(profiles.contains_key("claude_orchestrator_high"));
    assert!(profiles.contains_key("codex_builder_high"));
    assert!(profiles.contains_key("gemini_review_pro"));

    let agents = cat.agents.as_ref().expect("has agents");
    let agent_ids: Vec<&str> = agents.iter().map(|a| a.id.as_str()).collect();
    assert!(agent_ids.contains(&"lead_orchestrator"));
    assert!(agent_ids.contains(&"code_writer"));
    assert!(agent_ids.contains(&"proposal_writer"));
    assert!(agent_ids.contains(&"proposal_reviewer_ux"));

    let proposal_writer = agents
        .iter()
        .find(|agent| agent.id == "proposal_writer")
        .expect("proposal_writer in catalog");
    assert_eq!(
        proposal_writer.session_reuse_scope.as_deref(),
        Some("same_agent_family_within_run")
    );
    assert_eq!(
        proposal_writer.session_family_id.as_deref(),
        Some("proposal_authoring_loop")
    );
}

#[test]
fn p051_example_catalogs_explicitly_mark_xcode_required_tools_for_host_execution() {
    for relative_path in ["agents/agents.yaml"] {
        let cat_path = format!("{}/{}", fixtures_dir(), relative_path);
        let cat = workflow::catalog::load(&cat_path).expect("should parse agent catalog YAML");
        let agents = cat.agents.as_ref().expect("has agents");
        let missing: Vec<String> = agents
            .iter()
            .filter(|agent| {
                let has_xcode_required_tool = agent
                    .required_tools
                    .as_ref()
                    .map(|tools| tools.iter().any(|tool| declares_xcode_host_tool(tool)))
                    .unwrap_or(false);
                has_xcode_required_tool && agent.requires_xcode_host_execution != Some(true)
            })
            .map(|agent| agent.id.clone())
            .collect();

        assert!(
            missing.is_empty(),
            "{relative_path} agents with Xcode required_tools must explicitly set requires_xcode_host_execution: true: {missing:?}"
        );
    }
}

#[test]
fn p051_dogfood_workflow_runs_parallel_gemini_xcode_lanes() {
    let root = repo_root();
    let wf_path = root
        .join("docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-workflow.yaml")
        .to_string_lossy()
        .into_owned();
    let cat_path = root
        .join("docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-agents.yaml")
        .to_string_lossy()
        .into_owned();

    let plan = compiler::compile(&wf_path, &cat_path).expect("should compile P051 dogfood plan");
    let review_state = &plan.states["state_2_parallel_gemini_xcode_review"];

    assert_eq!(
        review_state.tasks.len(),
        2,
        "dogfood review fan-out is parallel"
    );
    for agent_id in ["p051_gemini_ux_xcode", "p051_gemini_ui_xcode"] {
        let task = review_state
            .tasks
            .iter()
            .find(|task| task.agent.agent_id == agent_id)
            .unwrap_or_else(|| panic!("missing {agent_id} reviewer task"));

        assert_eq!(task.agent.provider, "gemini", "{agent_id} uses Gemini");
        assert_eq!(
            task.agent.requested_mcp_server_ids,
            vec!["xcode".to_string()],
            "{agent_id} requests only brokered Xcode MCP"
        );
        assert!(
            task.agent.xcode_broker_required,
            "{agent_id} is marked as requiring the Xcode broker"
        );
    }
}

fn declares_xcode_host_tool(command: &str) -> bool {
    ["xcodebuild", "simctl", "xcrun"]
        .iter()
        .any(|tool| command.contains(tool))
}

#[test]
fn test_compile_full_mvp_live_plan() {
    let wf_path = format!("{}/workflows/full-mvp-live.yaml", fixtures_dir());
    let cat_path = format!("{}/agents/agents.yaml", fixtures_dir());

    let plan = compiler::compile(&wf_path, &cat_path).expect("should compile plan");

    assert_eq!(plan.initial_state, "state_1_idea_received");
    assert_eq!(plan.states.len(), 12);

    // Verify provider resolution
    let s1 = &plan.states["state_1_idea_received"];
    assert_eq!(s1.owner.agent_id, "lead_orchestrator");
    assert_eq!(s1.owner.provider, "claude", "lead_orchestrator uses claude");

    let s4 = &plan.states["state_4_proposal_reviewed"];
    assert_eq!(
        s4.owner.provider, "claude",
        "state_4 owner=lead_orchestrator → claude"
    );
    // Parallel tasks should have mixed providers
    let ux_task = s4
        .tasks
        .iter()
        .find(|t| t.agent.agent_id == "proposal_reviewer_ux");
    assert!(ux_task.is_some(), "should have UX reviewer task");
    assert_eq!(
        ux_task.unwrap().agent.provider,
        "gemini",
        "UX reviewer uses gemini"
    );

    let arch_task = s4
        .tasks
        .iter()
        .find(|t| t.agent.agent_id == "proposal_reviewer_architect");
    assert!(arch_task.is_some(), "should have architect reviewer task");
    assert_eq!(
        arch_task.unwrap().agent.provider,
        "codex",
        "architect uses codex"
    );
    assert_eq!(
        arch_task.unwrap().agent.requested_mcp_server_ids,
        vec!["xcode".to_string(), "context7".to_string()],
        "architect MCP intent comes from codex_architect_high backend_profile"
    );

    // Verify code_writer → codex
    let s7 = &plan.states["state_7_implementation_started"];
    let cw_task = s7.tasks.iter().find(|t| t.agent.agent_id == "code_writer");
    assert!(cw_task.is_some(), "state_7 should have code_writer task");
    assert_eq!(cw_task.unwrap().agent.provider, "codex");
    assert_eq!(
        cw_task.unwrap().agent.requested_mcp_server_ids,
        vec!["context7".to_string(), "xcode".to_string()],
        "code_writer MCP intent comes from codex_builder_high backend_profile"
    );

    let proposal_writer = &plan.states["state_2_proposal_drafted"].owner;
    assert_eq!(
        proposal_writer.requested_mcp_server_ids,
        vec!["xcode".to_string(), "context7".to_string()],
        "proposal_writer MCP intent comes from codex_writer_high backend_profile"
    );
    assert_eq!(
        proposal_writer.session_reuse_scope.as_deref(),
        Some("same_agent_family_within_run")
    );
    assert_eq!(
        proposal_writer.session_family_id.as_deref(),
        Some("proposal_authoring_loop")
    );

    // Verify manual gates
    let s3 = &plan.states["state_3_initial_proposal_approval"];
    assert!(s3.is_manual_gate);
    let s6 = &plan.states["state_6_implementation_approval"];
    assert!(s6.is_manual_gate);
    let s11 = &plan.states["state_11_manual_release"];
    assert!(s11.is_manual_gate);

    // Verify end state
    let s12 = &plan.states["state_12_workflow_complete"];
    assert!(s12.is_end);

    // Verify loop resolution
    let s5 = &plan.states["state_5_proposal_refined"];
    let lc = s5.loop_config.as_ref().expect("state_5 has loop config");
    assert_eq!(lc.counter, "proposal_revision_count");
    assert_eq!(lc.max, 15, "max should be resolved from vars");

    // Verify variables were loaded
    assert!(plan.variables.contains_key("proposal_score_target"));
}

#[test]
fn p051_catalog_lint_sets_xcode_signals_from_mcp_and_declared_commands() {
    let plan = compile_from_strings(
        r#"
initial_state: start
states:
  start:
    label: Start
    type: end
    owner: builder
    run:
      sequence:
        - agent: builder
          task: build
"#,
        r#"
backend_profiles:
  builder_profile:
    provider: codex_acp
    model: gpt-5.4
    mcp:
      - xcode
permission_profiles:
  CODE:
    shell:
      allow:
        - xcodebuild -project "Chainworks Forge.xcodeproj" build
agents:
  - id: builder
    backend_profile: builder_profile
    permission_profile: CODE
    required_tools:
      - simctl list devices
    prompt: "Build through the declared commands."
"#,
    );

    let agent = &plan.states["start"].owner;
    assert!(
        agent.xcode_broker_required,
        "requesting the xcode MCP server must require brokered MCP"
    );
    assert!(
        agent.xcode_shim_injection_signal,
        "declared xcodebuild/simctl commands must request shim injection"
    );
    assert!(
        agent.requires_xcode_host_execution,
        "declared Xcode shell commands must be marked for host execution"
    );
}

#[test]
fn p051_catalog_lint_rejects_structured_absolute_xcode_paths() {
    let error = compile_error_from_strings(
        r#"
initial_state: start
states:
  start:
    label: Start
    type: end
    owner: builder
"#,
        r#"
backend_profiles:
  builder_profile:
    provider: codex_acp
    model: gpt-5.4
agents:
  - id: builder
    backend_profile: builder_profile
    required_tools:
      - /usr/bin/xcodebuild -project "Chainworks Forge.xcodeproj" build
"#,
    );

    assert!(
        error.contains("P051 direct-command catalog lint failed"),
        "compile should fail through the P051 scanner: {error}"
    );
    assert!(
        error.contains("p051_absolute_xcode_tool_path"),
        "absolute xcodebuild path should be rejected: {error}"
    );
}

#[test]
fn p051_catalog_lint_rejects_unknown_xcrun_flags() {
    let error = compile_error_from_strings(
        r#"
initial_state: start
states:
  start:
    label: Start
    type: end
    owner: builder
"#,
        r#"
backend_profiles:
  builder_profile:
    provider: codex_acp
    model: gpt-5.4
agents:
  - id: builder
    backend_profile: builder_profile
    required_tools:
      - xcrun --diagnose simctl list devices
"#,
    );

    assert!(
        error.contains("P051 direct-command catalog lint failed"),
        "compile should fail through the P051 scanner: {error}"
    );
    assert!(
        error.contains("p051_xcrun_unknown_flag"),
        "unknown xcrun flags should be rejected: {error}"
    );
}

#[test]
fn p051_catalog_lint_allows_proposal_non_consuming_xcrun_flags() {
    compile_from_strings(
        r#"
initial_state: start
states:
  start:
    label: Start
    type: end
    owner: builder
"#,
        r#"
backend_profiles:
  builder_profile:
    provider: codex_acp
    model: gpt-5.4
agents:
  - id: builder
    backend_profile: builder_profile
    required_tools:
      - xcrun --verbose --log simctl list devices
      - xcrun --no-cache --kill-cache xcodebuild -version
      - xcrun --show-sdk-platform-path --sdk iphoneos
      - xcrun -l swift --version
      - xcrun --help
      - xcrun -h
      - xcrun --version
"#,
    );
}

#[test]
fn p051_workflow_run_task_sets_xcode_signals_for_invoked_agent() {
    let plan = compile_from_strings(
        r#"
initial_state: start
states:
  start:
    label: Start
    type: end
    owner: builder
    run:
      sequence:
        - agent: builder
          task: xcodebuild -project "Chainworks Forge.xcodeproj" build
"#,
        r#"
backend_profiles:
  builder_profile:
    provider: codex_acp
    model: gpt-5.4
agents:
  - id: builder
    backend_profile: builder_profile
    prompt: "Build from workflow task instructions."
"#,
    );

    let agent = &plan.states["start"].owner;
    assert!(
        agent.xcode_shim_injection_signal,
        "workflow run-block Xcode commands must request shim injection"
    );
    assert!(
        agent.requires_xcode_host_execution,
        "workflow run-block Xcode commands must route through host execution"
    );
}

#[test]
fn p051_workflow_run_task_rejects_direct_mcpbridge_bypass() {
    let error = compile_error_from_strings(
        r#"
initial_state: start
states:
  start:
    label: Start
    type: end
    owner: builder
    run:
      sequence:
        - agent: builder
          task: xcrun mcpbridge
"#,
        r#"
backend_profiles:
  builder_profile:
    provider: codex_acp
    model: gpt-5.4
agents:
  - id: builder
    backend_profile: builder_profile
    prompt: "Build from workflow task instructions."
"#,
    );

    assert!(
        error.contains("states.start.run.sequence[0].task"),
        "workflow run-block task path should be reported: {error}"
    );
    assert!(
        error.contains("p051_direct_mcpbridge_command"),
        "direct mcpbridge use should fail closed: {error}"
    );
}

#[test]
fn p051_agent_adapter_raw_commands_set_xcode_signals_for_agent() {
    let plan = compile_from_strings(
        r#"
initial_state: start
states:
  start:
    label: Start
    type: end
    owner: builder
"#,
        r#"
backend_profiles:
  builder_profile:
    provider: codex_acp
    model: gpt-5.4
agents:
  - id: builder
    backend_profile: builder_profile
    prompt: "Build from adapter-specific launch instructions."
    codex:
      launch:
        commands:
          - xcodebuild -project "Chainworks Forge.xcodeproj" build
"#,
    );

    let agent = &plan.states["start"].owner;
    assert!(
        agent.xcode_shim_injection_signal,
        "adapter-specific raw Xcode commands must request shim injection"
    );
    assert!(
        agent.requires_xcode_host_execution,
        "adapter-specific raw Xcode commands must route through host execution"
    );
}

#[test]
fn p051_agent_adapter_raw_commands_reject_direct_mcpbridge_bypass() {
    let error = compile_error_from_strings(
        r#"
initial_state: start
states:
  start:
    label: Start
    type: end
    owner: builder
"#,
        r#"
backend_profiles:
  builder_profile:
    provider: codex_acp
    model: gpt-5.4
agents:
  - id: builder
    backend_profile: builder_profile
    prompt: "Build from adapter-specific launch instructions."
    gemini:
      session_new:
        args:
          - xcrun mcpbridge
"#,
    );

    assert!(
        error.contains("agents.builder.gemini.session_new.args[0]"),
        "adapter-specific raw command path should include the owning agent id: {error}"
    );
    assert!(
        error.contains("p051_direct_mcpbridge_command"),
        "adapter-specific direct mcpbridge use should fail closed: {error}"
    );
}

#[test]
fn p051_agent_allowed_commands_set_xcode_signals_for_agent() {
    let plan = compile_from_strings(
        r#"
initial_state: start
states:
  start:
    label: Start
    type: end
    owner: builder
"#,
        r#"
backend_profiles:
  builder_profile:
    provider: codex_acp
    model: gpt-5.4
agents:
  - id: builder
    backend_profile: builder_profile
    prompt: "Build from declared allowed commands."
    allowed_commands:
      - xcodebuild -project "Chainworks Forge.xcodeproj" build
"#,
    );

    let agent = &plan.states["start"].owner;
    assert!(
        agent.xcode_shim_injection_signal,
        "allowed_commands Xcode declarations must request shim injection"
    );
    assert!(
        agent.requires_xcode_host_execution,
        "allowed_commands Xcode declarations must route through host execution"
    );
}

#[test]
fn p051_workflow_state_raw_shell_commands_set_xcode_signals_for_owner() {
    let plan = compile_from_strings(
        r#"
initial_state: start
states:
  start:
    label: Start
    type: end
    owner: builder
    tools:
      shell:
        commands:
          - xcodebuild -project "Chainworks Forge.xcodeproj" build
"#,
        r#"
backend_profiles:
  builder_profile:
    provider: codex_acp
    model: gpt-5.4
agents:
  - id: builder
    backend_profile: builder_profile
    prompt: "Build from state-owned shell commands."
"#,
    );

    let agent = &plan.states["start"].owner;
    assert!(
        agent.xcode_shim_injection_signal,
        "state-owned raw workflow shell commands must request shim injection"
    );
    assert!(
        agent.requires_xcode_host_execution,
        "state-owned raw workflow shell commands must route through host execution"
    );
}

#[test]
fn p051_workflow_state_raw_shell_commands_reject_direct_mcpbridge_bypass() {
    let error = compile_error_from_strings(
        r#"
initial_state: start
states:
  start:
    label: Start
    type: end
    owner: builder
    tools:
      shell:
        commands:
          - xcrun mcpbridge
"#,
        r#"
backend_profiles:
  builder_profile:
    provider: codex_acp
    model: gpt-5.4
agents:
  - id: builder
    backend_profile: builder_profile
    prompt: "Build from state-owned shell commands."
"#,
    );

    assert!(
        error.contains("states.start.tools.shell.commands[0]"),
        "state-owned raw workflow command path should be reported: {error}"
    );
    assert!(
        error.contains("p051_direct_mcpbridge_command"),
        "state-owned direct mcpbridge use should fail closed: {error}"
    );
}

#[test]
fn steward_metadata_contract_tests_freeze_workflow_metadata_and_parsed_snapshots() {
    let plan = compile_from_strings(
        r#"
workflow:
  id: steward-workflow
  name: Steward Workflow
  family: mvp_live
  risk_class: high
  stack: swiftui
variables:
  max_iterations: 2
initial_state: start
states:
  start:
    label: Start
    type: end
    owner: steward
"#,
        r#"
backend_profiles:
  steward_profile:
    provider: claude
    model: steward-model
agents:
  - id: steward
    backend_profile: steward_profile
    prompt: "observe"
"#,
    );

    assert_eq!(plan.workflow_family.as_deref(), Some("mvp_live"));
    assert_eq!(plan.risk_class.as_deref(), Some("high"));
    assert_eq!(plan.stack.as_deref(), Some("swiftui"));
    assert!(
        plan.workflow_snapshot_hash.len() == 64,
        "workflow snapshot hash must be a hex sha256"
    );
    assert!(
        plan.catalog_snapshot_hash.len() == 64,
        "catalog snapshot hash must be a hex sha256"
    );
    assert!(
        plan.workflow_snapshot_json
            .contains("\"initial_state\":\"start\""),
        "workflow snapshot must be parsed canonical JSON, not raw YAML"
    );
    assert!(
        plan.catalog_snapshot_json.contains("\"backend_profiles\""),
        "catalog snapshot must preserve parsed catalog truth"
    );
}

#[test]
fn steward_metadata_contract_tests_snapshot_hashes_are_canonical_over_yaml_ordering() {
    let catalog_a = r#"
backend_profiles:
  steward_profile:
    provider: claude
    model: steward-model
agents:
  - id: steward
    backend_profile: steward_profile
    prompt: "observe"
"#;
    let catalog_b = r#"
agents:
  - prompt: "observe"
    backend_profile: steward_profile
    id: steward
backend_profiles:
  steward_profile:
    model: steward-model
    provider: claude
"#;
    let workflow_a = r#"
workflow:
  id: steward-workflow
  name: Steward Workflow
  family: mvp_live
  risk_class: high
  stack: swiftui
variables:
  max_iterations: 2
initial_state: start
states:
  start:
    label: Start
    type: end
    owner: steward
"#;
    let workflow_b = r#"
states:
  start:
    owner: steward
    type: end
    label: Start
initial_state: start
variables:
  max_iterations: 2
workflow:
  stack: swiftui
  risk_class: high
  family: mvp_live
  name: Steward Workflow
  id: steward-workflow
"#;

    let workflow_a_path = write_temp_fixture("workflow-a.yaml", workflow_a);
    let workflow_b_path = write_temp_fixture("workflow-b.yaml", workflow_b);
    let catalog_a_path = write_temp_fixture("catalog-a.yaml", catalog_a);
    let catalog_b_path = write_temp_fixture("catalog-b.yaml", catalog_b);
    let plan_a = compiler::compile(&workflow_a_path, &catalog_a_path).expect("plan a compiles");
    let plan_b = compiler::compile(&workflow_b_path, &catalog_b_path).expect("plan b compiles");

    assert_eq!(plan_a.workflow_snapshot_json, plan_b.workflow_snapshot_json);
    assert_eq!(plan_a.catalog_snapshot_json, plan_b.catalog_snapshot_json);
    assert_eq!(plan_a.workflow_snapshot_hash, plan_b.workflow_snapshot_hash);
    assert_eq!(plan_a.catalog_snapshot_hash, plan_b.catalog_snapshot_hash);
}

#[test]
fn test_compile_n_phase_ordering() {
    let wf_path = format!("{}/workflows/full-mvp-live.yaml", fixtures_dir());
    let cat_path = format!("{}/agents/agents.yaml", fixtures_dir());

    let plan = compiler::compile(&wf_path, &cat_path).expect("should compile plan");

    // ── state_11_manual_release: post_approval_tasks have sequential phases (0, 1) ──
    let s11 = &plan.states["state_11_manual_release"];
    assert!(s11.is_manual_gate, "state_11 must be a manual_gate");
    assert_eq!(
        s11.post_approval_tasks.len(),
        2,
        "state_11 must have 2 post-approval sequence tasks (commit_and_push, build_and_distribute)"
    );
    assert_eq!(
        s11.post_approval_tasks[0].phase, 0,
        "first post_approval task (commit_and_push) must be phase 0"
    );
    assert_eq!(
        s11.post_approval_tasks[1].phase, 1,
        "second post_approval task (build_and_distribute) must be phase 1"
    );

    // ── state_9_implementation_reviewed: parallel(0) → then(1, 2, 3) ──
    let s9 = &plan.states["state_9_implementation_reviewed"];
    // Parallel tasks: security_checker and docs_guardian at phase 0
    let phase_0: Vec<_> = s9.tasks.iter().filter(|t| t.phase == 0).collect();
    assert_eq!(
        phase_0.len(),
        2,
        "state_9 must have 2 parallel tasks at phase 0 (security_checker, docs_guardian)"
    );
    for t in &phase_0 {
        assert!(
            t.parallel,
            "phase 0 tasks in state_9 must be marked parallel"
        );
    }

    // Then tasks: auditor at phase 1, prepush at phase 2, aggregation at phase 3
    let phase_1: Vec<_> = s9.tasks.iter().filter(|t| t.phase == 1).collect();
    assert_eq!(
        phase_1.len(),
        1,
        "state_9 must have 1 task at phase 1 (auditor)"
    );
    assert_eq!(
        phase_1[0].agent.agent_id, "proposal_implementation_auditor",
        "phase 1 task must be the auditor"
    );

    let phase_2: Vec<_> = s9.tasks.iter().filter(|t| t.phase == 2).collect();
    assert_eq!(
        phase_2.len(),
        1,
        "state_9 must have 1 task at phase 2 (prepush)"
    );
    assert_eq!(
        phase_2[0].agent.agent_id, "prepush_code_reviewer",
        "phase 2 task must be prepush_code_reviewer"
    );

    let phase_3: Vec<_> = s9.tasks.iter().filter(|t| t.phase == 3).collect();
    assert_eq!(
        phase_3.len(),
        1,
        "state_9 must have 1 task at phase 3 (aggregation)"
    );
    assert_eq!(
        phase_3[0].agent.agent_id, "lead_orchestrator",
        "phase 3 task must be lead_orchestrator (aggregate)"
    );

    // ── state_4_proposal_reviewed: parallel(0) → then(1) ──
    let s4 = &plan.states["state_4_proposal_reviewed"];
    let s4_phase_0: Vec<_> = s4.tasks.iter().filter(|t| t.phase == 0).collect();
    assert_eq!(
        s4_phase_0.len(),
        4,
        "state_4 must have 4 parallel tasks at phase 0"
    );
    for t in &s4_phase_0 {
        assert!(
            t.parallel,
            "phase 0 tasks in state_4 must be marked parallel"
        );
    }

    let s4_phase_1: Vec<_> = s4.tasks.iter().filter(|t| t.phase == 1).collect();
    assert_eq!(
        s4_phase_1.len(),
        1,
        "state_4 must have 1 then-task at phase 1 (aggregate_proposal_reviews)"
    );
    assert_eq!(
        s4_phase_1[0].task_name, "aggregate_proposal_reviews",
        "phase 1 task in state_4 must be aggregate_proposal_reviews"
    );
    assert!(
        !s4_phase_1[0].parallel,
        "then-task must not be marked parallel"
    );
}

#[test]
fn test_output_contract_is_authoritative_and_carries_human_format() {
    let plan = compile_from_strings(
        r#"
initial_state: start
states:
  start:
    label: Start
    owner: reviewer
    run:
      sequence:
        - agent: reviewer
          task: write_review
          outputs:
            - proposal_review_po
"#,
        r#"
backend_profiles:
  review_profile:
    provider: codex_acp
contracts:
  proposal_review_v1:
    format: json
    human_format: markdown
    machine_format: json
    validation_mode: strict_structured
    raw_artifact_name: proposal_review_raw
    normalized_artifact_name: proposal_review_normalized
    required_fields:
      - agent_id
      - verdict
  proposal_review_po:
    format: text
    human_format: prose
    machine_format: yaml
    validation_mode: lenient
    raw_artifact_name: proposal_review_po_raw
    normalized_artifact_name: proposal_review_po
    required_fields:
      - wrong_field
agents:
  - id: reviewer
    backend_profile: review_profile
    outputs:
      - proposal_review_po
    output_contract: proposal_review_v1
"#,
    );

    let task = &plan.states["start"].tasks[0];
    let schema = task
        .output_schemas
        .get("proposal_review_po")
        .expect("output schema should be resolved");

    assert_eq!(schema.contract_id, "proposal_review_v1");
    assert_eq!(schema.format, "json");
    assert_eq!(schema.human_format.as_deref(), Some("markdown"));
    assert_eq!(schema.machine_format.as_deref(), Some("json"));
    assert_eq!(schema.validation_mode.as_deref(), Some("strict_structured"));
    assert_eq!(
        schema.normalized_artifact_name.as_deref(),
        Some("proposal_review_normalized")
    );
    assert_eq!(
        schema.raw_artifact_name.as_deref(),
        Some("proposal_review_raw")
    );
    assert_eq!(schema.required_fields, vec!["agent_id", "verdict"]);
}

#[test]
fn proposal_061_catalog_provider_aliases_use_shared_provider_family_resolver() {
    let workflow = r#"
initial_state: start
states:
  start:
    label: Start
    owner: reviewer
    run:
      sequence:
        - agent: reviewer
          task: write_review
"#;
    let catalog = r#"
backend_profiles:
  review_profile:
    provider: openai_codex
agents:
  - id: reviewer
    backend_profile: review_profile
"#;

    let plan = compile_from_strings(workflow, catalog);
    let task = &plan.states["start"].tasks[0];
    assert_eq!(plan.states["start"].owner.provider, "codex");
    assert_eq!(task.agent.provider, "codex");
}

#[test]
fn proposal_061_catalog_unknown_provider_fails_validation() {
    let workflow = r#"
initial_state: start
states:
  start:
    label: Start
    owner: reviewer
    run:
      sequence:
        - agent: reviewer
          task: write_review
"#;
    let catalog = r#"
backend_profiles:
  review_profile:
    provider: mystery_acp
agents:
  - id: reviewer
    backend_profile: review_profile
"#;

    let error = compile_result_from_strings(workflow, catalog)
        .expect_err("unknown provider aliases must fail catalog validation");
    let message = format!("{error:#}");
    assert!(
        message.contains("unknown provider family alias: mystery_acp"),
        "expected typed provider-family error in chain, got: {message}"
    );
    assert!(
        message.contains("Agent 'reviewer' backend_profile 'review_profile'"),
        "expected catalog context, got: {message}"
    );
}

#[test]
fn proposal_053_output_policies_compile_reuse_policy() {
    let plan = compile_from_strings(
        r#"
initial_state: start
states:
  start:
    label: Start
    owner: reviewer
    run:
      sequence:
        - agent: reviewer
          task: write_review
          outputs:
            - proposal_review
          output_policies:
            proposal_review:
              reuse_policy: allow_unchanged_existing
"#,
        r#"
backend_profiles:
  review_profile:
    provider: codex_acp
agents:
  - id: reviewer
    backend_profile: review_profile
    outputs:
      - proposal_review
"#,
    );

    let task = &plan.states["start"].tasks[0];
    assert_eq!(
        task.output_policies["proposal_review"].reuse_policy,
        workflow::plan::OutputReusePolicy::AllowUnchangedExisting
    );
}

#[test]
fn proposal_053_output_policies_reject_unknown_output_keys() {
    let err = compile_result_from_strings(
        r#"
initial_state: start
states:
  start:
    label: Start
    owner: reviewer
    run:
      sequence:
        - agent: reviewer
          task: write_review
          outputs:
            - proposal_review
          output_policies:
            stale_review:
              reuse_policy: allow_unchanged_existing
"#,
        r#"
backend_profiles:
  review_profile:
    provider: codex_acp
agents:
  - id: reviewer
    backend_profile: review_profile
    outputs:
      - proposal_review
"#,
    )
    .unwrap_err();

    assert!(
        err.to_string()
            .contains("output_policies key 'stale_review'"),
        "unexpected error: {err}"
    );
}

#[test]
fn proposal_053_output_policies_reject_unknown_reuse_policy_values() {
    let err = compile_result_from_strings(
        r#"
initial_state: start
states:
  start:
    label: Start
    owner: reviewer
    run:
      sequence:
        - agent: reviewer
          task: write_review
          outputs:
            - proposal_review
          output_policies:
            proposal_review:
              reuse_policy: sometimes_reuse
"#,
        r#"
backend_profiles:
  review_profile:
    provider: codex_acp
agents:
  - id: reviewer
    backend_profile: review_profile
    outputs:
      - proposal_review
"#,
    )
    .unwrap_err();

    assert!(
        err.to_string().contains("loading workflow definition"),
        "unexpected error: {err}"
    );
}

#[test]
fn proposal_053_legacy_broad_discovery_policy_defaults_disabled() {
    let plan = compile_from_strings(
        r#"
initial_state: start
states:
  start:
    label: Start
    owner: reviewer
"#,
        r#"
backend_profiles:
  review_profile:
    provider: codex_acp
agents:
  - id: reviewer
    backend_profile: review_profile
"#,
    );

    assert_eq!(
        plan.legacy_broad_discovery_policy,
        workflow::plan::LegacyBroadDiscoveryPolicy::Disabled
    );
}

#[test]
fn proposal_053_legacy_broad_discovery_policy_compiles_workflow_opt_in() {
    let plan = compile_from_strings(
        r#"
discovery:
  legacy_broad_discovery_policy: workflow_opt_in
initial_state: start
states:
  start:
    label: Start
    owner: reviewer
"#,
        r#"
backend_profiles:
  review_profile:
    provider: codex_acp
agents:
  - id: reviewer
    backend_profile: review_profile
"#,
    );

    assert_eq!(
        plan.legacy_broad_discovery_policy,
        workflow::plan::LegacyBroadDiscoveryPolicy::WorkflowOptIn
    );
}

#[test]
fn proposal_053_legacy_broad_discovery_policy_rejects_unknown_values() {
    let err = compile_result_from_strings(
        r#"
discovery:
  legacy_broad_discovery_policy: always
initial_state: start
states:
  start:
    label: Start
    owner: reviewer
"#,
        r#"
backend_profiles:
  review_profile:
    provider: codex_acp
agents:
  - id: reviewer
    backend_profile: review_profile
"#,
    )
    .unwrap_err();

    assert!(
        err.to_string().contains("loading workflow definition"),
        "unexpected error: {err}"
    );
}

#[test]
fn test_multi_output_agent_output_contract_only_binds_matching_output() {
    let plan = compile_from_strings(
        r#"
initial_state: start
states:
  start:
    label: Start
    owner: writer
    run:
      sequence:
        - agent: writer
          task: write_outputs
          outputs:
            - implementation_progress
            - implementation_self_assessment
            - changed_files_manifest
            - tests_result
"#,
        r#"
backend_profiles:
  writer_profile:
    provider: codex_acp
contracts:
  implementation_self_assessment_v1:
    format: json
    validation_mode: strict_structured
    required_fields:
      - seemingly_complete
  implementation_self_assessment_v2:
    format: json
    validation_mode: strict_structured
    required_fields:
      - implementation_complete
  implementation_progress:
    format: markdown
    validation_mode: lenient
    required_fields: []
  changed_files_manifest:
    format: json
    validation_mode: strict_structured
    required_fields:
      - files
  tests_result:
    format: json
    validation_mode: strict_structured
    required_fields:
      - status
agents:
  - id: writer
    backend_profile: writer_profile
    outputs:
      - implementation_progress
      - implementation_self_assessment
      - changed_files_manifest
      - tests_result
    output_contract: implementation_self_assessment_v2
"#,
    );

    let schemas = &plan.states["start"].tasks[0].output_schemas;
    assert_eq!(
        schemas["implementation_self_assessment"].contract_id,
        "implementation_self_assessment_v2"
    );
    assert_eq!(
        schemas["implementation_progress"].contract_id,
        "implementation_progress"
    );
    assert_eq!(
        schemas["changed_files_manifest"].contract_id,
        "changed_files_manifest"
    );
    assert_eq!(schemas["tests_result"].contract_id, "tests_result");
}

#[test]
fn test_contract_binding_matches_normalized_and_raw_artifacts_exactly() {
    let plan = compile_from_strings(
        r#"
initial_state: start
states:
  start:
    label: Start
    owner: reviewer
    run:
      sequence:
        - agent: reviewer
          task: write_review
          outputs:
            - proposal_review_normalized
            - proposal_review_raw
"#,
        r#"
backend_profiles:
  review_profile:
    provider: codex_acp
contracts:
  proposal_review_v1:
    format: json
    human_format: markdown
    machine_format: json
    validation_mode: strict_structured
    raw_artifact_name: proposal_review_raw
    normalized_artifact_name: proposal_review_normalized
    required_fields:
      - agent_id
      - verdict
agents:
  - id: reviewer
    backend_profile: review_profile
    outputs:
      - proposal_review_normalized
      - proposal_review_raw
"#,
    );

    let task = &plan.states["start"].tasks[0];
    let normalized = task
        .output_schemas
        .get("proposal_review_normalized")
        .expect("normalized schema should be resolved");
    let raw = task
        .output_schemas
        .get("proposal_review_raw")
        .expect("raw schema should be resolved");

    assert_eq!(normalized.contract_id, "proposal_review_v1");
    assert_eq!(raw.contract_id, "proposal_review_v1");
    assert_eq!(
        normalized.normalized_artifact_name.as_deref(),
        Some("proposal_review_normalized")
    );
    assert_eq!(
        raw.raw_artifact_name.as_deref(),
        Some("proposal_review_raw")
    );
}

#[test]
fn test_contract_binding_uses_versioned_and_stem_fallbacks() {
    let plan = compile_from_strings(
        r#"
initial_state: start
states:
  start:
    label: Start
    owner: reviewer
    run:
        sequence:
        - agent: reviewer
          task: write_review
          outputs:
            - proposal_review
            - proposal_review_v2
            - proposal_review-v3
"#,
        r#"
backend_profiles:
  review_profile:
    provider: codex_acp
contracts:
  proposal_review_v1:
    format: json
    human_format: markdown
    machine_format: json
    validation_mode: strict_structured
    required_fields:
      - agent_id
      - verdict
agents:
  - id: reviewer
    backend_profile: review_profile
    outputs:
      - proposal_review
      - proposal_review_v2
"#,
    );

    let task = &plan.states["start"].tasks[0];
    let stem = task
        .output_schemas
        .get("proposal_review")
        .expect("stem fallback should resolve");
    let versioned = task
        .output_schemas
        .get("proposal_review_v2")
        .expect("versioned fallback should resolve");
    let hyphen_versioned = task
        .output_schemas
        .get("proposal_review-v3")
        .expect("hyphenated versioned fallback should resolve");

    assert_eq!(stem.contract_id, "proposal_review_v1");
    assert_eq!(versioned.contract_id, "proposal_review_v1");
    assert_eq!(hyphen_versioned.contract_id, "proposal_review_v1");
    assert_eq!(stem.human_format.as_deref(), Some("markdown"));
    assert_eq!(versioned.machine_format.as_deref(), Some("json"));
    assert_eq!(hyphen_versioned.machine_format.as_deref(), Some("json"));
}
