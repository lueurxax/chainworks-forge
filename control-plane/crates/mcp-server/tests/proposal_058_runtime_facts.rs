use std::sync::Arc;

use chrono::Utc;
use db::pool::create_pool;
use db::repos::{
    agent_execution_runtime_facts, agent_executions, agent_retry_budget_ledger, ideas, runs,
    sessions, stages,
};
use domain::agent::{
    AgentExecution, AgentExecutionRuntimeFacts, AgentFailureKind, AgentOutputSettlement,
};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{AgentExecutionId, IdeaId, RunId, StageExecutionId};
use domain::run::{Run, RunStatus};
use domain::session::{SessionGeneration, SessionGenerationStatus, SessionLineage};
use domain::stage::{StageExecution, StageStatus};
use engine::command_handler::CommandHandler;
use engine::event_bus;
use engine::work_queue::WorkQueue;
use mcp_server::protocol::JsonRpcRequest;
use mcp_server::server::McpServer;

fn make_run(run_id: RunId, idea_id: IdeaId) -> Run {
    Run {
        id: run_id,
        idea_id,
        status: RunStatus::Ready,
        workflow_id: "wf".into(),
        workflow_title: "Workflow".into(),
        workspace_root: "/tmp/ws".into(),
        artifact_root: "/tmp/artifacts".into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        cancellation_settlement_log: None,
        current_state: Some("state_1".into()),
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

async fn seed_execution(pool: &sqlx::SqlitePool) -> (RunId, StageExecutionId, AgentExecutionId) {
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();
    let agent_execution_id = AgentExecutionId::new();

    ideas::insert(
        pool,
        &Idea {
            id: idea_id,
            title: "Idea".into(),
            body: "Body".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        },
    )
    .await
    .unwrap();
    runs::insert(pool, &make_run(run_id, idea_id))
        .await
        .unwrap();
    stages::insert(
        pool,
        &StageExecution {
            id: stage_execution_id,
            run_id,
            stage_id: "state_1".into(),
            label: "State 1".into(),
            status: StageStatus::Running,
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
    agent_executions::insert(
        pool,
        &AgentExecution {
            id: agent_execution_id,
            stage_execution_id,
            agent_id: "code_writer".into(),
            provider: "claude".into(),
            model: Some("sonnet".into()),
            started_at: Utc::now(),
            completed_at: None,
            status: domain::agent::AgentStatus::Running,
            owner_execution_lineage_id: Some("lineage-owner-1".into()),
            session_lineage_id: Some("session-lineage-1".into()),
            session_generation_id: Some("session-generation-1".into()),
            rehydrated_from_checkpoint_artifact_id: Some("checkpoint-1".into()),
            invocation_owner_key: Some("owner-key".into()),
            session_reuse_scope: Some("same_agent_family_within_run".into()),
            session_family_id: Some("family-1".into()),
            session_reuse_disposition: Some("fresh".into()),
            session_reset_reason: None,
            backend_profile_id: Some("codex_with_mcp".into()),
            requested_mcp_extensions_json: Some(r#"["filesystem"]"#.into()),
            predicted_mcp_extensions_json: Some(r#"["filesystem"]"#.into()),
            predicted_mcp_runtime_ids_json: Some(r#"["fs-runtime"]"#.into()),
            actual_mcp_extensions_json: Some(r#"["filesystem"]"#.into()),
            actual_mcp_runtime_ids_json: Some(r#"["fs-runtime"]"#.into()),
            denied_mcp_extensions_json: Some("[]".into()),
            mcp_blocking_issues_json: Some("[]".into()),
            actual_mcp_observation_json: Some(
                r#"{"source":"provider_session_new_response"}"#.into(),
            ),
            mcp_session_startup_latency_ms: Some(17),
        },
    )
    .await
    .unwrap();

    sessions::insert_lineage(
        pool,
        &SessionLineage {
            id: "session-lineage-1".into(),
            run_id: run_id.to_string(),
            agent_id: "code_writer".into(),
            lineage_id: "session-family-1".into(),
            session_reuse_scope: "same_agent_family_within_run".into(),
            session_family_id: Some("family-1".into()),
            active_generation_id: Some("session-generation-1".into()),
            created_at: Utc::now(),
            closed_at: None,
        },
    )
    .await
    .unwrap();
    sessions::insert_generation(
        pool,
        &SessionGeneration {
            id: "session-generation-1".into(),
            lineage_id: "session-lineage-1".into(),
            generation: 1,
            invocation_owner_key: "owner-key".into(),
            provider_session_id: Some("provider-session-1".into()),
            binding_fingerprint: "fingerprint-1".into(),
            rehydrated_from_checkpoint_artifact_id: Some("checkpoint-1".into()),
            working_directory: "/tmp/ws".into(),
            workspace_mode: "workspace".into(),
            runtime_provider: "claude".into(),
            runtime_model: "sonnet".into(),
            status: SessionGenerationStatus::Active,
            turn_count: 0,
            estimated_input_tokens: 0,
            latest_cached_input_tokens: None,
            latest_output_tokens: None,
            latest_model_context_window: None,
            cumulative_prompt_tokens: 0,
            cumulative_cost_cents: 0,
            created_at: Utc::now(),
            last_activity_at: None,
            ended_at: None,
            end_reason: None,
        },
    )
    .await
    .unwrap();

    (run_id, stage_execution_id, agent_execution_id)
}

fn make_command_handler(pool: sqlx::SqlitePool) -> CommandHandler {
    let events = event_bus::new_bus(32);
    CommandHandler::new(pool.clone(), events.clone(), WorkQueue::new(pool))
}

#[tokio::test]
async fn proposal_058_reports_get_includes_runtime_facts_with_snake_case_fields() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let (run_id, stage_execution_id, agent_execution_id) = seed_execution(&pool).await;
    sqlx::query("UPDATE agent_executions SET session_reuse_disposition = ?1 WHERE id = ?2")
        .bind("fresh_after_transport_error")
        .bind(agent_execution_id.to_string())
        .execute(&pool)
        .await
        .unwrap();
    let ledger = agent_retry_budget_ledger::upsert_quota_failure(
        &pool,
        run_id,
        stage_execution_id,
        agent_execution_id,
        Some(Utc::now() + chrono::Duration::minutes(30)),
    )
    .await
    .unwrap();
    let now = Utc::now();
    let mut facts = AgentExecutionRuntimeFacts::defaults_for(agent_execution_id, now);
    facts.failure_kind = Some(AgentFailureKind::ProviderQuota);
    facts.failure_kind_raw_debug = Some("future_provider_quota_variant".into());
    facts.failure_message_redacted = Some("limit resets 10pm (Asia/Nicosia)".into());
    facts.retry_after = Some(now);
    facts.operator_action_hint = Some(domain::agent::OperatorActionHint::WaitUntilRetryAfter);
    facts.output_settlement = AgentOutputSettlement::MissingRequiredOutputs;
    facts.valid_required_outputs = false;
    facts.late_output_count = 2;
    facts.ignored_late_output_count = 1;
    facts.session_reuse_reason = Some("same_family_within_run".into());
    facts.quota_ledger_id = Some(ledger.id.clone());
    agent_execution_runtime_facts::upsert(&pool, &facts)
        .await
        .unwrap();

    let payload = mcp_server::tools::reports::execute(
        "reports.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &make_command_handler(pool.clone()),
        &auth::Principal::new("operator", auth::PrincipalClass::Operator),
    )
    .await
    .unwrap();

    let canonical = payload
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["report_kind"] == serde_json::json!("mcp_execution_truth"))
        .expect("mcp execution truth report");
    let execution = &canonical["agent_executions"][0];
    let runtime_facts = &execution["runtime_facts"];

    assert_eq!(
        execution["agent_execution_id"],
        agent_execution_id.to_string()
    );
    assert_eq!(
        execution["backend_profile_id"],
        serde_json::json!("codex_with_mcp")
    );
    assert_eq!(
        runtime_facts["agent_execution_id"],
        agent_execution_id.to_string()
    );
    assert_eq!(runtime_facts["failure_kind"], "provider_quota");
    assert_eq!(
        runtime_facts["failure_kind_raw_debug"],
        "future_provider_quota_variant"
    );
    assert_eq!(runtime_facts["failure_kind_version"], 1);
    assert_eq!(
        runtime_facts["failure_message_redacted"],
        "limit resets 10pm (Asia/Nicosia)"
    );
    assert_eq!(runtime_facts["retry_after"], now.to_rfc3339());
    assert_eq!(
        runtime_facts["operator_action_hint"],
        "wait_until_retry_after"
    );
    assert_eq!(
        runtime_facts["output_settlement"],
        "missing_required_outputs"
    );
    assert_eq!(runtime_facts["valid_required_outputs"], false);
    assert_eq!(runtime_facts["late_output_count"], 2);
    assert_eq!(runtime_facts["ignored_late_output_count"], 1);
    assert_eq!(
        runtime_facts["session_reuse_reason"],
        "same_family_within_run"
    );
    assert_eq!(
        runtime_facts["fresh_provider_process"],
        serde_json::json!(true)
    );
    assert_eq!(runtime_facts["provider_session_id"], "provider-session-1");
    assert_eq!(
        runtime_facts["active_session_generation_id"],
        "session-generation-1"
    );
    assert_eq!(runtime_facts["active_generation_matches_execution"], true);
    assert_eq!(runtime_facts["generation_status"], "active");
    assert_eq!(runtime_facts["quota_ledger_id"], ledger.id);
    assert!(runtime_facts["created_at"].is_string());
    assert!(runtime_facts["updated_at"].is_string());

    let observer_payload = mcp_server::tools::reports::execute(
        "reports.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &make_command_handler(pool.clone()),
        &auth::Principal::new("observer", auth::PrincipalClass::Observer),
    )
    .await
    .unwrap();
    let observer_canonical = observer_payload
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["report_kind"] == serde_json::json!("mcp_execution_truth"))
        .expect("observer mcp execution truth report");
    assert_eq!(
        observer_canonical["agent_executions"][0]["runtime_facts"]["failure_kind_raw_debug"],
        serde_json::Value::Null
    );

    let server = McpServer::new(
        pool.clone(),
        Arc::new(make_command_handler(pool.clone())),
        auth::PrincipalTable::test_fixture(),
    );
    let resource_response = server
        .handle_request(
            JsonRpcRequest {
                jsonrpc: "2.0".into(),
                id: Some(serde_json::json!(1)),
                method: "resources/read".into(),
                params: Some(serde_json::json!({
                    "uri": format!("report://{}", run_id),
                })),
            },
            &auth::Principal::new("observer", auth::PrincipalClass::Observer),
        )
        .await;
    assert!(
        resource_response.error.is_none(),
        "resource read error: {:?}",
        resource_response.error
    );
    let resource_text = resource_response.result.as_ref().unwrap()["contents"][0]["text"]
        .as_str()
        .expect("resource text");
    let resource_payload: serde_json::Value = serde_json::from_str(resource_text).unwrap();
    let resource_runtime_facts = &resource_payload["agent_executions"][0]["runtime_facts"];
    assert_eq!(
        resource_runtime_facts["agent_execution_id"],
        agent_execution_id.to_string()
    );
    assert_eq!(resource_runtime_facts["failure_kind"], "provider_quota");
    assert_eq!(
        resource_runtime_facts["failure_kind_raw_debug"],
        serde_json::Value::Null
    );
    assert_eq!(
        resource_runtime_facts["failure_message_redacted"],
        "limit resets 10pm (Asia/Nicosia)"
    );
    assert_eq!(
        resource_runtime_facts["session_generation_id"],
        "session-generation-1"
    );
    assert_eq!(
        resource_runtime_facts["fresh_provider_process"],
        serde_json::json!(true)
    );
    assert_eq!(resource_runtime_facts["quota_ledger_id"], ledger.id);
}
