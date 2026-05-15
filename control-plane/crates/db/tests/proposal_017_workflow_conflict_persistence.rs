use chrono::{Duration, Utc};
use db::pool::create_pool;
use db::repos::{ideas, runs, stages, workflow_conflicts};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{IdeaId, RunId, StageExecutionId};
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageStatus};
use domain::workflow_conflict::{
    candidate_transition_hash, workflow_conflict_fingerprint, AdvisoryHintExtraction,
    CandidateTransitionEvaluation, CandidateTransitionResult, WorkflowAdvisoryRejectionRecord,
    WorkflowConflictReason, WorkflowConflictRecord, WorkflowConflictStatus,
    WorkflowTransitionCursorRecord,
};

async fn test_pool() -> sqlx::SqlitePool {
    let pool = create_pool("sqlite::memory:")
        .await
        .expect("in-memory pool failed");
    let writer = std::sync::Arc::new(db::writer::DbWriter::new(pool.clone()));
    db::writer::register_shared_writer(&pool, writer)
        .await
        .expect("test shared DbWriter registration failed");
    pool
}

async fn seed_run_and_stage(pool: &sqlx::SqlitePool) -> (RunId, StageExecutionId) {
    let now = Utc::now();
    let idea = Idea {
        id: IdeaId::new(),
        title: "P017 idea".into(),
        body: "body".into(),
        workspace_root_path: None,
        project_key: None,
        status: IdeaStatus::Active,
        created_at: now,
        archived_at: None,
    };
    ideas::insert(pool, &idea).await.unwrap();

    let run = Run {
        id: RunId::new(),
        idea_id: idea.id,
        status: RunStatus::Running,
        workflow_id: "workflow-conflict-test-workflow".into(),
        workflow_title: "Workflow Conflict Test Workflow".into(),
        workspace_root: "/tmp/ws".into(),
        artifact_root: "/tmp/artifacts".into(),
        started_at: now,
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        cancellation_settlement_log: None,
        current_state: Some("state_4_proposal_reviewed".into()),
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
    };
    runs::insert(pool, &run).await.unwrap();

    let stage = StageExecution {
        id: StageExecutionId::new(),
        run_id: run.id,
        stage_id: "state_4_proposal_reviewed".into(),
        label: "Proposal reviewed".into(),
        status: StageStatus::Blocked,
        iteration: 1,
        attempt_number: 1,
        settlement_kind: None,
        started_at: now,
        completed_at: None,
        owner_agent: Some("proposal_reviewer".into()),
        provider: Some("codex".into()),
        model: Some("test-model".into()),
        stage_type: None,
        validation_failure_json: None,
        evidence_packet_json: None,
        recovery_snapshot_json: None,
        retry_reason: None,
    };
    stages::insert(pool, &stage).await.unwrap();
    (run.id, stage.id)
}

fn candidate(result: CandidateTransitionResult) -> CandidateTransitionEvaluation {
    CandidateTransitionEvaluation {
        transition_id: "review_failed_to_refinement".into(),
        from_state_id: "state_4_proposal_reviewed".into(),
        to_state_id: "state_5_proposal_refined".into(),
        condition_expression_id: Some("proposal_review_summary.pass_false".into()),
        result,
        required_artifacts: vec!["proposal_review_summary".into()],
        missing_artifacts: vec![],
        missing_fields: vec![],
        source_artifact_ids: vec!["proposal_review_summary:latest".into()],
        source_agent_execution_id: Some("agent-exec-review-aggregate".into()),
        sanitized_diagnostic: None,
    }
}

fn conflict_record(
    run_id: RunId,
    stage_execution_id: StageExecutionId,
    conflict_id: &str,
    operator_label: &str,
    status: WorkflowConflictStatus,
    updated_at_offset_seconds: i64,
) -> WorkflowConflictRecord {
    let candidates = vec![candidate(CandidateTransitionResult::NotMatched)];
    let candidate_hash = candidate_transition_hash(&candidates);
    let advisory_evidence_refs = vec!["run_state_projection.next_stage".into()];
    let reason = WorkflowConflictReason::NoDeclarativeTransitionMatched;
    let now = Utc::now() + Duration::seconds(updated_at_offset_seconds);
    WorkflowConflictRecord {
        conflict_id: conflict_id.into(),
        conflict_fingerprint: workflow_conflict_fingerprint(
            &run_id.to_string(),
            "state_4_proposal_reviewed",
            &reason,
            &candidate_hash,
            &advisory_evidence_refs,
        ),
        run_id: run_id.to_string(),
        stage_execution_id: Some(stage_execution_id.to_string()),
        lineage_id: Some(stage_execution_id.to_string()),
        current_state_id: "state_4_proposal_reviewed".into(),
        reason,
        operator_label: operator_label.into(),
        status,
        candidate_transitions: candidates,
        candidate_transition_hash: candidate_hash,
        advisory_evidence_refs,
        lead_agent_id: None,
        mediation_record_id: None,
        created_at: now,
        updated_at: now,
        resolved_at: None,
        superseded_by_conflict_id: None,
        resolution_record_json: None,
        terminal_failure_reason: None,
        diagnostic_redaction_tier: "operator_safe".into(),
    }
}

fn advisory_rejection_record(
    run_id: RunId,
    stage_execution_id: StageExecutionId,
) -> WorkflowAdvisoryRejectionRecord {
    WorkflowAdvisoryRejectionRecord {
        rejection_id: "rejection-p017-1".into(),
        run_id: run_id.to_string(),
        stage_execution_id: Some(stage_execution_id.to_string()),
        lineage_id: Some(stage_execution_id.to_string()),
        current_state_id: "state_4_proposal_reviewed".into(),
        selected_transition_id: "review_failed_to_refinement".into(),
        selected_next_state_id: "state_5_proposal_refined".into(),
        advisory_next_stage_hint: Some("state_3_proposal_drafted".into()),
        advisory_next_action: Some("revise_proposal".into()),
        advisory_hint_hash: "sha256:advisory".into(),
        advisory_hint_provenance: vec![AdvisoryHintExtraction {
            source_artifact_id: "proposal_review_summary:latest".into(),
            source_agent_execution_id: Some("agent-exec-review-aggregate".into()),
            advisory_path: "$.next_stage".into(),
            raw_value_hash: "sha256:raw".into(),
            redacted_value: Some("state_3_proposal_drafted".into()),
            graph_membership_result: "absent_from_graph".into(),
            superseded_by_projection: false,
            included_in_candidate_transition_hash: true,
        }],
        graph_membership_result: "absent_from_graph".into(),
        created_at: Utc::now(),
    }
}

#[tokio::test]
async fn p017_workflow_conflict_upsert_is_stable_by_fingerprint() {
    let pool = test_pool().await;
    let (run_id, stage_execution_id) = seed_run_and_stage(&pool).await;
    let first = conflict_record(
        run_id,
        stage_execution_id,
        "conflict-original",
        "No declarative transition matched",
        WorkflowConflictStatus::Unresolved,
        0,
    );
    let second = conflict_record(
        run_id,
        stage_execution_id,
        "conflict-duplicate",
        "No declarative transition matched on re-evaluation",
        WorkflowConflictStatus::LeadMediationPending,
        30,
    );

    let inserted = workflow_conflicts::upsert_conflict_by_fingerprint(&pool, &first)
        .await
        .unwrap();
    let updated = workflow_conflicts::upsert_conflict_by_fingerprint(&pool, &second)
        .await
        .unwrap();
    let history = workflow_conflicts::list_conflict_history_for_run(&pool, run_id)
        .await
        .unwrap();

    assert_eq!(inserted.conflict_id, "conflict-original");
    assert_eq!(updated.conflict_id, "conflict-original");
    assert_eq!(updated.created_at, inserted.created_at);
    assert_eq!(
        updated.operator_label,
        "No declarative transition matched on re-evaluation"
    );
    assert_eq!(history.len(), 1);
}

#[tokio::test]
async fn p017_new_current_state_conflict_supersedes_prior_blocking_conflict() {
    let pool = test_pool().await;
    let (run_id, stage_execution_id) = seed_run_and_stage(&pool).await;
    let mut first = conflict_record(
        run_id,
        stage_execution_id,
        "conflict-first",
        "First current-state conflict",
        WorkflowConflictStatus::Unresolved,
        0,
    );
    first.conflict_fingerprint = "sha256:p017-first-current-conflict".into();
    let mut second = conflict_record(
        run_id,
        stage_execution_id,
        "conflict-second",
        "Second current-state conflict",
        WorkflowConflictStatus::Unresolved,
        30,
    );
    second.conflict_fingerprint = "sha256:p017-second-current-conflict".into();

    workflow_conflicts::upsert_conflict_by_fingerprint(&pool, &first)
        .await
        .unwrap();
    let stored_second = workflow_conflicts::upsert_conflict_by_fingerprint(&pool, &second)
        .await
        .unwrap();
    let history = workflow_conflicts::list_conflict_history_for_run(&pool, run_id)
        .await
        .unwrap();
    let current = workflow_conflicts::get_current_blocking_conflict(&pool, run_id)
        .await
        .unwrap()
        .expect("newer conflict should remain current");

    assert_eq!(history.len(), 2);
    assert_eq!(history[0].conflict_id, "conflict-first");
    assert_eq!(history[0].status, WorkflowConflictStatus::Superseded);
    assert_eq!(
        history[0].superseded_by_conflict_id.as_deref(),
        Some("conflict-second")
    );
    assert_eq!(history[1].conflict_id, "conflict-second");
    assert_eq!(history[1].status, WorkflowConflictStatus::Unresolved);
    assert_eq!(current.conflict_id, stored_second.conflict_id);
}

#[tokio::test]
async fn p017_current_blocking_conflict_ignores_resolved_status() {
    let pool = test_pool().await;
    let (run_id, stage_execution_id) = seed_run_and_stage(&pool).await;
    let conflict = conflict_record(
        run_id,
        stage_execution_id,
        "conflict-current",
        "No declarative transition matched",
        WorkflowConflictStatus::Unresolved,
        0,
    );
    let stored = workflow_conflicts::upsert_conflict_by_fingerprint(&pool, &conflict)
        .await
        .unwrap();
    assert_eq!(
        workflow_conflicts::get_current_blocking_conflict(&pool, run_id)
            .await
            .unwrap()
            .unwrap()
            .conflict_id,
        stored.conflict_id
    );

    workflow_conflicts::transition_conflict_status(
        &pool,
        &stored.conflict_id,
        WorkflowConflictStatus::Resolved,
        Utc::now(),
        Some(serde_json::json!({"selected_transition_id": "review_failed_to_refinement"})),
        None,
        None,
    )
    .await
    .unwrap();

    assert!(
        workflow_conflicts::get_current_blocking_conflict(&pool, run_id)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn p017_terminal_unverifiable_sets_resolved_at_for_terminal_conflicts() {
    let pool = test_pool().await;
    let (run_id, stage_execution_id) = seed_run_and_stage(&pool).await;
    let conflict = conflict_record(
        run_id,
        stage_execution_id,
        "conflict-terminal",
        "Workflow conflict became terminal",
        WorkflowConflictStatus::Unresolved,
        0,
    );
    let stored = workflow_conflicts::upsert_conflict_by_fingerprint(&pool, &conflict)
        .await
        .unwrap();
    let transitioned_at = Utc::now();

    let updated = workflow_conflicts::transition_conflict_status(
        &pool,
        &stored.conflict_id,
        WorkflowConflictStatus::TerminalUnverifiable,
        transitioned_at,
        None,
        Some("terminal_unverifiable".into()),
        None,
    )
    .await
    .unwrap();

    assert_eq!(updated.status, WorkflowConflictStatus::TerminalUnverifiable);
    assert_eq!(updated.resolved_at, Some(transitioned_at));
    assert!(
        workflow_conflicts::get_current_blocking_conflict(&pool, run_id)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn p017_conflict_insert_rejects_invalid_redaction_tier() {
    let pool = test_pool().await;
    let (run_id, stage_execution_id) = seed_run_and_stage(&pool).await;
    let mut conflict = conflict_record(
        run_id,
        stage_execution_id,
        "conflict-invalid-tier",
        "Bad diagnostic tier",
        WorkflowConflictStatus::Unresolved,
        0,
    );
    conflict.diagnostic_redaction_tier = "DEBUG".into();

    let error = workflow_conflicts::upsert_conflict_by_fingerprint(&pool, &conflict)
        .await
        .expect_err("invalid redaction tier should fail schema validation");

    let error_chain = format!("{error:#}");
    assert!(
        error_chain.contains("CHECK constraint failed"),
        "unexpected error: {error_chain}"
    );
}

#[tokio::test]
async fn p017_advisory_rejection_persists_selected_graph_transition() {
    let pool = test_pool().await;
    let (run_id, stage_execution_id) = seed_run_and_stage(&pool).await;
    let rejection = advisory_rejection_record(run_id, stage_execution_id);

    workflow_conflicts::insert_advisory_rejection(&pool, &rejection)
        .await
        .unwrap();
    let stored = workflow_conflicts::list_advisory_rejections_for_run(&pool, run_id)
        .await
        .unwrap();

    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].selected_next_state_id, "state_5_proposal_refined");
    assert_eq!(
        stored[0].advisory_next_stage_hint.as_deref(),
        Some("state_3_proposal_drafted")
    );
    assert_eq!(stored[0].graph_membership_result, "absent_from_graph");
}

#[tokio::test]
async fn p017_workflow_conflict_resolution_records_operator_feedback_metrics() {
    let pool = test_pool().await;
    let (run_id, stage_execution_id) = seed_run_and_stage(&pool).await;
    let conflict = conflict_record(
        run_id,
        stage_execution_id,
        "conflict-metrics",
        "No declarative transition matched",
        WorkflowConflictStatus::Unresolved,
        -90,
    );
    let stored = workflow_conflicts::upsert_conflict_by_fingerprint(&pool, &conflict)
        .await
        .unwrap();
    let resolved_at = Utc::now();

    let mut tx = pool.begin().await.unwrap();
    let resolved = workflow_conflicts::transition_conflict_status_tx(
        &mut tx,
        &stored.conflict_id,
        WorkflowConflictStatus::Resolved,
        resolved_at,
        Some(serde_json::json!({
            "resolution_kind": "operator_selected_candidate_transition",
            "action_class": "operator_selected_candidate_transition",
        })),
        None,
        None,
    )
    .await
    .unwrap();
    workflow_conflicts::record_recovery_action_chosen_tx(
        &mut tx,
        &resolved,
        "operator_selected_candidate_transition",
        "mcp",
        "accepted",
        resolved_at,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let events = workflow_conflicts::list_metric_events_for_run(&pool, run_id)
        .await
        .unwrap();
    let names: Vec<&str> = events
        .iter()
        .map(|event| event.metric_name.as_str())
        .collect();

    assert!(names.contains(&"workflow_conflict_time_to_resolution_seconds"));
    assert!(names.contains(&"conflict_reason_to_action_outcome_total"));
    assert!(names.contains(&"recovery_action_chosen_total"));
    assert!(events.iter().any(|event| {
        event.metric_name == "recovery_action_chosen_total"
            && event.labels_json["conflict_reason"] == "no_declarative_transition_matched"
            && event.labels_json["action_class"] == "operator_selected_candidate_transition"
            && event.labels_json["source_surface"] == "mcp"
    }));
}

/// P017 R2 / OPS-001: phase_c_validation_outcome_total has at least one
/// production caller and inserts a metric_event with the right labels.
#[tokio::test]
async fn p017_phase_c_validation_outcome_metric_emits() {
    let pool = test_pool().await;
    let (run_id, _stage_execution_id) = seed_run_and_stage(&pool).await;
    let now = Utc::now();

    let mut tx = pool.begin().await.unwrap();
    workflow_conflicts::record_phase_c_validation_outcome_tx(
        &mut tx, run_id, "pass", "compile", now,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let events = workflow_conflicts::list_metric_events_for_run(&pool, run_id)
        .await
        .unwrap();
    assert!(events.iter().any(|event| {
        event.metric_name == "phase_c_validation_outcome_total"
            && event.labels_json["outcome"] == "pass"
            && event.labels_json["source"] == "compile"
    }));
}

/// P017 R2 / OPS-001: lead_mediation_attempt_total emits with the
/// per-attempt labels the audit asked for (result, mediation id,
/// attempt number, lead agent id).
#[tokio::test]
async fn p017_lead_mediation_attempt_metric_emits() {
    let pool = test_pool().await;
    let (run_id, _stage_execution_id) = seed_run_and_stage(&pool).await;
    let now = Utc::now();

    let mut tx = pool.begin().await.unwrap();
    workflow_conflicts::record_lead_mediation_attempt_tx(
        &mut tx,
        &run_id.to_string(),
        Some("conflict-X"),
        "med-attempt-1",
        "lead-agent-1",
        "validated_awaiting_confirmation",
        2,
        now,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let events = workflow_conflicts::list_metric_events_for_run(&pool, run_id)
        .await
        .unwrap();
    let attempt_event = events
        .iter()
        .find(|event| event.metric_name == "lead_mediation_attempt_total")
        .expect("lead_mediation_attempt_total metric event must exist");
    assert_eq!(attempt_event.conflict_id.as_deref(), Some("conflict-X"));
    assert_eq!(
        attempt_event.labels_json["result"],
        "validated_awaiting_confirmation"
    );
    assert_eq!(
        attempt_event.labels_json["mediation_record_id"],
        "med-attempt-1"
    );
    assert_eq!(attempt_event.labels_json["attempt_number"], 2);
    assert_eq!(attempt_event.labels_json["lead_agent_id"], "lead-agent-1");
}

/// P017 R2 / OPS-001: external_catalog_warning_total emits with typed
/// warning kind, decision, and source surface labels.
#[tokio::test]
async fn p017_external_catalog_warning_metric_emits() {
    let pool = test_pool().await;
    let (run_id, _stage_execution_id) = seed_run_and_stage(&pool).await;
    let now = Utc::now();

    let mut tx = pool.begin().await.unwrap();
    workflow_conflicts::record_external_catalog_warning_tx(
        &mut tx,
        &run_id.to_string(),
        "P017_PHASE_C_EXTERNAL_CATALOG_UNDISCOVERED",
        "enabled",
        "legacy_discovery_override",
        now,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let events = workflow_conflicts::list_metric_events_for_run(&pool, run_id)
        .await
        .unwrap();
    let warning_event = events
        .iter()
        .find(|event| event.metric_name == "external_catalog_warning_total")
        .expect("external_catalog_warning_total metric event must exist");
    assert_eq!(
        warning_event.labels_json["warning_kind"],
        "P017_PHASE_C_EXTERNAL_CATALOG_UNDISCOVERED"
    );
    assert_eq!(warning_event.labels_json["decision"], "enabled");
    assert_eq!(
        warning_event.labels_json["source_surface"],
        "legacy_discovery_override"
    );
}

/// P017 R4 / OPS-002: Phase C compile failure path emits with NULL run_id.
#[tokio::test]
async fn p017_phase_c_validation_failure_metric_emits_without_run() {
    let pool = test_pool().await;
    let now = Utc::now();
    workflow_conflicts::record_phase_c_validation_failure(
        &pool,
        "lead_missing",
        Some("examples/workflows/test.yaml"),
        Some("examples/agents/test.yaml"),
        now,
    )
    .await
    .unwrap();

    // The metric exists with NULL run_id — query directly via SQL since
    // list_metric_events_for_run is run-scoped.
    let row: (String, Option<String>, String) = sqlx::query_as(
        "SELECT metric_name, run_id, labels_json
         FROM workflow_conflict_metric_events
         WHERE metric_name = 'phase_c_validation_outcome_total'
           AND labels_json LIKE '%fail%'
         LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, "phase_c_validation_outcome_total");
    assert!(row.1.is_none(), "fail-path event must have NULL run_id");
    assert!(row.2.contains("\"outcome\":\"fail\""));
    assert!(row.2.contains("lead_missing"));
}

/// P017 R4 / OPS-002: duplicate_mediation_session_total emits when a
/// resume / orchestrator replay finds an active mediation already exists.
#[tokio::test]
async fn p017_duplicate_mediation_session_metric_emits() {
    let pool = test_pool().await;
    let (run_id, _stage_execution_id) = seed_run_and_stage(&pool).await;
    let now = Utc::now();
    let mut tx = pool.begin().await.unwrap();
    workflow_conflicts::record_duplicate_mediation_session_tx(
        &mut tx,
        &run_id.to_string(),
        "conflict-X",
        "med-X",
        "try_initiate",
        now,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let events = workflow_conflicts::list_metric_events_for_run(&pool, run_id)
        .await
        .unwrap();
    let event = events
        .iter()
        .find(|e| e.metric_name == "duplicate_mediation_session_total")
        .expect("duplicate_mediation_session_total must exist");
    assert_eq!(event.conflict_id.as_deref(), Some("conflict-X"));
    assert_eq!(event.labels_json["mediation_record_id"], "med-X");
    assert_eq!(event.labels_json["detection_source"], "try_initiate");
}

/// P017 R4 / OPS-002: report_readback_completeness emits a ratio of
/// expected→present fields with the correct labels.
#[tokio::test]
async fn p017_report_readback_completeness_metric_emits() {
    let pool = test_pool().await;
    let (run_id, _stage_execution_id) = seed_run_and_stage(&pool).await;
    let now = Utc::now();
    let expected = ["a", "b", "c", "d"];
    let present = ["a", "b", "c"];
    let mut tx = pool.begin().await.unwrap();
    workflow_conflicts::record_report_readback_completeness_tx(
        &mut tx,
        &run_id.to_string(),
        Some("conflict-Y"),
        &expected,
        &present,
        "mcp.workflow_conflict_json",
        now,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let events = workflow_conflicts::list_metric_events_for_run(&pool, run_id)
        .await
        .unwrap();
    let event = events
        .iter()
        .find(|e| e.metric_name == "report_readback_completeness")
        .expect("report_readback_completeness must exist");
    assert_eq!(event.unit, "ratio");
    assert!(
        (event.value - 0.75).abs() < 1e-6,
        "ratio should be 3/4 = 0.75; got {}",
        event.value
    );
    assert_eq!(event.labels_json["surface"], "mcp.workflow_conflict_json");
}

/// P017 R4 / OPS-002: phase_c_lead_inventory_external_catalog_total
/// emits with the inventory_result + enforcement_decision labels.
#[tokio::test]
async fn p017_phase_c_lead_inventory_external_catalog_metric_emits() {
    let pool = test_pool().await;
    let (run_id, _stage_execution_id) = seed_run_and_stage(&pool).await;
    let now = Utc::now();
    let mut tx = pool.begin().await.unwrap();
    workflow_conflicts::record_phase_c_lead_inventory_external_catalog_tx(
        &mut tx,
        Some(&run_id.to_string()),
        "zero_active_externals",
        "waive_warning_window",
        Some("examples/agents/test.yaml"),
        now,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let events = workflow_conflicts::list_metric_events_for_run(&pool, run_id)
        .await
        .unwrap();
    let event = events
        .iter()
        .find(|e| e.metric_name == "phase_c_lead_inventory_external_catalog_total")
        .expect("phase_c_lead_inventory_external_catalog_total must exist");
    assert_eq!(
        event.labels_json["inventory_result"],
        "zero_active_externals"
    );
    assert_eq!(
        event.labels_json["enforcement_decision"],
        "waive_warning_window"
    );
}

/// P017 R4 / OPS-002: mediation_late_output_ignored_total emits with
/// the per-mediation reason label.
#[tokio::test]
async fn p017_mediation_late_output_ignored_metric_emits() {
    let pool = test_pool().await;
    let (run_id, _stage_execution_id) = seed_run_and_stage(&pool).await;
    let now = Utc::now();
    let mut tx = pool.begin().await.unwrap();
    workflow_conflicts::record_mediation_late_output_ignored_tx(
        &mut tx,
        &run_id.to_string(),
        Some("conflict-Z"),
        "med-Z",
        "mediation_terminal_or_missing",
        now,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let events = workflow_conflicts::list_metric_events_for_run(&pool, run_id)
        .await
        .unwrap();
    let event = events
        .iter()
        .find(|e| e.metric_name == "mediation_late_output_ignored_total")
        .expect("mediation_late_output_ignored_total must exist");
    assert_eq!(event.conflict_id.as_deref(), Some("conflict-Z"));
    assert_eq!(event.labels_json["mediation_record_id"], "med-Z");
    assert_eq!(event.labels_json["reason"], "mediation_terminal_or_missing");
}

/// P017 R6 / OPS-001: mediation_retry_budget_exhausted_total must have
/// a focused metric-emission test, not only a helper definition.
#[tokio::test]
async fn p017_mediation_retry_budget_exhausted_metric_emits() {
    let pool = test_pool().await;
    let (run_id, _stage_execution_id) = seed_run_and_stage(&pool).await;
    let now = Utc::now();
    let mut tx = pool.begin().await.unwrap();
    workflow_conflicts::record_mediation_retry_budget_exhausted_tx(
        &mut tx,
        &run_id.to_string(),
        "mediation-1",
        Some("codex-default"),
        "provider_quota",
        now,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let events = workflow_conflicts::list_metric_events_for_run(&pool, run_id)
        .await
        .unwrap();
    let event = events
        .iter()
        .find(|e| e.metric_name == "mediation_retry_budget_exhausted_total")
        .expect("mediation_retry_budget_exhausted_total must exist");
    assert_eq!(event.unit, "count");
    assert_eq!(event.labels_json["mediation_record_id"], "mediation-1");
    assert_eq!(event.labels_json["provider_profile_id"], "codex-default");
    assert_eq!(event.labels_json["conflict_reason"], "provider_quota");
}

/// P017 R6 / OPS-001: phase_b_dogfood_mediation_completion_rate
/// must be emitted as a runtime metric event with workflow/conflict labels.
#[tokio::test]
async fn p017_phase_b_dogfood_mediation_completion_rate_metric_emits() {
    let pool = test_pool().await;
    let (run_id, _stage_execution_id) = seed_run_and_stage(&pool).await;
    let now = Utc::now();
    let mut tx = pool.begin().await.unwrap();
    workflow_conflicts::record_phase_b_dogfood_mediation_completion_rate_tx(
        &mut tx,
        Some(&run_id.to_string()),
        "full-mvp-live",
        "same_run_continue",
        1.0,
        10,
        "phase_b_dogfood_exit_record",
        now,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let events = workflow_conflicts::list_metric_events_for_run(&pool, run_id)
        .await
        .unwrap();
    let event = events
        .iter()
        .find(|e| e.metric_name == "phase_b_dogfood_mediation_completion_rate")
        .expect("phase_b_dogfood_mediation_completion_rate must exist");
    assert_eq!(event.unit, "ratio");
    assert!((event.value - 1.0).abs() < 1e-6);
    assert_eq!(event.labels_json["workflow_id"], "full-mvp-live");
    assert_eq!(event.labels_json["conflict_reason"], "same_run_continue");
    assert_eq!(event.labels_json["sample_size"], 10);
}

/// P017 R6 / OPS-001: phase_b_dogfood_operator_guidance_sufficient_total
/// must be emitted as a runtime metric event with action/result labels.
#[tokio::test]
async fn p017_phase_b_dogfood_operator_guidance_sufficient_metric_emits() {
    let pool = test_pool().await;
    let (run_id, _stage_execution_id) = seed_run_and_stage(&pool).await;
    let now = Utc::now();
    let mut tx = pool.begin().await.unwrap();
    workflow_conflicts::record_phase_b_dogfood_operator_guidance_sufficient_tx(
        &mut tx,
        Some(&run_id.to_string()),
        "lead_mediation_guidance",
        "sufficient",
        10,
        "phase_b_dogfood_exit_record",
        now,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let events = workflow_conflicts::list_metric_events_for_run(&pool, run_id)
        .await
        .unwrap();
    let event = events
        .iter()
        .find(|e| e.metric_name == "phase_b_dogfood_operator_guidance_sufficient_total")
        .expect("phase_b_dogfood_operator_guidance_sufficient_total must exist");
    assert_eq!(event.unit, "count");
    assert_eq!(event.value, 10.0);
    assert_eq!(event.labels_json["action_class"], "lead_mediation_guidance");
    assert_eq!(event.labels_json["result"], "sufficient");
}

/// P017 R4 / API-002: persisting per-attempt cost + transcript via
/// `update_attempt_attribution` is reflected in `find_by_id`.
#[tokio::test]
async fn p017_per_attempt_cost_and_transcript_persisted() {
    let pool = test_pool().await;
    let (run_id, stage_execution_id) = seed_run_and_stage(&pool).await;
    let exec_id = domain::ids::AgentExecutionId::new();
    let exec = domain::agent::AgentExecution {
        id: exec_id,
        stage_execution_id: Some(stage_execution_id),
        agent_id: "lead-agent".into(),
        provider: "claude".into(),
        model: None,
        started_at: Utc::now(),
        completed_at: None,
        status: domain::agent::AgentStatus::Running,
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
    };
    let _ = run_id; // run association via stage
    db::repos::agent_executions::insert(&pool, &exec)
        .await
        .unwrap();

    // First call: cost only (transcript_artifact_id stays None — no
    // FK violation).
    db::repos::agent_executions::update_attempt_attribution(
        &pool,
        exec_id,
        Some(42),  // total_cost_cents
        Some(100), // input_tokens
        Some(25),  // output_tokens
        Some(10),  // cached_input_tokens
        None,      // transcript_artifact_id stays None
    )
    .await
    .unwrap();

    let after = db::repos::agent_executions::find_by_id(&pool, exec_id)
        .await
        .unwrap()
        .expect("execution must still exist");
    assert_eq!(after.total_cost_cents, Some(42));
    assert_eq!(after.input_tokens, Some(100));
    assert_eq!(after.output_tokens, Some(25));
    assert_eq!(after.cached_input_tokens, Some(10));
    assert!(after.transcript_artifact_id.is_none());

    // Second call: insert a real artifact + link it as the transcript.
    let artifact = domain::artifact::Artifact {
        id: domain::ids::ArtifactId::new(),
        run_id,
        stage_id: "state_test".into(),
        agent_id: exec.agent_id.clone(),
        name: "session_transcript".into(),
        contract_id: "session_transcript".into(),
        format: domain::artifact::ArtifactFormat::Markdown,
        file_path: "/tmp/transcript.md".into(),
        checksum_sha256: None,
        size_bytes: None,
        provider: exec.provider.clone(),
        model: None,
        created_at: Utc::now(),
        is_pinned: false,
        report_kind: Some("session_transcript".into()),
        report_version: Some(1),
        agent_execution_id: None,
    };
    let artifact_id = artifact.id.to_string();
    db::repos::artifacts::insert(&pool, &artifact)
        .await
        .unwrap();

    db::repos::agent_executions::update_attempt_attribution(
        &pool,
        exec_id,
        None,
        None,
        None,
        None,
        Some(&artifact_id),
    )
    .await
    .unwrap();
    let after2 = db::repos::agent_executions::find_by_id(&pool, exec_id)
        .await
        .unwrap()
        .expect("execution must still exist");
    assert_eq!(
        after2.transcript_artifact_id.as_deref(),
        Some(artifact_id.as_str())
    );
    // Cost values from the first call must still be there (COALESCE).
    assert_eq!(after2.total_cost_cents, Some(42));
}

/// P017 R5 / OPS-003: advisory_rejection_total emits per insert and
/// also emits invalid_next_stage_hint_non_blocking_total when the
/// graph_membership_result is `absent_from_graph`.
#[tokio::test]
async fn p017_advisory_rejection_metrics_emit() {
    let pool = test_pool().await;
    let (run_id, stage_execution_id) = seed_run_and_stage(&pool).await;
    let rec = advisory_rejection_record(run_id, stage_execution_id);
    workflow_conflicts::insert_advisory_rejection(&pool, &rec)
        .await
        .unwrap();

    let events = workflow_conflicts::list_metric_events_for_run(&pool, run_id)
        .await
        .unwrap();
    let names: Vec<&str> = events.iter().map(|e| e.metric_name.as_str()).collect();
    assert!(names.contains(&"advisory_rejection_total"));
    assert!(names.contains(&"invalid_next_stage_hint_non_blocking_total"));

    let advisory = events
        .iter()
        .find(|e| e.metric_name == "advisory_rejection_total")
        .unwrap();
    assert_eq!(
        advisory.labels_json["graph_membership_result"],
        "absent_from_graph"
    );

    let invalid = events
        .iter()
        .find(|e| e.metric_name == "invalid_next_stage_hint_non_blocking_total")
        .unwrap();
    assert_eq!(
        invalid.labels_json["advisory_next_action"],
        "revise_proposal"
    );
}

/// P017 R5 / OPS-003: workflow_conflict_current_total emits per
/// upsert with (reason, status) labels.
#[tokio::test]
async fn p017_workflow_conflict_current_metric_emits() {
    let pool = test_pool().await;
    let (run_id, stage_execution_id) = seed_run_and_stage(&pool).await;
    let rec = conflict_record(
        run_id,
        stage_execution_id,
        "conflict-current",
        "No declarative transition matched",
        WorkflowConflictStatus::Unresolved,
        0,
    );
    workflow_conflicts::upsert_conflict_by_fingerprint(&pool, &rec)
        .await
        .unwrap();

    let events = workflow_conflicts::list_metric_events_for_run(&pool, run_id)
        .await
        .unwrap();
    let event = events
        .iter()
        .find(|e| e.metric_name == "workflow_conflict_current_total")
        .expect("workflow_conflict_current_total must exist");
    assert_eq!(
        event.labels_json["reason"],
        "no_declarative_transition_matched"
    );
    assert_eq!(event.labels_json["status"], "unresolved");
}

/// P017 R5 / OPS-003: terminal_unverifiable_total emits when a
/// conflict transitions to TerminalUnverifiable.
#[tokio::test]
async fn p017_terminal_unverifiable_metric_emits() {
    let pool = test_pool().await;
    let (run_id, stage_execution_id) = seed_run_and_stage(&pool).await;
    let rec = conflict_record(
        run_id,
        stage_execution_id,
        "conflict-terminal",
        "No declarative transition matched",
        WorkflowConflictStatus::Unresolved,
        0,
    );
    let stored = workflow_conflicts::upsert_conflict_by_fingerprint(&pool, &rec)
        .await
        .unwrap();

    workflow_conflicts::transition_conflict_status(
        &pool,
        &stored.conflict_id,
        WorkflowConflictStatus::TerminalUnverifiable,
        Utc::now(),
        None,
        Some("operator_abandoned_after_lead_failure".into()),
        None,
    )
    .await
    .unwrap();

    let events = workflow_conflicts::list_metric_events_for_run(&pool, run_id)
        .await
        .unwrap();
    let event = events
        .iter()
        .find(|e| e.metric_name == "terminal_unverifiable_total")
        .expect("terminal_unverifiable_total must exist");
    assert_eq!(
        event.labels_json["terminal_failure_reason"],
        "operator_abandoned_after_lead_failure"
    );
}

#[tokio::test]
async fn p017_transition_cursor_upserts_run_settlement_boundary() {
    let pool = test_pool().await;
    let (run_id, _stage_execution_id) = seed_run_and_stage(&pool).await;
    let now = Utc::now();
    let first = WorkflowTransitionCursorRecord {
        schema_version: WorkflowTransitionCursorRecord::SCHEMA_VERSION.to_string(),
        run_id: run_id.to_string(),
        current_state_id: "state_4_proposal_reviewed".into(),
        cursor_status: "awaiting_conflict_resolution".into(),
        resume_policy: "await_conflict_resolution".into(),
        selected_transition_id: None,
        selected_next_state_id: None,
        conflict_id: Some("conflict-1".into()),
        conflict_fingerprint: Some("sha256:conflict".into()),
        candidate_transition_hash: Some("sha256:candidates".into()),
        terminal_failure_reason: None,
        updated_at: now,
    };
    let second = WorkflowTransitionCursorRecord {
        cursor_status: "graph_transition_selected".into(),
        resume_policy: "continue_from_selected_transition".into(),
        selected_transition_id: Some(
            "state_4_proposal_reviewed__to__state_5_proposal_refined__0".into(),
        ),
        selected_next_state_id: Some("state_5_proposal_refined".into()),
        conflict_id: None,
        conflict_fingerprint: None,
        terminal_failure_reason: None,
        updated_at: now + Duration::seconds(1),
        ..first.clone()
    };

    workflow_conflicts::upsert_transition_cursor(&pool, &first)
        .await
        .unwrap();
    workflow_conflicts::upsert_transition_cursor(&pool, &second)
        .await
        .unwrap();

    let stored = workflow_conflicts::get_transition_cursor(&pool, run_id)
        .await
        .unwrap()
        .expect("transition cursor should be readable");
    assert_eq!(stored.cursor_status, "graph_transition_selected");
    assert_eq!(stored.resume_policy, "continue_from_selected_transition");
    assert_eq!(
        stored.selected_next_state_id.as_deref(),
        Some("state_5_proposal_refined")
    );
    assert_eq!(stored.conflict_id, None);
}
