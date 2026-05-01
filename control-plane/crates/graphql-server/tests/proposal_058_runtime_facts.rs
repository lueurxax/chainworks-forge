use std::collections::BTreeMap;
use std::sync::Arc;

use async_graphql::Request;
use chrono::Utc;
use db::pool::create_pool;
use db::repos::{
    agent_execution_discovery_diagnostics, agent_execution_runtime_facts, agent_executions,
    agent_retry_budget_ledger, artifact_contracts, artifacts, ideas, runs, sessions, stages,
};
use domain::agent::{
    AgentExecution, AgentExecutionRuntimeFacts, AgentFailureKind, AgentOutputSettlement,
};
use domain::artifact::{Artifact, ArtifactFormat};
use domain::artifact_contracts::ActiveArtifactGenerationInput;
use domain::discovery::{
    AgentExecutionDiscoveryDiagnostics, DiscoveryDiagnosticsV1, ExpectedOutputRole,
    OutputDiscoveryDecision, OutputDiscoveryProvenance, OutputDiscoveryReason,
    OutputDiscoveryStatus, DISCOVERY_DIAGNOSTICS_V1_SCHEMA_VERSION,
};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{AgentExecutionId, ArtifactId, IdeaId, RunId, StageExecutionId};
use domain::run::{Run, RunStatus};
use domain::session::{SessionGeneration, SessionGenerationStatus, SessionLineage};
use domain::stage::{StageExecution, StageStatus};
use engine::command_handler::CommandHandler;
use engine::event_bus;
use engine::lifecycle_reporter::LifecycleReporter;
use engine::work_queue::WorkQueue;
use graphql_server::schema::build_schema;

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

async fn seed_execution(
    pool: &sqlx::SqlitePool,
) -> (RunId, StageExecutionId, AgentExecutionId, ArtifactId) {
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();
    let agent_execution_id = AgentExecutionId::new();
    let artifact_id = ArtifactId::new();

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
            session_reuse_disposition: Some("reused".into()),
            session_reset_reason: Some("operator_reset".into()),
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

    (run_id, stage_execution_id, agent_execution_id, artifact_id)
}

fn make_schema(pool: sqlx::SqlitePool) -> graphql_server::schema::AppSchema {
    let events = event_bus::new_bus(16);
    let handler = Arc::new(CommandHandler::new(
        pool.clone(),
        events.clone(),
        WorkQueue::new(pool.clone()),
    ));
    build_schema(
        pool,
        handler,
        events.clone(),
        auth::PrincipalTable::test_fixture(),
        LifecycleReporter::new(15, "test-build", events),
    )
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
async fn proposal_058_agent_execution_exposes_runtime_facts_and_session_provenance() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let (run_id, stage_execution_id, agent_execution_id, _artifact_id) =
        seed_execution(&pool).await;
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
    facts.provider_exit_status = Some(137);
    facts.transport_error_code = Some("EPIPE".into());
    facts.supervision_classification = Some("idle_hang_after_progress".into());
    facts.output_settlement = AgentOutputSettlement::MissingRequiredOutputs;
    facts.valid_required_outputs = false;
    facts.late_output_count = 2;
    facts.ignored_late_output_count = 1;
    facts.session_reuse_reason = Some("same_family_within_run".into());
    facts.quota_ledger_id = Some(ledger.id.clone());
    agent_execution_runtime_facts::upsert(&pool, &facts)
        .await
        .unwrap();

    let schema = make_schema(pool);
    let response = schema
        .execute(
            Request::new(format!(
                r#"{{
                    stage(id: "{}") {{
                        id
                        executions {{
                            sessionReuseDisposition
                            sessionResetReason
                            ownerExecutionLineageId
                            sessionLineageId
                            sessionGenerationId
                            rehydratedFromCheckpointArtifactId
                            invocationOwnerKey
                            sessionReuseScope
                            sessionFamilyId
                            runtimeFacts {{
                                agentExecutionId
                                failureKind
                                failureKindRawDebug
                                failureKindVersion
                                failureMessageRedacted
                                failureMessageRedactionVersion
                                retryAfter
                                operatorActionHint
                                providerExitStatus
                                transportErrorCode
                                supervisionClassification
                                outputSettlement
                                validRequiredOutputs
                                lateOutputCount
                                ignoredLateOutputCount
                                sessionReuseReason
                                providerSessionId
                                activeSessionGenerationId
                                activeGenerationMatchesExecution
                                generationStatus
                                quotaLedgerId
                                createdAt
                                updatedAt
                            }}
                        }}
                    }}
                }}"#,
                stage_execution_id
            ))
            .data(auth::Principal::new(
                "operator",
                auth::PrincipalClass::Operator,
            )),
        )
        .await;

    assert!(
        response.errors.is_empty(),
        "graphql errors: {:?}",
        response.errors
    );
    let data = response.data.into_json().unwrap();
    let execution = &data["stage"]["executions"][0];
    assert_eq!(execution["sessionReuseDisposition"], "reused");
    assert_eq!(execution["sessionResetReason"], "operator_reset");
    assert_eq!(execution["ownerExecutionLineageId"], "lineage-owner-1");
    assert_eq!(execution["sessionLineageId"], "session-lineage-1");
    assert_eq!(execution["sessionGenerationId"], "session-generation-1");
    assert_eq!(
        execution["rehydratedFromCheckpointArtifactId"],
        "checkpoint-1"
    );
    assert_eq!(execution["invocationOwnerKey"], "owner-key");
    assert_eq!(
        execution["sessionReuseScope"],
        "same_agent_family_within_run"
    );
    assert_eq!(execution["sessionFamilyId"], "family-1");

    let runtime_facts = &execution["runtimeFacts"];
    assert_eq!(
        runtime_facts["agentExecutionId"],
        agent_execution_id.to_string()
    );
    assert_eq!(runtime_facts["failureKind"], "PROVIDER_QUOTA");
    assert_eq!(
        runtime_facts["failureKindRawDebug"],
        "future_provider_quota_variant"
    );
    assert_eq!(runtime_facts["failureKindVersion"], 1);
    assert_eq!(
        runtime_facts["failureMessageRedacted"],
        "limit resets 10pm (Asia/Nicosia)"
    );
    assert_eq!(runtime_facts["retryAfter"], now.to_rfc3339());
    assert_eq!(
        runtime_facts["operatorActionHint"],
        "wait_until_retry_after"
    );
    assert_eq!(runtime_facts["providerExitStatus"], 137);
    assert_eq!(runtime_facts["transportErrorCode"], "EPIPE");
    assert_eq!(
        runtime_facts["supervisionClassification"],
        "idle_hang_after_progress"
    );
    assert_eq!(
        runtime_facts["outputSettlement"],
        "MISSING_REQUIRED_OUTPUTS"
    );
    assert_eq!(runtime_facts["validRequiredOutputs"], false);
    assert_eq!(runtime_facts["lateOutputCount"], 2);
    assert_eq!(runtime_facts["ignoredLateOutputCount"], 1);
    assert_eq!(
        runtime_facts["sessionReuseReason"],
        "same_family_within_run"
    );
    assert_eq!(runtime_facts["providerSessionId"], "provider-session-1");
    assert_eq!(
        runtime_facts["activeSessionGenerationId"],
        "session-generation-1"
    );
    assert_eq!(runtime_facts["activeGenerationMatchesExecution"], true);
    assert_eq!(runtime_facts["generationStatus"], "active");
    assert_eq!(runtime_facts["quotaLedgerId"], ledger.id);
    assert!(runtime_facts["createdAt"].is_string());
    assert!(runtime_facts["updatedAt"].is_string());
    assert_eq!(data["stage"]["id"], stage_execution_id.to_string());
    assert_eq!(data["stage"]["executions"].as_array().unwrap().len(), 1);
    assert_eq!(
        data["stage"]["executions"][0]["runtimeFacts"]["createdAt"],
        now.to_rfc3339()
    );
}

#[tokio::test]
async fn proposal_053_agent_execution_projects_discovery_reconciliation_pending() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let (_run_id, stage_execution_id, agent_execution_id, _artifact_id) =
        seed_execution(&pool).await;
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

    let schema = make_schema(pool);
    let response = schema
        .execute(
            Request::new(format!(
                r#"{{
                    stage(id: "{}") {{
                        executions {{
                            runtimeFacts {{
                                outputSettlement
                                validRequiredOutputs
                            }}
                            discoveryDiagnostics
                        }}
                    }}
                }}"#,
                stage_execution_id
            ))
            .data(auth::Principal::new(
                "operator",
                auth::PrincipalClass::Operator,
            )),
        )
        .await;

    assert!(
        response.errors.is_empty(),
        "graphql errors: {:?}",
        response.errors
    );
    let data = response.data.into_json().unwrap();
    let execution = &data["stage"]["executions"][0];
    assert_eq!(
        execution["runtimeFacts"]["outputSettlement"],
        "VALID_OUTPUTS_FROM_COMPLETED_EXECUTION"
    );
    assert_eq!(execution["runtimeFacts"]["validRequiredOutputs"], false);
    assert_eq!(
        execution["discoveryDiagnostics"]["reconciliation_pending"],
        true
    );
    assert_eq!(
        execution["discoveryDiagnostics"]["stale_output_count"],
        serde_json::json!(1)
    );
    assert_eq!(
        execution["discoveryDiagnostics"]["missing_required_output_count"],
        serde_json::json!(1)
    );
    assert_eq!(
        execution["discoveryDiagnostics"]["payload"]["resume_warnings"],
        serde_json::json!(["reconciliation_pending"])
    );
    assert_eq!(
        execution["discoveryDiagnostics"]["runtime_facts_present"],
        true
    );
}

#[tokio::test]
async fn proposal_058_runtime_facts_redacts_only_raw_debug_for_observers() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let (_run_id, stage_execution_id, agent_execution_id, _artifact_id) =
        seed_execution(&pool).await;
    let now = Utc::now();
    let mut facts = AgentExecutionRuntimeFacts::defaults_for(agent_execution_id, now);
    facts.failure_kind = Some(AgentFailureKind::ProviderQuota);
    facts.failure_kind_raw_debug = Some("future_provider_quota_variant".into());
    facts.failure_message_redacted = Some("limit resets 10pm (Asia/Nicosia)".into());
    facts.output_settlement = AgentOutputSettlement::MissingRequiredOutputs;
    agent_execution_runtime_facts::upsert(&pool, &facts)
        .await
        .unwrap();

    let schema = make_schema(pool);
    let response = schema
        .execute(
            Request::new(format!(
                r#"{{
                    stage(id: "{}") {{
                        executions {{
                            runtimeFacts {{
                                failureKind
                                failureKindRawDebug
                                failureMessageRedacted
                                outputSettlement
                            }}
                        }}
                    }}
                }}"#,
                stage_execution_id
            ))
            .data(auth::Principal::new(
                "observer",
                auth::PrincipalClass::Observer,
            )),
        )
        .await;

    assert!(
        response.errors.is_empty(),
        "graphql errors: {:?}",
        response.errors
    );
    let data = response.data.into_json().unwrap();
    let runtime_facts = &data["stage"]["executions"][0]["runtimeFacts"];
    assert_eq!(runtime_facts["failureKind"], "PROVIDER_QUOTA");
    assert!(runtime_facts["failureKindRawDebug"].is_null());
    assert_eq!(
        runtime_facts["failureMessageRedacted"],
        "limit resets 10pm (Asia/Nicosia)"
    );
    assert_eq!(
        runtime_facts["outputSettlement"],
        "MISSING_REQUIRED_OUTPUTS"
    );

    let response = schema
        .execute(
            Request::new(format!(
                r#"{{
                    agentExecutions(stageExecutionId: "{}") {{
                        runtimeFacts {{
                            failureKind
                            failureKindRawDebug
                            outputSettlement
                        }}
                    }}
                }}"#,
                stage_execution_id
            ))
            .data(auth::Principal::new(
                "observer",
                auth::PrincipalClass::Observer,
            )),
        )
        .await;

    assert!(
        response.errors.is_empty(),
        "graphql errors: {:?}",
        response.errors
    );
    let data = response.data.into_json().unwrap();
    let runtime_facts = &data["agentExecutions"][0]["runtimeFacts"];
    assert_eq!(runtime_facts["failureKind"], "PROVIDER_QUOTA");
    assert!(runtime_facts["failureKindRawDebug"].is_null());
    assert_eq!(
        runtime_facts["outputSettlement"],
        "MISSING_REQUIRED_OUTPUTS"
    );
}

#[tokio::test]
async fn proposal_058_runtime_facts_marks_fresh_after_budget_as_fresh_process() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let (_run_id, stage_execution_id, agent_execution_id, _artifact_id) =
        seed_execution(&pool).await;
    let facts = AgentExecutionRuntimeFacts::defaults_for(agent_execution_id, Utc::now());
    agent_execution_runtime_facts::upsert(&pool, &facts)
        .await
        .unwrap();
    sqlx::query("UPDATE agent_executions SET session_reuse_disposition = ?1 WHERE id = ?2")
        .bind("fresh_after_budget")
        .bind(agent_execution_id.to_string())
        .execute(&pool)
        .await
        .unwrap();

    let schema = make_schema(pool);
    let response = schema
        .execute(
            Request::new(format!(
                r#"{{
                    stage(id: "{}") {{
                        executions {{
                            sessionReuseDisposition
                            runtimeFacts {{
                                freshProviderProcess
                            }}
                        }}
                    }}
                }}"#,
                stage_execution_id
            ))
            .data(auth::Principal::new(
                "observer",
                auth::PrincipalClass::Observer,
            )),
        )
        .await;

    assert!(
        response.errors.is_empty(),
        "graphql errors: {:?}",
        response.errors
    );
    let data = response.data.into_json().unwrap();
    let execution = &data["stage"]["executions"][0];
    assert_eq!(execution["sessionReuseDisposition"], "fresh_after_budget");
    assert_eq!(execution["runtimeFacts"]["freshProviderProcess"], true);
}

#[tokio::test]
async fn proposal_058_artifact_projection_populates_source_generation_fields() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let (run_id, stage_execution_id, agent_execution_id, _artifact_id) =
        seed_execution(&pool).await;
    let artifact_id = ArtifactId::new();

    artifacts::insert(
        &pool,
        &Artifact {
            id: artifact_id,
            run_id,
            stage_id: "state_1".into(),
            agent_id: "code_writer".into(),
            name: "prepush_review_report".into(),
            contract_id: "prepush_review_v1".into(),
            format: ArtifactFormat::Json,
            file_path: "/tmp/artifacts/prepush.json".into(),
            checksum_sha256: Some("sha256".into()),
            size_bytes: Some(128),
            provider: "claude".into(),
            model: Some("sonnet".into()),
            created_at: Utc::now(),
            is_pinned: false,
            report_kind: None,
            report_version: None,
            agent_execution_id: None,
        },
    )
    .await
    .unwrap();
    db::repos::projections::rebuild_all_for_run(&pool, run_id)
        .await
        .unwrap();

    artifact_contracts::upsert_generation_and_rebuild(
        &pool,
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id,
            contract_id: "prepush_review_v1".into(),
            canonical_path: "review/prepush.json".into(),
            raw_path: "review/prepush.json".into(),
            raw_status: "PASS".into(),
            generation_id: "generation-1".into(),
            source_agent_execution_id: Some(agent_execution_id.to_string()),
            source_stage_execution_id: Some(stage_execution_id.to_string()),
            source_session_generation_id: Some("session-generation-1".into()),
            source_work_item_id: Some("work-item-1".into()),
            supersedes_generation_id: Some("generation-0".into()),
            output_settlement: AgentOutputSettlement::ValidOutputsFromCompletedExecution,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();

    let schema = make_schema(pool);
    let response = schema
        .execute(
            Request::new(format!(
                r#"{{
                    artifacts(runId: "{}") {{
                        artifactGenerationId
                        sourceAgentExecutionId
                        sourceStageExecutionId
                        sourceSessionGenerationId
                        sourceWorkItemId
                        supersedesArtifactGenerationId
                        outputSettlement
                        sourceGenerationVerified
                    }}
                }}"#,
                run_id
            ))
            .data(auth::Principal::new(
                "operator",
                auth::PrincipalClass::Operator,
            )),
        )
        .await;

    assert!(
        response.errors.is_empty(),
        "graphql errors: {:?}",
        response.errors
    );
    let data = response.data.into_json().unwrap();
    let artifact = &data["artifacts"][0];
    assert_eq!(artifact["artifactGenerationId"], "generation-1");
    assert_eq!(
        artifact["sourceAgentExecutionId"],
        agent_execution_id.to_string()
    );
    assert_eq!(
        artifact["sourceStageExecutionId"],
        stage_execution_id.to_string()
    );
    assert_eq!(
        artifact["sourceSessionGenerationId"],
        "session-generation-1"
    );
    assert_eq!(artifact["sourceWorkItemId"], "work-item-1");
    assert_eq!(artifact["supersedesArtifactGenerationId"], "generation-0");
    assert_eq!(
        artifact["outputSettlement"],
        "VALID_OUTPUTS_FROM_COMPLETED_EXECUTION"
    );
    assert_eq!(artifact["sourceGenerationVerified"], false);
}
