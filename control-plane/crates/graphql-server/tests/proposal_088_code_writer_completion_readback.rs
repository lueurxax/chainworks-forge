use std::sync::Arc;

use async_graphql::Request;
use chrono::Utc;
use db::pool::create_pool;
use db::repos::{code_writer_completion_receipts, ideas, runs, stages, work_items};
use db::work_item::{WorkItem, WorkItemKind, WorkItemStatus};
use domain::agent::{AgentExecution, AgentStatus};
use domain::code_writer_completion::{
    CodeWriterCompletionOutputDecisionRecord, CodeWriterCompletionReceiptRecord,
    CodeWriterCompletionTextCaptureRecord, CodeWriterOutputSettlementRow,
};
use domain::commands::{CallerContext, CallerSurface, Command, RetryStageCmd};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{AgentExecutionId, IdeaId, RunId, StageExecutionId};
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageStatus};
use domain::PrincipalClass;
use engine::command_handler::{CommandHandler, CommandResult};
use engine::event_bus;
use engine::lifecycle_reporter::LifecycleReporter;
use engine::work_queue::WorkQueue;
use graphql_server::schema::build_schema;

fn make_run(run_id: RunId, idea_id: IdeaId) -> Run {
    Run {
        id: run_id,
        idea_id,
        status: RunStatus::Ready,
        workflow_id: "wf-p088".into(),
        workflow_title: "P088".into(),
        workspace_root: "/tmp/ws".into(),
        artifact_root: "/tmp/artifacts".into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        cancellation_settlement_log: None,
        current_state: Some("state_code".into()),
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

fn operator_caller() -> CallerContext {
    CallerContext {
        surface: CallerSurface::Graphql,
        principal_id: "operator-p088".into(),
        principal_class: PrincipalClass::Operator,
        caller_tool: "stages.retry".into(),
        request_id: None,
        caller_class: None,
        token_id: None,
        mcp_idempotency_key: None,
        mcp_idempotency_request_hash: None,
        boundary_row_id: None,
    }
}

async fn seed_receipt(
    pool: &sqlx::SqlitePool,
) -> (RunId, StageExecutionId, AgentExecutionId, String) {
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();
    let agent_execution_id = AgentExecutionId::new();
    let receipt_id = "p088-receipt-readback".to_string();

    ideas::insert(
        pool,
        &Idea {
            id: idea_id,
            title: "P088 idea".into(),
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
            stage_id: "state_code".into(),
            label: "Code".into(),
            status: StageStatus::Failed,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            owner_agent: Some("code_writer".into()),
            provider: Some("junie".into()),
            model: Some("junie-default".into()),
            stage_type: None,
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
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
            provider: "junie".into(),
            model: Some("junie-default".into()),
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            status: AgentStatus::Failed,
            owner_execution_lineage_id: None,
            session_lineage_id: None,
            session_generation_id: Some("session-generation-p088".into()),
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

    let receipt = CodeWriterCompletionReceiptRecord {
        id: receipt_id.clone(),
        run_id,
        stage_execution_id,
        agent_execution_id,
        session_generation_id: Some("session-generation-p088".into()),
        original_runtime_receipt_id: Some("runtime-original".into()),
        completion_repair_runtime_receipt_id: Some("runtime-repair".into()),
        provider: "junie".into(),
        model: Some("junie-default".into()),
        completion_mode: Some("code_writer_completion_repair_turn".into()),
        published_at: None,
        activation_source: "p037_idle_terminalization".into(),
        ingestion_boundary_failure: Some(
            "terminal_response_capture_truncated_before_output".into(),
        ),
        work_change_kind: Some("current_attempt_diff".into()),
        pre_prompt_worktree_fingerprint_path: Some(".chainworks/p088/pre.json".into()),
        post_prompt_worktree_fingerprint_path: Some(".chainworks/p088/post.json".into()),
        pre_prompt_worktree_fingerprint_sha256: Some("sha256:pre".into()),
        post_prompt_worktree_fingerprint_sha256: Some("sha256:post".into()),
        current_attempt_changed_path_count: 2,
        preexisting_dirty_path_count: 1,
        completion_status: "missing_required_outputs".into(),
        failure_class: Some("terminal_response_completed_missing_required_outputs".into()),
        provider_runtime_family: Some("junie_acp".into()),
        completion_boundary_subtype: Some("junie_repair_outputs_partially_materialized".into()),
        final_payload_status: Some("present".into()),
        progress_before_handoff: Some("worktree_diff_detected".into()),
        runtime_preflight_phase: Some("passed".into()),
        runtime_tool_path_preflight_json: Some(
            serde_json::json!({"status": "passed", "provider_launched": true}).to_string(),
        ),
        final_completion_payload_capture_json: Some(
            serde_json::json!({
                "schema": "p090_final_completion_payload_capture_v1",
                "status": "captured",
                "capture_record_ref": "original:0",
                "redacted_text_artifact_path": ".chainworks/p090/final-payload.redacted.txt",
                "failure_envelope_authority": "provider_claim_rejected"
            })
            .to_string(),
        ),
        engine_failure_envelope_json: Some(
            serde_json::json!({
                "schema_version": "code_writer_engine_failure.v1",
                "source": "engine_synthesized",
                "completion_boundary_subtype": "provider_authored_engine_failure_spoof_rejected"
            })
            .to_string(),
        ),
        repair_failure_envelope_json: Some(
            serde_json::json!({
                "schema_version": "code_writer_repair_failure.v1",
                "source": "engine_synthesized",
                "completion_boundary_subtype": "junie_repair_outputs_partially_materialized"
            })
            .to_string(),
        ),
        repair_materialization_summary_json: Some(
            serde_json::json!({
                "schema": "p090_repair_materialization_summary_v1",
                "fresh_count": 1,
                "malformed_count": 1
            })
            .to_string(),
        ),
        repair_materialization_mode: Some("staged_per_output".into()),
        strict_final_payload_enabled: true,
        staged_repair_settlement_enabled: true,
        terminal_response_status: Some("completed".into()),
        completion_turn_attempted: true,
        completion_turn_result: Some("unknown_future_completion_result".into()),
        completion_text_capture_count: 1,
        completion_text_absence_count: 0,
        completion_repair_text_status: Some("captured".into()),
        completion_repair_raw_text_artifact_path: Some(".chainworks/p088/raw.txt".into()),
        completion_repair_redacted_text_artifact_path: Some(".chainworks/p088/redacted.txt".into()),
        completion_repair_text_absence_reason: None,
        fresh_required_output_count: 1,
        stale_required_output_count: 1,
        missing_required_output_count: 1,
        control_plane_output_count: 1,
        completion_repair_turn_count: 1,
        generic_repair_turn_count: 0,
        missing_outputs: vec!["implementation_progress".into()],
        stale_outputs: vec!["implementation_self_assessment".into()],
        transcript_status: Some("unavailable".into()),
        transcript_absence_reason: Some("session_reuse_without_terminal_capture".into()),
        receipt_artifact_path: Some(".chainworks/p088/receipt.json".into()),
        failed_stage_evidence_path: Some(".chainworks/p088/failed-stage.json".into()),
        created_at: Utc::now(),
    };
    let text_capture = CodeWriterCompletionTextCaptureRecord {
        receipt_id: receipt_id.clone(),
        prompt_kind: "completion_repair".into(),
        turn_index: 1,
        terminal_response_status: Some("completed".into()),
        completion_text_status: "captured".into(),
        completion_text_capture_source: Some("streamed_update_tail".into()),
        completion_text_raw_byte_limit: Some(65536),
        completion_text_captured_byte_count: Some(128),
        completion_text_truncated: false,
        extraction_input_truncated: true,
        extraction_input_sha256: Some("sha256:input".into()),
        raw_text_artifact_path: Some(".chainworks/p090/final-payload.raw.txt".into()),
        redacted_text_artifact_path: Some(".chainworks/p090/final-payload.redacted.txt".into()),
        text_absence_reason: None,
        created_at: Utc::now(),
    };
    let output_decision = CodeWriterCompletionOutputDecisionRecord {
        receipt_id: receipt_id.clone(),
        output_name: "implementation_self_assessment".into(),
        contract_id: Some("implementation_self_assessment_v2".into()),
        canonical_path: "implementation/self-assessment.json".into(),
        pre_prompt_sha256: Some("sha256:stale".into()),
        post_prompt_sha256: Some("sha256:fresh".into()),
        content_sha256: Some("sha256:fresh".into()),
        settlement_source: Some("provider_envelope".into()),
        validation_status: Some("fresh".into()),
        rejection_reason: Some("unknown_future_rejection".into()),
    };
    let rows = vec![
        CodeWriterOutputSettlementRow {
            id: format!("{receipt_id}:implementation_progress"),
            receipt_id: receipt_id.clone(),
            run_id,
            stage_id: "state_code".into(),
            stage_execution_id,
            agent_execution_id,
            session_generation_id: Some("session-generation-p088".into()),
            repair_attempt: 1,
            output_name: "implementation_progress".into(),
            contract_id: "implementation_progress".into(),
            source_kind: "chainworks_output".into(),
            source_generation_owner: "agent".into(),
            candidate_digest: Some("sha256:progress".into()),
            staging_path: Some(".chainworks/p090/staged/progress.md".into()),
            canonical_path: "implementation/progress.md".into(),
            canonical_before_sha256: None,
            canonical_after_sha256: Some("sha256:progress".into()),
            decision: "accepted".into(),
            rejection_reason: None,
            materialization_state: "committed".into(),
            active_pointer_generation_id: Some("session-generation-p090".into()),
            created_at: Utc::now(),
            committed_at: Some(Utc::now()),
        },
        CodeWriterOutputSettlementRow {
            id: format!("{receipt_id}:implementation_self_assessment"),
            receipt_id: receipt_id.clone(),
            run_id,
            stage_id: "state_code".into(),
            stage_execution_id,
            agent_execution_id,
            session_generation_id: Some("session-generation-p088".into()),
            repair_attempt: 1,
            output_name: "implementation_self_assessment".into(),
            contract_id: "implementation_self_assessment_v2".into(),
            source_kind: "chainworks_output".into(),
            source_generation_owner: "agent".into(),
            candidate_digest: Some("sha256:malformed".into()),
            staging_path: Some(".chainworks/p090/staged/self-assessment.json".into()),
            canonical_path: "implementation/self-assessment.json".into(),
            canonical_before_sha256: Some("sha256:old".into()),
            canonical_after_sha256: None,
            decision: "rejected".into(),
            rejection_reason: Some("malformed_json".into()),
            materialization_state: "not_materialized".into(),
            active_pointer_generation_id: None,
            created_at: Utc::now(),
            committed_at: None,
        },
    ];
    code_writer_completion_receipts::upsert_with_runtime_receipts_and_settlement_rows(
        pool,
        &receipt,
        &[text_capture],
        &[output_decision],
        &rows,
        None,
        None,
    )
    .await
    .unwrap();

    (run_id, stage_execution_id, agent_execution_id, receipt_id)
}

async fn seed_p088_source_invoke_work_item(
    pool: &sqlx::SqlitePool,
    run_id: RunId,
    stage_execution_id: StageExecutionId,
    agent_execution_id: AgentExecutionId,
) {
    let now = Utc::now();
    let source_work_item_id = format!("p088-source-invoke:{stage_execution_id}");
    work_items::enqueue(
        pool,
        &WorkItem {
            id: source_work_item_id.clone(),
            kind: WorkItemKind::InvokeAgent,
            payload_json: serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "state_code",
                "stage_execution_id": stage_execution_id.to_string(),
                "agent_id": "code_writer",
                "provider": "codex",
                "model": "gpt-5",
                "prompt": "write code and required outputs",
                "task_name": "code",
                "task_inputs": [],
                "task_outputs": [],
                "declared_outputs": [],
                "requested_mcp_server_ids": [],
                "session_reuse_scope": "same_agent_family_within_run",
                "session_family_id": "code_writer",
                "worktree_write_enabled": false,
                "p058_claimed": {
                    "agent_execution_id": agent_execution_id.to_string(),
                    "artifact_claim_key": {
                        "run_id": run_id.to_string(),
                        "stage_execution_id": stage_execution_id.to_string(),
                        "agent_execution_id": agent_execution_id.to_string(),
                        "source_work_item_id": source_work_item_id,
                    }
                }
            })
            .to_string(),
            status: WorkItemStatus::Failed,
            run_id: Some(run_id),
            stage_id: Some("state_code".into()),
            created_at: now,
            scheduled_at: now,
            attempt_count: 1,
            last_error: Some("P088 missing required outputs".into()),
        },
    )
    .await
    .unwrap();
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

#[tokio::test]
async fn proposal_088_targeted_retry_recovery_carries_preserved_evidence_to_activation_and_readback(
) {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    db::writer::register_shared_writer(&pool, Arc::new(db::writer::DbWriter::new(pool.clone())))
        .await
        .unwrap();
    let (run_id, stage_execution_id, agent_execution_id, receipt_id) = seed_receipt(&pool).await;
    seed_p088_source_invoke_work_item(&pool, run_id, stage_execution_id, agent_execution_id).await;

    let handler = CommandHandler::new(
        pool.clone(),
        event_bus::new_bus(16),
        WorkQueue::new(pool.clone()),
    );
    // P083: request_id is now required for all RetryStage paths (including narrow retry).
    let narrow_retry_request_id = uuid::Uuid::new_v4().to_string();
    let result = handler
        .handle(
            Command::RetryStage(RetryStageCmd {
                run_id,
                stage_id: "state_code".into(),
                consume_quota_budget_now: false,
                agent_execution_id: Some(agent_execution_id),
                legacy_discovery_override_policy: None,
                legacy_discovery_override_reason: None,
                operator_instruction: None,
                request_id: Some(narrow_retry_request_id),
            }),
            operator_caller(),
        )
        .await
        .unwrap();
    assert!(matches!(
        result.result,
        CommandResult::StageRetryScheduled { .. }
    ));

    let retry_item = work_items::list_by_run(&pool, run_id)
        .await
        .unwrap()
        .into_iter()
        .find(|item| {
            item.kind == WorkItemKind::InvokeAgent && item.status == WorkItemStatus::Pending
        })
        .expect("targeted retry should enqueue one pending InvokeAgent item");
    let retry_payload: serde_json::Value = serde_json::from_str(&retry_item.payload_json).unwrap();
    let source_agent_execution_id = agent_execution_id.to_string();
    assert_eq!(
        retry_payload
            .pointer("/p088/activation_source")
            .and_then(serde_json::Value::as_str),
        Some("operator_retry_completion_recovery")
    );
    assert_eq!(
        retry_payload
            .pointer("/p088/operator_retry_completion_recovery")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        retry_payload
            .pointer("/p088/preserved_historical_evidence_packet_path")
            .and_then(serde_json::Value::as_str),
        Some(".chainworks/p088/failed-stage.json")
    );
    assert_eq!(
        retry_payload
            .pointer("/p088/source_agent_execution_id")
            .and_then(serde_json::Value::as_str),
        Some(source_agent_execution_id.as_str())
    );
    assert_eq!(
        retry_payload
            .get("retry_reason")
            .and_then(serde_json::Value::as_str),
        Some("operator_retry_completion_recovery")
    );

    let claimed = engine::executor::claim_next_invoke_agent_with_start(&pool)
        .await
        .unwrap()
        .expect("retry InvokeAgent should be claimable");
    assert_eq!(claimed.work_item_id, retry_item.id);
    let claimed_item = work_items::find_by_id(&pool, &claimed.work_item_id)
        .await
        .unwrap()
        .expect("claimed retry work item should remain readable");
    let claimed_payload: serde_json::Value =
        serde_json::from_str(&claimed_item.payload_json).unwrap();
    let claimed_agent_execution_id = claimed.agent_execution_id.to_string();
    assert_eq!(
        claimed_payload
            .pointer("/p088/activation_source")
            .and_then(serde_json::Value::as_str),
        Some("operator_retry_completion_recovery")
    );
    assert_eq!(
        claimed_payload
            .pointer("/p088/preserved_historical_evidence_packet_path")
            .and_then(serde_json::Value::as_str),
        Some(".chainworks/p088/failed-stage.json")
    );
    assert_eq!(
        claimed_payload
            .pointer("/p058_claimed/agent_execution_id")
            .and_then(serde_json::Value::as_str),
        Some(claimed_agent_execution_id.as_str())
    );

    let schema = make_schema(pool);
    let response = schema
        .execute(
            Request::new(format!(
                r#"{{
                    run(id: "{run_id}") {{
                        implementationCompletion {{
                            activationSource
                            failedStageEvidencePath
                            receiptArtifactPath
                            nextOperatorAction {{ value raw known }}
                        }}
                        codeWriterCompletionReceipts {{
                            id
                            agentExecutionId
                            activationSource
                            failedStageEvidencePath
                            receiptArtifactPath
                        }}
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
        "graphql errors: {:?}",
        response.errors
    );
    let data = response.data.into_json().unwrap();
    let implementation_completion = &data["run"]["implementationCompletion"];
    assert_eq!(
        implementation_completion["activationSource"],
        "p037_idle_terminalization"
    );
    assert_eq!(
        implementation_completion["failedStageEvidencePath"],
        ".chainworks/p088/failed-stage.json"
    );
    assert_eq!(
        implementation_completion["receiptArtifactPath"],
        ".chainworks/p088/receipt.json"
    );
    assert_eq!(
        implementation_completion["nextOperatorAction"]["value"],
        "unknown"
    );

    let receipt = &data["run"]["codeWriterCompletionReceipts"][0];
    assert_eq!(receipt["id"], receipt_id);
    assert_eq!(receipt["agentExecutionId"], agent_execution_id.to_string());
    assert_eq!(receipt["activationSource"], "p037_idle_terminalization");
    assert_eq!(
        receipt["failedStageEvidencePath"],
        ".chainworks/p088/failed-stage.json"
    );
}

#[tokio::test]
async fn proposal_088_graphql_exposes_code_writer_completion_receipts_by_run_and_execution() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    db::writer::register_shared_writer(&pool, Arc::new(db::writer::DbWriter::new(pool.clone())))
        .await
        .unwrap();
    let (run_id, stage_execution_id, agent_execution_id, receipt_id) = seed_receipt(&pool).await;
    let schema = make_schema(pool);

    let response = schema
        .execute(
            Request::new(format!(
                r#"{{
                    run(id: "{run_id}") {{
                        implementationCompletion {{
                            status {{ value raw known }}
                            ingestionBoundaryFailure {{ value raw known }}
                            completionTurnResult {{ value raw known }}
                            providerRuntimeFamily
                            completionBoundarySubtype {{ value raw known }}
                            finalPayloadStatus
                            runtimePreflightPhase
                            runtimeToolPathPreflightJson
                            finalCompletionPayloadCaptureJson
                            engineFailureEnvelopeJson
                            repairFailureEnvelopeJson
                            failureEnvelopeAuthority
                            repairMaterializationMode
                            strictFinalPayloadEnabled
                            stagedRepairSettlementEnabled
                            nextOperatorAction {{ value raw known }}
                            completionTurnAttempted
                            promptTemplateId
                            freshRequiredOutputCount
                            staleRequiredOutputCount
                            missingRequiredOutputCount
                            controlPlaneOutputCount
                            completionTextCaptures {{
                                completionTextCaptureSource
                                extractionInputTruncated
                            }}
                        }}
                        codeWriterCompletionReceipts {{
                            id
                            runId
                            stageExecutionId
                            agentExecutionId
                            provider
                            model
                            activationSource
                            ingestionBoundaryFailure
                            workChangeKind
                            completionStatus
                            failureClass
                            providerRuntimeFamily
                            completionBoundarySubtype
                            finalPayloadStatus
                            runtimePreflightPhase
                            runtimeToolPathPreflightJson
                            engineFailureEnvelopeJson
                            repairFailureEnvelopeJson
                            repairMaterializationMode
                            strictFinalPayloadEnabled
                            stagedRepairSettlementEnabled
                            terminalResponseStatus
                            completionTurnAttempted
                            completionTurnResult
                            freshRequiredOutputCount
                            staleRequiredOutputCount
                            missingRequiredOutputCount
                            controlPlaneOutputCount
                            transcriptStatus
                            transcriptAbsenceReason
                            textCaptures {{
                                promptKind
                                completionTextStatus
                                completionTextCaptureSource
                                extractionInputTruncated
                                extractionInputSha256
                            }}
                            outputDecisions {{
                                outputName
                                contractId
                                canonicalPath
                                settlementSource
                                validationStatus
                                rejectionReason
                            }}
                            settlementRows {{
                                outputName
                                decision
                                sourceGenerationOwner
                                materializationState
                                activePointerGenerationId
                                rejectionReason
                            }}
                        }}
                    }}
                    stage(id: "{stage_execution_id}") {{
                        executions {{
                            id
                            codeWriterCompletionReceipt {{
                                id
                                agentExecutionId
                                completionStatus
                                completionTurnResult
                            }}
                        }}
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
        "graphql errors: {:?}",
        response.errors
    );
    let data = response.data.into_json().unwrap();
    let implementation_completion = &data["run"]["implementationCompletion"];
    assert_eq!(implementation_completion["status"]["value"], "failed");
    assert_eq!(implementation_completion["status"]["known"], true);
    assert_eq!(
        implementation_completion["ingestionBoundaryFailure"]["value"],
        "terminal_response_capture_truncated_before_output"
    );
    assert_eq!(
        implementation_completion["completionTurnResult"]["value"],
        "unknown"
    );
    assert_eq!(
        implementation_completion["completionTurnResult"]["raw"],
        "unknown_future_completion_result"
    );
    assert_eq!(
        implementation_completion["completionTurnResult"]["known"],
        false
    );
    assert_eq!(
        implementation_completion["providerRuntimeFamily"],
        "junie_acp"
    );
    assert_eq!(
        implementation_completion["completionBoundarySubtype"]["value"],
        "junie_repair_outputs_partially_materialized"
    );
    assert_eq!(
        implementation_completion["completionBoundarySubtype"]["raw"],
        "junie_repair_outputs_partially_materialized"
    );
    assert_eq!(
        implementation_completion["completionBoundarySubtype"]["known"],
        true
    );
    assert_eq!(implementation_completion["finalPayloadStatus"], "present");
    assert_eq!(implementation_completion["runtimePreflightPhase"], "passed");
    assert_eq!(
        implementation_completion["failureEnvelopeAuthority"],
        "provider_claim_rejected"
    );
    let engine_envelope: serde_json::Value = serde_json::from_str(
        implementation_completion["engineFailureEnvelopeJson"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        engine_envelope["schema_version"],
        "code_writer_engine_failure.v1"
    );
    let repair_envelope: serde_json::Value = serde_json::from_str(
        implementation_completion["repairFailureEnvelopeJson"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        repair_envelope["schema_version"],
        "code_writer_repair_failure.v1"
    );
    let final_payload: serde_json::Value = serde_json::from_str(
        implementation_completion["finalCompletionPayloadCaptureJson"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        final_payload["redacted_text_artifact_path"],
        ".chainworks/p090/final-payload.redacted.txt"
    );
    assert_eq!(
        implementation_completion["repairMaterializationMode"],
        "staged_per_output"
    );
    assert_eq!(implementation_completion["strictFinalPayloadEnabled"], true);
    assert_eq!(
        implementation_completion["stagedRepairSettlementEnabled"],
        true
    );
    assert_eq!(
        implementation_completion["nextOperatorAction"]["value"],
        "unknown"
    );
    assert_eq!(
        implementation_completion["promptTemplateId"],
        "code_writer_completion_repair_v1"
    );
    assert_eq!(
        implementation_completion["completionTextCaptures"][0]["completionTextCaptureSource"],
        "streamed_update_tail"
    );
    let receipt = &data["run"]["codeWriterCompletionReceipts"][0];
    assert_eq!(receipt["id"], receipt_id);
    assert_eq!(receipt["agentExecutionId"], agent_execution_id.to_string());
    assert_eq!(receipt["activationSource"], "p037_idle_terminalization");
    assert_eq!(
        receipt["ingestionBoundaryFailure"],
        "terminal_response_capture_truncated_before_output"
    );
    assert_eq!(receipt["workChangeKind"], "current_attempt_diff");
    assert_eq!(
        receipt["failureClass"],
        "terminal_response_completed_missing_required_outputs"
    );
    assert_eq!(receipt["providerRuntimeFamily"], "junie_acp");
    assert_eq!(
        receipt["completionBoundarySubtype"],
        "junie_repair_outputs_partially_materialized"
    );
    assert_eq!(receipt["runtimePreflightPhase"], "passed");
    assert!(receipt["engineFailureEnvelopeJson"]
        .as_str()
        .unwrap()
        .contains("code_writer_engine_failure.v1"));
    assert!(receipt["repairFailureEnvelopeJson"]
        .as_str()
        .unwrap()
        .contains("code_writer_repair_failure.v1"));
    let settlement_rows = receipt["settlementRows"].as_array().unwrap();
    assert_eq!(settlement_rows.len(), 2);
    let accepted = settlement_rows
        .iter()
        .find(|row| row["outputName"] == "implementation_progress")
        .unwrap();
    assert_eq!(accepted["decision"], "accepted");
    assert_eq!(accepted["sourceGenerationOwner"], "agent");
    assert_eq!(accepted["materializationState"], "committed");
    assert_eq!(
        accepted["activePointerGenerationId"],
        "session-generation-p090"
    );
    let rejected = settlement_rows
        .iter()
        .find(|row| row["outputName"] == "implementation_self_assessment")
        .unwrap();
    assert_eq!(rejected["decision"], "rejected");
    assert_eq!(rejected["rejectionReason"], "malformed_json");
    assert_eq!(
        rejected["activePointerGenerationId"],
        serde_json::Value::Null
    );
    assert_eq!(
        receipt["completionTurnResult"],
        "unknown_future_completion_result"
    );
    assert_eq!(receipt["freshRequiredOutputCount"], 1);
    assert_eq!(receipt["staleRequiredOutputCount"], 1);
    assert_eq!(receipt["missingRequiredOutputCount"], 1);
    assert_eq!(receipt["controlPlaneOutputCount"], 1);
    assert_eq!(
        receipt["transcriptAbsenceReason"],
        "session_reuse_without_terminal_capture"
    );
    assert_eq!(
        receipt["textCaptures"][0]["completionTextCaptureSource"],
        "streamed_update_tail"
    );
    assert_eq!(receipt["textCaptures"][0]["extractionInputTruncated"], true);
    assert_eq!(receipt["outputDecisions"][0]["validationStatus"], "fresh");
    assert_eq!(
        receipt["outputDecisions"][0]["rejectionReason"],
        "unknown_future_rejection"
    );

    let execution_receipt = &data["stage"]["executions"][0]["codeWriterCompletionReceipt"];
    assert_eq!(execution_receipt["id"], receipt_id);
    assert_eq!(
        execution_receipt["completionTurnResult"],
        "unknown_future_completion_result"
    );
}
