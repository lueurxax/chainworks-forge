/// P058 Phase 0-1: Escalation schema, persistence, and domain type validation.
/// Verifies the migration creates all required tables with correct columns,
/// that ledger/event insert/read round-trips work, and that pause reason vocabulary
/// matches the catalog defined in the proposal.
use chrono::Utc;
use db::pool::create_pool;
use db::repos::escalation;
use domain::escalation::{
    EscalationEvent, EscalationExecutionMetadata, EscalationLedger, EscalationPauseReason,
    EscalationTierKind,
};
use domain::ids::{AgentExecutionId, RunId};

async fn insert_minimal_run(pool: &sqlx::SqlitePool, run_id: RunId) {
    use chrono::Utc;
    use db::repos::{ideas, runs};
    use domain::idea::{Idea, IdeaStatus};
    use domain::ids::IdeaId;
    use domain::run::{Run, RunStatus};
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
    assert_eq!(row.trigger_raw.as_deref(), Some("repeated_same_blocker_digest"));
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
    escalation::insert_ledger_tx(&mut tx, &ledger).await.unwrap();

    let event = EscalationEvent {
        id: "event-001".into(),
        escalation_ledger_id: "ledger-events-001".into(),
        event_kind_raw: "escalation.tier_selected".into(),
        tier_id: Some("primary_retry".into()),
        tier_kind_raw: Some("same_backend_retry".into()),
        trigger_raw: Some("contract_output_failure".into()),
        pause_reason_raw: None,
        payload_json: None,
        redaction_version: Some("redaction_v1".into()),
        created_at: now,
    };
    escalation::insert_event_tx(&mut tx, &event).await.unwrap();
    tx.commit().await.unwrap();

    let events = escalation::find_events_by_ledger(&pool, "ledger-events-001")
        .await
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_kind_raw, "escalation.tier_selected");
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
    escalation::insert_ledger_tx(&mut tx, &ledger).await.unwrap();

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
        payload_json: Some(r#"{"from_tier":"primary_retry","to_tier":"frontier_profile"}"#.into()),
        redaction_version: Some("redaction_v1".into()),
        created_at: now,
    };
    let mut tx = pool.begin().await.unwrap();
    escalation::insert_event_tx(&mut tx, &good_event).await.unwrap();
    tx.commit().await.unwrap();

    let events = escalation::find_events_by_ledger(&pool, "ledger-json-good-001")
        .await
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id, "event-good-json");
    assert_eq!(
        events[0].redaction_version.as_deref(),
        Some("redaction_v1")
    );
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
    use domain::agent::{AgentExecution, AgentStatus};
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
    escalation::insert_ledger_tx(&mut tx, &ledger).await.unwrap();
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
    escalation::insert_ledger_tx(&mut tx, &ledger).await.unwrap();

    let event_no_redaction = EscalationEvent {
        id: "event-no-redact".into(),
        escalation_ledger_id: "ledger-no-redact-001".into(),
        event_kind_raw: "escalation.tier_selected".into(),
        tier_id: Some("primary_retry".into()),
        tier_kind_raw: Some("same_backend_retry".into()),
        trigger_raw: Some("contract_output_failure".into()),
        pause_reason_raw: None,
        payload_json: Some(r#"{"key":"value"}"#.into()),
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
    assert_eq!(reasons.len(), 13, "proposal pause_reason_catalog has 13 entries");
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
    use db::repos::agent_execution_runtime_facts;
    use domain::agent::AgentExecutionRuntimeFacts;
    use chrono::Utc;
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

    use db::repos::{agent_executions, stages};
    use domain::agent::{AgentExecution, AgentStatus};
    use domain::ids::StageExecutionId;
    use domain::stage::{StageExecution, StageStatus};
    use chrono::Utc;
    let now = Utc::now();
    let stage_id = StageExecutionId::new();
    let exec_id = AgentExecutionId::new();
    stages::insert(&pool, &StageExecution {
        id: stage_id, run_id, stage_id: "state_3".into(), label: "Impl".into(),
        status: StageStatus::Running, iteration: 1, attempt_number: 1,
        settlement_kind: None, started_at: now, completed_at: None,
        owner_agent: None, provider: None, model: None, stage_type: None,
        validation_failure_json: None, evidence_packet_json: None,
        recovery_snapshot_json: None, retry_reason: None,
    }).await.unwrap();
    agent_executions::insert(&pool, &AgentExecution {
        id: exec_id, stage_execution_id: Some(stage_id),
        agent_id: "code_writer".into(), provider: "claude".into(),
        model: Some("sonnet".into()), status: AgentStatus::Running,
        started_at: now, completed_at: None,
        owner_execution_lineage_id: None, session_lineage_id: None,
        session_generation_id: None, rehydrated_from_checkpoint_artifact_id: None,
        invocation_owner_key: Some("owner".into()), session_reuse_scope: None,
        session_family_id: None, session_reuse_disposition: None,
        session_reset_reason: None, backend_profile_id: None,
        requested_mcp_extensions_json: None, predicted_mcp_extensions_json: None,
        predicted_mcp_runtime_ids_json: None, actual_mcp_extensions_json: None,
        actual_mcp_runtime_ids_json: None, denied_mcp_extensions_json: None,
        mcp_blocking_issues_json: None, actual_mcp_observation_json: None,
        actual_xcode_runtime_observation_json: None, mcp_session_startup_latency_ms: None,
        owner_kind: None, owner_id: None, lead_mediation_record_id: None,
        origin_stage_execution_id: None, total_cost_cents: None,
        input_tokens: None, output_tokens: None, cached_input_tokens: None,
        transcript_artifact_id: None, actual_toolchain_mapping_diagnostics_json: None,
        escalation_policy_id: None,
        escalation_policy_hash: None,
        escalation_tier_id: None,
        escalation_tier_kind_raw: None,
        escalation_trigger_raw: None,
        escalation_digest_version: None,
        escalation_ledger_id: None,
    }).await.unwrap();
    insert_runtime_facts_row(&pool, exec_id).await;

    let mut tx = pool.begin().await.unwrap();
    let result = escalation::update_shadow_escalation_columns_tx(
        &mut tx,
        &exec_id.to_string(),
        Some("primary_retry"),
        Some("contract_output_failure"),
        Some("{ not valid json ]]]"),
    ).await;
    assert!(result.is_err(), "must reject malformed would_select_decision_json");
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("malformed JSON"), "error must say malformed JSON; got: {msg}");
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
    ).await;
    assert!(result.is_err(), "must fail when no runtime_facts row exists");
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("no agent_execution_runtime_facts"), "error must name the missing row; got: {msg}");
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
    escalation::insert_ledger_tx(&mut tx, &ledger).await.unwrap();

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
