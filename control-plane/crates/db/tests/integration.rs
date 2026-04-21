use chrono::Utc;
use db::pool::create_pool;
use db::repos::{
    agent_executions, approvals, artifacts, command_journal, ideas, projections, runs, scheduler,
    stages, steward, validation, work_items,
};
use domain::agent::{AgentExecution, AgentStatus};
use domain::approval::{Approval, ApprovalDecision};
use domain::artifact::{Artifact, ArtifactFormat};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{AgentExecutionId, ApprovalId, ArtifactId, IdeaId, RunId, StageExecutionId};
use domain::provider::{InvokeAgentCapacityConfig, ProviderFamily};
use domain::run::{Run, RunStatus};
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

#[tokio::test]
async fn proposal_061_scheduler_foundation_tables_and_repos_round_trip() {
    use sqlx::Row;

    let pool = test_pool().await;
    let rows = sqlx::query(
        r#"SELECT name
           FROM sqlite_master
           WHERE type = 'table'
             AND name IN (
               'scheduler_service_state',
               'scheduler_queue_summaries',
               'scheduler_health_snapshots',
               'scheduler_db_writer_observations',
               'host_interruption_epochs',
               'host_interruption_affected_executions'
             )
           ORDER BY name ASC"#,
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let table_names: Vec<String> = rows.into_iter().map(|row| row.get("name")).collect();

    assert_eq!(
        table_names,
        vec![
            "host_interruption_affected_executions".to_string(),
            "host_interruption_epochs".to_string(),
            "scheduler_db_writer_observations".to_string(),
            "scheduler_health_snapshots".to_string(),
            "scheduler_queue_summaries".to_string(),
            "scheduler_service_state".to_string(),
        ]
    );
    assert!(scheduler::list_queue_summaries(&pool)
        .await
        .unwrap()
        .is_empty());

    let now = Utc::now();
    let state = scheduler::SchedulerServiceState {
        scope: "global".into(),
        scope_id: "".into(),
        last_served_at: Some(now),
        last_claimed_work_item_id: Some("work-123".into()),
        updated_at: now,
    };
    scheduler::upsert_service_state(&pool, &state)
        .await
        .unwrap();
    let stored_state = scheduler::get_service_state(&pool, "global", "")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored_state.scope, state.scope);
    assert_eq!(
        stored_state.last_claimed_work_item_id,
        state.last_claimed_work_item_id
    );

    let snapshot = scheduler::SchedulerHealthSnapshot {
        id: "snapshot-1".into(),
        queued_count: 3,
        oldest_queued_age_ms: 15_000,
        global_queue_depth: 7,
        active_agent_executions: 2,
        db_writer_wait_p95_ms: Some(42),
        command_latency_p95_ms_json: Some(
            r#"{"approve_stage":120,"retry_stage":180,"cancel_run":90}"#.into(),
        ),
        last_host_interruption_epoch_id: Some("epoch-latest".into()),
        sustained_backpressure_state: "clear".into(),
        stale_after_ms: 60_000,
        updated_at: now,
    };
    scheduler::insert_health_snapshot(&pool, &snapshot)
        .await
        .unwrap();
    let latest = scheduler::latest_health_snapshot(&pool)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(latest.id, snapshot.id);
    assert_eq!(latest.queued_count, snapshot.queued_count);
    assert_eq!(latest.db_writer_wait_p95_ms, Some(42));
    assert_eq!(
        latest.command_latency_p95_ms_json,
        snapshot.command_latency_p95_ms_json
    );
    assert_eq!(
        latest.last_host_interruption_epoch_id,
        Some("epoch-latest".into())
    );
    assert!(!latest.is_stale_at(now + chrono::Duration::milliseconds(59_999)));
    assert!(latest.is_stale_at(now + chrono::Duration::milliseconds(60_000)));
}

#[tokio::test]
async fn proposal_061_scheduler_refresh_populates_runtime_health_metrics() {
    let pool = test_pool().await;
    let now = Utc::now();

    for offset in 0..100 {
        scheduler::record_db_writer_wait_observation(
            &pool,
            "fixture_writer_wait",
            77,
            now - chrono::Duration::milliseconds(offset),
        )
        .await
        .unwrap();
    }

    for millis in 1..=20 {
        let created_at = now - chrono::Duration::seconds(60 - millis);
        let completed_at = created_at + chrono::Duration::milliseconds(millis);
        let id = format!("approve-latency-{millis}");
        command_journal::record(
            &pool,
            &id,
            "ApproveStage",
            "{}",
            None,
            created_at,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
        command_journal::complete_entry(&pool, &id, completed_at)
            .await
            .unwrap();
    }

    for millis in [5_i64, 10, 15, 20] {
        let created_at = now - chrono::Duration::seconds(120 - millis);
        let completed_at = created_at + chrono::Duration::milliseconds(millis);
        let id = format!("retry-latency-{millis}");
        command_journal::record(
            &pool,
            &id,
            "RetryStage",
            "{}",
            None,
            created_at,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
        command_journal::complete_entry(&pool, &id, completed_at)
            .await
            .unwrap();
    }

    scheduler::refresh_queue_summaries(&pool, &InvokeAgentCapacityConfig::default())
        .await
        .unwrap();

    let latest = scheduler::latest_health_snapshot(&pool)
        .await
        .unwrap()
        .expect("scheduler health snapshot");
    assert_eq!(latest.db_writer_wait_p95_ms, Some(77));

    let latency_json = latest
        .command_latency_p95_ms_json
        .expect("command latency p95 json");
    let latency: serde_json::Value = serde_json::from_str(&latency_json).unwrap();
    assert_eq!(latency["approve_stage"], 19);
    assert_eq!(latency["retry_stage"], 20);
    assert!(latency["cancel_run"].is_null());

    assert_eq!(
        scheduler::latest_db_writer_wait_p95_ms(&pool)
            .await
            .unwrap(),
        Some(77)
    );
    assert_eq!(
        scheduler::command_latency_p95_ms_json(&pool).await.unwrap(),
        Some(latency_json)
    );
}

#[tokio::test]
async fn proposal_061_sustained_backpressure_requires_two_snapshots_to_fire_and_clear() {
    let pool = test_pool().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();
    seed_minimal_stage(&pool, IdeaId::new(), run_id, stage_execution_id, now).await;
    insert_pending_invoke_agent_work(
        &pool,
        "sustained-backpressure-work",
        run_id,
        stage_execution_id,
        "codex",
        now - chrono::Duration::minutes(6),
    )
    .await;

    scheduler::refresh_queue_summaries(&pool, &InvokeAgentCapacityConfig::default())
        .await
        .unwrap();
    let first_high = scheduler::latest_health_snapshot(&pool)
        .await
        .unwrap()
        .expect("first high-pressure snapshot");
    assert_eq!(first_high.sustained_backpressure_state, "pending_active");

    scheduler::refresh_queue_summaries(&pool, &InvokeAgentCapacityConfig::default())
        .await
        .unwrap();
    let second_high = scheduler::latest_health_snapshot(&pool)
        .await
        .unwrap()
        .expect("second high-pressure snapshot");
    assert_eq!(second_high.sustained_backpressure_state, "active");
    let notification = scheduler::latest_backpressure_notification(&pool)
        .await
        .unwrap()
        .expect("active backpressure notification");
    assert_eq!(notification.run_id, Some(run_id.to_string()));
    assert_eq!(
        notification.stage_execution_id,
        Some(stage_execution_id.to_string())
    );
    assert_eq!(notification.provider_family.as_deref(), Some("codex"));
    assert_eq!(notification.state, "active");

    sqlx::query(
        "UPDATE work_items SET status = 'cancelled' WHERE id = 'sustained-backpressure-work'",
    )
    .execute(&pool)
    .await
    .unwrap();

    scheduler::refresh_queue_summaries(&pool, &InvokeAgentCapacityConfig::default())
        .await
        .unwrap();
    let first_clear = scheduler::latest_health_snapshot(&pool)
        .await
        .unwrap()
        .expect("first clear snapshot");
    assert_eq!(first_clear.sustained_backpressure_state, "pending_clear");

    scheduler::refresh_queue_summaries(&pool, &InvokeAgentCapacityConfig::default())
        .await
        .unwrap();
    let second_clear = scheduler::latest_health_snapshot(&pool)
        .await
        .unwrap()
        .expect("second clear snapshot");
    assert_eq!(second_clear.sustained_backpressure_state, "clear");
    let clear_notification = scheduler::latest_backpressure_notification(&pool)
        .await
        .unwrap()
        .expect("clear backpressure notification");
    assert_eq!(clear_notification.state, "clear");
    assert_eq!(clear_notification.top_reason, "clear");
    assert_eq!(clear_notification.global_queue_depth, 0);
}

#[tokio::test]
async fn proposal_061_host_interruption_readback_round_trip_by_run() {
    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();
    let agent_execution_id = AgentExecutionId::new();
    let now = Utc::now();

    sqlx::query(
        r#"INSERT INTO ideas (id, title, body, status, created_at)
           VALUES (?1, 'Host interruption idea', 'body', 'active', ?2)"#,
    )
    .bind(idea_id.to_string())
    .bind(now.to_rfc3339())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO runs
           (id, idea_id, status, workflow_id, workflow_title, workspace_root, artifact_root, started_at)
           VALUES (?1, ?2, 'running', 'wf-p061-host', 'Host interruption workflow', '/tmp/ws', '/tmp/art', ?3)"#,
    )
    .bind(run_id.to_string())
    .bind(idea_id.to_string())
    .bind(now.to_rfc3339())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO stage_executions
           (id, run_id, stage_id, label, status, started_at)
           VALUES (?1, ?2, 'stage-p061-host', 'Host interruption stage', 'running', ?3)"#,
    )
    .bind(stage_execution_id.to_string())
    .bind(run_id.to_string())
    .bind(now.to_rfc3339())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO agent_executions
           (id, stage_execution_id, agent_id, provider, provider_family, status, started_at)
           VALUES (?1, ?2, 'agent-p061-host', 'codex', 'codex', 'running', ?3)"#,
    )
    .bind(agent_execution_id.to_string())
    .bind(stage_execution_id.to_string())
    .bind(now.to_rfc3339())
    .execute(&pool)
    .await
    .unwrap();

    scheduler::insert_host_interruption_epoch(
        &pool,
        &scheduler::HostInterruptionEpoch {
            id: "epoch-sleep-1".into(),
            kind: "sleep_wake".into(),
            started_at: now - chrono::Duration::seconds(90),
            ended_at: Some(now - chrono::Duration::seconds(30)),
            monotonic_gap_ms: Some(60_000),
            wall_clock_gap_ms: Some(90_000),
            details_json: Some(r#"{"source":"wall_clock_gap"}"#.into()),
            created_at: now,
        },
    )
    .await
    .unwrap();
    scheduler::insert_host_interruption_affected_execution(
        &pool,
        &scheduler::HostInterruptionAffectedExecution {
            epoch_id: "epoch-sleep-1".into(),
            agent_execution_id: agent_execution_id.to_string(),
            run_id: Some(run_id.to_string()),
            stage_execution_id: stage_execution_id.to_string(),
            provider_family: Some("codex".into()),
            action: "recovering_from_system_sleep".into(),
            retry_enqueued_at: Some(now + chrono::Duration::seconds(5)),
            created_at: now,
        },
    )
    .await
    .unwrap();

    let readback = scheduler::list_host_interruption_epochs_by_run(&pool, &run_id.to_string())
        .await
        .unwrap();

    assert_eq!(readback.len(), 1);
    assert_eq!(readback[0].epoch.kind, "sleep_wake");
    assert_eq!(
        readback[0].affected_executions[0].agent_execution_id,
        agent_execution_id.to_string()
    );
    assert_eq!(
        readback[0].affected_executions[0].action,
        "recovering_from_system_sleep"
    );
}

#[tokio::test]
async fn proposal_061_queue_summary_upsert_and_zero_count_cleanup_round_trip() {
    let pool = test_pool().await;
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();
    let now = Utc::now();
    let summary = scheduler::SchedulerQueueSummary {
        scope: "run".into(),
        scope_id: run_id.to_string(),
        run_id: Some(run_id.to_string()),
        stage_execution_id: Some(stage_execution_id.to_string()),
        provider_family: Some("gemini".into()),
        top_reason: "provider_capacity".into(),
        queued_count: 4,
        oldest_queued_age_ms: 30_000,
        global_queue_depth: 11,
        stale_after_ms: 60_000,
        updated_at: now,
    };

    scheduler::upsert_queue_summary(&pool, &summary)
        .await
        .unwrap();

    let by_run = scheduler::list_queue_summaries_by_run(&pool, &run_id.to_string())
        .await
        .unwrap();
    assert_eq!(by_run.len(), 1);
    assert_eq!(by_run[0].provider_family.as_deref(), Some("gemini"));
    assert_eq!(by_run[0].queued_count, 4);
    assert_eq!(by_run[0].global_queue_depth, 11);
    assert!(!by_run[0].is_stale_at(now + chrono::Duration::milliseconds(59_999)));
    assert!(by_run[0].is_stale_at(now + chrono::Duration::milliseconds(60_000)));

    let by_stage = scheduler::list_queue_summaries_by_stage(&pool, &stage_execution_id.to_string())
        .await
        .unwrap();
    assert_eq!(by_stage, by_run);

    let cleared = scheduler::SchedulerQueueSummary {
        queued_count: 0,
        updated_at: now + chrono::Duration::milliseconds(1),
        ..summary
    };
    scheduler::upsert_queue_summary(&pool, &cleared)
        .await
        .unwrap();

    assert!(scheduler::list_queue_summaries(&pool)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn proposal_061_capacity_claim_skips_blocked_invoke_agent_and_refreshes_projection() {
    use std::collections::BTreeMap;

    use sqlx::Row;

    let pool = test_pool().await;
    let now = Utc::now();
    let idea_id = IdeaId::new();
    let blocked_run_id = RunId::new();
    let blocked_stage_execution_id = StageExecutionId::new();
    seed_minimal_stage(
        &pool,
        idea_id,
        blocked_run_id,
        blocked_stage_execution_id,
        now,
    )
    .await;
    let eligible_run_id = RunId::new();
    let eligible_stage_execution_id = StageExecutionId::new();
    seed_minimal_stage(
        &pool,
        IdeaId::new(),
        eligible_run_id,
        eligible_stage_execution_id,
        now,
    )
    .await;

    sqlx::query(
        r#"INSERT INTO agent_executions
           (id, stage_execution_id, agent_id, provider, provider_family, status, started_at)
           VALUES ('active-gemini', ?1, 'agent-active', 'gemini', 'gemini', 'running', ?2)"#,
    )
    .bind(blocked_stage_execution_id.to_string())
    .bind(now.to_rfc3339())
    .execute(&pool)
    .await
    .unwrap();

    insert_pending_invoke_agent_work(
        &pool,
        "blocked-gemini-work",
        blocked_run_id,
        blocked_stage_execution_id,
        "gemini",
        now,
    )
    .await;
    insert_pending_invoke_agent_work(
        &pool,
        "eligible-codex-work",
        eligible_run_id,
        eligible_stage_execution_id,
        "codex",
        now + chrono::Duration::milliseconds(1),
    )
    .await;

    let capacity = InvokeAgentCapacityConfig {
        global_active_agent_executions: 20,
        per_run_active_agent_executions: 4,
        provider_caps: BTreeMap::from([
            (ProviderFamily::Claude, 8),
            (ProviderFamily::Gemini, 1),
            (ProviderFamily::Codex, 1),
            (ProviderFamily::Auggie, 1),
            (ProviderFamily::Junie, 1),
        ]),
    };

    scheduler::refresh_queue_summaries(&pool, &capacity)
        .await
        .unwrap();
    let blocked_summary =
        scheduler::list_queue_summaries_by_run(&pool, &blocked_run_id.to_string())
            .await
            .unwrap()
            .into_iter()
            .find(|summary| summary.top_reason == "provider_capacity")
            .expect("blocked run should have provider-capacity summary");
    assert_eq!(blocked_summary.provider_family.as_deref(), Some("gemini"));
    assert_eq!(blocked_summary.queued_count, 1);
    assert_eq!(blocked_summary.global_queue_depth, 2);

    let claimed = work_items::claim_next_with_invoke_agent_capacity(&pool, &capacity)
        .await
        .unwrap()
        .expect("eligible later candidate should be claimed");
    assert_eq!(claimed.id, "eligible-codex-work");

    let statuses = sqlx::query(
        r#"SELECT id, status
           FROM work_items
           WHERE id IN ('blocked-gemini-work', 'eligible-codex-work')
           ORDER BY id ASC"#,
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(statuses[0].get::<String, _>("id"), "blocked-gemini-work");
    assert_eq!(statuses[0].get::<String, _>("status"), "pending");
    assert_eq!(statuses[1].get::<String, _>("id"), "eligible-codex-work");
    assert_eq!(statuses[1].get::<String, _>("status"), "running");

    let global_state = scheduler::get_service_state(&pool, "global", "")
        .await
        .unwrap()
        .expect("global scheduler service state");
    assert_eq!(
        global_state.last_claimed_work_item_id.as_deref(),
        Some("eligible-codex-work")
    );
}

#[tokio::test]
async fn proposal_061_capacity_claim_prefers_least_recently_served_run_within_window() {
    let pool = test_pool().await;
    let now = Utc::now();
    let recently_served_run_id = RunId::new();
    let recently_served_stage_execution_id = StageExecutionId::new();
    seed_minimal_stage(
        &pool,
        IdeaId::new(),
        recently_served_run_id,
        recently_served_stage_execution_id,
        now,
    )
    .await;
    let least_recently_served_run_id = RunId::new();
    let least_recently_served_stage_execution_id = StageExecutionId::new();
    seed_minimal_stage(
        &pool,
        IdeaId::new(),
        least_recently_served_run_id,
        least_recently_served_stage_execution_id,
        now,
    )
    .await;

    scheduler::upsert_service_state(
        &pool,
        &scheduler::SchedulerServiceState {
            scope: "run".into(),
            scope_id: recently_served_run_id.to_string(),
            last_served_at: Some(now),
            last_claimed_work_item_id: Some("recently-served-previous".into()),
            updated_at: now,
        },
    )
    .await
    .unwrap();
    scheduler::upsert_service_state(
        &pool,
        &scheduler::SchedulerServiceState {
            scope: "run".into(),
            scope_id: least_recently_served_run_id.to_string(),
            last_served_at: Some(now - chrono::Duration::seconds(60)),
            last_claimed_work_item_id: Some("least-recently-served-previous".into()),
            updated_at: now,
        },
    )
    .await
    .unwrap();

    insert_pending_invoke_agent_work(
        &pool,
        "recently-served-work",
        recently_served_run_id,
        recently_served_stage_execution_id,
        "codex",
        now - chrono::Duration::seconds(2),
    )
    .await;
    insert_pending_invoke_agent_work(
        &pool,
        "least-recently-served-work",
        least_recently_served_run_id,
        least_recently_served_stage_execution_id,
        "codex",
        now - chrono::Duration::seconds(1),
    )
    .await;

    let claimed = work_items::claim_next_with_invoke_agent_capacity(
        &pool,
        &InvokeAgentCapacityConfig::default(),
    )
    .await
    .unwrap()
    .expect("eligible least-recently-served run candidate should be claimed");
    assert_eq!(claimed.id, "least-recently-served-work");

    let least_recently_served_state =
        scheduler::get_service_state(&pool, "run", &least_recently_served_run_id.to_string())
            .await
            .unwrap()
            .expect("least-recently-served run state");
    assert_eq!(
        least_recently_served_state
            .last_claimed_work_item_id
            .as_deref(),
        Some("least-recently-served-work")
    );

    let recently_served_work = sqlx::query_scalar::<_, String>(
        "SELECT status FROM work_items WHERE id = 'recently-served-work'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(recently_served_work, "pending");
}

#[tokio::test]
async fn proposal_061_capacity_claim_reports_all_blocked_window_without_claiming() {
    use std::collections::BTreeMap;

    use sqlx::Row;

    let pool = test_pool().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();
    seed_minimal_stage(&pool, IdeaId::new(), run_id, stage_execution_id, now).await;

    sqlx::query(
        r#"INSERT INTO agent_executions
           (id, stage_execution_id, agent_id, provider, provider_family, status, started_at)
           VALUES ('active-codex', ?1, 'agent-active', 'codex', 'codex', 'running', ?2)"#,
    )
    .bind(stage_execution_id.to_string())
    .bind(now.to_rfc3339())
    .execute(&pool)
    .await
    .unwrap();

    insert_pending_invoke_agent_work(
        &pool,
        "blocked-codex-work-1",
        run_id,
        stage_execution_id,
        "codex",
        now - chrono::Duration::seconds(10),
    )
    .await;
    insert_pending_invoke_agent_work(
        &pool,
        "blocked-codex-work-2",
        run_id,
        stage_execution_id,
        "codex",
        now - chrono::Duration::seconds(5),
    )
    .await;

    let capacity = InvokeAgentCapacityConfig {
        global_active_agent_executions: 20,
        per_run_active_agent_executions: 4,
        provider_caps: BTreeMap::from([
            (ProviderFamily::Claude, 8),
            (ProviderFamily::Gemini, 4),
            (ProviderFamily::Codex, 1),
            (ProviderFamily::Auggie, 1),
            (ProviderFamily::Junie, 1),
        ]),
    };

    let result = work_items::claim_next_with_invoke_agent_capacity_result(&pool, &capacity)
        .await
        .unwrap();
    assert!(result.item.is_none());
    assert!(result.all_invoke_agent_candidates_blocked);

    let statuses = sqlx::query(
        r#"SELECT id, status
           FROM work_items
           WHERE id IN ('blocked-codex-work-1', 'blocked-codex-work-2')
           ORDER BY id ASC"#,
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(statuses.len(), 2);
    assert!(statuses
        .iter()
        .all(|row| row.get::<String, _>("status") == "pending"));

    assert!(scheduler::get_service_state(&pool, "global", "")
        .await
        .unwrap()
        .is_none());

    scheduler::refresh_queue_summaries(&pool, &capacity)
        .await
        .unwrap();
    let summary = scheduler::list_queue_summaries_by_run(&pool, &run_id.to_string())
        .await
        .unwrap()
        .into_iter()
        .find(|summary| summary.top_reason == "provider_capacity")
        .expect("blocked candidate window should refresh provider-capacity summary");
    assert_eq!(summary.provider_family.as_deref(), Some("codex"));
    assert_eq!(summary.queued_count, 2);
    assert_eq!(summary.global_queue_depth, 2);
}

#[tokio::test]
async fn proposal_061_queue_position_hint_reports_non_eta_run_and_stage_position() {
    let pool = test_pool().await;
    let now = Utc::now();
    let first_run_id = RunId::new();
    let first_stage_execution_id = StageExecutionId::new();
    seed_minimal_stage(
        &pool,
        IdeaId::new(),
        first_run_id,
        first_stage_execution_id,
        now,
    )
    .await;
    let target_run_id = RunId::new();
    let target_stage_execution_id = StageExecutionId::new();
    seed_minimal_stage(
        &pool,
        IdeaId::new(),
        target_run_id,
        target_stage_execution_id,
        now,
    )
    .await;

    insert_pending_invoke_agent_work(
        &pool,
        "first-work",
        first_run_id,
        first_stage_execution_id,
        "claude",
        now - chrono::Duration::seconds(30),
    )
    .await;
    insert_pending_invoke_agent_work(
        &pool,
        "target-work-oldest",
        target_run_id,
        target_stage_execution_id,
        "codex",
        now - chrono::Duration::seconds(20),
    )
    .await;
    insert_pending_invoke_agent_work(
        &pool,
        "target-work-newer",
        target_run_id,
        target_stage_execution_id,
        "gemini",
        now - chrono::Duration::seconds(10),
    )
    .await;

    let run_hint = scheduler::queue_position_hint_by_run(&pool, &target_run_id.to_string())
        .await
        .unwrap()
        .expect("target run should have queue position hint");
    assert_eq!(run_hint.scope, "run");
    assert_eq!(run_hint.scope_id, target_run_id.to_string());
    assert_eq!(run_hint.queue_position, 2);
    assert_eq!(run_hint.queued_ahead_count, 1);
    assert_eq!(run_hint.global_queue_depth, 3);
    assert_eq!(run_hint.scoped_queued_count, 2);
    assert!(run_hint.oldest_queued_age_ms >= 20_000);

    let stage_hint =
        scheduler::queue_position_hint_by_stage(&pool, &target_stage_execution_id.to_string())
            .await
            .unwrap()
            .expect("target stage should have queue position hint");
    assert_eq!(stage_hint.scope, "stage");
    assert_eq!(
        stage_hint.stage_execution_id.as_deref(),
        Some(target_stage_execution_id.to_string().as_str())
    );
    assert_eq!(stage_hint.queue_position, 2);
    assert_eq!(stage_hint.scoped_queued_count, 2);
}

#[tokio::test]
async fn proposal_061_agent_execution_insert_canonicalizes_provider_family() {
    use sqlx::Row;

    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();
    let now = Utc::now();
    seed_minimal_stage(&pool, idea_id, run_id, stage_execution_id, now).await;

    let execution = AgentExecution {
        id: AgentExecutionId::new(),
        stage_execution_id,
        agent_id: "code_writer".into(),
        provider: "codex_acp".into(),
        model: None,
        started_at: now,
        completed_at: None,
        status: AgentStatus::Running,
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
    agent_executions::insert(&pool, &execution).await.unwrap();

    let stored = agent_executions::find_by_id(&pool, execution.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.provider, "codex");

    let row = sqlx::query("SELECT provider, provider_family FROM agent_executions WHERE id = ?1")
        .bind(execution.id.to_string())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>("provider"), "codex");
    assert_eq!(row.get::<String, _>("provider_family"), "codex");
}

#[tokio::test]
async fn proposal_061_hot_index_query_plans_cover_scheduler_scans() {
    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();
    let now = Utc::now();
    seed_minimal_stage(&pool, idea_id, run_id, stage_execution_id, now).await;

    for i in 0..1000 {
        sqlx::query(
            r#"INSERT INTO work_items
               (id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at)
               VALUES (?1, 'invoke_agent', '{}', 'pending', ?2, ?3, ?4, ?5)"#,
        )
        .bind(format!("p061-work-{i}"))
        .bind(run_id.to_string())
        .bind(stage_execution_id.to_string())
        .bind(now.to_rfc3339())
        .bind((now + chrono::Duration::milliseconds(i)).to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();
    }

    for i in 0..500 {
        sqlx::query(
            r#"INSERT INTO agent_executions
               (id, stage_execution_id, agent_id, provider, provider_family, status, started_at)
               VALUES (?1, ?2, ?3, 'gemini', 'gemini', 'running', ?4)"#,
        )
        .bind(format!("p061-agent-{i}"))
        .bind(stage_execution_id.to_string())
        .bind(format!("agent-{i}"))
        .bind(now.to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();
    }

    for i in 0..20 {
        let id = StageExecutionId::new();
        sqlx::query(
            r#"INSERT INTO stage_executions
               (id, run_id, stage_id, label, status, started_at)
               VALUES (?1, ?2, ?3, ?4, 'pending', ?5)"#,
        )
        .bind(id.to_string())
        .bind(run_id.to_string())
        .bind(format!("stage-extra-{i}"))
        .bind(format!("Stage extra {i}"))
        .bind(now.to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();
    }

    let work_plan = explain_details(
        &pool,
        r#"EXPLAIN QUERY PLAN
           SELECT id FROM work_items
           WHERE kind = 'invoke_agent' AND status = 'pending' AND scheduled_at <= '9999'
           ORDER BY scheduled_at ASC
           LIMIT 50"#,
    )
    .await;
    assert!(
        work_plan.contains("idx_work_items_kind_status_scheduled_at"),
        "work item global scan should use hot index, got {work_plan}"
    );

    let run_work_plan = explain_details(
        &pool,
        &format!(
            r#"EXPLAIN QUERY PLAN
               SELECT id FROM work_items
               WHERE run_id = '{}' AND status = 'pending' AND kind = 'invoke_agent' AND scheduled_at <= '9999'
               ORDER BY scheduled_at ASC
               LIMIT 50"#,
            run_id
        ),
    )
    .await;
    assert!(
        run_work_plan.contains("idx_work_items_run_status_kind_scheduled_at"),
        "run-local work item scan should use hot index, got {run_work_plan}"
    );

    let agent_provider_plan = explain_details(
        &pool,
        r#"EXPLAIN QUERY PLAN
           SELECT id FROM agent_executions
           WHERE status = 'running' AND provider_family = 'gemini'"#,
    )
    .await;
    assert!(
        agent_provider_plan.contains("idx_agent_executions_status_provider_family"),
        "agent provider active-count scan should use hot index, got {agent_provider_plan}"
    );

    let agent_status_plan = explain_details(
        &pool,
        r#"EXPLAIN QUERY PLAN
           SELECT id FROM agent_executions
           WHERE status = 'running'"#,
    )
    .await;
    assert!(
        agent_status_plan.contains("idx_agent_executions_status"),
        "agent status active-count scan should use hot index, got {agent_status_plan}"
    );

    let stage_plan = explain_details(
        &pool,
        &format!(
            r#"EXPLAIN QUERY PLAN
               SELECT id FROM stage_executions
               WHERE run_id = '{}'
               ORDER BY id ASC"#,
            run_id
        ),
    )
    .await;
    assert!(
        stage_plan.contains("idx_stage_executions_run_id_id"),
        "stage run lookup should use hot index, got {stage_plan}"
    );
}

async fn explain_details(pool: &sqlx::SqlitePool, sql: &str) -> String {
    use sqlx::Row;

    let rows = sqlx::query(sql).fetch_all(pool).await.unwrap();
    rows.into_iter()
        .map(|row| row.get::<String, _>("detail"))
        .collect::<Vec<_>>()
        .join("\n")
}

async fn insert_pending_invoke_agent_work(
    pool: &sqlx::SqlitePool,
    id: &str,
    run_id: RunId,
    stage_execution_id: StageExecutionId,
    provider: &str,
    scheduled_at: chrono::DateTime<Utc>,
) {
    let payload = serde_json::json!({
        "run_id": run_id.to_string(),
        "stage_id": "stage-p061",
        "stage_execution_id": stage_execution_id.to_string(),
        "agent_id": format!("{provider}-agent"),
        "provider": provider,
    });
    sqlx::query(
        r#"INSERT INTO work_items
           (id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at)
           VALUES (?1, 'invoke_agent', ?2, 'pending', ?3, 'stage-p061', ?4, ?5)"#,
    )
    .bind(id)
    .bind(payload.to_string())
    .bind(run_id.to_string())
    .bind(scheduled_at.to_rfc3339())
    .bind(scheduled_at.to_rfc3339())
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_minimal_stage(
    pool: &sqlx::SqlitePool,
    idea_id: IdeaId,
    run_id: RunId,
    stage_execution_id: StageExecutionId,
    now: chrono::DateTime<Utc>,
) {
    sqlx::query(
        r#"INSERT INTO ideas (id, title, body, status, created_at)
           VALUES (?1, 'P061 idea', 'body', 'active', ?2)"#,
    )
    .bind(idea_id.to_string())
    .bind(now.to_rfc3339())
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        r#"INSERT INTO runs
           (id, idea_id, status, workflow_id, workflow_title, workspace_root, artifact_root, started_at)
           VALUES (?1, ?2, 'running', 'wf-p061', 'P061', '/tmp/ws', '/tmp/art', ?3)"#,
    )
    .bind(run_id.to_string())
    .bind(idea_id.to_string())
    .bind(now.to_rfc3339())
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        r#"INSERT INTO stage_executions
           (id, run_id, stage_id, label, status, started_at)
           VALUES (?1, ?2, 'stage-p061', 'Stage P061', 'running', ?3)"#,
    )
    .bind(stage_execution_id.to_string())
    .bind(run_id.to_string())
    .bind(now.to_rfc3339())
    .execute(pool)
    .await
    .unwrap();
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
