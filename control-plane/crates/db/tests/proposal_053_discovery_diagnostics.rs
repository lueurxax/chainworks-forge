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
    AgentExecutionDiscoveryDiagnostics, BoundedMetaRootDiscovery, DiscoveryDiagnosticsV1,
    ExpectedOutputRole, LegacyBroadDiscoveryPolicy, LegacyDiscoveryOverrideInput,
    LegacyDiscoveryOverrideStatus, OutputDiscoveryDecision, OutputDiscoveryProvenance,
    OutputDiscoveryReason, OutputDiscoveryStatus, DISCOVERY_DIAGNOSTICS_V1_SCHEMA_VERSION,
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
        review_routing_json: None,
        closeout_readiness_mode: None,
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
            stage_execution_id: Some(stage_execution_id),
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

fn stale_decision(agent_execution_id: AgentExecutionId) -> OutputDiscoveryDecision {
    OutputDiscoveryDecision {
        output_name: "proposal_review".into(),
        output_role: ExpectedOutputRole::Machine,
        target_path: "proposal_review.json".into(),
        companion_of: None,
        status: OutputDiscoveryStatus::Missing,
        reason: OutputDiscoveryReason::StaleExpectedOutput,
        provenance: Some(OutputDiscoveryProvenance::ExactPath),
        canonical_path: Some("/tmp/artifacts/proposal_review.json".into()),
        root_class: None,
        baseline_status: None,
        size_bytes: Some(128),
        content_digest: Some("sha256:stale".into()),
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

fn discovery_payload(
    agent_execution_id: AgentExecutionId,
    decisions: Vec<OutputDiscoveryDecision>,
    now: chrono::DateTime<Utc>,
) -> DiscoveryDiagnosticsV1 {
    DiscoveryDiagnosticsV1 {
        schema_version: DISCOVERY_DIAGNOSTICS_V1_SCHEMA_VERSION.to_string(),
        agent_execution_id: agent_execution_id.to_string(),
        decisions,
        pre_prompt_expected_outputs: Vec::new(),
        legacy_broad_discovery_used: false,
        bounded_meta_root_discovery: None,
        git_manifest_status: None,
        resume_warnings: Vec::new(),
        warnings: Vec::new(),
        generated_at: now,
        acp_pre_initialize_local_latency_ms: None,
        acp_initialize_latency_ms: None,
        acp_session_new_latency_ms: None,
        acp_prompt_duration_ms: None,
        acp_pre_prompt_metadata_latency_ms: None,
        acp_pre_prompt_metadata_timeout: None,
        acp_pre_prompt_metadata_digest_bytes: None,
        acp_expected_output_spec_count: None,
        acp_control_plane_manifest_latency_ms: None,
        acp_exact_output_acceptance_latency_ms: None,
        acp_meta_root_discovery_latency_ms: None,
        acp_git_changed_files_latency_ms: None,
        acp_expected_outputs_found_count: None,
        acp_expected_outputs_missing_count: None,
        acp_expected_outputs_stale_count: None,
        acp_expected_outputs_rejected_count: None,
        acp_meta_discovery_truncated: None,
        acp_meta_discovery_truncation_reason: None,
        acp_legacy_broad_discovery_policy: None,
        acp_legacy_broad_discovery_used: None,
        acp_git_manifest_status: None,
        acp_resume_discovery_warning: None,
        acp_discovery_schema_version: None,
        acp_discovery_override_status: None,
        acp_missing_required_output_count: None,
        acp_rejected_output_count: None,
        acp_stale_output_count: None,
        acp_exact_output_acceptance_timeout: None,
        acp_exact_output_aggregate_bytes: None,
        acp_exact_output_aggregate_cap_hit: None,
        acp_cap_validation_sample_size: None,
        acp_cap_validation_p90_output_bytes: None,
        acp_cap_validation_p90_aggregate_bytes: None,
        acp_legacy_broad_discovery_timeout_ms: None,
        acp_legacy_broad_discovery_truncation_reason: None,
        acp_reconciliation_pending: None,
    }
}

#[tokio::test]
async fn proposal_053_discovery_diagnostics_roundtrip_and_list_by_run() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let (run_id, agent_execution_id) = seed_execution(&pool).await;
    let now = Utc::now();
    let mut payload = discovery_payload(
        agent_execution_id,
        vec![
            missing_decision(agent_execution_id),
            stale_decision(agent_execution_id),
        ],
        now,
    );
    payload.legacy_broad_discovery_used = true;
    payload.bounded_meta_root_discovery = Some(BoundedMetaRootDiscovery {
        root_path: "/tmp/ws/.chainworks/runs/current".into(),
        artifact_paths: vec!["run/report.json".into()],
        files_visited: 3,
        total_bytes: 2048,
        latency_ms: Some(33),
        truncated_by_file_cap: false,
        truncated_by_file_size: false,
        truncated_by_total_bytes: true,
        warnings: vec!["meta_root_total_bytes_cap_hit".into()],
    });
    payload.git_manifest_status = Some("not_git_repository".into());
    payload.resume_warnings = vec!["reconciliation_pending".into()];
    payload.acp_pre_initialize_local_latency_ms = Some(11);
    payload.acp_initialize_latency_ms = Some(12);
    payload.acp_session_new_latency_ms = Some(13);
    payload.acp_prompt_duration_ms = Some(14);
    payload.acp_pre_prompt_metadata_latency_ms = Some(15);
    payload.acp_pre_prompt_metadata_timeout = Some(false);
    payload.acp_pre_prompt_metadata_digest_bytes = Some(16);
    payload.acp_expected_output_spec_count = Some(2);
    payload.acp_control_plane_manifest_latency_ms = Some(17);
    payload.acp_exact_output_acceptance_latency_ms = Some(18);
    payload.acp_meta_root_discovery_latency_ms = Some(33);
    payload.acp_git_changed_files_latency_ms = Some(19);
    payload.acp_legacy_broad_discovery_policy = Some("workflow_opt_in".into());
    payload.acp_legacy_broad_discovery_used = Some(true);
    payload.acp_git_manifest_status = Some("not_git_repository".into());
    payload.acp_resume_discovery_warning = Some("reconciliation_pending".into());
    payload.acp_discovery_schema_version =
        Some(DISCOVERY_DIAGNOSTICS_V1_SCHEMA_VERSION.to_string());
    payload.acp_discovery_override_status = Some("not_requested".into());
    payload.acp_exact_output_acceptance_timeout = Some(false);
    payload.acp_exact_output_aggregate_bytes = Some(20);
    payload.acp_exact_output_aggregate_cap_hit = Some(false);
    payload.acp_cap_validation_sample_size = Some(6);
    payload.acp_cap_validation_p90_output_bytes = Some(1024);
    payload.acp_cap_validation_p90_aggregate_bytes = Some(4096);
    payload.acp_legacy_broad_discovery_timeout_ms = Some(5000);
    payload.acp_legacy_broad_discovery_truncation_reason = Some("total_bytes_cap".into());
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
    assert_eq!(read.missing_required_output_count, 2);
    assert_eq!(read.rejected_output_count, 0);
    assert_eq!(read.stale_output_count, 1);
    assert_eq!(read.resume_warning_count, 1);
    assert_eq!(read.payload.decisions.len(), 2);
    let projected = &read.payload;
    assert_eq!(projected.acp_pre_initialize_local_latency_ms, Some(11));
    assert_eq!(projected.acp_initialize_latency_ms, Some(12));
    assert_eq!(projected.acp_session_new_latency_ms, Some(13));
    assert_eq!(projected.acp_prompt_duration_ms, Some(14));
    assert_eq!(projected.acp_pre_prompt_metadata_latency_ms, Some(15));
    assert_eq!(projected.acp_pre_prompt_metadata_timeout, Some(false));
    assert_eq!(projected.acp_pre_prompt_metadata_digest_bytes, Some(16));
    assert_eq!(projected.acp_expected_output_spec_count, Some(2));
    assert_eq!(projected.acp_control_plane_manifest_latency_ms, Some(17));
    assert_eq!(projected.acp_exact_output_acceptance_latency_ms, Some(18));
    assert_eq!(projected.acp_meta_root_discovery_latency_ms, Some(33));
    assert_eq!(projected.acp_git_changed_files_latency_ms, Some(19));
    assert_eq!(projected.acp_expected_outputs_found_count, Some(0));
    assert_eq!(projected.acp_expected_outputs_missing_count, Some(2));
    assert_eq!(projected.acp_expected_outputs_stale_count, Some(1));
    assert_eq!(projected.acp_expected_outputs_rejected_count, Some(0));
    assert_eq!(projected.acp_meta_discovery_truncated, Some(true));
    assert_eq!(
        projected.acp_meta_discovery_truncation_reason.as_deref(),
        Some("total_bytes")
    );
    assert_eq!(
        projected.acp_legacy_broad_discovery_policy.as_deref(),
        Some("workflow_opt_in")
    );
    assert_eq!(projected.acp_legacy_broad_discovery_used, Some(true));
    assert_eq!(
        projected.acp_git_manifest_status.as_deref(),
        Some("not_git_repository")
    );
    assert_eq!(
        projected.acp_resume_discovery_warning.as_deref(),
        Some("reconciliation_pending")
    );
    assert_eq!(
        projected.acp_discovery_schema_version.as_deref(),
        Some(DISCOVERY_DIAGNOSTICS_V1_SCHEMA_VERSION)
    );
    assert_eq!(
        projected.acp_discovery_override_status.as_deref(),
        Some("not_requested")
    );
    assert_eq!(projected.acp_missing_required_output_count, Some(2));
    assert_eq!(projected.acp_rejected_output_count, Some(0));
    assert_eq!(projected.acp_stale_output_count, Some(1));
    assert_eq!(projected.acp_exact_output_acceptance_timeout, Some(false));
    assert_eq!(projected.acp_exact_output_aggregate_bytes, Some(20));
    assert_eq!(projected.acp_exact_output_aggregate_cap_hit, Some(false));
    assert_eq!(projected.acp_cap_validation_sample_size, Some(6));
    assert_eq!(projected.acp_cap_validation_p90_output_bytes, Some(1024));
    assert_eq!(projected.acp_cap_validation_p90_aggregate_bytes, Some(4096));
    assert_eq!(projected.acp_legacy_broad_discovery_timeout_ms, Some(5000));
    assert_eq!(
        projected
            .acp_legacy_broad_discovery_truncation_reason
            .as_deref(),
        Some("total_bytes_cap")
    );

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
    let payload = discovery_payload(
        agent_execution_id,
        vec![accepted_decision(agent_execution_id)],
        now,
    );
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
