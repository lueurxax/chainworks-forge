use chrono::Utc;
use db::pool::create_pool;
use db::repos::{artifact_contracts, ideas, runs};
use domain::artifact_contracts::{ActiveArtifactGenerationInput, ArtifactContractOverrideInput};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{ArtifactId, IdeaId, RunId};
use domain::run::{Run, RunStatus};

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
    let pool = create_pool("sqlite::memory:").await.unwrap();
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
async fn proposal_057_implementation_and_tests_contract_fields_are_canonical() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
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
    let pool = create_pool("sqlite::memory:").await.unwrap();
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
    let pool = create_pool("sqlite::memory:").await.unwrap();
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
    let pool = create_pool("sqlite::memory:").await.unwrap();
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
    let pool = create_pool("sqlite::memory:").await.unwrap();
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
    let pool = create_pool("sqlite::memory:").await.unwrap();
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
async fn proposal_057_agent_written_run_state_is_advisory_and_superseded() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
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
