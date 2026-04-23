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
    create_pool("sqlite::memory:")
        .await
        .expect("in-memory pool failed")
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
        workflow_id: "proposal-017-workflow".into(),
        workflow_title: "Proposal 017 Workflow".into(),
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

    assert!(
        error
            .to_string()
            .contains("CHECK constraint failed"),
        "unexpected error: {error:#}"
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
