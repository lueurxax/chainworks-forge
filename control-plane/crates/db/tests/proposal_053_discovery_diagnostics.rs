use std::collections::BTreeMap;

use chrono::Utc;
use db::pool::create_pool;
use db::repos::{
    agent_execution_discovery_diagnostics, agent_execution_runtime_facts, agent_executions,
    command_journal, ideas, legacy_discovery_overrides, runs, stages,
};
use domain::agent::{
    AgentExecution, AgentExecutionRuntimeFacts, AgentOutputSettlement, AgentStatus,
};
use domain::discovery::{
    AgentExecutionDiscoveryDiagnostics, DiscoveryDiagnosticsV1, ExpectedOutputRole,
    LegacyBroadDiscoveryPolicy, LegacyDiscoveryOverrideInput, LegacyDiscoveryOverrideStatus,
    OutputDiscoveryDecision, OutputDiscoveryReason, OutputDiscoveryStatus,
    DISCOVERY_DIAGNOSTICS_V1_SCHEMA_VERSION,
};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{AgentExecutionId, IdeaId, RunId, StageExecutionId};
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageStatus};

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
    }
}

async fn seed_execution(pool: &sqlx::SqlitePool) -> (RunId, AgentExecutionId) {
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();
    let agent_execution_id = AgentExecutionId::new();
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
            stage_execution_id,
            agent_id: "code_writer".into(),
            provider: "claude".into(),
            model: Some("sonnet".into()),
            started_at: Utc::now(),
            completed_at: None,
            status: AgentStatus::Running,
            owner_execution_lineage_id: Some(stage_execution_id.to_string()),
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
        },
    )
    .await
    .unwrap();
    (run_id, agent_execution_id)
}

fn missing_decision(agent_execution_id: AgentExecutionId) -> OutputDiscoveryDecision {
    OutputDiscoveryDecision {
        output_name: "implementation_self_assessment".into(),
        output_role: ExpectedOutputRole::Machine,
        target_path: "implementation/self-assessment.json".into(),
        companion_of: None,
        status: OutputDiscoveryStatus::Missing,
        reason: OutputDiscoveryReason::MissingAfterPrompt,
        provenance: None,
        canonical_path: None,
        root_class: None,
        baseline_status: None,
        size_bytes: None,
        content_digest: None,
        max_bytes_applied: Some(10 * 1024 * 1024),
        aggregate_bytes_after_acceptance: None,
        accepted_payload_ref: None,
        accepted_bytes_sha256: None,
        generated_by: None,
        diagnostics: BTreeMap::from([(
            "agent_execution_id".to_string(),
            agent_execution_id.to_string(),
        )]),
        decision_at: Utc::now(),
    }
}

fn accepted_decision(agent_execution_id: AgentExecutionId) -> OutputDiscoveryDecision {
    OutputDiscoveryDecision {
        output_name: "implementation_self_assessment".into(),
        output_role: ExpectedOutputRole::Machine,
        target_path: "implementation/self-assessment.json".into(),
        companion_of: None,
        status: OutputDiscoveryStatus::Accepted,
        reason: OutputDiscoveryReason::ExactPathNew,
        provenance: None,
        canonical_path: Some("/tmp/artifacts/implementation/self-assessment.json".into()),
        root_class: None,
        baseline_status: None,
        size_bytes: Some(128),
        content_digest: Some("sha256:accepted".into()),
        max_bytes_applied: Some(10 * 1024 * 1024),
        aggregate_bytes_after_acceptance: Some(128),
        accepted_payload_ref: Some("provider_envelope:implementation_self_assessment".into()),
        accepted_bytes_sha256: Some("accepted".into()),
        generated_by: Some(agent_execution_id.to_string()),
        diagnostics: BTreeMap::new(),
        decision_at: Utc::now(),
    }
}

#[tokio::test]
async fn proposal_053_discovery_diagnostics_roundtrip_and_list_by_run() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let (run_id, agent_execution_id) = seed_execution(&pool).await;
    let now = Utc::now();
    let payload = DiscoveryDiagnosticsV1 {
        schema_version: DISCOVERY_DIAGNOSTICS_V1_SCHEMA_VERSION.to_string(),
        agent_execution_id: agent_execution_id.to_string(),
        decisions: vec![missing_decision(agent_execution_id)],
        pre_prompt_expected_outputs: Vec::new(),
        legacy_broad_discovery_used: true,
        bounded_meta_root_discovery: None,
        git_manifest_status: Some("not_git_repository".into()),
        resume_warnings: vec!["reconciliation_pending".into()],
        warnings: Vec::new(),
        generated_at: now,
    };
    let diagnostics = AgentExecutionDiscoveryDiagnostics::from_payload(payload, now);

    agent_execution_discovery_diagnostics::upsert(&pool, &diagnostics)
        .await
        .unwrap();

    let read =
        agent_execution_discovery_diagnostics::find_by_execution_id(&pool, agent_execution_id)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(
        read.discovery_schema_version,
        DISCOVERY_DIAGNOSTICS_V1_SCHEMA_VERSION
    );
    assert!(read.legacy_broad_discovery_used);
    assert_eq!(read.missing_required_output_count, 1);
    assert_eq!(read.rejected_output_count, 0);
    assert_eq!(read.resume_warning_count, 1);
    assert_eq!(read.payload.decisions.len(), 1);

    let by_run = agent_execution_discovery_diagnostics::list_by_run(&pool, run_id)
        .await
        .unwrap();
    assert_eq!(by_run.len(), 1);
    assert_eq!(by_run[0].agent_execution_id, agent_execution_id.to_string());
}

#[tokio::test]
async fn proposal_053_discovery_diagnostics_readback_marks_reconciliation_pending() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let (_run_id, agent_execution_id) = seed_execution(&pool).await;
    let now = Utc::now();
    let payload = DiscoveryDiagnosticsV1 {
        schema_version: DISCOVERY_DIAGNOSTICS_V1_SCHEMA_VERSION.to_string(),
        agent_execution_id: agent_execution_id.to_string(),
        decisions: vec![accepted_decision(agent_execution_id)],
        pre_prompt_expected_outputs: Vec::new(),
        legacy_broad_discovery_used: false,
        bounded_meta_root_discovery: None,
        git_manifest_status: None,
        resume_warnings: Vec::new(),
        warnings: Vec::new(),
        generated_at: now,
    };
    let diagnostics = AgentExecutionDiscoveryDiagnostics::from_payload(payload, now);
    agent_execution_discovery_diagnostics::upsert(&pool, &diagnostics)
        .await
        .unwrap();

    let missing_facts_readback =
        agent_execution_discovery_diagnostics::find_readback_by_execution_id(
            &pool,
            agent_execution_id,
        )
        .await
        .unwrap()
        .unwrap();
    assert!(missing_facts_readback.reconciliation_pending);
    assert!(!missing_facts_readback.runtime_facts_present);
    assert!(missing_facts_readback
        .projected_payload()
        .resume_warnings
        .contains(&"reconciliation_pending".to_string()));

    let mut facts = AgentExecutionRuntimeFacts::defaults_for(agent_execution_id, Utc::now());
    facts.output_settlement = AgentOutputSettlement::ValidOutputsFromCompletedExecution;
    facts.valid_required_outputs = true;
    agent_execution_runtime_facts::upsert(&pool, &facts)
        .await
        .unwrap();

    let missing_generation_readback =
        agent_execution_discovery_diagnostics::find_readback_by_execution_id(
            &pool,
            agent_execution_id,
        )
        .await
        .unwrap()
        .unwrap();
    assert!(missing_generation_readback.reconciliation_pending);
    assert!(missing_generation_readback.runtime_facts_present);
    assert_eq!(
        missing_generation_readback.matching_active_artifact_generation_count,
        0
    );
    assert!(missing_generation_readback
        .reconciliation_warnings
        .iter()
        .any(|warning| warning.contains("active artifact generation truth")));
}

#[tokio::test]
async fn proposal_053_discovery_diagnostics_legacy_override_binds_and_consumes_pending_retry() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    ideas::insert(
        &pool,
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
    runs::insert(&pool, &make_run(run_id, idea_id))
        .await
        .unwrap();
    let stage_execution_id = StageExecutionId::new();
    stages::insert(
        &pool,
        &StageExecution {
            id: stage_execution_id,
            run_id,
            stage_id: "state_1".into(),
            label: "State 1".into(),
            status: StageStatus::Pending,
            iteration: 1,
            attempt_number: 2,
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
            retry_reason: Some("operator_retry".into()),
        },
    )
    .await
    .unwrap();
    let journal_id = uuid::Uuid::new_v4().to_string();
    let run_id_str = run_id.to_string();
    command_journal::record(
        &pool,
        &journal_id,
        "RetryStage",
        "{}",
        Some(&run_id_str),
        Utc::now(),
        Some("mcp"),
        Some("operator-1"),
        Some("operator"),
        Some("stages.retry"),
        None,
    )
    .await
    .unwrap();

    let mut tx = pool.begin().await.unwrap();
    let created = legacy_discovery_overrides::create_for_pending_retry_tx(
        &mut tx,
        &LegacyDiscoveryOverrideInput {
            run_id,
            stage_id: "state_1".into(),
            workflow_id: "wf".into(),
            target_stage_execution_id: stage_execution_id,
            target_attempt_number: 2,
            actor_id: "operator-1".into(),
            reason: "legacy workflow lacks declared outputs".into(),
            requested_policy: LegacyBroadDiscoveryPolicy::WorkflowOptIn,
            from_policy: LegacyBroadDiscoveryPolicy::Disabled,
            approval_source: "stages.retry".into(),
            journal_id: journal_id.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(created.status, LegacyDiscoveryOverrideStatus::Pending);

    let consumed = legacy_discovery_overrides::consume_pending_for_stage_tx(
        &mut tx,
        run_id,
        "state_1",
        stage_execution_id,
        2,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(consumed.override_id, created.override_id);
    assert_eq!(consumed.status, LegacyDiscoveryOverrideStatus::Consumed);
    assert_eq!(
        consumed.requested_policy,
        LegacyBroadDiscoveryPolicy::WorkflowOptIn
    );
    tx.commit().await.unwrap();

    let readback = legacy_discovery_overrides::list_by_run(&pool, run_id)
        .await
        .unwrap();
    assert_eq!(readback.len(), 1);
    assert_eq!(readback[0].status, LegacyDiscoveryOverrideStatus::Consumed);
    assert!(readback[0].consumed_at.is_some());
}

#[tokio::test]
async fn proposal_053_discovery_diagnostics_legacy_override_rejects_duplicate_and_started_retry_targets(
) {
    let pool = create_pool(":memory:").await.unwrap();
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    ideas::insert(
        &pool,
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
    runs::insert(&pool, &make_run(run_id, idea_id))
        .await
        .unwrap();

    let pending_stage_execution_id = StageExecutionId::new();
    stages::insert(
        &pool,
        &StageExecution {
            id: pending_stage_execution_id,
            run_id,
            stage_id: "state_1".into(),
            label: "State 1".into(),
            status: StageStatus::Pending,
            iteration: 1,
            attempt_number: 2,
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
            retry_reason: Some("operator_retry".into()),
        },
    )
    .await
    .unwrap();
    let started_stage_execution_id = StageExecutionId::new();
    stages::insert(
        &pool,
        &StageExecution {
            id: started_stage_execution_id,
            run_id,
            stage_id: "state_1".into(),
            label: "State 1".into(),
            status: StageStatus::Running,
            iteration: 1,
            attempt_number: 3,
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
            retry_reason: Some("operator_retry".into()),
        },
    )
    .await
    .unwrap();

    let journal_id = uuid::Uuid::new_v4().to_string();
    let run_id_str = run_id.to_string();
    command_journal::record(
        &pool,
        &journal_id,
        "OverrideLegacyDiscoveryPolicy",
        "{}",
        Some(&run_id_str),
        Utc::now(),
        Some("mcp"),
        Some("operator-1"),
        Some("operator"),
        Some("legacy_discovery_override_create"),
        None,
    )
    .await
    .unwrap();
    let started_journal_id = uuid::Uuid::new_v4().to_string();
    command_journal::record(
        &pool,
        &started_journal_id,
        "OverrideLegacyDiscoveryPolicy",
        "{}",
        Some(&run_id_str),
        Utc::now(),
        Some("mcp"),
        Some("operator-1"),
        Some("operator"),
        Some("legacy_discovery_override_create"),
        None,
    )
    .await
    .unwrap();

    let mut tx = pool.begin().await.unwrap();
    let input = LegacyDiscoveryOverrideInput {
        run_id,
        stage_id: "state_1".into(),
        workflow_id: "wf".into(),
        target_stage_execution_id: pending_stage_execution_id,
        target_attempt_number: 2,
        actor_id: "operator-1".into(),
        reason: "legacy workflow lacks declared outputs".into(),
        requested_policy: LegacyBroadDiscoveryPolicy::WorkflowOptIn,
        from_policy: LegacyBroadDiscoveryPolicy::Disabled,
        approval_source: "legacy_discovery_override_create".into(),
        journal_id,
    };
    legacy_discovery_overrides::create_for_pending_retry_tx(&mut tx, &input)
        .await
        .unwrap();
    let duplicate_error = legacy_discovery_overrides::create_for_pending_retry_tx(&mut tx, &input)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        duplicate_error.contains("already exists"),
        "unexpected duplicate error: {duplicate_error}"
    );

    let started_error = legacy_discovery_overrides::create_for_pending_retry_tx(
        &mut tx,
        &LegacyDiscoveryOverrideInput {
            target_stage_execution_id: started_stage_execution_id,
            target_attempt_number: 3,
            journal_id: started_journal_id,
            ..input
        },
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(
        started_error.contains("pending retry attempt"),
        "unexpected started-target error: {started_error}"
    );
}
