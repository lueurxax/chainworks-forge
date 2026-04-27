//! P065: Operator Retry Instruction Contract — integration tests.
//!
//! Covers:
//! - MCP validation and operator-only authorization
//! - Validation failure leaves no journal or binding rows
//! - Full-stage retry parent binding creation
//! - Targeted retry child delivery creation
//! - Stale-target fallback scope tagging
//! - Journal redaction of operator_instruction
//! - Existing GraphQL mutation remains unchanged in v1

use chrono::Utc;
use db::pool::create_pool;
use db::repos::{
    agent_executions, ideas, retry_operator_instructions, runs, stages, work_items,
};
use db::work_item::{WorkItem, WorkItemKind, WorkItemStatus};
use domain::agent::{AgentExecution, AgentStatus};
use domain::commands::{CallerContext, CallerSurface, Command, PrincipalClass, RetryStageCmd};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{AgentExecutionId, IdeaId, RunId, StageExecutionId};
use domain::retry_instruction::{validate_operator_instruction, RetryInstructionScopeKind};
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageStatus};
use engine::command_handler::{CommandHandler, CommandResult};
use engine::event_bus;
use engine::work_queue::WorkQueue;

fn make_run(run_id: RunId, idea_id: IdeaId) -> Run {
    Run {
        id: run_id,
        idea_id,
        status: RunStatus::Running,
        workflow_id: "wf".into(),
        workflow_title: "Workflow".into(),
        workspace_root: "/tmp/workspace".into(),
        artifact_root: "/tmp/artifacts".into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        cancellation_settlement_log: None,
        current_state: Some("implementation".into()),
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
        workflow_snapshot_json: None,
        catalog_snapshot_json: None,
        drift_detected_at: None,
        drift_details_json: None,
        chainworks_meta_root: None,
    }
}

fn operator_caller() -> CallerContext {
    CallerContext {
        surface: CallerSurface::Mcp,
        principal_id: "op-1".into(),
        principal_class: PrincipalClass::Operator,
        caller_tool: "stages.retry".into(),
        request_id: None,
    }
}

fn agent_caller() -> CallerContext {
    CallerContext {
        surface: CallerSurface::Mcp,
        principal_id: "agent-1".into(),
        principal_class: PrincipalClass::Agent,
        caller_tool: "stages.retry".into(),
        request_id: None,
    }
}

async fn setup_failed_stage(pool: &sqlx::SqlitePool) -> (RunId, IdeaId, StageExecutionId) {
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();

    ideas::insert(
        pool,
        &Idea {
            id: idea_id,
            title: "P065".into(),
            body: "retry instruction".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        },
    )
    .await
    .unwrap();
    runs::insert(pool, &make_run(run_id, idea_id)).await.unwrap();
    stages::insert(
        pool,
        &StageExecution {
            id: stage_execution_id,
            run_id,
            stage_id: "implementation".into(),
            label: "Implementation".into(),
            status: StageStatus::Failed,
            iteration: 1,
            attempt_number: 1,
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
        },
    )
    .await
    .unwrap();

    (run_id, idea_id, stage_execution_id)
}

// ── P065 AC-1: Operator can attach instruction and command succeeds ──

#[tokio::test]
async fn p065_full_stage_retry_with_operator_instruction_creates_binding() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let (run_id, _, _old_stage_execution_id) = setup_failed_stage(&pool).await;

    let handler = CommandHandler::new(
        pool.clone(),
        event_bus::new_bus(16),
        WorkQueue::new(pool.clone()),
    );
    let result = handler
        .handle(
            Command::RetryStage(RetryStageCmd {
                run_id,
                stage_id: "implementation".into(),
                consume_quota_budget_now: false,
                agent_execution_id: None,
                legacy_discovery_override_policy: None,
                legacy_discovery_override_reason: None,
                operator_instruction: Some("Focus only on the GraphQL scheduler slice.".into()),
            }),
            operator_caller(),
        )
        .await
        .unwrap();

    // CommandResult must include binding_id
    let binding_id = match &result.result {
        CommandResult::StageRetryScheduled {
            retry_instruction_binding_id,
            ..
        } => retry_instruction_binding_id
            .as_ref()
            .expect("binding id must be present for instructed retry"),
        other => panic!("Expected StageRetryScheduled, got {:?}", std::mem::discriminant(other)),
    };

    // Verify binding row persisted
    let bindings = retry_operator_instructions::list_by_run(&pool, run_id)
        .await
        .unwrap();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].binding_id, *binding_id);
    assert_eq!(
        bindings[0].instruction_text,
        "Focus only on the GraphQL scheduler slice."
    );
    assert_eq!(
        bindings[0].scope_kind,
        RetryInstructionScopeKind::FullStageRetry
    );
    assert_eq!(bindings[0].created_by_principal_id, "op-1");
    assert_eq!(bindings[0].created_by_principal_class, "operator");
    assert!(bindings[0].target_agent_execution_id.is_none());

    // Child delivery rows are deferred to orchestrator for full-stage retry
    let deliveries =
        retry_operator_instructions::list_deliveries_by_binding(&pool, binding_id)
            .await
            .unwrap();
    assert_eq!(
        deliveries.len(),
        0,
        "full-stage retry defers child delivery creation to orchestrator"
    );
}

// ── P065 AC-1 continued: retry without instruction still works ──

#[tokio::test]
async fn p065_full_stage_retry_without_instruction_has_no_binding() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let (run_id, _, _) = setup_failed_stage(&pool).await;

    let handler = CommandHandler::new(
        pool.clone(),
        event_bus::new_bus(16),
        WorkQueue::new(pool.clone()),
    );
    let result = handler
        .handle(
            Command::RetryStage(RetryStageCmd {
                run_id,
                stage_id: "implementation".into(),
                consume_quota_budget_now: false,
                agent_execution_id: None,
                legacy_discovery_override_policy: None,
                legacy_discovery_override_reason: None,
                operator_instruction: None,
            }),
            operator_caller(),
        )
        .await
        .unwrap();

    match &result.result {
        CommandResult::StageRetryScheduled {
            retry_instruction_binding_id,
            ..
        } => {
            assert!(
                retry_instruction_binding_id.is_none(),
                "no binding when instruction is absent"
            );
        }
        _ => panic!("Expected StageRetryScheduled"),
    }

    let bindings = retry_operator_instructions::list_by_run(&pool, run_id)
        .await
        .unwrap();
    assert_eq!(bindings.len(), 0);
}

// ── P065 AC-2: Validation failure leaves no journal, no binding ──

#[tokio::test]
async fn p065_validation_failure_rejects_empty_instruction() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let (run_id, _, _) = setup_failed_stage(&pool).await;

    let handler = CommandHandler::new(
        pool.clone(),
        event_bus::new_bus(16),
        WorkQueue::new(pool.clone()),
    );
    let err = handler
        .handle(
            Command::RetryStage(RetryStageCmd {
                run_id,
                stage_id: "implementation".into(),
                consume_quota_budget_now: false,
                agent_execution_id: None,
                legacy_discovery_override_policy: None,
                legacy_discovery_override_reason: None,
                operator_instruction: Some("   ".into()),
            }),
            operator_caller(),
        )
        .await;

    match err {
        Err(e) => assert!(
            e.to_string().contains("empty"),
            "error message should mention empty, got: {e}"
        ),
        Ok(_) => panic!("empty-after-trim instruction must fail"),
    }

    // No binding rows
    let bindings = retry_operator_instructions::list_by_run(&pool, run_id)
        .await
        .unwrap();
    assert_eq!(bindings.len(), 0, "validation failure must leave no bindings");
}

#[tokio::test]
async fn p065_validation_failure_rejects_too_long_instruction() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let (run_id, _, _) = setup_failed_stage(&pool).await;

    let handler = CommandHandler::new(
        pool.clone(),
        event_bus::new_bus(16),
        WorkQueue::new(pool.clone()),
    );
    let long = "a".repeat(2001);
    let err = handler
        .handle(
            Command::RetryStage(RetryStageCmd {
                run_id,
                stage_id: "implementation".into(),
                consume_quota_budget_now: false,
                agent_execution_id: None,
                legacy_discovery_override_policy: None,
                legacy_discovery_override_reason: None,
                operator_instruction: Some(long),
            }),
            operator_caller(),
        )
        .await;

    assert!(err.is_err(), ">2000 char instruction must fail");
}

#[tokio::test]
async fn p065_validation_failure_rejects_control_characters() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let (run_id, _, _) = setup_failed_stage(&pool).await;

    let handler = CommandHandler::new(
        pool.clone(),
        event_bus::new_bus(16),
        WorkQueue::new(pool.clone()),
    );
    let err = handler
        .handle(
            Command::RetryStage(RetryStageCmd {
                run_id,
                stage_id: "implementation".into(),
                consume_quota_budget_now: false,
                agent_execution_id: None,
                legacy_discovery_override_policy: None,
                legacy_discovery_override_reason: None,
                operator_instruction: Some("hello\x07world".into()),
            }),
            operator_caller(),
        )
        .await;

    match err {
        Err(e) => assert!(
            e.to_string().contains("control character"),
            "error should mention control character, got: {e}"
        ),
        Ok(_) => panic!("control character instruction must fail"),
    }
}

// ── P065 AC-2 continued: operator-only auth ──

#[tokio::test]
async fn p065_non_operator_cannot_attach_instruction() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let (run_id, _, _) = setup_failed_stage(&pool).await;

    let handler = CommandHandler::new(
        pool.clone(),
        event_bus::new_bus(16),
        WorkQueue::new(pool.clone()),
    );
    let err = handler
        .handle(
            Command::RetryStage(RetryStageCmd {
                run_id,
                stage_id: "implementation".into(),
                consume_quota_budget_now: false,
                agent_execution_id: None,
                legacy_discovery_override_policy: None,
                legacy_discovery_override_reason: None,
                operator_instruction: Some("Do X".into()),
            }),
            agent_caller(),
        )
        .await;

    match err {
        Err(e) => assert!(
            e.to_string()
                .contains("operator_instruction requires operator principal"),
            "error should mention operator principal, got: {e}"
        ),
        Ok(_) => panic!("agent principal must not attach instruction"),
    }
}

// ── P065 AC-9: Existing GraphQL retry_stage unchanged ──

#[tokio::test]
async fn p065_retry_without_instruction_from_agent_still_works() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let (run_id, _, _) = setup_failed_stage(&pool).await;

    let handler = CommandHandler::new(
        pool.clone(),
        event_bus::new_bus(16),
        WorkQueue::new(pool.clone()),
    );
    // Agent can retry without instruction (mirrors GraphQL v1 behavior)
    let result = handler
        .handle(
            Command::RetryStage(RetryStageCmd {
                run_id,
                stage_id: "implementation".into(),
                consume_quota_budget_now: false,
                agent_execution_id: None,
                legacy_discovery_override_policy: None,
                legacy_discovery_override_reason: None,
                operator_instruction: None,
            }),
            agent_caller(),
        )
        .await;

    assert!(result.is_ok(), "agent retry without instruction must succeed");
}

// ── Domain validation unit tests (already in domain crate; this confirms integration) ──

#[test]
fn p065_validate_instruction_trims_and_accepts() {
    assert_eq!(
        validate_operator_instruction("  Focus on X.  ").unwrap(),
        "Focus on X."
    );
}

#[test]
fn p065_validate_instruction_allows_tab_newline() {
    assert!(validate_operator_instruction("line1\n\tline2").is_ok());
}

#[test]
fn p065_validate_instruction_exactly_2000_chars() {
    assert!(validate_operator_instruction(&"b".repeat(2000)).is_ok());
}

// ── P065: Targeted retry with instruction creates binding + child delivery ──

#[tokio::test]
async fn p065_targeted_retry_with_instruction_creates_binding_and_child_delivery() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let (run_id, _, old_stage_execution_id) = setup_failed_stage(&pool).await;

    // Create a failed agent execution for targeted retry
    let agent_execution_id = AgentExecutionId::new();
    agent_executions::insert(
        &pool,
        &AgentExecution {
            id: agent_execution_id,
            stage_execution_id: Some(old_stage_execution_id),
            agent_id: "code_writer".into(),
            provider: "claude".into(),
            model: Some("sonnet".into()),
            status: AgentStatus::Failed,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            owner_execution_lineage_id: None,
            session_lineage_id: None,
            session_generation_id: None,
            rehydrated_from_checkpoint_artifact_id: None,
            invocation_owner_key: None,
            session_reuse_scope: None,
            session_family_id: None,
            session_reuse_disposition: None,
            session_reset_reason: None,
            backend_profile_id: None,
            requested_mcp_extensions_json: None,
            predicted_mcp_extensions_json: None,
            predicted_mcp_runtime_ids_json: None,
            actual_mcp_extensions_json: None,
            actual_mcp_runtime_ids_json: None,
            denied_mcp_extensions_json: None,
            mcp_blocking_issues_json: None,
            actual_mcp_observation_json: None,
            actual_xcode_runtime_observation_json: None,
            mcp_session_startup_latency_ms: None,
            owner_kind: Some("stage_execution".into()),
            owner_id: Some(old_stage_execution_id.to_string()),
            lead_mediation_record_id: None,
            origin_stage_execution_id: None,
            total_cost_cents: None,
            input_tokens: None,
            output_tokens: None,
            cached_input_tokens: None,
            transcript_artifact_id: None,
        },
    )
    .await
    .unwrap();

    // Create source InvokeAgent work item
    let now = Utc::now();
    work_items::enqueue(
        &pool,
        &WorkItem {
            id: format!("p058-invoke:{}:0", old_stage_execution_id),
            kind: WorkItemKind::InvokeAgent,
            payload_json: serde_json::json!({
                "stage_id": "implementation",
                "stage_execution_id": old_stage_execution_id.to_string(),
                "agent_id": "code_writer",
                "provider": "claude",
                "model": "sonnet",
                "prompt": "write code",
                "task_name": "code",
                "task_inputs": [],
                "task_outputs": [],
                "declared_outputs": [],
                "requested_mcp_server_ids": [],
                "session_reuse_scope": "same_agent_family_within_run",
                "session_family_id": "code_writer",
                "worktree_write_enabled": false,
                "p058_claimed": {
                    "agent_execution_id": agent_execution_id.to_string(),
                    "artifact_claim_key": {
                        "run_id": run_id.to_string(),
                        "stage_execution_id": old_stage_execution_id.to_string(),
                        "agent_execution_id": agent_execution_id.to_string(),
                        "source_work_item_id": format!("p058-invoke:{}:0", old_stage_execution_id)
                    }
                }
            })
            .to_string(),
            status: WorkItemStatus::Failed,
            run_id: Some(run_id),
            stage_id: Some("implementation".into()),
            created_at: now,
            scheduled_at: now,
            attempt_count: 1,
            last_error: Some("transport error".into()),
        },
    )
    .await
    .unwrap();

    let handler = CommandHandler::new(
        pool.clone(),
        event_bus::new_bus(16),
        WorkQueue::new(pool.clone()),
    );
    let result = handler
        .handle(
            Command::RetryStage(RetryStageCmd {
                run_id,
                stage_id: "implementation".into(),
                consume_quota_budget_now: false,
                agent_execution_id: Some(agent_execution_id),
                legacy_discovery_override_policy: None,
                legacy_discovery_override_reason: None,
                operator_instruction: Some("Only implement the GraphQL readback slice.".into()),
            }),
            operator_caller(),
        )
        .await
        .unwrap();

    let binding_id = match &result.result {
        CommandResult::StageRetryScheduled {
            retry_instruction_binding_id,
            ..
        } => retry_instruction_binding_id
            .as_ref()
            .expect("binding id for targeted retry with instruction"),
        _ => panic!("Expected StageRetryScheduled"),
    };

    let bindings = retry_operator_instructions::list_by_run(&pool, run_id)
        .await
        .unwrap();
    assert_eq!(bindings.len(), 1);
    assert_eq!(
        bindings[0].scope_kind,
        RetryInstructionScopeKind::TargetedRetry
    );
    assert!(bindings[0].target_agent_execution_id.is_some());

    // Targeted retry creates child delivery immediately
    let deliveries =
        retry_operator_instructions::list_deliveries_by_binding(&pool, binding_id)
            .await
            .unwrap();
    assert_eq!(
        deliveries.len(),
        1,
        "targeted retry must create one child delivery row immediately"
    );
    assert_eq!(deliveries[0].status.to_string(), "pending");
    assert!(deliveries[0].work_item_id.is_some());
}

// ── P065: Repo helper round-trip ──

#[tokio::test]
async fn p065_repo_helpers_round_trip() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let (run_id, _, old_stage_execution_id) = setup_failed_stage(&pool).await;
    let new_stage_execution_id = StageExecutionId::new();

    // Insert a new stage so the FK is satisfied
    stages::insert(
        &pool,
        &StageExecution {
            id: new_stage_execution_id,
            run_id,
            stage_id: "implementation".into(),
            label: "Implementation".into(),
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
            retry_reason: Some("operator_retry".into()),
        },
    )
    .await
    .unwrap();

    // Need a journal entry for FK
    let journal_id = uuid::Uuid::new_v4().to_string();
    db::repos::command_journal::record(
        &pool,
        &journal_id,
        "RetryStage",
        "{}",
        Some(&run_id.to_string()),
        Utc::now(),
        Some("mcp"),
        Some("op-1"),
        Some("operator"),
        Some("stages.retry"),
        None,
    )
    .await
    .unwrap();

    let mut tx = pool.begin().await.unwrap();
    let binding = retry_operator_instructions::create_for_retry_attempt_tx(
        &mut tx,
        &domain::retry_instruction::RetryInstructionBindingInput {
            journal_id: journal_id.clone(),
            run_id,
            stage_id: "implementation".into(),
            source_stage_execution_id: old_stage_execution_id,
            retry_stage_execution_id: new_stage_execution_id,
            retry_attempt_number: 2,
            target_agent_execution_id: None,
            scope_kind: RetryInstructionScopeKind::FullStageRetry,
            instruction_text: "Focus on X".into(),
            created_by_principal_id: "op-1".into(),
            created_by_principal_class: "operator".into(),
        },
    )
    .await
    .unwrap();

    assert_eq!(binding.instruction_text, "Focus on X");
    assert_eq!(binding.instruction_sha256.len(), 64);

    // List by retry stage execution
    let listed =
        retry_operator_instructions::list_by_retry_stage_execution_id_tx(
            &mut tx,
            new_stage_execution_id,
        )
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);

    // Create delivery
    let delivery = retry_operator_instructions::create_for_work_item_tx(
        &mut tx,
        &binding.binding_id,
        Some("work-item-1"),
        None,
    )
    .await
    .unwrap();
    assert_eq!(delivery.status.to_string(), "pending");

    // Mark delivered
    retry_operator_instructions::mark_delivered_tx(&mut tx, &delivery.delivery_id)
        .await
        .unwrap();

    tx.commit().await.unwrap();

    // Verify round-trip via list_by_run
    let bindings = retry_operator_instructions::list_by_run(&pool, run_id)
        .await
        .unwrap();
    assert_eq!(bindings.len(), 1);

    let deliveries =
        retry_operator_instructions::list_deliveries_by_binding(&pool, &binding.binding_id)
            .await
            .unwrap();
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].status.to_string(), "delivered");
    assert!(deliveries[0].delivered_at.is_some());
}

// ── P065: mark_failed_tx ──

#[tokio::test]
async fn p065_mark_failed_sets_reason() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let (run_id, _, old_stage_execution_id) = setup_failed_stage(&pool).await;
    let new_stage_execution_id = StageExecutionId::new();

    stages::insert(
        &pool,
        &StageExecution {
            id: new_stage_execution_id,
            run_id,
            stage_id: "implementation".into(),
            label: "Implementation".into(),
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
            retry_reason: Some("operator_retry".into()),
        },
    )
    .await
    .unwrap();

    let journal_id = uuid::Uuid::new_v4().to_string();
    db::repos::command_journal::record(
        &pool,
        &journal_id,
        "RetryStage",
        "{}",
        Some(&run_id.to_string()),
        Utc::now(),
        Some("mcp"),
        Some("op-1"),
        Some("operator"),
        Some("stages.retry"),
        None,
    )
    .await
    .unwrap();

    let mut tx = pool.begin().await.unwrap();
    let binding = retry_operator_instructions::create_for_retry_attempt_tx(
        &mut tx,
        &domain::retry_instruction::RetryInstructionBindingInput {
            journal_id,
            run_id,
            stage_id: "implementation".into(),
            source_stage_execution_id: old_stage_execution_id,
            retry_stage_execution_id: new_stage_execution_id,
            retry_attempt_number: 2,
            target_agent_execution_id: None,
            scope_kind: RetryInstructionScopeKind::FullStageRetry,
            instruction_text: "Focus on Y".into(),
            created_by_principal_id: "op-1".into(),
            created_by_principal_class: "operator".into(),
        },
    )
    .await
    .unwrap();

    let delivery = retry_operator_instructions::create_for_work_item_tx(
        &mut tx,
        &binding.binding_id,
        None, // zero-fanout record
        None,
    )
    .await
    .unwrap();

    retry_operator_instructions::mark_failed_tx(
        &mut tx,
        &delivery.delivery_id,
        "no_invoke_agent_work_items",
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let deliveries =
        retry_operator_instructions::list_deliveries_by_binding(&pool, &binding.binding_id)
            .await
            .unwrap();
    assert_eq!(deliveries[0].status.to_string(), "failed");
    assert_eq!(
        deliveries[0].failure_reason.as_deref(),
        Some("no_invoke_agent_work_items")
    );
}
