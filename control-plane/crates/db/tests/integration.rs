use chrono::Utc;
use db::pool::create_pool;
use db::repos::{
    agent_executions, approvals, artifact_contracts as artifact_contract_repos, artifacts, ideas,
    projections, runs, sessions, stages, steward, validation, work_items,
};
use db::work_item::{WorkItem, WorkItemKind, WorkItemStatus};
use domain::agent::{AgentExecution, AgentStatus};
use domain::approval::{Approval, ApprovalDecision};
use domain::artifact::{Artifact, ArtifactFormat};
use domain::artifact_contracts::{
    parse_implementation_self_assessment_v2, ActiveArtifactGenerationInput, ContractParseContext,
    ImplementationSelfAssessmentStatus, IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH,
    IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{AgentExecutionId, ApprovalId, ArtifactId, IdeaId, RunId, StageExecutionId};
use domain::run::{Run, RunStatus};
use domain::session::{SessionGeneration, SessionGenerationStatus, SessionLineage};
use domain::stage::{StageExecution, StageSettlementKind, StageStatus};
use domain::steward::{
    CohortQuality, StewardAnalysis, StewardAnalysisRunLink, StewardAnalysisStatus,
    StewardRecommendation,
};
use domain::validation::{
    ContractValidationMetadata, OutputValidationResult, RecoveryRecommendation,
    ValidationFailureClass, ValidationFailureRecord, ValidationStatus,
};

async fn test_pool() -> sqlx::SqlitePool {
    create_pool("sqlite::memory:")
        .await
        .expect("in-memory pool failed")
}

async fn seed_contract_run(pool: &sqlx::SqlitePool) -> RunId {
    let idea = Idea {
        id: IdeaId::new(),
        title: "Contract idea".into(),
        body: "body".into(),
        workspace_root_path: None,
        project_key: None,
        status: IdeaStatus::Active,
        created_at: Utc::now(),
        archived_at: None,
    };
    ideas::insert(pool, &idea).await.unwrap();

    let run = Run {
        id: RunId::new(),
        idea_id: idea.id,
        status: RunStatus::Running,
        workflow_id: "wf-contract".into(),
        workflow_title: "Contract Workflow".into(),
        workspace_root: "/tmp/ws".into(),
        artifact_root: "/tmp/art".into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        cancellation_settlement_log: None,
        current_state: Some("state_8_implementation".into()),
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
    run.id
}

async fn insert_contract_artifact(
    pool: &sqlx::SqlitePool,
    run_id: RunId,
    contract_id: &str,
    name: &str,
) -> ArtifactId {
    let artifact = Artifact {
        id: ArtifactId::new(),
        run_id,
        stage_id: "state_8_implementation".into(),
        agent_id: "code_writer".into(),
        name: name.into(),
        contract_id: contract_id.into(),
        format: ArtifactFormat::Json,
        file_path: format!("/tmp/art/{name}.json"),
        checksum_sha256: None,
        size_bytes: None,
        provider: "codex".into(),
        model: None,
        created_at: Utc::now(),
        is_pinned: false,
        report_kind: None,
        report_version: None,
    };
    artifacts::insert(pool, &artifact).await.unwrap();
    artifact.id
}

fn contract_context(declared_contract_id: Option<&str>) -> ContractParseContext {
    ContractParseContext {
        run_id: "run".into(),
        run_age: None,
        declared_contract_id: declared_contract_id.map(str::to_string),
        canonical_artifact_path: IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH.into(),
        raw_artifact_path: Some(IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH.into()),
        source_generation_id: None,
        artifact_created_at: Some(Utc::now()),
        v2_generation_seen_for_run: false,
        legacy_v1_generation_available: false,
    }
}

fn valid_v2_handoff_required() -> serde_json::Value {
    serde_json::json!({
        "implementation_complete": true,
        "verification_green": true,
        "remaining_code_tasks": [],
        "handoff_tasks": [{
            "summary": "Collect signed manual smoke evidence.",
            "owner_class": "manual_evidence",
            "target_stage": "state_10_release_gate",
            "blocking_review": true,
            "evidence": "Manual smoke evidence is intentionally collected after implementation review."
        }],
        "known_risks": ["Manual evidence still gates release."],
        "tests_run": ["cargo test -p domain"],
        "docs_impacted": []
    })
}

#[tokio::test]
async fn artifact_contract_summary_persists_active_dimensions() {
    let pool = test_pool().await;
    let run_id = seed_contract_run(&pool).await;
    let artifact_id = insert_contract_artifact(
        &pool,
        run_id,
        IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
        "implementation-self-assessment-v2",
    )
    .await;
    let summary = parse_implementation_self_assessment_v2(
        &valid_v2_handoff_required(),
        contract_context(Some(IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID)),
    );

    let stored = artifact_contract_repos::persist_implementation_self_assessment_summary(
        &pool,
        run_id,
        artifact_id,
        IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
        &summary,
        Utc::now(),
    )
    .await
    .unwrap();

    assert_eq!(stored.artifact_id, artifact_id);
    assert!(stored.is_active);
    assert_eq!(stored.source_kind, "v2");
    assert_eq!(
        stored.summary.status,
        ImplementationSelfAssessmentStatus::HandoffRequired
    );
    assert_eq!(stored.summary.handoff_task_count, Some(1));
    assert_eq!(stored.summary.blocking_review_handoff_task_count, Some(1));
    assert_eq!(
        stored
            .summary
            .owner_class_counts
            .get("manual_evidence")
            .copied(),
        Some(1)
    );
}

#[tokio::test]
async fn artifact_contract_summary_active_generation_uses_domain_handoff_required_status() {
    let pool = test_pool().await;
    let run_id = seed_contract_run(&pool).await;
    let tmp = tempfile::tempdir().unwrap();
    let raw_path = tmp.path().join("implementation/self-assessment.json");
    std::fs::create_dir_all(raw_path.parent().unwrap()).unwrap();
    std::fs::write(
        &raw_path,
        serde_json::to_vec(&valid_v2_handoff_required()).unwrap(),
    )
    .unwrap();

    artifact_contract_repos::upsert_generation_and_rebuild(
        &pool,
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID.into(),
            canonical_path: IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH.into(),
            raw_path: raw_path.to_string_lossy().into_owned(),
            raw_status: "complete".into(),
            generation_id: "handoff-generation".into(),
            source_agent_execution_id: Some("agent-exec-handoff".into()),
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

    let status = artifact_contract_repos::canonical_contract_field(
        &pool,
        run_id,
        "implementation_self_assessment_v2",
        "status",
    )
    .await
    .unwrap();
    assert_eq!(status, Some(serde_json::json!("handoff_required")));
    let handoff_count = artifact_contract_repos::canonical_contract_field(
        &pool,
        run_id,
        "implementation_self_assessment_v2",
        "blocking_review_handoff_task_count",
    )
    .await
    .unwrap();
    assert_eq!(handoff_count, Some(serde_json::json!(1)));
}

#[tokio::test]
async fn artifact_contract_summary_prefers_invalid_v2_over_legacy_v1() {
    let pool = test_pool().await;
    let run_id = seed_contract_run(&pool).await;
    let legacy_artifact_id = insert_contract_artifact(
        &pool,
        run_id,
        "implementation_self_assessment_v1",
        "implementation-self-assessment-v1",
    )
    .await;
    let legacy_summary = parse_implementation_self_assessment_v2(
        &serde_json::json!({ "seemingly_complete": true }),
        contract_context(Some("implementation_self_assessment_v1")),
    );
    artifact_contract_repos::persist_implementation_self_assessment_summary(
        &pool,
        run_id,
        legacy_artifact_id,
        "implementation_self_assessment_v1",
        &legacy_summary,
        Utc::now(),
    )
    .await
    .unwrap();

    let v2_artifact_id = insert_contract_artifact(
        &pool,
        run_id,
        "raw_output",
        "implementation-self-assessment-raw-v2",
    )
    .await;
    let invalid_v2_summary = parse_implementation_self_assessment_v2(
        &valid_v2_handoff_required(),
        contract_context(None),
    );
    let active = artifact_contract_repos::persist_implementation_self_assessment_summary(
        &pool,
        run_id,
        v2_artifact_id,
        "raw_output",
        &invalid_v2_summary,
        Utc::now(),
    )
    .await
    .unwrap();

    assert_eq!(active.artifact_id, v2_artifact_id);
    assert_eq!(active.source_kind, "v2");
    assert_eq!(
        active.summary.status,
        ImplementationSelfAssessmentStatus::Invalid
    );
    assert!(active
        .summary
        .validation_errors
        .iter()
        .any(|issue| issue.code == "raw_only_v2_artifact"));
}

#[tokio::test]
async fn artifact_contract_summary_keeps_v2_active_over_later_legacy_same_path() {
    let pool = test_pool().await;
    let run_id = seed_contract_run(&pool).await;
    let v2_artifact_id = insert_contract_artifact(
        &pool,
        run_id,
        IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
        "implementation-self-assessment-v2",
    )
    .await;
    let v2_summary = parse_implementation_self_assessment_v2(
        &valid_v2_handoff_required(),
        contract_context(Some(IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID)),
    );
    artifact_contract_repos::persist_implementation_self_assessment_summary(
        &pool,
        run_id,
        v2_artifact_id,
        IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
        &v2_summary,
        Utc::now(),
    )
    .await
    .unwrap();

    let legacy_artifact_id = insert_contract_artifact(
        &pool,
        run_id,
        "implementation_self_assessment_v1",
        "implementation-self-assessment-v1",
    )
    .await;
    let legacy_summary = parse_implementation_self_assessment_v2(
        &serde_json::json!({ "seemingly_complete": true }),
        contract_context(Some("implementation_self_assessment_v1")),
    );
    let active = artifact_contract_repos::persist_implementation_self_assessment_summary(
        &pool,
        run_id,
        legacy_artifact_id,
        "implementation_self_assessment_v1",
        &legacy_summary,
        Utc::now(),
    )
    .await
    .unwrap();
    let legacy_record =
        artifact_contract_repos::find_implementation_self_assessment_summary_by_artifact(
            &pool,
            legacy_artifact_id,
        )
        .await
        .unwrap()
        .expect("legacy summary should be stored");

    assert_eq!(active.artifact_id, v2_artifact_id);
    assert!(active.is_active);
    assert!(!legacy_record.is_active);
    assert!(legacy_record
        .summary
        .warnings
        .iter()
        .any(|issue| issue.code == "same_path_contract_conflict"));
}

#[tokio::test]
async fn v1_fallback_retirement_check_reports_active_non_terminal_legacy_runs() {
    let pool = test_pool().await;
    let active_legacy_run_id = seed_contract_run(&pool).await;
    let active_legacy_artifact_id = insert_contract_artifact(
        &pool,
        active_legacy_run_id,
        "implementation_self_assessment_v1",
        "active-legacy-implementation-self-assessment",
    )
    .await;
    let legacy_summary = parse_implementation_self_assessment_v2(
        &serde_json::json!({ "seemingly_complete": true }),
        contract_context(Some("implementation_self_assessment_v1")),
    );
    artifact_contract_repos::persist_implementation_self_assessment_summary(
        &pool,
        active_legacy_run_id,
        active_legacy_artifact_id,
        "implementation_self_assessment_v1",
        &legacy_summary,
        Utc::now(),
    )
    .await
    .unwrap();

    let terminal_legacy_run_id = seed_contract_run(&pool).await;
    let terminal_legacy_artifact_id = insert_contract_artifact(
        &pool,
        terminal_legacy_run_id,
        "implementation_self_assessment_v1",
        "terminal-legacy-implementation-self-assessment",
    )
    .await;
    artifact_contract_repos::persist_implementation_self_assessment_summary(
        &pool,
        terminal_legacy_run_id,
        terminal_legacy_artifact_id,
        "implementation_self_assessment_v1",
        &legacy_summary,
        Utc::now(),
    )
    .await
    .unwrap();
    runs::update_status(&pool, terminal_legacy_run_id, RunStatus::Completed)
        .await
        .unwrap();

    let check = artifact_contract_repos::v1_fallback_retirement_check(&pool)
        .await
        .unwrap();
    assert!(!check.safe_to_retire());
    assert_eq!(check.active_non_terminal_v1_only_run_count(), 1);
    assert_eq!(
        check.active_non_terminal_v1_only_run_ids,
        vec![active_legacy_run_id]
    );

    runs::update_status(&pool, active_legacy_run_id, RunStatus::Completed)
        .await
        .unwrap();
    let check = artifact_contract_repos::v1_fallback_retirement_check(&pool)
        .await
        .unwrap();
    assert!(check.safe_to_retire());
    assert_eq!(check.active_non_terminal_v1_only_run_count(), 0);
}

#[tokio::test]
async fn sqlite_pool_uses_30_second_busy_timeout() {
    let pool = test_pool().await;

    let busy_timeout_ms: i64 = sqlx::query_scalar("PRAGMA busy_timeout")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(busy_timeout_ms, 30_000);
}

#[tokio::test]
async fn migrations_create_hot_scheduler_indexes() {
    let pool = test_pool().await;

    let indexes: Vec<String> = sqlx::query_scalar(
        r#"SELECT name
           FROM sqlite_master
           WHERE type = 'index'
             AND name IN (
               'idx_work_items_status_kind_scheduled',
               'idx_agent_executions_status',
               'idx_agent_executions_status_provider',
               'idx_agent_executions_status_stage'
             )
           ORDER BY name"#,
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(
        indexes,
        vec![
            "idx_agent_executions_status".to_string(),
            "idx_agent_executions_status_provider".to_string(),
            "idx_agent_executions_status_stage".to_string(),
            "idx_work_items_status_kind_scheduled".to_string(),
        ]
    );
}

#[tokio::test]
async fn completing_invoke_agent_enqueues_post_completion_advance_run() {
    let pool = test_pool().await;
    let idea = Idea {
        id: IdeaId::new(),
        title: "Idea".into(),
        body: "body".into(),
        workspace_root_path: None,
        project_key: None,
        status: IdeaStatus::Active,
        created_at: Utc::now(),
        archived_at: None,
    };
    ideas::insert(&pool, &idea).await.unwrap();

    let run = Run {
        id: RunId::new(),
        idea_id: idea.id,
        status: RunStatus::Running,
        workflow_id: "wf-invoke-completion".into(),
        workflow_title: "Invoke Completion".into(),
        workspace_root: "/tmp/ws".into(),
        artifact_root: "/tmp/art".into(),
        started_at: Utc::now(),
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
    runs::insert(&pool, &run).await.unwrap();

    let now = Utc::now();
    let invoke_id = "p058-invoke:completion-finalizer-test:0";
    work_items::enqueue(
        &pool,
        &WorkItem {
            id: invoke_id.into(),
            kind: WorkItemKind::InvokeAgent,
            payload_json: serde_json::json!({
                "run_id": run.id.to_string(),
                "stage_id": "state_4_proposal_reviewed",
                "stage_execution_id": StageExecutionId::new().to_string(),
                "agent_id": "proposal_reviewer_architect",
                "provider": "codex",
            })
            .to_string(),
            status: WorkItemStatus::Running,
            run_id: Some(run.id),
            stage_id: Some("state_4_proposal_reviewed".into()),
            created_at: now,
            scheduled_at: now,
            attempt_count: 1,
            last_error: None,
        },
    )
    .await
    .unwrap();

    work_items::complete(&pool, invoke_id).await.unwrap();

    let items = work_items::list_by_run(&pool, run.id).await.unwrap();
    let invoke = items
        .iter()
        .find(|item| item.id == invoke_id)
        .expect("completed invoke item");
    assert_eq!(invoke.status, WorkItemStatus::Completed);

    let pending_advances: Vec<_> = items
        .iter()
        .filter(|item| {
            item.kind == WorkItemKind::AdvanceRun && item.status == WorkItemStatus::Pending
        })
        .collect();
    assert_eq!(
        pending_advances.len(),
        1,
        "completion must leave a durable post-completion AdvanceRun wake-up"
    );

    let payload: serde_json::Value =
        serde_json::from_str(&pending_advances[0].payload_json).unwrap();
    assert_eq!(payload["run_id"], run.id.to_string());
    assert_eq!(payload["completed_invoke_work_item_id"], invoke_id);
    assert_eq!(payload["reason"], "invoke_agent_completed");
}

#[tokio::test]
async fn session_lineage_migration_renames_legacy_table_and_creates_canonical_tables() {
    use sqlx::Row;

    let pool = test_pool().await;

    let rows = sqlx::query(
        r#"SELECT name
           FROM sqlite_master
           WHERE type = 'table'
             AND name IN ('session_lineages_legacy', 'session_lineages', 'session_generations', 'session_events')
           ORDER BY name ASC"#,
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let names: Vec<String> = rows.into_iter().map(|row| row.get("name")).collect();

    assert_eq!(
        names,
        vec![
            "session_events".to_string(),
            "session_generations".to_string(),
            "session_lineages".to_string(),
            "session_lineages_legacy".to_string(),
        ]
    );
}

#[tokio::test]
async fn session_generation_usage_update_persists_budget_snapshot_fields() {
    let pool = test_pool().await;
    let idea = Idea {
        id: IdeaId::new(),
        title: "Idea".into(),
        body: "body".into(),
        workspace_root_path: None,
        project_key: None,
        status: IdeaStatus::Active,
        created_at: Utc::now(),
        archived_at: None,
    };
    ideas::insert(&pool, &idea).await.unwrap();

    let run = Run {
        id: RunId::new(),
        idea_id: idea.id,
        status: RunStatus::Running,
        workflow_id: "wf-session-budget".into(),
        workflow_title: "Session Budget".into(),
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
    };
    runs::insert(&pool, &run).await.unwrap();

    let lineage = domain::session::SessionLineage {
        id: "lineage-budget".into(),
        run_id: run.id.to_string(),
        agent_id: "proposal_writer".into(),
        lineage_id: "proposal-loop".into(),
        session_reuse_scope: "same_agent_family_within_run".into(),
        session_family_id: Some("proposal-loop".into()),
        active_generation_id: Some("generation-budget".into()),
        created_at: Utc::now(),
        closed_at: None,
    };
    db::repos::sessions::insert_lineage(&pool, &lineage)
        .await
        .unwrap();

    let created_at = Utc::now();
    db::repos::sessions::insert_generation(
        &pool,
        &domain::session::SessionGeneration {
            id: "generation-budget".into(),
            lineage_id: lineage.id.clone(),
            generation: 1,
            invocation_owner_key: "owner".into(),
            provider_session_id: None,
            binding_fingerprint: "fingerprint".into(),
            rehydrated_from_checkpoint_artifact_id: None,
            working_directory: "/tmp/ws".into(),
            workspace_mode: "read_only".into(),
            runtime_provider: "claude".into(),
            runtime_model: "sonnet".into(),
            status: domain::session::SessionGenerationStatus::Active,
            turn_count: 0,
            estimated_input_tokens: 0,
            latest_cached_input_tokens: None,
            latest_output_tokens: None,
            latest_model_context_window: None,
            cumulative_prompt_tokens: 0,
            cumulative_cost_cents: 0,
            created_at,
            last_activity_at: None,
            ended_at: None,
            end_reason: None,
        },
    )
    .await
    .unwrap();

    let last_activity_at = Utc::now();
    db::repos::sessions::update_generation_usage(
        &pool,
        "generation-budget",
        "provider-session",
        1,
        12_000,
        17,
        12_000,
        Some(3_000),
        Some(1_200),
        Some(200_000),
        last_activity_at,
    )
    .await
    .unwrap();

    let generation = db::repos::sessions::find_active_generation(&pool, &lineage.id)
        .await
        .unwrap()
        .expect("generation should exist");
    assert_eq!(
        generation.provider_session_id.as_deref(),
        Some("provider-session")
    );
    assert_eq!(generation.turn_count, 1);
    assert_eq!(generation.estimated_input_tokens, 12_000);
    assert_eq!(generation.latest_cached_input_tokens, Some(3_000));
    assert_eq!(generation.latest_output_tokens, Some(1_200));
    assert_eq!(generation.latest_model_context_window, Some(200_000));
    assert_eq!(generation.cumulative_prompt_tokens, 12_000);
    assert_eq!(generation.cumulative_cost_cents, 17);
    assert_eq!(generation.last_activity_at, Some(last_activity_at));
}

#[tokio::test]
async fn session_lookup_helpers_read_by_id() {
    let pool = test_pool().await;
    let idea = Idea {
        id: IdeaId::new(),
        title: "Idea".into(),
        body: "body".into(),
        workspace_root_path: None,
        project_key: None,
        status: IdeaStatus::Active,
        created_at: Utc::now(),
        archived_at: None,
    };
    ideas::insert(&pool, &idea).await.unwrap();

    let run = Run {
        id: RunId::new(),
        idea_id: idea.id,
        status: RunStatus::Running,
        workflow_id: "wf-session-lookup".into(),
        workflow_title: "Session Lookup".into(),
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
    };
    runs::insert(&pool, &run).await.unwrap();

    let lineage = SessionLineage {
        id: "session-lineage-lookup".into(),
        run_id: run.id.to_string(),
        agent_id: "code_writer".into(),
        lineage_id: "session-family-lookup".into(),
        session_reuse_scope: "same_agent_family_within_run".into(),
        session_family_id: Some("family-lookup".into()),
        active_generation_id: Some("session-generation-lookup".into()),
        created_at: Utc::now(),
        closed_at: None,
    };
    sessions::insert_lineage(&pool, &lineage).await.unwrap();

    let generation = SessionGeneration {
        id: "session-generation-lookup".into(),
        lineage_id: lineage.id.clone(),
        generation: 3,
        invocation_owner_key: "owner-key".into(),
        provider_session_id: Some("provider-session-lookup".into()),
        binding_fingerprint: "fingerprint-lookup".into(),
        rehydrated_from_checkpoint_artifact_id: None,
        working_directory: "/tmp/ws".into(),
        workspace_mode: "workspace".into(),
        runtime_provider: "claude".into(),
        runtime_model: "sonnet".into(),
        status: SessionGenerationStatus::Closed,
        turn_count: 4,
        estimated_input_tokens: 10,
        latest_cached_input_tokens: Some(2),
        latest_output_tokens: Some(8),
        latest_model_context_window: Some(4096),
        cumulative_prompt_tokens: 11,
        cumulative_cost_cents: 12,
        created_at: Utc::now(),
        last_activity_at: None,
        ended_at: None,
        end_reason: Some("completed".into()),
    };
    sessions::insert_generation(&pool, &generation)
        .await
        .unwrap();

    let found_lineage = sessions::find_lineage_by_id(&pool, &lineage.id)
        .await
        .unwrap()
        .expect("lineage");
    let found_generation = sessions::find_generation_by_id(&pool, &generation.id)
        .await
        .unwrap()
        .expect("generation");

    assert_eq!(
        found_lineage.active_generation_id.as_deref(),
        Some("session-generation-lookup")
    );
    assert_eq!(
        found_generation.provider_session_id.as_deref(),
        Some("provider-session-lookup")
    );
    assert_eq!(found_generation.status, SessionGenerationStatus::Closed);
    assert_eq!(found_generation.lineage_id, lineage.id);
}

#[tokio::test]
async fn session_lookup_helpers_return_none_for_missing_rows() {
    let pool = test_pool().await;

    assert!(sessions::find_lineage_by_id(&pool, "missing-lineage")
        .await
        .unwrap()
        .is_none());
    assert!(sessions::find_generation_by_id(&pool, "missing-generation")
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn steward_run_metadata_and_project_key_roundtrip() {
    let pool = test_pool().await;
    let idea = Idea {
        id: IdeaId::new(),
        title: "Steward idea".into(),
        body: "body".into(),
        workspace_root_path: None,
        project_key: Some("crypto-savings".into()),
        status: IdeaStatus::Active,
        created_at: Utc::now(),
        archived_at: None,
    };
    ideas::insert(&pool, &idea).await.unwrap();

    let loaded_idea = ideas::find_by_id(&pool, idea.id)
        .await
        .unwrap()
        .expect("idea should roundtrip");
    assert_eq!(loaded_idea.project_key.as_deref(), Some("crypto-savings"));

    let run = Run {
        id: RunId::new(),
        idea_id: idea.id,
        status: RunStatus::Running,
        workflow_id: "wf-steward".into(),
        workflow_title: "Steward Workflow".into(),
        workspace_root: "/tmp/ws".into(),
        artifact_root: "/tmp/art".into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        cancellation_settlement_log: None,
        current_state: Some("start".into()),
        workflow_yaml_path: Some("/tmp/workflow.yaml".into()),
        agent_catalog_yaml_path: Some("/tmp/catalog.yaml".into()),
        worktree_root: None,
        base_branch: None,
        base_revision: None,
        target_branch: None,
        delivery_configuration_json: None,
        delivery_preflight_json: None,
        workflow_family: Some("mvp_live".into()),
        project_key: Some("crypto-savings".into()),
        risk_class: Some("high".into()),
        stack: Some("swiftui".into()),
        workflow_snapshot_hash: Some("a".repeat(64)),
        catalog_snapshot_hash: Some("b".repeat(64)),
        workflow_snapshot_json: Some(r#"{"workflow":{"id":"wf-steward"}}"#.into()),
        catalog_snapshot_json: Some(r#"{"agents":[]}"#.into()),
        drift_detected_at: None,
        drift_details_json: None,
        chainworks_meta_root: None,
    };
    runs::insert(&pool, &run).await.unwrap();

    let loaded = runs::find_by_id(&pool, run.id)
        .await
        .unwrap()
        .expect("run should roundtrip");
    assert_eq!(loaded.workflow_family.as_deref(), Some("mvp_live"));
    assert_eq!(loaded.project_key.as_deref(), Some("crypto-savings"));
    assert_eq!(loaded.risk_class.as_deref(), Some("high"));
    assert_eq!(loaded.stack.as_deref(), Some("swiftui"));
    assert_eq!(loaded.workflow_snapshot_hash, Some("a".repeat(64)));
    assert_eq!(loaded.catalog_snapshot_hash, Some("b".repeat(64)));
    assert!(loaded
        .workflow_snapshot_json
        .as_deref()
        .unwrap()
        .contains("wf-steward"));
    assert!(loaded
        .catalog_snapshot_json
        .as_deref()
        .unwrap()
        .contains("agents"));
}

#[tokio::test]
async fn steward_analysis_schema_roundtrips_p049_contract() {
    let pool = test_pool().await;
    let now = Utc::now();
    let analysis = StewardAnalysis {
        id: "analysis-db".into(),
        created_at: now,
        window_start: now,
        window_end: now,
        run_count: 7,
        cohort_keys_json: serde_json::json!({
            "workflow_family": "mvp_live",
            "risk_class": "high"
        })
        .to_string(),
        cohort_quality: CohortQuality::Acceptable,
        status: StewardAnalysisStatus::Completed,
        degradation_count: 2,
        improvement_count: 1,
        workflow_snapshot_artifact_hash: "workflow-artifact-hash".into(),
        agent_catalog_snapshot_hash: "catalog-hash".into(),
        steward_config_snapshot_hash: "config-hash".into(),
        metrics_snapshot_artifact_id: Some("steward/metrics-window.json".into()),
        baseline_snapshot_artifact_id: Some("steward/baseline-window.json".into()),
        agent_catalog_snapshot_artifact_id: Some("steward/catalog-snapshot.json".into()),
        workflow_snapshot_artifact_id: Some("steward/workflow-snapshot.json".into()),
        config_change_log_artifact_id: Some("steward/config-change-log.json".into()),
        health_report_artifact_id: Some("steward/reports/health-report.json".into()),
        degradation_alert_artifact_id: Some("steward/reports/degradation-alert.json".into()),
        agent_tuning_artifact_id: Some("steward/proposals/agent-tuning.json".into()),
        workflow_tuning_artifact_id: Some("steward/proposals/workflow-tuning.json".into()),
        experiment_plan_artifact_id: Some("steward/proposals/experiment-plan.json".into()),
        audit_report_artifact_id: Some("steward/reports/audit-report.json".into()),
        trigger_reason: "manual".into(),
        error_summary: None,
    };
    steward::insert_analysis(&pool, &analysis).await.unwrap();
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "Steward DB idea".into(),
            body: "body".into(),
            workspace_root_path: None,
            project_key: Some("crypto-savings".into()),
            status: IdeaStatus::Active,
            created_at: now,
            archived_at: None,
        },
    )
    .await
    .unwrap();
    runs::insert(
        &pool,
        &Run {
            id: run_id,
            idea_id,
            status: RunStatus::Completed,
            workflow_id: "wf".into(),
            workflow_title: "Workflow".into(),
            workspace_root: "/tmp/ws".into(),
            artifact_root: "/tmp/art".into(),
            started_at: now,
            completed_at: Some(now),
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
            workflow_family: Some("mvp_live".into()),
            project_key: Some("crypto-savings".into()),
            risk_class: Some("high".into()),
            stack: Some("swiftui".into()),
            workflow_snapshot_hash: Some("a".repeat(64)),
            catalog_snapshot_hash: Some("b".repeat(64)),
            workflow_snapshot_json: Some("{}".into()),
            catalog_snapshot_json: Some("{}".into()),
            drift_detected_at: None,
            drift_details_json: None,
            chainworks_meta_root: None,
        },
    )
    .await
    .unwrap();
    steward::insert_run_link(
        &pool,
        &StewardAnalysisRunLink {
            id: "link-db".into(),
            analysis_id: analysis.id.clone(),
            run_id: run_id.to_string(),
            role: "implicated".into(),
        },
    )
    .await
    .unwrap();
    steward::insert_recommendation(
        &pool,
        &StewardRecommendation {
            id: "rec-db".into(),
            analysis_id: analysis.id.clone(),
            created_at: now,
            category: "degradation".into(),
            summary: "Regression detected".into(),
            target_metric: "failed_run_rate".into(),
            confidence_level: "medium".into(),
            status: "proposed".into(),
            source_artifact_name: Some("deterministic_signal".into()),
            decision_comment: None,
            decided_at: None,
        },
    )
    .await
    .unwrap();

    let loaded = steward::find_analysis(&pool, &analysis.id)
        .await
        .unwrap()
        .expect("analysis should roundtrip");
    assert_eq!(loaded.run_count, 7);
    assert_eq!(loaded.degradation_count, 2);
    assert_eq!(loaded.trigger_reason, "manual");
    assert_eq!(
        loaded.workflow_snapshot_artifact_id.as_deref(),
        Some("steward/workflow-snapshot.json")
    );
    let links = steward::list_run_links(&pool, &analysis.id).await.unwrap();
    assert_eq!(links[0].role, "implicated");
    let recommendations = steward::list_recommendations(&pool, &analysis.id)
        .await
        .unwrap();
    assert_eq!(recommendations[0].target_metric, "failed_run_rate");
    assert_eq!(
        recommendations[0].source_artifact_name.as_deref(),
        Some("deterministic_signal")
    );
}

#[tokio::test]
async fn agent_execution_provenance_round_trips_without_lineage_joins() {
    let pool = test_pool().await;
    let idea = Idea {
        id: IdeaId::new(),
        title: "Idea".into(),
        body: "body".into(),
        workspace_root_path: None,
        project_key: None,
        status: IdeaStatus::Active,
        created_at: Utc::now(),
        archived_at: None,
    };
    ideas::insert(&pool, &idea).await.unwrap();

    let run = Run {
        id: RunId::new(),
        idea_id: idea.id,
        status: RunStatus::Running,
        workflow_id: "wf-session".into(),
        workflow_title: "Session".into(),
        workspace_root: "/tmp/ws".into(),
        artifact_root: "/tmp/art".into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        cancellation_settlement_log: Some("[{\"agent_execution_id\":\"ae-1\"}]".into()),
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
    };
    runs::insert(&pool, &run).await.unwrap();

    let stage = StageExecution {
        id: StageExecutionId::new(),
        run_id: run.id,
        stage_id: "proposal".into(),
        label: "Proposal".into(),
        status: StageStatus::Running,
        iteration: 1,
        attempt_number: 1,
        settlement_kind: None,
        started_at: Utc::now(),
        completed_at: None,
        owner_agent: Some("proposal_writer".into()),
        provider: Some("claude".into()),
        model: None,
        stage_type: None,
        validation_failure_json: None,
        evidence_packet_json: None,
        recovery_snapshot_json: None,
        retry_reason: None,
    };
    stages::insert(&pool, &stage).await.unwrap();

    let execution = AgentExecution {
        id: AgentExecutionId::new(),
        stage_execution_id: stage.id,
        agent_id: "proposal_writer".into(),
        provider: "claude".into(),
        model: Some("sonnet".into()),
        started_at: Utc::now(),
        completed_at: None,
        status: AgentStatus::Running,
        owner_execution_lineage_id: Some("stage-execution-lineage-1".into()),
        session_lineage_id: Some("lineage-1".into()),
        session_generation_id: Some("generation-2".into()),
        rehydrated_from_checkpoint_artifact_id: Some("artifact-3".into()),
        invocation_owner_key: Some("run:agent:stage:task:lineage".into()),
        session_reuse_scope: Some("same_agent_family_within_run".into()),
        session_family_id: Some("proposal_authoring_loop".into()),
        session_reuse_disposition: Some("reused_after_resume".into()),
        session_reset_reason: Some("operator_reset".into()),
        backend_profile_id: None,
        requested_mcp_extensions_json: None,
        predicted_mcp_extensions_json: None,
        predicted_mcp_runtime_ids_json: None,
        actual_mcp_extensions_json: None,
        actual_mcp_runtime_ids_json: None,
        denied_mcp_extensions_json: None,
        mcp_blocking_issues_json: None,
        actual_mcp_observation_json: None,
        mcp_session_startup_latency_ms: None,
    };
    agent_executions::insert(&pool, &execution).await.unwrap();

    let found = agent_executions::find_by_stage(&pool, stage.id)
        .await
        .unwrap()
        .into_iter()
        .next()
        .expect("agent execution");

    assert_eq!(found.session_lineage_id.as_deref(), Some("lineage-1"));
    assert_eq!(
        found.owner_execution_lineage_id.as_deref(),
        Some("stage-execution-lineage-1")
    );
    assert_eq!(found.session_generation_id.as_deref(), Some("generation-2"));
    assert_eq!(
        found.rehydrated_from_checkpoint_artifact_id.as_deref(),
        Some("artifact-3")
    );
    assert_eq!(
        found.invocation_owner_key.as_deref(),
        Some("run:agent:stage:task:lineage")
    );
    assert_eq!(
        found.session_reuse_scope.as_deref(),
        Some("same_agent_family_within_run")
    );
    assert_eq!(
        found.session_family_id.as_deref(),
        Some("proposal_authoring_loop")
    );
    assert_eq!(
        found.session_reuse_disposition.as_deref(),
        Some("reused_after_resume")
    );
    assert_eq!(
        found.session_reset_reason.as_deref(),
        Some("operator_reset")
    );
}

#[tokio::test]
async fn proposal_048_persistence_fields_round_trip() {
    let pool = test_pool().await;
    let idea = Idea {
        id: IdeaId::new(),
        title: "Idea".into(),
        body: "body".into(),
        workspace_root_path: None,
        project_key: None,
        status: IdeaStatus::Active,
        created_at: Utc::now(),
        archived_at: None,
    };
    ideas::insert(&pool, &idea).await.unwrap();

    let run = Run {
        id: RunId::new(),
        idea_id: idea.id,
        status: RunStatus::Running,
        workflow_id: "wf-p048".into(),
        workflow_title: "P048".into(),
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
        delivery_preflight_json: Some(r#"{"passed":true}"#.into()),
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
    runs::insert(&pool, &run).await.unwrap();

    let stage = StageExecution {
        id: StageExecutionId::new(),
        run_id: run.id,
        stage_id: "stage-p048".into(),
        label: "Stage P048".into(),
        status: StageStatus::Failed,
        iteration: 1,
        attempt_number: 1,
        settlement_kind: Some(StageSettlementKind::Failed),
        started_at: Utc::now(),
        completed_at: Some(Utc::now()),
        owner_agent: Some("agent-p048".into()),
        provider: Some("codex".into()),
        model: Some("gpt-5.4".into()),
        stage_type: None,
        validation_failure_json: Some(r#"{"failureClass":"output_contract_mismatch"}"#.into()),
        evidence_packet_json: Some(r#"{"failure_summary":"bad output"}"#.into()),
        recovery_snapshot_json: Some(r#"{"action":"retry_failed_agent"}"#.into()),
        retry_reason: None,
    };
    stages::insert(&pool, &stage).await.unwrap();

    let execution = AgentExecution {
        id: AgentExecutionId::new(),
        stage_execution_id: stage.id,
        agent_id: "agent-p048".into(),
        provider: "codex".into(),
        model: Some("gpt-5.4".into()),
        started_at: Utc::now(),
        completed_at: Some(Utc::now()),
        status: AgentStatus::Failed,
        owner_execution_lineage_id: None,
        session_lineage_id: None,
        session_generation_id: None,
        rehydrated_from_checkpoint_artifact_id: None,
        invocation_owner_key: None,
        session_reuse_scope: None,
        session_family_id: None,
        session_reuse_disposition: None,
        session_reset_reason: None,
        backend_profile_id: Some("codex_with_mcp".into()),
        requested_mcp_extensions_json: Some(r#"["filesystem"]"#.into()),
        predicted_mcp_extensions_json: Some(r#"["filesystem"]"#.into()),
        predicted_mcp_runtime_ids_json: Some(r#"["fs-runtime"]"#.into()),
        actual_mcp_extensions_json: Some(r#"[]"#.into()),
        actual_mcp_runtime_ids_json: Some(r#"[]"#.into()),
        denied_mcp_extensions_json: Some(r#"["filesystem"]"#.into()),
        mcp_blocking_issues_json: Some(r#"["missing registry entry: filesystem"]"#.into()),
        actual_mcp_observation_json: Some(
            r#"{"source":"not_started_blocked_before_session_new"}"#.into(),
        ),
        mcp_session_startup_latency_ms: Some(42),
    };
    agent_executions::insert(&pool, &execution).await.unwrap();

    let found_run = runs::find_by_id(&pool, run.id).await.unwrap().unwrap();
    assert_eq!(
        found_run.delivery_preflight_json,
        run.delivery_preflight_json
    );

    let found_stage = stages::find_by_id(&pool, stage.id).await.unwrap().unwrap();
    assert_eq!(
        found_stage.validation_failure_json,
        stage.validation_failure_json
    );
    assert_eq!(found_stage.evidence_packet_json, stage.evidence_packet_json);
    assert_eq!(
        found_stage.recovery_snapshot_json,
        stage.recovery_snapshot_json
    );

    let found_execution = agent_executions::find_by_id(&pool, execution.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        found_execution.backend_profile_id,
        execution.backend_profile_id
    );
    assert_eq!(
        found_execution.requested_mcp_extensions_json,
        execution.requested_mcp_extensions_json
    );
    assert_eq!(
        found_execution.predicted_mcp_runtime_ids_json,
        execution.predicted_mcp_runtime_ids_json
    );
    assert_eq!(
        found_execution.mcp_blocking_issues_json,
        execution.mcp_blocking_issues_json
    );
    assert_eq!(
        found_execution.actual_mcp_observation_json,
        execution.actual_mcp_observation_json
    );
    assert_eq!(
        found_execution.mcp_session_startup_latency_ms,
        execution.mcp_session_startup_latency_ms
    );
}

#[tokio::test]
async fn stage_projection_validation_flag_is_attempt_scoped() {
    let pool = test_pool().await;
    let idea = Idea {
        id: IdeaId::new(),
        title: "Idea".into(),
        body: "body".into(),
        workspace_root_path: None,
        project_key: None,
        status: IdeaStatus::Active,
        created_at: Utc::now(),
        archived_at: None,
    };
    ideas::insert(&pool, &idea).await.unwrap();

    let run = Run {
        id: RunId::new(),
        idea_id: idea.id,
        status: RunStatus::Running,
        workflow_id: "wf-validation".into(),
        workflow_title: "Validation".into(),
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
    };
    runs::insert(&pool, &run).await.unwrap();

    let failed_attempt = StageExecution {
        id: StageExecutionId::new(),
        run_id: run.id,
        stage_id: "proposal_review".into(),
        label: "Proposal review".into(),
        status: StageStatus::Failed,
        iteration: 1,
        attempt_number: 1,
        settlement_kind: Some(StageSettlementKind::Failed),
        started_at: Utc::now(),
        completed_at: Some(Utc::now()),
        owner_agent: Some("reviewer".into()),
        provider: Some("claude".into()),
        model: None,
        stage_type: None,
        validation_failure_json: None,
        evidence_packet_json: None,
        recovery_snapshot_json: None,
        retry_reason: None,
    };
    let successful_retry = StageExecution {
        id: StageExecutionId::new(),
        run_id: run.id,
        stage_id: "proposal_review".into(),
        label: "Proposal review".into(),
        status: StageStatus::Completed,
        iteration: 2,
        attempt_number: 2,
        settlement_kind: Some(StageSettlementKind::Completed),
        started_at: Utc::now(),
        completed_at: Some(Utc::now()),
        owner_agent: Some("reviewer".into()),
        provider: Some("claude".into()),
        model: None,
        stage_type: None,
        validation_failure_json: None,
        evidence_packet_json: None,
        recovery_snapshot_json: None,
        retry_reason: None,
    };
    stages::insert(&pool, &failed_attempt).await.unwrap();
    stages::insert(&pool, &successful_retry).await.unwrap();

    let failed_agent_execution = AgentExecution {
        id: AgentExecutionId::new(),
        stage_execution_id: failed_attempt.id,
        agent_id: "reviewer".into(),
        provider: "claude".into(),
        model: None,
        started_at: Utc::now(),
        completed_at: Some(Utc::now()),
        status: AgentStatus::Failed,
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
        mcp_session_startup_latency_ms: None,
    };
    let retry_agent_execution = AgentExecution {
        id: AgentExecutionId::new(),
        stage_execution_id: successful_retry.id,
        agent_id: "reviewer".into(),
        provider: "claude".into(),
        model: None,
        started_at: Utc::now(),
        completed_at: Some(Utc::now()),
        status: AgentStatus::Completed,
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
        mcp_session_startup_latency_ms: None,
    };
    agent_executions::insert(&pool, &failed_agent_execution)
        .await
        .unwrap();
    agent_executions::insert(&pool, &retry_agent_execution)
        .await
        .unwrap();

    let artifact = Artifact {
        id: ArtifactId::new(),
        run_id: run.id,
        stage_id: failed_attempt.stage_id.clone(),
        agent_id: "reviewer".into(),
        name: "validation_failure_reviewer".into(),
        contract_id: "validation_failure_record".into(),
        format: ArtifactFormat::Json,
        file_path: "/tmp/art/validation-failure.json".into(),
        checksum_sha256: None,
        size_bytes: None,
        provider: "claude".into(),
        model: None,
        created_at: Utc::now(),
        is_pinned: false,
        report_kind: Some("validation_failure".into()),
        report_version: Some(1),
    };
    artifacts::insert(&pool, &artifact).await.unwrap();

    validation::insert(
        &pool,
        &ValidationFailureRecord {
            id: "55555555-5555-5555-5555-555555555555".into(),
            artifact_id: artifact.id,
            timestamp: Utc::now(),
            agent_id: "reviewer".into(),
            stage_id: failed_attempt.stage_id.clone(),
            stage_execution_id: failed_attempt.id,
            agent_execution_id: failed_agent_execution.id,
            run_id: run.id,
            output_results: vec![OutputValidationResult {
                output_name: "proposal_review".into(),
                contract_id: Some("proposal_review_v1".into()),
                status: ValidationStatus::Failed,
                missing_fields: vec!["summary".into()],
                validation_error: Some("Missing required fields: summary".into()),
                raw_payload_size: 12,
            }],
            failure_summary: "proposal_review: Missing required fields: summary".into(),
            failure_class: ValidationFailureClass::OutputContractMismatch,
            contract_metadata: vec![ContractValidationMetadata {
                output_name: "proposal_review".into(),
                contract_id: "proposal_review_v1".into(),
                machine_format: "json".into(),
                validation_mode: "strict_structured".into(),
                required_field_count: 1,
                raw_artifact_name: Some("proposal_review_raw".into()),
                normalized_artifact_name: Some("proposal_review".into()),
            }],
            raw_output_exists: true,
            receipt_exists: false,
            transcript_exists: true,
            recovery_recommendation: RecoveryRecommendation {
                action: "retry_failed_agent".into(),
                explanation: "Retry the failed agent.".into(),
            },
        },
    )
    .await
    .unwrap();

    projections::rebuild_all_for_run(&pool, run.id)
        .await
        .unwrap();
    let rows = projections::list_stages_projection(&pool, &run.id.to_string())
        .await
        .unwrap();

    let failed_row = rows
        .iter()
        .find(|row| row.id == failed_attempt.id.to_string())
        .expect("failed attempt row");
    let retry_row = rows
        .iter()
        .find(|row| row.id == successful_retry.id.to_string())
        .expect("retry row");

    assert!(failed_row.has_validation_failure);
    assert!(!retry_row.has_validation_failure);
}

#[tokio::test]
async fn test_idea_insert_and_find() {
    let pool = test_pool().await;
    let idea = Idea {
        id: IdeaId::new(),
        title: "Test Idea".into(),
        body: "Body content".into(),
        workspace_root_path: None,
        project_key: None,
        status: IdeaStatus::Draft,
        created_at: Utc::now(),
        archived_at: None,
    };
    ideas::insert(&pool, &idea).await.expect("insert failed");
    let found = ideas::find_by_id(&pool, idea.id)
        .await
        .expect("find failed");
    assert!(found.is_some());
    assert_eq!(found.unwrap().title, "Test Idea");
}

#[tokio::test]
async fn test_run_insert_and_find() {
    let pool = test_pool().await;
    let idea = Idea {
        id: IdeaId::new(),
        title: "Idea for run".into(),
        body: "body".into(),
        workspace_root_path: None,
        project_key: None,
        status: IdeaStatus::Active,
        created_at: Utc::now(),
        archived_at: None,
    };
    ideas::insert(&pool, &idea).await.unwrap();

    let run = Run {
        id: RunId::new(),
        idea_id: idea.id,
        status: RunStatus::Pending,
        workflow_id: "wf1".into(),
        workflow_title: "Workflow 1".into(),
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
    };
    runs::insert(&pool, &run).await.unwrap();
    let found = runs::find_by_id(&pool, run.id).await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().status, RunStatus::Pending);
}

#[tokio::test]
async fn test_run_status_update() {
    let pool = test_pool().await;
    let idea = Idea {
        id: IdeaId::new(),
        title: "Idea".into(),
        body: "body".into(),
        workspace_root_path: None,
        project_key: None,
        status: IdeaStatus::Active,
        created_at: Utc::now(),
        archived_at: None,
    };
    ideas::insert(&pool, &idea).await.unwrap();
    let run = Run {
        id: RunId::new(),
        idea_id: idea.id,
        status: RunStatus::Pending,
        workflow_id: "wf1".into(),
        workflow_title: "WF".into(),
        workspace_root: "/tmp".into(),
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
    };
    runs::insert(&pool, &run).await.unwrap();
    runs::update_status(&pool, run.id, RunStatus::Running)
        .await
        .unwrap();
    let found = runs::find_by_id(&pool, run.id).await.unwrap().unwrap();
    assert_eq!(found.status, RunStatus::Running);
}

// ---------------------------------------------------------------------------
// Parity harness (ARCH-002 / P027)
// Proves that projection layer accurately reflects canonical run/stage state.
// ---------------------------------------------------------------------------

/// After rebuild_all_for_run, run_summaries must mirror canonical table counts.
#[tokio::test]
async fn test_projection_parity_after_rebuild() {
    let pool = test_pool().await;

    let idea = Idea {
        id: IdeaId::new(),
        title: "Parity idea".into(),
        body: "body".into(),
        workspace_root_path: None,
        project_key: None,
        status: IdeaStatus::Active,
        created_at: Utc::now(),
        archived_at: None,
    };
    ideas::insert(&pool, &idea).await.unwrap();

    let run = Run {
        id: RunId::new(),
        idea_id: idea.id,
        status: RunStatus::Running,
        workflow_id: "wf-parity".into(),
        workflow_title: "Parity Workflow".into(),
        workspace_root: "/tmp/parity".into(),
        artifact_root: "/tmp/parity/art".into(),
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
    };
    runs::insert(&pool, &run).await.unwrap();

    // Insert one completed stage and one failed stage.
    let completed_stage = StageExecution {
        id: StageExecutionId::new(),
        run_id: run.id,
        stage_id: "stage_a".into(),
        label: "Stage A".into(),
        status: StageStatus::Completed,
        iteration: 1,
        attempt_number: 1,
        settlement_kind: Some(domain::stage::StageSettlementKind::Completed),
        started_at: Utc::now(),
        completed_at: Some(Utc::now()),
        owner_agent: None,
        provider: None,
        model: None,
        stage_type: None,
        validation_failure_json: None,
        evidence_packet_json: None,
        recovery_snapshot_json: None,
        retry_reason: None,
    };
    let failed_stage = StageExecution {
        id: StageExecutionId::new(),
        run_id: run.id,
        stage_id: "stage_b".into(),
        label: "Stage B".into(),
        status: StageStatus::Failed,
        iteration: 1,
        attempt_number: 1,
        settlement_kind: Some(domain::stage::StageSettlementKind::Failed),
        started_at: Utc::now(),
        completed_at: Some(Utc::now()),
        owner_agent: None,
        provider: None,
        model: None,
        stage_type: None,
        validation_failure_json: None,
        evidence_packet_json: None,
        recovery_snapshot_json: None,
        retry_reason: None,
    };
    stages::insert(&pool, &completed_stage).await.unwrap();
    stages::insert(&pool, &failed_stage).await.unwrap();

    // Rebuild projections.
    projections::rebuild_all_for_run(&pool, run.id)
        .await
        .unwrap();

    // Query via projection layer and verify counts match canonical state.
    let projection_rows = projections::list_active_projection(&pool).await.unwrap();
    let row = projection_rows
        .iter()
        .find(|r| r.id == run.id.to_string())
        .expect("run missing from projection layer after rebuild");

    assert_eq!(
        row.status,
        run.status.to_string(),
        "projection status must match canonical status"
    );
    assert_eq!(row.total_stages, 2, "total_stages must count both stages");
    assert_eq!(
        row.completed_stages, 1,
        "completed_stages must reflect one completed stage"
    );
    assert_eq!(
        row.failed_stages, 1,
        "failed_stages must reflect one failed stage"
    );
    assert_eq!(
        row.pending_approvals, 0,
        "pending_approvals must be zero without approvals"
    );

    // Stage projection parity.
    let stage_rows = projections::list_stages_projection(&pool, &run.id.to_string())
        .await
        .unwrap();
    assert_eq!(
        stage_rows.len(),
        2,
        "stage projection must surface both stages"
    );

    let stage_a = stage_rows.iter().find(|s| s.stage_id == "stage_a").unwrap();
    assert_eq!(stage_a.status, StageStatus::Completed.to_string());

    let stage_b = stage_rows.iter().find(|s| s.stage_id == "stage_b").unwrap();
    assert_eq!(stage_b.status, StageStatus::Failed.to_string());
}

// ---------------------------------------------------------------------------
// File-backed SQLite durability proof (REQ-002 / READY-001)
//
// Proves that canonical state written to a file-backed SQLite database survives
// process restart: data is written, the pool is closed, a new pool is opened on
// the same file, and all entities are still readable with projections intact.
// ---------------------------------------------------------------------------

/// Write a full workflow slice to a file-backed SQLite database, close the
/// connection, reopen it, and verify all entities and projections are durable.
#[tokio::test]
async fn test_file_backed_sqlite_durability_across_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let db_file = tmp.path().join("parity.db");
    let db_url = format!("sqlite://{}", db_file.display());

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_id = StageExecutionId::new();
    let artifact_id = ArtifactId::new();

    // ── Write phase (simulates first daemon boot) ─────────────────────────────
    {
        let pool = create_pool(&db_url).await.expect("first open failed");

        let idea = Idea {
            id: idea_id,
            title: "Durable idea".into(),
            body: "body".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        };
        ideas::insert(&pool, &idea).await.unwrap();

        let run = Run {
            id: run_id,
            idea_id,
            status: RunStatus::Running,
            workflow_id: "wf-durable".into(),
            workflow_title: "Durable Workflow".into(),
            workspace_root: tmp.path().to_string_lossy().into_owned(),
            artifact_root: tmp.path().to_string_lossy().into_owned(),
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
        };
        runs::insert(&pool, &run).await.unwrap();

        let stage = StageExecution {
            id: stage_id,
            run_id,
            stage_id: "build".into(),
            label: "Build".into(),
            status: StageStatus::Completed,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: Some(StageSettlementKind::Completed),
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            owner_agent: None,
            provider: None,
            model: None,
            stage_type: None,
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        };
        stages::insert(&pool, &stage).await.unwrap();

        let artifact = Artifact {
            id: artifact_id,
            run_id,
            stage_id: "build".into(),
            agent_id: "claude".into(),
            name: "report.json".into(),
            contract_id: "claude.output".into(),
            format: ArtifactFormat::Json,
            file_path: "/tmp/report.json".into(),
            checksum_sha256: None,
            size_bytes: None,
            provider: "claude".into(),
            model: None,
            created_at: Utc::now(),
            is_pinned: false,
            report_kind: Some("execution_report".into()),
            report_version: Some(1),
        };
        artifacts::insert(&pool, &artifact).await.unwrap();
        projections::rebuild_all_for_run(&pool, run_id)
            .await
            .unwrap();

        // Pool drops here — simulates process exit / connection close
        pool.close().await;
    }

    assert!(
        db_file.exists(),
        "SQLite file must persist after pool close"
    );

    // ── Read phase (simulates daemon restart) ─────────────────────────────────
    {
        let pool = create_pool(&db_url).await.expect("reopen failed");

        // Canonical repos must return the written entities
        let found_idea = ideas::find_by_id(&pool, idea_id).await.unwrap();
        assert!(found_idea.is_some(), "idea must survive pool close/reopen");
        assert_eq!(found_idea.unwrap().title, "Durable idea");

        let found_run = runs::find_by_id(&pool, run_id).await.unwrap();
        assert!(found_run.is_some(), "run must survive pool close/reopen");
        assert_eq!(found_run.unwrap().status, RunStatus::Running);

        let run_stages = stages::list_by_run(&pool, run_id).await.unwrap();
        assert_eq!(run_stages.len(), 1, "stage must survive pool close/reopen");
        assert_eq!(run_stages[0].status, StageStatus::Completed);

        let run_artifacts = artifacts::list_by_run(&pool, run_id).await.unwrap();
        assert_eq!(
            run_artifacts.len(),
            1,
            "artifact must survive pool close/reopen"
        );
        assert_eq!(run_artifacts[0].name, "report.json");

        // Projections survive and report correct values
        let proj_rows = projections::list_active_projection(&pool).await.unwrap();
        let proj = proj_rows
            .iter()
            .find(|r| r.id == run_id.to_string())
            .expect("run projection must survive restart");
        assert_eq!(proj.total_stages, 1);
        assert_eq!(proj.completed_stages, 1);
        // Verify artifact survives via artifact projection
        let art_proj = projections::list_artifacts_projection(&pool, &run_id.to_string())
            .await
            .unwrap();
        assert!(
            !art_proj.is_empty(),
            "artifact projection must survive restart"
        );

        pool.close().await;
    }
}

// ---------------------------------------------------------------------------
// Projection parity comparison harness (REQ-005 / PROD-001)
//
// Proves that the projection layer accurately mirrors the canonical repository
// values across all four projection surfaces (run, stages, artifacts, approvals).
// This is the in-process parity comparison tool called for by the proposal.
// ---------------------------------------------------------------------------

/// Compare projection-layer output against canonical repo state for a multi-surface
/// workflow slice: run summary, stage list, artifact index.
/// All projection counts must exactly match the canonical table counts after rebuild.
#[tokio::test]
async fn test_projection_parity_matches_canonical_repo_values() {
    let pool = create_pool("sqlite::memory:").await.unwrap();

    let idea_id = IdeaId::new();
    let run_id = RunId::new();

    let idea = Idea {
        id: idea_id,
        title: "Parity harness idea".into(),
        body: "body".into(),
        workspace_root_path: None,
        project_key: None,
        status: IdeaStatus::Active,
        created_at: Utc::now(),
        archived_at: None,
    };
    ideas::insert(&pool, &idea).await.unwrap();

    let run = Run {
        id: run_id,
        idea_id,
        status: RunStatus::Running,
        workflow_id: "wf-parity2".into(),
        workflow_title: "Parity Harness".into(),
        workspace_root: "/tmp/ph".into(),
        artifact_root: "/tmp/ph/art".into(),
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
    };
    runs::insert(&pool, &run).await.unwrap();

    // Insert three stages with distinct statuses
    let stage_specs: &[(&str, StageStatus, Option<StageSettlementKind>)] = &[
        (
            "alpha",
            StageStatus::Completed,
            Some(StageSettlementKind::Completed),
        ),
        (
            "beta",
            StageStatus::Failed,
            Some(StageSettlementKind::Failed),
        ),
        ("gamma", StageStatus::Pending, None),
    ];

    for (sid, status, kind) in stage_specs {
        let s = StageExecution {
            id: StageExecutionId::new(),
            run_id,
            stage_id: (*sid).to_string(),
            label: sid.to_uppercase(),
            status: status.clone(),
            iteration: 1,
            attempt_number: 1,
            settlement_kind: kind.clone(),
            started_at: Utc::now(),
            completed_at: if kind.is_some() {
                Some(Utc::now())
            } else {
                None
            },
            owner_agent: None,
            provider: None,
            model: None,
            stage_type: None,
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        };
        stages::insert(&pool, &s).await.unwrap();
    }

    // Insert two artifacts
    for n in 0u8..2 {
        let art = Artifact {
            id: ArtifactId::new(),
            run_id,
            stage_id: "alpha".into(),
            agent_id: "claude".into(),
            name: format!("artifact_{n}.json"),
            contract_id: "claude.output".into(),
            format: ArtifactFormat::Json,
            file_path: format!("/tmp/ph/artifact_{n}.json"),
            checksum_sha256: None,
            size_bytes: None,
            provider: "claude".into(),
            model: None,
            created_at: Utc::now(),
            is_pinned: false,
            report_kind: None,
            report_version: None,
        };
        artifacts::insert(&pool, &art).await.unwrap();
    }

    // Rebuild projections
    projections::rebuild_all_for_run(&pool, run_id)
        .await
        .unwrap();

    // ── Run summary projection vs canonical ──────────────────────────────────
    let canonical_stages = stages::list_by_run(&pool, run_id).await.unwrap();
    let canonical_artifacts = artifacts::list_by_run(&pool, run_id).await.unwrap();

    let proj_rows = projections::list_active_projection(&pool).await.unwrap();
    let proj = proj_rows
        .iter()
        .find(|r| r.id == run_id.to_string())
        .expect("run must appear in projection after rebuild");

    assert_eq!(
        proj.total_stages as usize,
        canonical_stages.len(),
        "total_stages projection must match canonical stage count"
    );
    assert_eq!(
        proj.completed_stages as usize,
        canonical_stages
            .iter()
            .filter(|s| s.status == StageStatus::Completed)
            .count(),
        "completed_stages projection must match canonical count"
    );
    assert_eq!(
        proj.failed_stages as usize,
        canonical_stages
            .iter()
            .filter(|s| s.status == StageStatus::Failed)
            .count(),
        "failed_stages projection must match canonical count"
    );
    // has_artifacts is surfaced per-stage (StageSummaryRow), not on RunProjectionRow.
    // Verify via artifact projection count instead.
    let art_proj = projections::list_artifacts_projection(&pool, &run_id.to_string())
        .await
        .unwrap();
    assert_eq!(
        art_proj.len(),
        canonical_artifacts.len(),
        "artifact projection count must match canonical artifact count (has_artifacts parity)"
    );

    // ── Stage projection vs canonical ────────────────────────────────────────
    let stage_proj = projections::list_stages_projection(&pool, &run_id.to_string())
        .await
        .unwrap();
    assert_eq!(
        stage_proj.len(),
        canonical_stages.len(),
        "stage projection row count must match canonical stage count"
    );
    for canonical in &canonical_stages {
        let proj_stage = stage_proj
            .iter()
            .find(|s| s.stage_id == canonical.stage_id)
            .unwrap_or_else(|| {
                panic!("stage {} missing from stage projection", canonical.stage_id)
            });
        assert_eq!(
            proj_stage.status,
            canonical.status.to_string(),
            "stage projection status must match canonical for {}",
            canonical.stage_id
        );
    }

    // ── Artifact projection vs canonical ─────────────────────────────────────
    let artifact_proj = projections::list_artifacts_projection(&pool, &run_id.to_string())
        .await
        .unwrap();
    assert_eq!(
        artifact_proj.len(),
        canonical_artifacts.len(),
        "artifact projection row count must match canonical artifact count"
    );
}

/// Projection list without a prior rebuild still returns runs (zero counts).
#[tokio::test]
async fn test_projection_list_before_rebuild_returns_run_with_zero_counts() {
    let pool = test_pool().await;

    let idea = Idea {
        id: IdeaId::new(),
        title: "Cold idea".into(),
        body: "body".into(),
        workspace_root_path: None,
        project_key: None,
        status: IdeaStatus::Active,
        created_at: Utc::now(),
        archived_at: None,
    };
    ideas::insert(&pool, &idea).await.unwrap();

    let run = Run {
        id: RunId::new(),
        idea_id: idea.id,
        status: RunStatus::Pending,
        workflow_id: "wf-cold".into(),
        workflow_title: "Cold Workflow".into(),
        workspace_root: "/tmp/cold".into(),
        artifact_root: "/tmp/cold/art".into(),
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
    };
    runs::insert(&pool, &run).await.unwrap();

    // No rebuild — projection layer should still surface the run via LEFT JOIN.
    let rows = projections::list_active_projection(&pool).await.unwrap();
    let row = rows
        .iter()
        .find(|r| r.id == run.id.to_string())
        .expect("run must appear in projection list even before first rebuild");

    assert_eq!(row.total_stages, 0);
    assert_eq!(row.completed_stages, 0);
    assert_eq!(row.failed_stages, 0);
}

#[tokio::test]
async fn run_projection_derives_cancellation_settlement_summary_from_canonical_log() {
    let pool = test_pool().await;
    let idea = Idea {
        id: IdeaId::new(),
        title: "Idea".into(),
        body: "body".into(),
        workspace_root_path: None,
        project_key: None,
        status: IdeaStatus::Active,
        created_at: Utc::now(),
        archived_at: None,
    };
    ideas::insert(&pool, &idea).await.unwrap();

    let run = Run {
        id: RunId::new(),
        idea_id: idea.id,
        status: RunStatus::Cancelled,
        workflow_id: "wf-cancel".into(),
        workflow_title: "Cancelled".into(),
        workspace_root: "/tmp/ws".into(),
        artifact_root: "/tmp/art".into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: Some(Utc::now()),
        cancellation_settled_at: Some(Utc::now()),
        cancellation_settlement_log: Some(
            serde_json::json!([
                {
                    "agent_execution_id": "ae-1",
                    "agent_id": "proposal_writer",
                    "prior_status": "running",
                    "terminal_status": "cancelled",
                    "session_close_attempted": true,
                    "session_close_succeeded": true,
                    "settled_at": "2026-04-15T10:00:00Z"
                },
                {
                    "agent_execution_id": "ae-2",
                    "agent_id": "reviewer",
                    "prior_status": "running",
                    "terminal_status": "cancelled",
                    "session_close_attempted": true,
                    "session_close_succeeded": false,
                    "settled_at": "2026-04-15T10:00:02Z"
                }
            ])
            .to_string(),
        ),
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
    };
    runs::insert(&pool, &run).await.unwrap();

    projections::rebuild_all_for_run(&pool, run.id)
        .await
        .unwrap();
    let found = projections::find_run_projection(&pool, &run.id.to_string())
        .await
        .unwrap()
        .expect("run projection");

    assert_eq!(
        found.cancellation_settlement_summary.as_deref(),
        Some("2/2 agents settled, 1 sessions closed")
    );
}

#[tokio::test]
async fn rebuild_all_for_run_refreshes_run_state_projection_status() {
    let pool = test_pool().await;
    let idea = Idea {
        id: IdeaId::new(),
        title: "Idea".into(),
        body: "body".into(),
        workspace_root_path: None,
        project_key: None,
        status: IdeaStatus::Active,
        created_at: Utc::now(),
        archived_at: None,
    };
    ideas::insert(&pool, &idea).await.unwrap();

    let run = Run {
        id: RunId::new(),
        idea_id: idea.id,
        status: RunStatus::Running,
        workflow_id: "wf-status".into(),
        workflow_title: "Status Projection".into(),
        workspace_root: "/tmp/ws".into(),
        artifact_root: "/tmp/art".into(),
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
        workflow_snapshot_json: None,
        catalog_snapshot_json: None,
        drift_detected_at: None,
        drift_details_json: None,
        chainworks_meta_root: None,
    };
    runs::insert(&pool, &run).await.unwrap();

    projections::rebuild_all_for_run(&pool, run.id)
        .await
        .unwrap();
    let initial = artifact_contract_repos::find_run_state_projection(&pool, run.id)
        .await
        .unwrap()
        .expect("run-state projection");
    let initial_state = initial.run_state_json;
    assert_eq!(initial_state["status"], "running");

    runs::update_status(&pool, run.id, RunStatus::Blocked)
        .await
        .unwrap();
    projections::rebuild_all_for_run(&pool, run.id)
        .await
        .unwrap();

    let refreshed = artifact_contract_repos::find_run_state_projection(&pool, run.id)
        .await
        .unwrap()
        .expect("run-state projection");
    let refreshed_state = refreshed.run_state_json;
    assert_eq!(refreshed_state["status"], "blocked");
}

/// R7 bar: executed approval_inbox projection-vs-canonical parity.
/// Proves list_pending_inbox_projection() equals filtering canonical approvals
/// repo by decision ∈ {Pending, Requested}.
#[tokio::test]
async fn test_approval_inbox_projection_parity_vs_canonical() {
    let pool = create_pool("sqlite::memory:").await.unwrap();

    let idea_id = IdeaId::new();
    let run_id = RunId::new();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "Approval parity idea".into(),
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
        &pool,
        &Run {
            id: run_id,
            idea_id,
            status: RunStatus::Running,
            workflow_id: "wf-approval-parity".into(),
            workflow_title: "Approval Parity".into(),
            workspace_root: "/tmp/ap".into(),
            artifact_root: "/tmp/ap/art".into(),
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
        },
    )
    .await
    .unwrap();

    // Insert approvals with ALL decision types so the projection filter
    // has a real job to do.
    let now = Utc::now();
    let specs: &[(&str, ApprovalDecision)] = &[
        ("gate_pending", ApprovalDecision::Pending),
        ("gate_requested", ApprovalDecision::Requested),
        ("gate_granted", ApprovalDecision::Granted),
        ("gate_rejected", ApprovalDecision::Rejected),
        ("gate_expired", ApprovalDecision::Expired),
    ];
    for (stage_id, decision) in specs {
        let a = Approval {
            id: ApprovalId::new(),
            run_id,
            stage_id: (*stage_id).to_string(),
            decision: decision.clone(),
            requested_at: now,
            decided_at: if matches!(
                decision,
                ApprovalDecision::Granted | ApprovalDecision::Rejected
            ) {
                Some(now)
            } else {
                None
            },
            comment: None,
            expires_at: None,
        };
        approvals::insert(&pool, &a).await.unwrap();
    }

    // Rebuild projections so approval_inbox reflects current canonical state.
    projections::rebuild_all_for_run(&pool, run_id)
        .await
        .unwrap();

    // ── Canonical: read approvals repo directly and filter in-memory ──
    let canonical_all = approvals::list_by_run(&pool, run_id).await.unwrap();
    assert_eq!(
        canonical_all.len(),
        specs.len(),
        "canonical repo must return all inserted approvals"
    );
    let mut canonical_pending: Vec<String> = canonical_all
        .iter()
        .filter(|a| {
            matches!(
                a.decision,
                ApprovalDecision::Pending | ApprovalDecision::Requested
            )
        })
        .map(|a| a.id.to_string())
        .collect();
    canonical_pending.sort();

    // ── Projection: read approval_inbox (projection layer) ──
    let projection_rows = projections::list_pending_inbox_projection(&pool)
        .await
        .unwrap();
    let mut projection_ids: Vec<String> = projection_rows.iter().map(|r| r.id.clone()).collect();
    projection_ids.sort();

    // ── Hard parity assertion ──
    assert_eq!(
        projection_ids, canonical_pending,
        "approval_inbox projection IDs must equal canonical filter: decision ∈ {{Pending, Requested}}"
    );

    // Field-level parity for each pending approval: projection decision
    // string must equal canonical decision's string form.
    for proj_row in &projection_rows {
        let canonical = canonical_all
            .iter()
            .find(|a| a.id.to_string() == proj_row.id)
            .expect("every projection row must have a canonical approval");
        assert_eq!(
            proj_row.decision,
            canonical.decision.to_string(),
            "projection decision must match canonical decision for approval {}",
            proj_row.id
        );
        assert_eq!(
            proj_row.stage_id, canonical.stage_id,
            "projection stage_id must match canonical stage_id for approval {}",
            proj_row.id
        );
        assert_eq!(
            proj_row.run_id,
            canonical.run_id.to_string(),
            "projection run_id must match canonical run_id for approval {}",
            proj_row.id
        );
    }

    // Granted/Rejected/Expired approvals MUST NOT appear in the projection.
    for approval in &canonical_all {
        let in_projection = projection_rows
            .iter()
            .any(|r| r.id == approval.id.to_string());
        let should_be_in_projection = matches!(
            approval.decision,
            ApprovalDecision::Pending | ApprovalDecision::Requested
        );
        assert_eq!(
            in_projection, should_be_in_projection,
            "approval {} ({}) presence in projection mismatches canonical filter",
            approval.id, approval.decision
        );
    }
}
