use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use domain::agent::{AgentExecutionRuntimePromptReceiptRecord, AgentExecutionRuntimeReceiptRecord};
use domain::code_writer_completion::{
    CodeWriterCompletionOutputDecisionRecord, CodeWriterCompletionPromptEvidenceReadback,
    CodeWriterCompletionReceiptReadback, CodeWriterCompletionReceiptRecord,
    CodeWriterCompletionTextCaptureRecord, CodeWriterOutputSettlementRow,
};
use domain::ids::{AgentExecutionId, RunId};

use super::agent_execution_runtime_receipts;

pub async fn upsert(
    pool: &SqlitePool,
    receipt: &CodeWriterCompletionReceiptRecord,
    text_captures: &[CodeWriterCompletionTextCaptureRecord],
    output_decisions: &[CodeWriterCompletionOutputDecisionRecord],
) -> Result<()> {
    if let Some(existing) = find_by_execution_id(pool, receipt.agent_execution_id).await? {
        if existing.receipt.id != receipt.id {
            return Err(anyhow!("completion_receipt_conflict"));
        }
        if existing.receipt != *receipt {
            return Err(anyhow!("completion_receipt_conflict"));
        }
        if existing.text_captures != text_captures {
            return Err(anyhow!("completion_receipt_conflict"));
        }
        if existing.output_decisions != output_decisions {
            return Err(anyhow!("completion_receipt_conflict"));
        }
    }

    let mut tx =
        crate::writer::begin_repository_transaction(pool, "code_writer_completion_receipts.upsert")
            .await?;
    upsert_tx(&mut tx, receipt, text_captures, output_decisions).await?;
    tx.commit().await?;
    crate::repos::projections::rebuild_run_summary(pool, receipt.run_id).await?;
    Ok(())
}

pub async fn upsert_with_settlement_rows(
    pool: &SqlitePool,
    receipt: &CodeWriterCompletionReceiptRecord,
    text_captures: &[CodeWriterCompletionTextCaptureRecord],
    output_decisions: &[CodeWriterCompletionOutputDecisionRecord],
    settlement_rows: &[CodeWriterOutputSettlementRow],
) -> Result<()> {
    validate_settlement_rows_belong_to_receipt(receipt, settlement_rows)?;
    detect_settlement_digest_conflict(pool, receipt, settlement_rows).await?;
    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "code_writer_completion_receipts.upsert_with_settlement_rows",
    )
    .await?;
    upsert_tx(&mut tx, receipt, text_captures, output_decisions).await?;
    replace_settlement_rows_tx(&mut tx, &receipt.id, settlement_rows).await?;
    tx.commit().await?;
    crate::repos::projections::rebuild_run_summary(pool, receipt.run_id).await?;
    Ok(())
}

pub async fn upsert_with_runtime_receipts(
    pool: &SqlitePool,
    receipt: &CodeWriterCompletionReceiptRecord,
    text_captures: &[CodeWriterCompletionTextCaptureRecord],
    output_decisions: &[CodeWriterCompletionOutputDecisionRecord],
    original_runtime_receipt: Option<&AgentExecutionRuntimeReceiptRecord>,
    repair_runtime_receipt: Option<&AgentExecutionRuntimePromptReceiptRecord>,
) -> Result<()> {
    if let Some(existing) = find_by_execution_id(pool, receipt.agent_execution_id).await? {
        if existing.receipt.id != receipt.id
            || existing.receipt != *receipt
            || existing.text_captures != text_captures
            || existing.output_decisions != output_decisions
        {
            return Err(anyhow!("completion_receipt_conflict"));
        }
    }

    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "code_writer_completion_receipts.upsert_with_runtime_receipts",
    )
    .await?;
    if let Some(original_runtime_receipt) = original_runtime_receipt {
        agent_execution_runtime_receipts::upsert_tx(&mut tx, original_runtime_receipt).await?;
    }
    if let Some(repair_runtime_receipt) = repair_runtime_receipt {
        agent_execution_runtime_receipts::upsert_prompt_receipt_tx(&mut tx, repair_runtime_receipt)
            .await?;
    }
    upsert_tx(&mut tx, receipt, text_captures, output_decisions).await?;
    tx.commit().await?;
    crate::repos::projections::rebuild_run_summary(pool, receipt.run_id).await?;
    Ok(())
}

pub async fn upsert_with_runtime_receipts_and_settlement_rows(
    pool: &SqlitePool,
    receipt: &CodeWriterCompletionReceiptRecord,
    text_captures: &[CodeWriterCompletionTextCaptureRecord],
    output_decisions: &[CodeWriterCompletionOutputDecisionRecord],
    settlement_rows: &[CodeWriterOutputSettlementRow],
    original_runtime_receipt: Option<&AgentExecutionRuntimeReceiptRecord>,
    repair_runtime_receipt: Option<&AgentExecutionRuntimePromptReceiptRecord>,
) -> Result<()> {
    validate_settlement_rows_belong_to_receipt(receipt, settlement_rows)?;
    if let Some(existing) = find_by_execution_id(pool, receipt.agent_execution_id).await? {
        if existing.receipt.id != receipt.id
            || existing.text_captures != text_captures
            || existing.output_decisions != output_decisions
        {
            return Err(anyhow!("completion_receipt_conflict"));
        }
    }
    detect_settlement_digest_conflict(pool, receipt, settlement_rows).await?;

    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "code_writer_completion_receipts.upsert_with_runtime_receipts_and_settlement_rows",
    )
    .await?;
    if let Some(original_runtime_receipt) = original_runtime_receipt {
        agent_execution_runtime_receipts::upsert_tx(&mut tx, original_runtime_receipt).await?;
    }
    if let Some(repair_runtime_receipt) = repair_runtime_receipt {
        agent_execution_runtime_receipts::upsert_prompt_receipt_tx(&mut tx, repair_runtime_receipt)
            .await?;
    }
    upsert_tx(&mut tx, receipt, text_captures, output_decisions).await?;
    replace_settlement_rows_tx(&mut tx, &receipt.id, settlement_rows).await?;
    tx.commit().await?;
    crate::repos::projections::rebuild_run_summary(pool, receipt.run_id).await?;
    Ok(())
}

pub async fn upsert_tx(
    tx: &mut Transaction<'_, Sqlite>,
    receipt: &CodeWriterCompletionReceiptRecord,
    text_captures: &[CodeWriterCompletionTextCaptureRecord],
    output_decisions: &[CodeWriterCompletionOutputDecisionRecord],
) -> Result<()> {
    let missing_outputs_json = encode_string_vec(&receipt.missing_outputs)?;
    let stale_outputs_json = encode_string_vec(&receipt.stale_outputs)?;
    sqlx::query(
        r#"INSERT INTO code_writer_completion_receipts
           (id, run_id, stage_execution_id, agent_execution_id, session_generation_id,
            original_runtime_receipt_id, completion_repair_runtime_receipt_id, provider, model,
            completion_mode, published_at, activation_source, ingestion_boundary_failure, work_change_kind,
            pre_prompt_worktree_fingerprint_path, post_prompt_worktree_fingerprint_path,
            pre_prompt_worktree_fingerprint_sha256, post_prompt_worktree_fingerprint_sha256,
            current_attempt_changed_path_count, preexisting_dirty_path_count, completion_status,
            failure_class, provider_runtime_family, completion_boundary_subtype,
            final_payload_status, progress_before_handoff, runtime_preflight_phase,
            runtime_tool_path_preflight_json, final_completion_payload_capture_json,
            engine_failure_envelope_json, repair_failure_envelope_json,
            repair_materialization_summary_json, repair_materialization_mode,
            strict_final_payload_enabled, staged_repair_settlement_enabled,
            terminal_response_status, completion_turn_attempted,
            completion_turn_result, completion_text_capture_count, completion_text_absence_count,
            completion_repair_text_status, completion_repair_raw_text_artifact_path,
            completion_repair_redacted_text_artifact_path, completion_repair_text_absence_reason,
            fresh_required_output_count, stale_required_output_count, missing_required_output_count,
            control_plane_output_count, completion_repair_turn_count, generic_repair_turn_count,
            missing_outputs, stale_outputs, transcript_status, transcript_absence_reason,
            receipt_artifact_path, failed_stage_evidence_path, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                   ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28,
                   ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38, ?39, ?40, ?41,
                   ?42, ?43, ?44, ?45, ?46, ?47, ?48, ?49, ?50, ?51, ?52, ?53, ?54,
                   ?55, ?56, ?57)
           ON CONFLICT(agent_execution_id) DO UPDATE SET
             id = excluded.id,
             run_id = excluded.run_id,
             stage_execution_id = excluded.stage_execution_id,
             session_generation_id = excluded.session_generation_id,
             original_runtime_receipt_id = excluded.original_runtime_receipt_id,
             completion_repair_runtime_receipt_id = excluded.completion_repair_runtime_receipt_id,
             provider = excluded.provider,
             model = excluded.model,
             completion_mode = excluded.completion_mode,
             published_at = excluded.published_at,
             activation_source = excluded.activation_source,
             ingestion_boundary_failure = excluded.ingestion_boundary_failure,
             work_change_kind = excluded.work_change_kind,
             pre_prompt_worktree_fingerprint_path = excluded.pre_prompt_worktree_fingerprint_path,
             post_prompt_worktree_fingerprint_path = excluded.post_prompt_worktree_fingerprint_path,
             pre_prompt_worktree_fingerprint_sha256 = excluded.pre_prompt_worktree_fingerprint_sha256,
             post_prompt_worktree_fingerprint_sha256 = excluded.post_prompt_worktree_fingerprint_sha256,
             current_attempt_changed_path_count = excluded.current_attempt_changed_path_count,
             preexisting_dirty_path_count = excluded.preexisting_dirty_path_count,
             completion_status = excluded.completion_status,
             failure_class = excluded.failure_class,
             provider_runtime_family = excluded.provider_runtime_family,
             completion_boundary_subtype = excluded.completion_boundary_subtype,
             final_payload_status = excluded.final_payload_status,
             progress_before_handoff = excluded.progress_before_handoff,
             runtime_preflight_phase = excluded.runtime_preflight_phase,
             runtime_tool_path_preflight_json = excluded.runtime_tool_path_preflight_json,
             final_completion_payload_capture_json = excluded.final_completion_payload_capture_json,
             engine_failure_envelope_json = excluded.engine_failure_envelope_json,
             repair_failure_envelope_json = excluded.repair_failure_envelope_json,
             repair_materialization_summary_json = excluded.repair_materialization_summary_json,
             repair_materialization_mode = excluded.repair_materialization_mode,
             strict_final_payload_enabled = excluded.strict_final_payload_enabled,
             staged_repair_settlement_enabled = excluded.staged_repair_settlement_enabled,
             terminal_response_status = excluded.terminal_response_status,
             completion_turn_attempted = excluded.completion_turn_attempted,
             completion_turn_result = excluded.completion_turn_result,
             completion_text_capture_count = excluded.completion_text_capture_count,
             completion_text_absence_count = excluded.completion_text_absence_count,
             completion_repair_text_status = excluded.completion_repair_text_status,
             completion_repair_raw_text_artifact_path = excluded.completion_repair_raw_text_artifact_path,
             completion_repair_redacted_text_artifact_path = excluded.completion_repair_redacted_text_artifact_path,
             completion_repair_text_absence_reason = excluded.completion_repair_text_absence_reason,
             fresh_required_output_count = excluded.fresh_required_output_count,
             stale_required_output_count = excluded.stale_required_output_count,
             missing_required_output_count = excluded.missing_required_output_count,
             control_plane_output_count = excluded.control_plane_output_count,
             completion_repair_turn_count = excluded.completion_repair_turn_count,
             generic_repair_turn_count = excluded.generic_repair_turn_count,
             missing_outputs = excluded.missing_outputs,
             stale_outputs = excluded.stale_outputs,
             transcript_status = excluded.transcript_status,
             transcript_absence_reason = excluded.transcript_absence_reason,
             receipt_artifact_path = excluded.receipt_artifact_path,
             failed_stage_evidence_path = excluded.failed_stage_evidence_path,
             created_at = excluded.created_at"#,
    )
    .bind(&receipt.id)
    .bind(receipt.run_id.to_string())
    .bind(receipt.stage_execution_id.to_string())
    .bind(receipt.agent_execution_id.to_string())
    .bind(&receipt.session_generation_id)
    .bind(&receipt.original_runtime_receipt_id)
    .bind(&receipt.completion_repair_runtime_receipt_id)
    .bind(&receipt.provider)
    .bind(&receipt.model)
    .bind(&receipt.completion_mode)
    .bind(receipt.published_at.map(|published_at| published_at.to_rfc3339()))
    .bind(&receipt.activation_source)
    .bind(&receipt.ingestion_boundary_failure)
    .bind(&receipt.work_change_kind)
    .bind(&receipt.pre_prompt_worktree_fingerprint_path)
    .bind(&receipt.post_prompt_worktree_fingerprint_path)
    .bind(&receipt.pre_prompt_worktree_fingerprint_sha256)
    .bind(&receipt.post_prompt_worktree_fingerprint_sha256)
    .bind(receipt.current_attempt_changed_path_count)
    .bind(receipt.preexisting_dirty_path_count)
    .bind(&receipt.completion_status)
    .bind(&receipt.failure_class)
    .bind(&receipt.provider_runtime_family)
    .bind(&receipt.completion_boundary_subtype)
    .bind(&receipt.final_payload_status)
    .bind(&receipt.progress_before_handoff)
    .bind(&receipt.runtime_preflight_phase)
    .bind(&receipt.runtime_tool_path_preflight_json)
    .bind(&receipt.final_completion_payload_capture_json)
    .bind(&receipt.engine_failure_envelope_json)
    .bind(&receipt.repair_failure_envelope_json)
    .bind(&receipt.repair_materialization_summary_json)
    .bind(&receipt.repair_materialization_mode)
    .bind(receipt.strict_final_payload_enabled)
    .bind(receipt.staged_repair_settlement_enabled)
    .bind(&receipt.terminal_response_status)
    .bind(receipt.completion_turn_attempted)
    .bind(&receipt.completion_turn_result)
    .bind(receipt.completion_text_capture_count)
    .bind(receipt.completion_text_absence_count)
    .bind(&receipt.completion_repair_text_status)
    .bind(&receipt.completion_repair_raw_text_artifact_path)
    .bind(&receipt.completion_repair_redacted_text_artifact_path)
    .bind(&receipt.completion_repair_text_absence_reason)
    .bind(receipt.fresh_required_output_count)
    .bind(receipt.stale_required_output_count)
    .bind(receipt.missing_required_output_count)
    .bind(receipt.control_plane_output_count)
    .bind(receipt.completion_repair_turn_count)
    .bind(receipt.generic_repair_turn_count)
    .bind(&missing_outputs_json)
    .bind(&stale_outputs_json)
    .bind(&receipt.transcript_status)
    .bind(&receipt.transcript_absence_reason)
    .bind(&receipt.receipt_artifact_path)
    .bind(&receipt.failed_stage_evidence_path)
    .bind(receipt.created_at.to_rfc3339())
    .execute(&mut **tx)
    .await?;

    sqlx::query("DELETE FROM code_writer_completion_text_captures WHERE receipt_id = ?1")
        .bind(&receipt.id)
        .execute(&mut **tx)
        .await?;
    for capture in text_captures {
        insert_text_capture_tx(tx, capture).await?;
    }

    sqlx::query("DELETE FROM code_writer_completion_output_decisions WHERE receipt_id = ?1")
        .bind(&receipt.id)
        .execute(&mut **tx)
        .await?;
    for decision in output_decisions {
        insert_output_decision_tx(tx, decision).await?;
    }

    sqlx::query(
        r#"INSERT INTO code_writer_completion_receipt_links
           (agent_execution_id, receipt_id, run_id, stage_execution_id, created_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?5)
           ON CONFLICT(agent_execution_id) DO UPDATE SET
             receipt_id = excluded.receipt_id,
             run_id = excluded.run_id,
             stage_execution_id = excluded.stage_execution_id,
             updated_at = excluded.updated_at"#,
    )
    .bind(receipt.agent_execution_id.to_string())
    .bind(&receipt.id)
    .bind(receipt.run_id.to_string())
    .bind(receipt.stage_execution_id.to_string())
    .bind(receipt.created_at.to_rfc3339())
    .execute(&mut **tx)
    .await?;

    Ok(())
}

fn validate_settlement_rows_belong_to_receipt(
    receipt: &CodeWriterCompletionReceiptRecord,
    settlement_rows: &[CodeWriterOutputSettlementRow],
) -> Result<()> {
    for row in settlement_rows {
        if row.receipt_id != receipt.id
            || row.run_id != receipt.run_id
            || row.stage_execution_id != receipt.stage_execution_id
            || row.agent_execution_id != receipt.agent_execution_id
            || row.session_generation_id.is_none()
        {
            return Err(anyhow!("code_writer_output_settlement_receipt_mismatch"));
        }
    }
    Ok(())
}

async fn detect_settlement_digest_conflict(
    pool: &SqlitePool,
    receipt: &CodeWriterCompletionReceiptRecord,
    settlement_rows: &[CodeWriterOutputSettlementRow],
) -> Result<()> {
    for row in settlement_rows {
        let Some(candidate_digest) = row.candidate_digest.as_deref() else {
            continue;
        };
        let existing = sqlx::query(
            r#"SELECT id, candidate_digest
               FROM code_writer_output_settlement_rows
               WHERE agent_execution_id = ?1
                 AND repair_attempt = ?2
                 AND output_name = ?3
                 AND candidate_digest IS NOT NULL"#,
        )
        .bind(receipt.agent_execution_id.to_string())
        .bind(row.repair_attempt)
        .bind(&row.output_name)
        .fetch_optional(pool)
        .await?;
        if let Some(existing) = existing {
            let existing_id: String = existing.get("id");
            let existing_digest: String = existing.get("candidate_digest");
            if existing_id != row.id || existing_digest != candidate_digest {
                return Err(anyhow!("code_writer_output_settlement_conflict"));
            }
        }
    }
    Ok(())
}

async fn replace_settlement_rows_tx(
    tx: &mut Transaction<'_, Sqlite>,
    receipt_id: &str,
    settlement_rows: &[CodeWriterOutputSettlementRow],
) -> Result<()> {
    sqlx::query("DELETE FROM code_writer_output_settlement_rows WHERE receipt_id = ?1")
        .bind(receipt_id)
        .execute(&mut **tx)
        .await?;
    for row in settlement_rows {
        insert_settlement_row_tx(tx, row).await?;
    }
    Ok(())
}

async fn insert_settlement_row_tx(
    tx: &mut Transaction<'_, Sqlite>,
    row: &CodeWriterOutputSettlementRow,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO code_writer_output_settlement_rows
           (id, receipt_id, run_id, stage_id, stage_execution_id, agent_execution_id,
            session_generation_id, repair_attempt, output_name, contract_id, source_kind,
            source_generation_owner, candidate_digest, staging_path, canonical_path,
            canonical_before_sha256, canonical_after_sha256, decision, rejection_reason,
            materialization_state, active_pointer_generation_id, created_at, committed_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                   ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)"#,
    )
    .bind(&row.id)
    .bind(&row.receipt_id)
    .bind(row.run_id.to_string())
    .bind(&row.stage_id)
    .bind(row.stage_execution_id.to_string())
    .bind(row.agent_execution_id.to_string())
    .bind(&row.session_generation_id)
    .bind(row.repair_attempt)
    .bind(&row.output_name)
    .bind(&row.contract_id)
    .bind(&row.source_kind)
    .bind(&row.source_generation_owner)
    .bind(&row.candidate_digest)
    .bind(&row.staging_path)
    .bind(&row.canonical_path)
    .bind(&row.canonical_before_sha256)
    .bind(&row.canonical_after_sha256)
    .bind(&row.decision)
    .bind(&row.rejection_reason)
    .bind(&row.materialization_state)
    .bind(&row.active_pointer_generation_id)
    .bind(row.created_at.to_rfc3339())
    .bind(
        row.committed_at
            .map(|committed_at| committed_at.to_rfc3339()),
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn find_by_execution_id(
    pool: &SqlitePool,
    agent_execution_id: AgentExecutionId,
) -> Result<Option<CodeWriterCompletionReceiptReadback>> {
    let row = sqlx::query(sqlx::AssertSqlSafe(receipt_select_sql(
        "WHERE agent_execution_id = ?1",
    )))
    .bind(agent_execution_id.to_string())
    .fetch_optional(pool)
    .await?;
    match row {
        Some(row) => readback_for_row(pool, &row).await.map(Some),
        None => Ok(None),
    }
}

pub async fn list_by_run(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<Vec<CodeWriterCompletionReceiptReadback>> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(receipt_select_sql(
        "WHERE run_id = ?1 ORDER BY created_at ASC",
    )))
    .bind(run_id.to_string())
    .fetch_all(pool)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(readback_for_row(pool, &row).await?);
    }
    Ok(out)
}

pub async fn list_canonical_by_run(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<Vec<CodeWriterCompletionReceiptReadback>> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(receipt_select_sql(
        r#"INNER JOIN code_writer_completion_receipt_links cwc_link
             ON cwc_link.receipt_id = code_writer_completion_receipts.id
           WHERE cwc_link.run_id = ?1
           ORDER BY cwc_link.updated_at DESC, code_writer_completion_receipts.created_at DESC"#,
    )))
    .bind(run_id.to_string())
    .fetch_all(pool)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(readback_for_row(pool, &row).await?);
    }
    Ok(out)
}

pub async fn consecutive_completed_no_diff_count_by_run(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<usize> {
    let receipts = list_canonical_by_run(pool, run_id).await?;
    let mut count = 0;
    for readback in receipts {
        let receipt = readback.receipt;
        if receipt.completion_status != "complete" {
            break;
        }
        if receipt.current_attempt_changed_path_count == 0 {
            count += 1;
            continue;
        }
        break;
    }
    Ok(count)
}

async fn readback_for_row(
    pool: &SqlitePool,
    row: &sqlx::sqlite::SqliteRow,
) -> Result<CodeWriterCompletionReceiptReadback> {
    let receipt = parse_receipt_row(row)?;
    let text_captures = list_text_captures(pool, &receipt.id).await?;
    let output_decisions = list_output_decisions(pool, &receipt.id).await?;
    let settlement_rows = list_settlement_rows(pool, &receipt.id).await?;
    let prompt_evidence = completion_prompt_evidence(pool, receipt.agent_execution_id).await?;
    Ok(CodeWriterCompletionReceiptReadback {
        receipt,
        text_captures,
        output_decisions,
        settlement_rows,
        prompt_evidence,
    })
}

async fn completion_prompt_evidence(
    pool: &SqlitePool,
    agent_execution_id: AgentExecutionId,
) -> Result<Option<CodeWriterCompletionPromptEvidenceReadback>> {
    let runtime_receipts =
        agent_execution_runtime_receipts::list_by_execution_id(pool, agent_execution_id).await?;
    Ok(runtime_receipts
        .into_iter()
        .find(|receipt| receipt.prompt_kind == "code_writer_completion_repair")
        .map(|receipt| CodeWriterCompletionPromptEvidenceReadback {
            runtime_receipt_id: receipt.runtime_receipt_id,
            prompt_kind: receipt.prompt_kind,
            turn_index: receipt.turn_index,
            prompt_template_id: receipt.prompt_template_id,
            prompt_template_version: receipt.prompt_template_version,
            prompt_sha256: receipt.prompt_sha256,
            redacted_prompt_artifact_path: receipt.redacted_prompt_artifact_path,
            expected_output_contract_snapshot_sha256: receipt
                .expected_output_contract_snapshot_sha256,
            expected_output_contract_snapshot_path: receipt.expected_output_contract_snapshot_path,
            repair_or_settlement_reason: receipt.repair_or_settlement_reason,
        }))
}

async fn insert_text_capture_tx(
    tx: &mut Transaction<'_, Sqlite>,
    capture: &CodeWriterCompletionTextCaptureRecord,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO code_writer_completion_text_captures
           (receipt_id, prompt_kind, turn_index, terminal_response_status, completion_text_status,
            completion_text_capture_source, completion_text_raw_byte_limit,
            completion_text_captured_byte_count, completion_text_truncated,
            extraction_input_truncated, extraction_input_sha256, raw_text_artifact_path,
            redacted_text_artifact_path, text_absence_reason, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)"#,
    )
    .bind(&capture.receipt_id)
    .bind(&capture.prompt_kind)
    .bind(capture.turn_index)
    .bind(&capture.terminal_response_status)
    .bind(&capture.completion_text_status)
    .bind(&capture.completion_text_capture_source)
    .bind(capture.completion_text_raw_byte_limit)
    .bind(capture.completion_text_captured_byte_count)
    .bind(capture.completion_text_truncated)
    .bind(capture.extraction_input_truncated)
    .bind(&capture.extraction_input_sha256)
    .bind(&capture.raw_text_artifact_path)
    .bind(&capture.redacted_text_artifact_path)
    .bind(&capture.text_absence_reason)
    .bind(capture.created_at.to_rfc3339())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn insert_output_decision_tx(
    tx: &mut Transaction<'_, Sqlite>,
    decision: &CodeWriterCompletionOutputDecisionRecord,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO code_writer_completion_output_decisions
           (receipt_id, output_name, contract_id, canonical_path, pre_prompt_sha256,
            post_prompt_sha256, content_sha256, settlement_source, validation_status,
            rejection_reason)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
    )
    .bind(&decision.receipt_id)
    .bind(&decision.output_name)
    .bind(&decision.contract_id)
    .bind(&decision.canonical_path)
    .bind(&decision.pre_prompt_sha256)
    .bind(&decision.post_prompt_sha256)
    .bind(&decision.content_sha256)
    .bind(&decision.settlement_source)
    .bind(&decision.validation_status)
    .bind(&decision.rejection_reason)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn list_text_captures(
    pool: &SqlitePool,
    receipt_id: &str,
) -> Result<Vec<CodeWriterCompletionTextCaptureRecord>> {
    let rows = sqlx::query(
        r#"SELECT receipt_id, prompt_kind, turn_index, terminal_response_status,
                  completion_text_status, completion_text_capture_source,
                  completion_text_raw_byte_limit, completion_text_captured_byte_count,
                  completion_text_truncated, extraction_input_truncated, extraction_input_sha256,
                  raw_text_artifact_path, redacted_text_artifact_path, text_absence_reason,
                  created_at
           FROM code_writer_completion_text_captures
           WHERE receipt_id = ?1
           ORDER BY turn_index ASC, prompt_kind ASC"#,
    )
    .bind(receipt_id)
    .fetch_all(pool)
    .await?;
    rows.iter().map(parse_text_capture_row).collect()
}

async fn list_output_decisions(
    pool: &SqlitePool,
    receipt_id: &str,
) -> Result<Vec<CodeWriterCompletionOutputDecisionRecord>> {
    let rows = sqlx::query(
        r#"SELECT receipt_id, output_name, contract_id, canonical_path, pre_prompt_sha256,
                  post_prompt_sha256, content_sha256, settlement_source, validation_status,
                  rejection_reason
           FROM code_writer_completion_output_decisions
           WHERE receipt_id = ?1
           ORDER BY output_name ASC"#,
    )
    .bind(receipt_id)
    .fetch_all(pool)
    .await?;
    rows.iter().map(parse_output_decision_row).collect()
}

async fn list_settlement_rows(
    pool: &SqlitePool,
    receipt_id: &str,
) -> Result<Vec<CodeWriterOutputSettlementRow>> {
    let rows = sqlx::query(
        r#"SELECT id, receipt_id, run_id, stage_id, stage_execution_id, agent_execution_id,
                  session_generation_id, repair_attempt, output_name, contract_id, source_kind,
                  source_generation_owner, candidate_digest, staging_path, canonical_path,
                  canonical_before_sha256, canonical_after_sha256, decision, rejection_reason,
                  materialization_state, active_pointer_generation_id, created_at, committed_at
           FROM code_writer_output_settlement_rows
           WHERE receipt_id = ?1
           ORDER BY output_name ASC"#,
    )
    .bind(receipt_id)
    .fetch_all(pool)
    .await?;
    rows.iter().map(parse_settlement_row).collect()
}

pub async fn list_p090_recoverable_settlement_rows_by_run(
    pool: &SqlitePool,
    run_id: domain::ids::RunId,
) -> Result<Vec<CodeWriterOutputSettlementRow>> {
    let rows = sqlx::query(
        r#"SELECT id, receipt_id, run_id, stage_id, stage_execution_id, agent_execution_id,
                  session_generation_id, repair_attempt, output_name, contract_id, source_kind,
                  source_generation_owner, candidate_digest, staging_path, canonical_path,
                  canonical_before_sha256, canonical_after_sha256, decision, rejection_reason,
                  materialization_state, active_pointer_generation_id, created_at, committed_at
           FROM code_writer_output_settlement_rows
           WHERE run_id = ?1
             AND materialization_state IN ('staged', 'committed', 'failed')
           ORDER BY created_at ASC, output_name ASC"#,
    )
    .bind(run_id.to_string())
    .fetch_all(pool)
    .await?;
    rows.iter().map(parse_settlement_row).collect()
}

pub async fn update_p090_settlement_row_recovery_state(
    pool: &SqlitePool,
    row_id: &str,
    materialization_state: &str,
    canonical_after_sha256: Option<&str>,
    committed_at: Option<chrono::DateTime<chrono::Utc>>,
    rejection_reason: Option<&str>,
) -> Result<()> {
    sqlx::query(
        r#"UPDATE code_writer_output_settlement_rows
           SET materialization_state = ?2,
               canonical_after_sha256 = ?3,
               committed_at = ?4,
               rejection_reason = COALESCE(?5, rejection_reason)
           WHERE id = ?1"#,
    )
    .bind(row_id)
    .bind(materialization_state)
    .bind(canonical_after_sha256)
    .bind(committed_at.map(|dt| dt.to_rfc3339()))
    .bind(rejection_reason)
    .execute(pool)
    .await?;
    Ok(())
}

fn receipt_select_sql(where_clause: &str) -> String {
    format!(
        r#"SELECT code_writer_completion_receipts.id,
                  code_writer_completion_receipts.run_id,
                  code_writer_completion_receipts.stage_execution_id,
                  code_writer_completion_receipts.agent_execution_id,
                  code_writer_completion_receipts.session_generation_id,
                  code_writer_completion_receipts.original_runtime_receipt_id,
                  code_writer_completion_receipts.completion_repair_runtime_receipt_id,
                  code_writer_completion_receipts.provider,
                  code_writer_completion_receipts.model,
                  code_writer_completion_receipts.completion_mode,
                  code_writer_completion_receipts.published_at,
                  code_writer_completion_receipts.activation_source,
                  code_writer_completion_receipts.ingestion_boundary_failure,
                  code_writer_completion_receipts.work_change_kind,
                  code_writer_completion_receipts.pre_prompt_worktree_fingerprint_path,
                  code_writer_completion_receipts.post_prompt_worktree_fingerprint_path,
                  code_writer_completion_receipts.pre_prompt_worktree_fingerprint_sha256,
                  code_writer_completion_receipts.post_prompt_worktree_fingerprint_sha256,
                  code_writer_completion_receipts.current_attempt_changed_path_count,
                  code_writer_completion_receipts.preexisting_dirty_path_count,
                  code_writer_completion_receipts.completion_status,
                  code_writer_completion_receipts.failure_class,
                  code_writer_completion_receipts.provider_runtime_family,
                  code_writer_completion_receipts.completion_boundary_subtype,
                  code_writer_completion_receipts.final_payload_status,
                  code_writer_completion_receipts.progress_before_handoff,
                  code_writer_completion_receipts.runtime_preflight_phase,
                  code_writer_completion_receipts.runtime_tool_path_preflight_json,
                  code_writer_completion_receipts.final_completion_payload_capture_json,
                  code_writer_completion_receipts.engine_failure_envelope_json,
                  code_writer_completion_receipts.repair_failure_envelope_json,
                  code_writer_completion_receipts.repair_materialization_summary_json,
                  code_writer_completion_receipts.repair_materialization_mode,
                  code_writer_completion_receipts.strict_final_payload_enabled,
                  code_writer_completion_receipts.staged_repair_settlement_enabled,
                  code_writer_completion_receipts.terminal_response_status,
                  code_writer_completion_receipts.completion_turn_attempted,
                  code_writer_completion_receipts.completion_turn_result,
                  code_writer_completion_receipts.completion_text_capture_count,
                  code_writer_completion_receipts.completion_text_absence_count,
                  code_writer_completion_receipts.completion_repair_text_status,
                  code_writer_completion_receipts.completion_repair_raw_text_artifact_path,
                  code_writer_completion_receipts.completion_repair_redacted_text_artifact_path,
                  code_writer_completion_receipts.completion_repair_text_absence_reason,
                  code_writer_completion_receipts.fresh_required_output_count,
                  code_writer_completion_receipts.stale_required_output_count,
                  code_writer_completion_receipts.missing_required_output_count,
                  code_writer_completion_receipts.control_plane_output_count,
                  code_writer_completion_receipts.completion_repair_turn_count,
                  code_writer_completion_receipts.generic_repair_turn_count,
                  code_writer_completion_receipts.missing_outputs,
                  code_writer_completion_receipts.stale_outputs,
                  code_writer_completion_receipts.transcript_status,
                  code_writer_completion_receipts.transcript_absence_reason,
                  code_writer_completion_receipts.receipt_artifact_path,
                  code_writer_completion_receipts.failed_stage_evidence_path,
                  code_writer_completion_receipts.created_at
           FROM code_writer_completion_receipts {where_clause}"#
    )
}

fn parse_receipt_row(row: &sqlx::sqlite::SqliteRow) -> Result<CodeWriterCompletionReceiptRecord> {
    let run_id: String = row.get("run_id");
    let stage_execution_id: String = row.get("stage_execution_id");
    let agent_execution_id: String = row.get("agent_execution_id");
    let created_at_raw: String = row.get("created_at");
    let published_at_raw: Option<String> = row.get("published_at");
    let missing_outputs_json: String = row.get("missing_outputs");
    let stale_outputs_json: String = row.get("stale_outputs");
    Ok(CodeWriterCompletionReceiptRecord {
        id: row.get("id"),
        run_id: run_id.parse()?,
        stage_execution_id: stage_execution_id.parse()?,
        agent_execution_id: agent_execution_id.parse()?,
        session_generation_id: row.get("session_generation_id"),
        original_runtime_receipt_id: row.get("original_runtime_receipt_id"),
        completion_repair_runtime_receipt_id: row.get("completion_repair_runtime_receipt_id"),
        provider: row.get("provider"),
        model: row.get("model"),
        completion_mode: row.get("completion_mode"),
        published_at: published_at_raw.as_deref().map(parse_time).transpose()?,
        activation_source: row.get("activation_source"),
        ingestion_boundary_failure: row.get("ingestion_boundary_failure"),
        work_change_kind: row.get("work_change_kind"),
        pre_prompt_worktree_fingerprint_path: row.get("pre_prompt_worktree_fingerprint_path"),
        post_prompt_worktree_fingerprint_path: row.get("post_prompt_worktree_fingerprint_path"),
        pre_prompt_worktree_fingerprint_sha256: row.get("pre_prompt_worktree_fingerprint_sha256"),
        post_prompt_worktree_fingerprint_sha256: row.get("post_prompt_worktree_fingerprint_sha256"),
        current_attempt_changed_path_count: row.get("current_attempt_changed_path_count"),
        preexisting_dirty_path_count: row.get("preexisting_dirty_path_count"),
        completion_status: row.get("completion_status"),
        failure_class: row.get("failure_class"),
        provider_runtime_family: row.get("provider_runtime_family"),
        completion_boundary_subtype: row.get("completion_boundary_subtype"),
        final_payload_status: row.get("final_payload_status"),
        progress_before_handoff: row.get("progress_before_handoff"),
        runtime_preflight_phase: row.get("runtime_preflight_phase"),
        runtime_tool_path_preflight_json: row.get("runtime_tool_path_preflight_json"),
        final_completion_payload_capture_json: row.get("final_completion_payload_capture_json"),
        engine_failure_envelope_json: row.get("engine_failure_envelope_json"),
        repair_failure_envelope_json: row.get("repair_failure_envelope_json"),
        repair_materialization_summary_json: row.get("repair_materialization_summary_json"),
        repair_materialization_mode: row.get("repair_materialization_mode"),
        strict_final_payload_enabled: row.get("strict_final_payload_enabled"),
        staged_repair_settlement_enabled: row.get("staged_repair_settlement_enabled"),
        terminal_response_status: row.get("terminal_response_status"),
        completion_turn_attempted: row.get("completion_turn_attempted"),
        completion_turn_result: row.get("completion_turn_result"),
        completion_text_capture_count: row.get("completion_text_capture_count"),
        completion_text_absence_count: row.get("completion_text_absence_count"),
        completion_repair_text_status: row.get("completion_repair_text_status"),
        completion_repair_raw_text_artifact_path: row
            .get("completion_repair_raw_text_artifact_path"),
        completion_repair_redacted_text_artifact_path: row
            .get("completion_repair_redacted_text_artifact_path"),
        completion_repair_text_absence_reason: row.get("completion_repair_text_absence_reason"),
        fresh_required_output_count: row.get("fresh_required_output_count"),
        stale_required_output_count: row.get("stale_required_output_count"),
        missing_required_output_count: row.get("missing_required_output_count"),
        control_plane_output_count: row.get("control_plane_output_count"),
        completion_repair_turn_count: row.get("completion_repair_turn_count"),
        generic_repair_turn_count: row.get("generic_repair_turn_count"),
        missing_outputs: parse_string_vec(&missing_outputs_json)?,
        stale_outputs: parse_string_vec(&stale_outputs_json)?,
        transcript_status: row.get("transcript_status"),
        transcript_absence_reason: row.get("transcript_absence_reason"),
        receipt_artifact_path: row.get("receipt_artifact_path"),
        failed_stage_evidence_path: row.get("failed_stage_evidence_path"),
        created_at: parse_time(&created_at_raw)?,
    })
}

fn parse_text_capture_row(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<CodeWriterCompletionTextCaptureRecord> {
    let created_at_raw: String = row.get("created_at");
    Ok(CodeWriterCompletionTextCaptureRecord {
        receipt_id: row.get("receipt_id"),
        prompt_kind: row.get("prompt_kind"),
        turn_index: row.get("turn_index"),
        terminal_response_status: row.get("terminal_response_status"),
        completion_text_status: row.get("completion_text_status"),
        completion_text_capture_source: row.get("completion_text_capture_source"),
        completion_text_raw_byte_limit: row.get("completion_text_raw_byte_limit"),
        completion_text_captured_byte_count: row.get("completion_text_captured_byte_count"),
        completion_text_truncated: row.get("completion_text_truncated"),
        extraction_input_truncated: row.get("extraction_input_truncated"),
        extraction_input_sha256: row.get("extraction_input_sha256"),
        raw_text_artifact_path: row.get("raw_text_artifact_path"),
        redacted_text_artifact_path: row.get("redacted_text_artifact_path"),
        text_absence_reason: row.get("text_absence_reason"),
        created_at: parse_time(&created_at_raw)?,
    })
}

fn parse_output_decision_row(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<CodeWriterCompletionOutputDecisionRecord> {
    Ok(CodeWriterCompletionOutputDecisionRecord {
        receipt_id: row.get("receipt_id"),
        output_name: row.get("output_name"),
        contract_id: row.get("contract_id"),
        canonical_path: row.get("canonical_path"),
        pre_prompt_sha256: row.get("pre_prompt_sha256"),
        post_prompt_sha256: row.get("post_prompt_sha256"),
        content_sha256: row.get("content_sha256"),
        settlement_source: row.get("settlement_source"),
        validation_status: row.get("validation_status"),
        rejection_reason: row.get("rejection_reason"),
    })
}

fn parse_settlement_row(row: &sqlx::sqlite::SqliteRow) -> Result<CodeWriterOutputSettlementRow> {
    let run_id: String = row.get("run_id");
    let stage_execution_id: String = row.get("stage_execution_id");
    let agent_execution_id: String = row.get("agent_execution_id");
    let created_at_raw: String = row.get("created_at");
    let committed_at_raw: Option<String> = row.get("committed_at");
    Ok(CodeWriterOutputSettlementRow {
        id: row.get("id"),
        receipt_id: row.get("receipt_id"),
        run_id: run_id.parse()?,
        stage_id: row.get("stage_id"),
        stage_execution_id: stage_execution_id.parse()?,
        agent_execution_id: agent_execution_id.parse()?,
        session_generation_id: row.get("session_generation_id"),
        repair_attempt: row.get("repair_attempt"),
        output_name: row.get("output_name"),
        contract_id: row.get("contract_id"),
        source_kind: row.get("source_kind"),
        source_generation_owner: row.get("source_generation_owner"),
        candidate_digest: row.get("candidate_digest"),
        staging_path: row.get("staging_path"),
        canonical_path: row.get("canonical_path"),
        canonical_before_sha256: row.get("canonical_before_sha256"),
        canonical_after_sha256: row.get("canonical_after_sha256"),
        decision: row.get("decision"),
        rejection_reason: row.get("rejection_reason"),
        materialization_state: row.get("materialization_state"),
        active_pointer_generation_id: row.get("active_pointer_generation_id"),
        created_at: parse_time(&created_at_raw)?,
        committed_at: committed_at_raw.as_deref().map(parse_time).transpose()?,
    })
}

fn parse_time(raw: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(raw)?.with_timezone(&Utc))
}

fn encode_string_vec(values: &[String]) -> Result<String> {
    Ok(serde_json::to_string(values)?)
}

fn parse_string_vec(raw: &str) -> Result<Vec<String>> {
    Ok(serde_json::from_str(raw)?)
}
