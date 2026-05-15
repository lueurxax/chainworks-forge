/// P066 Phase 0 MCP surface tests.
///
/// Covers:
/// - NULL actual_toolchain_mapping_diagnostics_json → legacy_row_unavailable sentinel
/// - Stored disabled_by_policy JSON → correct MCP key exposure
/// - policy_source is always runplan_snapshot or synthesized_legacy (never agent_catalog)
/// - Absolute paths are not exposed in MCP reports
use chrono::Utc;
use db::pool::create_pool;
use db::repos::{agent_executions, ideas, runs, stages};
use domain::agent::{AgentExecution, AgentStatus};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{AgentExecutionId, IdeaId, RunId, StageExecutionId};
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageStatus};
use engine::command_handler::CommandHandler;
use engine::event_bus;
use engine::work_queue::WorkQueue;

async fn test_pool() -> sqlx::SqlitePool {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    db::writer::register_shared_writer(
        &pool,
        std::sync::Arc::new(db::writer::DbWriter::new(pool.clone())),
    )
    .await
    .unwrap();
    pool
}

fn make_run(run_id: RunId, idea_id: IdeaId) -> Run {
    Run {
        id: run_id,
        idea_id,
        status: RunStatus::Running,
        workflow_id: "wf-p066-mcp".into(),
        workflow_title: "P066 MCP Test".into(),
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
        review_routing_json: None,
        closeout_readiness_mode: None,
    }
}

fn make_command_handler(pool: sqlx::SqlitePool) -> CommandHandler {
    let events = event_bus::new_bus(16);
    CommandHandler::new(pool.clone(), events, WorkQueue::new(pool))
}

async fn seed_execution_with_diagnostics(
    pool: &sqlx::SqlitePool,
    diagnostics_json: Option<&str>,
) -> (RunId, StageExecutionId, AgentExecutionId) {
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();
    let agent_execution_id = AgentExecutionId::new();

    ideas::insert(
        pool,
        &Idea {
            id: idea_id,
            title: "P066 MCP test idea".into(),
            body: "body".into(),
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
            stage_execution_id: Some(stage_execution_id),
            agent_id: "code_writer".to_string(),
            provider: "claude".to_string(),
            model: Some("claude-sonnet-4-6".to_string()),
            started_at: Utc::now(),
            completed_at: None,
            status: AgentStatus::Running,
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
            owner_kind: None,
            owner_id: None,
            lead_mediation_record_id: None,
            origin_stage_execution_id: None,
            total_cost_cents: None,
            input_tokens: None,
            output_tokens: None,
            cached_input_tokens: None,
            transcript_artifact_id: None,
            actual_toolchain_mapping_diagnostics_json: None,
            escalation_policy_id: None,
            escalation_policy_hash: None,
            escalation_tier_id: None,
            escalation_tier_kind_raw: None,
            escalation_trigger_raw: None,
            escalation_digest_version: None,
            escalation_ledger_id: None,
        },
    )
    .await
    .unwrap();

    if let Some(json) = diagnostics_json {
        agent_executions::update_toolchain_mapping_diagnostics(pool, agent_execution_id, json)
            .await
            .unwrap();
    }

    (run_id, stage_execution_id, agent_execution_id)
}

fn mcp_execution_truth_report(payload: &serde_json::Value) -> &serde_json::Value {
    payload
        .as_array()
        .expect("payload must be array")
        .iter()
        .find(|item| item["report_kind"] == "mcp_execution_truth")
        .expect("mcp_execution_truth report must be present")
}

/// P066: NULL diagnostics column → legacy_row_unavailable sentinel synthesized in MCP.
/// Matches normative example mcp_legacy from proposal §normative_examples.
#[tokio::test]
async fn p066_mcp_null_toolchain_diagnostics_synthesizes_legacy_row_unavailable() {
    let pool = test_pool().await;
    let (run_id, _, _) = seed_execution_with_diagnostics(&pool, None).await;

    let payload = mcp_server::tools::reports::execute(
        "reports.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &make_command_handler(pool.clone()),
        &auth::Principal::new("operator", auth::PrincipalClass::Operator),
    )
    .await
    .unwrap();

    let canonical = mcp_execution_truth_report(&payload);
    let exec = &canonical["agent_executions"][0];
    let diag = &exec["actual_toolchain_mapping_diagnostics"];

    assert_eq!(
        diag["mapping_state"], "legacy_row_unavailable",
        "NULL column must synthesize legacy_row_unavailable"
    );
    assert_eq!(diag["mapping_enabled"], false);
    assert_eq!(diag["inactive_reason"], "legacy_row");
    assert_eq!(diag["policy_source"], "synthesized_legacy");
    assert!(diag["policy_version"].is_null());
    assert_eq!(diag["version"], 1);
}

/// P066: Stored disabled_by_policy JSON → correct MCP key exposure.
/// policy_source must be runplan_snapshot (never agent_catalog).
#[tokio::test]
async fn p066_mcp_disabled_by_policy_diagnostics_exposed_correctly() {
    let diagnostics_json = serde_json::json!({
        "version": 1,
        "mapping_state": "disabled_by_policy",
        "mapping_enabled": false,
        "inactive_reason": "policy_disabled",
        "policy_source": "runplan_snapshot",
        "policy_version": 1,
        "provider_family": "claude"
    })
    .to_string();

    let pool = test_pool().await;
    let (run_id, _, _) = seed_execution_with_diagnostics(&pool, Some(&diagnostics_json)).await;

    let payload = mcp_server::tools::reports::execute(
        "reports.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &make_command_handler(pool.clone()),
        &auth::Principal::new("operator", auth::PrincipalClass::Operator),
    )
    .await
    .unwrap();

    let canonical = mcp_execution_truth_report(&payload);
    let exec = &canonical["agent_executions"][0];
    let diag = &exec["actual_toolchain_mapping_diagnostics"];

    assert_eq!(diag["mapping_state"], "disabled_by_policy");
    assert_eq!(diag["mapping_enabled"], false);
    assert_eq!(diag["inactive_reason"], "policy_disabled");
    assert_eq!(
        diag["policy_source"], "runplan_snapshot",
        "policy_source must be runplan_snapshot for compiled executions"
    );
    assert_ne!(
        diag["policy_source"], "agent_catalog",
        "agent_catalog must never be an authoritative policy_source per DEC-003"
    );
    assert_eq!(diag["policy_version"], 1);
    assert_eq!(diag["provider_family"], "claude");
    assert_eq!(diag["version"], 1);
}

/// P066: Absolute filesystem paths must not appear in MCP reports.
#[tokio::test]
async fn p066_mcp_absolute_paths_not_exposed_in_reports() {
    let diagnostics_json = serde_json::json!({
        "version": 1,
        "mapping_state": "active",
        "mapping_enabled": true,
        "inactive_reason": null,
        "policy_source": "runplan_snapshot",
        "policy_version": 1,
        "provider_family": "xcode",
        "absolute_derived_data_path": "/Users/testuser/toolchain_home/providers/xcode/run-xyz/xcode/DerivedData"
    })
    .to_string();

    let pool = test_pool().await;
    let (run_id, _, _) = seed_execution_with_diagnostics(&pool, Some(&diagnostics_json)).await;

    let payload = mcp_server::tools::reports::execute(
        "reports.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &make_command_handler(pool.clone()),
        &auth::Principal::new("operator", auth::PrincipalClass::Operator),
    )
    .await
    .unwrap();

    let canonical = mcp_execution_truth_report(&payload);
    let diag = &canonical["agent_executions"][0]["actual_toolchain_mapping_diagnostics"];
    let diag_str = diag.to_string();

    assert!(
        !diag_str.contains("/Users/testuser"),
        "absolute paths must not appear in MCP diagnostics payload"
    );
    assert_eq!(diag["mapping_state"], "active");
}
