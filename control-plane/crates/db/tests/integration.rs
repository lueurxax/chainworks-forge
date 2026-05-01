use chrono::Utc;
use db::pool::create_pool;
use db::repos::{
    agent_executions, agent_retry_budget_ledger, approvals,
    artifact_contracts as artifact_contract_repos, artifacts, ideas, projections, runs, stages,
    steward, validation,
};
use domain::agent::{AgentExecution, AgentStatus, ArtifactSourceClaimState};
use domain::approval::{Approval, ApprovalDecision};
use domain::artifact::{Artifact, ArtifactFormat};
use domain::artifact_contracts::{ArtifactSourceGenerationClaim, ArtifactSourceGenerationClaimKey};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{AgentExecutionId, ApprovalId, ArtifactId, IdeaId, RunId, StageExecutionId};
use domain::mediation::OwnerKind;
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
use domain::xcode_runtime::{
    McpBrokerObservation, XcodeHostExecutorEvent, XcodeRuntimeFailureClass,
    XcodeRuntimeObservation, XcodeRuntimeObservationUpdate, XcodeShimEvent, XcodeShimWarningEvent,
    XCODE_RUNTIME_OBSERVATION_MAX_BYTES, XCODE_RUNTIME_OBSERVATION_MAX_EVENTS,
};

async fn test_pool() -> sqlx::SqlitePool {
    create_pool("sqlite::memory:")
        .await
        .expect("in-memory pool failed")
}

async fn insert_p017_run(pool: &sqlx::SqlitePool) -> RunId {
    let idea = Idea {
        id: IdeaId::new(),
        title: "P017".into(),
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
        workflow_id: "wf-p017".into(),
        workflow_title: "P017".into(),
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
        review_routing_json: None,
        closeout_readiness_mode: None,
    };
    runs::insert(pool, &run).await.unwrap();
    run.id
}

async fn insert_p051_test_agent_execution(pool: &sqlx::SqlitePool) -> AgentExecutionId {
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
    ideas::insert(pool, &idea).await.unwrap();

    let run = Run {
        id: RunId::new(),
        idea_id: idea.id,
        status: RunStatus::Running,
        workflow_id: "wf-p051".into(),
        workflow_title: "P051".into(),
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
        review_routing_json: None,
        closeout_readiness_mode: None,
    };
    runs::insert(pool, &run).await.unwrap();

    let stage = StageExecution {
        id: StageExecutionId::new(),
        run_id: run.id,
        stage_id: "stage-p051".into(),
        label: "Stage P051".into(),
        status: StageStatus::Running,
        iteration: 1,
        attempt_number: 1,
        settlement_kind: None,
        started_at: Utc::now(),
        completed_at: None,
        owner_agent: Some("xcode_agent".into()),
        provider: Some("codex".into()),
        model: None,
        stage_type: None,
        validation_failure_json: None,
        evidence_packet_json: None,
        recovery_snapshot_json: None,
        retry_reason: None,
    };
    stages::insert(pool, &stage).await.unwrap();

    let execution = AgentExecution {
        id: AgentExecutionId::new(),
        stage_execution_id: Some(stage.id),
        agent_id: "xcode_agent".into(),
        provider: "codex".into(),
        model: None,

        started_at: Utc::now(),
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
    };
    agent_executions::insert(pool, &execution).await.unwrap();

    execution.id
}

#[tokio::test]
async fn p017_mediation_owned_agent_execution_does_not_require_stage_execution() {
    let pool = test_pool().await;
    let now = Utc::now();
    let execution = AgentExecution {
        id: AgentExecutionId::new(),
        stage_execution_id: None,
        agent_id: "lead_orchestrator".into(),
        provider: "codex".into(),
        model: Some("test-model".into()),
        started_at: now,
        completed_at: None,
        status: AgentStatus::Running,
        owner_execution_lineage_id: Some("mediation-001".into()),
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
        owner_kind: Some("lead_conflict_mediation".into()),
        owner_id: Some("mediation-001".into()),
        lead_mediation_record_id: Some("mediation-001".into()),
        origin_stage_execution_id: None,
        total_cost_cents: None,
        input_tokens: None,
        output_tokens: None,
        cached_input_tokens: None,
        transcript_artifact_id: None,
    };

    agent_executions::insert(&pool, &execution)
        .await
        .expect("insert mediation-owned execution");

    let stored = agent_executions::find_by_id(&pool, execution.id)
        .await
        .expect("find execution")
        .expect("execution exists");

    assert_eq!(stored.stage_execution_id, None);
    assert_eq!(
        stored.owner_kind.as_deref(),
        Some("lead_conflict_mediation")
    );
    assert_eq!(stored.owner_id.as_deref(), Some("mediation-001"));
}

#[tokio::test]
async fn p017_mediation_owned_retry_budget_and_artifact_claims_are_owner_keyed() {
    let pool = test_pool().await;
    let run_id = insert_p017_run(&pool).await;
    let now = Utc::now();
    let execution = AgentExecution {
        id: AgentExecutionId::new(),
        stage_execution_id: None,
        agent_id: "lead_orchestrator".into(),
        provider: "codex".into(),
        model: Some("test-model".into()),
        started_at: now,
        completed_at: None,
        status: AgentStatus::Running,
        owner_execution_lineage_id: Some("mediation-claim-001".into()),
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
        owner_kind: Some("lead_conflict_mediation".into()),
        owner_id: Some("mediation-claim-001".into()),
        lead_mediation_record_id: Some("mediation-claim-001".into()),
        origin_stage_execution_id: None,
        total_cost_cents: None,
        input_tokens: None,
        output_tokens: None,
        cached_input_tokens: None,
        transcript_artifact_id: None,
    };
    agent_executions::insert(&pool, &execution).await.unwrap();

    let mut tx = pool.begin().await.unwrap();
    let ledger = agent_retry_budget_ledger::upsert_quota_failure_for_owner_tx(
        &mut tx,
        run_id,
        OwnerKind::LeadConflictMediation,
        "mediation-claim-001".into(),
        None,
        execution.id,
        None,
    )
    .await
    .unwrap();
    let owner_rows = agent_retry_budget_ledger::list_quota_for_owner_tx(
        &mut tx,
        run_id,
        OwnerKind::LeadConflictMediation,
        "mediation-claim-001",
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    assert_eq!(ledger.stage_execution_id, None);
    assert_eq!(ledger.owner_kind, OwnerKind::LeadConflictMediation);
    assert_eq!(ledger.owner_id, "mediation-claim-001");
    assert_eq!(owner_rows.len(), 1);
    assert_eq!(owner_rows[0].id, ledger.id);

    let key = ArtifactSourceGenerationClaimKey {
        run_id,
        owner_kind: OwnerKind::LeadConflictMediation,
        owner_id: "mediation-claim-001".into(),
        stage_execution_id: None,
        agent_execution_id: execution.id,
        source_work_item_id: "lead-mediation-work-item".into(),
    };
    artifact_contract_repos::insert_source_generation_claim(
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
            created_at: now,
            updated_at: now,
        },
    )
    .await
    .unwrap();

    let stored_claim = artifact_contract_repos::load_source_generation_claim(&pool, &key)
        .await
        .unwrap()
        .expect("mediation-owned claim");
    assert_eq!(stored_claim.key.stage_execution_id, None);
    assert_eq!(
        stored_claim.key.owner_kind,
        OwnerKind::LeadConflictMediation
    );
    assert_eq!(stored_claim.key.owner_id, "mediation-claim-001");
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
        review_routing_json: None,
        closeout_readiness_mode: None,
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

    let live_progress_at = last_activity_at + chrono::Duration::seconds(30);
    db::repos::sessions::touch_generation_activity(&pool, "generation-budget", live_progress_at)
        .await
        .unwrap();
    let generation = db::repos::sessions::find_active_generation(&pool, &lineage.id)
        .await
        .unwrap()
        .expect("generation should still exist");
    assert_eq!(generation.turn_count, 1);
    assert_eq!(generation.cumulative_prompt_tokens, 12_000);
    assert_eq!(generation.cumulative_cost_cents, 17);
    assert_eq!(generation.last_activity_at, Some(live_progress_at));
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
        review_routing_json: None,
        closeout_readiness_mode: None,
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
            review_routing_json: None,
            closeout_readiness_mode: None,
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
        review_routing_json: None,
        closeout_readiness_mode: None,
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
        stage_execution_id: Some(stage.id),
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
        review_routing_json: None,
        closeout_readiness_mode: None,
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
        stage_execution_id: Some(stage.id),
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
        actual_xcode_runtime_observation_json: None,
        mcp_session_startup_latency_ms: Some(42),
        owner_kind: None,
        owner_id: None,
        lead_mediation_record_id: None,
        origin_stage_execution_id: None,
        total_cost_cents: None,
        input_tokens: None,
        output_tokens: None,
        cached_input_tokens: None,
        transcript_artifact_id: None,
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
async fn proposal_051_xcode_runtime_observation_append_recovers_corrupt_json() {
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
        workflow_id: "wf-p051".into(),
        workflow_title: "P051".into(),
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
        review_routing_json: None,
        closeout_readiness_mode: None,
    };
    runs::insert(&pool, &run).await.unwrap();

    let stage = StageExecution {
        id: StageExecutionId::new(),
        run_id: run.id,
        stage_id: "stage-p051".into(),
        label: "Stage P051".into(),
        status: StageStatus::Running,
        iteration: 1,
        attempt_number: 1,
        settlement_kind: None,
        started_at: Utc::now(),
        completed_at: None,
        owner_agent: Some("xcode_agent".into()),
        provider: Some("codex".into()),
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
        stage_execution_id: Some(stage.id),
        agent_id: "xcode_agent".into(),
        provider: "codex".into(),
        model: None,
        started_at: Utc::now(),
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
    };
    agent_executions::insert(&pool, &execution).await.unwrap();

    agent_executions::append_xcode_runtime_observation(
        &pool,
        execution.id,
        XcodeRuntimeObservationUpdate::McpBrokerObservation(McpBrokerObservation {
            source: "xcode_mcp_broker".into(),
            backend_start_disposition: "spawned".into(),
            pool_id: Some("pool-1".into()),
            lease_id: Some("lease-1".into()),
            xcode_pid: Some("77907".into()),
            backend_process_id: Some(24837),
            http_endpoint: Some("127.0.0.1:<redacted>".into()),
            xcode_home_disposition: Some("host_user_home".into()),
            xcode_tmpdir_disposition: Some("host_user_temp".into()),
            simulator_selection: None,
            sibling_leases_at_spawn: Some(1),
            backend_initialize_wait_ms: Some(420),
            backend_startup_latency_ms: Some(23031),
            http_session_startup_latency_ms: Some(42),
            backend_failure_class: None,
            originating_execution_id: None,
            prompt_cycle_index: Some(0),
            status_update: None,
        }),
    )
    .await
    .unwrap();

    agent_executions::append_xcode_runtime_observation(
        &pool,
        execution.id,
        XcodeRuntimeObservationUpdate::XcodeShimEvent(XcodeShimEvent::Warning(
            XcodeShimWarningEvent {
                ts: Utc::now(),
                policy_reason: "xcode_absolute_path_in_prompt".into(),
                source_field: "agent.prompt".into(),
                matched_substring: "/usr/bin/xcrun mcpbridge".into(),
                excerpt: "run /usr/bin/xcrun mcpbridge".into(),
            },
        )),
    )
    .await
    .unwrap();

    let found = agent_executions::find_by_id(&pool, execution.id)
        .await
        .unwrap()
        .unwrap();
    let observation: XcodeRuntimeObservation = serde_json::from_str(
        found
            .actual_xcode_runtime_observation_json
            .as_deref()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(observation.version, 1);
    assert_eq!(observation.mcp_broker_observations.len(), 1);
    assert_eq!(observation.xcode_shim_events.len(), 1);
    assert_eq!(
        observation.mcp_broker_observations[0].lease_id.as_deref(),
        Some("lease-1")
    );

    sqlx::query(
        "UPDATE agent_executions SET actual_xcode_runtime_observation_json = ? WHERE id = ?",
    )
    .bind("{not-json")
    .bind(execution.id.to_string())
    .execute(&pool)
    .await
    .unwrap();

    agent_executions::append_xcode_runtime_observation(
        &pool,
        execution.id,
        XcodeRuntimeObservationUpdate::XcodeHostExecutorEvent(XcodeHostExecutorEvent {
            ts: Utc::now(),
            tool: "xcodebuild".into(),
            argv: vec!["build".into()],
            cwd: "/tmp/ws".into(),
            host_env_disposition: "host_user_home".into(),
            env_allowlist_applied: vec!["HOME".into(), "TMPDIR".into()],
            env_dropped_from_provider: vec!["CODEX_HOME".into()],
            selected_simulator_id: None,
            exit_status: 0,
            duration_ms: 17,
        }),
    )
    .await
    .unwrap();

    agent_executions::append_xcode_runtime_observation(
        &pool,
        execution.id,
        XcodeRuntimeObservationUpdate::McpBrokerStatusUpdate(
            domain::xcode_runtime::McpBrokerStatusUpdate {
                lease_id: "lease-1".into(),
                backend_failure_class: XcodeRuntimeFailureClass::PoolPidDrift,
                status_update: "backend_failed_after_spawn".into(),
            },
        ),
    )
    .await
    .unwrap();

    let recovered = agent_executions::find_by_id(&pool, execution.id)
        .await
        .unwrap()
        .unwrap();
    let recovered_observation: XcodeRuntimeObservation = serde_json::from_str(
        recovered
            .actual_xcode_runtime_observation_json
            .as_deref()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(recovered_observation.xcode_host_executor_events.len(), 1);
    assert_eq!(recovered_observation.mcp_broker_observations.len(), 1);
    assert_eq!(
        recovered_observation.mcp_broker_observations[0]
            .status_update
            .as_deref(),
        Some("backend_failed_after_spawn")
    );
    assert_eq!(recovered_observation.storage.corrupt_json_recovery_count, 1);
    assert_eq!(
        recovered_observation.storage.corrupt_json_quarantined_bytes,
        "{not-json".len()
    );
}

#[tokio::test]
async fn proposal_051_xcode_runtime_observation_append_serializes_parallel_writers() {
    let tmp = tempfile::tempdir().unwrap();
    let db_file = tmp.path().join("p051-observation-contention.db");
    let db_url = format!("sqlite://{}", db_file.display());
    let pool = create_pool(&db_url).await.unwrap();
    let execution_id = insert_p051_test_agent_execution(&pool).await;
    let writer_count = 12usize;
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(writer_count));

    let mut handles = Vec::new();
    for idx in 0..writer_count {
        let pool = pool.clone();
        let barrier = barrier.clone();
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            agent_executions::append_xcode_runtime_observation(
                &pool,
                execution_id,
                XcodeRuntimeObservationUpdate::McpBrokerObservation(McpBrokerObservation {
                    source: "xcode_mcp_broker".into(),
                    backend_start_disposition: "lease_active".into(),
                    pool_id: Some("pool-1".into()),
                    lease_id: Some(format!("lease-{idx}")),
                    xcode_pid: Some("36971".into()),
                    backend_process_id: Some(25000 + idx as i64),
                    http_endpoint: Some(format!("http://127.0.0.1:4000/xcode-mcp/lease-{idx}")),
                    xcode_home_disposition: Some("host_operator_home_available".into()),
                    xcode_tmpdir_disposition: Some("darwin_tmpdir_available".into()),
                    simulator_selection: None,
                    sibling_leases_at_spawn: Some(idx as i64),
                    backend_initialize_wait_ms: Some(0),
                    backend_startup_latency_ms: None,
                    http_session_startup_latency_ms: None,
                    backend_failure_class: None,
                    originating_execution_id: Some(execution_id.to_string()),
                    prompt_cycle_index: None,
                    status_update: Some(format!("parallel observation {idx}")),
                }),
            )
            .await
        }));
    }

    for handle in handles {
        handle.await.unwrap().unwrap();
    }

    let execution = agent_executions::find_by_id(&pool, execution_id)
        .await
        .unwrap()
        .expect("execution should exist");
    let observation: XcodeRuntimeObservation = serde_json::from_str(
        execution
            .actual_xcode_runtime_observation_json
            .as_deref()
            .expect("observation should be persisted"),
    )
    .unwrap();
    assert_eq!(observation.mcp_broker_observations.len(), writer_count);
    assert!(!observation.storage.truncated);
    assert_eq!(observation.storage.total_events_dropped, 0);
}

#[tokio::test]
async fn proposal_051_xcode_runtime_observation_append_enforces_event_and_byte_bounds() {
    let pool = test_pool().await;
    let execution_id = insert_p051_test_agent_execution(&pool).await;

    for idx in 0..(XCODE_RUNTIME_OBSERVATION_MAX_EVENTS + 2) {
        agent_executions::append_xcode_runtime_observation(
            &pool,
            execution_id,
            XcodeRuntimeObservationUpdate::McpBrokerObservation(McpBrokerObservation {
                source: "xcode_mcp_broker".into(),
                backend_start_disposition: "spawned".into(),
                pool_id: Some("pool-1".into()),
                lease_id: Some(format!("lease-{idx}")),
                xcode_pid: Some("77907".into()),
                backend_process_id: Some(24837),
                http_endpoint: Some("127.0.0.1:<redacted>".into()),
                xcode_home_disposition: Some("host_user_home".into()),
                xcode_tmpdir_disposition: Some("host_user_temp".into()),
                simulator_selection: None,
                sibling_leases_at_spawn: Some(1),
                backend_initialize_wait_ms: Some(420),
                backend_startup_latency_ms: Some(23031),
                http_session_startup_latency_ms: Some(42),
                backend_failure_class: None,
                originating_execution_id: None,
                prompt_cycle_index: Some(idx as i64),
                status_update: None,
            }),
        )
        .await
        .unwrap();
    }

    let found = agent_executions::find_by_id(&pool, execution_id)
        .await
        .unwrap()
        .unwrap();
    let observation: XcodeRuntimeObservation = serde_json::from_str(
        found
            .actual_xcode_runtime_observation_json
            .as_deref()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        observation.total_event_count(),
        XCODE_RUNTIME_OBSERVATION_MAX_EVENTS
    );
    assert!(observation.storage.truncated);
    assert_eq!(observation.storage.total_events_dropped, 2);
    assert_eq!(observation.storage.mcp_broker_observations_dropped, 2);
    assert_eq!(
        observation.mcp_broker_observations[0].lease_id.as_deref(),
        Some("lease-2")
    );

    let oversized_execution_id = insert_p051_test_agent_execution(&pool).await;
    agent_executions::append_xcode_runtime_observation(
        &pool,
        oversized_execution_id,
        XcodeRuntimeObservationUpdate::XcodeShimEvent(XcodeShimEvent::Warning(
            XcodeShimWarningEvent {
                ts: Utc::now(),
                policy_reason: "xcode_absolute_path_in_prompt".into(),
                source_field: "agent.prompt".into(),
                matched_substring: "/usr/bin/xcrun mcpbridge".into(),
                excerpt: "x".repeat(XCODE_RUNTIME_OBSERVATION_MAX_BYTES + 1),
            },
        )),
    )
    .await
    .unwrap();

    let oversized_found = agent_executions::find_by_id(&pool, oversized_execution_id)
        .await
        .unwrap()
        .unwrap();
    let oversized_json = oversized_found
        .actual_xcode_runtime_observation_json
        .as_deref()
        .unwrap();
    let oversized_observation: XcodeRuntimeObservation =
        serde_json::from_str(oversized_json).unwrap();
    assert!(oversized_json.len() <= XCODE_RUNTIME_OBSERVATION_MAX_BYTES);
    assert!(oversized_observation.storage.truncated);
    assert_eq!(oversized_observation.total_event_count(), 0);
    assert_eq!(oversized_observation.storage.total_events_dropped, 1);
    assert_eq!(oversized_observation.storage.xcode_shim_events_dropped, 1);
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
        review_routing_json: None,
        closeout_readiness_mode: None,
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
        stage_execution_id: Some(failed_attempt.id),
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
    };
    let retry_agent_execution = AgentExecution {
        id: AgentExecutionId::new(),
        stage_execution_id: Some(successful_retry.id),
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
        agent_execution_id: None,
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
        review_routing_json: None,
        closeout_readiness_mode: None,
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
        review_routing_json: None,
        closeout_readiness_mode: None,
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
        review_routing_json: None,
        closeout_readiness_mode: None,
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

/// Northbound projections must expose canonical run status even when the
/// denormalized summary row has not caught up yet.
#[tokio::test]
async fn test_projection_status_uses_canonical_run_when_summary_lags() {
    let pool = test_pool().await;

    let idea = Idea {
        id: IdeaId::new(),
        title: "Lagging summary idea".into(),
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
        workflow_id: "wf-lag".into(),
        workflow_title: "Lag Workflow".into(),
        workspace_root: "/tmp/lag".into(),
        artifact_root: "/tmp/lag/art".into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        cancellation_settlement_log: None,
        current_state: Some("review".into()),
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
    runs::insert(&pool, &run).await.unwrap();
    projections::rebuild_all_for_run(&pool, run.id)
        .await
        .unwrap();
    runs::update_status(&pool, run.id, RunStatus::Blocked)
        .await
        .unwrap();

    let active = projections::list_active_projection(&pool).await.unwrap();
    let active_row = active
        .iter()
        .find(|row| row.id == run.id.to_string())
        .expect("active projection row");
    assert_eq!(active_row.status, "blocked");
    assert!(active_row.projection_lag);

    let by_idea = projections::list_by_idea_projection(&pool, &idea.id.to_string())
        .await
        .unwrap();
    assert_eq!(by_idea[0].status, "blocked");
    assert!(by_idea[0].projection_lag);

    let found = projections::find_run_projection(&pool, &run.id.to_string())
        .await
        .unwrap()
        .expect("run projection");
    assert_eq!(found.status, "blocked");
    assert!(found.projection_lag);
}

/// Northbound run summaries must expose canonical pending approval counts even
/// when the denormalized run summary has not caught up yet.
#[tokio::test]
async fn test_projection_pending_approvals_uses_canonical_approvals_when_summary_lags() {
    let pool = test_pool().await;

    let idea = Idea {
        id: IdeaId::new(),
        title: "Lagging approval summary idea".into(),
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
        status: RunStatus::WaitingApproval,
        workflow_id: "wf-approval-lag".into(),
        workflow_title: "Approval Lag Workflow".into(),
        workspace_root: "/tmp/approval-lag".into(),
        artifact_root: "/tmp/approval-lag/art".into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        cancellation_settlement_log: None,
        current_state: Some("manual_gate".into()),
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
    runs::insert(&pool, &run).await.unwrap();

    let requested_at = Utc::now();
    let approval = Approval {
        id: ApprovalId::new(),
        run_id: run.id,
        stage_id: "manual_gate".into(),
        decision: ApprovalDecision::Requested,
        requested_at,
        decided_at: None,
        comment: None,
        expires_at: None,
    };
    approvals::insert(&pool, &approval).await.unwrap();
    projections::rebuild_all_for_run(&pool, run.id)
        .await
        .unwrap();

    approvals::resolve(
        &pool,
        approval.id,
        ApprovalDecision::Granted,
        Utc::now(),
        Some("approved".into()),
    )
    .await
    .unwrap();

    let active = projections::list_active_projection(&pool).await.unwrap();
    let active_row = active
        .iter()
        .find(|row| row.id == run.id.to_string())
        .expect("active projection row");
    assert_eq!(active_row.pending_approvals, 0);
    assert!(active_row.projection_lag);

    let by_idea = projections::list_by_idea_projection(&pool, &idea.id.to_string())
        .await
        .unwrap();
    assert_eq!(by_idea[0].pending_approvals, 0);
    assert!(by_idea[0].projection_lag);

    let found = projections::find_run_projection(&pool, &run.id.to_string())
        .await
        .unwrap()
        .expect("run projection");
    assert_eq!(found.pending_approvals, 0);
    assert!(found.projection_lag);
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
            review_routing_json: None,
            closeout_readiness_mode: None,
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
            agent_execution_id: None,
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
        review_routing_json: None,
        closeout_readiness_mode: None,
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
            agent_execution_id: None,
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
        review_routing_json: None,
        closeout_readiness_mode: None,
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
        review_routing_json: None,
        closeout_readiness_mode: None,
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
        review_routing_json: None,
        closeout_readiness_mode: None,
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
            review_routing_json: None,
            closeout_readiness_mode: None,
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
