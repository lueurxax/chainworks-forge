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
