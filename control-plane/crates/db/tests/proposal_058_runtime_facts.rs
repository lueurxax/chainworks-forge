use chrono::Utc;
use db::pool::create_pool;
use db::repos::{
    agent_execution_runtime_facts, agent_executions, agent_retry_budget_ledger, artifact_contracts,
    artifacts, escalation, ideas, runs, stages, work_items,
};
use db::work_item::{WorkItem, WorkItemKind, WorkItemStatus};
use domain::agent::{
    AgentExecution, AgentExecutionRuntimeFacts, AgentFailureKind, AgentOutputSettlement,
    ArtifactSourceClaimState, OperatorActionHint,
};
use domain::artifact::{Artifact, ArtifactFormat};
use domain::artifact_contracts::{
    ActiveArtifactGenerationInput, ArtifactSourceGenerationClaim, ArtifactSourceGenerationClaimKey,
    SourceGenerationImportDecision,
};
use domain::escalation::{EscalationEvent, EscalationExecutionMetadata, EscalationLedger};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{AgentExecutionId, ArtifactId, IdeaId, RunId, StageExecutionId};
use domain::mediation::OwnerKind;
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageStatus};

async fn test_pool() -> sqlx::SqlitePool {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let writer = std::sync::Arc::new(db::writer::DbWriter::new(pool.clone()));
    db::writer::register_shared_writer(&pool, writer)
        .await
        .expect("test shared DbWriter registration failed");
    pool
}

fn make_run(run_id: RunId, idea_id: IdeaId) -> Run {
    Run {
        id: run_id,
        idea_id,
        status: RunStatus::Running,
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
        drift_detected_at: None,
        drift_details_json: None,
        workflow_snapshot_json: None,
        catalog_snapshot_json: None,
        chainworks_meta_root: None,
        review_routing_json: None,
        closeout_readiness_mode: None,
    }
}

async fn seed_execution(
    pool: &sqlx::SqlitePool,
) -> (RunId, StageExecutionId, AgentExecutionId, String) {
    use db::writer::{register_shared_writer, DbWriter};
    use std::sync::Arc;
    // P075 DbWriter must be registered before calling repos that use execute_repository_write!.
    let writer = Arc::new(DbWriter::new(pool.clone()));
    register_shared_writer(pool, writer).await.unwrap();
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();
    let agent_execution_id = AgentExecutionId::new();
    let source_work_item_id = uuid::Uuid::new_v4().to_string();
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
            stage_execution_id: Some(stage_execution_id),
            agent_id: "code_writer".into(),
            provider: "claude".into(),
            model: Some("sonnet".into()),
            started_at: Utc::now(),
            completed_at: None,
            status: domain::agent::AgentStatus::Running,
            owner_execution_lineage_id: Some(stage_execution_id.to_string()),
            session_lineage_id: Some("lineage-1".into()),
            session_generation_id: Some("generation-1".into()),
            rehydrated_from_checkpoint_artifact_id: None,
            invocation_owner_key: Some("owner-key".into()),
            session_reuse_scope: Some("same_agent_family_within_run".into()),
            session_family_id: Some("family-1".into()),
            session_reuse_disposition: Some("fresh".into()),
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
    (
        run_id,
        stage_execution_id,
        agent_execution_id,
        source_work_item_id,
    )
}

#[tokio::test]
async fn proposal_058_runtime_facts_upsert_preserves_unknown_raw_debug() {
    let pool = test_pool().await;
    let (run_id, stage_execution_id, agent_execution_id, _work_item_id) =
        seed_execution(&pool).await;
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
    facts.failure_message_redacted = Some("limit resets 10pm Asia/Nicosia".into());
    facts.retry_after = Some(now);
    facts.operator_action_hint = Some(OperatorActionHint::WaitUntilRetryAfter);
    facts.output_settlement = AgentOutputSettlement::MissingRequiredOutputs;
    facts.quota_ledger_id = Some(ledger.id.clone());
    agent_execution_runtime_facts::upsert(&pool, &facts)
        .await
        .unwrap();

    let read = agent_execution_runtime_facts::find_by_execution_id(&pool, agent_execution_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(read.failure_kind, Some(AgentFailureKind::ProviderQuota));
    assert_eq!(
        read.failure_kind_raw_debug.as_deref(),
        Some("future_provider_quota_variant")
    );
    assert_eq!(
        read.operator_action_hint,
        Some(OperatorActionHint::WaitUntilRetryAfter)
    );
    assert_eq!(
        read.output_settlement,
        AgentOutputSettlement::MissingRequiredOutputs
    );
    assert_eq!(read.quota_ledger_id.as_deref(), Some(ledger.id.as_str()));
}

#[tokio::test]
async fn proposal_058_counts_recent_escalation_launches_for_storm_detection() {
    let pool = test_pool().await;
    let (run_id, _stage_execution_id, agent_execution_id, _work_item_id) =
        seed_execution(&pool).await;
    let now = Utc::now();
    escalation::insert_ledger(
        &pool,
        &EscalationLedger {
            id: "ledger-p058-recent-launches".into(),
            run_id,
            stage_id: "state_1".into(),
            agent_id: "code_writer".into(),
            policy_id: "policy-p058".into(),
            policy_hash: "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .into(),
            status_raw: "active".into(),
            current_tier_id: Some("primary_retry".into()),
            current_tier_kind_raw: Some("same_backend_retry".into()),
            chain_attempt_index: 0,
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
        &pool,
        &EscalationExecutionMetadata {
            agent_execution_id,
            escalation_ledger_id: "ledger-p058-recent-launches".into(),
            tier_id: "primary_retry".into(),
            tier_kind_raw: "same_backend_retry".into(),
            tier_attempt_index: 0,
            trigger_raw: Some("contract_output_failure".into()),
            digest_version: Some("escalation_blocker_digest_v1".into()),
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

    let recent = escalation::count_recent_metas_by_ledger(
        &pool,
        "ledger-p058-recent-launches",
        now - chrono::Duration::seconds(300),
    )
    .await
    .unwrap();
    let future = escalation::count_recent_metas_by_ledger(
        &pool,
        "ledger-p058-recent-launches",
        now + chrono::Duration::seconds(1),
    )
    .await
    .unwrap();

    assert_eq!(recent, 1);
    assert_eq!(future, 0);
}

#[tokio::test]
async fn proposal_058_late_frame_event_and_runtime_facts_share_transaction() {
    let pool = test_pool().await;
    let (run_id, _stage_execution_id, agent_execution_id, _work_item_id) =
        seed_execution(&pool).await;
    let now = Utc::now();
    escalation::insert_ledger(
        &pool,
        &EscalationLedger {
            id: "ledger-p058-late-frame".into(),
            run_id,
            stage_id: "state_1".into(),
            agent_id: "code_writer".into(),
            policy_id: "policy-p058".into(),
            policy_hash: "sha256:abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd"
                .into(),
            status_raw: "active".into(),
            current_tier_id: Some("primary_retry".into()),
            current_tier_kind_raw: Some("same_backend_retry".into()),
            chain_attempt_index: 0,
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
        &pool,
        &EscalationExecutionMetadata {
            agent_execution_id,
            escalation_ledger_id: "ledger-p058-late-frame".into(),
            tier_id: "primary_retry".into(),
            tier_kind_raw: "same_backend_retry".into(),
            tier_attempt_index: 0,
            trigger_raw: Some("contract_output_failure".into()),
            digest_version: Some("escalation_blocker_digest_v1".into()),
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

    let before_late_frame =
        db::metrics::get_counter("escalation_provider_late_frame_after_detach_total");
    let before_pause =
        db::metrics::get_counter("escalation_pause_total:provider_session_force_detached");
    let mut rollback_tx = pool.begin().await.unwrap();
    let mut facts = AgentExecutionRuntimeFacts::defaults_for(agent_execution_id, now);
    facts.output_settlement = AgentOutputSettlement::IgnoredLateOutputs;
    facts.late_output_count = 1;
    facts.ignored_late_output_count = 1;
    agent_execution_runtime_facts::upsert_tx(&mut rollback_tx, &facts)
        .await
        .unwrap();
    escalation::insert_event_tx(
        &mut rollback_tx,
        &EscalationEvent {
            id: "p058-late-frame-rollback".into(),
            escalation_ledger_id: "ledger-p058-late-frame".into(),
            event_kind_raw: "escalation.provider_late_frame_after_detach".into(),
            tier_id: Some("primary_retry".into()),
            tier_kind_raw: Some("same_backend_retry".into()),
            trigger_raw: Some("contract_output_failure".into()),
            pause_reason_raw: Some("provider_session_force_detached".into()),
            payload_json: Some(r#"{"event_kind_raw":"escalation.provider_late_frame_after_detach","tier_id":"primary_retry","tier_kind_raw":"same_backend_retry","trigger_raw":"contract_output_failure","digest_version":"escalation_blocker_digest_v1","redacted_evidence_ref":"sha256:abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd"}"#.into()),
            redaction_version: Some("redaction_v1".into()),
            created_at: now,
        },
    )
    .await
    .unwrap();
    rollback_tx.rollback().await.unwrap();

    assert!(
        agent_execution_runtime_facts::find_by_execution_id(&pool, agent_execution_id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        escalation::find_events_by_ledger(&pool, "ledger-p058-late-frame")
            .await
            .unwrap()
            .is_empty()
    );

    let mut commit_tx = pool.begin().await.unwrap();
    let meta = escalation::find_execution_metadata_for_agent_tx(
        &mut commit_tx,
        &agent_execution_id.to_string(),
    )
    .await
    .unwrap()
    .expect("escalation metadata must be visible inside the commit transaction");
    assert_eq!(meta.escalation_ledger_id, "ledger-p058-late-frame");
    agent_execution_runtime_facts::upsert_tx(&mut commit_tx, &facts)
        .await
        .unwrap();
    escalation::insert_event_tx(
        &mut commit_tx,
        &EscalationEvent {
            id: "p058-late-frame-commit".into(),
            escalation_ledger_id: "ledger-p058-late-frame".into(),
            event_kind_raw: "escalation.provider_late_frame_after_detach".into(),
            tier_id: Some(meta.tier_id),
            tier_kind_raw: Some(meta.tier_kind_raw),
            trigger_raw: meta.trigger_raw,
            pause_reason_raw: Some("provider_session_force_detached".into()),
            payload_json: Some(r#"{"event_kind_raw":"escalation.provider_late_frame_after_detach","tier_id":"primary_retry","tier_kind_raw":"same_backend_retry","trigger_raw":"contract_output_failure","digest_version":"escalation_blocker_digest_v1","redacted_evidence_ref":"sha256:abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd"}"#.into()),
            redaction_version: Some("redaction_v1".into()),
            created_at: now,
        },
    )
    .await
    .unwrap();
    commit_tx.commit().await.unwrap();

    let events = escalation::find_events_by_ledger(&pool, "ledger-p058-late-frame")
        .await
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].event_kind_raw,
        "escalation.provider_late_frame_after_detach"
    );
    assert_eq!(
        db::metrics::get_counter("escalation_provider_late_frame_after_detach_total"),
        before_late_frame + 2,
        "metrics emit when the authoritative event is written"
    );
    assert_eq!(
        db::metrics::get_counter("escalation_pause_total:provider_session_force_detached"),
        before_pause + 2,
        "pause metric is labeled by durable pause reason"
    );
}

#[tokio::test]
async fn proposal_058_unknown_failure_kind_backfills_raw_debug_from_stored_value() {
    let pool = test_pool().await;
    let (_run_id, _stage_id, agent_execution_id, _work_item_id) = seed_execution(&pool).await;
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"INSERT INTO agent_execution_runtime_facts
           (agent_execution_id, failure_kind, failure_kind_raw_debug, failure_kind_version,
            failure_message_redacted, failure_message_redaction_version, retry_after,
            operator_action_hint, provider_exit_status, transport_error_code,
            supervision_classification, output_settlement, valid_required_outputs,
            late_output_count, ignored_late_output_count, session_reuse_reason,
            quota_ledger_id, created_at, updated_at)
           VALUES (?1, 'future_provider_shape', NULL, 1, NULL, 1, NULL, NULL,
                   NULL, NULL, NULL, 'none', 0, 0, 0, NULL, NULL, ?2, ?2)"#,
    )
    .bind(agent_execution_id.to_string())
    .bind(now)
    .execute(&pool)
    .await
    .unwrap();

    let read = agent_execution_runtime_facts::find_by_execution_id(&pool, agent_execution_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(read.failure_kind, Some(AgentFailureKind::Unknown));
    assert_eq!(
        read.failure_kind_raw_debug.as_deref(),
        Some("future_provider_shape")
    );
}

#[tokio::test]
async fn proposal_058_runtime_facts_quota_ledger_id_is_schema_foreign_key() {
    let pool = test_pool().await;
    let (_run_id, _stage_execution_id, agent_execution_id, _work_item_id) =
        seed_execution(&pool).await;

    let fk_rows = sqlx::query("PRAGMA foreign_key_list(agent_execution_runtime_facts)")
        .fetch_all(&pool)
        .await
        .unwrap();
    let has_quota_ledger_fk = fk_rows.iter().any(|row| {
        use sqlx::Row;
        row.get::<String, _>("from") == "quota_ledger_id"
            && row.get::<String, _>("table") == "agent_retry_budget_ledger"
            && row.get::<String, _>("to") == "id"
    });
    assert!(
        has_quota_ledger_fk,
        "agent_execution_runtime_facts.quota_ledger_id must reference agent_retry_budget_ledger(id)"
    );

    let now = Utc::now();
    let mut facts = AgentExecutionRuntimeFacts::defaults_for(agent_execution_id, now);
    facts.quota_ledger_id = Some("missing-ledger-id".into());
    let error = agent_execution_runtime_facts::upsert(&pool, &facts)
        .await
        .expect_err("dangling quota_ledger_id should violate the FK");
    assert!(
        error.to_string().contains("FOREIGN KEY") || error.to_string().contains("foreign key"),
        "unexpected FK error: {error}"
    );
}

#[tokio::test]
async fn proposal_058_quota_ledger_idempotency_key_is_execution_scoped() {
    let pool = test_pool().await;
    let (run_id, stage_execution_id, agent_execution_id, _work_item_id) =
        seed_execution(&pool).await;
    let retry_after = Utc::now() + chrono::Duration::minutes(30);

    let first = agent_retry_budget_ledger::upsert_quota_failure(
        &pool,
        run_id,
        stage_execution_id,
        agent_execution_id,
        Some(retry_after),
    )
    .await
    .unwrap();
    let second = agent_retry_budget_ledger::upsert_quota_failure(
        &pool,
        run_id,
        stage_execution_id,
        agent_execution_id,
        Some(retry_after),
    )
    .await
    .unwrap();

    assert_eq!(first.id, second.id);
    assert_eq!(first.failure_kind, AgentFailureKind::ProviderQuota);
    assert_eq!(first.retry_after, Some(retry_after));
    assert_eq!(first.normal_budget_consumed, false);
    assert_eq!(first.state, "waiting_for_reset");
    assert!(first.idempotency_key.contains(&run_id.to_string()));
    assert!(first
        .idempotency_key
        .contains(&stage_execution_id.to_string()));
    assert!(first
        .idempotency_key
        .contains(&agent_execution_id.to_string()));
    assert!(first.idempotency_key.contains("provider_quota"));
}

#[tokio::test]
async fn proposal_058_retry_enqueue_pending_supersession_blocks_late_import() {
    let pool = test_pool().await;
    let (run_id, stage_execution_id, agent_execution_id, source_work_item_id) =
        seed_execution(&pool).await;
    let key = ArtifactSourceGenerationClaimKey {
        run_id,
        owner_kind: OwnerKind::StageExecution,
        owner_id: stage_execution_id.to_string(),
        stage_execution_id: Some(stage_execution_id),
        agent_execution_id,
        source_work_item_id: source_work_item_id.clone(),
    };
    artifact_contracts::insert_source_generation_claim(
        &pool,
        ArtifactSourceGenerationClaim {
            key: key.clone(),
            current_session_generation_id: Some("generation-1".into()),
            claim_state: ArtifactSourceClaimState::Active,
            superseding_work_item_id: None,
            superseded_by_agent_execution_id: None,
            supersession_journal_id: None,
            superseded_at: None,
            closed_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
    )
    .await
    .unwrap();
    let retry_work_item = WorkItem {
        id: "retry-work-item-1".into(),
        kind: WorkItemKind::InvokeAgent,
        payload_json: "{}".into(),
        status: WorkItemStatus::Pending,
        run_id: Some(run_id),
        stage_id: Some("state_1".into()),
        created_at: Utc::now(),
        scheduled_at: Utc::now(),
        attempt_count: 0,
        last_error: None,
    };
    work_items::enqueue(&pool, &retry_work_item).await.unwrap();
    artifact_contracts::mark_claim_superseded_pending_retry(
        &pool,
        &key,
        &retry_work_item.id,
        "journal-1",
    )
    .await
    .unwrap();

    let decision = artifact_contracts::import_generation_with_claim_cas(
        &pool,
        &key,
        "generation-1",
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "prepush_review_v1".into(),
            canonical_path: "review/prepush.json".into(),
            raw_path: "review/prepush.json".into(),
            raw_status: "PASS".into(),
            generation_id: "artifact-generation-old".into(),
            source_agent_execution_id: Some(agent_execution_id.to_string()),
            source_stage_execution_id: Some(stage_execution_id.to_string()),
            source_session_generation_id: Some("generation-1".into()),
            source_work_item_id: Some(source_work_item_id),
            supersedes_generation_id: None,
            output_settlement: AgentOutputSettlement::ValidOutputsFromCompletedExecution,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();

    assert_eq!(decision, SourceGenerationImportDecision::IgnoredLateOutputs);
    let active = artifact_contracts::canonical_contract_field(
        &pool,
        run_id,
        "prepush_review_report",
        "status",
    )
    .await
    .unwrap();
    assert_eq!(active, None);
    let projection = artifact_contracts::find_run_state_projection(&pool, run_id)
        .await
        .unwrap()
        .expect("ignored-late evidence rebuilds projection");
    let invalid_required = projection
        .active_index_json
        .get("invalid_required_artifacts")
        .and_then(|value| value.as_array())
        .expect("invalid required artifact evidence array");
    assert!(
        invalid_required.iter().any(|entry| {
            entry
                .get("output_settlement")
                .and_then(|value| value.as_str())
                == Some("ignored_late_outputs")
                && entry.get("raw_status").and_then(|value| value.as_str()) == Some("PASS")
        }),
        "ignored-late generation should be present in projection evidence: {invalid_required:?}"
    );
    let claim = artifact_contracts::load_source_generation_claim(&pool, &key)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        claim.claim_state,
        ArtifactSourceClaimState::SupersededPendingRetry
    );
    assert_eq!(
        claim.superseding_work_item_id.as_deref(),
        Some("retry-work-item-1")
    );
}

#[tokio::test]
async fn proposal_058_import_before_supersession_keeps_accepted_active_truth() {
    let pool = test_pool().await;
    let (run_id, stage_execution_id, agent_execution_id, source_work_item_id) =
        seed_execution(&pool).await;
    let key = ArtifactSourceGenerationClaimKey {
        run_id,
        owner_kind: OwnerKind::StageExecution,
        owner_id: stage_execution_id.to_string(),
        stage_execution_id: Some(stage_execution_id),
        agent_execution_id,
        source_work_item_id: source_work_item_id.clone(),
    };
    artifact_contracts::insert_source_generation_claim(
        &pool,
        ArtifactSourceGenerationClaim {
            key: key.clone(),
            current_session_generation_id: Some("generation-1".into()),
            claim_state: ArtifactSourceClaimState::Active,
            superseding_work_item_id: None,
            superseded_by_agent_execution_id: None,
            supersession_journal_id: None,
            superseded_at: None,
            closed_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
    )
    .await
    .unwrap();

    let decision = artifact_contracts::import_generation_with_claim_cas(
        &pool,
        &key,
        "generation-1",
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "prepush_review_v1".into(),
            canonical_path: "review/prepush.json".into(),
            raw_path: "review/prepush.json".into(),
            raw_status: "PASS".into(),
            generation_id: "accepted-before-retry".into(),
            source_agent_execution_id: Some(agent_execution_id.to_string()),
            source_stage_execution_id: Some(stage_execution_id.to_string()),
            source_session_generation_id: Some("generation-1".into()),
            source_work_item_id: Some(source_work_item_id.clone()),
            supersedes_generation_id: None,
            output_settlement: AgentOutputSettlement::ValidOutputsFromCompletedExecution,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();
    assert_eq!(decision, SourceGenerationImportDecision::Activated);

    artifact_contracts::mark_claim_superseded_pending_retry(
        &pool,
        &key,
        "retry-work-item-after-accept",
        "journal-after-accept",
    )
    .await
    .unwrap();

    let active = artifact_contracts::canonical_contract_field(
        &pool,
        run_id,
        "prepush_review_report",
        "status",
    )
    .await
    .unwrap();
    assert_eq!(active, Some(serde_json::json!("pass")));
    let claim = artifact_contracts::load_source_generation_claim(&pool, &key)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        claim.claim_state,
        ArtifactSourceClaimState::SupersededPendingRetry
    );
}

#[tokio::test]
async fn proposal_058_ignored_late_projection_and_runtime_facts_commit_or_rollback_together() {
    let pool = test_pool().await;
    let (run_id, stage_execution_id, agent_execution_id, source_work_item_id) =
        seed_execution(&pool).await;
    let key = ArtifactSourceGenerationClaimKey {
        run_id,
        owner_kind: OwnerKind::StageExecution,
        owner_id: stage_execution_id.to_string(),
        stage_execution_id: Some(stage_execution_id),
        agent_execution_id,
        source_work_item_id: source_work_item_id.clone(),
    };
    artifact_contracts::insert_source_generation_claim(
        &pool,
        ArtifactSourceGenerationClaim {
            key: key.clone(),
            current_session_generation_id: Some("generation-1".into()),
            claim_state: ArtifactSourceClaimState::SupersededPendingRetry,
            superseding_work_item_id: Some("retry-work-item-1".into()),
            superseded_by_agent_execution_id: None,
            supersession_journal_id: Some("journal-1".into()),
            superseded_at: Some(Utc::now()),
            closed_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
    )
    .await
    .unwrap();

    let input = ActiveArtifactGenerationInput {
        run_id,
        artifact_id: ArtifactId::new(),
        contract_id: "prepush_review_v1".into(),
        canonical_path: "review/prepush.json".into(),
        raw_path: "review/prepush.json".into(),
        raw_status: "PASS".into(),
        generation_id: "ignored-late-commit-or-rollback".into(),
        source_agent_execution_id: Some(agent_execution_id.to_string()),
        source_stage_execution_id: Some(stage_execution_id.to_string()),
        source_session_generation_id: Some("generation-1".into()),
        source_work_item_id: Some(source_work_item_id),
        supersedes_generation_id: None,
        output_settlement: AgentOutputSettlement::ValidOutputsFromCompletedExecution,
        partial: false,
        warnings: vec![],
    };
    let mut facts = AgentExecutionRuntimeFacts::defaults_for(agent_execution_id, Utc::now());
    facts.output_settlement = AgentOutputSettlement::IgnoredLateOutputs;
    facts.ignored_late_output_count = 1;
    facts.late_output_count = 1;

    let mut rollback_tx = pool.begin().await.unwrap();
    let decision = artifact_contracts::import_generation_with_claim_cas_tx(
        &mut rollback_tx,
        &key,
        "generation-1",
        input.clone(),
    )
    .await
    .unwrap();
    assert_eq!(decision, SourceGenerationImportDecision::IgnoredLateOutputs);
    agent_execution_runtime_facts::upsert_tx(&mut rollback_tx, &facts)
        .await
        .unwrap();
    rollback_tx.rollback().await.unwrap();

    assert!(artifact_contracts::find_run_state_projection(&pool, run_id)
        .await
        .unwrap()
        .is_none());
    assert!(
        agent_execution_runtime_facts::find_by_execution_id(&pool, agent_execution_id)
            .await
            .unwrap()
            .is_none()
    );

    let mut commit_tx = pool.begin().await.unwrap();
    let decision = artifact_contracts::import_generation_with_claim_cas_tx(
        &mut commit_tx,
        &key,
        "generation-1",
        input,
    )
    .await
    .unwrap();
    assert_eq!(decision, SourceGenerationImportDecision::IgnoredLateOutputs);
    agent_execution_runtime_facts::upsert_tx(&mut commit_tx, &facts)
        .await
        .unwrap();
    commit_tx.commit().await.unwrap();

    let projection = artifact_contracts::find_run_state_projection(&pool, run_id)
        .await
        .unwrap()
        .expect("ignored-late projection commits with runtime facts");
    assert!(projection
        .active_index_json
        .get("invalid_required_artifacts")
        .and_then(|value| value.as_array())
        .unwrap()
        .iter()
        .any(|entry| entry
            .get("output_settlement")
            .and_then(|value| value.as_str())
            == Some("ignored_late_outputs")));
    let read = agent_execution_runtime_facts::find_by_execution_id(&pool, agent_execution_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        read.output_settlement,
        AgentOutputSettlement::IgnoredLateOutputs
    );
    assert_eq!(read.ignored_late_output_count, 1);
}

#[tokio::test]
async fn proposal_058_import_cas_and_runtime_facts_share_transaction_boundary() {
    let pool = test_pool().await;
    let (run_id, stage_execution_id, agent_execution_id, source_work_item_id) =
        seed_execution(&pool).await;
    let key = ArtifactSourceGenerationClaimKey {
        run_id,
        owner_kind: OwnerKind::StageExecution,
        owner_id: stage_execution_id.to_string(),
        stage_execution_id: Some(stage_execution_id),
        agent_execution_id,
        source_work_item_id: source_work_item_id.clone(),
    };
    artifact_contracts::insert_source_generation_claim(
        &pool,
        ArtifactSourceGenerationClaim {
            key: key.clone(),
            current_session_generation_id: Some("generation-1".into()),
            claim_state: ArtifactSourceClaimState::Active,
            superseding_work_item_id: None,
            superseded_by_agent_execution_id: None,
            supersession_journal_id: None,
            superseded_at: None,
            closed_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
    )
    .await
    .unwrap();

    let artifact_id = ArtifactId::new();
    let artifact = Artifact {
        id: artifact_id,
        run_id,
        stage_id: "state_1".into(),
        agent_id: "code_writer".into(),
        name: "prepush_review_report".into(),
        contract_id: "prepush_review_v1".into(),
        format: ArtifactFormat::Json,
        file_path: "review/prepush.json".into(),
        checksum_sha256: None,
        size_bytes: Some(128),
        provider: "claude".into(),
        model: Some("sonnet".into()),
        created_at: Utc::now(),
        is_pinned: false,
        report_kind: None,
        report_version: None,
        agent_execution_id: None,
    };
    let input = ActiveArtifactGenerationInput {
        run_id,
        artifact_id,
        contract_id: "prepush_review_v1".into(),
        canonical_path: "review/prepush.json".into(),
        raw_path: "review/prepush.json".into(),
        raw_status: "PASS".into(),
        generation_id: "artifact-generation-commit-or-rollback".into(),
        source_agent_execution_id: Some(agent_execution_id.to_string()),
        source_stage_execution_id: Some(stage_execution_id.to_string()),
        source_session_generation_id: Some("generation-1".into()),
        source_work_item_id: Some(source_work_item_id),
        supersedes_generation_id: None,
        output_settlement: AgentOutputSettlement::ValidOutputsFromCompletedExecution,
        partial: false,
        warnings: vec![],
    };
    let mut facts = AgentExecutionRuntimeFacts::defaults_for(agent_execution_id, Utc::now());
    facts.output_settlement = AgentOutputSettlement::ValidOutputsFromCompletedExecution;
    facts.valid_required_outputs = true;

    let mut rollback_tx = pool.begin().await.unwrap();
    artifacts::insert_tx(&mut rollback_tx, &artifact)
        .await
        .unwrap();
    let decision = artifact_contracts::import_generation_with_claim_cas_tx(
        &mut rollback_tx,
        &key,
        "generation-1",
        input.clone(),
    )
    .await
    .unwrap();
    assert_eq!(decision, SourceGenerationImportDecision::Activated);
    agent_execution_runtime_facts::upsert_tx(&mut rollback_tx, &facts)
        .await
        .unwrap();
    artifact_contracts::close_source_generation_claim_tx(&mut rollback_tx, &key)
        .await
        .unwrap();
    rollback_tx.rollback().await.unwrap();

    assert_eq!(
        artifact_contracts::canonical_contract_field(
            &pool,
            run_id,
            "prepush_review_report",
            "status"
        )
        .await
        .unwrap(),
        None
    );
    assert!(
        agent_execution_runtime_facts::find_by_execution_id(&pool, agent_execution_id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        artifacts::find_by_id(&pool, artifact_id)
            .await
            .unwrap()
            .is_none(),
        "declared artifact row must roll back with contract import"
    );
    let claim = artifact_contracts::load_source_generation_claim(&pool, &key)
        .await
        .unwrap()
        .expect("claim should survive rolled-back import");
    assert_eq!(claim.claim_state, ArtifactSourceClaimState::Active);

    let mut commit_tx = pool.begin().await.unwrap();
    artifacts::insert_tx(&mut commit_tx, &artifact)
        .await
        .unwrap();
    let decision = artifact_contracts::import_generation_with_claim_cas_tx(
        &mut commit_tx,
        &key,
        "generation-1",
        input,
    )
    .await
    .unwrap();
    assert_eq!(decision, SourceGenerationImportDecision::Activated);
    agent_execution_runtime_facts::upsert_tx(&mut commit_tx, &facts)
        .await
        .unwrap();
    artifact_contracts::close_source_generation_claim_tx(&mut commit_tx, &key)
        .await
        .unwrap();
    commit_tx.commit().await.unwrap();
    artifact_contracts::export_projection_files(&pool, run_id)
        .await
        .unwrap();

    assert_eq!(
        artifact_contracts::canonical_contract_field(
            &pool,
            run_id,
            "prepush_review_report",
            "status"
        )
        .await
        .unwrap(),
        Some(serde_json::json!("pass"))
    );
    let read = agent_execution_runtime_facts::find_by_execution_id(&pool, agent_execution_id)
        .await
        .unwrap()
        .unwrap();
    assert!(read.valid_required_outputs);
    let artifact_read = artifacts::find_by_id(&pool, artifact_id)
        .await
        .unwrap()
        .expect("declared artifact row should commit with accepted import");
    assert_eq!(artifact_read.contract_id, "prepush_review_v1");
    let claim = artifact_contracts::load_source_generation_claim(&pool, &key)
        .await
        .unwrap()
        .expect("claim should commit closed with accepted import");
    assert_eq!(claim.claim_state, ArtifactSourceClaimState::Closed);
}
