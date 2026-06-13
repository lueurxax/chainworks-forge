use std::collections::BTreeMap;
use std::sync::Arc;

use chrono::Utc;
use db::pool::create_pool;
use db::repos::{
    agent_execution_discovery_diagnostics, agent_execution_runtime_facts, agent_executions,
    agent_retry_budget_ledger, escalation, ideas, runs, sessions, stages,
};
use domain::agent::{
    AgentExecution, AgentExecutionRuntimeFacts, AgentFailureKind, AgentOutputSettlement,
};
use domain::discovery::{
    AgentExecutionDiscoveryDiagnostics, DiscoveryDiagnosticsV1, ExpectedOutputRole,
    OutputDiscoveryDecision, OutputDiscoveryProvenance, OutputDiscoveryReason,
    OutputDiscoveryStatus, DISCOVERY_DIAGNOSTICS_V1_SCHEMA_VERSION,
};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{AgentExecutionId, IdeaId, RunId, StageExecutionId};
use domain::run::{Run, RunStatus};
use domain::session::{SessionGeneration, SessionGenerationStatus, SessionLineage};
use domain::stage::{StageExecution, StageStatus};
use engine::command_handler::CommandHandler;
use engine::event_bus;
use engine::work_queue::WorkQueue;
use mcp_server::protocol::JsonRpcRequest;
use mcp_server::server::McpServer;

async fn test_pool() -> sqlx::SqlitePool {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    db::writer::register_shared_writer(
        &pool,
        std::sync::Arc::new(db::writer::DbWriter::new(pool.clone())),
    )
    .await
    .unwrap();
    pool
}

fn make_run(run_id: RunId, idea_id: IdeaId) -> Run {
    Run {
        id: run_id,
        idea_id,
        status: RunStatus::Ready,
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
        workflow_snapshot_json: None,
        catalog_snapshot_json: None,
        drift_detected_at: None,
        drift_details_json: None,
        chainworks_meta_root: None,
        review_routing_json: None,
        closeout_readiness_mode: None,
    }
}

async fn setup_pool() -> sqlx::SqlitePool {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let writer = Arc::new(db::writer::DbWriter::new(pool.clone()));
    db::writer::register_shared_writer(&pool, writer)
        .await
        .unwrap();
    pool
}

async fn seed_execution(pool: &sqlx::SqlitePool) -> (RunId, StageExecutionId, AgentExecutionId) {
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
            status: domain::agent::AgentStatus::Running,
            owner_execution_lineage_id: Some("lineage-owner-1".into()),
            session_lineage_id: Some("session-lineage-1".into()),
            session_generation_id: Some("session-generation-1".into()),
            rehydrated_from_checkpoint_artifact_id: Some("checkpoint-1".into()),
            invocation_owner_key: Some("owner-key".into()),
            session_reuse_scope: Some("same_agent_family_within_run".into()),
            session_family_id: Some("family-1".into()),
            session_reuse_disposition: Some("fresh".into()),
            session_reset_reason: None,
            backend_profile_id: Some("codex_with_mcp".into()),
            requested_mcp_extensions_json: Some(r#"["filesystem"]"#.into()),
            predicted_mcp_extensions_json: Some(r#"["filesystem"]"#.into()),
            predicted_mcp_runtime_ids_json: Some(r#"["fs-runtime"]"#.into()),
            actual_mcp_extensions_json: Some(r#"["filesystem"]"#.into()),
            actual_mcp_runtime_ids_json: Some(r#"["fs-runtime"]"#.into()),
            denied_mcp_extensions_json: Some("[]".into()),
            mcp_blocking_issues_json: Some("[]".into()),
            actual_mcp_observation_json: Some(
                r#"{"source":"provider_session_new_response"}"#.into(),
            ),
            actual_xcode_runtime_observation_json: None,
            mcp_session_startup_latency_ms: Some(17),
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

    sessions::insert_lineage(
        pool,
        &SessionLineage {
            id: "session-lineage-1".into(),
            run_id: run_id.to_string(),
            agent_id: "code_writer".into(),
            lineage_id: "session-family-1".into(),
            session_reuse_scope: "same_agent_family_within_run".into(),
            session_family_id: Some("family-1".into()),
            active_generation_id: Some("session-generation-1".into()),
            created_at: Utc::now(),
            closed_at: None,
        },
    )
    .await
    .unwrap();
    sessions::insert_generation(
        pool,
        &SessionGeneration {
            id: "session-generation-1".into(),
            lineage_id: "session-lineage-1".into(),
            generation: 1,
            invocation_owner_key: "owner-key".into(),
            provider_session_id: Some("provider-session-1".into()),
            binding_fingerprint: "fingerprint-1".into(),
            rehydrated_from_checkpoint_artifact_id: Some("checkpoint-1".into()),
            working_directory: "/tmp/ws".into(),
            workspace_mode: "workspace".into(),
            runtime_provider: "claude".into(),
            runtime_model: "sonnet".into(),
            status: SessionGenerationStatus::Active,
            turn_count: 0,
            estimated_input_tokens: 0,
            latest_cached_input_tokens: None,
            latest_output_tokens: None,
            latest_model_context_window: None,
            cumulative_prompt_tokens: 0,
            cumulative_cost_cents: 0,
            created_at: Utc::now(),
            last_activity_at: None,
            ended_at: None,
            end_reason: None,
        },
    )
    .await
    .unwrap();

    (run_id, stage_execution_id, agent_execution_id)
}

fn make_command_handler(pool: sqlx::SqlitePool) -> CommandHandler {
    let events = event_bus::new_bus(32);
    CommandHandler::new(pool.clone(), events.clone(), WorkQueue::new(pool))
}

fn accepted_discovery_decision(agent_execution_id: AgentExecutionId) -> OutputDiscoveryDecision {
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

fn stale_discovery_decision(agent_execution_id: AgentExecutionId) -> OutputDiscoveryDecision {
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
async fn proposal_058_reports_get_includes_runtime_facts_with_snake_case_fields() {
    let pool = test_pool().await;

    let (run_id, stage_execution_id, agent_execution_id) = seed_execution(&pool).await;
    sqlx::query("UPDATE agent_executions SET session_reuse_disposition = ?1 WHERE id = ?2")
        .bind("fresh_after_transport_error")
        .bind(agent_execution_id.to_string())
        .execute(&pool)
        .await
        .unwrap();
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
    facts.failure_message_redacted = Some("limit resets 10pm (Asia/Nicosia)".into());
    facts.retry_after = Some(now);
    facts.operator_action_hint = Some(domain::agent::OperatorActionHint::WaitUntilRetryAfter);
    facts.output_settlement = AgentOutputSettlement::MissingRequiredOutputs;
    facts.valid_required_outputs = false;
    facts.late_output_count = 2;
    facts.ignored_late_output_count = 1;
    facts.session_reuse_reason = Some("same_family_within_run".into());
    facts.quota_ledger_id = Some(ledger.id.clone());
    agent_execution_runtime_facts::upsert(&pool, &facts)
        .await
        .unwrap();

    let payload = mcp_server::tools::reports::execute(
        "reports.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &make_command_handler(pool.clone()),
        &auth::Principal::new("operator", auth::PrincipalClass::Operator),
    )
    .await
    .unwrap();

    let canonical = payload["reports"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["report_kind"] == serde_json::json!("mcp_execution_truth"))
        .expect("mcp execution truth report");
    let execution = &canonical["agent_executions"][0];
    let runtime_facts = &execution["runtime_facts"];

    assert_eq!(
        execution["agent_execution_id"],
        agent_execution_id.to_string()
    );
    assert_eq!(
        execution["backend_profile_id"],
        serde_json::json!("codex_with_mcp")
    );
    assert_eq!(
        runtime_facts["agent_execution_id"],
        agent_execution_id.to_string()
    );
    assert_eq!(runtime_facts["failure_kind"], "provider_quota");
    assert_eq!(
        runtime_facts["failure_kind_raw_debug"],
        "future_provider_quota_variant"
    );
    assert_eq!(runtime_facts["failure_kind_version"], 1);
    assert_eq!(
        runtime_facts["failure_message_redacted"],
        "limit resets 10pm (Asia/Nicosia)"
    );
    assert_eq!(runtime_facts["retry_after"], now.to_rfc3339());
    assert_eq!(
        runtime_facts["operator_action_hint"],
        "wait_until_retry_after"
    );
    assert_eq!(
        runtime_facts["output_settlement"],
        "missing_required_outputs"
    );
    assert_eq!(runtime_facts["valid_required_outputs"], false);
    assert_eq!(runtime_facts["late_output_count"], 2);
    assert_eq!(runtime_facts["ignored_late_output_count"], 1);
    assert_eq!(
        runtime_facts["session_reuse_reason"],
        "same_family_within_run"
    );
    assert_eq!(
        runtime_facts["fresh_provider_process"],
        serde_json::json!(true)
    );
    assert_eq!(runtime_facts["provider_session_id"], "provider-session-1");
    assert_eq!(
        runtime_facts["active_session_generation_id"],
        "session-generation-1"
    );
    assert_eq!(runtime_facts["active_generation_matches_execution"], true);
    assert_eq!(runtime_facts["generation_status"], "active");
    assert_eq!(runtime_facts["quota_ledger_id"], ledger.id);
    assert!(runtime_facts["created_at"].is_string());
    assert!(runtime_facts["updated_at"].is_string());

    let observer_payload = mcp_server::tools::reports::execute(
        "reports.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &make_command_handler(pool.clone()),
        &auth::Principal::new("observer", auth::PrincipalClass::Observer),
    )
    .await
    .unwrap();
    let observer_canonical = observer_payload["reports"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["report_kind"] == serde_json::json!("mcp_execution_truth"))
        .expect("observer mcp execution truth report");
    assert_eq!(
        observer_canonical["agent_executions"][0]["runtime_facts"]["failure_kind_raw_debug"],
        serde_json::Value::Null
    );

    let server = McpServer::new(
        pool.clone(),
        Arc::new(make_command_handler(pool.clone())),
        auth::PrincipalTable::test_fixture(),
    );
    let resource_response = server
        .handle_request(
            JsonRpcRequest {
                jsonrpc: "2.0".into(),
                id: Some(serde_json::json!(1)),
                method: "resources/read".into(),
                params: Some(serde_json::json!({
                    "uri": format!("report://{}", run_id),
                })),
            },
            &auth::Principal::new("operator", auth::PrincipalClass::Operator),
        )
        .await;
    let observer_server = McpServer::new(
        pool.clone(),
        Arc::new(make_command_handler(pool.clone())),
        auth::PrincipalTable::test_fixture(),
    );
    let observer_resource_response = observer_server
        .handle_request(
            JsonRpcRequest {
                jsonrpc: "2.0".into(),
                id: Some(serde_json::json!(99)),
                method: "resources/read".into(),
                params: Some(serde_json::json!({ "uri": format!("report://{}", run_id) })),
            },
            &auth::Principal::new("obs-denial", auth::PrincipalClass::Observer),
        )
        .await;
    assert!(
        observer_resource_response.error.is_none(),
        "Observer report:// read should succeed with redacted payload: {:?}",
        observer_resource_response.error
    );
    let observer_resource_text = observer_resource_response.result.as_ref().unwrap()["contents"][0]
        ["text"]
        .as_str()
        .expect("observer resource text");
    let observer_resource_payload: serde_json::Value =
        serde_json::from_str(observer_resource_text).unwrap();
    assert_eq!(
        observer_resource_payload["agent_executions"][0]["runtime_facts"]["failure_kind_raw_debug"],
        serde_json::Value::Null,
        "Observer report:// payload must redact raw runtime facts"
    );

    assert!(
        resource_response.error.is_none(),
        "Operator resource read error: {:?}",
        resource_response.error
    );
    let resource_text = resource_response.result.as_ref().unwrap()["contents"][0]["text"]
        .as_str()
        .expect("resource text");
    let resource_payload: serde_json::Value = serde_json::from_str(resource_text).unwrap();
    let resource_runtime_facts = &resource_payload["agent_executions"][0]["runtime_facts"];
    assert_eq!(
        resource_runtime_facts["agent_execution_id"],
        agent_execution_id.to_string()
    );
    assert_eq!(resource_runtime_facts["failure_kind"], "provider_quota");
    // Operator sees the unredacted raw_debug value.
    assert_eq!(
        resource_runtime_facts["failure_kind_raw_debug"],
        "future_provider_quota_variant"
    );
    assert_eq!(
        resource_runtime_facts["failure_message_redacted"],
        "limit resets 10pm (Asia/Nicosia)"
    );
    assert_eq!(
        resource_runtime_facts["session_generation_id"],
        "session-generation-1"
    );
    assert_eq!(
        resource_runtime_facts["fresh_provider_process"],
        serde_json::json!(true)
    );
    assert_eq!(resource_runtime_facts["quota_ledger_id"], ledger.id);
}

#[tokio::test]
async fn proposal_053_reports_get_projects_discovery_reconciliation_pending() {
    let pool = test_pool().await;

    let (run_id, _stage_execution_id, agent_execution_id) = seed_execution(&pool).await;
    let now = Utc::now();
    let diagnostics = AgentExecutionDiscoveryDiagnostics::from_payload(
        discovery_payload(
            agent_execution_id,
            vec![
                accepted_discovery_decision(agent_execution_id),
                stale_discovery_decision(agent_execution_id),
            ],
            now,
        ),
        now,
    );
    agent_execution_discovery_diagnostics::upsert(&pool, &diagnostics)
        .await
        .unwrap();
    let mut facts = AgentExecutionRuntimeFacts::defaults_for(agent_execution_id, now);
    facts.output_settlement = AgentOutputSettlement::ValidOutputsFromCompletedExecution;
    facts.valid_required_outputs = true;
    agent_execution_runtime_facts::upsert(&pool, &facts)
        .await
        .unwrap();

    let payload = mcp_server::tools::reports::execute(
        "reports.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &make_command_handler(pool.clone()),
        &auth::Principal::new("operator", auth::PrincipalClass::Operator),
    )
    .await
    .unwrap();

    let canonical = payload["reports"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["report_kind"] == serde_json::json!("mcp_execution_truth"))
        .expect("mcp execution truth report");
    let execution = &canonical["agent_executions"][0];
    assert_eq!(
        execution["discovery_diagnostics"]["reconciliation_pending"],
        true
    );
    assert_eq!(
        execution["discovery_diagnostics"]["stale_output_count"],
        serde_json::json!(1)
    );
    assert_eq!(
        execution["discovery_diagnostics"]["missing_required_output_count"],
        serde_json::json!(1)
    );
    assert_eq!(
        execution["discovery_diagnostics"]["payload"]["resume_warnings"],
        serde_json::json!(["reconciliation_pending"])
    );
    assert_eq!(
        execution["discovery_diagnostics"]["runtime_facts_present"],
        true
    );
    assert_eq!(
        execution["discovery_diagnostics"]["matching_active_artifact_generation_count"],
        0
    );
    assert_eq!(
        execution["runtime_facts"]["output_settlement"],
        "valid_outputs_from_completed_execution"
    );
    assert_eq!(execution["runtime_facts"]["valid_required_outputs"], false);
}

// ── SEC-004: non-Operator MCP readback authz contract ─────────────────────────

/// SEC-004: build_escalation_readback_summary_json must NOT include dominant_pause_reason_raw.
/// Agent/Observer principals see only paused_chain_count and has_active_escalation.
#[tokio::test]
async fn p058_sec004_non_operator_summary_excludes_dominant_pause_reason() {
    use domain::escalation::EscalationLedger;
    use domain::ids::RunId;
    use mcp_server::tools::runs::build_escalation_readback_summary_json;

    let pool = setup_pool().await;
    let run_id = RunId::new();
    let idea_id = domain::ids::IdeaId::new();
    ideas::insert(
        &pool,
        &domain::idea::Idea {
            id: idea_id,
            title: "sec004 test".into(),
            body: "authz".into(),
            workspace_root_path: None,
            project_key: None,
            status: domain::idea::IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        },
    )
    .await
    .unwrap();
    runs::insert(
        &pool,
        &domain::run::Run {
            id: run_id,
            idea_id,
            status: domain::run::RunStatus::Running,
            workflow_id: "wf".into(),
            workflow_title: "Workflow".into(),
            workspace_root: "/tmp".into(),
            artifact_root: "/tmp".into(),
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

    let now = Utc::now();
    // Insert a paused ledger so there IS a pause reason that could be leaked.
    let ledger = EscalationLedger {
        id: "ledger-sec004-authz".into(),
        run_id,
        stage_id: "state_3".into(),
        agent_id: "code_writer".into(),
        policy_id: "policy-sec004".into(),
        policy_hash: "sha256:s4authz".into(),
        status_raw: "paused".into(),
        current_tier_id: None,
        current_tier_kind_raw: None,
        chain_attempt_index: 1,
        trigger_raw: Some("contract_output_failure".into()),
        pause_reason_raw: Some("escalation_chain_exhausted".into()),
        operator_action_hint: None,
        runbook_anchor: None,
        created_at: now,
        updated_at: now,
    };
    escalation::insert_ledger(&pool, &ledger).await.unwrap();

    let summary = build_escalation_readback_summary_json(&pool, run_id)
        .await
        .unwrap();

    assert!(
        summary.get("dominant_pause_reason_raw").is_none(),
        "non-Operator summary must not expose dominant_pause_reason_raw (SEC-004); got: {summary:?}"
    );
    assert_eq!(
        summary.get("paused_chain_count").and_then(|v| v.as_i64()),
        Some(1),
        "paused_chain_count must reflect the paused ledger"
    );
    assert_eq!(
        summary
            .get("has_active_escalation")
            .and_then(|v| v.as_bool()),
        Some(true),
    );
    assert_eq!(
        summary.get("chains_redacted").and_then(|v| v.as_bool()),
        Some(true),
    );
}

// ── SEC HIGH-001: run:// resource must not leak snapshot fields to non-Operator ─────────────────

/// SEC HIGH-001: Agent principals must not recover catalog_snapshot_json (which contains frozen
/// escalation policies) or other operator-only fields via the run:// resource surface.
/// Operator readback must be unaffected.
#[tokio::test]
async fn p058_sec001_run_resource_agent_cannot_see_snapshot_fields() {
    let pool = setup_pool().await;
    let idea_id = domain::ids::IdeaId::new();
    let run_id = RunId::new();

    ideas::insert(
        &pool,
        &domain::idea::Idea {
            id: idea_id,
            title: "sec001 run-resource test".into(),
            body: "escalation policy leak prevention".into(),
            workspace_root_path: None,
            project_key: None,
            status: domain::idea::IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        },
    )
    .await
    .unwrap();

    runs::insert(
        &pool,
        &domain::run::Run {
            id: run_id,
            idea_id,
            status: domain::run::RunStatus::Running,
            workflow_id: "wf-snap".into(),
            workflow_title: "Snapshot Run".into(),
            workspace_root: "/tmp/ws".into(),
            artifact_root: "/tmp/art".into(),
            started_at: Utc::now(),
            completed_at: None,
            cancellation_requested_at: None,
            cancellation_settled_at: None,
            cancellation_settlement_log: None,
            current_state: Some("state_1".into()),
            workflow_yaml_path: Some("/workspace/workflows/main.yaml".into()),
            agent_catalog_yaml_path: Some("/workspace/agents/agents.yaml".into()),
            worktree_root: Some("/tmp/worktrees/cw-test".into()),
            base_branch: Some("main".into()),
            base_revision: None,
            target_branch: Some("cw/feature".into()),
            delivery_configuration_json: Some(r#"{"repo_identifier":"repo-snap"}"#.into()),
            delivery_preflight_json: Some(r#"{"passed":true}"#.into()),
            workflow_family: None,
            project_key: None,
            risk_class: None,
            stack: None,
            workflow_snapshot_hash: Some("sha256:wf-snap".into()),
            catalog_snapshot_hash: Some("sha256:cat-snap".into()),
            workflow_snapshot_json: Some(r#"{"states":{"state_1":{}}}"#.into()),
            catalog_snapshot_json: Some(
                r#"{"escalation_policies":[{"policy_id":"p_secret"}]}"#.into(),
            ),
            drift_detected_at: None,
            drift_details_json: Some(r#"{"policy_hash_mismatch":true}"#.into()),
            chainworks_meta_root: Some("/Users/user/Documents/Chainworks Forge/.chainworks".into()),
            review_routing_json: None,
            closeout_readiness_mode: None,
        },
    )
    .await
    .unwrap();

    let server = McpServer::new(
        pool.clone(),
        Arc::new(make_command_handler(pool.clone())),
        auth::PrincipalTable::test_fixture(),
    );

    let operator_only_fields = [
        "catalog_snapshot_json",
        "workflow_snapshot_json",
        "delivery_configuration_json",
        "delivery_preflight_json",
        "drift_details_json",
        "chainworks_meta_root",
        "workflow_yaml_path",
        "agent_catalog_yaml_path",
        "worktree_root",
    ];

    // Agent principal must not receive any operator-only snapshot fields via run://.
    let agent_response = server
        .handle_request(
            JsonRpcRequest {
                jsonrpc: "2.0".into(),
                id: Some(serde_json::json!(1)),
                method: "resources/read".into(),
                params: Some(serde_json::json!({ "uri": format!("run://{}", run_id) })),
            },
            &auth::Principal::new("agent-sec001", auth::PrincipalClass::Agent),
        )
        .await;
    assert!(
        agent_response.error.is_none(),
        "run:// resource read error for agent: {:?}",
        agent_response.error
    );
    let agent_text = agent_response.result.as_ref().unwrap()["contents"][0]["text"]
        .as_str()
        .expect("agent resource text");
    let agent_run: serde_json::Value = serde_json::from_str(agent_text).unwrap();
    for field in &operator_only_fields {
        assert!(
            agent_run.get(*field).is_none(),
            "Agent must not receive {field} via run:// (SEC HIGH-001); got: {agent_run:?}"
        );
    }

    // Observer principal must also be redacted.
    let observer_response = server
        .handle_request(
            JsonRpcRequest {
                jsonrpc: "2.0".into(),
                id: Some(serde_json::json!(2)),
                method: "resources/read".into(),
                params: Some(serde_json::json!({ "uri": format!("run://{}", run_id) })),
            },
            &auth::Principal::new("obs-sec001", auth::PrincipalClass::Observer),
        )
        .await;
    let observer_text = observer_response.result.as_ref().unwrap()["contents"][0]["text"]
        .as_str()
        .expect("observer resource text");
    let observer_run: serde_json::Value = serde_json::from_str(observer_text).unwrap();
    for field in &operator_only_fields {
        assert!(
            observer_run.get(*field).is_none(),
            "Observer must not receive {field} via run:// (SEC HIGH-001)"
        );
    }

    // Operator principal must still receive the full run including snapshot fields.
    let operator_response = server
        .handle_request(
            JsonRpcRequest {
                jsonrpc: "2.0".into(),
                id: Some(serde_json::json!(3)),
                method: "resources/read".into(),
                params: Some(serde_json::json!({ "uri": format!("run://{}", run_id) })),
            },
            &auth::Principal::new("op-sec001", auth::PrincipalClass::Operator),
        )
        .await;
    let operator_text = operator_response.result.as_ref().unwrap()["contents"][0]["text"]
        .as_str()
        .expect("operator resource text");
    let operator_run: serde_json::Value = serde_json::from_str(operator_text).unwrap();
    assert!(
        operator_run.get("catalog_snapshot_json").is_some(),
        "Operator must receive catalog_snapshot_json via run:// (SEC HIGH-001)"
    );
    assert!(
        operator_run.get("workflow_snapshot_json").is_some(),
        "Operator must receive workflow_snapshot_json via run://"
    );
    assert!(
        operator_run.get("chainworks_meta_root").is_some(),
        "Operator must receive chainworks_meta_root via run://"
    );
}
