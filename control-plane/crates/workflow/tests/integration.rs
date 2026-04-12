use workflow::compiler;

fn fixtures_dir() -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    format!("{manifest}/../../../examples")
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
    assert_eq!(s4.owner.provider, "claude", "state_4 owner=lead_orchestrator → claude");
    // Parallel tasks should have mixed providers
    let ux_task = s4.tasks.iter().find(|t| t.agent.agent_id == "proposal_reviewer_ux");
    assert!(ux_task.is_some(), "should have UX reviewer task");
    assert_eq!(ux_task.unwrap().agent.provider, "gemini", "UX reviewer uses gemini");

    let arch_task = s4.tasks.iter().find(|t| t.agent.agent_id == "proposal_reviewer_architect");
    assert!(arch_task.is_some(), "should have architect reviewer task");
    assert_eq!(arch_task.unwrap().agent.provider, "codex", "architect uses codex");

    // Verify code_writer → codex
    let s7 = &plan.states["state_7_implementation_started"];
    let cw_task = s7.tasks.iter().find(|t| t.agent.agent_id == "code_writer");
    assert!(cw_task.is_some(), "state_7 should have code_writer task");
    assert_eq!(cw_task.unwrap().agent.provider, "codex");

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
