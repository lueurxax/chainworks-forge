use chrono::{Duration, Utc};
use db::pool::create_pool;
use db::repos::{escalation as escalation_repo, ideas, runs, stages, work_items};
use db::work_item::{WorkItem, WorkItemKind, WorkItemStatus};
use domain::commands::{
    CallerContext, Command, PrincipalClass, ResumeEscalationChainCmd, ResumeEscalationDeadlineCmd,
};
use domain::escalation::{EscalationExecutionMetadata, EscalationLedger};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{AgentExecutionId, IdeaId, RunId, StageExecutionId};
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageSettlementKind, StageStatus};
use engine::command_handler::{CommandHandler, CommandResult};
use engine::event_bus;
use engine::work_queue::WorkQueue;
use std::sync::Arc;

const LEDGER_ID: &str = "ledger-p058-deadline-resume";
const POLICY_ID: &str = "proposal-writer-escalation";
const CURRENT_TIER_ID: &str = "gemini_reasoning_fallback";

struct Fixture {
    pool: sqlx::SqlitePool,
    handler: CommandHandler,
    run_id: RunId,
    source_stage_execution_id: StageExecutionId,
    source_agent_execution_id: AgentExecutionId,
    ledger_created_at: chrono::DateTime<Utc>,
}

async fn setup_fixture(pause_reason: &str) -> Fixture {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let writer = Arc::new(db::writer::DbWriter::new(pool.clone()));
    db::writer::register_shared_writer(&pool, writer)
        .await
        .unwrap();

    let events = event_bus::new_bus(64);
    let handler = CommandHandler::new(pool.clone(), events, WorkQueue::new(pool.clone()));
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let workflow_snapshot = serde_json::json!({
        "workflow": {"id": "p058_deadline_resume"},
        "initial_state": "state_5_proposal_refined",
        "states": {
            "state_5_proposal_refined": {
                "label": "Proposal refinement",
                "owner": "proposal_writer",
                "type": "end"
            }
        }
    })
    .to_string();
    let catalog_snapshot = serde_json::json!({
        "backend_profiles": {
            "claude_writer_primary": {
                "provider": "claude_acp",
                "model": "opus",
                "effort": "high",
                "max_turns": 12
            },
            "gemini_reasoning": {
                "provider": "gemini_acp",
                "model": "gemini-3.1-pro-preview",
                "effort": "high",
                "max_turns": 16
            },
            "lead_profile": {
                "provider": "claude_acp",
                "model": "sonnet"
            }
        },
        "permission_profiles": {"lead_perm": {}},
        "contracts": {"lead_contract": {"format": "json"}},
        "agents": [
            {
                "id": "proposal_writer",
                "backend_profile": "claude_writer_primary",
                "output_contract": "proposal_v1"
            },
            {
                "id": "lead_orchestrator",
                "system_role": "lead",
                "backend_profile": "lead_profile",
                "permission_profile": "lead_perm",
                "lead_resolution_contract": "lead_contract"
            }
        ],
        "escalation_policies": [{
            "policy_id": POLICY_ID,
            "schema_version": "escalation_policy_v1",
            "enabled_default": true,
            "applies_to": {"agent_id": "proposal_writer"},
            "max_chain_attempts": 4,
            "max_chain_wall_clock_seconds": 120,
            "triggers": ["transport_failure"],
            "tiers": [
                {
                    "tier_id": "primary_retry",
                    "kind": "same_backend_retry",
                    "max_attempts": 1
                },
                {
                    "tier_id": CURRENT_TIER_ID,
                    "kind": "backend_profile",
                    "backend_profile_id": "gemini_reasoning",
                    "max_attempts": 2
                },
                {
                    "tier_id": "human_pause",
                    "kind": "pause"
                }
            ]
        }]
    })
    .to_string();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P058 recovery".into(),
            body: "Resume an elapsed escalation deadline".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        },
    )
    .await
    .unwrap();

    let run = Run {
        id: run_id,
        idea_id,
        status: RunStatus::Blocked,
        workflow_id: "p058_deadline_resume".into(),
        workflow_title: "P058 deadline resume".into(),
        workspace_root: "/tmp/p058-deadline-resume".into(),
        artifact_root: "/tmp/p058-deadline-resume/.chainworks".into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        cancellation_settlement_log: None,
        current_state: Some("state_5_proposal_refined".into()),
        workflow_yaml_path: None,
        agent_catalog_yaml_path: None,
        worktree_root: None,
        base_branch: None,
        base_revision: None,
        target_branch: None,
        delivery_configuration_json: None,
        delivery_preflight_json: None,
        workflow_family: None,
        project_key: None,
        risk_class: None,
        stack: None,
        workflow_snapshot_hash: None,
        catalog_snapshot_hash: None,
        workflow_snapshot_json: Some(workflow_snapshot),
        catalog_snapshot_json: Some(catalog_snapshot),
        drift_detected_at: None,
        drift_details_json: None,
        chainworks_meta_root: None,
        review_routing_json: None,
        closeout_readiness_mode: None,
    };
    runs::insert(&pool, &run).await.unwrap();

    let source_stage_execution_id = StageExecutionId::new();
    let now = Utc::now();
    stages::insert(
        &pool,
        &StageExecution {
            id: source_stage_execution_id,
            run_id,
            stage_id: "state_5_proposal_refined".into(),
            label: "Proposal refinement".into(),
            status: StageStatus::Failed,
            iteration: 3,
            attempt_number: 2,
            settlement_kind: Some(StageSettlementKind::Failed),
            started_at: now - Duration::minutes(5),
            completed_at: Some(now - Duration::minutes(1)),
            owner_agent: Some("proposal_writer".into()),
            provider: Some("claude_acp".into()),
            model: Some("opus".into()),
            stage_type: None,
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        },
    )
    .await
    .unwrap();

    let source_agent_execution_id = AgentExecutionId::new();
    sqlx::query(
        r#"INSERT INTO agent_executions
           (id, stage_execution_id, agent_id, provider, provider_family, model, status,
            started_at, completed_at, owner_kind, owner_id)
           VALUES (?1, ?2, 'proposal_writer', 'claude_acp', 'claude', 'opus', 'failed',
                   ?3, ?4, 'stage_execution', ?2)"#,
    )
    .bind(source_agent_execution_id.to_string())
    .bind(source_stage_execution_id.to_string())
    .bind((now - Duration::minutes(5)).to_rfc3339())
    .bind((now - Duration::minutes(1)).to_rfc3339())
    .execute(&pool)
    .await
    .unwrap();

    let plan = engine::command_handler::compile_run_plan_from_snapshot(&run)
        .unwrap()
        .unwrap();
    let policy_hash = plan
        .escalation_policies
        .iter()
        .find(|policy| policy.policy_id == POLICY_ID)
        .unwrap()
        .policy_hash
        .clone();
    let ledger_created_at = now - Duration::hours(2);
    escalation_repo::insert_ledger(
        &pool,
        &EscalationLedger {
            id: LEDGER_ID.into(),
            run_id,
            stage_id: "state_5_proposal_refined".into(),
            stage_execution_id: Some(source_stage_execution_id.to_string()),
            agent_id: "proposal_writer".into(),
            policy_id: POLICY_ID.into(),
            policy_hash,
            status_raw: "paused".into(),
            current_tier_id: Some(CURRENT_TIER_ID.into()),
            current_tier_kind_raw: Some("backend_profile".into()),
            chain_attempt_index: 2,
            trigger_raw: Some("transport_failure".into()),
            pause_reason_raw: Some(pause_reason.into()),
            operator_action_hint: Some("Operator action required".into()),
            runbook_anchor: Some("escalation/deadline-elapsed".into()),
            created_at: ledger_created_at,
            updated_at: now - Duration::minutes(1),
        },
    )
    .await
    .unwrap();
    escalation_repo::insert_execution_metadata(
        &pool,
        &EscalationExecutionMetadata {
            agent_execution_id: source_agent_execution_id,
            escalation_ledger_id: LEDGER_ID.into(),
            tier_id: "primary_retry".into(),
            tier_kind_raw: "same_backend_retry".into(),
            tier_attempt_index: 0,
            trigger_raw: Some("transport_failure".into()),
            digest_version: Some("escalation_blocker_digest_v1".into()),
            capacity_probe_counter: 0,
            created_at: now - Duration::minutes(5),
            updated_at: now - Duration::minutes(1),
            would_select_tier_id: None,
            would_select_trigger_raw: None,
            would_select_decision_json: None,
        },
    )
    .await
    .unwrap();
    work_items::enqueue(
        &pool,
        &WorkItem {
            id: "p058-deadline-source-invoke".into(),
            kind: WorkItemKind::InvokeAgent,
            payload_json: serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "state_5_proposal_refined",
                "stage_execution_id": source_stage_execution_id.to_string(),
                "agent_id": "proposal_writer",
                "provider": "claude_acp",
                "backend_profile_id": "claude_writer_primary",
                "model": "opus",
                "effort": "high",
                "max_turns": 12,
                "prompt": "Refine the proposal",
                "output_contract": "proposal_v1",
                "task_outputs": ["proposal_current"],
                "declared_outputs": [],
                "p058_claimed": {
                    "agent_execution_id": source_agent_execution_id.to_string()
                }
            })
            .to_string(),
            status: WorkItemStatus::Completed,
            run_id: Some(run_id),
            stage_id: Some("state_5_proposal_refined".into()),
            created_at: now - Duration::minutes(5),
            scheduled_at: now - Duration::minutes(5),
            attempt_count: 1,
            last_error: None,
        },
    )
    .await
    .unwrap();

    Fixture {
        pool,
        handler,
        run_id,
        source_stage_execution_id,
        source_agent_execution_id,
        ledger_created_at,
    }
}

fn operator_caller() -> CallerContext {
    CallerContext::mcp(
        "operator-p058",
        &PrincipalClass::Operator,
        "runs.resume_escalation_deadline",
    )
}

fn chain_operator_caller() -> CallerContext {
    CallerContext::mcp(
        "operator-p058",
        &PrincipalClass::Operator,
        "runs.resume_escalation_chain",
    )
}

async fn setup_exhausted_chain_fixture() -> Fixture {
    let fixture = setup_fixture("escalation_chain_exhausted").await;
    sqlx::query(
        r#"UPDATE escalation_ledger
           SET current_tier_id = 'human_pause',
               current_tier_kind_raw = 'pause',
               chain_attempt_index = 4,
               operator_action_hint = 'Operator recovery required',
               runbook_anchor = 'escalation/chain-exhausted'
           WHERE id = ?1"#,
    )
    .bind(LEDGER_ID)
    .execute(&fixture.pool)
    .await
    .unwrap();
    fixture
}

#[tokio::test]
async fn p058_operator_resume_creates_linked_window_and_queues_gemini() {
    let fixture = setup_fixture("escalation_deadline_elapsed").await;
    let idempotency_key = uuid::Uuid::now_v7().to_string();

    let commanded = fixture
        .handler
        .handle(
            Command::ResumeEscalationDeadline(ResumeEscalationDeadlineCmd {
                run_id: fixture.run_id,
                escalation_ledger_id: LEDGER_ID.into(),
                reason: "Resume the frozen Gemini fallback after operator validation".into(),
                idempotency_key: idempotency_key.clone(),
            }),
            operator_caller(),
        )
        .await
        .unwrap();

    let (deadline_window_id, retry_stage_execution_id, work_item_id) = match commanded.result {
        CommandResult::EscalationDeadlineResumed {
            deadline_window_id,
            retry_stage_execution_id,
            work_item_id,
            provider,
            tier_id,
            replayed,
            ..
        } => {
            assert_eq!(provider, "gemini_acp");
            assert_eq!(tier_id, CURRENT_TIER_ID);
            assert!(!replayed);
            (deadline_window_id, retry_stage_execution_id, work_item_id)
        }
        _ => panic!("unexpected command result"),
    };

    let ledger = escalation_repo::find_ledger_by_id(&fixture.pool, LEDGER_ID)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ledger.created_at, fixture.ledger_created_at);
    assert_eq!(ledger.status_raw, "active");
    assert_eq!(ledger.pause_reason_raw, None);
    assert_eq!(ledger.chain_attempt_index, 2);

    let windows = escalation_repo::find_deadline_windows_by_ledger(&fixture.pool, LEDGER_ID)
        .await
        .unwrap();
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].id, deadline_window_id);
    assert_eq!(windows[0].tier_id, CURRENT_TIER_ID);
    assert_eq!(
        windows[0].source_pause_reason_raw,
        "escalation_deadline_elapsed"
    );
    assert_eq!(windows[0].previous_window_id, None);
    assert_eq!(
        windows[0].source_stage_execution_id,
        fixture.source_stage_execution_id.to_string()
    );
    assert_eq!(
        windows[0].source_agent_execution_id,
        fixture.source_agent_execution_id.to_string()
    );
    assert!(windows[0].expires_at > windows[0].starts_at);

    let source_stage = stages::find_by_id(&fixture.pool, fixture.source_stage_execution_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(source_stage.status, StageStatus::Failed);
    assert_eq!(
        source_stage.settlement_kind,
        Some(StageSettlementKind::Failed)
    );

    let pending = work_items::list_by_run(&fixture.pool, fixture.run_id)
        .await
        .unwrap()
        .into_iter()
        .filter(|item| {
            item.kind == WorkItemKind::InvokeAgent && item.status == WorkItemStatus::Pending
        })
        .collect::<Vec<_>>();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, work_item_id);
    let payload: serde_json::Value = serde_json::from_str(&pending[0].payload_json).unwrap();
    assert_eq!(payload["provider"], "gemini_acp");
    assert_eq!(payload["backend_profile_id"], "gemini_reasoning");
    assert_eq!(
        payload.pointer("/targeted_retry/escalation/deadline_window_id"),
        Some(&serde_json::json!(deadline_window_id))
    );
    assert_eq!(
        payload["stage_execution_id"],
        retry_stage_execution_id.to_string()
    );

    let events = escalation_repo::find_events_by_ledger(&fixture.pool, LEDGER_ID)
        .await
        .unwrap();
    assert!(events.iter().any(|event| {
        event.event_kind_raw == "escalation.deadline_window_resumed"
            && event
                .payload_json
                .as_deref()
                .and_then(|payload| serde_json::from_str::<serde_json::Value>(payload).ok())
                .and_then(|payload| payload["deadline_window_id"].as_str().map(str::to_owned))
                .as_deref()
                == Some(deadline_window_id.as_str())
    }));

    let replay = fixture
        .handler
        .handle(
            Command::ResumeEscalationDeadline(ResumeEscalationDeadlineCmd {
                run_id: fixture.run_id,
                escalation_ledger_id: LEDGER_ID.into(),
                reason: "Resume the frozen Gemini fallback after operator validation".into(),
                idempotency_key,
            }),
            operator_caller(),
        )
        .await
        .unwrap();
    match replay.result {
        CommandResult::EscalationDeadlineResumed {
            deadline_window_id: replay_window_id,
            work_item_id: replay_work_item_id,
            replayed,
            ..
        } => {
            assert_eq!(replay_window_id, deadline_window_id);
            assert_eq!(replay_work_item_id, work_item_id);
            assert!(replayed);
        }
        _ => panic!("unexpected replay result"),
    }
    assert_eq!(
        escalation_repo::find_deadline_windows_by_ledger(&fixture.pool, LEDGER_ID)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        work_items::list_by_run(&fixture.pool, fixture.run_id)
            .await
            .unwrap()
            .into_iter()
            .filter(|item| item.kind == WorkItemKind::InvokeAgent
                && item.status == WorkItemStatus::Pending)
            .count(),
        1
    );
}

#[tokio::test]
async fn p058_resume_rejects_non_deadline_pause_without_mutation() {
    let fixture = setup_fixture("provider_session_force_detached").await;
    let result = fixture
        .handler
        .handle(
            Command::ResumeEscalationDeadline(ResumeEscalationDeadlineCmd {
                run_id: fixture.run_id,
                escalation_ledger_id: LEDGER_ID.into(),
                reason: "Must remain denied".into(),
                idempotency_key: uuid::Uuid::now_v7().to_string(),
            }),
            operator_caller(),
        )
        .await;
    let error = match result {
        Ok(_) => panic!("only escalation_deadline_elapsed may be resumed"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("P058_RESUME_REASON_NOT_ALLOWED"));
    assert!(
        escalation_repo::find_deadline_windows_by_ledger(&fixture.pool, LEDGER_ID)
            .await
            .unwrap()
            .is_empty()
    );
    let ledger = escalation_repo::find_ledger_by_id(&fixture.pool, LEDGER_ID)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ledger.status_raw, "paused");
    assert_eq!(
        ledger.pause_reason_raw.as_deref(),
        Some("provider_session_force_detached")
    );
    assert_eq!(
        work_items::list_by_run(&fixture.pool, fixture.run_id)
            .await
            .unwrap()
            .into_iter()
            .filter(|item| item.kind == WorkItemKind::InvokeAgent
                && item.status == WorkItemStatus::Pending)
            .count(),
        0
    );
}

#[tokio::test]
async fn p058_resume_requires_operator_at_engine_boundary() {
    let fixture = setup_fixture("escalation_deadline_elapsed").await;
    let caller = CallerContext::mcp(
        "agent-p058",
        &PrincipalClass::Agent,
        "runs.resume_escalation_deadline",
    );
    let result = fixture
        .handler
        .handle(
            Command::ResumeEscalationDeadline(ResumeEscalationDeadlineCmd {
                run_id: fixture.run_id,
                escalation_ledger_id: LEDGER_ID.into(),
                reason: "Agent must not resume".into(),
                idempotency_key: uuid::Uuid::now_v7().to_string(),
            }),
            caller,
        )
        .await;
    let error = match result {
        Ok(_) => panic!("non-operator caller must be rejected"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("requires operator principal"));
    assert!(
        escalation_repo::find_deadline_windows_by_ledger(&fixture.pool, LEDGER_ID)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn p058_operator_resume_exhausted_chain_atomically_queues_explicit_tier() {
    let fixture = setup_exhausted_chain_fixture().await;
    let idempotency_key = uuid::Uuid::now_v7().to_string();

    let commanded = fixture
        .handler
        .handle(
            Command::ResumeEscalationChain(ResumeEscalationChainCmd {
                run_id: fixture.run_id,
                escalation_ledger_id: LEDGER_ID.into(),
                target_tier_id: CURRENT_TIER_ID.into(),
                reason: "Open one audited recovery attempt after provider validation".into(),
                operator_instruction: Some(
                    "Remove only fields rejected by the rollout contract validator.".into(),
                ),
                idempotency_key: idempotency_key.clone(),
            }),
            chain_operator_caller(),
        )
        .await
        .unwrap();

    let (window_id, retry_stage_execution_id, work_item_id) = match commanded.result {
        CommandResult::EscalationChainResumed {
            deadline_window_id,
            retry_stage_execution_id,
            work_item_id,
            provider,
            tier_id,
            replayed,
            ..
        } => {
            assert_eq!(provider, "gemini_acp");
            assert_eq!(tier_id, CURRENT_TIER_ID);
            assert!(!replayed);
            (deadline_window_id, retry_stage_execution_id, work_item_id)
        }
        _ => panic!("unexpected command result"),
    };

    let run = runs::find_by_id(&fixture.pool, fixture.run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(run.status, RunStatus::Running);
    assert_eq!(
        run.current_state.as_deref(),
        Some("state_5_proposal_refined")
    );

    let ledger = escalation_repo::find_ledger_by_id(&fixture.pool, LEDGER_ID)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ledger.created_at, fixture.ledger_created_at);
    assert_eq!(ledger.status_raw, "active");
    assert_eq!(ledger.current_tier_id.as_deref(), Some(CURRENT_TIER_ID));
    assert_eq!(
        ledger.current_tier_kind_raw.as_deref(),
        Some("backend_profile")
    );
    assert_eq!(ledger.pause_reason_raw, None);
    assert_eq!(ledger.chain_attempt_index, 4);

    let windows = escalation_repo::find_deadline_windows_by_ledger(&fixture.pool, LEDGER_ID)
        .await
        .unwrap();
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].id, window_id);
    assert_eq!(windows[0].tier_id, CURRENT_TIER_ID);
    assert_eq!(
        windows[0].source_pause_reason_raw,
        "escalation_chain_exhausted"
    );

    let retry_stage = stages::find_by_id(&fixture.pool, retry_stage_execution_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retry_stage.status, StageStatus::Running);
    assert!(retry_stage
        .retry_reason
        .as_deref()
        .is_some_and(|reason| reason.starts_with("p058_chain_resume:")));

    let pending = work_items::list_by_run(&fixture.pool, fixture.run_id)
        .await
        .unwrap()
        .into_iter()
        .filter(|item| {
            item.kind == WorkItemKind::InvokeAgent && item.status == WorkItemStatus::Pending
        })
        .collect::<Vec<_>>();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, work_item_id);
    let payload: serde_json::Value = serde_json::from_str(&pending[0].payload_json).unwrap();
    assert_eq!(payload["provider"], "gemini_acp");
    assert_eq!(payload["backend_profile_id"], "gemini_reasoning");
    assert_eq!(
        payload.pointer("/operator_retry_instruction/instruction"),
        Some(&serde_json::json!(
            "Remove only fields rejected by the rollout contract validator."
        ))
    );
    assert_eq!(
        payload.pointer("/operator_retry_instruction/scope_kind"),
        Some(&serde_json::json!("targeted_retry"))
    );
    assert_eq!(
        payload.pointer("/targeted_retry/reason"),
        Some(&serde_json::json!("p058_exhausted_chain_resume"))
    );
    assert_eq!(
        payload.pointer("/targeted_retry/escalation/deadline_window_id"),
        Some(&serde_json::json!(window_id))
    );

    let authority_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM retry_stage_execution_authorities WHERE target_stage_execution_id = ?1 AND authority_state = 'active'",
    )
    .bind(retry_stage_execution_id.to_string())
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert_eq!(authority_count, 1);

    let instruction_binding_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM retry_operator_instruction_bindings WHERE retry_stage_execution_id = ?1 AND instruction_text = ?2",
    )
    .bind(retry_stage_execution_id.to_string())
    .bind("Remove only fields rejected by the rollout contract validator.")
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert_eq!(instruction_binding_count, 1);
    let instruction_delivery_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM retry_operator_instruction_deliveries WHERE work_item_id = ?1 AND status = 'pending'",
    )
    .bind(&work_item_id)
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert_eq!(instruction_delivery_count, 1);

    let events = escalation_repo::find_events_by_ledger(&fixture.pool, LEDGER_ID)
        .await
        .unwrap();
    assert!(events
        .iter()
        .any(|event| event.event_kind_raw == "escalation.chain_window_resumed"));

    let replay = fixture
        .handler
        .handle(
            Command::ResumeEscalationChain(ResumeEscalationChainCmd {
                run_id: fixture.run_id,
                escalation_ledger_id: LEDGER_ID.into(),
                target_tier_id: CURRENT_TIER_ID.into(),
                reason: "Open one audited recovery attempt after provider validation".into(),
                operator_instruction: Some(
                    "Remove only fields rejected by the rollout contract validator.".into(),
                ),
                idempotency_key,
            }),
            chain_operator_caller(),
        )
        .await
        .unwrap();
    assert!(matches!(
        replay.result,
        CommandResult::EscalationChainResumed { replayed: true, .. }
    ));
    assert_eq!(
        escalation_repo::find_deadline_windows_by_ledger(&fixture.pool, LEDGER_ID)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn p058_exhausted_chain_resume_rejects_invalid_tier_without_mutation() {
    let fixture = setup_exhausted_chain_fixture().await;
    let result = fixture
        .handler
        .handle(
            Command::ResumeEscalationChain(ResumeEscalationChainCmd {
                run_id: fixture.run_id,
                escalation_ledger_id: LEDGER_ID.into(),
                target_tier_id: "human_pause".into(),
                reason: "Must remain denied".into(),
                operator_instruction: None,
                idempotency_key: uuid::Uuid::now_v7().to_string(),
            }),
            chain_operator_caller(),
        )
        .await;
    let error = match result {
        Ok(_) => panic!("pause tier must not be queued as a provider recovery"),
        Err(error) => error,
    };
    assert!(error
        .to_string()
        .contains("P058_CHAIN_RESUME_TIER_NOT_SUPPORTED"));
    assert!(
        escalation_repo::find_deadline_windows_by_ledger(&fixture.pool, LEDGER_ID)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        work_items::list_by_run(&fixture.pool, fixture.run_id)
            .await
            .unwrap()
            .into_iter()
            .filter(|item| item.kind == WorkItemKind::InvokeAgent
                && item.status == WorkItemStatus::Pending)
            .count(),
        0
    );
    let ledger = escalation_repo::find_ledger_by_id(&fixture.pool, LEDGER_ID)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ledger.status_raw, "paused");
    assert_eq!(
        ledger.pause_reason_raw.as_deref(),
        Some("escalation_chain_exhausted")
    );
}

#[tokio::test]
async fn p058_exhausted_chain_resume_rejects_invalid_instruction_without_mutation() {
    let fixture = setup_exhausted_chain_fixture().await;
    let result = fixture
        .handler
        .handle(
            Command::ResumeEscalationChain(ResumeEscalationChainCmd {
                run_id: fixture.run_id,
                escalation_ledger_id: LEDGER_ID.into(),
                target_tier_id: CURRENT_TIER_ID.into(),
                reason: "Must remain denied".into(),
                operator_instruction: Some("   ".into()),
                idempotency_key: uuid::Uuid::now_v7().to_string(),
            }),
            chain_operator_caller(),
        )
        .await;
    let error = match result {
        Ok(_) => panic!("blank operator instruction must be rejected"),
        Err(error) => error,
    };
    assert!(error
        .to_string()
        .contains("operator_instruction validation"));
    assert!(
        escalation_repo::find_deadline_windows_by_ledger(&fixture.pool, LEDGER_ID)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        work_items::list_by_run(&fixture.pool, fixture.run_id)
            .await
            .unwrap()
            .into_iter()
            .filter(|item| item.kind == WorkItemKind::InvokeAgent
                && item.status == WorkItemStatus::Pending)
            .count(),
        0
    );
}

#[tokio::test]
async fn p058_exhausted_chain_resume_requires_operator_at_engine_boundary() {
    let fixture = setup_exhausted_chain_fixture().await;
    let caller = CallerContext::mcp(
        "agent-p058",
        &PrincipalClass::Agent,
        "runs.resume_escalation_chain",
    );
    let result = fixture
        .handler
        .handle(
            Command::ResumeEscalationChain(ResumeEscalationChainCmd {
                run_id: fixture.run_id,
                escalation_ledger_id: LEDGER_ID.into(),
                target_tier_id: CURRENT_TIER_ID.into(),
                reason: "Agent must not resume".into(),
                operator_instruction: None,
                idempotency_key: uuid::Uuid::now_v7().to_string(),
            }),
            caller,
        )
        .await;
    let error = match result {
        Ok(_) => panic!("non-operator caller must be rejected"),
        Err(error) => error,
    };
    assert!(error
        .to_string()
        .contains("ResumeEscalationChain requires operator principal"));
}
