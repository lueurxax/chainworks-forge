use std::sync::Arc;

use chrono::Utc;
use db::pool::create_pool;
use db::repos::{code_writer_completion_receipts, ideas, runs, stages};
use domain::agent::{AgentExecution, AgentStatus};
use domain::code_writer_completion::{
    CodeWriterCompletionOutputDecisionRecord, CodeWriterCompletionReceiptRecord,
    CodeWriterCompletionTextCaptureRecord, CodeWriterOutputSettlementRow,
};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{AgentExecutionId, IdeaId, RunId, StageExecutionId};
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageStatus};
use engine::command_handler::CommandHandler;
use engine::event_bus;
use engine::work_queue::WorkQueue;
use mcp_server::tools::{reports, runs as mcp_runs};

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

async fn seed_receipt(
    pool: &sqlx::SqlitePool,
) -> (RunId, StageExecutionId, AgentExecutionId, String) {
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();
    let agent_execution_id = AgentExecutionId::new();
    let receipt_id = "p088-mcp-receipt-readback".to_string();

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
        ingestion_boundary_failure: Some("extraction_input_truncated".into()),
        work_change_kind: Some("current_attempt_diff".into()),
        pre_prompt_worktree_fingerprint_path: Some(".chainworks/p088/pre.json".into()),
        post_prompt_worktree_fingerprint_path: Some(".chainworks/p088/post.json".into()),
        pre_prompt_worktree_fingerprint_sha256: Some("sha256:pre".into()),
        post_prompt_worktree_fingerprint_sha256: Some("sha256:post".into()),
        current_attempt_changed_path_count: 2,
        preexisting_dirty_path_count: 0,
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

fn make_command_handler(pool: sqlx::SqlitePool) -> CommandHandler {
    let events = event_bus::new_bus(64);
    CommandHandler::new(pool.clone(), events, WorkQueue::new(pool))
}

#[tokio::test]
async fn proposal_088_mcp_report_exposes_code_writer_completion_receipts() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    db::writer::register_shared_writer(&pool, Arc::new(db::writer::DbWriter::new(pool.clone())))
        .await
        .unwrap();
    let (run_id, _stage_execution_id, agent_execution_id, receipt_id) = seed_receipt(&pool).await;
    let handler = make_command_handler(pool.clone());
    let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);

    let result = reports::execute(
        "reports.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &handler,
        &principal,
    )
    .await
    .unwrap();
    let reports = result.as_array().expect("reports array");
    let mcp_truth = reports
        .iter()
        .find(|report| report["report_kind"] == serde_json::json!("mcp_execution_truth"))
        .expect("mcp execution truth report");

    let receipt = &mcp_truth["code_writer_completion_receipts"][0];
    assert_eq!(receipt["receipt"]["id"], receipt_id);
    assert_eq!(
        receipt["receipt"]["agent_execution_id"],
        agent_execution_id.to_string()
    );
    assert_eq!(
        receipt["receipt"]["ingestion_boundary_failure"],
        "extraction_input_truncated"
    );
    assert_eq!(
        receipt["receipt"]["completion_turn_result"],
        "unknown_future_completion_result"
    );
    assert_eq!(receipt["receipt"]["provider_runtime_family"], "junie_acp");
    assert_eq!(
        receipt["receipt"]["completion_boundary_subtype"],
        "junie_repair_outputs_partially_materialized"
    );
    assert_eq!(receipt["receipt"]["runtime_preflight_phase"], "passed");
    let final_payload: serde_json::Value = serde_json::from_str(
        receipt["receipt"]["final_completion_payload_capture_json"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        final_payload["redacted_text_artifact_path"],
        ".chainworks/p090/final-payload.redacted.txt"
    );
    assert_eq!(
        final_payload["failure_envelope_authority"],
        "provider_claim_rejected"
    );
    let settlement_rows = receipt["settlement_rows"].as_array().unwrap();
    assert_eq!(settlement_rows.len(), 2);
    let accepted = settlement_rows
        .iter()
        .find(|row| row["output_name"] == "implementation_progress")
        .unwrap();
    assert_eq!(accepted["decision"], "accepted");
    assert_eq!(accepted["source_generation_owner"], "agent");
    assert_eq!(
        accepted["active_pointer_generation_id"],
        "session-generation-p090"
    );
    assert_eq!(
        receipt["text_captures"][0]["completion_text_capture_source"],
        "streamed_update_tail"
    );
    assert_eq!(
        receipt["text_captures"][0]["extraction_input_truncated"],
        true
    );
    assert_eq!(receipt["output_decisions"][0]["validation_status"], "fresh");
    assert_eq!(
        receipt["output_decisions"][0]["rejection_reason"],
        "unknown_future_rejection"
    );

    let execution_receipt = &mcp_truth["agent_executions"][0]["code_writer_completion_receipt"];
    assert_eq!(execution_receipt["status"]["value"], "failed");
    assert_eq!(execution_receipt["completion_turn_result"]["known"], false);

    let implementation_completion = &mcp_truth["implementationCompletion"];
    assert_eq!(implementation_completion["status"]["value"], "failed");
    assert_eq!(implementation_completion["status"]["known"], true);
    assert_eq!(
        implementation_completion["ingestion_boundary_failure"]["value"],
        "extraction_input_truncated"
    );
    assert_eq!(
        implementation_completion["completion_turn_result"]["value"],
        "unknown"
    );
    assert_eq!(
        implementation_completion["completion_turn_result"]["raw"],
        "unknown_future_completion_result"
    );
    assert_eq!(
        implementation_completion["completion_turn_result"]["known"],
        false
    );
    assert_eq!(
        implementation_completion["completion_boundary_subtype"]["value"],
        "junie_repair_outputs_partially_materialized"
    );
    assert_eq!(
        implementation_completion["completion_boundary_subtype"]["known"],
        true
    );
    assert_eq!(
        implementation_completion["provider_runtime_family"],
        "junie_acp"
    );
    assert_eq!(
        implementation_completion["runtime_preflight_phase"],
        "passed"
    );
    assert_eq!(
        implementation_completion["failure_envelope_authority"],
        "provider_claim_rejected"
    );
    assert_eq!(
        implementation_completion["repair_materialization_mode"],
        "staged_per_output"
    );
    assert_eq!(
        implementation_completion["next_operator_action"]["value"],
        "unknown"
    );
    assert_eq!(
        implementation_completion["completion_text_captures"][0]["completion_text_capture_source"],
        "streamed_update_tail"
    );
    assert_eq!(
        mcp_truth["agent_executions"][0]["code_writer_completion_receipt"]["status"]["value"],
        "failed"
    );
}

#[tokio::test]
async fn proposal_088_mcp_runs_get_and_list_expose_implementation_completion() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    db::writer::register_shared_writer(&pool, Arc::new(db::writer::DbWriter::new(pool.clone())))
        .await
        .unwrap();
    let (run_id, _stage_execution_id, _agent_execution_id, _receipt_id) = seed_receipt(&pool).await;
    let handler = make_command_handler(pool.clone());
    let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);

    let run_get = mcp_runs::execute(
        "runs.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &handler,
        &principal,
    )
    .await
    .unwrap();
    assert_eq!(
        run_get["implementationCompletion"]["status"]["value"],
        "failed"
    );
    assert_eq!(
        run_get["implementationCompletion"]["completion_turn_result"]["known"],
        false
    );
    assert_eq!(
        run_get["implementationCompletion"]["completion_boundary_subtype"]["value"],
        "junie_repair_outputs_partially_materialized"
    );

    let run_list = mcp_runs::execute(
        "runs.list",
        serde_json::json!({}),
        &pool,
        &handler,
        &principal,
    )
    .await
    .unwrap();
    let listed = run_list
        .as_array()
        .expect("runs.list array")
        .iter()
        .find(|item| item["id"] == serde_json::json!(run_id.to_string()))
        .expect("seeded run listed");
    assert_eq!(
        listed["implementationCompletion"]["ingestion_boundary_failure"]["value"],
        "extraction_input_truncated"
    );
    assert_eq!(
        listed["implementationCompletion"]["completion_boundary_subtype"]["value"],
        "junie_repair_outputs_partially_materialized"
    );
}
