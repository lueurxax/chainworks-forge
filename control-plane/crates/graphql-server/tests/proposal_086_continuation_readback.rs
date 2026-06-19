use std::sync::Arc;

use async_graphql::Request;
use chrono::Utc;
use db::pool::create_pool;
use db::repos::{agent_work_continuations, ideas, runs, sessions, stages};
use domain::agent::{AgentExecution, AgentStatus};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{AgentExecutionId, IdeaId, RunId, StageExecutionId};
use domain::run::{Run, RunStatus};
use domain::session::{SessionGeneration, SessionGenerationStatus, SessionLineage};
use domain::stage::{StageExecution, StageStatus};
use engine::command_handler::CommandHandler;
use engine::event_bus;
use engine::lifecycle_reporter::LifecycleReporter;
use engine::work_queue::WorkQueue;
use graphql_server::schema::build_schema;

async fn test_pool() -> sqlx::SqlitePool {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    db::writer::register_shared_writer(&pool, Arc::new(db::writer::DbWriter::new(pool.clone())))
        .await
        .unwrap();
    pool
}

fn make_run(run_id: RunId, idea_id: IdeaId) -> Run {
    Run {
        id: run_id,
        idea_id,
        status: RunStatus::Running,
        workflow_id: "proposal-086".into(),
        workflow_title: "P086".into(),
        workspace_root: "/tmp/ws".into(),
        artifact_root: "/tmp/artifacts".into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        cancellation_settlement_log: None,
        current_state: Some("state_10_implementation_refined".into()),
        workflow_yaml_path: None,
        agent_catalog_yaml_path: None,
        worktree_root: Some("/tmp/ws/.chainworks/worktrees/p086".into()),
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
        chainworks_meta_root: Some("/tmp/ws/.chainworks/runs/p086".into()),
        review_routing_json: None,
        closeout_readiness_mode: None,
    }
}

async fn seed_continuation_readback(
    pool: &sqlx::SqlitePool,
) -> (RunId, StageExecutionId, AgentExecutionId, String) {
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();
    let agent_execution_id = AgentExecutionId::new();
    let session_lineage_id = "p086-lineage".to_string();
    let session_generation_id = "p086-generation-1".to_string();
    let continuation_id = "p086-continuation-readback".to_string();
    let now = Utc::now();

    ideas::insert(
        pool,
        &Idea {
            id: idea_id,
            title: "P086 idea".into(),
            body: "Body".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: now,
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
            stage_id: "state_10_implementation_refined".into(),
            label: "Implementation refined".into(),
            status: StageStatus::Completed,
            iteration: 10,
            attempt_number: 1,
            settlement_kind: None,
            started_at: now,
            completed_at: Some(now),
            owner_agent: Some("code_writer".into()),
            provider: Some("claude".into()),
            model: Some("claude-sonnet".into()),
            stage_type: Some("agent".into()),
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        },
    )
    .await
    .unwrap();
    sessions::insert_lineage(
        pool,
        &SessionLineage {
            id: session_lineage_id.clone(),
            run_id: run_id.to_string(),
            agent_id: "code_writer".into(),
            lineage_id: "lineage-code-writer".into(),
            session_reuse_scope: "run".into(),
            session_family_id: Some("family-code-writer".into()),
            active_generation_id: Some(session_generation_id.clone()),
            created_at: now,
            closed_at: None,
        },
    )
    .await
    .unwrap();
    sessions::insert_generation(
        pool,
        &SessionGeneration {
            id: session_generation_id.clone(),
            lineage_id: session_lineage_id.clone(),
            generation: 1,
            invocation_owner_key: "code_writer:state_10".into(),
            provider_session_id: Some("provider-session-p086".into()),
            binding_fingerprint: "binding-p086".into(),
            rehydrated_from_checkpoint_artifact_id: None,
            working_directory: "/tmp/ws/.chainworks/worktrees/p086".into(),
            workspace_mode: "worktree".into(),
            runtime_provider: "claude".into(),
            runtime_model: "claude-sonnet".into(),
            status: SessionGenerationStatus::Active,
            turn_count: 1,
            estimated_input_tokens: 100,
            latest_cached_input_tokens: None,
            latest_output_tokens: Some(20),
            latest_model_context_window: None,
            cumulative_prompt_tokens: 100,
            cumulative_cost_cents: 1,
            created_at: now,
            last_activity_at: Some(now),
            ended_at: None,
            end_reason: None,
        },
    )
    .await
    .unwrap();
    db::repos::agent_executions::insert(
        pool,
        &AgentExecution {
            id: agent_execution_id,
            stage_execution_id: Some(stage_execution_id),
            agent_id: "code_writer".into(),
            provider: "claude".into(),
            model: Some("claude-sonnet".into()),
            started_at: now,
            completed_at: Some(now),
            status: AgentStatus::Completed,
            owner_execution_lineage_id: None,
            session_lineage_id: Some(session_lineage_id),
            session_generation_id: Some(session_generation_id),
            rehydrated_from_checkpoint_artifact_id: None,
            invocation_owner_key: None,
            session_reuse_scope: Some("run".into()),
            session_family_id: Some("family-code-writer".into()),
            session_reuse_disposition: Some("reused".into()),
            session_reset_reason: None,
            backend_profile_id: Some("claude_code_writer_acp".into()),
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
            owner_kind: Some("stage_execution".into()),
            owner_id: Some(stage_execution_id.to_string()),
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

    let admission = agent_work_continuations::ContinuationAdmission {
        continuation_id: continuation_id.clone(),
        command_journal_id: "p086-command-readback".into(),
        run_id: run_id.to_string(),
        stage_execution_id: stage_execution_id.to_string(),
        agent_execution_id: agent_execution_id.to_string(),
        mode: "live_handle_continuation".into(),
        trigger_kind: "operator_mcp".into(),
        idempotency_scope: format!("{}:{agent_execution_id}", run_id),
        idempotency_key: "p086-readback-key".into(),
        request_fingerprint_sha256:
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
        lead_decision_artifact_id: None,
        lead_decision_artifact_sha256: None,
        continuation_instruction_sha256: None,
        budget_json: Some(r#"{"max_turns":1}"#.into()),
        caller_principal_id: "operator".into(),
        caller_surface: "mcp".into(),
        caller_principal_class: "operator".into(),
        caller_tool: "agents.continue_work".into(),
        created_at: now.to_rfc3339(),
    };
    match agent_work_continuations::admit_continuation_atomic(
        pool,
        &admission,
        r#"{"tool":"agents.continue_work"}"#,
    )
    .await
    .unwrap()
    {
        agent_work_continuations::AtomicAdmissionOutcome::Accepted => {}
        _ => panic!("expected accepted continuation admission"),
    }
    agent_work_continuations::set_canonical_request_artifact(
        pool,
        &continuation_id,
        "artifact:p086:canonical-request",
    )
    .await
    .unwrap();
    agent_work_continuations::set_evidence_artifact_ids(
        pool,
        &continuation_id,
        Some("artifact:p086:attach-receipt"),
        Some("artifact:p086:evidence-bundle"),
        Some("artifact:p086:worktree-readback"),
        Some("artifact:p086:report"),
    )
    .await
    .unwrap();
    agent_work_continuations::settle_with_artifacts(
        pool,
        &continuation_id,
        "succeeded",
        None,
        "artifact:p086:response",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "artifact:p086:result",
    )
    .await
    .unwrap();
    agent_work_continuations::record_p086_continuation_metric_event(
        pool,
        Some(&run_id.to_string()),
        Some(&stage_execution_id.to_string()),
        Some(&agent_execution_id.to_string()),
        Some(&continuation_id),
        "continuation_settlement_total",
        serde_json::json!({
            "mode": "live_handle_continuation",
            "trigger_kind": "operator_mcp",
            "terminal_status": "succeeded"
        }),
        1,
    )
    .await
    .unwrap();
    agent_work_continuations::record_p086_continuation_metric_event(
        pool,
        Some(&run_id.to_string()),
        Some(&stage_execution_id.to_string()),
        Some(&agent_execution_id.to_string()),
        Some(&continuation_id),
        "continuation_fresh_session_avoided_total",
        serde_json::json!({"outcome": "reused_existing_session"}),
        1,
    )
    .await
    .unwrap();
    for (metric_name, labels, value) in [
        (
            "continuation_changed_files_total",
            serde_json::json!({"terminal_status": "succeeded"}),
            2,
        ),
        (
            "continuation_tests_or_gates_total",
            serde_json::json!({"terminal_status": "succeeded"}),
            2,
        ),
        (
            "continuation_tests_passed_total",
            serde_json::json!({"terminal_status": "succeeded"}),
            1,
        ),
        (
            "continuation_useful_progress_total",
            serde_json::json!({"terminal_status": "succeeded"}),
            1,
        ),
        (
            "continuation_followup_validation_total",
            serde_json::json!({"validation_outcome": "success"}),
            1,
        ),
        (
            "continuation_time_saved_seconds",
            serde_json::json!({"estimate_source": "fresh_retry_estimate_seconds"}),
            120,
        ),
        (
            "continuation_provider_session_budget_input_tokens_total",
            serde_json::json!({"budget_dimension": "input_tokens"}),
            100,
        ),
        (
            "continuation_provider_session_budget_output_tokens_total",
            serde_json::json!({"budget_dimension": "output_tokens"}),
            40,
        ),
        (
            "continuation_provider_session_budget_cached_input_tokens_total",
            serde_json::json!({"budget_dimension": "cached_input_tokens"}),
            20,
        ),
        (
            "continuation_provider_session_budget_cost_cents_total",
            serde_json::json!({"budget_dimension": "cost_cents"}),
            7,
        ),
        (
            "continuation_resurrection_total",
            serde_json::json!({"resurrection_status": "unsupported"}),
            1,
        ),
    ] {
        agent_work_continuations::record_p086_continuation_metric_event(
            pool,
            Some(&run_id.to_string()),
            Some(&stage_execution_id.to_string()),
            Some(&agent_execution_id.to_string()),
            Some(&continuation_id),
            metric_name,
            labels,
            value,
        )
        .await
        .unwrap();
    }

    (
        run_id,
        stage_execution_id,
        agent_execution_id,
        continuation_id,
    )
}

#[tokio::test]
async fn p086_graphql_continuation_readback_exposes_terminal_fields_without_mutation() {
    let pool = test_pool().await;
    let (run_id, _stage_execution_id, agent_execution_id, continuation_id) =
        seed_continuation_readback(&pool).await;
    let events = event_bus::new_bus(16);
    let handler = Arc::new(CommandHandler::new(
        pool.clone(),
        events.clone(),
        WorkQueue::new(pool.clone()),
    ));
    let schema = build_schema(
        pool,
        handler,
        events.clone(),
        auth::PrincipalTable::test_fixture(),
        LifecycleReporter::new(15, "test-build", events.clone()),
    );

    let sdl = schema.sdl();
    assert!(
        !sdl.contains("continueWork(") && !sdl.contains("agentsContinueWork("),
        "P086 must remain read-only over GraphQL; continuation mutation leaked into SDL"
    );

    let response = schema
        .execute(
            Request::new(format!(
                r#"{{
                  continuationStatus(agentExecutionId: "{agent_execution_id}") {{
                    freshnessState
                    active {{ id }}
                    history {{
                      id
                      runId
                      stageExecutionId
                      agentExecutionId
                      modeRaw
                      modeDisplay
                      triggerKindRaw
                      triggerKindDisplay
                      statusRaw
                      statusDisplay
                      isTerminal
                      requestFingerprintSha256
                      canonicalRequestArtifactId
                      attachReceiptArtifactId
                      evidenceBundleArtifactId
                      worktreeReadbackArtifactId
                      continuationReportArtifactId
                      responseFingerprintSha256
                      responseArtifactId
                      resultOrNoProgressArtifactId
                    }}
                  }}
                  continuationCandidates(runId: "{run_id}") {{
                    freshnessState
                    candidates {{
                      agentExecutionId
                      runId
                      stageExecutionId
                      agentRole
                      statusRaw
                      statusDisplay
                      eligible
                      disabledReason
                      providerSessionId
                    }}
                  }}
                  continuationMetricsSummary(runId: "{run_id}") {{
                    runId
                    admissionTotal
                    acceptedTotal
                    successTotal
                    freshSessionAvoidedTotal
                    operatorMcpTotal
                    changedFilesTotal
                    testsOrGatesTotal
                    terminalTotal
                    usefulProgressTotal
                    usefulProgressRate
                    noProgressRate
                    testsPassedAfterContinuationTotal
                    followupValidationTotal
                    followupValidationSuccessTotal
                    followupValidationSuccessRate
                    operatorMcpSuccessTotal
                    operatorMcpSuccessRate
                    timeSavedSecondsTotal
                    timeSavedSampleCount
                    averageTimeSavedSeconds
                    providerSessionBudgetInputTokensTotal
                    providerSessionBudgetOutputTokensTotal
                    providerSessionBudgetCachedInputTokensTotal
                    providerSessionBudgetCostCentsTotal
                    providerSessionResurrectionAttachSuccessTotal
                    providerSessionResurrectionAttachFailureTotal
                    resurrectionUnsupportedTotal
                  }}
                  continuations(runId: "{run_id}") {{
                    id
                    statusRaw
                    responseArtifactId
                    continuationReportArtifactId
                  }}
                }}"#
            ))
            .data(auth::Principal::new(
                "operator",
                auth::PrincipalClass::Operator,
            )),
        )
        .await;

    assert!(
        response.errors.is_empty(),
        "GraphQL continuation readback failed: {:?}",
        response.errors
    );
    let data = response.data.into_json().unwrap();
    let status = &data["continuationStatus"];
    assert_eq!(status["freshnessState"], "live");
    assert!(
        status["active"].is_null(),
        "terminal row must not be active"
    );
    let row = &status["history"][0];
    assert_eq!(row["id"], continuation_id);
    assert_eq!(row["modeRaw"], "live_handle_continuation");
    assert_eq!(row["modeDisplay"], "Live Handle Continuation");
    assert_eq!(row["triggerKindRaw"], "operator_mcp");
    assert_eq!(row["triggerKindDisplay"], "Operator MCP");
    assert_eq!(row["statusRaw"], "succeeded");
    assert_eq!(row["statusDisplay"], "Succeeded");
    assert_eq!(row["isTerminal"], true);
    assert_eq!(
        row["requestFingerprintSha256"],
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert_eq!(
        row["canonicalRequestArtifactId"],
        "artifact:p086:canonical-request"
    );
    assert_eq!(
        row["attachReceiptArtifactId"],
        "artifact:p086:attach-receipt"
    );
    assert_eq!(
        row["evidenceBundleArtifactId"],
        "artifact:p086:evidence-bundle"
    );
    assert_eq!(
        row["worktreeReadbackArtifactId"],
        "artifact:p086:worktree-readback"
    );
    assert_eq!(row["continuationReportArtifactId"], "artifact:p086:report");
    assert_eq!(
        row["responseFingerprintSha256"],
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    );
    assert_eq!(row["responseArtifactId"], "artifact:p086:response");
    assert_eq!(row["resultOrNoProgressArtifactId"], "artifact:p086:result");

    let candidate = &data["continuationCandidates"]["candidates"][0];
    assert_eq!(
        candidate["agentExecutionId"],
        agent_execution_id.to_string()
    );
    assert_eq!(candidate["runId"], run_id.to_string());
    assert_eq!(candidate["agentRole"], "code_writer");
    assert_eq!(candidate["statusRaw"], "completed");
    assert_eq!(candidate["statusDisplay"], "Completed");
    assert_eq!(candidate["eligible"], true);
    assert_eq!(candidate["providerSessionId"], "provider-session-p086");

    let metrics = &data["continuationMetricsSummary"];
    assert_eq!(metrics["runId"], run_id.to_string());
    assert_eq!(metrics["admissionTotal"], 1);
    assert_eq!(metrics["acceptedTotal"], 1);
    assert_eq!(metrics["successTotal"], 1);
    assert_eq!(metrics["freshSessionAvoidedTotal"], 1);
    assert_eq!(metrics["operatorMcpTotal"], 1);
    assert_eq!(metrics["changedFilesTotal"], 2);
    assert_eq!(metrics["testsOrGatesTotal"], 2);
    assert_eq!(metrics["terminalTotal"], 1);
    assert_eq!(metrics["usefulProgressTotal"], 1);
    assert_eq!(metrics["usefulProgressRate"], 1.0);
    assert_eq!(metrics["noProgressRate"], 0.0);
    assert_eq!(metrics["testsPassedAfterContinuationTotal"], 1);
    assert_eq!(metrics["followupValidationTotal"], 1);
    assert_eq!(metrics["followupValidationSuccessTotal"], 1);
    assert_eq!(metrics["followupValidationSuccessRate"], 1.0);
    assert_eq!(metrics["operatorMcpSuccessTotal"], 1);
    assert_eq!(metrics["operatorMcpSuccessRate"], 1.0);
    assert_eq!(metrics["timeSavedSecondsTotal"], 120);
    assert_eq!(metrics["timeSavedSampleCount"], 1);
    assert_eq!(metrics["averageTimeSavedSeconds"], 120.0);
    assert_eq!(metrics["providerSessionBudgetInputTokensTotal"], 100);
    assert_eq!(metrics["providerSessionBudgetOutputTokensTotal"], 40);
    assert_eq!(metrics["providerSessionBudgetCachedInputTokensTotal"], 20);
    assert_eq!(metrics["providerSessionBudgetCostCentsTotal"], 7);
    assert_eq!(metrics["providerSessionResurrectionAttachSuccessTotal"], 0);
    assert_eq!(metrics["providerSessionResurrectionAttachFailureTotal"], 1);
    assert_eq!(metrics["resurrectionUnsupportedTotal"], 1);
    assert_eq!(data["continuations"][0]["id"], continuation_id);
    assert_eq!(data["continuations"][0]["statusRaw"], "succeeded");
    assert_eq!(
        data["continuations"][0]["responseArtifactId"],
        "artifact:p086:response"
    );
}
