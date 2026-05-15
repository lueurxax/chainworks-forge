use chrono::{Duration, Utc};
use db::pool::create_pool;
use db::repos::{
    agent_execution_runtime_receipts, agent_executions, code_writer_completion_receipts, ideas,
    runs, stages,
};
use domain::agent::{
    AgentExecution, AgentExecutionRuntimePromptReceiptRecord, AgentExecutionRuntimeReceiptRecord,
    AgentStatus,
};
use domain::code_writer_completion::{
    CodeWriterCompletionOutputDecisionRecord, CodeWriterCompletionReceiptRecord,
    CodeWriterCompletionTextCaptureRecord,
};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{AgentExecutionId, IdeaId, RunId, StageExecutionId};
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageStatus};
use sqlx::Row;

async fn setup_db() -> sqlx::SqlitePool {
    let pool = create_pool("sqlite::memory:")
        .await
        .expect("in-memory pool failed");
    let writer = std::sync::Arc::new(db::writer::DbWriter::new(pool.clone()));
    db::writer::register_shared_writer(&pool, writer)
        .await
        .expect("test shared DbWriter registration failed");
    pool
}

async fn seed_execution(pool: &sqlx::SqlitePool) -> (RunId, StageExecutionId, AgentExecutionId) {
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_id = StageExecutionId::new();
    let exec_id = AgentExecutionId::new();

    ideas::insert(
        pool,
        &Idea {
            id: idea_id,
            title: "P088 test".into(),
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
        pool,
        &Run {
            id: run_id,
            idea_id,
            status: RunStatus::Running,
            workflow_id: "wf-p088".into(),
            workflow_title: "P088".into(),
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
        },
    )
    .await
    .unwrap();

    stages::insert(
        pool,
        &StageExecution {
            id: stage_id,
            run_id,
            stage_id: "state_implementation".into(),
            label: "Implementation".into(),
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
            id: exec_id,
            stage_execution_id: Some(stage_id),
            agent_id: "code_writer".into(),
            provider: "claude".into(),
            model: Some("sonnet".into()),
            started_at: Utc::now(),
            completed_at: None,
            status: AgentStatus::Running,
            owner_execution_lineage_id: None,
            session_lineage_id: Some("lineage-1".into()),
            session_generation_id: Some("generation-1".into()),
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

    (run_id, stage_id, exec_id)
}

async fn seed_additional_execution(
    pool: &sqlx::SqlitePool,
    stage_id: StageExecutionId,
    agent_id: &str,
) -> AgentExecutionId {
    let exec_id = AgentExecutionId::new();
    agent_executions::insert(
        pool,
        &AgentExecution {
            id: exec_id,
            stage_execution_id: Some(stage_id),
            agent_id: agent_id.into(),
            provider: "claude".into(),
            model: Some("sonnet".into()),
            started_at: Utc::now(),
            completed_at: None,
            status: AgentStatus::Running,
            owner_execution_lineage_id: None,
            session_lineage_id: Some("lineage-1".into()),
            session_generation_id: Some(format!("generation-{exec_id}")),
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
    exec_id
}

fn runtime_receipt(exec_id: AgentExecutionId) -> AgentExecutionRuntimeReceiptRecord {
    AgentExecutionRuntimeReceiptRecord {
        agent_execution_id: exec_id,
        provider: "claude".into(),
        transport_family: "acp".into(),
        status: "completed".into(),
        failure_phase: None,
        event_count: 3,
        last_event_kind: Some("terminal_response".into()),
        last_event_at_ms: Some(42),
        receipt_json: "{}".into(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

fn prompt_runtime_receipt(
    exec_id: AgentExecutionId,
    runtime_receipt_id: &str,
    prompt_kind: &str,
    turn_index: i64,
) -> AgentExecutionRuntimePromptReceiptRecord {
    AgentExecutionRuntimePromptReceiptRecord {
        runtime_receipt_id: runtime_receipt_id.into(),
        agent_execution_id: exec_id,
        prompt_kind: prompt_kind.into(),
        turn_index,
        prompt_template_id: Some(format!("{prompt_kind}_template")),
        prompt_template_version: Some(1),
        prompt_sha256: Some(format!("{prompt_kind}-sha")),
        redacted_prompt_artifact_path: Some(format!(".chainworks/prompts/{prompt_kind}.md")),
        expected_output_contract_snapshot_sha256: Some("contract-sha".into()),
        expected_output_contract_snapshot_path: Some(".chainworks/contracts/snapshot.json".into()),
        repair_or_settlement_reason: Some("missing_required_outputs".into()),
        runtime_receipt: runtime_receipt(exec_id),
    }
}

fn minimal_completion_receipt(
    id: &str,
    run_id: RunId,
    stage_id: StageExecutionId,
    exec_id: AgentExecutionId,
    completion_status: &str,
    created_at: chrono::DateTime<Utc>,
) -> CodeWriterCompletionReceiptRecord {
    CodeWriterCompletionReceiptRecord {
        id: id.into(),
        run_id,
        stage_execution_id: stage_id,
        agent_execution_id: exec_id,
        session_generation_id: Some(format!("generation-{exec_id}")),
        original_runtime_receipt_id: Some(format!("{exec_id}:original:0")),
        completion_repair_runtime_receipt_id: None,
        provider: "claude".into(),
        model: Some("sonnet".into()),
        completion_mode: Some("provider_envelope".into()),
        published_at: (completion_status == "complete").then_some(created_at),
        activation_source: "declared_output_settlement_failed".into(),
        ingestion_boundary_failure: Some("none".into()),
        work_change_kind: Some("current_attempt_diff".into()),
        pre_prompt_worktree_fingerprint_path: None,
        post_prompt_worktree_fingerprint_path: None,
        pre_prompt_worktree_fingerprint_sha256: None,
        post_prompt_worktree_fingerprint_sha256: None,
        current_attempt_changed_path_count: 1,
        preexisting_dirty_path_count: 0,
        completion_status: completion_status.into(),
        failure_class: (completion_status != "complete")
            .then(|| "work_completed_missing_current_attempt_outputs".into()),
        provider_runtime_family: Some("claude_acp".into()),
        completion_boundary_subtype: Some("none".into()),
        final_payload_status: Some("not_applicable".into()),
        progress_before_handoff: Some("none".into()),
        runtime_preflight_phase: Some("ready".into()),
        runtime_tool_path_preflight_json: None,
        final_completion_payload_capture_json: None,
        engine_failure_envelope_json: None,
        repair_failure_envelope_json: None,
        repair_materialization_summary_json: None,
        repair_materialization_mode: Some("legacy_all_or_nothing".into()),
        strict_final_payload_enabled: false,
        staged_repair_settlement_enabled: false,
        terminal_response_status: Some("completed".into()),
        completion_turn_attempted: false,
        completion_turn_result: Some("not_attempted".into()),
        completion_text_capture_count: 0,
        completion_text_absence_count: 0,
        completion_repair_text_status: None,
        completion_repair_raw_text_artifact_path: None,
        completion_repair_redacted_text_artifact_path: None,
        completion_repair_text_absence_reason: None,
        fresh_required_output_count: (completion_status == "complete") as i64,
        stale_required_output_count: 0,
        missing_required_output_count: (completion_status != "complete") as i64,
        control_plane_output_count: 0,
        completion_repair_turn_count: 0,
        generic_repair_turn_count: 0,
        missing_outputs: if completion_status == "complete" {
            Vec::new()
        } else {
            vec!["implementation_progress".into()]
        },
        stale_outputs: Vec::new(),
        transcript_status: Some("unavailable".into()),
        transcript_absence_reason: Some("provider_did_not_supply".into()),
        receipt_artifact_path: None,
        failed_stage_evidence_path: None,
        created_at,
    }
}

fn minimal_completion_receipt_for_provider(
    id: &str,
    run_id: RunId,
    stage_id: StageExecutionId,
    exec_id: AgentExecutionId,
    provider: &str,
    created_at: chrono::DateTime<Utc>,
) -> CodeWriterCompletionReceiptRecord {
    let mut receipt = minimal_completion_receipt(
        id,
        run_id,
        stage_id,
        exec_id,
        "missing_required_outputs",
        created_at,
    );
    receipt.provider = provider.into();
    receipt.model = Some(match provider {
        "claude" => "claude-sonnet-4.5".into(),
        "codex" => "gpt-5.1-codex".into(),
        "junie" => "junie-acp".into(),
        _ => "provider-model".into(),
    });
    receipt.activation_source = "declared_output_settlement_failed".into();
    receipt.ingestion_boundary_failure = Some("chainworks_output_not_extracted".into());
    receipt.completion_mode = Some("acp_final_text_chainworks_output".into());
    receipt
}

#[tokio::test]
async fn proposal_088_runtime_receipts_preserve_prompt_level_rows_per_execution() {
    let pool = setup_db().await;
    let (_, _, exec_id) = seed_execution(&pool).await;

    let original = runtime_receipt(exec_id);
    let repair = prompt_runtime_receipt(
        exec_id,
        "rr-completion-repair",
        "code_writer_completion_repair",
        1,
    );

    agent_execution_runtime_receipts::upsert(&pool, &original)
        .await
        .unwrap();
    agent_execution_runtime_receipts::upsert_prompt_receipt(&pool, &repair)
        .await
        .unwrap();

    let listed = agent_execution_runtime_receipts::list_by_execution_id(&pool, exec_id)
        .await
        .unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].prompt_kind, "original");
    assert_eq!(listed[0].turn_index, 0);
    assert_eq!(listed[1].prompt_kind, "code_writer_completion_repair");
    assert_eq!(listed[1].turn_index, 1);
    assert_eq!(
        listed[1].redacted_prompt_artifact_path.as_deref(),
        Some(".chainworks/prompts/code_writer_completion_repair.md")
    );

    let original_readback = agent_execution_runtime_receipts::find_by_execution_id(&pool, exec_id)
        .await
        .unwrap()
        .expect("legacy readback should return original prompt receipt");
    assert_eq!(original_readback.agent_execution_id, exec_id);
    assert_eq!(original_readback.status, "completed");
}

#[tokio::test]
async fn proposal_088_completion_receipt_round_trips_with_text_and_output_decisions() {
    let pool = setup_db().await;
    let (run_id, stage_id, exec_id) = seed_execution(&pool).await;

    let receipt = CodeWriterCompletionReceiptRecord {
        id: "p088-receipt".into(),
        run_id,
        stage_execution_id: stage_id,
        agent_execution_id: exec_id,
        session_generation_id: Some("generation-1".into()),
        original_runtime_receipt_id: Some("rr-original".into()),
        completion_repair_runtime_receipt_id: Some("rr-completion-repair".into()),
        provider: "claude".into(),
        model: Some("sonnet".into()),
        completion_mode: Some("code_writer_completion_repair_turn".into()),
        published_at: None,
        activation_source: "declared_output_settlement_failed".into(),
        ingestion_boundary_failure: Some("none".into()),
        work_change_kind: Some("current_attempt_diff".into()),
        pre_prompt_worktree_fingerprint_path: Some(".chainworks/fingerprints/pre.json".into()),
        post_prompt_worktree_fingerprint_path: Some(".chainworks/fingerprints/post.json".into()),
        pre_prompt_worktree_fingerprint_sha256: Some("pre-sha".into()),
        post_prompt_worktree_fingerprint_sha256: Some("post-sha".into()),
        current_attempt_changed_path_count: 2,
        preexisting_dirty_path_count: 0,
        completion_status: "missing_required_outputs".into(),
        failure_class: Some("work_completed_missing_current_attempt_outputs".into()),
        provider_runtime_family: Some("claude_acp".into()),
        completion_boundary_subtype: Some("none".into()),
        final_payload_status: Some("repair_required".into()),
        progress_before_handoff: Some("none".into()),
        runtime_preflight_phase: Some("ready".into()),
        runtime_tool_path_preflight_json: None,
        final_completion_payload_capture_json: None,
        engine_failure_envelope_json: None,
        repair_failure_envelope_json: None,
        repair_materialization_summary_json: None,
        repair_materialization_mode: Some("legacy_all_or_nothing".into()),
        strict_final_payload_enabled: false,
        staged_repair_settlement_enabled: false,
        terminal_response_status: Some("completed".into()),
        completion_turn_attempted: true,
        completion_turn_result: Some("failed_missing_outputs".into()),
        completion_text_capture_count: 1,
        completion_text_absence_count: 0,
        completion_repair_text_status: Some("captured".into()),
        completion_repair_raw_text_artifact_path: None,
        completion_repair_redacted_text_artifact_path: Some(".chainworks/text/redacted.md".into()),
        completion_repair_text_absence_reason: None,
        fresh_required_output_count: 1,
        stale_required_output_count: 1,
        missing_required_output_count: 2,
        control_plane_output_count: 1,
        completion_repair_turn_count: 1,
        generic_repair_turn_count: 0,
        missing_outputs: vec!["implementation_progress".into(), "tests_result".into()],
        stale_outputs: vec!["implementation_self_assessment".into()],
        transcript_status: Some("unavailable".into()),
        transcript_absence_reason: Some("provider_did_not_supply".into()),
        receipt_artifact_path: Some(".chainworks/receipts/code-writer.json".into()),
        failed_stage_evidence_path: Some(".chainworks/evidence/failed.json".into()),
        created_at: Utc::now(),
    };
    let text_capture = CodeWriterCompletionTextCaptureRecord {
        receipt_id: receipt.id.clone(),
        prompt_kind: "code_writer_completion_repair".into(),
        turn_index: 1,
        terminal_response_status: Some("completed".into()),
        completion_text_status: "captured".into(),
        completion_text_capture_source: Some("terminal_final_response".into()),
        completion_text_raw_byte_limit: Some(262144),
        completion_text_captured_byte_count: Some(128),
        completion_text_truncated: false,
        extraction_input_truncated: false,
        extraction_input_sha256: Some("text-sha".into()),
        raw_text_artifact_path: None,
        redacted_text_artifact_path: Some(".chainworks/text/redacted.md".into()),
        text_absence_reason: None,
        created_at: Utc::now(),
    };
    let output_decision = CodeWriterCompletionOutputDecisionRecord {
        receipt_id: receipt.id.clone(),
        output_name: "implementation_progress".into(),
        contract_id: Some("implementation_progress_v1".into()),
        canonical_path: "implementation/progress.md".into(),
        pre_prompt_sha256: None,
        post_prompt_sha256: Some("post-output-sha".into()),
        content_sha256: Some("post-output-sha".into()),
        settlement_source: Some("code_writer_completion_repair_turn".into()),
        validation_status: Some("valid".into()),
        rejection_reason: None,
    };

    let original_runtime = runtime_receipt(exec_id);
    let repair_runtime = prompt_runtime_receipt(
        exec_id,
        "rr-completion-repair",
        "code_writer_completion_repair",
        1,
    );

    code_writer_completion_receipts::upsert_with_runtime_receipts(
        &pool,
        &receipt,
        &[text_capture.clone()],
        &[output_decision.clone()],
        Some(&original_runtime),
        Some(&repair_runtime),
    )
    .await
    .unwrap();

    let found = code_writer_completion_receipts::find_by_execution_id(&pool, exec_id)
        .await
        .unwrap()
        .expect("receipt should round-trip");
    assert_eq!(found.receipt, receipt);
    assert_eq!(found.text_captures, vec![text_capture]);
    assert_eq!(found.output_decisions, vec![output_decision]);
    assert_eq!(
        found
            .prompt_evidence
            .as_ref()
            .and_then(|evidence| evidence.prompt_template_id.as_deref()),
        Some("code_writer_completion_repair_template")
    );

    let listed = code_writer_completion_receipts::list_by_run(&pool, run_id)
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].receipt.id, "p088-receipt");

    let link = sqlx::query(
        "SELECT receipt_id, run_id, stage_execution_id FROM code_writer_completion_receipt_links WHERE agent_execution_id = ?1",
    )
    .bind(exec_id.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    let linked_receipt_id: String = link.get("receipt_id");
    let linked_run_id: String = link.get("run_id");
    let linked_stage_execution_id: String = link.get("stage_execution_id");
    assert_eq!(linked_receipt_id, "p088-receipt");
    assert_eq!(linked_run_id, run_id.to_string());
    assert_eq!(linked_stage_execution_id, stage_id.to_string());
}

#[tokio::test]
async fn proposal_088_provider_independence_round_trips_claude_codex_junie_receipts() {
    let pool = setup_db().await;
    let (run_id, stage_id, claude_exec_id) = seed_execution(&pool).await;
    let codex_exec_id = seed_additional_execution(&pool, stage_id, "code_writer").await;
    let junie_exec_id = seed_additional_execution(&pool, stage_id, "code_writer").await;

    let provider_execs = [
        ("claude", claude_exec_id),
        ("codex", codex_exec_id),
        ("junie", junie_exec_id),
    ];
    for (provider, exec_id) in provider_execs {
        sqlx::query("UPDATE agent_executions SET provider = ?1, model = ?2 WHERE id = ?3")
            .bind(provider)
            .bind(match provider {
                "claude" => "claude-sonnet-4.5",
                "codex" => "gpt-5.1-codex",
                "junie" => "junie-acp",
                _ => "provider-model",
            })
            .bind(exec_id.to_string())
            .execute(&pool)
            .await
            .unwrap();

        let receipt = minimal_completion_receipt_for_provider(
            &format!("p088-{provider}-receipt"),
            run_id,
            stage_id,
            exec_id,
            provider,
            Utc::now(),
        );
        code_writer_completion_receipts::upsert(&pool, &receipt, &[], &[])
            .await
            .unwrap();
    }

    let canonical = code_writer_completion_receipts::list_canonical_by_run(&pool, run_id)
        .await
        .unwrap();
    let mut providers: Vec<_> = canonical
        .iter()
        .map(|receipt| receipt.receipt.provider.as_str())
        .collect();
    providers.sort_unstable();
    assert_eq!(providers, vec!["claude", "codex", "junie"]);
    assert!(canonical.iter().all(|receipt| {
        receipt.receipt.failure_class.as_deref()
            == Some("work_completed_missing_current_attempt_outputs")
            && receipt.receipt.completion_turn_result.as_deref() == Some("not_attempted")
    }));

    let summary = domain::code_writer_completion::project_implementation_completion(&canonical);
    assert_eq!(summary.status.value, "failed");
    assert_eq!(
        summary.next_operator_action.value,
        "fix_chainworks_output_extraction"
    );
}

#[tokio::test]
async fn proposal_088_completion_receipt_replay_detects_text_capture_drift() {
    let pool = setup_db().await;
    let (run_id, stage_id, exec_id) = seed_execution(&pool).await;

    let receipt = CodeWriterCompletionReceiptRecord {
        id: "p088-receipt-conflict".into(),
        run_id,
        stage_execution_id: stage_id,
        agent_execution_id: exec_id,
        session_generation_id: Some("generation-1".into()),
        original_runtime_receipt_id: Some("rr-original".into()),
        completion_repair_runtime_receipt_id: None,
        provider: "claude".into(),
        model: Some("sonnet".into()),
        completion_mode: None,
        published_at: None,
        activation_source: "declared_output_settlement_failed".into(),
        ingestion_boundary_failure: Some("none".into()),
        work_change_kind: Some("current_attempt_diff".into()),
        pre_prompt_worktree_fingerprint_path: None,
        post_prompt_worktree_fingerprint_path: None,
        pre_prompt_worktree_fingerprint_sha256: None,
        post_prompt_worktree_fingerprint_sha256: None,
        current_attempt_changed_path_count: 1,
        preexisting_dirty_path_count: 0,
        completion_status: "missing_required_outputs".into(),
        failure_class: Some("work_completed_missing_current_attempt_outputs".into()),
        provider_runtime_family: Some("claude_acp".into()),
        completion_boundary_subtype: Some("none".into()),
        final_payload_status: Some("repair_required".into()),
        progress_before_handoff: Some("none".into()),
        runtime_preflight_phase: Some("ready".into()),
        runtime_tool_path_preflight_json: None,
        final_completion_payload_capture_json: None,
        engine_failure_envelope_json: None,
        repair_failure_envelope_json: None,
        repair_materialization_summary_json: None,
        repair_materialization_mode: Some("legacy_all_or_nothing".into()),
        strict_final_payload_enabled: false,
        staged_repair_settlement_enabled: false,
        terminal_response_status: Some("completed".into()),
        completion_turn_attempted: false,
        completion_turn_result: Some("not_attempted".into()),
        completion_text_capture_count: 1,
        completion_text_absence_count: 0,
        completion_repair_text_status: None,
        completion_repair_raw_text_artifact_path: None,
        completion_repair_redacted_text_artifact_path: None,
        completion_repair_text_absence_reason: None,
        fresh_required_output_count: 0,
        stale_required_output_count: 0,
        missing_required_output_count: 1,
        control_plane_output_count: 0,
        completion_repair_turn_count: 0,
        generic_repair_turn_count: 1,
        missing_outputs: vec!["implementation_progress".into()],
        stale_outputs: Vec::new(),
        transcript_status: Some("unavailable".into()),
        transcript_absence_reason: Some("transcript_not_collected".into()),
        receipt_artifact_path: Some(".chainworks/receipts/code-writer.json".into()),
        failed_stage_evidence_path: Some(".chainworks/evidence/failed.json".into()),
        created_at: Utc::now(),
    };
    let text_capture = CodeWriterCompletionTextCaptureRecord {
        receipt_id: receipt.id.clone(),
        prompt_kind: "original".into(),
        turn_index: 0,
        terminal_response_status: Some("completed".into()),
        completion_text_status: "captured".into(),
        completion_text_capture_source: Some("terminal_final_response".into()),
        completion_text_raw_byte_limit: Some(262144),
        completion_text_captured_byte_count: Some(128),
        completion_text_truncated: false,
        extraction_input_truncated: false,
        extraction_input_sha256: Some("text-sha".into()),
        raw_text_artifact_path: None,
        redacted_text_artifact_path: Some(".chainworks/text/redacted.md".into()),
        text_absence_reason: None,
        created_at: receipt.created_at,
    };
    let output_decision = CodeWriterCompletionOutputDecisionRecord {
        receipt_id: receipt.id.clone(),
        output_name: "implementation_progress".into(),
        contract_id: Some("implementation_progress_v1".into()),
        canonical_path: "implementation/progress.md".into(),
        pre_prompt_sha256: None,
        post_prompt_sha256: None,
        content_sha256: None,
        settlement_source: Some("missing".into()),
        validation_status: Some("missing".into()),
        rejection_reason: Some("missing_required_output".into()),
    };

    code_writer_completion_receipts::upsert(
        &pool,
        &receipt,
        &[text_capture.clone()],
        &[output_decision.clone()],
    )
    .await
    .unwrap();

    let mut drifted_capture = text_capture;
    drifted_capture.extraction_input_sha256 = Some("different-sha".into());
    let error = code_writer_completion_receipts::upsert(
        &pool,
        &receipt,
        &[drifted_capture],
        &[output_decision],
    )
    .await
    .unwrap_err();
    assert!(
        error.to_string().contains("completion_receipt_conflict"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn proposal_088_canonical_readback_uses_receipt_link_not_latest_created_at() {
    let pool = setup_db().await;
    let (run_id, stage_id, active_exec_id) = seed_execution(&pool).await;
    let historical_exec_id = seed_additional_execution(&pool, stage_id, "code_writer").await;

    let active_receipt = minimal_completion_receipt(
        "p088-active-linked",
        run_id,
        stage_id,
        active_exec_id,
        "missing_required_outputs",
        Utc::now() - Duration::minutes(10),
    );
    let historical_receipt = minimal_completion_receipt(
        "p088-historical-newer",
        run_id,
        stage_id,
        historical_exec_id,
        "complete",
        Utc::now(),
    );

    code_writer_completion_receipts::upsert(&pool, &active_receipt, &[], &[])
        .await
        .unwrap();
    code_writer_completion_receipts::upsert(&pool, &historical_receipt, &[], &[])
        .await
        .unwrap();
    sqlx::query("DELETE FROM code_writer_completion_receipt_links WHERE receipt_id = ?1")
        .bind(&historical_receipt.id)
        .execute(&pool)
        .await
        .unwrap();

    let all = code_writer_completion_receipts::list_by_run(&pool, run_id)
        .await
        .unwrap();
    assert_eq!(all.len(), 2);
    let canonical = code_writer_completion_receipts::list_canonical_by_run(&pool, run_id)
        .await
        .unwrap();
    assert_eq!(canonical.len(), 1);
    assert_eq!(canonical[0].receipt.id, "p088-active-linked");
    let summary = domain::code_writer_completion::project_implementation_completion(&canonical);
    assert_eq!(summary.status.value, "failed");
}

#[tokio::test]
async fn proposal_088_canonical_readback_returns_empty_when_links_are_missing() {
    let pool = setup_db().await;
    let (run_id, stage_id, exec_id) = seed_execution(&pool).await;

    let receipt = minimal_completion_receipt(
        "p088-unlinked",
        run_id,
        stage_id,
        exec_id,
        "missing_required_outputs",
        Utc::now(),
    );
    code_writer_completion_receipts::upsert(&pool, &receipt, &[], &[])
        .await
        .unwrap();
    sqlx::query("DELETE FROM code_writer_completion_receipt_links WHERE receipt_id = ?1")
        .bind(&receipt.id)
        .execute(&pool)
        .await
        .unwrap();

    let all = code_writer_completion_receipts::list_by_run(&pool, run_id)
        .await
        .unwrap();
    assert_eq!(all.len(), 1);
    let canonical = code_writer_completion_receipts::list_canonical_by_run(&pool, run_id)
        .await
        .unwrap();
    assert!(
        canonical.is_empty(),
        "unlinked P088 receipts must not become canonical readback"
    );
}

#[tokio::test]
async fn proposal_090_receipt_round_trips_boundary_subtype_and_preflight_readback() {
    let pool = setup_db().await;
    let (run_id, stage_id, exec_id) = seed_execution(&pool).await;
    let mut receipt = minimal_completion_receipt(
        "p090-boundary-subtype",
        run_id,
        stage_id,
        exec_id,
        "missing_required_outputs",
        Utc::now(),
    );
    receipt.provider = "junie".into();
    receipt.provider_runtime_family = Some("junie_acp".into());
    receipt.completion_boundary_subtype =
        Some("junie_runtime_tool_path_failure_before_publication".into());
    receipt.final_payload_status = Some("missing".into());
    receipt.progress_before_handoff = Some("none".into());
    receipt.runtime_preflight_phase = Some("failed_no_launch".into());
    receipt.runtime_tool_path_preflight_json = Some(
        serde_json::json!({
            "status": "failed",
            "attempt_count": 1,
            "provider_launched": false,
            "failed_operation_class": "read_project_file",
            "failure_category": "permission_denied"
        })
        .to_string(),
    );
    receipt.strict_final_payload_enabled = true;
    receipt.staged_repair_settlement_enabled = false;
    receipt.repair_materialization_mode = Some("legacy_all_or_nothing".into());

    code_writer_completion_receipts::upsert(&pool, &receipt, &[], &[])
        .await
        .unwrap();

    let canonical = code_writer_completion_receipts::list_canonical_by_run(&pool, run_id)
        .await
        .unwrap();
    assert_eq!(canonical.len(), 1);
    assert_eq!(
        canonical[0].receipt.completion_boundary_subtype.as_deref(),
        Some("junie_runtime_tool_path_failure_before_publication")
    );
    assert_eq!(
        canonical[0].receipt.runtime_preflight_phase.as_deref(),
        Some("failed_no_launch")
    );
    assert_eq!(canonical[0].receipt.strict_final_payload_enabled, true);

    let summary = domain::code_writer_completion::project_implementation_completion(&canonical);
    assert_eq!(
        summary.completion_boundary_subtype.value,
        "junie_runtime_tool_path_failure_before_publication"
    );
    assert_eq!(summary.completion_boundary_subtype.known, true);
    assert_eq!(summary.final_payload_status.as_deref(), Some("missing"));
    assert_eq!(
        summary.runtime_tool_path_preflight_json.as_deref(),
        receipt.runtime_tool_path_preflight_json.as_deref()
    );
    assert_eq!(
        summary.repair_materialization_mode.as_deref(),
        Some("legacy_all_or_nothing")
    );
}

#[tokio::test]
async fn proposal_090_settlement_rows_are_receipt_linked_and_idempotent_by_candidate_digest() {
    let pool = setup_db().await;
    let (run_id, stage_id, exec_id) = seed_execution(&pool).await;
    let mut receipt = minimal_completion_receipt(
        "p090-settlement",
        run_id,
        stage_id,
        exec_id,
        "partial_evidence",
        Utc::now(),
    );
    receipt.provider = "junie".into();
    receipt.completion_boundary_subtype =
        Some("junie_repair_outputs_partially_materialized".into());
    receipt.repair_materialization_mode = Some("staged_per_output".into());
    receipt.staged_repair_settlement_enabled = true;

    let accepted = domain::code_writer_completion::CodeWriterOutputSettlementRow {
        id: "p090-row-progress".into(),
        receipt_id: receipt.id.clone(),
        run_id,
        stage_id: "state_implementation".into(),
        stage_execution_id: stage_id,
        agent_execution_id: exec_id,
        session_generation_id: Some(format!("generation-{exec_id}")),
        repair_attempt: 1,
        output_name: "implementation_progress".into(),
        contract_id: "implementation_progress".into(),
        source_kind: "repair_chainworks_output".into(),
        source_generation_owner: "agent".into(),
        candidate_digest: Some("sha256:progress".into()),
        staging_path: Some(".chainworks/runs/r/repair-staging/progress.md".into()),
        canonical_path: "implementation/progress.md".into(),
        canonical_before_sha256: None,
        canonical_after_sha256: Some("sha256:progress".into()),
        decision: "accepted".into(),
        rejection_reason: None,
        materialization_state: "committed".into(),
        active_pointer_generation_id: Some("gen-progress".into()),
        created_at: Utc::now(),
        committed_at: Some(Utc::now()),
    };
    let rejected = domain::code_writer_completion::CodeWriterOutputSettlementRow {
        id: "p090-row-self-assessment".into(),
        receipt_id: receipt.id.clone(),
        output_name: "implementation_self_assessment".into(),
        contract_id: "implementation_self_assessment_v2".into(),
        candidate_digest: Some("sha256:bad-self-assessment".into()),
        canonical_path: "implementation/self-assessment.json".into(),
        canonical_before_sha256: Some("sha256:old-valid".into()),
        canonical_after_sha256: None,
        decision: "rejected".into(),
        rejection_reason: Some("malformed_json".into()),
        materialization_state: "not_materialized".into(),
        active_pointer_generation_id: None,
        ..accepted.clone()
    };

    code_writer_completion_receipts::upsert_with_settlement_rows(
        &pool,
        &receipt,
        &[],
        &[],
        &[accepted.clone(), rejected.clone()],
    )
    .await
    .unwrap();
    code_writer_completion_receipts::upsert_with_settlement_rows(
        &pool,
        &receipt,
        &[],
        &[],
        &[accepted.clone(), rejected.clone()],
    )
    .await
    .unwrap();

    let found = code_writer_completion_receipts::find_by_execution_id(&pool, exec_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.settlement_rows.len(), 2);
    assert_eq!(
        found
            .settlement_rows
            .iter()
            .find(|row| row.output_name == "implementation_self_assessment")
            .unwrap()
            .canonical_before_sha256
            .as_deref(),
        Some("sha256:old-valid")
    );

    let staged = domain::code_writer_completion::CodeWriterOutputSettlementRow {
        id: "p090-row-staged-crash".into(),
        receipt_id: receipt.id.clone(),
        run_id,
        stage_id: "state_implementation".into(),
        stage_execution_id: stage_id,
        agent_execution_id: exec_id,
        session_generation_id: Some(format!("generation-{exec_id}")),
        repair_attempt: 2,
        output_name: "tests_result".into(),
        contract_id: "tests_result".into(),
        source_kind: "repair_chainworks_output".into(),
        source_generation_owner: "agent".into(),
        candidate_digest: Some("sha256:tests".into()),
        staging_path: Some(".chainworks/runs/r/repair-staging/tests.json".into()),
        canonical_path: "implementation/tests.json".into(),
        canonical_before_sha256: Some("sha256:old-tests".into()),
        canonical_after_sha256: None,
        decision: "accepted".into(),
        rejection_reason: None,
        materialization_state: "staged".into(),
        active_pointer_generation_id: Some("gen-tests".into()),
        created_at: Utc::now(),
        committed_at: None,
    };
    code_writer_completion_receipts::upsert_with_settlement_rows(
        &pool,
        &receipt,
        &[],
        &[],
        &[accepted.clone(), rejected.clone(), staged.clone()],
    )
    .await
    .unwrap();
    let recoverable =
        code_writer_completion_receipts::list_p090_recoverable_settlement_rows_by_run(
            &pool, run_id,
        )
        .await
        .unwrap();
    assert!(recoverable.iter().any(|row| row.id == staged.id));
    code_writer_completion_receipts::update_p090_settlement_row_recovery_state(
        &pool,
        &staged.id,
        "failed",
        Some("sha256:old-tests"),
        None,
        Some("startup_recovery_left_staged_output_unpromoted"),
    )
    .await
    .unwrap();
    let recovered = code_writer_completion_receipts::find_by_execution_id(&pool, exec_id)
        .await
        .unwrap()
        .unwrap();
    let staged_after_recovery = recovered
        .settlement_rows
        .iter()
        .find(|row| row.id == staged.id)
        .unwrap();
    assert_eq!(staged_after_recovery.materialization_state, "failed");
    assert_eq!(
        staged_after_recovery.rejection_reason.as_deref(),
        Some("startup_recovery_left_staged_output_unpromoted")
    );

    let mut conflicting = accepted.clone();
    conflicting.id = "p090-row-progress-conflict".into();
    conflicting.candidate_digest = Some("sha256:different".into());
    let error = code_writer_completion_receipts::upsert_with_settlement_rows(
        &pool,
        &receipt,
        &[],
        &[],
        &[conflicting],
    )
    .await
    .expect_err("different digest for same repair attempt/output should fail");
    assert!(
        error
            .to_string()
            .contains("code_writer_output_settlement_conflict"),
        "unexpected error: {error:#}"
    );
}
