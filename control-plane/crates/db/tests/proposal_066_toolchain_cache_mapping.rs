/// P066 Phase 0 DB integration tests.
///
/// Covers:
/// - Migration 037 adds actual_toolchain_mapping_diagnostics_json column
/// - Column is nullable, new rows default to NULL
/// - update_toolchain_mapping_diagnostics writes and reads back correctly
/// - find_by_id returns the stored JSON

use chrono::Utc;
use db::pool::create_pool;
use db::repos::{agent_executions, ideas, runs, stages};
use domain::agent::{AgentExecution, AgentStatus};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{AgentExecutionId, IdeaId, RunId, StageExecutionId};
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageStatus};

async fn setup_db() -> sqlx::SqlitePool {
    create_pool("sqlite::memory:").await.expect("in-memory pool failed")
}

async fn seed_execution(pool: &sqlx::SqlitePool) -> AgentExecutionId {
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_id = StageExecutionId::new();
    let exec_id = AgentExecutionId::new();

    ideas::insert(pool, &Idea {
        id: idea_id,
        title: "P066 test".into(),
        body: "body".into(),
        workspace_root_path: None,
        project_key: None,
        status: IdeaStatus::Active,
        created_at: Utc::now(),
        archived_at: None,
    }).await.unwrap();

    runs::insert(pool, &Run {
        id: run_id,
        idea_id,
        status: RunStatus::Running,
        workflow_id: "wf-p066".into(),
        workflow_title: "P066".into(),
        workspace_root: "/tmp/ws".into(),
        artifact_root: "/tmp/art".into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        cancellation_settlement_log: None,
        current_state: None,
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
    }).await.unwrap();

    stages::insert(pool, &StageExecution {
        id: stage_id,
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
    }).await.unwrap();

    agent_executions::insert(pool, &AgentExecution {
        id: exec_id,
        stage_execution_id: Some(stage_id),
        agent_id: "test_agent".to_string(),
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
    }).await.unwrap();

    exec_id
}

#[tokio::test]
async fn p066_migration_adds_toolchain_diagnostics_column() {
    let pool = setup_db().await;
    let rows: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('agent_executions')")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(
        rows.contains(&"actual_toolchain_mapping_diagnostics_json".to_string()),
        "migration 037 should add actual_toolchain_mapping_diagnostics_json column"
    );
}

#[tokio::test]
async fn p066_new_execution_has_null_toolchain_diagnostics() {
    let pool = setup_db().await;
    let exec_id = seed_execution(&pool).await;

    let found = agent_executions::find_by_id(&pool, exec_id)
        .await
        .unwrap()
        .expect("execution should exist");
    assert!(
        found.actual_toolchain_mapping_diagnostics_json.is_none(),
        "new execution without toolchain mapping should have NULL diagnostics"
    );
}

#[tokio::test]
async fn p066_update_toolchain_diagnostics_persists_and_reads_back() {
    let pool = setup_db().await;
    let exec_id = seed_execution(&pool).await;

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

    agent_executions::update_toolchain_mapping_diagnostics(&pool, exec_id, &diagnostics_json)
        .await
        .unwrap();

    let found = agent_executions::find_by_id(&pool, exec_id)
        .await
        .unwrap()
        .expect("execution should exist");
    let stored = found
        .actual_toolchain_mapping_diagnostics_json
        .expect("diagnostics should be non-null after update");

    let parsed: serde_json::Value = serde_json::from_str(&stored).unwrap();
    assert_eq!(parsed["mapping_state"].as_str().unwrap(), "disabled_by_policy");
    assert_eq!(parsed["mapping_enabled"].as_bool().unwrap(), false);
    assert_eq!(parsed["policy_source"].as_str().unwrap(), "runplan_snapshot");
}
