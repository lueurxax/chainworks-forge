use super::*;
use crate::agent_mission_context::{
    finalize_owner_prompt_v1, finalize_task_prompt_v1,
    validate_persisted_v1_payload_prompt_with_truth,
};

struct Fixture {
    _root: tempfile::TempDir,
    pool: sqlx::SqlitePool,
    orchestrator: Orchestrator,
    plan: RunPlan,
    run: Run,
    idea: domain::idea::Idea,
    stage: StageExecution,
}

impl Fixture {
    async fn new() -> Self {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let workflow_path = root.join("examples/workflows/full-mvp-live.yaml");
        let catalog_path = root.join("examples/agents/agents.yaml");
        let plan = workflow::compiler::compile(
            workflow_path.to_str().unwrap(),
            catalog_path.to_str().unwrap(),
        )
        .unwrap();
        let temp = tempfile::tempdir().unwrap();
        let pool = test_pool().await;
        let mut run = test_run(RunId::new());
        run.current_state = Some("state_9_implementation_reviewed".into());
        run.workflow_yaml_path = Some(workflow_path.to_string_lossy().into_owned());
        run.agent_catalog_yaml_path = Some(catalog_path.to_string_lossy().into_owned());
        run.workspace_root = temp.path().to_string_lossy().into_owned();
        run.artifact_root = run.workspace_root.clone();
        run.chainworks_meta_root = Some(run.workspace_root.clone());
        run.worktree_root = Some(
            temp.path()
                .join("implementation")
                .to_string_lossy()
                .into_owned(),
        );
        set_snapshot_quartet(
            &mut run,
            &plan.workflow_snapshot_json,
            &plan.catalog_snapshot_json,
        );
        let idea = test_idea(run.idea_id);
        ideas::insert(&pool, &idea).await.unwrap();
        runs::insert(&pool, &run).await.unwrap();
        let stage = StageExecution {
            id: StageExecutionId::new(),
            run_id: run.id,
            stage_id: run.current_state.clone().unwrap(),
            label: "Implementation reviewed".into(),
            status: StageStatus::Running,
            iteration: 1,
            attempt_number: 2,
            settlement_kind: None,
            started_at: Utc::now(),
            completed_at: None,
            owner_agent: None,
            provider: None,
            model: None,
            stage_type: None,
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        };
        stages::insert(&pool, &stage).await.unwrap();
        let orchestrator = Orchestrator::new(
            pool.clone(),
            crate::event_bus::new_bus(64),
            WorkQueue::new(pool.clone()),
        );
        Self {
            _root: temp,
            pool,
            orchestrator,
            plan,
            run,
            idea,
            stage,
        }
    }

    async fn enqueue(&self, agent_id: &str) -> WorkItem {
        let state = &self.plan.states[&self.stage.stage_id];
        let (index, task) = state
            .tasks
            .iter()
            .chain(&state.post_approval_tasks)
            .enumerate()
            .find(|(_, task)| task.agent.agent_id == agent_id)
            .unwrap();
        let prompt = finalize_task_prompt_v1(
            &self.plan,
            &self.run,
            state,
            task,
            &self.idea,
            "Inspect the frozen implementation task.",
        )
        .unwrap();
        self.orchestrator
            .enqueue_invoke_agent(
                self.run.id,
                &self.stage,
                task,
                &prompt,
                index,
                state.tasks.len(),
                &self.plan,
                &self.run,
            )
            .await
            .unwrap();
        work_items::list_by_run(&self.pool, self.run.id)
            .await
            .unwrap()
            .into_iter()
            .find(|item| item.id == format!("p058-invoke:{}:{index}", self.stage.id))
            .unwrap()
    }
}

#[tokio::test]
async fn worktree_strategy_v1_roundtrips_enqueued_static_authority() {
    for (agent_id, explicit, post_approval, expected) in [
        (
            "security_checker",
            None,
            false,
            Some("shared_implementation_worktree"),
        ),
        (
            "proposal_implementation_auditor",
            None,
            false,
            Some("shared_implementation_worktree"),
        ),
        (
            "prepush_code_reviewer",
            None,
            false,
            Some("shared_implementation_worktree"),
        ),
        (
            "docs_guardian",
            None,
            false,
            Some("shared_implementation_worktree"),
        ),
        (
            "security_checker",
            Some("meta_only"),
            false,
            Some("meta_only"),
        ),
        (
            "security_checker",
            Some("dedicated"),
            false,
            Some("dedicated"),
        ),
        (
            "security_checker",
            None,
            true,
            Some("shared_implementation_worktree"),
        ),
        ("lead_orchestrator", None, false, None),
    ] {
        let mut f = Fixture::new().await;
        if let Some(strategy) = explicit {
            for state in f.plan.states.values_mut() {
                for agent in std::iter::once(&mut state.owner)
                    .chain(state.tasks.iter_mut().map(|task| &mut task.agent))
                    .chain(
                        state
                            .post_approval_tasks
                            .iter_mut()
                            .map(|task| &mut task.agent),
                    )
                {
                    if agent.agent_id == agent_id {
                        agent.worktree_strategy = Some(strategy.into());
                    }
                }
            }
        }
        if post_approval {
            let state = f.plan.states.get_mut(&f.stage.stage_id).unwrap();
            let index = state
                .tasks
                .iter()
                .position(|task| task.agent.agent_id == agent_id)
                .unwrap();
            let task = state.tasks.remove(index);
            state.post_approval_tasks.push(task);
        }
        let item = f.enqueue(agent_id).await;
        let payload: serde_json::Value = serde_json::from_str(&item.payload_json).unwrap();
        assert_eq!(
            payload["worktree_strategy"],
            serde_json::json!(expected),
            "{agent_id}"
        );
        validate_persisted_v1_payload_prompt_with_truth(&f.plan, &f.run, &f.idea, &payload)
            .unwrap_or_else(|error| {
                panic!("enqueued {agent_id} must remain valid on copy: {error}")
            });
        for invalid in [
            None,
            Some("shared_implementation_worktree"),
            Some("meta_only"),
            Some("dedicated"),
            Some("arbitrary"),
        ] {
            if invalid == expected {
                continue;
            }
            let mut mutated = payload.clone();
            mutated["worktree_strategy"] = serde_json::json!(invalid);
            let error =
                validate_persisted_v1_payload_prompt_with_truth(&f.plan, &f.run, &f.idea, &mutated)
                    .expect_err("a copied payload cannot select its own worktree strategy")
                    .to_string();
            assert!(
                error.contains("field 'worktree_strategy' differs from frozen authority"),
                "{error}"
            );
        }
    }
}

#[tokio::test]
async fn worktree_strategy_v1_write_enabled_reviewer_cannot_derive_readonly_strategy() {
    let mut f = Fixture::new().await;
    for state in f.plan.states.values_mut() {
        for agent in std::iter::once(&mut state.owner)
            .chain(state.tasks.iter_mut().map(|task| &mut task.agent))
            .chain(
                state
                    .post_approval_tasks
                    .iter_mut()
                    .map(|task| &mut task.agent),
            )
        {
            if agent.agent_id == "security_checker" {
                agent.worktree_write_enabled = true;
                assert_eq!(agent.worktree_strategy, None);
            }
        }
    }
    let item = f.enqueue("security_checker").await;
    let mut payload: serde_json::Value = serde_json::from_str(&item.payload_json).unwrap();
    assert_eq!(payload["worktree_strategy"], serde_json::Value::Null);
    validate_persisted_v1_payload_prompt_with_truth(&f.plan, &f.run, &f.idea, &payload).unwrap();
    payload["worktree_strategy"] = serde_json::json!("shared_implementation_worktree");
    let error = validate_persisted_v1_payload_prompt_with_truth(&f.plan, &f.run, &f.idea, &payload)
        .expect_err("write-enabled reviewer must not gain read-only fallback authority")
        .to_string();
    assert!(
        error.contains("field 'worktree_strategy' differs from frozen authority"),
        "{error}"
    );
}

#[tokio::test]
async fn worktree_strategy_v1_owner_cannot_borrow_static_task_derivation() {
    let mut f = Fixture::new().await;
    let item = f.enqueue("security_checker").await;
    let mut payload: serde_json::Value = serde_json::from_str(&item.payload_json).unwrap();
    let state = f.plan.states.get_mut(&f.stage.stage_id).unwrap();
    state.owner = state
        .tasks
        .iter()
        .find(|task| task.agent.agent_id == "security_checker")
        .unwrap()
        .agent
        .clone();
    let prompt = finalize_owner_prompt_v1(
        &f.plan,
        &f.run,
        &f.plan.states[&f.stage.stage_id],
        &f.idea,
        "Owner only",
    )
    .unwrap();
    payload["prompt"] = serde_json::json!(prompt);
    let error = validate_persisted_v1_payload_prompt_with_truth(&f.plan, &f.run, &f.idea, &payload)
        .expect_err("state owner must not inherit the strategy of a static reviewer task")
        .to_string();
    assert!(
        error.contains("field 'worktree_strategy' differs from frozen authority"),
        "{error}"
    );
    payload["worktree_strategy"] = serde_json::Value::Null;
    validate_persisted_v1_payload_prompt_with_truth(&f.plan, &f.run, &f.idea, &payload).unwrap();
}

#[tokio::test]
async fn worktree_strategy_v1_advance_after_preflight_failure_schedules_p058_retry() {
    let f = Fixture::new().await;
    let security = f.enqueue("security_checker").await;
    let docs = f.enqueue("docs_guardian").await;
    let failed_id = AgentExecutionId::new();
    let now = Utc::now();
    let policy = f
        .plan
        .escalation_policies
        .iter()
        .find(|p| p.policy_id == "security_checker_quota_escalation")
        .unwrap();
    let primary = &policy.tiers[0];
    let next = &policy.tiers[1];
    let mut payload: serde_json::Value = serde_json::from_str(&security.payload_json).unwrap();
    payload["p058_claimed"] = serde_json::json!({"agent_execution_id": failed_id.to_string()});
    sqlx::query("UPDATE work_items SET payload_json = ?1, status = 'failed' WHERE id = ?2")
        .bind(payload.to_string())
        .bind(&security.id)
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE work_items SET status = 'completed' WHERE id = ?1")
        .bind(&docs.id)
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO agent_executions (id, stage_execution_id, agent_id, provider, provider_family, status, started_at, completed_at, owner_kind, owner_id) VALUES (?1, ?2, 'security_checker', 'claude_acp', 'claude', 'failed', ?3, ?3, 'stage_execution', ?2)")
        .bind(failed_id.to_string()).bind(f.stage.id.to_string()).bind(now.to_rfc3339()).execute(&f.pool).await.unwrap();
    let mut facts = AgentExecutionRuntimeFacts::defaults_for(failed_id, now);
    facts.failure_kind = Some(AgentFailureKind::Unknown);
    facts.supervision_classification = Some("xcode_target_not_found".into());
    facts.runtime_preflight_provider_launched = Some(false);
    agent_execution_runtime_facts::upsert(&f.pool, &facts)
        .await
        .unwrap();
    escalation::insert_ledger(
        &f.pool,
        &domain::escalation::EscalationLedger {
            id: "worktree-strategy-ledger".into(),
            run_id: f.run.id,
            stage_id: f.stage.stage_id.clone(),
            stage_execution_id: None,
            agent_id: "security_checker".into(),
            policy_id: policy.policy_id.clone(),
            policy_hash: policy.policy_hash.clone(),
            status_raw: "active".into(),
            current_tier_id: Some(next.tier_id.clone()),
            current_tier_kind_raw: Some(next.kind.clone()),
            chain_attempt_index: 1,
            trigger_raw: Some("contract_output_failure".into()),
            pause_reason_raw: None,
            operator_action_hint: None,
            runbook_anchor: None,
            created_at: now,
            updated_at: now,
        },
    )
    .await
    .unwrap();
    escalation::insert_execution_metadata(
        &f.pool,
        &domain::escalation::EscalationExecutionMetadata {
            agent_execution_id: failed_id,
            escalation_ledger_id: "worktree-strategy-ledger".into(),
            tier_id: primary.tier_id.clone(),
            tier_kind_raw: primary.kind.clone(),
            tier_attempt_index: 0,
            trigger_raw: Some("contract_output_failure".into()),
            digest_version: None,
            capacity_probe_counter: 0,
            created_at: now,
            updated_at: now,
            would_select_tier_id: None,
            would_select_trigger_raw: None,
            would_select_decision_json: None,
        },
    )
    .await
    .unwrap();
    let advance_id = format!("advance-after-invoke:{}", security.id);
    work_items::enqueue(
        &f.pool,
        &WorkItem {
            id: advance_id.clone(),
            kind: WorkItemKind::AdvanceRun,
            payload_json: serde_json::json!({"run_id": f.run.id.to_string()}).to_string(),
            status: WorkItemStatus::Pending,
            run_id: Some(f.run.id),
            stage_id: None,
            created_at: now,
            scheduled_at: now - chrono::Duration::seconds(5),
            attempt_count: 0,
            last_error: None,
        },
    )
    .await
    .unwrap();
    let events = crate::event_bus::new_bus(64);
    let executor = crate::executor::BackgroundExecutor::new(
        f.pool.clone(),
        WorkQueue::new(f.pool.clone()),
        Arc::new(f.orchestrator),
        Arc::new(acp::AcpRuntimeManager::new_with_adapters(vec![])),
        events,
    );
    let result = executor.process_next_item().await;
    assert!(
        matches!(result, Ok(true)),
        "advance must create the retry, not fail frozen authority: {result:?}"
    );
    let items = work_items::list_by_run(&f.pool, f.run.id).await.unwrap();
    assert_eq!(
        items.iter().find(|i| i.id == advance_id).unwrap().status,
        WorkItemStatus::Completed
    );
    let retry = items
        .iter()
        .filter(|i| i.kind == WorkItemKind::InvokeAgent && i.status == WorkItemStatus::Pending)
        .collect::<Vec<_>>();
    assert_eq!(retry.len(), 1);
    let retry_payload: serde_json::Value = serde_json::from_str(&retry[0].payload_json).unwrap();
    assert_eq!(
        retry_payload["worktree_strategy"],
        "shared_implementation_worktree"
    );
    assert_eq!(retry_payload["backend_profile_id"], "codex_architect_high");
    assert_eq!(
        retry_payload.pointer("/targeted_retry/reason").unwrap(),
        "p058_escalation_retry"
    );
    assert_eq!(
        stages::find_by_id(&f.pool, f.stage.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        StageStatus::Skipped
    );
    assert_eq!(
        runs::find_by_id(&f.pool, f.run.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        RunStatus::Running
    );
    let target = retry_payload["stage_execution_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(
        stages::find_by_id(&f.pool, target)
            .await
            .unwrap()
            .unwrap()
            .status,
        StageStatus::Running
    );
}
