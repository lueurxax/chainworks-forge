use chrono::Utc;
use db::pool::create_pool;
use db::repos::{artifact_contracts, ideas, runs};
use domain::artifact_contracts::{ActiveArtifactGenerationInput, ArtifactContractOverrideInput};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{ArtifactId, IdeaId, RunId};
use domain::run::{Run, RunStatus};

#[test]
fn proposal_094_required_metrics_inventory_and_emitters_are_wired() {
    for metric in [
        "quality_gate_blocker_assessments_total",
        "quality_gate_blocker_validation_rejections_total",
        "quality_gate_blocker_freshness_total",
        "implementation_refine_loops_avoided_total",
        "followup_proposal_seeds_created_total",
        "external_blockers_accepted_total",
        "invalid_blocker_claims_total",
        "review_refresh_required_total",
        "output_settlement_required_before_boundary_total",
        "human_boundary_approval_latency_seconds",
        "post_boundary_reopen_total",
        "false_external_blocker_rate",
        "repeated_blocker_no_progress_total",
        "accepted_boundary_later_rejected_percent",
    ] {
        assert!(
            db::metrics::P094_REQUIRED_METRICS.contains(&metric),
            "P094 required metric missing from inventory: {metric}"
        );
    }

    db::metrics::reset_for_tests();
    db::metrics::record_p094_invalid_blocker_claim("missing_evidence");
    db::metrics::record_p094_review_refresh_required("implementation_review");
    db::metrics::record_p094_output_settlement_required_before_boundary("missing_output");
    db::metrics::record_p094_repeated_blocker_no_progress("sig-1");
    db::metrics::record_p094_implementation_refine_loop_avoided("P094");
    db::metrics::record_p094_external_blocker_accepted("remote_environment_required");
    db::metrics::record_p094_boundary_approval("granted");
    db::metrics::record_p094_false_external_blocker_rate("operator_reopened");
    db::metrics::record_p094_accepted_boundary_later_rejected("operator_rejected_boundary");
    db::metrics::record_p094_human_boundary_approval_latency(std::time::Duration::from_secs(3));

    assert_eq!(db::metrics::get_counter("invalid_blocker_claims_total"), 1);
    assert_eq!(db::metrics::get_counter("review_refresh_required_total"), 1);
    assert_eq!(
        db::metrics::get_counter("output_settlement_required_before_boundary_total"),
        1
    );
    assert_eq!(
        db::metrics::get_counter("repeated_blocker_no_progress_total"),
        1
    );
    assert_eq!(
        db::metrics::get_counter_with_label(
            "implementation_refine_loops_avoided_total",
            "proposal_id=P094"
        ),
        1
    );
    assert_eq!(
        db::metrics::get_gauge("false_external_blocker_rate"),
        Some(100)
    );
    assert_eq!(
        db::metrics::get_gauge("accepted_boundary_later_rejected_percent"),
        Some(100)
    );
    assert_eq!(
        db::metrics::get_hot_read_p95("human_boundary_approval_latency_seconds"),
        Some(3)
    );
    let rollout_metrics = db::metrics::p094_rollout_metric_values_json();
    assert_eq!(
        rollout_metrics["implementation_refine_loops_avoided_total"]["value"],
        1
    );
    assert_eq!(
        rollout_metrics["false_external_blocker_rate"]["kind"],
        "gauge"
    );
    assert_eq!(
        rollout_metrics["accepted_boundary_later_rejected_percent"]["unit"],
        "percent"
    );
}

#[tokio::test]
async fn proposal_094_followup_seed_generation_records_metric() {
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let (run_id, _) =
        seed_run_with_meta(&pool, Some(tmp.path().to_string_lossy().into_owned())).await;
    let raw_path = tmp.path().join("followup-proposal-seed.json");
    std::fs::write(
        &raw_path,
        serde_json::json!({
            "schema_version": "followup_proposal_seed_v1",
            "status": "seeded",
            "tail_class": "followup_code_tail"
        })
        .to_string(),
    )
    .unwrap();

    db::metrics::reset_for_tests();
    artifact_contracts::upsert_verified_generation_and_rebuild(
        &pool,
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "followup_proposal_seed_v1".into(),
            canonical_path: "quality-gate/followup-proposal-seed.json".into(),
            raw_path: raw_path.to_string_lossy().into_owned(),
            raw_status: "seeded".into(),
            generation_id: "p094-followup-seed-1".into(),
            source_agent_execution_id: Some("lead_orchestrator".into()),
            source_stage_execution_id: Some("state_9_followup_proposal_seeded".into()),
            source_session_generation_id: None,
            source_work_item_id: None,
            supersedes_generation_id: None,
            output_settlement:
                domain::agent::AgentOutputSettlement::ValidOutputsFromCompletedExecution,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();

    assert_eq!(
        db::metrics::get_counter("followup_proposal_seeds_created_total"),
        1
    );
    assert_eq!(
        db::metrics::get_counter_with_label(
            "followup_proposal_seeds_created_total",
            "tail_class=followup_code_tail"
        ),
        1
    );
}

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
        status: RunStatus::WaitingApproval,
        workflow_id: "wf".into(),
        workflow_title: "Workflow".into(),
        workspace_root: "/tmp/ws".into(),
        artifact_root: "/tmp/artifacts".into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        cancellation_settlement_log: None,
        current_state: Some("state_11_manual_release".into()),
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
        chainworks_meta_root: Some("/tmp/chainworks/run-1".into()),
        review_routing_json: None,
        closeout_readiness_mode: None,
    }
}

async fn seed_run(pool: &sqlx::SqlitePool) -> (RunId, IdeaId) {
    seed_run_with_meta(pool, None).await
}

async fn seed_run_with_meta(pool: &sqlx::SqlitePool, meta_root: Option<String>) -> (RunId, IdeaId) {
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
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
    let mut run = make_run(run_id, idea_id);
    run.chainworks_meta_root = meta_root;
    runs::insert(pool, &run).await.unwrap();
    (run_id, idea_id)
}

#[tokio::test]
async fn proposal_057_active_index_is_sqlite_owned_and_json_export_is_rebuildable() {
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let (run_id, _) =
        seed_run_with_meta(&pool, Some(tmp.path().to_string_lossy().into_owned())).await;

    artifact_contracts::upsert_generation_and_rebuild(
        &pool,
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "prepush_review_v1".into(),
            canonical_path: "review/prepush.json".into(),
            raw_path: "review/prepush.json".into(),
            raw_status: "PASS_WITH_NOTES".into(),
            generation_id: "agent-exec-1".into(),
            source_agent_execution_id: Some("agent-exec-1".into()),
            source_stage_execution_id: None,
            source_session_generation_id: None,
            source_work_item_id: None,
            supersedes_generation_id: None,
            output_settlement: domain::agent::AgentOutputSettlement::None,
            partial: false,
            warnings: vec!["normalized PASS_WITH_NOTES to pass".into()],
        },
    )
    .await
    .unwrap();

    let field = artifact_contracts::canonical_contract_field(
        &pool,
        run_id,
        "prepush_review_report",
        "status",
    )
    .await
    .unwrap();
    assert_eq!(field, Some(serde_json::json!("pass")));

    let projection = artifact_contracts::find_run_state_projection(&pool, run_id)
        .await
        .unwrap()
        .expect("projection row");
    assert_eq!(
        projection.active_index_json["contracts"]["prepush_review_v1"]["status"],
        "pass"
    );
    assert_eq!(
        projection.run_state_json["current_state"],
        "state_11_manual_release"
    );
    let active_index_path = tmp.path().join("artifacts/active-index.json");
    let run_state_path = tmp.path().join("state/run-state.json");
    assert!(active_index_path.exists());
    assert!(run_state_path.exists());

    std::fs::write(&active_index_path, r#"{"schema_version":"stale"}"#).unwrap();
    std::fs::write(&run_state_path, "not json").unwrap();
    artifact_contracts::rebuild_projection_and_exports(&pool, run_id)
        .await
        .unwrap();
    let exported_active: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&active_index_path).unwrap()).unwrap();
    let exported_run_state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&run_state_path).unwrap()).unwrap();
    assert_eq!(exported_active["owner"], "sqlite");
    assert_eq!(
        exported_active["contracts"]["prepush_review_v1"]["status"],
        "pass"
    );
    assert_eq!(exported_run_state["active_index_owner"], "sqlite");
}

#[tokio::test]
async fn proposal_057_repairs_legacy_invalid_contract_statuses_after_vocab_expansion() {
    let pool = test_pool().await;
    let (run_id, _) = seed_run(&pool).await;
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        r#"INSERT INTO artifact_contract_generations
           (generation_id, run_id, artifact_id, contract_id, canonical_path, raw_path, raw_status,
            canonical_status, source_agent_execution_id, source_stage_execution_id,
            source_session_generation_id, source_work_item_id, supersedes_generation_id,
            output_settlement, source_generation_verified, valid, partial, warnings_json,
            validation_errors_json, canonical_dimensions_json, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)"#,
    )
    .bind("gen-docs-aligned")
    .bind(run_id.to_string())
    .bind(ArtifactId::new().to_string())
    .bind("docs_report_v1")
    .bind("docs/report.json")
    .bind("docs/report.json")
    .bind("aligned")
    .bind("invalid")
    .bind("agent-docs")
    .bind("stage-review")
    .bind("session-1")
    .bind("work-1")
    .bind(Option::<String>::None)
    .bind("valid_outputs_from_completed_execution")
    .bind(1_i64)
    .bind(0_i64)
    .bind(0_i64)
    .bind("[]")
    .bind(r#"["unknown status value: aligned"]"#)
    .bind("{}")
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    let before =
        artifact_contracts::canonical_contract_field(&pool, run_id, "docs_report", "status")
            .await
            .unwrap();
    assert_eq!(before, None);

    let repaired =
        artifact_contracts::repair_contract_status_normalization_and_rebuild(&pool, run_id)
            .await
            .unwrap();
    assert_eq!(repaired, 1);

    let after =
        artifact_contracts::canonical_contract_field(&pool, run_id, "docs_report", "status")
            .await
            .unwrap();
    assert_eq!(after, Some(serde_json::json!("pass")));

    let projection = artifact_contracts::find_run_state_projection(&pool, run_id)
        .await
        .unwrap()
        .expect("projection row");
    assert_eq!(
        projection.active_index_json["contracts"]["docs_report_v1"]["status"],
        "pass"
    );
    assert_eq!(
        projection.active_index_json["contracts"]["docs_report_v1"]["validation_errors"],
        serde_json::json!([])
    );
}

#[tokio::test]
async fn proposal_057_implementation_and_tests_contract_fields_are_canonical() {
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let (run_id, _) =
        seed_run_with_meta(&pool, Some(tmp.path().to_string_lossy().into_owned())).await;
    let assessment_path = tmp.path().join("implementation/self-assessment.json");
    std::fs::create_dir_all(assessment_path.parent().unwrap()).unwrap();
    std::fs::write(
        &assessment_path,
        r#"{"implementation_complete":true,"verification_green":true,"remaining_code_tasks":[],"handoff_tasks":[],"known_risks":[],"tests_run":[],"docs_impacted":[]}"#,
    )
    .unwrap();

    artifact_contracts::upsert_generation_and_rebuild(
        &pool,
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "implementation_self_assessment_v2".into(),
            canonical_path: "implementation/self-assessment.json".into(),
            raw_path: assessment_path.to_string_lossy().into_owned(),
            raw_status: "complete".into(),
            generation_id: "agent-exec-impl".into(),
            source_agent_execution_id: Some("agent-exec-impl".into()),
            source_stage_execution_id: None,
            source_session_generation_id: None,
            source_work_item_id: None,
            supersedes_generation_id: None,
            output_settlement: domain::agent::AgentOutputSettlement::None,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();
    artifact_contracts::upsert_generation_and_rebuild(
        &pool,
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "tests_result_v1".into(),
            canonical_path: "implementation/tests.json".into(),
            raw_path: "implementation/tests.json".into(),
            raw_status: "green".into(),
            generation_id: "agent-exec-tests".into(),
            source_agent_execution_id: Some("agent-exec-tests".into()),
            source_stage_execution_id: None,
            source_session_generation_id: None,
            source_work_item_id: None,
            supersedes_generation_id: None,
            output_settlement: domain::agent::AgentOutputSettlement::None,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();

    let implementation_complete = artifact_contracts::canonical_contract_field(
        &pool,
        run_id,
        "implementation_self_assessment_v2",
        "implementation_complete",
    )
    .await
    .unwrap();
    assert_eq!(implementation_complete, Some(serde_json::json!(true)));
    let tests_status =
        artifact_contracts::canonical_contract_field(&pool, run_id, "tests_result_v1", "status")
            .await
            .unwrap();
    assert_eq!(tests_status, Some(serde_json::json!("green")));
}

#[tokio::test]
async fn proposal_057_operator_override_is_typed_expiring_and_visible() {
    let pool = test_pool().await;
    let (run_id, _) = seed_run(&pool).await;

    artifact_contracts::create_override_and_rebuild(
        &pool,
        ArtifactContractOverrideInput {
            run_id,
            contract_id: "audit_report_v1".into(),
            override_type: "implementation_status".into(),
            from_status: "needs_code_fixes".into(),
            to_status: "implemented".into(),
            reason: "operator verified code complete".into(),
            owner: "operator".into(),
            source_artifacts: vec!["audit/proposal-vs-implementation.json".into()],
            expires_at_stage: "state_11_manual_release".into(),
            journal_id: "journal-1".into(),
        },
    )
    .await
    .unwrap();

    let active = artifact_contracts::list_overrides(&pool, run_id)
        .await
        .unwrap();
    assert_eq!(active[0].to_status, "implemented");
    assert!(active[0].active);
    let overridden = artifact_contracts::canonical_contract_field(
        &pool,
        run_id,
        "audit_report",
        "implementation_status",
    )
    .await
    .unwrap();
    assert_eq!(overridden, Some(serde_json::json!("implemented")));

    let projection = artifact_contracts::find_run_state_projection(&pool, run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        projection.active_index_json["contracts"]["audit_report_v1"]["status"],
        "implemented"
    );
    assert_eq!(
        projection.active_index_json["contracts"]["audit_report_v1"]["status_overridden"],
        true
    );

    artifact_contracts::expire_overrides_for_stage(&pool, run_id, "state_11_manual_release")
        .await
        .unwrap();
    let expired = artifact_contracts::list_overrides(&pool, run_id)
        .await
        .unwrap();
    assert!(!expired[0].active);
    let after_expiry = artifact_contracts::canonical_contract_field(
        &pool,
        run_id,
        "audit_report",
        "implementation_status",
    )
    .await
    .unwrap();
    assert_eq!(after_expiry, None);
}

#[tokio::test]
async fn proposal_057_operator_override_rejects_untyped_status_truth() {
    let pool = test_pool().await;
    let (run_id, _) = seed_run(&pool).await;

    let result = artifact_contracts::create_override_and_rebuild(
        &pool,
        ArtifactContractOverrideInput {
            run_id,
            contract_id: "audit_report_v1".into(),
            override_type: "implementation_status".into(),
            from_status: "needs_code_fixes".into(),
            to_status: "banana".into(),
            reason: "invalid override must not become transition truth".into(),
            owner: "operator".into(),
            source_artifacts: vec!["audit/proposal-vs-implementation.json".into()],
            expires_at_stage: "state_11_manual_release".into(),
            journal_id: "journal-invalid".into(),
        },
    )
    .await;

    assert!(
        result.is_err(),
        "override target status must be validated before it can become canonical truth"
    );
}

#[tokio::test]
async fn proposal_057_audit_report_splits_implementation_and_release_evidence_truth() {
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let audit_path = tmp.path().join("audit/proposal-vs-implementation.json");
    std::fs::create_dir_all(audit_path.parent().unwrap()).unwrap();
    std::fs::write(
        &audit_path,
        r#"{"implementation_status":"Implemented","release_evidence_status":"blocked_pending_operator_evidence"}"#,
    )
    .unwrap();
    let (run_id, _) =
        seed_run_with_meta(&pool, Some(tmp.path().to_string_lossy().into_owned())).await;

    artifact_contracts::upsert_generation_and_rebuild(
        &pool,
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "audit_report_v1".into(),
            canonical_path: "audit/proposal-vs-implementation.json".into(),
            raw_path: audit_path.to_string_lossy().into_owned(),
            raw_status: "Implemented".into(),
            generation_id: "audit-agent-exec-1".into(),
            source_agent_execution_id: Some("audit-agent-exec-1".into()),
            source_stage_execution_id: None,
            source_session_generation_id: None,
            source_work_item_id: None,
            supersedes_generation_id: None,
            output_settlement:
                domain::agent::AgentOutputSettlement::ValidOutputsFromCompletedExecution,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();

    let implementation_status = artifact_contracts::canonical_contract_field(
        &pool,
        run_id,
        "audit_report",
        "implementation_status",
    )
    .await
    .unwrap();
    let release_evidence_status = artifact_contracts::canonical_contract_field(
        &pool,
        run_id,
        "audit_report",
        "release_evidence_status",
    )
    .await
    .unwrap();
    assert_eq!(
        implementation_status,
        Some(serde_json::json!("implemented"))
    );
    assert_eq!(
        release_evidence_status,
        Some(serde_json::json!("blocked_pending_operator_evidence"))
    );

    let projection = artifact_contracts::find_run_state_projection(&pool, run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        projection.active_index_json["contracts"]["audit_report_v1"]["implementation_status"],
        "implemented"
    );
    assert_eq!(
        projection.active_index_json["contracts"]["audit_report_v1"]["release_evidence_status"],
        "blocked_pending_operator_evidence"
    );

    std::fs::write(
        &audit_path,
        r#"{"implementation_status":"Implemented","release_evidence_status":"ready"}"#,
    )
    .unwrap();
    artifact_contracts::rebuild_projection_and_exports(&pool, run_id)
        .await
        .unwrap();
    let release_evidence_status_after_raw_mutation = artifact_contracts::canonical_contract_field(
        &pool,
        run_id,
        "audit_report",
        "release_evidence_status",
    )
    .await
    .unwrap();
    assert_eq!(
        release_evidence_status_after_raw_mutation,
        Some(serde_json::json!("blocked_pending_operator_evidence")),
        "raw audit file mutation after import must not change DB-owned release evidence truth"
    );
}

#[tokio::test]
async fn proposal_057_active_generation_records_supersession_edge() {
    let pool = test_pool().await;
    let (run_id, _) = seed_run(&pool).await;

    artifact_contracts::upsert_generation_and_rebuild(
        &pool,
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "prepush_review_v1".into(),
            canonical_path: "review/prepush.json".into(),
            raw_path: "review/prepush-a.json".into(),
            raw_status: "PASS".into(),
            generation_id: "agent-exec-a".into(),
            source_agent_execution_id: Some("agent-exec-a".into()),
            source_stage_execution_id: None,
            source_session_generation_id: None,
            source_work_item_id: None,
            supersedes_generation_id: None,
            output_settlement:
                domain::agent::AgentOutputSettlement::ValidOutputsFromCompletedExecution,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();

    artifact_contracts::upsert_generation_and_rebuild(
        &pool,
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "prepush_review_v1".into(),
            canonical_path: "review/prepush.json".into(),
            raw_path: "review/prepush-b.json".into(),
            raw_status: "PASS_WITH_NOTES".into(),
            generation_id: "agent-exec-b".into(),
            source_agent_execution_id: Some("agent-exec-b".into()),
            source_stage_execution_id: None,
            source_session_generation_id: None,
            source_work_item_id: None,
            supersedes_generation_id: None,
            output_settlement:
                domain::agent::AgentOutputSettlement::ValidOutputsFromCompletedExecution,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();

    let projection = artifact_contracts::find_run_state_projection(&pool, run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        projection.active_index_json["contracts"]["prepush_review_v1"]["generation_id"],
        "agent-exec-b"
    );
    assert_eq!(
        projection.active_index_json["contracts"]["prepush_review_v1"]["supersedes"],
        serde_json::json!(["agent-exec-a"])
    );
}

#[tokio::test]
async fn proposal_057_invalid_contract_is_structured_block_and_disables_raw_fallback() {
    let pool = test_pool().await;
    let (run_id, _) = seed_run(&pool).await;

    artifact_contracts::upsert_generation_and_rebuild(
        &pool,
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "prepush_review_v1".into(),
            canonical_path: "review/prepush.json".into(),
            raw_path: "review/prepush.json".into(),
            raw_status: "PASSISH".into(),
            generation_id: "agent-exec-invalid".into(),
            source_agent_execution_id: Some("agent-exec-invalid".into()),
            source_stage_execution_id: None,
            source_session_generation_id: None,
            source_work_item_id: None,
            supersedes_generation_id: None,
            output_settlement: domain::agent::AgentOutputSettlement::None,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();

    let result = artifact_contracts::canonical_contract_field_result(
        &pool,
        run_id,
        "prepush_review_report",
        "status",
    )
    .await
    .unwrap();
    assert!(matches!(
        result,
        artifact_contracts::CanonicalContractField::MissingControlled { .. }
    ));
    let projection = artifact_contracts::find_run_state_projection(&pool, run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        projection.active_index_json["invalid_required_artifacts"][0]["reason"],
        "invalid_required_artifact"
    );
    assert_eq!(
        projection.active_index_json["invalid_required_artifacts"][0]["validation_errors"][0],
        "unknown status value: PASSISH"
    );
}

#[tokio::test]
async fn proposal_057_superseded_invalid_contract_does_not_block_active_valid_contract() {
    let pool = test_pool().await;
    let (run_id, _) = seed_run(&pool).await;

    artifact_contracts::upsert_generation_and_rebuild(
        &pool,
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "prepush_review_v1".into(),
            canonical_path: "review/prepush.json".into(),
            raw_path: "review/prepush-invalid.json".into(),
            raw_status: "PASSISH".into(),
            generation_id: "agent-exec-invalid".into(),
            source_agent_execution_id: Some("agent-exec-invalid".into()),
            source_stage_execution_id: None,
            source_session_generation_id: None,
            source_work_item_id: None,
            supersedes_generation_id: None,
            output_settlement: domain::agent::AgentOutputSettlement::None,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();

    artifact_contracts::upsert_generation_and_rebuild(
        &pool,
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "prepush_review_v1".into(),
            canonical_path: "review/prepush.json".into(),
            raw_path: "review/prepush-valid.json".into(),
            raw_status: "pass".into(),
            generation_id: "agent-exec-valid".into(),
            source_agent_execution_id: Some("agent-exec-valid".into()),
            source_stage_execution_id: None,
            source_session_generation_id: None,
            source_work_item_id: None,
            supersedes_generation_id: Some("agent-exec-invalid".into()),
            output_settlement:
                domain::agent::AgentOutputSettlement::ValidOutputsFromCompletedExecution,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();

    let status = artifact_contracts::canonical_contract_field(
        &pool,
        run_id,
        "prepush_review_report",
        "status",
    )
    .await
    .unwrap();
    assert_eq!(status, Some(serde_json::json!("pass")));

    let projection = artifact_contracts::find_run_state_projection(&pool, run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        projection.active_index_json["invalid_required_artifacts"],
        serde_json::json!([]),
        "superseded invalid contract generations must not keep transition truth fail-closed"
    );
    assert_eq!(
        projection.active_index_json["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning
                .as_str()
                .unwrap_or_default()
                .contains("invalid_required_artifact")),
        false
    );
}

#[tokio::test]
async fn proposal_057_agent_written_run_state_is_advisory_and_superseded() {
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let (run_id, _) =
        seed_run_with_meta(&pool, Some(tmp.path().to_string_lossy().into_owned())).await;
    let raw_path = tmp.path().join("state/run-state.json");
    std::fs::create_dir_all(raw_path.parent().unwrap()).unwrap();
    std::fs::write(
        &raw_path,
        r#"{"schema_version":"agent-authored","current_state":"agent_claim"}"#,
    )
    .unwrap();

    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    artifact_contracts::record_run_state_advisory_tx(
        &mut tx,
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "run_state_projection_v1".into(),
            canonical_path: "state/run-state.json".into(),
            raw_path: raw_path.to_string_lossy().into_owned(),
            raw_status: "superseded_advisory".into(),
            generation_id: "agent-run-state-1".into(),
            source_agent_execution_id: Some("agent-exec-run-state".into()),
            source_stage_execution_id: None,
            source_session_generation_id: None,
            source_work_item_id: Some("work-item-run-state".into()),
            supersedes_generation_id: None,
            output_settlement: domain::agent::AgentOutputSettlement::None,
            partial: false,
            warnings: vec![
                "agent-authored state/run-state.json is advisory only; sqlite projection remains canonical"
                    .into(),
            ],
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    artifact_contracts::export_projection_files(&pool, run_id)
        .await
        .unwrap();

    let projection = artifact_contracts::find_run_state_projection(&pool, run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        projection.active_index_json["advisory_artifacts"][0]["contract_id"],
        "run_state_projection_v1"
    );
    assert_eq!(
        projection.active_index_json["advisory_artifacts"][0]["superseded_by"],
        "sqlite_run_state_projection"
    );
    assert!(projection.active_index_json["warnings"][0]
        .as_str()
        .unwrap()
        .contains("agent-authored state/run-state.json"));

    let exported: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&raw_path).unwrap()).unwrap();
    assert_eq!(exported["active_index_owner"], "sqlite");
    assert_ne!(exported["schema_version"], "agent-authored");
}

#[tokio::test]
async fn proposal_094_exposes_decomposition_plan_fields_as_canonical_truth() {
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let (run_id, _) =
        seed_run_with_meta(&pool, Some(tmp.path().to_string_lossy().into_owned())).await;
    let raw_path = tmp.path().join("proposal/decomposition-plan.json");
    std::fs::create_dir_all(raw_path.parent().unwrap()).unwrap();
    std::fs::write(
        &raw_path,
        serde_json::json!({
            "schema_version": "proposal_decomposition_plan_v1",
            "requires_split": false,
            "implementation_start_decision": "ready_with_declared_boundaries"
        })
        .to_string(),
    )
    .unwrap();

    artifact_contracts::upsert_generation_and_rebuild(
        &pool,
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "proposal_decomposition_plan_v1".into(),
            canonical_path: "proposal/decomposition-plan.json".into(),
            raw_path: raw_path.to_string_lossy().into_owned(),
            raw_status: "unknown".into(),
            generation_id: "p094-decomposition-1".into(),
            source_agent_execution_id: Some("agent-decomposition".into()),
            source_stage_execution_id: None,
            source_session_generation_id: None,
            source_work_item_id: None,
            supersedes_generation_id: None,
            output_settlement:
                domain::agent::AgentOutputSettlement::ValidOutputsFromCompletedExecution,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();

    assert_eq!(
        artifact_contracts::canonical_contract_field(
            &pool,
            run_id,
            "proposal_decomposition_plan",
            "status",
        )
        .await
        .unwrap(),
        Some(serde_json::json!("ready_with_declared_boundaries"))
    );
    assert_eq!(
        artifact_contracts::canonical_contract_field(
            &pool,
            run_id,
            "proposal_decomposition_plan",
            "requires_split",
        )
        .await
        .unwrap(),
        Some(serde_json::json!(false))
    );
    assert_eq!(
        artifact_contracts::canonical_contract_field(
            &pool,
            run_id,
            "proposal_decomposition_plan",
            "implementation_start_decision",
        )
        .await
        .unwrap(),
        Some(serde_json::json!("ready_with_declared_boundaries"))
    );

    let projection = artifact_contracts::find_run_state_projection(&pool, run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        projection.active_index_json["contracts"]["proposal_decomposition_plan_v1"]["status"],
        "ready_with_declared_boundaries"
    );
    assert_eq!(
        projection.active_index_json["contracts"]["proposal_decomposition_plan_v1"]
            ["requires_split"],
        false
    );
}

#[tokio::test]
async fn proposal_094_decomposition_split_required_requires_blocking_split_candidate() {
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let (run_id, _) =
        seed_run_with_meta(&pool, Some(tmp.path().to_string_lossy().into_owned())).await;

    let valid_path = tmp
        .path()
        .join("proposal/decomposition-split-required.json");
    std::fs::create_dir_all(valid_path.parent().unwrap()).unwrap();
    std::fs::write(
        &valid_path,
        serde_json::json!({
            "schema_version": "proposal_decomposition_plan_v1",
            "requires_split": true,
            "implementation_start_decision": "split_required",
            "split_candidates": [{
                "candidate_id": "expanded_reliability_matrix",
                "reason": "The reliability matrix exceeds the current proposal implementation slice.",
                "recommended_followup_title": "Reliability Matrix Expansion"
            }]
        })
        .to_string(),
    )
    .unwrap();

    artifact_contracts::upsert_generation_and_rebuild(
        &pool,
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "proposal_decomposition_plan_v1".into(),
            canonical_path: "proposal/decomposition-plan.json".into(),
            raw_path: valid_path.to_string_lossy().into_owned(),
            raw_status: "unknown".into(),
            generation_id: "p094-decomposition-split-valid".into(),
            source_agent_execution_id: Some("agent-decomposition".into()),
            source_stage_execution_id: None,
            source_session_generation_id: None,
            source_work_item_id: None,
            supersedes_generation_id: None,
            output_settlement:
                domain::agent::AgentOutputSettlement::ValidOutputsFromCompletedExecution,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();

    assert_eq!(
        artifact_contracts::canonical_contract_field(
            &pool,
            run_id,
            "proposal_decomposition_plan",
            "status",
        )
        .await
        .unwrap(),
        Some(serde_json::json!("split_required"))
    );
    assert_eq!(
        artifact_contracts::canonical_contract_field(
            &pool,
            run_id,
            "proposal_decomposition_plan",
            "requires_split",
        )
        .await
        .unwrap(),
        Some(serde_json::json!(true))
    );

    let malformed_meta_root = tmp.path().join("malformed-run");
    let (malformed_run_id, _) = seed_run_with_meta(
        &pool,
        Some(malformed_meta_root.to_string_lossy().into_owned()),
    )
    .await;

    let malformed_path =
        malformed_meta_root.join("proposal/decomposition-split-required-empty.json");
    std::fs::create_dir_all(malformed_path.parent().unwrap()).unwrap();
    std::fs::write(
        &malformed_path,
        serde_json::json!({
            "schema_version": "proposal_decomposition_plan_v1",
            "requires_split": true,
            "implementation_start_decision": "ready_with_declared_boundaries",
            "split_candidates": []
        })
        .to_string(),
    )
    .unwrap();

    artifact_contracts::upsert_generation_and_rebuild(
        &pool,
        ActiveArtifactGenerationInput {
            run_id: malformed_run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "proposal_decomposition_plan_v1".into(),
            canonical_path: "proposal/decomposition-plan.json".into(),
            raw_path: malformed_path.to_string_lossy().into_owned(),
            raw_status: "unknown".into(),
            generation_id: "p094-decomposition-split-invalid".into(),
            source_agent_execution_id: Some("agent-decomposition".into()),
            source_stage_execution_id: None,
            source_session_generation_id: None,
            source_work_item_id: None,
            supersedes_generation_id: None,
            output_settlement:
                domain::agent::AgentOutputSettlement::ValidOutputsFromCompletedExecution,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();

    assert_eq!(
        artifact_contracts::canonical_contract_field(
            &pool,
            malformed_run_id,
            "proposal_decomposition_plan",
            "requires_split",
        )
        .await
        .unwrap(),
        None,
        "requires_split=true without a blocking split candidate must fail closed"
    );
    assert_eq!(
        artifact_contracts::canonical_contract_field(
            &pool,
            malformed_run_id,
            "proposal_decomposition_plan",
            "implementation_start_decision",
        )
        .await
        .unwrap(),
        None,
        "malformed split-required plans must not expose implementation approval routing"
    );
}

#[tokio::test]
async fn proposal_094_exposes_blocker_boundary_fields_as_canonical_truth() {
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let (run_id, _) =
        seed_run_with_meta(&pool, Some(tmp.path().to_string_lossy().into_owned())).await;
    let raw_path = tmp.path().join("quality/blocker-boundary-status.json");
    std::fs::create_dir_all(raw_path.parent().unwrap()).unwrap();
    std::fs::write(
        &raw_path,
        serde_json::json!({
            "schema_version": "blocker_boundary_status_v1",
            "status": "awaiting_human_boundary_approval",
            "followup_proposal_required": true,
            "has_release_blocking_external_blockers": true,
            "has_no_release_blocking_external_blockers": false,
            "projection_integrity": "valid",
            "primary_owner_class": "external",
            "workflow_route_hint": "human_boundary_approval",
            "blocker_freshness": "fresh",
            "allowed_workflow_routes": ["state_9_blocker_boundary_approval"],
            "blockers": [{
                "id": "external-proof",
                "summary": "external proof required",
                "blocker_signature_id": "sig-external-proof",
                "evidence_fingerprint": "fingerprint-external-proof",
                "source_artifact_generation_id": "generation-external-proof",
                "observed_after_stage_execution_id": "stage-exec-1",
                "observed_after_agent_execution_id": "agent-exec-1",
                "owner_class": "external_environment",
                "class": "remote_host",
                "evidence_freshness": "fresh",
                "allowed_workflow_routes": ["state_9_blocker_boundary_approval"]
            }],
            "hard_blockers": [{
                "id": "external-proof",
                "blocker_signature_id": "sig-external-proof",
                "evidence_fingerprint": "fingerprint-external-proof"
            }]
        })
        .to_string(),
    )
    .unwrap();

    artifact_contracts::upsert_generation_and_rebuild(
        &pool,
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "blocker_boundary_status_v1".into(),
            canonical_path: "quality/blocker-boundary-status.json".into(),
            raw_path: raw_path.to_string_lossy().into_owned(),
            raw_status: "unknown".into(),
            generation_id: "p094-boundary-status-1".into(),
            source_agent_execution_id: Some("agent-boundary".into()),
            source_stage_execution_id: None,
            source_session_generation_id: None,
            source_work_item_id: None,
            supersedes_generation_id: None,
            output_settlement:
                domain::agent::AgentOutputSettlement::ValidOutputsFromCompletedExecution,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();

    for (field_name, expected) in [
        (
            "status",
            serde_json::json!("awaiting_human_boundary_approval"),
        ),
        ("followup_proposal_required", serde_json::json!(true)),
        (
            "has_release_blocking_external_blockers",
            serde_json::json!(true),
        ),
        (
            "has_no_release_blocking_external_blockers",
            serde_json::json!(false),
        ),
        ("projection_integrity", serde_json::json!("valid")),
        ("primary_owner_class", serde_json::json!("external")),
        (
            "workflow_route_hint",
            serde_json::json!("human_boundary_approval"),
        ),
        ("blocker_freshness", serde_json::json!("fresh")),
        (
            "allowed_workflow_routes",
            serde_json::json!(["state_9_blocker_boundary_approval"]),
        ),
    ] {
        assert_eq!(
            artifact_contracts::canonical_contract_field(
                &pool,
                run_id,
                "blocker_boundary_status",
                field_name,
            )
            .await
            .unwrap(),
            Some(expected),
            "field {field_name} should be canonical transition truth"
        );
    }

    assert_eq!(
        artifact_contracts::canonical_contract_field(
            &pool,
            run_id,
            "blocker_boundary_status",
            "unextracted_future_field",
        )
        .await
        .unwrap(),
        None,
        "unregistered P094 fields must fail closed instead of reading raw JSON ad hoc"
    );

    let readback = artifact_contracts::p094_readback_json(&pool, run_id)
        .await
        .unwrap();
    assert_eq!(
        readback["schema_version"],
        serde_json::json!("p094_boundary_readback_v1")
    );
    assert_eq!(
        readback["blocker_boundary_status"]["status"],
        serde_json::json!("awaiting_human_boundary_approval")
    );
    assert_eq!(
        readback["blocker_boundary_status"]["allowed_workflow_routes"],
        serde_json::json!(["state_9_blocker_boundary_approval"])
    );
    assert_eq!(
        readback["blocker_boundary_status"]["blockers"][0]["blocker_signature_id"],
        serde_json::json!("sig-external-proof")
    );
    assert_eq!(
        readback["blocker_boundary_status"]["hard_blockers"][0]["evidence_fingerprint"],
        serde_json::json!("fingerprint-external-proof")
    );
    assert_eq!(
        readback["followup_proposal_seeds"]
            .as_array()
            .expect("followup seeds lane should be an array")
            .len(),
        0
    );
}

#[tokio::test]
async fn proposal_094_readback_includes_boundary_approval_request_lane() {
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let (run_id, _) =
        seed_run_with_meta(&pool, Some(tmp.path().to_string_lossy().into_owned())).await;
    let raw_path = tmp
        .path()
        .join("quality/blocker-boundary-approval-request.json");
    std::fs::create_dir_all(raw_path.parent().unwrap()).unwrap();
    std::fs::write(
        &raw_path,
        serde_json::json!({
            "schema_version": "blocker_boundary_approval_request_v1",
            "status": "requested",
            "question": "Accept the server-evaluated boundary?",
            "allowed_decisions": ["accept", "reject"],
            "label_to_approval_state": {
                "accept": "granted",
                "reject": "rejected"
            }
        })
        .to_string(),
    )
    .unwrap();

    artifact_contracts::upsert_generation_and_rebuild(
        &pool,
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "blocker_boundary_approval_request_v1".into(),
            canonical_path: "quality/blocker-boundary-approval-request.json".into(),
            raw_path: raw_path.to_string_lossy().into_owned(),
            raw_status: "requested".into(),
            generation_id: "p094-approval-request-1".into(),
            source_agent_execution_id: Some("system.quality_gate_boundary".into()),
            source_stage_execution_id: Some("state_9_quality_gate_boundary_evaluated".into()),
            source_session_generation_id: None,
            source_work_item_id: None,
            supersedes_generation_id: None,
            output_settlement:
                domain::agent::AgentOutputSettlement::ValidOutputsFromCompletedExecution,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();

    let readback = artifact_contracts::p094_readback_json(&pool, run_id)
        .await
        .unwrap();
    assert_eq!(
        readback["blocker_boundary_approval_request"]["status"],
        serde_json::json!("requested")
    );
    assert_eq!(
        readback["blocker_boundary_approval_request"]["label_to_approval_state"]["reject"],
        serde_json::json!("rejected")
    );
}

#[tokio::test]
async fn proposal_094_readback_rejects_raw_path_outside_run_root() {
    let pool = test_pool().await;
    let meta = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let (run_id, _) =
        seed_run_with_meta(&pool, Some(meta.path().to_string_lossy().into_owned())).await;
    let outside_path = outside
        .path()
        .join("blocker-boundary-approval-request.json");
    std::fs::write(
        &outside_path,
        serde_json::json!({
            "schema_version": "blocker_boundary_approval_request_v1",
            "status": "requested",
            "secret": "outside-root"
        })
        .to_string(),
    )
    .unwrap();

    artifact_contracts::upsert_generation_and_rebuild(
        &pool,
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "blocker_boundary_approval_request_v1".into(),
            canonical_path: "quality/blocker-boundary-approval-request.json".into(),
            raw_path: outside_path.to_string_lossy().into_owned(),
            raw_status: "requested".into(),
            generation_id: "p094-approval-request-outside-root".into(),
            source_agent_execution_id: Some("system.quality_gate_boundary".into()),
            source_stage_execution_id: None,
            source_session_generation_id: None,
            source_work_item_id: None,
            supersedes_generation_id: None,
            output_settlement: domain::agent::AgentOutputSettlement::None,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();

    let readback = artifact_contracts::p094_readback_json(&pool, run_id)
        .await
        .unwrap();
    assert!(
        readback["blocker_boundary_approval_request"]["secret"].is_null()
            && readback["blocker_boundary_approval_request"]["status"].is_null(),
        "P094 readback must not expose raw_path payload fields outside the run root"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn proposal_094_readback_rejects_symlinked_raw_path() {
    let pool = test_pool().await;
    let meta = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let (run_id, _) =
        seed_run_with_meta(&pool, Some(meta.path().to_string_lossy().into_owned())).await;
    let outside_path = outside.path().join("secret.json");
    std::fs::write(
        &outside_path,
        serde_json::json!({
            "schema_version": "blocker_boundary_approval_request_v1",
            "status": "requested",
            "secret": "symlink-target"
        })
        .to_string(),
    )
    .unwrap();
    let link_path = meta
        .path()
        .join("quality/blocker-boundary-approval-request.json");
    std::fs::create_dir_all(link_path.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&outside_path, &link_path).unwrap();

    artifact_contracts::upsert_generation_and_rebuild(
        &pool,
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "blocker_boundary_approval_request_v1".into(),
            canonical_path: "quality/blocker-boundary-approval-request.json".into(),
            raw_path: link_path.to_string_lossy().into_owned(),
            raw_status: "requested".into(),
            generation_id: "p094-approval-request-symlink".into(),
            source_agent_execution_id: Some("system.quality_gate_boundary".into()),
            source_stage_execution_id: None,
            source_session_generation_id: None,
            source_work_item_id: None,
            supersedes_generation_id: None,
            output_settlement: domain::agent::AgentOutputSettlement::None,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();

    let readback = artifact_contracts::p094_readback_json(&pool, run_id)
        .await
        .unwrap();
    assert!(
        readback["blocker_boundary_approval_request"]["secret"].is_null()
            && readback["blocker_boundary_approval_request"]["status"].is_null(),
        "P094 readback must not follow symlinks to expose raw_path payload fields"
    );
}

#[tokio::test]
async fn proposal_094_normalizes_human_decision_labels_to_durable_approval_state() {
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let (run_id, _) =
        seed_run_with_meta(&pool, Some(tmp.path().to_string_lossy().into_owned())).await;
    let raw_path = tmp
        .path()
        .join("quality/blocker-boundary-human-decision.json");
    std::fs::create_dir_all(raw_path.parent().unwrap()).unwrap();
    std::fs::write(
        &raw_path,
        serde_json::json!({
            "schema_version": "blocker_boundary_human_decision_v1",
            "approval_id": "approval-p094-1",
            "decision_label": "accept",
            "canonical_approval_state": "granted",
            "comment": "Accepted as external evidence tail.",
            "decided_at": "2026-05-26T00:00:00Z",
            "decided_by": "operator"
        })
        .to_string(),
    )
    .unwrap();

    artifact_contracts::upsert_generation_and_rebuild(
        &pool,
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "blocker_boundary_human_decision_v1".into(),
            canonical_path: "quality/blocker-boundary-human-decision.json".into(),
            raw_path: raw_path.to_string_lossy().into_owned(),
            raw_status: "unknown".into(),
            generation_id: "p094-human-decision-1".into(),
            source_agent_execution_id: Some("operator-boundary-approval".into()),
            source_stage_execution_id: None,
            source_session_generation_id: None,
            source_work_item_id: None,
            supersedes_generation_id: None,
            output_settlement: domain::agent::AgentOutputSettlement::None,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();

    assert_eq!(
        artifact_contracts::canonical_contract_field(
            &pool,
            run_id,
            "blocker_boundary_human_decision",
            "status",
        )
        .await
        .unwrap(),
        Some(serde_json::json!("granted"))
    );
    let readback = artifact_contracts::p094_readback_json(&pool, run_id)
        .await
        .unwrap();
    assert_eq!(
        readback["blocker_boundary_human_decision"]["approval_id"],
        serde_json::json!("approval-p094-1")
    );
    assert_eq!(
        readback["blocker_boundary_human_decision"]["decided_by"],
        serde_json::json!("operator")
    );
}
