use chrono::Utc;
use db::pool::create_pool;
use db::repos::{ideas, output_contract_repair as ocr_repo, runs, stages};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{AgentExecutionId, IdeaId, RunId, StageExecutionId};
use domain::output_contract_repair::{
    LeaseKind, LeaseState, OutputContractRepairEventRow, OutputContractRepairLeaseRow,
};
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageStatus};

async fn setup_db() -> sqlx::SqlitePool {
    let pool = create_pool("sqlite::memory:")
        .await
        .expect("in-memory pool failed");
    let writer = std::sync::Arc::new(db::writer::DbWriter::new(pool.clone()));
    db::writer::register_shared_writer(&pool, writer)
        .await
        .expect("test shared DbWriter registration failed");
    pool
}

async fn seed_run(pool: &sqlx::SqlitePool) -> (RunId, StageExecutionId) {
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_id = StageExecutionId::new();

    ideas::insert(
        pool,
        &Idea {
            id: idea_id,
            title: "P079 test".into(),
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

    runs::insert(
        pool,
        &Run {
            id: run_id,
            idea_id,
            status: RunStatus::Running,
            workflow_id: "wf-p079".into(),
            workflow_title: "P079".into(),
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
            closeout_readiness_mode: None,
        },
    )
    .await
    .unwrap();

    stages::insert(
        pool,
        &StageExecution {
            id: stage_id,
            run_id,
            stage_id: "state_test".into(),
            label: "Test".into(),
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

    (run_id, stage_id)
}

fn make_event_row(
    run_id: RunId,
    stage_id: StageExecutionId,
    agent_exec_id: AgentExecutionId,
) -> OutputContractRepairEventRow {
    let now = Utc::now().to_rfc3339();
    OutputContractRepairEventRow {
        repair_attempt_id: uuid::Uuid::new_v4().to_string(),
        schema_version: "output_contract_repair.v1".into(),
        run_id: run_id.to_string(),
        stage_execution_id: stage_id.to_string(),
        agent_execution_id: agent_exec_id.to_string(),
        session_generation_id: uuid::Uuid::new_v4().to_string(),
        role: "lead_orchestrator".into(),
        provider_family: "gemini".into(),
        adapter_family: "gemini".into(),
        required_output_mode: "chainworks_output".into(),
        initial_failure_class: "missing_required_outputs".into(),
        initial_failure_subtype: None,
        status: "in_progress".into(),
        presentation_category: "informational".into(),
        recommended_next_action: "continue".into(),
        final_output_settlement: None,
        same_session_repair_json: None,
        transcript_recovery_json: None,
        provider_fallback_json: None,
        provider_plan_evidence_json: None,
        required_outputs_json: "[]".into(),
        permission_decisions_json: "[]".into(),
        repair_budget_consumed: false,
        fallback_budget_consumed: false,
        repair_prompt_template_version: Some("p079_repair_v1".into()),
        recovery_parser_version: Some("p079_recovery_v1".into()),
        policy_feature_flags_json: "[]".into(),
        evidence_artifact_path: None,
        lease_id: None,
        evidence_version: 1,
        projection_integrity: "fresh".into(),
        projection_stale_since: None,
        projection_schema_version: "output_contract_repair_events_v1".into(),
        projection_rebuild_attempts: 0,
        recorded_at: now.clone(),
        created_at: now.clone(),
        updated_at: now,
    }
}

fn make_lease_row(
    event_id: &str,
    run_id: RunId,
    stage_id: StageExecutionId,
    agent_exec_id: AgentExecutionId,
) -> OutputContractRepairLeaseRow {
    let now_ts = Utc::now();
    let expires = now_ts + chrono::Duration::seconds(180);
    let lease_key = format!("lk-{}", uuid::Uuid::new_v4());
    OutputContractRepairLeaseRow {
        lease_key,
        schema_version: "output_contract_repair_leases_v1".into(),
        repair_event_id: event_id.into(),
        run_id: run_id.to_string(),
        stage_execution_id: stage_id.to_string(),
        parent_agent_execution_id: agent_exec_id.to_string(),
        lease_kind: LeaseKind::Repair,
        lease_state: LeaseState::Reserved,
        settled_result: None,
        reclamation_reason: None,
        frozen_fallback_policy_hash: None,
        idempotency_token: uuid::Uuid::new_v4().to_string(),
        lease_owner_principal_id: "test-principal".into(),
        lease_acquired_at: now_ts.to_rfc3339(),
        lease_expires_at: expires.to_rfc3339(),
        lease_seconds: 180,
        dispatch_committed_at: None,
        version: 0,
        infra_retry_count: 0,
        created_at: now_ts.to_rfc3339(),
        updated_at: now_ts.to_rfc3339(),
    }
}

#[tokio::test]
async fn proposal_079_event_row_insert_and_fetch() {
    let pool = setup_db().await;
    let (run_id, stage_id) = seed_run(&pool).await;
    let agent_exec_id = AgentExecutionId::new();

    let event = make_event_row(run_id, stage_id, agent_exec_id);
    let attempt_id = event.repair_attempt_id.clone();

    ocr_repo::insert_repair_event(&pool, &event)
        .await
        .expect("insert_repair_event should succeed");

    let fetched =
        ocr_repo::get_repair_event_by_agent_execution_id(&pool, &agent_exec_id.to_string())
            .await
            .expect("get by agent_execution_id should succeed")
            .expect("row should be present");

    assert_eq!(fetched.repair_attempt_id, attempt_id);
    assert_eq!(fetched.status, "in_progress");
    assert_eq!(fetched.provider_family, "gemini");
    assert_eq!(fetched.initial_failure_class, "missing_required_outputs");
    // recorded_at is required by the approved v1 schema (DEFECT-006).
    assert!(!fetched.recorded_at.is_empty(), "recorded_at must be set");
}

#[tokio::test]
async fn proposal_079_lease_reserved_to_prompt_sent() {
    // REL-r2-1: reserved->prompt_sent transition commits dispatch_committed_at.
    let pool = setup_db().await;
    let (run_id, stage_id) = seed_run(&pool).await;
    let agent_exec_id = AgentExecutionId::new();

    let event = make_event_row(run_id, stage_id, agent_exec_id);
    let attempt_id = event.repair_attempt_id.clone();
    ocr_repo::insert_repair_event(&pool, &event).await.unwrap();

    let lease = make_lease_row(&attempt_id, run_id, stage_id, agent_exec_id);
    let lease_key = lease.lease_key.clone();
    ocr_repo::insert_lease(&pool, &lease).await.unwrap();

    let fetched = ocr_repo::get_lease_by_key(&pool, &lease_key)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fetched.lease_state, LeaseState::Reserved);
    assert!(fetched.dispatch_committed_at.is_none());

    let dispatch_ts = Utc::now().to_rfc3339();
    ocr_repo::transition_lease_to_prompt_sent(&pool, &lease_key, &dispatch_ts, &dispatch_ts)
        .await
        .expect("reserved->prompt_sent should succeed");

    let after = ocr_repo::get_lease_by_key(&pool, &lease_key)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.lease_state, LeaseState::PromptSent);
    assert!(
        after.dispatch_committed_at.is_some(),
        "dispatch_committed_at must be set after prompt_sent"
    );
}

#[tokio::test]
async fn proposal_079_settle_terminal_event_and_lease_atomic() {
    // DEFECT-004 fix: settle_terminal_event_and_lease uses a single transaction.
    let pool = setup_db().await;
    let (run_id, stage_id) = seed_run(&pool).await;
    let agent_exec_id = AgentExecutionId::new();

    let event = make_event_row(run_id, stage_id, agent_exec_id);
    let attempt_id = event.repair_attempt_id.clone();
    ocr_repo::insert_repair_event(&pool, &event).await.unwrap();

    let lease = make_lease_row(&attempt_id, run_id, stage_id, agent_exec_id);
    let lease_key = lease.lease_key.clone();
    ocr_repo::insert_lease(&pool, &lease).await.unwrap();

    let now = Utc::now().to_rfc3339();
    // EXTRA-003 fix: unsafe_continuation maps to blocked_missing_required_outputs (approved schema)
    // with initial_failure_subtype=unsafe_continuation for classification. blocked_unsafe_continuation
    // is not in the approved final_output_settlement enum.
    ocr_repo::settle_terminal_event_and_lease(
        &pool,
        &attempt_id,
        &lease_key,
        "failed",
        "failed",
        "inspect_repair_evidence",
        Some("blocked_missing_required_outputs"),
        "rejected_invalid",
        &now,
    )
    .await
    .expect("settle_terminal_event_and_lease should succeed");

    let event_after = ocr_repo::get_repair_event_by_repair_attempt_id(&pool, &attempt_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(event_after.status, "failed");
    assert_eq!(
        event_after.final_output_settlement.as_deref(),
        Some("blocked_missing_required_outputs")
    );

    let lease_after = ocr_repo::get_lease_by_key(&pool, &lease_key)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(lease_after.lease_state, LeaseState::Settled);
    assert_eq!(
        lease_after.settled_result.as_deref(),
        Some("rejected_invalid")
    );
}

#[tokio::test]
async fn proposal_079_reclamation_reason_enum_values() {
    // Verify that the approved reclamation reason values are accepted.
    let pool = setup_db().await;
    let (run_id, stage_id) = seed_run(&pool).await;
    let agent_exec_id = AgentExecutionId::new();

    let event = make_event_row(run_id, stage_id, agent_exec_id);
    let attempt_id = event.repair_attempt_id.clone();
    ocr_repo::insert_repair_event(&pool, &event).await.unwrap();

    let lease = make_lease_row(&attempt_id, run_id, stage_id, agent_exec_id);
    let lease_key = lease.lease_key.clone();
    ocr_repo::insert_lease(&pool, &lease).await.unwrap();

    let mut tx = pool.begin().await.unwrap();
    // The approved proposal vocabulary is ttl_expired_reserved / ttl_expired_prompt_sent.
    ocr_repo::settle_lease_tx(
        &mut tx,
        &lease_key,
        "unavailable",
        Some("ttl_expired_reserved"),
        &Utc::now().to_rfc3339(),
    )
    .await
    .expect("ttl_expired_reserved must be a valid reclamation_reason value (DEFECT-006)");
    tx.commit().await.unwrap();

    let lease_after = ocr_repo::get_lease_by_key(&pool, &lease_key)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        lease_after.reclamation_reason.as_deref(),
        Some("ttl_expired_reserved")
    );
}

#[tokio::test]
async fn proposal_079_unsafe_continuation_maps_to_approved_settlement() {
    // EXTRA-003 fix: blocked_unsafe_continuation is NOT in the approved final_output_settlement
    // enum (api-contract-r3-001). Unsafe continuation must settle as blocked_missing_required_outputs
    // with initial_failure_subtype=unsafe_continuation. This test verifies the approved mapping
    // is accepted and that the old (rejected) value would fail the CHECK constraint.
    let pool = setup_db().await;
    let (run_id, stage_id) = seed_run(&pool).await;
    let agent_exec_id = AgentExecutionId::new();

    let event = make_event_row(run_id, stage_id, agent_exec_id);
    let attempt_id = event.repair_attempt_id.clone();
    ocr_repo::insert_repair_event(&pool, &event).await.unwrap();

    let now = Utc::now().to_rfc3339();
    // Verify the approved settlement value for unsafe_continuation is accepted.
    ocr_repo::update_repair_event_status(
        &pool,
        &attempt_id,
        "failed",
        "failed",
        "inspect_repair_evidence",
        Some("blocked_missing_required_outputs"),
        &now,
    )
    .await
    .expect(
        "blocked_missing_required_outputs must be accepted for unsafe_continuation (EXTRA-003)",
    );

    let fetched = ocr_repo::get_repair_event_by_repair_attempt_id(&pool, &attempt_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        fetched.final_output_settlement.as_deref(),
        Some("blocked_missing_required_outputs"),
        "unsafe_continuation must settle as blocked_missing_required_outputs per approved schema"
    );
}

#[tokio::test]
async fn proposal_079_blocked_unsafe_continuation_rejected_by_schema() {
    // EXTRA-003 fix: blocked_unsafe_continuation is NOT an approved final_output_settlement value.
    // This regression test proves the DB CHECK constraint rejects it.
    let pool = setup_db().await;
    let (run_id, stage_id) = seed_run(&pool).await;
    let agent_exec_id = AgentExecutionId::new();

    let event = make_event_row(run_id, stage_id, agent_exec_id);
    let attempt_id = event.repair_attempt_id.clone();
    ocr_repo::insert_repair_event(&pool, &event).await.unwrap();

    let now = Utc::now().to_rfc3339();
    let result = ocr_repo::update_repair_event_status(
        &pool,
        &attempt_id,
        "failed",
        "failed",
        "inspect_repair_evidence",
        Some("blocked_unsafe_continuation"),
        &now,
    )
    .await;
    assert!(
        result.is_err(),
        "blocked_unsafe_continuation must be rejected by final_output_settlement CHECK constraint (EXTRA-003)"
    );
}
