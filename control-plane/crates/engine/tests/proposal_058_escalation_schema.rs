/// P058 Phase 0-1: Escalation schema, persistence, and domain type validation.
/// Verifies the migration creates all required tables with correct columns,
/// that ledger/event insert/read round-trips work, and that pause reason vocabulary
/// matches the catalog defined in the proposal.
use chrono::Utc;
use db::pool::create_pool;
use db::repos::escalation;
use domain::agent::AgentExecution;
use domain::escalation::{
    EscalationEvent, EscalationExecutionMetadata, EscalationLedger, EscalationPauseReason,
    EscalationTierKind,
};
use domain::ids::{AgentExecutionId, RunId};

async fn insert_minimal_run(pool: &sqlx::SqlitePool, run_id: RunId) {
    use chrono::Utc;
    use db::repos::{ideas, runs};
    use db::writer::{register_shared_writer, DbWriter};
    use domain::idea::{Idea, IdeaStatus};
    use domain::ids::IdeaId;
    use domain::run::{Run, RunStatus};
    use std::sync::Arc;
    // P075 DbWriter must be registered before calling repos that use execute_repository_write!.
    let writer = Arc::new(DbWriter::new(pool.clone()));
    register_shared_writer(pool, writer).await.unwrap();
    let idea_id = IdeaId::new();
    ideas::insert(
        pool,
        &Idea {
            id: idea_id,
            title: "P058 escalation".into(),
            body: "schema test".into(),
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
            workflow_id: "wf".into(),
            workflow_title: "Workflow".into(),
            workspace_root: "/tmp".into(),
            artifact_root: "/tmp".into(),
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
            review_routing_json: None,
            closeout_readiness_mode: None,
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn p058_escalation_ledger_insert_and_read_roundtrip() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    let now = Utc::now();
    let ledger = EscalationLedger {
        id: "ledger-test-001".into(),
        run_id,
        stage_id: "state_3_implementation".into(),
        agent_id: "code_writer".into(),
        policy_id: "code_writer_default_escalation".into(),
        policy_hash: "sha256:testpolicyhash".into(),
        status_raw: "active".into(),
        current_tier_id: Some("primary_retry".into()),
        current_tier_kind_raw: Some(EscalationTierKind::SameBackendRetry.to_string()),
        chain_attempt_index: 0,
        trigger_raw: None,
        pause_reason_raw: None,
        operator_action_hint: None,
        runbook_anchor: None,
        created_at: now,
        updated_at: now,
    };

    escalation::insert_ledger(&pool, &ledger).await.unwrap();

    let rows = escalation::find_ledgers_by_run(&pool, run_id)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "ledger-test-001");
    assert_eq!(rows[0].run_id, run_id);
    assert_eq!(rows[0].agent_id, "code_writer");
    assert_eq!(rows[0].policy_id, "code_writer_default_escalation");
    assert_eq!(rows[0].status_raw, "active");
    assert_eq!(rows[0].current_tier_id.as_deref(), Some("primary_retry"));
    assert_eq!(
        rows[0].current_tier_kind_raw.as_deref(),
        Some("same_backend_retry")
    );
}

#[tokio::test]
async fn p058_escalation_ledger_paused_chain_persists_pause_reason() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    let now = Utc::now();
    let pause_reason = EscalationPauseReason::EscalationChainExhausted.to_string();
    let ledger = EscalationLedger {
        id: "ledger-paused-001".into(),
        run_id,
        stage_id: "state_3".into(),
        agent_id: "code_writer".into(),
        policy_id: "policy-x".into(),
        policy_hash: "sha256:test".into(),
        status_raw: "exhausted".into(),
        current_tier_id: None,
        current_tier_kind_raw: None,
        chain_attempt_index: 6,
        trigger_raw: Some("repeated_same_blocker_digest".into()),
        pause_reason_raw: Some(pause_reason.clone()),
        operator_action_hint: Some("Extend the chain or accept terminal pause.".into()),
        runbook_anchor: Some("escalation/chain-exhausted".into()),
        created_at: now,
        updated_at: now,
    };

    escalation::insert_ledger(&pool, &ledger).await.unwrap();

    let row = escalation::find_ledger_by_id(&pool, "ledger-paused-001")
        .await
        .unwrap()
        .expect("ledger should be found");
    assert_eq!(row.status_raw, "exhausted");
    assert_eq!(row.pause_reason_raw.as_deref(), Some(pause_reason.as_str()));
    assert_eq!(row.chain_attempt_index, 6);
    assert_eq!(
        row.trigger_raw.as_deref(),
        Some("repeated_same_blocker_digest")
    );
}

#[tokio::test]
async fn p058_escalation_event_journal_insert_and_read() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    let now = Utc::now();
    let ledger = EscalationLedger {
        id: "ledger-events-001".into(),
        run_id,
        stage_id: "state_3".into(),
        agent_id: "code_writer".into(),
        policy_id: "policy-y".into(),
        policy_hash: "sha256:y".into(),
        status_raw: "active".into(),
        current_tier_id: Some("primary_retry".into()),
        current_tier_kind_raw: Some("same_backend_retry".into()),
        chain_attempt_index: 0,
        trigger_raw: None,
        pause_reason_raw: None,
        operator_action_hint: None,
        runbook_anchor: None,
        created_at: now,
        updated_at: now,
    };
    let mut tx = pool.begin().await.unwrap();
    escalation::insert_ledger_tx(&mut tx, &ledger)
        .await
        .unwrap();

    let event = EscalationEvent {
        id: "event-001".into(),
        escalation_ledger_id: "ledger-events-001".into(),
        event_kind_raw: "escalation.chain_exhausted".into(),
        tier_id: Some("primary_retry".into()),
        tier_kind_raw: Some("same_backend_retry".into()),
        trigger_raw: Some("contract_output_failure".into()),
        pause_reason_raw: Some("escalation_repeated_digest_no_progress".into()),
        payload_json: None,
        redaction_version: Some("redaction_v1".into()),
        created_at: now,
    };
    let before_exhausted =
        db::metrics::get_counter("chain_exhausted_total_by_terminal_tier_kind:same_backend_retry");
    let before_repeated = db::metrics::get_counter("escalation_repeated_digest_no_progress_total");
    escalation::insert_event_tx(&mut tx, &event).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        db::metrics::get_counter("chain_exhausted_total_by_terminal_tier_kind:same_backend_retry"),
        before_exhausted + 1
    );
    assert_eq!(
        db::metrics::get_counter("escalation_repeated_digest_no_progress_total"),
        before_repeated + 1
    );

    let events = escalation::find_events_by_ledger(&pool, "ledger-events-001")
        .await
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_kind_raw, "escalation.chain_exhausted");
    assert_eq!(events[0].tier_id.as_deref(), Some("primary_retry"));
    assert_eq!(
        events[0].trigger_raw.as_deref(),
        Some("contract_output_failure")
    );
    assert_eq!(
        events[0].redaction_version.as_deref(),
        Some("redaction_v1"),
        "redaction_version must round-trip through escalation_events"
    );
}

#[tokio::test]
async fn p058_escalation_event_rejects_malformed_payload_json() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    let now = Utc::now();
    let ledger = EscalationLedger {
        id: "ledger-json-bad-001".into(),
        run_id,
        stage_id: "state_3".into(),
        agent_id: "code_writer".into(),
        policy_id: "policy-j".into(),
        policy_hash: "sha256:j".into(),
        status_raw: "active".into(),
        current_tier_id: None,
        current_tier_kind_raw: None,
        chain_attempt_index: 0,
        trigger_raw: None,
        pause_reason_raw: None,
        operator_action_hint: None,
        runbook_anchor: None,
        created_at: now,
        updated_at: now,
    };
    let mut tx = pool.begin().await.unwrap();
    escalation::insert_ledger_tx(&mut tx, &ledger)
        .await
        .unwrap();

    let bad_event = EscalationEvent {
        id: "event-bad-json".into(),
        escalation_ledger_id: "ledger-json-bad-001".into(),
        event_kind_raw: "escalation.tier_selected".into(),
        tier_id: None,
        tier_kind_raw: None,
        trigger_raw: None,
        pause_reason_raw: None,
        payload_json: Some("{ not valid json ]]]".into()),
        redaction_version: None,
        created_at: now,
    };
    let result = escalation::insert_event_tx(&mut tx, &bad_event).await;
    assert!(
        result.is_err(),
        "insert_event_tx must reject malformed payload_json"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("payload_json"),
        "error must mention the field name; got: {err_msg}"
    );
}

#[tokio::test]
async fn p058_escalation_event_accepts_valid_payload_json() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    let now = Utc::now();
    let ledger = EscalationLedger {
        id: "ledger-json-good-001".into(),
        run_id,
        stage_id: "state_3".into(),
        agent_id: "code_writer".into(),
        policy_id: "policy-jg".into(),
        policy_hash: "sha256:jg".into(),
        status_raw: "active".into(),
        current_tier_id: None,
        current_tier_kind_raw: None,
        chain_attempt_index: 0,
        trigger_raw: None,
        pause_reason_raw: None,
        operator_action_hint: None,
        runbook_anchor: None,
        created_at: now,
        updated_at: now,
    };
    // Commit ledger first.
    escalation::insert_ledger(&pool, &ledger).await.unwrap();

    let good_event = EscalationEvent {
        id: "event-good-json".into(),
        escalation_ledger_id: "ledger-json-good-001".into(),
        event_kind_raw: "escalation.tier_advanced".into(),
        tier_id: Some("frontier_profile".into()),
        tier_kind_raw: Some("backend_profile".into()),
        trigger_raw: Some("contract_output_failure".into()),
        pause_reason_raw: None,
        payload_json: Some(r#"{"tier_id":"primary_retry","event_kind_raw":"escalation.tier_advanced","trigger_raw":"contract_output_failure"}"#.into()),
        redaction_version: Some("redaction_v1".into()),
        created_at: now,
    };
    let mut tx = pool.begin().await.unwrap();
    escalation::insert_event_tx(&mut tx, &good_event)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let events = escalation::find_events_by_ledger(&pool, "ledger-json-good-001")
        .await
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id, "event-good-json");
    assert_eq!(events[0].redaction_version.as_deref(), Some("redaction_v1"));
}

#[tokio::test]
async fn p058_escalation_execution_metadata_insert_and_read() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    let now = Utc::now();
    let ledger = EscalationLedger {
        id: "ledger-meta-001".into(),
        run_id,
        stage_id: "state_3".into(),
        agent_id: "code_writer".into(),
        policy_id: "policy-z".into(),
        policy_hash: "sha256:z".into(),
        status_raw: "active".into(),
        current_tier_id: None,
        current_tier_kind_raw: None,
        chain_attempt_index: 0,
        trigger_raw: None,
        pause_reason_raw: None,
        operator_action_hint: None,
        runbook_anchor: None,
        created_at: now,
        updated_at: now,
    };

    // Insert a minimal agent_execution to satisfy the FK.
    use db::repos::{agent_executions, stages};
    use domain::agent::AgentStatus;
    use domain::ids::StageExecutionId;
    use domain::stage::{StageExecution, StageStatus};
    let stage_id = StageExecutionId::new();
    let exec_id = AgentExecutionId::new();
    stages::insert(
        &pool,
        &StageExecution {
            id: stage_id,
            run_id,
            stage_id: "state_3".into(),
            label: "Implementation".into(),
            status: StageStatus::Running,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: now,
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
        &pool,
        &AgentExecution {
            id: exec_id,
            stage_execution_id: Some(stage_id),
            agent_id: "code_writer".into(),
            provider: "claude".into(),
            model: Some("sonnet".into()),
            status: AgentStatus::Running,
            started_at: now,
            completed_at: None,
            owner_execution_lineage_id: None,
            session_lineage_id: None,
            session_generation_id: None,
            rehydrated_from_checkpoint_artifact_id: None,
            invocation_owner_key: Some("owner".into()),
            session_reuse_scope: Some("same_agent_family_within_run".into()),
            session_family_id: Some("code_writer".into()),
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

    let mut tx = pool.begin().await.unwrap();
    escalation::insert_ledger_tx(&mut tx, &ledger)
        .await
        .unwrap();
    let meta = EscalationExecutionMetadata {
        agent_execution_id: exec_id,
        escalation_ledger_id: "ledger-meta-001".into(),
        tier_id: "primary_retry".into(),
        tier_kind_raw: EscalationTierKind::SameBackendRetry.to_string(),
        tier_attempt_index: 1,
        trigger_raw: Some("contract_output_failure".into()),
        digest_version: Some("escalation_blocker_digest_v1".into()),
        capacity_probe_counter: 0,
        created_at: now,
        updated_at: now,
        would_select_tier_id: None,
        would_select_trigger_raw: None,
        would_select_decision_json: None,
    };
    escalation::insert_execution_metadata_tx(&mut tx, &meta)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let rows = escalation::find_execution_metadata_by_ledger(&pool, "ledger-meta-001")
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].agent_execution_id, exec_id);
    assert_eq!(rows[0].tier_id, "primary_retry");
    assert_eq!(rows[0].tier_kind_raw, "same_backend_retry");
    assert_eq!(rows[0].tier_attempt_index, 1);
    assert_eq!(
        rows[0].digest_version.as_deref(),
        Some("escalation_blocker_digest_v1")
    );
    // Phase 1b: shadow columns are null when no runtime_facts row exists.
    assert!(rows[0].would_select_tier_id.is_none());
    assert!(rows[0].would_select_trigger_raw.is_none());
}

/// Phase 1b: find_execution_metadata_by_ledger reads shadow columns from
/// agent_execution_runtime_facts via LEFT JOIN when a row with shadow values exists.
#[tokio::test]
async fn p058_phase1b_shadow_readback_via_left_join() {
    use db::repos::agent_execution_runtime_facts;
    use db::repos::{agent_executions, escalation, stages};
    use domain::agent::{AgentExecution, AgentExecutionRuntimeFacts, AgentStatus};
    use domain::escalation::{EscalationExecutionMetadata, EscalationLedger, EscalationTierKind};
    use domain::ids::{AgentExecutionId, StageExecutionId};
    use domain::stage::{StageExecution, StageStatus};

    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;
    let now = Utc::now();

    let ledger = EscalationLedger {
        id: "ledger-shadow-1b-001".into(),
        run_id,
        stage_id: "state_impl".into(),
        agent_id: "code_writer".into(),
        policy_id: "p_shadow_test".into(),
        policy_hash: "sha256:shadow".into(),
        status_raw: "active".into(),
        current_tier_id: Some("primary_retry".into()),
        current_tier_kind_raw: Some("same_backend_retry".into()),
        chain_attempt_index: 0,
        trigger_raw: None,
        pause_reason_raw: None,
        operator_action_hint: None,
        runbook_anchor: None,
        created_at: now,
        updated_at: now,
    };
    escalation::insert_ledger(&pool, &ledger).await.unwrap();

    let stage_id = StageExecutionId::new();
    let exec_id = AgentExecutionId::new();
    stages::insert(
        &pool,
        &StageExecution {
            id: stage_id,
            run_id,
            stage_id: "state_impl".into(),
            label: "Impl".into(),
            status: StageStatus::Running,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: now,
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
        &pool,
        &AgentExecution {
            id: exec_id,
            stage_execution_id: Some(stage_id),
            agent_id: "code_writer".into(),
            provider: "claude".into(),
            model: None,
            status: AgentStatus::Running,
            started_at: now,
            completed_at: None,
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
            actual_toolchain_mapping_diagnostics_json: None,
            transcript_artifact_id: None,
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

    let mut tx = pool.begin().await.unwrap();
    escalation::insert_execution_metadata_tx(
        &mut tx,
        &EscalationExecutionMetadata {
            agent_execution_id: exec_id,
            escalation_ledger_id: "ledger-shadow-1b-001".into(),
            tier_id: "primary_retry".into(),
            tier_kind_raw: EscalationTierKind::SameBackendRetry.to_string(),
            tier_attempt_index: 0,
            trigger_raw: None,
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
    tx.commit().await.unwrap();

    // Insert a runtime_facts row with shadow columns populated.
    agent_execution_runtime_facts::upsert(
        &pool,
        &AgentExecutionRuntimeFacts::defaults_for(exec_id, now),
    )
    .await
    .unwrap();
    let mut shadow_tx = pool.begin().await.unwrap();
    escalation::update_shadow_escalation_columns_tx(
        &mut shadow_tx,
        &exec_id.to_string(),
        Some("frontier_profile"),
        Some("contract_output_failure"),
        None,
    )
    .await
    .unwrap();
    shadow_tx.commit().await.unwrap();

    // Verify the LEFT JOIN returns the shadow values.
    let rows = escalation::find_execution_metadata_by_ledger(&pool, "ledger-shadow-1b-001")
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].would_select_tier_id.as_deref(),
        Some("frontier_profile"),
        "Phase 1b: would_select_tier_id must be readable via LEFT JOIN"
    );
    assert_eq!(
        rows[0].would_select_trigger_raw.as_deref(),
        Some("contract_output_failure"),
        "Phase 1b: would_select_trigger_raw must be readable via LEFT JOIN"
    );
}

#[tokio::test]
async fn p058_escalation_tables_accept_unknown_future_raw_values() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    let now = Utc::now();
    let ledger = EscalationLedger {
        id: "ledger-future-001".into(),
        run_id,
        stage_id: "state_x".into(),
        agent_id: "future_agent_v99".into(),
        policy_id: "policy-future".into(),
        policy_hash: "sha256:future".into(),
        status_raw: "future_unknown_status".into(),
        current_tier_id: Some("future_tier".into()),
        current_tier_kind_raw: Some("future_tier_kind_v99".into()),
        chain_attempt_index: 99,
        trigger_raw: Some("future_unknown_trigger_v99".into()),
        pause_reason_raw: Some("future_unknown_pause_reason".into()),
        operator_action_hint: None,
        runbook_anchor: None,
        created_at: now,
        updated_at: now,
    };

    escalation::insert_ledger(&pool, &ledger).await.unwrap();

    let row = escalation::find_ledger_by_id(&pool, "ledger-future-001")
        .await
        .unwrap()
        .expect("ledger should be found");
    // Unknown raw values must round-trip unchanged.
    assert_eq!(row.status_raw, "future_unknown_status");
    assert_eq!(
        row.current_tier_kind_raw.as_deref(),
        Some("future_tier_kind_v99")
    );
    assert_eq!(
        row.trigger_raw.as_deref(),
        Some("future_unknown_trigger_v99")
    );
    assert_eq!(
        row.pause_reason_raw.as_deref(),
        Some("future_unknown_pause_reason")
    );
}

#[tokio::test]
async fn p058_escalation_event_rejects_missing_redaction_version() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    let now = Utc::now();
    let ledger = EscalationLedger {
        id: "ledger-no-redact-001".into(),
        run_id,
        stage_id: "state_3".into(),
        agent_id: "code_writer".into(),
        policy_id: "policy-nr".into(),
        policy_hash: "sha256:nr".into(),
        status_raw: "active".into(),
        current_tier_id: None,
        current_tier_kind_raw: None,
        chain_attempt_index: 0,
        trigger_raw: None,
        pause_reason_raw: None,
        operator_action_hint: None,
        runbook_anchor: None,
        created_at: now,
        updated_at: now,
    };
    let mut tx = pool.begin().await.unwrap();
    escalation::insert_ledger_tx(&mut tx, &ledger)
        .await
        .unwrap();

    let event_no_redaction = EscalationEvent {
        id: "event-no-redact".into(),
        escalation_ledger_id: "ledger-no-redact-001".into(),
        event_kind_raw: "escalation.tier_selected".into(),
        tier_id: Some("primary_retry".into()),
        tier_kind_raw: Some("same_backend_retry".into()),
        trigger_raw: Some("contract_output_failure".into()),
        pause_reason_raw: None,
        payload_json: Some(r#"{"tier_id":"primary_retry"}"#.into()),
        redaction_version: None,
        created_at: now,
    };
    let result = escalation::insert_event_tx(&mut tx, &event_no_redaction).await;
    assert!(
        result.is_err(),
        "insert_event_tx must reject missing redaction_version"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("redaction_version"),
        "error must mention redaction_version; got: {err_msg}"
    );
}

#[test]
fn p058_pause_reason_vocabulary_covers_all_13_catalog_entries() {
    use EscalationPauseReason::*;
    let reasons = [
        EscalationPolicyUnknownBackendProfile,
        EscalationPolicyAmbiguousAtCompile,
        EscalationPolicyUnsafeForSideEffectStage,
        EscalationPolicyDisabled,
        EscalationKillSwitchEngaged,
        EscalationChainExhausted,
        CapacityProbeFailed,
        ProviderSessionForceDetached,
        EscalationRecoveryInconsistent,
        EscalationRepeatedDigestNoProgress,
        EscalationDeadlineElapsed,
        HumanTierDeadlineElapsed,
        EscalationPolicyDrift,
    ];
    assert_eq!(
        reasons.len(),
        13,
        "proposal pause_reason_catalog has 13 entries"
    );
    for reason in &reasons {
        let s = reason.to_string();
        let parsed: EscalationPauseReason = s.parse().expect("pause reason must roundtrip");
        assert_eq!(reason, &parsed);
    }
}

#[test]
fn p058_tier_kind_vocabulary_covers_all_4_kinds() {
    use EscalationTierKind::*;
    let kinds = [SameBackendRetry, BackendProfile, LeadMediation, Pause];
    for kind in &kinds {
        let s = kind.to_string();
        let parsed: EscalationTierKind = s.parse().expect("tier kind must roundtrip");
        assert_eq!(kind, &parsed);
    }
}

// --- update_shadow_escalation_columns_tx tests ---

async fn insert_runtime_facts_row(pool: &sqlx::SqlitePool, exec_id: AgentExecutionId) {
    use chrono::Utc;
    use db::repos::agent_execution_runtime_facts;
    use domain::agent::AgentExecutionRuntimeFacts;
    let now = Utc::now();
    agent_execution_runtime_facts::upsert(
        pool,
        &AgentExecutionRuntimeFacts::defaults_for(exec_id, now),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn p058_shadow_update_rejects_malformed_decision_json() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    use chrono::Utc;
    use db::repos::{agent_executions, stages};
    use domain::agent::AgentStatus;
    use domain::ids::StageExecutionId;
    use domain::stage::{StageExecution, StageStatus};
    let now = Utc::now();
    let stage_id = StageExecutionId::new();
    let exec_id = AgentExecutionId::new();
    stages::insert(
        &pool,
        &StageExecution {
            id: stage_id,
            run_id,
            stage_id: "state_3".into(),
            label: "Impl".into(),
            status: StageStatus::Running,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: now,
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
        &pool,
        &AgentExecution {
            id: exec_id,
            stage_execution_id: Some(stage_id),
            agent_id: "code_writer".into(),
            provider: "claude".into(),
            model: Some("sonnet".into()),
            status: AgentStatus::Running,
            started_at: now,
            completed_at: None,
            owner_execution_lineage_id: None,
            session_lineage_id: None,
            session_generation_id: None,
            rehydrated_from_checkpoint_artifact_id: None,
            invocation_owner_key: Some("owner".into()),
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
    insert_runtime_facts_row(&pool, exec_id).await;

    let mut tx = pool.begin().await.unwrap();
    let result = escalation::update_shadow_escalation_columns_tx(
        &mut tx,
        &exec_id.to_string(),
        Some("primary_retry"),
        Some("contract_output_failure"),
        Some("{ not valid json ]]]"),
    )
    .await;
    assert!(
        result.is_err(),
        "must reject malformed would_select_decision_json"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("malformed JSON"),
        "error must say malformed JSON; got: {msg}"
    );
}

#[tokio::test]
async fn p058_shadow_update_fails_for_missing_runtime_facts_row() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    let nonexistent_id = AgentExecutionId::new();
    let mut tx = pool.begin().await.unwrap();
    let result = escalation::update_shadow_escalation_columns_tx(
        &mut tx,
        &nonexistent_id.to_string(),
        Some("primary_retry"),
        Some("contract_output_failure"),
        None,
    )
    .await;
    assert!(
        result.is_err(),
        "must fail when no runtime_facts row exists"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("no agent_execution_runtime_facts"),
        "error must name the missing row; got: {msg}"
    );
}

#[tokio::test]
async fn p058_escalation_event_rejects_unknown_redaction_version() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    let now = chrono::Utc::now();
    let ledger = EscalationLedger {
        id: "ledger-bad-redact-001".into(),
        run_id,
        stage_id: "state_3".into(),
        agent_id: "code_writer".into(),
        policy_id: "policy-br".into(),
        policy_hash: "sha256:br".into(),
        status_raw: "active".into(),
        current_tier_id: None,
        current_tier_kind_raw: None,
        chain_attempt_index: 0,
        trigger_raw: None,
        pause_reason_raw: None,
        operator_action_hint: None,
        runbook_anchor: None,
        created_at: now,
        updated_at: now,
    };
    let mut tx = pool.begin().await.unwrap();
    escalation::insert_ledger_tx(&mut tx, &ledger)
        .await
        .unwrap();

    let event_unknown_redact = EscalationEvent {
        id: "event-bad-redact".into(),
        escalation_ledger_id: "ledger-bad-redact-001".into(),
        event_kind_raw: "escalation.tier_selected".into(),
        tier_id: Some("primary_retry".into()),
        tier_kind_raw: Some("same_backend_retry".into()),
        trigger_raw: None,
        pause_reason_raw: None,
        payload_json: None,
        redaction_version: Some("arbitrary_unknown_stamp".into()),
        created_at: now,
    };
    let result = escalation::insert_event_tx(&mut tx, &event_unknown_redact).await;
    assert!(
        result.is_err(),
        "insert_event_tx must reject redaction_version not in the known allowlist"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("redaction_version") || err_msg.contains("allowlist"),
        "error must reference redaction_version or allowlist; got: {err_msg}"
    );
}

/// P058: escalation_* attribution columns on agent_executions must round-trip
/// through insert_tx and find_by_id — the migrated columns are not dead schema.
#[tokio::test]
async fn p058_agent_execution_escalation_columns_roundtrip() {
    use db::repos::{agent_executions, stages};
    use domain::agent::{AgentExecution, AgentStatus};
    use domain::ids::StageExecutionId;
    use domain::stage::{StageExecution, StageStatus};

    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    let now = Utc::now();
    let stage_id = StageExecutionId::new();
    let exec_id = AgentExecutionId::new();
    stages::insert(
        &pool,
        &StageExecution {
            id: stage_id,
            run_id,
            stage_id: "state_3".into(),
            label: "Implementation".into(),
            status: StageStatus::Running,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: now,
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

    let exec = AgentExecution {
        id: exec_id,
        stage_execution_id: Some(stage_id),
        agent_id: "code_writer".into(),
        provider: "claude".into(),
        model: Some("sonnet".into()),
        status: AgentStatus::Running,
        started_at: now,
        completed_at: None,
        owner_execution_lineage_id: None,
        session_lineage_id: None,
        session_generation_id: None,
        rehydrated_from_checkpoint_artifact_id: None,
        invocation_owner_key: Some("owner-key".into()),
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
        // P058 escalation attribution — populated to verify INSERT wires them correctly.
        escalation_policy_id: Some("code_writer_default_escalation".into()),
        escalation_policy_hash: Some("sha256:abc123".into()),
        escalation_tier_id: Some("primary_retry".into()),
        escalation_tier_kind_raw: Some("same_backend_retry".into()),
        escalation_trigger_raw: Some("contract_output_failure".into()),
        escalation_digest_version: Some("escalation_blocker_digest_v1".into()),
        escalation_ledger_id: Some("ledger-test-esc-roundtrip".into()),
    };

    agent_executions::insert(&pool, &exec).await.unwrap();

    let found = agent_executions::find_by_id(&pool, exec_id)
        .await
        .unwrap()
        .expect("agent_execution must be found after insert");

    assert_eq!(
        found.escalation_policy_id.as_deref(),
        Some("code_writer_default_escalation"),
        "escalation_policy_id must round-trip through INSERT"
    );
    assert_eq!(
        found.escalation_policy_hash.as_deref(),
        Some("sha256:abc123"),
        "escalation_policy_hash must round-trip"
    );
    assert_eq!(
        found.escalation_tier_id.as_deref(),
        Some("primary_retry"),
        "escalation_tier_id must round-trip"
    );
    assert_eq!(
        found.escalation_tier_kind_raw.as_deref(),
        Some("same_backend_retry"),
        "escalation_tier_kind_raw must round-trip"
    );
    assert_eq!(
        found.escalation_trigger_raw.as_deref(),
        Some("contract_output_failure"),
        "escalation_trigger_raw must round-trip"
    );
    assert_eq!(
        found.escalation_digest_version.as_deref(),
        Some("escalation_blocker_digest_v1"),
        "escalation_digest_version must round-trip"
    );
    assert_eq!(
        found.escalation_ledger_id.as_deref(),
        Some("ledger-test-esc-roundtrip"),
        "escalation_ledger_id must round-trip"
    );
}

#[tokio::test]
async fn p058_escalation_event_fk_rejects_orphan_insert() {
    // foreign_keys=ON is set by create_pool; inserting an event for a non-existent
    // ledger_id must fail with a FK violation, proving the constraint is enforced.
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let now = chrono::Utc::now();

    let orphan_event = domain::escalation::EscalationEvent {
        id: "event-orphan".into(),
        escalation_ledger_id: "nonexistent-ledger-id".into(),
        event_kind_raw: "escalation.tier_selected".into(),
        tier_id: None,
        tier_kind_raw: None,
        trigger_raw: None,
        pause_reason_raw: None,
        payload_json: None,
        redaction_version: Some("redaction_v1".into()),
        created_at: now,
    };
    let mut tx = pool.begin().await.unwrap();
    let result = escalation::insert_event_tx(&mut tx, &orphan_event).await;
    assert!(
        result.is_err(),
        "insert_event_tx must fail for orphan escalation_ledger_id (FK constraint)"
    );
}

/// SEC-004: update_ledger_tx must enforce the same byte caps as insert_ledger_tx.
#[tokio::test]
async fn p058_update_ledger_tx_rejects_oversized_status_raw() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    let now = Utc::now();
    let ledger = EscalationLedger {
        id: "ledger-update-cap-001".into(),
        run_id,
        stage_id: "state_3".into(),
        agent_id: "code_writer".into(),
        policy_id: "policy-cap".into(),
        policy_hash: "sha256:cap".into(),
        status_raw: "active".into(),
        current_tier_id: None,
        current_tier_kind_raw: None,
        chain_attempt_index: 0,
        trigger_raw: None,
        pause_reason_raw: None,
        operator_action_hint: None,
        runbook_anchor: None,
        created_at: now,
        updated_at: now,
    };
    escalation::insert_ledger(&pool, &ledger).await.unwrap();

    // status_raw is capped at FIELD_ENUM_RAW_MAX (256 bytes).
    let mut bad_ledger = ledger.clone();
    bad_ledger.status_raw = "x".repeat(300);
    let mut tx = pool.begin().await.unwrap();
    let result = escalation::update_ledger_tx(&mut tx, &bad_ledger).await;
    assert!(
        result.is_err(),
        "update_ledger_tx must reject oversized status_raw"
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("status_raw") && err.contains("exceeds maximum"),
        "error must name the field and mention the cap; got: {err}"
    );
}

/// SEC-004: update_ledger_tx must enforce byte caps on operator_action_hint and runbook_anchor.
#[tokio::test]
async fn p058_update_ledger_tx_rejects_oversized_hint_fields() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    let now = Utc::now();
    let ledger = EscalationLedger {
        id: "ledger-update-cap-002".into(),
        run_id,
        stage_id: "state_3".into(),
        agent_id: "code_writer".into(),
        policy_id: "policy-cap2".into(),
        policy_hash: "sha256:cap2".into(),
        status_raw: "paused".into(),
        current_tier_id: None,
        current_tier_kind_raw: None,
        chain_attempt_index: 0,
        trigger_raw: None,
        pause_reason_raw: None,
        operator_action_hint: None,
        runbook_anchor: None,
        created_at: now,
        updated_at: now,
    };
    escalation::insert_ledger(&pool, &ledger).await.unwrap();

    // operator_action_hint is capped at FIELD_HINT_ANCHOR_MAX (1024 bytes).
    let mut bad_ledger = ledger.clone();
    bad_ledger.operator_action_hint = Some("x".repeat(1100));
    let mut tx = pool.begin().await.unwrap();
    let result = escalation::update_ledger_tx(&mut tx, &bad_ledger).await;
    assert!(
        result.is_err(),
        "update_ledger_tx must reject oversized operator_action_hint"
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("operator_action_hint") && err.contains("exceeds maximum"),
        "error must name the field; got: {err}"
    );
}

/// BLOCK-6: The unique index on (run_id, stage_id, agent_id, policy_id) must prevent
/// two ledger rows for the same chain from being inserted.
#[tokio::test]
async fn p058_escalation_ledger_unique_chain_constraint_rejects_duplicate() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    let now = Utc::now();
    let ledger_a = EscalationLedger {
        id: "ledger-unique-001".into(),
        run_id,
        stage_id: "state_3".into(),
        agent_id: "code_writer".into(),
        policy_id: "policy-u".into(),
        policy_hash: "sha256:u1".into(),
        status_raw: "active".into(),
        current_tier_id: None,
        current_tier_kind_raw: None,
        chain_attempt_index: 0,
        trigger_raw: None,
        pause_reason_raw: None,
        operator_action_hint: None,
        runbook_anchor: None,
        created_at: now,
        updated_at: now,
    };
    escalation::insert_ledger(&pool, &ledger_a).await.unwrap();

    // Same run_id + stage_id + agent_id + policy_id, different ledger id — must be rejected.
    let ledger_b = EscalationLedger {
        id: "ledger-unique-002".into(),
        run_id,
        stage_id: "state_3".into(),
        agent_id: "code_writer".into(),
        policy_id: "policy-u".into(),
        policy_hash: "sha256:u2".into(),
        status_raw: "active".into(),
        current_tier_id: None,
        current_tier_kind_raw: None,
        chain_attempt_index: 0,
        trigger_raw: None,
        pause_reason_raw: None,
        operator_action_hint: None,
        runbook_anchor: None,
        created_at: now,
        updated_at: now,
    };
    let result = escalation::insert_ledger(&pool, &ledger_b).await;
    assert!(
        result.is_err(),
        "inserting a second ledger for the same (run_id, stage_id, agent_id, policy_id) must fail"
    );
}

/// SEC-003: insert_event_tx must enforce byte caps on event_kind_raw.
/// Oversized event_kind_raw strings must be rejected before they reach the database.
#[tokio::test]
async fn p058_insert_event_tx_rejects_oversized_event_kind_raw() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    let now = Utc::now();
    let ledger = EscalationLedger {
        id: "ledger-evt-cap-001".into(),
        run_id,
        stage_id: "state_3".into(),
        agent_id: "code_writer".into(),
        policy_id: "policy-evt-cap".into(),
        policy_hash: "sha256:cap".into(),
        status_raw: "active".into(),
        current_tier_id: None,
        current_tier_kind_raw: None,
        chain_attempt_index: 0,
        trigger_raw: None,
        pause_reason_raw: None,
        operator_action_hint: None,
        runbook_anchor: None,
        created_at: now,
        updated_at: now,
    };
    let mut tx = pool.begin().await.unwrap();
    escalation::insert_ledger_tx(&mut tx, &ledger)
        .await
        .unwrap();

    let bad_event = EscalationEvent {
        id: "event-evt-cap-001".into(),
        escalation_ledger_id: "ledger-evt-cap-001".into(),
        event_kind_raw: "x".repeat(300), // exceeds FIELD_ENUM_RAW_MAX (256)
        tier_id: None,
        tier_kind_raw: None,
        trigger_raw: None,
        pause_reason_raw: None,
        payload_json: None,
        redaction_version: Some("redaction_v1".into()),
        created_at: now,
    };
    let result = escalation::insert_event_tx(&mut tx, &bad_event).await;
    assert!(
        result.is_err(),
        "insert_event_tx must reject oversized event_kind_raw"
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("event_kind_raw") && err.contains("exceeds maximum"),
        "error must name the field and mention the cap; got: {err}"
    );
}

/// SEC-003: insert_event_tx must enforce byte caps on tier_id and trigger_raw.
#[tokio::test]
async fn p058_insert_event_tx_rejects_oversized_tier_and_trigger_fields() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    let now = Utc::now();
    let ledger = EscalationLedger {
        id: "ledger-evt-cap-002".into(),
        run_id,
        stage_id: "state_3".into(),
        agent_id: "code_writer".into(),
        policy_id: "policy-evt-cap2".into(),
        policy_hash: "sha256:cap2".into(),
        status_raw: "active".into(),
        current_tier_id: None,
        current_tier_kind_raw: None,
        chain_attempt_index: 0,
        trigger_raw: None,
        pause_reason_raw: None,
        operator_action_hint: None,
        runbook_anchor: None,
        created_at: now,
        updated_at: now,
    };
    let mut tx = pool.begin().await.unwrap();
    escalation::insert_ledger_tx(&mut tx, &ledger)
        .await
        .unwrap();

    // trigger_raw exceeds FIELD_ENUM_RAW_MAX (256)
    let bad_trigger_event = EscalationEvent {
        id: "event-evt-cap-002".into(),
        escalation_ledger_id: "ledger-evt-cap-002".into(),
        event_kind_raw: "escalation.tier_selected".into(),
        tier_id: None,
        tier_kind_raw: None,
        trigger_raw: Some("t".repeat(300)),
        pause_reason_raw: None,
        payload_json: None,
        redaction_version: Some("redaction_v1".into()),
        created_at: now,
    };
    let result = escalation::insert_event_tx(&mut tx, &bad_trigger_event).await;
    assert!(
        result.is_err(),
        "insert_event_tx must reject oversized trigger_raw"
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("trigger_raw") && err.contains("exceeds maximum"),
        "error must name trigger_raw and mention the cap; got: {err}"
    );
}

/// P058 SEC-003: validate_policies_for_ambiguous_bindings returns diagnostics when two
/// policies share the same (agent_id, backend_profile_id, stage_id) binding tuple.
#[test]
fn p058_compile_rejects_ambiguous_policy_bindings() {
    use workflow::escalation_policy::{
        validate_policies_for_ambiguous_bindings, AppliesToYaml, EscalationPolicyYaml,
        EscalationTierYaml, EscalationTriggerYaml,
    };

    let make_policy = |policy_id: &str, agent_id: &str| EscalationPolicyYaml {
        policy_id: policy_id.to_string(),
        schema_version: "escalation_policy_v1".into(),
        enabled_default: false,
        applies_to: AppliesToYaml {
            agent_id: Some(agent_id.into()),
            backend_profile_id: None,
            stage_id: None,
        },
        max_chain_attempts: 3,
        max_chain_wall_clock_seconds: 3600,
        triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
        tiers: vec![EscalationTierYaml {
            tier_id: "retry".into(),
            kind: "same_backend_retry".into(),
            backend_profile_id: None,
            max_attempts: Some(2),
        }],
    };

    let policies = vec![
        make_policy("policy_a", "code_writer"),
        make_policy("policy_b", "code_writer"),
    ];

    let empty_map = std::collections::HashMap::new();
    let diagnostics = validate_policies_for_ambiguous_bindings(&policies, &empty_map);
    assert!(
        !diagnostics.is_empty(),
        "two policies with the same agent_id binding must produce ambiguous diagnostics"
    );
    assert!(
        diagnostics
            .iter()
            .all(|d| d.pause_reason_code == "escalation_policy_ambiguous_at_compile"),
        "all diagnostics must use escalation_policy_ambiguous_at_compile"
    );
    let policy_ids: Vec<&str> = diagnostics.iter().map(|d| d.policy_id.as_str()).collect();
    assert!(
        policy_ids.contains(&"policy_a") && policy_ids.contains(&"policy_b"),
        "both conflicting policies must appear in diagnostics; got: {policy_ids:?}"
    );
}

/// P058 SEC-003: distinct agent_id bindings are NOT ambiguous.
#[test]
fn p058_compile_allows_distinct_policy_bindings() {
    use workflow::escalation_policy::{
        validate_policies_for_ambiguous_bindings, AppliesToYaml, EscalationPolicyYaml,
        EscalationTierYaml, EscalationTriggerYaml,
    };

    let make_policy = |policy_id: &str, agent_id: &str| EscalationPolicyYaml {
        policy_id: policy_id.to_string(),
        schema_version: "escalation_policy_v1".into(),
        enabled_default: false,
        applies_to: AppliesToYaml {
            agent_id: Some(agent_id.into()),
            backend_profile_id: None,
            stage_id: None,
        },
        max_chain_attempts: 3,
        max_chain_wall_clock_seconds: 3600,
        triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
        tiers: vec![EscalationTierYaml {
            tier_id: "retry".into(),
            kind: "same_backend_retry".into(),
            backend_profile_id: None,
            max_attempts: Some(2),
        }],
    };

    let policies = vec![
        make_policy("policy_a", "code_writer"),
        make_policy("policy_b", "reviewer"),
    ];

    let empty_map = std::collections::HashMap::new();
    let diagnostics = validate_policies_for_ambiguous_bindings(&policies, &empty_map);
    assert!(
        diagnostics.is_empty(),
        "distinct agent_id bindings must not be ambiguous; got: {diagnostics:?}"
    );
}

/// P058 cross-axis ambiguity: a policy bound by agent_id and another by backend_profile_id
/// are ambiguous when the agent uses that profile (equal-specificity binding_precedence rule).
#[test]
fn p058_compile_rejects_cross_axis_ambiguous_bindings() {
    use workflow::escalation_policy::{
        validate_policies_for_ambiguous_bindings, AppliesToYaml, EscalationPolicyYaml,
        EscalationTierYaml, EscalationTriggerYaml,
    };

    let tier = || EscalationTierYaml {
        tier_id: "retry".into(),
        kind: "same_backend_retry".into(),
        backend_profile_id: None,
        max_attempts: Some(2),
    };

    // Policy A binds to agent_id "code_writer" (no backend_profile_id, no stage_id).
    let policy_by_agent = EscalationPolicyYaml {
        policy_id: "by_agent".into(),
        schema_version: "escalation_policy_v1".into(),
        enabled_default: false,
        applies_to: AppliesToYaml {
            agent_id: Some("code_writer".into()),
            backend_profile_id: None,
            stage_id: None,
        },
        max_chain_attempts: 3,
        max_chain_wall_clock_seconds: 3600,
        triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
        tiers: vec![tier()],
    };

    // Policy B binds to backend_profile_id "claude_builder" (no agent_id, no stage_id).
    let policy_by_profile = EscalationPolicyYaml {
        policy_id: "by_profile".into(),
        schema_version: "escalation_policy_v1".into(),
        enabled_default: false,
        applies_to: AppliesToYaml {
            agent_id: None,
            backend_profile_id: Some("claude_builder".into()),
            stage_id: None,
        },
        max_chain_attempts: 3,
        max_chain_wall_clock_seconds: 3600,
        triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
        tiers: vec![tier()],
    };

    // code_writer uses claude_builder — cross-axis ambiguity.
    let mut agent_to_profile = std::collections::HashMap::new();
    agent_to_profile.insert("code_writer", "claude_builder");

    let policies = vec![policy_by_agent, policy_by_profile];
    let diagnostics = validate_policies_for_ambiguous_bindings(&policies, &agent_to_profile);
    assert!(
        !diagnostics.is_empty(),
        "cross-axis agent/profile binding must produce ambiguous diagnostics; got empty"
    );
    assert!(
        diagnostics
            .iter()
            .all(|d| d.pause_reason_code == "escalation_policy_ambiguous_at_compile"),
        "all diagnostics must use escalation_policy_ambiguous_at_compile"
    );
    let policy_ids: Vec<&str> = diagnostics.iter().map(|d| d.policy_id.as_str()).collect();
    assert!(
        policy_ids.contains(&"by_agent") && policy_ids.contains(&"by_profile"),
        "both conflicting policies must appear in diagnostics; got: {policy_ids:?}"
    );
}

/// P058 cross-axis: agent_id-only and backend_profile_id-only bindings for DIFFERENT profile are
/// NOT ambiguous (the agent uses a different profile, so only one policy can match).
#[test]
fn p058_compile_allows_cross_axis_non_overlapping_bindings() {
    use workflow::escalation_policy::{
        validate_policies_for_ambiguous_bindings, AppliesToYaml, EscalationPolicyYaml,
        EscalationTierYaml, EscalationTriggerYaml,
    };

    let tier = || EscalationTierYaml {
        tier_id: "retry".into(),
        kind: "same_backend_retry".into(),
        backend_profile_id: None,
        max_attempts: Some(2),
    };

    let policy_by_agent = EscalationPolicyYaml {
        policy_id: "by_agent".into(),
        schema_version: "escalation_policy_v1".into(),
        enabled_default: false,
        applies_to: AppliesToYaml {
            agent_id: Some("code_writer".into()),
            backend_profile_id: None,
            stage_id: None,
        },
        max_chain_attempts: 3,
        max_chain_wall_clock_seconds: 3600,
        triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
        tiers: vec![tier()],
    };

    // Policy targets a DIFFERENT profile than code_writer uses.
    let policy_by_different_profile = EscalationPolicyYaml {
        policy_id: "by_different_profile".into(),
        schema_version: "escalation_policy_v1".into(),
        enabled_default: false,
        applies_to: AppliesToYaml {
            agent_id: None,
            backend_profile_id: Some("some_other_profile".into()),
            stage_id: None,
        },
        max_chain_attempts: 3,
        max_chain_wall_clock_seconds: 3600,
        triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
        tiers: vec![tier()],
    };

    // code_writer uses claude_builder; some_other_profile is for a different agent.
    let mut agent_to_profile = std::collections::HashMap::new();
    agent_to_profile.insert("code_writer", "claude_builder");

    let policies = vec![policy_by_agent, policy_by_different_profile];
    let diagnostics = validate_policies_for_ambiguous_bindings(&policies, &agent_to_profile);
    assert!(
        diagnostics.is_empty(),
        "non-overlapping cross-axis bindings must not be ambiguous; got: {diagnostics:?}"
    );
}

/// P058 SEC-M2: stage-scoped cross-axis ambiguity — {stage_id, agent_id} vs
/// {stage_id, backend_profile_id} with matching agent+profile in the same stage is ambiguous.
#[test]
fn p058_compile_rejects_stage_scoped_cross_axis_ambiguous_bindings() {
    use workflow::escalation_policy::{
        validate_policies_for_ambiguous_bindings, AppliesToYaml, EscalationPolicyYaml,
        EscalationTierYaml, EscalationTriggerYaml,
    };

    let tier = || EscalationTierYaml {
        tier_id: "retry".into(),
        kind: "same_backend_retry".into(),
        backend_profile_id: None,
        max_attempts: Some(2),
    };

    // Policy A: {stage_id=state_3, agent_id=code_writer}
    let policy_stage_agent = EscalationPolicyYaml {
        policy_id: "stage_agent_policy".into(),
        schema_version: "escalation_policy_v1".into(),
        enabled_default: false,
        applies_to: AppliesToYaml {
            stage_id: Some("state_3".into()),
            agent_id: Some("code_writer".into()),
            backend_profile_id: None,
        },
        max_chain_attempts: 3,
        max_chain_wall_clock_seconds: 3600,
        triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
        tiers: vec![tier()],
    };

    // Policy B: {stage_id=state_3, backend_profile_id=claude_builder}
    let policy_stage_profile = EscalationPolicyYaml {
        policy_id: "stage_profile_policy".into(),
        schema_version: "escalation_policy_v1".into(),
        enabled_default: false,
        applies_to: AppliesToYaml {
            stage_id: Some("state_3".into()),
            agent_id: None,
            backend_profile_id: Some("claude_builder".into()),
        },
        max_chain_attempts: 3,
        max_chain_wall_clock_seconds: 3600,
        triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
        tiers: vec![tier()],
    };

    // code_writer uses claude_builder — both policies match agent in state_3.
    let mut agent_to_profile = std::collections::HashMap::new();
    agent_to_profile.insert("code_writer", "claude_builder");

    let policies = vec![policy_stage_agent, policy_stage_profile];
    let diagnostics = validate_policies_for_ambiguous_bindings(&policies, &agent_to_profile);
    assert!(
        !diagnostics.is_empty(),
        "stage-scoped cross-axis agent/profile binding must produce ambiguous diagnostics"
    );
    assert!(
        diagnostics
            .iter()
            .all(|d| d.pause_reason_code == "escalation_policy_ambiguous_at_compile"),
        "all diagnostics must use escalation_policy_ambiguous_at_compile"
    );
    let policy_ids: Vec<&str> = diagnostics.iter().map(|d| d.policy_id.as_str()).collect();
    assert!(
        policy_ids.contains(&"stage_agent_policy") && policy_ids.contains(&"stage_profile_policy"),
        "both conflicting policies must appear in diagnostics; got: {policy_ids:?}"
    );
}

/// P058 SEC-M2: stage-scoped {stage_id, agent_id} and {stage_id, backend_profile_id} for DIFFERENT
/// stages are NOT ambiguous.
#[test]
fn p058_compile_allows_stage_scoped_bindings_for_different_stages() {
    use workflow::escalation_policy::{
        validate_policies_for_ambiguous_bindings, AppliesToYaml, EscalationPolicyYaml,
        EscalationTierYaml, EscalationTriggerYaml,
    };

    let tier = || EscalationTierYaml {
        tier_id: "retry".into(),
        kind: "same_backend_retry".into(),
        backend_profile_id: None,
        max_attempts: Some(2),
    };

    let policy_stage_a = EscalationPolicyYaml {
        policy_id: "stage_a_agent".into(),
        schema_version: "escalation_policy_v1".into(),
        enabled_default: false,
        applies_to: AppliesToYaml {
            stage_id: Some("state_3".into()),
            agent_id: Some("code_writer".into()),
            backend_profile_id: None,
        },
        max_chain_attempts: 3,
        max_chain_wall_clock_seconds: 3600,
        triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
        tiers: vec![tier()],
    };

    // Different stage — no overlap.
    let policy_stage_b = EscalationPolicyYaml {
        policy_id: "stage_b_profile".into(),
        schema_version: "escalation_policy_v1".into(),
        enabled_default: false,
        applies_to: AppliesToYaml {
            stage_id: Some("state_5".into()),
            agent_id: None,
            backend_profile_id: Some("claude_builder".into()),
        },
        max_chain_attempts: 3,
        max_chain_wall_clock_seconds: 3600,
        triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
        tiers: vec![tier()],
    };

    let mut agent_to_profile = std::collections::HashMap::new();
    agent_to_profile.insert("code_writer", "claude_builder");

    let policies = vec![policy_stage_a, policy_stage_b];
    let diagnostics = validate_policies_for_ambiguous_bindings(&policies, &agent_to_profile);
    assert!(
        diagnostics.is_empty(),
        "different-stage bindings must not be ambiguous; got: {diagnostics:?}"
    );
}

/// P058 SEC-001: validate_policies_for_unsafe_stage_bindings rejects policies targeting
/// manual_gate stages via explicit stage_id binding.
#[test]
fn p058_compile_rejects_unsafe_side_effect_stage_binding() {
    use std::collections::HashSet;
    use workflow::escalation_policy::{
        validate_policies_for_unsafe_stage_bindings, AppliesToYaml, EscalationPolicyYaml,
        EscalationTierYaml, EscalationTriggerYaml,
    };

    let policy = EscalationPolicyYaml {
        policy_id: "unsafe_release_policy".into(),
        schema_version: "escalation_policy_v1".into(),
        enabled_default: false,
        applies_to: AppliesToYaml {
            agent_id: None,
            backend_profile_id: None,
            stage_id: Some("state_11_manual_release".into()),
        },
        max_chain_attempts: 3,
        max_chain_wall_clock_seconds: 3600,
        triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
        tiers: vec![EscalationTierYaml {
            tier_id: "retry".into(),
            kind: "same_backend_retry".into(),
            backend_profile_id: None,
            max_attempts: Some(2),
        }],
    };

    let mut unsafe_stage_ids = HashSet::new();
    unsafe_stage_ids.insert("state_11_manual_release".into());

    let diagnostics = validate_policies_for_unsafe_stage_bindings(
        &[policy],
        &unsafe_stage_ids,
        &HashSet::new(),
        &HashSet::new(),
    );
    assert!(
        !diagnostics.is_empty(),
        "policy targeting a manual_gate stage must produce unsafe-stage diagnostic"
    );
    assert_eq!(
        diagnostics[0].pause_reason_code,
        "escalation_policy_unsafe_for_side_effect_stage"
    );
    assert_eq!(diagnostics[0].policy_id, "unsafe_release_policy");
}

/// P058 SEC-001: policies NOT targeting unsafe stages or unsafe agents pass the check.
#[test]
fn p058_compile_allows_safe_stage_binding() {
    use std::collections::HashSet;
    use workflow::escalation_policy::{
        validate_policies_for_unsafe_stage_bindings, AppliesToYaml, EscalationPolicyYaml,
        EscalationTierYaml, EscalationTriggerYaml,
    };

    let policy = EscalationPolicyYaml {
        policy_id: "safe_impl_policy".into(),
        schema_version: "escalation_policy_v1".into(),
        enabled_default: false,
        applies_to: AppliesToYaml {
            agent_id: Some("code_writer".into()),
            backend_profile_id: None,
            stage_id: None,
        },
        max_chain_attempts: 3,
        max_chain_wall_clock_seconds: 3600,
        triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
        tiers: vec![EscalationTierYaml {
            tier_id: "retry".into(),
            kind: "same_backend_retry".into(),
            backend_profile_id: None,
            max_attempts: Some(2),
        }],
    };

    let mut unsafe_stage_ids = HashSet::new();
    unsafe_stage_ids.insert("state_11_manual_release".into());
    // code_writer is NOT in unsafe_agent_ids — it should pass.
    let mut unsafe_agent_ids = HashSet::new();
    unsafe_agent_ids.insert("release_agent".into());

    let diagnostics = validate_policies_for_unsafe_stage_bindings(
        &[policy],
        &unsafe_stage_ids,
        &unsafe_agent_ids,
        &HashSet::new(),
    );
    assert!(
        diagnostics.is_empty(),
        "policy targeting a safe agent must pass unsafe-stage check; got: {diagnostics:?}"
    );
}

/// SEC-001: policies binding via agent_id to an agent that runs in an unsafe stage must fail.
#[test]
fn p058_compile_rejects_agent_id_binding_to_unsafe_stage_agent() {
    use std::collections::HashSet;
    use workflow::escalation_policy::{
        validate_policies_for_unsafe_stage_bindings, AppliesToYaml, EscalationPolicyYaml,
        EscalationTierYaml, EscalationTriggerYaml,
    };

    let policy = EscalationPolicyYaml {
        policy_id: "agent_unsafe_policy".into(),
        schema_version: "escalation_policy_v1".into(),
        enabled_default: false,
        applies_to: AppliesToYaml {
            agent_id: Some("release_agent".into()),
            backend_profile_id: None,
            stage_id: None,
        },
        max_chain_attempts: 2,
        max_chain_wall_clock_seconds: 3600,
        triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
        tiers: vec![EscalationTierYaml {
            tier_id: "retry".into(),
            kind: "same_backend_retry".into(),
            backend_profile_id: None,
            max_attempts: Some(1),
        }],
    };

    let mut unsafe_agent_ids = HashSet::new();
    unsafe_agent_ids.insert("release_agent".into());

    let diagnostics = validate_policies_for_unsafe_stage_bindings(
        &[policy],
        &HashSet::new(),
        &unsafe_agent_ids,
        &HashSet::new(),
    );
    assert!(
        !diagnostics.is_empty(),
        "policy targeting an agent that runs in an unsafe stage must produce a diagnostic"
    );
    assert_eq!(
        diagnostics[0].pause_reason_code,
        "escalation_policy_unsafe_for_side_effect_stage"
    );
    assert!(
        diagnostics[0].detail.contains("release_agent"),
        "diagnostic detail must mention the agent id; got: {}",
        diagnostics[0].detail
    );
}

/// SEC-001: policies binding via backend_profile_id used by an unsafe-stage agent must fail.
#[test]
fn p058_compile_rejects_backend_profile_binding_to_unsafe_stage_profile() {
    use std::collections::HashSet;
    use workflow::escalation_policy::{
        validate_policies_for_unsafe_stage_bindings, AppliesToYaml, EscalationPolicyYaml,
        EscalationTierYaml, EscalationTriggerYaml,
    };

    let policy = EscalationPolicyYaml {
        policy_id: "profile_unsafe_policy".into(),
        schema_version: "escalation_policy_v1".into(),
        enabled_default: false,
        applies_to: AppliesToYaml {
            agent_id: None,
            backend_profile_id: Some("release_backend_profile".into()),
            stage_id: None,
        },
        max_chain_attempts: 2,
        max_chain_wall_clock_seconds: 3600,
        triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
        tiers: vec![EscalationTierYaml {
            tier_id: "retry".into(),
            kind: "same_backend_retry".into(),
            backend_profile_id: None,
            max_attempts: Some(1),
        }],
    };

    let mut unsafe_backend_profile_ids = HashSet::new();
    unsafe_backend_profile_ids.insert("release_backend_profile".into());

    let diagnostics = validate_policies_for_unsafe_stage_bindings(
        &[policy],
        &HashSet::new(),
        &HashSet::new(),
        &unsafe_backend_profile_ids,
    );
    assert!(
        !diagnostics.is_empty(),
        "policy targeting a backend_profile used in an unsafe stage must produce a diagnostic"
    );
    assert_eq!(
        diagnostics[0].pause_reason_code,
        "escalation_policy_unsafe_for_side_effect_stage"
    );
    assert!(
        diagnostics[0].detail.contains("release_backend_profile"),
        "diagnostic detail must mention the profile id; got: {}",
        diagnostics[0].detail
    );
}

/// SEC-002: validate_policy_structure is called on catalog-path policies before hash/freeze.
/// This test proves that structural defects (wrong schema_version, empty tiers, empty triggers,
/// empty applies_to, unknown tier kind, missing tier requirements) are caught on the catalog path,
/// not just the YAML-string parse path.
#[test]
fn p058_validate_policy_structure_catches_structural_defects() {
    use workflow::escalation_policy::{
        validate_policy_structure, AppliesToYaml, EscalationPolicyYaml, EscalationTierYaml,
        EscalationTriggerYaml,
    };

    let base = EscalationPolicyYaml {
        policy_id: "base_policy".into(),
        schema_version: "escalation_policy_v1".into(),
        enabled_default: false,
        applies_to: AppliesToYaml {
            agent_id: Some("code_writer".into()),
            backend_profile_id: None,
            stage_id: None,
        },
        max_chain_attempts: 3,
        max_chain_wall_clock_seconds: 3600,
        triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
        tiers: vec![EscalationTierYaml {
            tier_id: "t1".into(),
            kind: "same_backend_retry".into(),
            backend_profile_id: None,
            max_attempts: Some(2),
        }],
    };

    // Wrong schema_version.
    let mut p = base.clone();
    p.schema_version = "wrong_version".into();
    let err = validate_policy_structure(&p).unwrap_err();
    assert!(err.to_string().contains("schema_version"), "got: {err}");

    // Empty tiers.
    let mut p = base.clone();
    p.tiers = vec![];
    let err = validate_policy_structure(&p).unwrap_err();
    assert!(err.to_string().contains("tiers"), "got: {err}");

    // Empty triggers.
    let mut p = base.clone();
    p.triggers = vec![];
    let err = validate_policy_structure(&p).unwrap_err();
    assert!(err.to_string().contains("triggers"), "got: {err}");

    // Empty applies_to (no selector set).
    let mut p = base.clone();
    p.applies_to = AppliesToYaml {
        agent_id: None,
        backend_profile_id: None,
        stage_id: None,
    };
    let err = validate_policy_structure(&p).unwrap_err();
    assert!(err.to_string().contains("applies_to"), "got: {err}");

    // Unknown tier kind.
    let mut p = base.clone();
    p.tiers = vec![EscalationTierYaml {
        tier_id: "bad_tier".into(),
        kind: "totally_unknown_kind_v99".into(),
        backend_profile_id: None,
        max_attempts: Some(1),
    }];
    let err = validate_policy_structure(&p).unwrap_err();
    assert!(err.to_string().contains("unknown kind"), "got: {err}");

    // backend_profile tier missing backend_profile_id.
    let mut p = base.clone();
    p.tiers = vec![EscalationTierYaml {
        tier_id: "bp_tier".into(),
        kind: "backend_profile".into(),
        backend_profile_id: None,
        max_attempts: Some(1),
    }];
    let err = validate_policy_structure(&p).unwrap_err();
    assert!(err.to_string().contains("backend_profile_id"), "got: {err}");

    // same_backend_retry tier missing max_attempts.
    let mut p = base.clone();
    p.tiers = vec![EscalationTierYaml {
        tier_id: "retry_tier".into(),
        kind: "same_backend_retry".into(),
        backend_profile_id: None,
        max_attempts: None,
    }];
    let err = validate_policy_structure(&p).unwrap_err();
    assert!(err.to_string().contains("max_attempts"), "got: {err}");

    // Valid policy must pass.
    validate_policy_structure(&base)
        .expect("valid base policy must pass validate_policy_structure");
}

/// P058 idempotency: execution metadata table must reject duplicate (ledger, tier, attempt) rows.
/// This enforces the proposal idempotency key at the DB layer.
#[tokio::test]
async fn p058_execution_metadata_idempotency_key_rejects_duplicate_attempt() {
    use db::repos::{agent_executions, escalation, stages};
    use domain::agent::{AgentExecution, AgentStatus};
    use domain::ids::StageExecutionId;
    use domain::stage::{StageExecution, StageStatus};

    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    let now = Utc::now();
    let stage_id = StageExecutionId::new();

    stages::insert(
        &pool,
        &StageExecution {
            id: stage_id,
            run_id,
            stage_id: "state_3".into(),
            label: "Implementation".into(),
            status: StageStatus::Running,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: now,
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

    let ledger = EscalationLedger {
        id: "ledger-idem-001".into(),
        run_id,
        stage_id: "state_3".into(),
        agent_id: "code_writer".into(),
        policy_id: "policy-idem".into(),
        policy_hash: "sha256:idem".into(),
        status_raw: "active".into(),
        current_tier_id: None,
        current_tier_kind_raw: None,
        chain_attempt_index: 0,
        trigger_raw: None,
        pause_reason_raw: None,
        operator_action_hint: None,
        runbook_anchor: None,
        created_at: now,
        updated_at: now,
    };
    escalation::insert_ledger(&pool, &ledger).await.unwrap();

    // Insert two agent_executions so we have valid FKs for two metadata rows.
    let exec_id_1 = AgentExecutionId::new();
    let exec_id_2 = AgentExecutionId::new();
    for exec_id in [exec_id_1, exec_id_2] {
        agent_executions::insert(
            &pool,
            &AgentExecution {
                id: exec_id,
                stage_execution_id: Some(stage_id),
                agent_id: "code_writer".into(),
                provider: "claude".into(),
                model: Some("sonnet".into()),
                status: AgentStatus::Running,
                started_at: now,
                completed_at: None,
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
    }

    // Insert first metadata row (tier_id=primary_retry, tier_attempt_index=1).
    let mut tx = pool.begin().await.unwrap();
    let meta_1 = EscalationExecutionMetadata {
        agent_execution_id: exec_id_1,
        escalation_ledger_id: "ledger-idem-001".into(),
        tier_id: "primary_retry".into(),
        tier_kind_raw: EscalationTierKind::SameBackendRetry.to_string(),
        tier_attempt_index: 1,
        trigger_raw: Some("contract_output_failure".into()),
        digest_version: None,
        capacity_probe_counter: 0,
        created_at: now,
        updated_at: now,
        would_select_tier_id: None,
        would_select_trigger_raw: None,
        would_select_decision_json: None,
    };
    escalation::insert_execution_metadata_tx(&mut tx, &meta_1)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    // Insert a second metadata row with the SAME (ledger_id, tier_id, tier_attempt_index) — must fail.
    let mut tx2 = pool.begin().await.unwrap();
    let meta_2 = EscalationExecutionMetadata {
        agent_execution_id: exec_id_2,
        escalation_ledger_id: "ledger-idem-001".into(),
        tier_id: "primary_retry".into(),
        tier_kind_raw: EscalationTierKind::SameBackendRetry.to_string(),
        tier_attempt_index: 1, // same attempt index — must be rejected
        trigger_raw: None,
        digest_version: None,
        capacity_probe_counter: 0,
        created_at: now,
        updated_at: now,
        would_select_tier_id: None,
        would_select_trigger_raw: None,
        would_select_decision_json: None,
    };
    let result = escalation::insert_execution_metadata_tx(&mut tx2, &meta_2).await;
    assert!(
        result.is_err(),
        "duplicate (ledger_id, tier_id, tier_attempt_index) must fail the idempotency constraint"
    );
}

/// BLOCK-6: A different policy_id on the same run+stage+agent is a distinct chain and must succeed.
#[tokio::test]
async fn p058_escalation_ledger_unique_chain_allows_different_policy() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    let now = Utc::now();
    for (ledger_id, policy_id) in [
        ("ledger-diff-policy-001", "policy-a"),
        ("ledger-diff-policy-002", "policy-b"),
    ] {
        let ledger = EscalationLedger {
            id: ledger_id.into(),
            run_id,
            stage_id: "state_3".into(),
            agent_id: "code_writer".into(),
            policy_id: policy_id.into(),
            policy_hash: "sha256:hash".into(),
            status_raw: "active".into(),
            current_tier_id: None,
            current_tier_kind_raw: None,
            chain_attempt_index: 0,
            trigger_raw: None,
            pause_reason_raw: None,
            operator_action_hint: None,
            runbook_anchor: None,
            created_at: now,
            updated_at: now,
        };
        escalation::insert_ledger(&pool, &ledger)
            .await
            .expect("different policy_id on same stage+agent must be a separate chain");
    }
}

// ── SEC-002: structural validation on catalog compile path ─────────────────────

/// SEC-002: validate_policy_structure must reject a policy with wrong schema_version when
/// called on the catalog path (not just the parse_policy YAML path).
#[test]
fn p058_validate_policy_structure_rejects_wrong_schema_version() {
    use workflow::escalation_policy::{
        validate_policy_structure, AppliesToYaml, EscalationPolicyYaml, EscalationTierYaml,
        EscalationTriggerYaml,
    };

    let bad_policy = EscalationPolicyYaml {
        policy_id: "bad_schema_version".into(),
        schema_version: "escalation_policy_v99_future".into(), // wrong
        enabled_default: false,
        applies_to: AppliesToYaml {
            agent_id: Some("code_writer".into()),
            backend_profile_id: None,
            stage_id: None,
        },
        max_chain_attempts: 2,
        max_chain_wall_clock_seconds: 3600,
        triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
        tiers: vec![EscalationTierYaml {
            tier_id: "retry".into(),
            kind: "same_backend_retry".into(),
            backend_profile_id: None,
            max_attempts: Some(1),
        }],
    };

    let result = validate_policy_structure(&bad_policy);
    assert!(
        result.is_err(),
        "validate_policy_structure must reject wrong schema_version"
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("schema_version"),
        "error must mention schema_version; got: {err}"
    );
}

/// SEC-002: validate_policy_structure must reject a policy with empty tiers.
#[test]
fn p058_validate_policy_structure_rejects_empty_tiers() {
    use workflow::escalation_policy::{
        validate_policy_structure, AppliesToYaml, EscalationPolicyYaml, EscalationTriggerYaml,
    };

    let bad_policy = EscalationPolicyYaml {
        policy_id: "empty_tiers".into(),
        schema_version: "escalation_policy_v1".into(),
        enabled_default: false,
        applies_to: AppliesToYaml {
            agent_id: Some("code_writer".into()),
            backend_profile_id: None,
            stage_id: None,
        },
        max_chain_attempts: 2,
        max_chain_wall_clock_seconds: 3600,
        triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
        tiers: vec![], // empty — must be rejected
    };

    let result = validate_policy_structure(&bad_policy);
    assert!(
        result.is_err(),
        "validate_policy_structure must reject empty tiers"
    );
    assert!(result.unwrap_err().to_string().contains("tiers"));
}

/// SEC-002: validate_policy_structure must reject a policy with empty applies_to.
#[test]
fn p058_validate_policy_structure_rejects_empty_applies_to() {
    use workflow::escalation_policy::{
        validate_policy_structure, AppliesToYaml, EscalationPolicyYaml, EscalationTierYaml,
        EscalationTriggerYaml,
    };

    let bad_policy = EscalationPolicyYaml {
        policy_id: "empty_applies_to".into(),
        schema_version: "escalation_policy_v1".into(),
        enabled_default: false,
        applies_to: AppliesToYaml {
            agent_id: None,
            backend_profile_id: None,
            stage_id: None, // all None — must be rejected
        },
        max_chain_attempts: 2,
        max_chain_wall_clock_seconds: 3600,
        triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
        tiers: vec![EscalationTierYaml {
            tier_id: "retry".into(),
            kind: "same_backend_retry".into(),
            backend_profile_id: None,
            max_attempts: Some(1),
        }],
    };

    let result = validate_policy_structure(&bad_policy);
    assert!(
        result.is_err(),
        "validate_policy_structure must reject empty applies_to"
    );
    assert!(result.unwrap_err().to_string().contains("applies_to"));
}

// ── SEC-003: is_safe_ref_value bypass shapes ───────────────────────────────────

/// SEC-003: insert_event_tx must reject redacted_evidence_ref values that look like
/// credential strings after an approved prefix (e.g. sha256:sk-...).
#[tokio::test]
async fn p058_sec003_event_rejects_credential_bypass_in_ref_field() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    let now = chrono::Utc::now();
    let ledger = EscalationLedger {
        id: "ledger-sec003-cred".into(),
        run_id,
        stage_id: "state_3".into(),
        agent_id: "code_writer".into(),
        policy_id: "policy-sec003".into(),
        policy_hash: "sha256:abc".into(),
        status_raw: "active".into(),
        current_tier_id: None,
        current_tier_kind_raw: None,
        chain_attempt_index: 0,
        trigger_raw: None,
        pause_reason_raw: None,
        operator_action_hint: None,
        runbook_anchor: None,
        created_at: now,
        updated_at: now,
    };
    let mut tx = pool.begin().await.unwrap();
    escalation::insert_ledger_tx(&mut tx, &ledger)
        .await
        .unwrap();

    // sha256:sk-... looks like a credential prefixed with an approved algorithm name.
    let bypass_payload = r#"{"redacted_evidence_ref":"sha256:sk-abcdefghijklmnopqrstuvwxyz01234"}"#;
    let bad_event = EscalationEvent {
        id: "event-sec003-cred".into(),
        escalation_ledger_id: "ledger-sec003-cred".into(),
        event_kind_raw: "escalation.tier_selected".into(),
        tier_id: None,
        tier_kind_raw: None,
        trigger_raw: None,
        pause_reason_raw: None,
        payload_json: Some(bypass_payload.into()),
        redaction_version: Some("redaction_v1".into()),
        created_at: now,
    };
    let result = escalation::insert_event_tx(&mut tx, &bad_event).await;
    assert!(
        result.is_err(),
        "insert_event_tx must reject credential-shaped bypass in redacted_evidence_ref; \
         sha256:sk-... must not pass validation"
    );
}

/// SEC-003: insert_event_tx must reject redacted_evidence_ref values containing URL schemes
/// after an approved prefix (e.g. sha256:https://...).
#[tokio::test]
async fn p058_sec003_event_rejects_url_bypass_in_ref_field() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    let now = chrono::Utc::now();
    let ledger = EscalationLedger {
        id: "ledger-sec003-url".into(),
        run_id,
        stage_id: "state_3".into(),
        agent_id: "code_writer".into(),
        policy_id: "policy-sec003-url".into(),
        policy_hash: "sha256:url".into(),
        status_raw: "active".into(),
        current_tier_id: None,
        current_tier_kind_raw: None,
        chain_attempt_index: 0,
        trigger_raw: None,
        pause_reason_raw: None,
        operator_action_hint: None,
        runbook_anchor: None,
        created_at: now,
        updated_at: now,
    };
    let mut tx = pool.begin().await.unwrap();
    escalation::insert_ledger_tx(&mut tx, &ledger)
        .await
        .unwrap();

    // sha256:https://... looks like a URL prefixed with an approved algorithm name.
    let bypass_payload = r#"{"redacted_evidence_ref":"sha256:https://evil.example.com/path"}"#;
    let bad_event = EscalationEvent {
        id: "event-sec003-url".into(),
        escalation_ledger_id: "ledger-sec003-url".into(),
        event_kind_raw: "escalation.tier_selected".into(),
        tier_id: None,
        tier_kind_raw: None,
        trigger_raw: None,
        pause_reason_raw: None,
        payload_json: Some(bypass_payload.into()),
        redaction_version: Some("redaction_v1".into()),
        created_at: now,
    };
    let result = escalation::insert_event_tx(&mut tx, &bad_event).await;
    assert!(
        result.is_err(),
        "insert_event_tx must reject URL-scheme bypass in redacted_evidence_ref; \
         sha256:https://... must not pass validation"
    );
}

/// SEC-003: insert_event_tx must reject redacted_evidence_ref values containing absolute paths
/// after an approved prefix (e.g. sha256:/Users/...).
#[tokio::test]
async fn p058_sec003_event_rejects_absolute_path_bypass_in_ref_field() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    let now = chrono::Utc::now();
    let ledger = EscalationLedger {
        id: "ledger-sec003-path".into(),
        run_id,
        stage_id: "state_3".into(),
        agent_id: "code_writer".into(),
        policy_id: "policy-sec003-path".into(),
        policy_hash: "sha256:path".into(),
        status_raw: "active".into(),
        current_tier_id: None,
        current_tier_kind_raw: None,
        chain_attempt_index: 0,
        trigger_raw: None,
        pause_reason_raw: None,
        operator_action_hint: None,
        runbook_anchor: None,
        created_at: now,
        updated_at: now,
    };
    let mut tx = pool.begin().await.unwrap();
    escalation::insert_ledger_tx(&mut tx, &ledger)
        .await
        .unwrap();

    // sha256:/Users/... looks like an absolute path prefixed with an approved hash prefix.
    let bypass_payload = r#"{"redacted_evidence_ref":"sha256:/Users/attacker/secret.txt"}"#;
    let bad_event = EscalationEvent {
        id: "event-sec003-path".into(),
        escalation_ledger_id: "ledger-sec003-path".into(),
        event_kind_raw: "escalation.tier_selected".into(),
        tier_id: None,
        tier_kind_raw: None,
        trigger_raw: None,
        pause_reason_raw: None,
        payload_json: Some(bypass_payload.into()),
        redaction_version: Some("redaction_v1".into()),
        created_at: now,
    };
    let result = escalation::insert_event_tx(&mut tx, &bad_event).await;
    assert!(
        result.is_err(),
        "insert_event_tx must reject absolute-path bypass in redacted_evidence_ref; \
         sha256:/Users/... must not pass validation"
    );
}

/// SEC-003: a well-formed sha256 hex digest must still be accepted after the fix.
#[tokio::test]
async fn p058_sec003_event_accepts_valid_sha256_ref() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    let now = chrono::Utc::now();
    let ledger = EscalationLedger {
        id: "ledger-sec003-good".into(),
        run_id,
        stage_id: "state_3".into(),
        agent_id: "code_writer".into(),
        policy_id: "policy-sec003-good".into(),
        policy_hash: "sha256:good".into(),
        status_raw: "active".into(),
        current_tier_id: None,
        current_tier_kind_raw: None,
        chain_attempt_index: 0,
        trigger_raw: None,
        pause_reason_raw: None,
        operator_action_hint: None,
        runbook_anchor: None,
        created_at: now,
        updated_at: now,
    };
    escalation::insert_ledger(&pool, &ledger).await.unwrap();

    // A valid sha256 hex string — 64 hex characters.
    let valid_hash = "sha256:a3f1b2c4d5e6f7890123456789abcdef0123456789abcdef0123456789abcdef";
    let good_payload = format!(r#"{{"redacted_evidence_ref":"{valid_hash}"}}"#);
    let good_event = EscalationEvent {
        id: "event-sec003-good".into(),
        escalation_ledger_id: "ledger-sec003-good".into(),
        event_kind_raw: "escalation.tier_selected".into(),
        tier_id: None,
        tier_kind_raw: None,
        trigger_raw: None,
        pause_reason_raw: None,
        payload_json: Some(good_payload),
        redaction_version: Some("redaction_v1".into()),
        created_at: now,
    };
    let mut tx = pool.begin().await.unwrap();
    let result = escalation::insert_event_tx(&mut tx, &good_event).await;
    assert!(
        result.is_ok(),
        "insert_event_tx must accept a valid sha256 hex digest ref; got: {:?}",
        result.err()
    );
}

// SEC-004 authz contract test lives in mcp-server/tests/proposal_058_runtime_facts.rs
// since it requires mcp_server::tools::runs which is not a dep of this crate.

/// P058-SEC-M1: shadow decision JSON validator rejects unknown top-level keys.
#[tokio::test]
async fn p058_shadow_decision_json_rejects_unknown_keys() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    use chrono::Utc;
    use db::repos::{agent_executions, stages};
    use domain::agent::{AgentExecution, AgentStatus};
    use domain::ids::StageExecutionId;
    use domain::stage::{StageExecution, StageStatus};
    let now = Utc::now();
    let stage_id = StageExecutionId::new();
    let exec_id = AgentExecutionId::new();
    stages::insert(
        &pool,
        &StageExecution {
            id: stage_id,
            run_id,
            stage_id: "state_3".into(),
            label: "Impl".into(),
            status: StageStatus::Running,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: now,
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
        &pool,
        &AgentExecution {
            id: exec_id,
            stage_execution_id: Some(stage_id),
            agent_id: "code_writer".into(),
            provider: "claude".into(),
            model: Some("sonnet".into()),
            status: AgentStatus::Running,
            started_at: now,
            completed_at: None,
            owner_execution_lineage_id: None,
            session_lineage_id: None,
            session_generation_id: None,
            rehydrated_from_checkpoint_artifact_id: None,
            invocation_owner_key: Some("owner".into()),
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
    insert_runtime_facts_row(&pool, exec_id).await;

    // Unknown key "secret" must be rejected (P058-SEC-M1 allowlist enforcement).
    let mut tx = pool.begin().await.unwrap();
    let result = escalation::update_shadow_escalation_columns_tx(
        &mut tx,
        &exec_id.to_string(),
        Some("primary_retry"),
        Some("contract_output_failure"),
        Some(r#"{"tier_id": "t1", "secret": "sk-abc"}"#),
    )
    .await;
    assert!(
        result.is_err(),
        "unknown key in shadow decision JSON must be rejected"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("unknown key"),
        "error must mention unknown key; got: {msg}"
    );
}

/// P058-SEC-M1: shadow decision JSON validator accepts valid tier-metadata-only JSON.
#[tokio::test]
async fn p058_shadow_decision_json_accepts_valid_tier_metadata() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    use chrono::Utc;
    use db::repos::{agent_executions, stages};
    use domain::agent::{AgentExecution, AgentStatus};
    use domain::ids::StageExecutionId;
    use domain::stage::{StageExecution, StageStatus};
    let now = Utc::now();
    let stage_id = StageExecutionId::new();
    let exec_id = AgentExecutionId::new();
    stages::insert(
        &pool,
        &StageExecution {
            id: stage_id,
            run_id,
            stage_id: "state_3".into(),
            label: "Impl".into(),
            status: StageStatus::Running,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: now,
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
        &pool,
        &AgentExecution {
            id: exec_id,
            stage_execution_id: Some(stage_id),
            agent_id: "code_writer".into(),
            provider: "claude".into(),
            model: Some("sonnet".into()),
            status: AgentStatus::Running,
            started_at: now,
            completed_at: None,
            owner_execution_lineage_id: None,
            session_lineage_id: None,
            session_generation_id: None,
            rehydrated_from_checkpoint_artifact_id: None,
            invocation_owner_key: Some("owner".into()),
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
    insert_runtime_facts_row(&pool, exec_id).await;

    let valid_decision = r#"{"tier_id": "primary_retry", "tier_kind_raw": "same_backend_retry", "policy_id": "code_writer_default", "redaction_version": "redaction_v1"}"#;
    let mut tx = pool.begin().await.unwrap();
    let result = escalation::update_shadow_escalation_columns_tx(
        &mut tx,
        &exec_id.to_string(),
        Some("primary_retry"),
        Some("contract_output_failure"),
        Some(valid_decision),
    )
    .await;
    assert!(
        result.is_ok(),
        "valid shadow decision JSON must be accepted; got: {:?}",
        result.err()
    );
}

/// P058 Phase 1: insert_or_ignore_ledger is idempotent — concurrent creation returns same id.
#[tokio::test]
async fn p058_insert_or_ignore_ledger_idempotent() {
    use chrono::Utc;
    use db::repos::escalation;

    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;
    let now = Utc::now();

    let ledger = EscalationLedger {
        id: "ledger-idempotent-1".into(),
        run_id,
        stage_id: "state_3".into(),
        agent_id: "code_writer".into(),
        policy_id: "policy_a".into(),
        policy_hash: "sha256:abcdef1234".into(),
        status_raw: "active".into(),
        current_tier_id: Some("primary_retry".into()),
        current_tier_kind_raw: Some("same_backend_retry".into()),
        chain_attempt_index: 0,
        trigger_raw: None,
        pause_reason_raw: None,
        operator_action_hint: None,
        runbook_anchor: None,
        created_at: now,
        updated_at: now,
    };

    // First insertion succeeds and returns the new id.
    let id1 = escalation::insert_or_ignore_ledger(&pool, &ledger)
        .await
        .unwrap();
    assert_eq!(
        id1, "ledger-idempotent-1",
        "first insert must return the inserted id"
    );

    // Second insertion (same chain key, different id) returns the FIRST id (idempotent).
    let ledger2 = EscalationLedger {
        id: "ledger-idempotent-2".into(),
        ..ledger.clone()
    };
    let id2 = escalation::insert_or_ignore_ledger(&pool, &ledger2)
        .await
        .unwrap();
    assert_eq!(
        id2, "ledger-idempotent-1",
        "second insert_or_ignore for same chain key must return existing id, not new id"
    );
}

// ---------------------------------------------------------------------------
// BLOCK-2 hardening: shadow decision JSON must reject credential-shaped values
// for every identifier field, not just policy_hash/policy_id/decision_reason.
// P058-SEC-M1 / security-report HIGH-002.
// ---------------------------------------------------------------------------

/// Build a minimal pool + agent_execution + runtime_facts row for shadow column tests.
async fn setup_shadow_test_exec(pool: &sqlx::SqlitePool) -> AgentExecutionId {
    use chrono::Utc;
    use db::repos::{agent_executions, stages};
    use domain::agent::AgentStatus;
    use domain::ids::StageExecutionId;
    use domain::stage::{StageExecution, StageStatus};
    let run_id = RunId::new();
    insert_minimal_run(pool, run_id).await;
    let now = Utc::now();
    let stage_id = StageExecutionId::new();
    let exec_id = AgentExecutionId::new();
    stages::insert(
        pool,
        &StageExecution {
            id: stage_id,
            run_id,
            stage_id: "state_sec".into(),
            label: "Sec".into(),
            status: StageStatus::Running,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: now,
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
        &domain::agent::AgentExecution {
            id: exec_id,
            stage_execution_id: Some(stage_id),
            agent_id: "code_writer".into(),
            provider: "claude".into(),
            model: Some("sonnet".into()),
            status: AgentStatus::Running,
            started_at: now,
            completed_at: None,
            owner_execution_lineage_id: None,
            session_lineage_id: None,
            session_generation_id: None,
            rehydrated_from_checkpoint_artifact_id: None,
            invocation_owner_key: Some("owner".into()),
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
    insert_runtime_facts_row(pool, exec_id).await;
    exec_id
}

/// BLOCK-2 / P058-SEC-M1: tier_id must reject sk-* credential prefix.
#[tokio::test]
async fn p058_shadow_json_rejects_sk_credential_in_tier_id() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let exec_id = setup_shadow_test_exec(&pool).await;
    let bad = r#"{"tier_id": "sk-abcdef1234567890", "policy_id": "p1"}"#;
    let mut tx = pool.begin().await.unwrap();
    let result = escalation::update_shadow_escalation_columns_tx(
        &mut tx,
        &exec_id.to_string(),
        None,
        None,
        Some(bad),
    )
    .await;
    assert!(
        result.is_err(),
        "sk-* credential in tier_id must be rejected"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("credential") || msg.contains("P058-SEC-M1"),
        "error must reference credential rejection; got: {msg}"
    );
}

/// BLOCK-2 / P058-SEC-M1: trigger_raw must reject Bearer token prefix.
#[tokio::test]
async fn p058_shadow_json_rejects_bearer_token_in_trigger_raw() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let exec_id = setup_shadow_test_exec(&pool).await;
    let bad = r#"{"trigger_raw": "Bearer eyJhbGciOiJIUzI1NiJ9", "policy_id": "p1"}"#;
    let mut tx = pool.begin().await.unwrap();
    let result = escalation::update_shadow_escalation_columns_tx(
        &mut tx,
        &exec_id.to_string(),
        None,
        None,
        Some(bad),
    )
    .await;
    assert!(
        result.is_err(),
        "Bearer token in trigger_raw must be rejected"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("whitespace") || msg.contains("credential") || msg.contains("P058-SEC-M1"),
        "error must reference whitespace or credential rejection; got: {msg}"
    );
}

/// BLOCK-2 / P058-SEC-M1: tier_kind_raw must reject absolute Unix path.
#[tokio::test]
async fn p058_shadow_json_rejects_absolute_path_in_tier_kind_raw() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let exec_id = setup_shadow_test_exec(&pool).await;
    let bad = r#"{"tier_kind_raw": "same_backend_retry", "tier_id": "/Users/user/secret.txt"}"#;
    let mut tx = pool.begin().await.unwrap();
    let result = escalation::update_shadow_escalation_columns_tx(
        &mut tx,
        &exec_id.to_string(),
        None,
        None,
        Some(bad),
    )
    .await;
    assert!(result.is_err(), "absolute path in tier_id must be rejected");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("credential") || msg.contains("path") || msg.contains("P058-SEC-M1"),
        "error must reference credential/path rejection; got: {msg}"
    );
}

/// BLOCK-2 / P058-SEC-M1: chain_attempt_index must be a number, not a string.
#[tokio::test]
async fn p058_shadow_json_rejects_string_chain_attempt_index() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let exec_id = setup_shadow_test_exec(&pool).await;
    let bad = r#"{"chain_attempt_index": "two", "policy_id": "p1"}"#;
    let mut tx = pool.begin().await.unwrap();
    let result = escalation::update_shadow_escalation_columns_tx(
        &mut tx,
        &exec_id.to_string(),
        None,
        None,
        Some(bad),
    )
    .await;
    assert!(
        result.is_err(),
        "string chain_attempt_index must be rejected"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("chain_attempt_index") || msg.contains("number"),
        "error must reference chain_attempt_index or number type; got: {msg}"
    );
}

/// BLOCK-2 / P058-SEC-M1: redaction_version in shadow JSON must be in known allowlist.
#[tokio::test]
async fn p058_shadow_json_rejects_unknown_redaction_version() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let exec_id = setup_shadow_test_exec(&pool).await;
    let bad = r#"{"redaction_version": "arbitrary_v99", "policy_id": "p1"}"#;
    let mut tx = pool.begin().await.unwrap();
    let result = escalation::update_shadow_escalation_columns_tx(
        &mut tx,
        &exec_id.to_string(),
        None,
        None,
        Some(bad),
    )
    .await;
    assert!(
        result.is_err(),
        "unknown redaction_version in shadow JSON must be rejected"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("redaction_version") || msg.contains("allowlist"),
        "error must reference redaction_version or allowlist; got: {msg}"
    );
}

/// BLOCK-2 / P058-SEC-M1: timestamp_utc must not accept prose or credential-shaped strings.
#[tokio::test]
async fn p058_shadow_json_rejects_prose_in_timestamp_utc() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let exec_id = setup_shadow_test_exec(&pool).await;
    let bad = r#"{"timestamp_utc": "Bearer sk-abc not a timestamp", "policy_id": "p1"}"#;
    let mut tx = pool.begin().await.unwrap();
    let result = escalation::update_shadow_escalation_columns_tx(
        &mut tx,
        &exec_id.to_string(),
        None,
        None,
        Some(bad),
    )
    .await;
    assert!(result.is_err(), "prose in timestamp_utc must be rejected");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("timestamp_utc") || msg.contains("ISO 8601") || msg.contains("P058-SEC-M1"),
        "error must reference timestamp_utc or ISO 8601; got: {msg}"
    );
}

/// BLOCK-2 / P058-SEC-M1: would_select_tier_id column input rejects sk-* credential prefix.
#[tokio::test]
async fn p058_shadow_column_tier_id_rejects_credential_prefix() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let exec_id = setup_shadow_test_exec(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    let result = escalation::update_shadow_escalation_columns_tx(
        &mut tx,
        &exec_id.to_string(),
        Some("sk-abcdef1234567890"), // credential-shaped tier_id input
        None,
        None,
    )
    .await;
    assert!(
        result.is_err(),
        "sk-* in would_select_tier_id must be rejected"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("credential") || msg.contains("P058-SEC-M1"),
        "error must reference credential rejection; got: {msg}"
    );
}

/// BLOCK-2 / P058-SEC-M1: would_select_trigger_raw column input rejects absolute path.
#[tokio::test]
async fn p058_shadow_column_trigger_raw_rejects_absolute_path() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let exec_id = setup_shadow_test_exec(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    let result = escalation::update_shadow_escalation_columns_tx(
        &mut tx,
        &exec_id.to_string(),
        None,
        Some("/Users/user/tokens.json"), // absolute path as trigger
        None,
    )
    .await;
    assert!(
        result.is_err(),
        "absolute path in would_select_trigger_raw must be rejected"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("credential") || msg.contains("path") || msg.contains("P058-SEC-M1"),
        "error must reference credential/path rejection; got: {msg}"
    );
}

/// BLOCK-2 / P058-SEC-M1: would_select_trigger_raw column input rejects whitespace prose.
#[tokio::test]
async fn p058_shadow_column_trigger_raw_rejects_prose() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let exec_id = setup_shadow_test_exec(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    let result = escalation::update_shadow_escalation_columns_tx(
        &mut tx,
        &exec_id.to_string(),
        None,
        Some("This is a raw agent transcript line with prose"), // prose
        None,
    )
    .await;
    assert!(
        result.is_err(),
        "prose in would_select_trigger_raw must be rejected"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("whitespace") || msg.contains("identifier") || msg.contains("P058-SEC-M1"),
        "error must reference whitespace or identifier; got: {msg}"
    );
}

/// BLOCK-2 / P058-SEC-M1: would_select_tier_id column input accepts valid identifier.
#[tokio::test]
async fn p058_shadow_column_inputs_accept_valid_identifiers() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let exec_id = setup_shadow_test_exec(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    let result = escalation::update_shadow_escalation_columns_tx(
        &mut tx,
        &exec_id.to_string(),
        Some("primary_retry"),
        Some("contract_output_failure"),
        None,
    )
    .await;
    assert!(
        result.is_ok(),
        "valid identifier column inputs must be accepted; got: {:?}",
        result.err()
    );
}

/// MEDIUM-002 hardening: identifier fields must reject absolute Unix paths outside /users and /home.
/// Covers /tmp, /private/tmp, and /opt — path shapes not in the original PATH_PATTERNS prefix list.
#[tokio::test]
async fn p058_shadow_column_rejects_tmp_and_opt_absolute_paths() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let exec_id = setup_shadow_test_exec(&pool).await;

    for bad_path in ["/tmp/token", "/private/tmp/key", "/opt/secret"] {
        let mut tx = pool.begin().await.unwrap();
        let result = escalation::update_shadow_escalation_columns_tx(
            &mut tx,
            &exec_id.to_string(),
            None,
            Some(bad_path),
            None,
        )
        .await;
        assert!(
            result.is_err(),
            "absolute path '{bad_path}' in would_select_trigger_raw must be rejected"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("credential") || msg.contains("path") || msg.contains("P058-SEC-MEDIUM"),
            "error must reference credential/path rejection for '{bad_path}'; got: {msg}"
        );
    }
}

/// BLOCK-4 / MEDIUM-001: direct SQL INSERT into escalation_events without
/// redaction_version must fail the NOT NULL constraint (no DEFAULT present).
#[tokio::test]
async fn p058_escalation_events_direct_insert_without_redaction_version_fails() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    let now = chrono::Utc::now();
    let ledger = EscalationLedger {
        id: "ledger-block4-001".into(),
        run_id,
        stage_id: "state_block4".into(),
        agent_id: "code_writer".into(),
        policy_id: "policy-block4".into(),
        policy_hash: "sha256:block4".into(),
        status_raw: "active".into(),
        current_tier_id: None,
        current_tier_kind_raw: None,
        chain_attempt_index: 0,
        trigger_raw: None,
        pause_reason_raw: None,
        operator_action_hint: None,
        runbook_anchor: None,
        created_at: now,
        updated_at: now,
    };
    escalation::insert_ledger(&pool, &ledger).await.unwrap();

    // Attempt direct SQL INSERT omitting redaction_version — must fail NOT NULL.
    let result = sqlx::query(
        "INSERT INTO escalation_events \
         (id, escalation_ledger_id, event_kind_raw, created_at) \
         VALUES ('evt-block4', 'ledger-block4-001', 'escalation.tier_selected', ?1)",
    )
    .bind(now.to_rfc3339())
    .execute(&pool)
    .await;

    assert!(
        result.is_err(),
        "direct INSERT without redaction_version must fail NOT NULL or CHECK constraint"
    );
}

/// BLOCK-4 / MEDIUM-001: direct SQL INSERT with invalid redaction_version must fail CHECK.
#[tokio::test]
async fn p058_escalation_events_direct_insert_with_invalid_redaction_version_fails() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    let now = chrono::Utc::now();
    let ledger = EscalationLedger {
        id: "ledger-block4-002".into(),
        run_id,
        stage_id: "state_block4b".into(),
        agent_id: "code_writer".into(),
        policy_id: "policy-block4b".into(),
        policy_hash: "sha256:block4b".into(),
        status_raw: "active".into(),
        current_tier_id: None,
        current_tier_kind_raw: None,
        chain_attempt_index: 0,
        trigger_raw: None,
        pause_reason_raw: None,
        operator_action_hint: None,
        runbook_anchor: None,
        created_at: now,
        updated_at: now,
    };
    escalation::insert_ledger(&pool, &ledger).await.unwrap();

    // INSERT with an unknown redaction_version stamp — must fail CHECK constraint.
    let result = sqlx::query(
        "INSERT INTO escalation_events \
         (id, escalation_ledger_id, event_kind_raw, redaction_version, created_at) \
         VALUES ('evt-block4b', 'ledger-block4-002', 'escalation.tier_selected', 'v99_unreviewed', ?1)"
    )
    .bind(now.to_rfc3339())
    .execute(&pool)
    .await;

    assert!(
        result.is_err(),
        "direct INSERT with unknown redaction_version must fail CHECK constraint"
    );
}

// ── MEDIUM-002 regression: insert_or_ignore_ledger rejects stale policy_hash ──

/// P058 MEDIUM-002: insert_or_ignore_ledger must refuse to reuse an existing chain row
/// when the candidate's policy_hash differs from the stored row's policy_hash. Silent
/// reuse would attach new executions to a chain whose frozen policy no longer matches
/// the run plan, silently undermining escalation attribution audit invariants.
#[tokio::test]
async fn p058_insert_or_ignore_ledger_rejects_policy_hash_drift() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    let now = Utc::now();
    let original_ledger = EscalationLedger {
        id: uuid::Uuid::new_v4().to_string(),
        run_id,
        stage_id: "impl_stage".into(),
        agent_id: "code_writer".into(),
        policy_id: "drift_test_policy".into(),
        policy_hash: "sha256:aaaaaaaaaaaaaaaa".into(),
        status_raw: "active".into(),
        current_tier_id: Some("retry_tier".into()),
        current_tier_kind_raw: Some("same_backend_retry".into()),
        chain_attempt_index: 0,
        trigger_raw: None,
        pause_reason_raw: None,
        operator_action_hint: None,
        runbook_anchor: None,
        created_at: now,
        updated_at: now,
    };

    // First insert succeeds.
    let ledger_id = escalation::insert_or_ignore_ledger(&pool, &original_ledger)
        .await
        .expect("first insert_or_ignore_ledger must succeed");
    assert_eq!(ledger_id, original_ledger.id);

    // Second call with same key but different policy_hash must fail.
    let drifted_ledger = EscalationLedger {
        id: uuid::Uuid::new_v4().to_string(),
        run_id,
        stage_id: "impl_stage".into(),
        agent_id: "code_writer".into(),
        policy_id: "drift_test_policy".into(),
        policy_hash: "sha256:bbbbbbbbbbbbbbbb".into(), // different hash → drift
        status_raw: "active".into(),
        current_tier_id: Some("retry_tier".into()),
        current_tier_kind_raw: Some("same_backend_retry".into()),
        chain_attempt_index: 0,
        trigger_raw: None,
        pause_reason_raw: None,
        operator_action_hint: None,
        runbook_anchor: None,
        created_at: now,
        updated_at: now,
    };

    let result = escalation::insert_or_ignore_ledger(&pool, &drifted_ledger).await;
    assert!(
        result.is_err(),
        "insert_or_ignore_ledger with drifted policy_hash must return an error (MEDIUM-002)"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("escalation_policy_drift"),
        "error must mention escalation_policy_drift; got: {err_msg}"
    );

    // P058 policy-drift durability: the existing ledger row must now be in drift-paused
    // state so the pause is observable through GraphQL/MCP readback. This proves that
    // insert_or_ignore_ledger opens a durable pause rather than only returning an error.
    let (status, pause_reason): (String, Option<String>) =
        sqlx::query_as("SELECT status_raw, pause_reason_raw FROM escalation_ledger WHERE id = ?")
            .bind(&original_ledger.id)
            .fetch_one(&pool)
            .await
            .expect("original ledger must still exist after drift detection");
    assert_eq!(
        status, "paused",
        "ledger must be in 'paused' status after drift detection; got: {status}"
    );
    assert_eq!(
        pause_reason.as_deref(),
        Some("escalation_policy_drift"),
        "pause_reason_raw must be 'escalation_policy_drift'; got: {pause_reason:?}"
    );
}

/// P058 MEDIUM-002 (positive case): insert_or_ignore_ledger returns the existing
/// ledger_id without error when the candidate's policy_hash matches the stored row.
#[tokio::test]
async fn p058_insert_or_ignore_ledger_returns_existing_id_on_matching_hash() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;

    let now = Utc::now();
    let ledger = EscalationLedger {
        id: "idempotent-ledger-001".into(),
        run_id,
        stage_id: "idempotent_stage".into(),
        agent_id: "code_writer".into(),
        policy_id: "idempotent_policy".into(),
        policy_hash: "sha256:cccccccccccccccc".into(),
        status_raw: "active".into(),
        current_tier_id: Some("tier_a".into()),
        current_tier_kind_raw: Some("same_backend_retry".into()),
        chain_attempt_index: 0,
        trigger_raw: None,
        pause_reason_raw: None,
        operator_action_hint: None,
        runbook_anchor: None,
        created_at: now,
        updated_at: now,
    };

    let id1 = escalation::insert_or_ignore_ledger(&pool, &ledger)
        .await
        .expect("first call must succeed");

    // Second call with same key AND same hash must be idempotent.
    let ledger2 = EscalationLedger {
        id: "idempotent-ledger-002".into(), // different candidate id — must return first
        ..ledger.clone()
    };
    let id2 = escalation::insert_or_ignore_ledger(&pool, &ledger2)
        .await
        .expect("idempotent second call must succeed when hashes match");

    assert_eq!(
        id1, id2,
        "idempotent calls must return the same existing ledger_id"
    );
    assert_eq!(
        id1, "idempotent-ledger-001",
        "returned id must be the original row's id"
    );
}

/// Regression test: next_tier_attempt_index_tx must return 0 for the first insertion and
/// monotonically increment for subsequent insertions on the same (ledger_id, tier_id).
/// Without this fix, hardcoded 0 would violate the UNIQUE(escalation_ledger_id, tier_id,
/// tier_attempt_index) index on the second execution.
#[tokio::test]
async fn p058_next_tier_attempt_index_increments_per_tier_prevents_idempotency_violation() {
    use db::repos::{agent_executions, escalation, stages};
    use domain::agent::{AgentExecution, AgentStatus};
    use domain::ids::{AgentExecutionId, StageExecutionId};
    use domain::stage::{StageExecution, StageStatus};

    let pool = create_pool("sqlite::memory:").await.unwrap();
    let run_id = RunId::new();
    insert_minimal_run(&pool, run_id).await;
    let now = Utc::now();

    let ledger = EscalationLedger {
        id: "ledger-retry-idx-001".into(),
        run_id,
        stage_id: "state_impl".into(),
        agent_id: "code_writer".into(),
        policy_id: "test_policy".into(),
        policy_hash: "sha256:aaaa".into(),
        status_raw: "active".into(),
        current_tier_id: Some("primary_retry".into()),
        current_tier_kind_raw: Some("same_backend_retry".into()),
        chain_attempt_index: 0,
        trigger_raw: None,
        pause_reason_raw: None,
        operator_action_hint: None,
        runbook_anchor: None,
        created_at: now,
        updated_at: now,
    };
    escalation::insert_ledger(&pool, &ledger).await.unwrap();

    let stage_id = StageExecutionId::new();
    stages::insert(
        &pool,
        &StageExecution {
            id: stage_id,
            run_id,
            stage_id: "state_impl".into(),
            label: "Impl".into(),
            status: StageStatus::Running,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: now,
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

    // Helper to insert an agent_execution and an execution_metadata row using
    // next_tier_attempt_index_tx to compute the attempt index.
    let insert_attempt = |pool: &sqlx::SqlitePool, ledger_id: &str, tier_id: &str| {
        let pool = pool.clone();
        let ledger_id = ledger_id.to_string();
        let tier_id = tier_id.to_string();
        async move {
            let exec_id = AgentExecutionId::new();
            agent_executions::insert(
                &pool,
                &AgentExecution {
                    id: exec_id,
                    stage_execution_id: Some(stage_id),
                    agent_id: "code_writer".into(),
                    provider: "claude".into(),
                    model: None,
                    status: AgentStatus::Running,
                    started_at: now,
                    completed_at: None,
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
                    actual_toolchain_mapping_diagnostics_json: None,
                    transcript_artifact_id: None,
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

            let mut tx = pool.begin().await.unwrap();
            let idx = escalation::next_tier_attempt_index_tx(&mut tx, &ledger_id, &tier_id)
                .await
                .expect("next_tier_attempt_index_tx must not fail");
            escalation::insert_execution_metadata_tx(
                &mut tx,
                &EscalationExecutionMetadata {
                    agent_execution_id: exec_id,
                    escalation_ledger_id: ledger_id,
                    tier_id,
                    tier_kind_raw: EscalationTierKind::SameBackendRetry.to_string(),
                    tier_attempt_index: idx,
                    trigger_raw: None,
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
            tx.commit().await.unwrap();
            idx
        }
    };

    // First attempt: expect index 0.
    let idx0 = insert_attempt(&pool, "ledger-retry-idx-001", "primary_retry").await;
    assert_eq!(idx0, 0, "first attempt on a fresh tier must use index 0");

    // Second attempt on same tier: expect index 1 (not a duplicate-key violation).
    let idx1 = insert_attempt(&pool, "ledger-retry-idx-001", "primary_retry").await;
    assert_eq!(idx1, 1, "second attempt on same tier must use index 1");

    // Third attempt on same tier: expect index 2.
    let idx2 = insert_attempt(&pool, "ledger-retry-idx-001", "primary_retry").await;
    assert_eq!(idx2, 2, "third attempt on same tier must use index 2");

    // Different tier_id on the same ledger: index counter is independent.
    let idx_other = insert_attempt(&pool, "ledger-retry-idx-001", "frontier_profile").await;
    assert_eq!(
        idx_other, 0,
        "first attempt on a different tier must start at 0"
    );

    // Verify all four rows are readable without constraint errors.
    let rows = escalation::find_execution_metadata_by_ledger(&pool, "ledger-retry-idx-001")
        .await
        .unwrap();
    assert_eq!(
        rows.len(),
        4,
        "all four execution-metadata rows must be readable"
    );
}
