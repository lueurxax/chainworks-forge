use anyhow::{bail, Result};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use domain::output_contract_repair::{
    FallbackParentLink, LeaseState, OutputContractRepairEventRow, OutputContractRepairLeaseRow,
};

// ---------------------------------------------------------------------------
// Repair events
// ---------------------------------------------------------------------------

pub async fn insert_repair_event(pool: &SqlitePool, row: &OutputContractRepairEventRow) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO output_contract_repair_events (
            repair_attempt_id, schema_version,
            run_id, stage_execution_id, agent_execution_id, session_generation_id,
            role, provider_family, adapter_family, required_output_mode,
            initial_failure_class, initial_failure_subtype,
            status, presentation_category, recommended_next_action, final_output_settlement,
            same_session_repair_json, transcript_recovery_json, provider_fallback_json,
            provider_plan_evidence_json, required_outputs_json, permission_decisions_json,
            repair_budget_consumed, fallback_budget_consumed,
            repair_prompt_template_version, recovery_parser_version,
            policy_feature_flags_json, evidence_artifact_path, lease_id,
            evidence_version, projection_integrity, projection_stale_since,
            projection_schema_version, projection_rebuild_attempts,
            recorded_at, created_at, updated_at
        ) VALUES (
            ?, ?,
            ?, ?, ?, ?,
            ?, ?, ?, ?,
            ?, ?,
            ?, ?, ?, ?,
            ?, ?, ?,
            ?, ?, ?,
            ?, ?,
            ?, ?,
            ?, ?, ?,
            ?, ?, ?,
            ?, ?,
            ?, ?, ?
        )
        "#,
    )
    .bind(&row.repair_attempt_id)
    .bind(&row.schema_version)
    .bind(&row.run_id)
    .bind(&row.stage_execution_id)
    .bind(&row.agent_execution_id)
    .bind(&row.session_generation_id)
    .bind(&row.role)
    .bind(&row.provider_family)
    .bind(&row.adapter_family)
    .bind(&row.required_output_mode)
    .bind(&row.initial_failure_class)
    .bind(&row.initial_failure_subtype)
    .bind(&row.status)
    .bind(&row.presentation_category)
    .bind(&row.recommended_next_action)
    .bind(&row.final_output_settlement)
    .bind(&row.same_session_repair_json)
    .bind(&row.transcript_recovery_json)
    .bind(&row.provider_fallback_json)
    .bind(&row.provider_plan_evidence_json)
    .bind(&row.required_outputs_json)
    .bind(&row.permission_decisions_json)
    .bind(row.repair_budget_consumed as i64)
    .bind(row.fallback_budget_consumed as i64)
    .bind(&row.repair_prompt_template_version)
    .bind(&row.recovery_parser_version)
    .bind(&row.policy_feature_flags_json)
    .bind(&row.evidence_artifact_path)
    .bind(&row.lease_id)
    .bind(row.evidence_version)
    .bind(&row.projection_integrity)
    .bind(&row.projection_stale_since)
    .bind(&row.projection_schema_version)
    .bind(row.projection_rebuild_attempts)
    .bind(&row.recorded_at)
    .bind(&row.created_at)
    .bind(&row.updated_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_repair_event_status(
    pool: &SqlitePool,
    repair_attempt_id: &str,
    status: &str,
    presentation_category: &str,
    recommended_next_action: &str,
    final_output_settlement: Option<&str>,
    updated_at: &str,
) -> Result<()> {
    let rows_affected = sqlx::query(
        r#"
        UPDATE output_contract_repair_events
        SET status = ?,
            presentation_category = ?,
            recommended_next_action = ?,
            final_output_settlement = ?,
            evidence_version = evidence_version + 1,
            updated_at = ?
        WHERE repair_attempt_id = ?
        "#,
    )
    .bind(status)
    .bind(presentation_category)
    .bind(recommended_next_action)
    .bind(final_output_settlement)
    .bind(updated_at)
    .bind(repair_attempt_id)
    .execute(pool)
    .await?
    .rows_affected();

    if rows_affected == 0 {
        bail!("output_contract_repair_events: no row found for repair_attempt_id={repair_attempt_id}");
    }
    Ok(())
}

pub async fn update_repair_event_subobjects(
    pool: &SqlitePool,
    repair_attempt_id: &str,
    same_session_repair_json: Option<&str>,
    transcript_recovery_json: Option<&str>,
    provider_fallback_json: Option<&str>,
    permission_decisions_json: Option<&str>,
    repair_budget_consumed: bool,
    fallback_budget_consumed: bool,
    updated_at: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE output_contract_repair_events
        SET same_session_repair_json = COALESCE(?, same_session_repair_json),
            transcript_recovery_json = COALESCE(?, transcript_recovery_json),
            provider_fallback_json = COALESCE(?, provider_fallback_json),
            permission_decisions_json = COALESCE(?, permission_decisions_json),
            repair_budget_consumed = MAX(repair_budget_consumed, ?),
            fallback_budget_consumed = MAX(fallback_budget_consumed, ?),
            evidence_version = evidence_version + 1,
            updated_at = ?
        WHERE repair_attempt_id = ?
        "#,
    )
    .bind(same_session_repair_json)
    .bind(transcript_recovery_json)
    .bind(provider_fallback_json)
    .bind(permission_decisions_json)
    .bind(repair_budget_consumed as i64)
    .bind(fallback_budget_consumed as i64)
    .bind(updated_at)
    .bind(repair_attempt_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// P079 MISSING-004: update the plan-evidence JSON on an existing evidence row.
/// Called after plan files have been collected and redacted.
pub async fn update_plan_evidence(
    pool: &SqlitePool,
    repair_attempt_id: &str,
    provider_plan_evidence_json: &str,
    updated_at: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE output_contract_repair_events
        SET provider_plan_evidence_json = ?,
            evidence_version = evidence_version + 1,
            updated_at = ?
        WHERE repair_attempt_id = ?
        "#,
    )
    .bind(provider_plan_evidence_json)
    .bind(updated_at)
    .bind(repair_attempt_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_projection_stale(
    pool: &SqlitePool,
    repair_attempt_id: &str,
    stale_since: &str,
    updated_at: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE output_contract_repair_events
        SET projection_integrity = 'stale',
            projection_stale_since = ?,
            projection_rebuild_attempts = projection_rebuild_attempts + 1,
            updated_at = ?
        WHERE repair_attempt_id = ? AND projection_integrity = 'fresh'
        "#,
    )
    .bind(stale_since)
    .bind(updated_at)
    .bind(repair_attempt_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_projection_fresh(pool: &SqlitePool, repair_attempt_id: &str, updated_at: &str) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE output_contract_repair_events
        SET projection_integrity = 'fresh',
            projection_stale_since = NULL,
            updated_at = ?
        WHERE repair_attempt_id = ?
        "#,
    )
    .bind(updated_at)
    .bind(repair_attempt_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Promote the top-level initial_failure_subtype for unsafe_continuation and similar
/// terminal cases where the subtype is known only after the repair turn completes.
pub async fn update_initial_failure_subtype(
    pool: &SqlitePool,
    repair_attempt_id: &str,
    initial_failure_subtype: &str,
    updated_at: &str,
) -> Result<()> {
    let rows_affected = sqlx::query(
        r#"
        UPDATE output_contract_repair_events
        SET initial_failure_subtype = ?,
            evidence_version = evidence_version + 1,
            updated_at = ?
        WHERE repair_attempt_id = ?
        "#,
    )
    .bind(initial_failure_subtype)
    .bind(updated_at)
    .bind(repair_attempt_id)
    .execute(pool)
    .await?
    .rows_affected();

    if rows_affected == 0 {
        bail!("update_initial_failure_subtype: no row found for repair_attempt_id={repair_attempt_id}");
    }
    Ok(())
}

pub async fn mark_projection_permanently_stale(
    pool: &SqlitePool,
    repair_attempt_id: &str,
    updated_at: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE output_contract_repair_events
        SET projection_integrity = 'permanently_stale',
            recommended_next_action = 'manual_investigation',
            evidence_version = evidence_version + 1,
            updated_at = ?
        WHERE repair_attempt_id = ? AND projection_integrity = 'stale'
        "#,
    )
    .bind(updated_at)
    .bind(repair_attempt_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_repair_event_by_agent_execution_id(
    pool: &SqlitePool,
    agent_execution_id: &str,
) -> Result<Option<OutputContractRepairEventRow>> {
    let row = sqlx::query(
        r#"
        SELECT repair_attempt_id, schema_version,
               run_id, stage_execution_id, agent_execution_id, session_generation_id,
               role, provider_family, adapter_family, required_output_mode,
               initial_failure_class, initial_failure_subtype,
               status, presentation_category, recommended_next_action, final_output_settlement,
               same_session_repair_json, transcript_recovery_json, provider_fallback_json,
               provider_plan_evidence_json, required_outputs_json, permission_decisions_json,
               repair_budget_consumed, fallback_budget_consumed,
               repair_prompt_template_version, recovery_parser_version,
               policy_feature_flags_json, evidence_artifact_path, lease_id,
               evidence_version, projection_integrity, projection_stale_since,
               projection_schema_version, projection_rebuild_attempts,
               recorded_at, created_at, updated_at
        FROM output_contract_repair_events
        WHERE agent_execution_id = ?
        "#,
    )
    .bind(agent_execution_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(map_event_row))
}

pub async fn get_repair_event_by_repair_attempt_id(
    pool: &SqlitePool,
    repair_attempt_id: &str,
) -> Result<Option<OutputContractRepairEventRow>> {
    let row = sqlx::query(
        r#"
        SELECT repair_attempt_id, schema_version,
               run_id, stage_execution_id, agent_execution_id, session_generation_id,
               role, provider_family, adapter_family, required_output_mode,
               initial_failure_class, initial_failure_subtype,
               status, presentation_category, recommended_next_action, final_output_settlement,
               same_session_repair_json, transcript_recovery_json, provider_fallback_json,
               provider_plan_evidence_json, required_outputs_json, permission_decisions_json,
               repair_budget_consumed, fallback_budget_consumed,
               repair_prompt_template_version, recovery_parser_version,
               policy_feature_flags_json, evidence_artifact_path, lease_id,
               evidence_version, projection_integrity, projection_stale_since,
               projection_schema_version, projection_rebuild_attempts,
               recorded_at, created_at, updated_at
        FROM output_contract_repair_events
        WHERE repair_attempt_id = ?
        "#,
    )
    .bind(repair_attempt_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(map_event_row))
}

pub async fn list_repair_events_for_run(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Vec<OutputContractRepairEventRow>> {
    let rows = sqlx::query(
        r#"
        SELECT repair_attempt_id, schema_version,
               run_id, stage_execution_id, agent_execution_id, session_generation_id,
               role, provider_family, adapter_family, required_output_mode,
               initial_failure_class, initial_failure_subtype,
               status, presentation_category, recommended_next_action, final_output_settlement,
               same_session_repair_json, transcript_recovery_json, provider_fallback_json,
               provider_plan_evidence_json, required_outputs_json, permission_decisions_json,
               repair_budget_consumed, fallback_budget_consumed,
               repair_prompt_template_version, recovery_parser_version,
               policy_feature_flags_json, evidence_artifact_path, lease_id,
               evidence_version, projection_integrity, projection_stale_since,
               projection_schema_version, projection_rebuild_attempts,
               recorded_at, created_at, updated_at
        FROM output_contract_repair_events
        WHERE run_id = ?
        ORDER BY created_at ASC
        "#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(map_event_row).collect())
}

fn map_event_row(row: sqlx::sqlite::SqliteRow) -> OutputContractRepairEventRow {
    OutputContractRepairEventRow {
        repair_attempt_id: row.get("repair_attempt_id"),
        schema_version: row.get("schema_version"),
        run_id: row.get("run_id"),
        stage_execution_id: row.get("stage_execution_id"),
        agent_execution_id: row.get("agent_execution_id"),
        session_generation_id: row.get("session_generation_id"),
        role: row.get("role"),
        provider_family: row.get("provider_family"),
        adapter_family: row.get("adapter_family"),
        required_output_mode: row.get("required_output_mode"),
        initial_failure_class: row.get("initial_failure_class"),
        initial_failure_subtype: row.get("initial_failure_subtype"),
        status: row.get("status"),
        presentation_category: row.get("presentation_category"),
        recommended_next_action: row.get("recommended_next_action"),
        final_output_settlement: row.get("final_output_settlement"),
        same_session_repair_json: row.get("same_session_repair_json"),
        transcript_recovery_json: row.get("transcript_recovery_json"),
        provider_fallback_json: row.get("provider_fallback_json"),
        provider_plan_evidence_json: row.get("provider_plan_evidence_json"),
        required_outputs_json: row.get("required_outputs_json"),
        permission_decisions_json: row.get("permission_decisions_json"),
        repair_budget_consumed: row.get::<i64, _>("repair_budget_consumed") != 0,
        fallback_budget_consumed: row.get::<i64, _>("fallback_budget_consumed") != 0,
        repair_prompt_template_version: row.get("repair_prompt_template_version"),
        recovery_parser_version: row.get("recovery_parser_version"),
        policy_feature_flags_json: row.get("policy_feature_flags_json"),
        evidence_artifact_path: row.get("evidence_artifact_path"),
        lease_id: row.get("lease_id"),
        evidence_version: row.get::<i64, _>("evidence_version"),
        projection_integrity: row.get("projection_integrity"),
        projection_stale_since: row.get("projection_stale_since"),
        projection_schema_version: row.get("projection_schema_version"),
        projection_rebuild_attempts: row.get::<i64, _>("projection_rebuild_attempts"),
        recorded_at: row.get("recorded_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

// ---------------------------------------------------------------------------
// Leases
// ---------------------------------------------------------------------------

/// Insert a new lease in 'reserved' state.
/// MUST be called before any ACP prompt dispatch (durable-commit invariant REL-r2-1).
pub async fn insert_lease(pool: &SqlitePool, row: &OutputContractRepairLeaseRow) -> Result<()> {
    if row.lease_state.to_string() != "reserved" {
        bail!(
            "insert_lease: initial lease state must be 'reserved', got '{}'",
            row.lease_state
        );
    }
    sqlx::query(
        r#"
        INSERT INTO output_contract_repair_leases (
            lease_key, schema_version, repair_event_id, run_id, stage_execution_id,
            parent_agent_execution_id,
            lease_kind, lease_state, settled_result, reclamation_reason,
            frozen_fallback_policy_hash, idempotency_token,
            lease_owner_principal_id, lease_acquired_at, lease_expires_at, lease_seconds,
            dispatch_committed_at, version, infra_retry_count, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&row.lease_key)
    .bind(&row.schema_version)
    .bind(&row.repair_event_id)
    .bind(&row.run_id)
    .bind(&row.stage_execution_id)
    .bind(&row.parent_agent_execution_id)
    .bind(row.lease_kind.to_string())
    .bind("reserved")
    .bind(&row.settled_result)
    .bind(&row.reclamation_reason)
    .bind(&row.frozen_fallback_policy_hash)
    .bind(&row.idempotency_token)
    .bind(&row.lease_owner_principal_id)
    .bind(&row.lease_acquired_at)
    .bind(&row.lease_expires_at)
    .bind(row.lease_seconds)
    .bind(&row.dispatch_committed_at)
    .bind(row.version)
    .bind(row.infra_retry_count)
    .bind(&row.created_at)
    .bind(&row.updated_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Insert repair event and its initial reserved lease atomically.
/// Prevents crash-window orphan: without a transaction a crash between the two
/// separate inserts would leave an `in_progress` event row with no lease for the
/// sweep to reclaim.
pub async fn insert_repair_event_and_lease(
    pool: &SqlitePool,
    event: &OutputContractRepairEventRow,
    lease: &OutputContractRepairLeaseRow,
) -> Result<()> {
    if lease.lease_state.to_string() != "reserved" {
        bail!(
            "insert_repair_event_and_lease: initial lease state must be 'reserved', got '{}'",
            lease.lease_state
        );
    }
    let mut tx = pool.begin().await?;
    sqlx::query(
        r#"
        INSERT INTO output_contract_repair_events (
            repair_attempt_id, schema_version,
            run_id, stage_execution_id, agent_execution_id, session_generation_id,
            role, provider_family, adapter_family, required_output_mode,
            initial_failure_class, initial_failure_subtype,
            status, presentation_category, recommended_next_action, final_output_settlement,
            same_session_repair_json, transcript_recovery_json, provider_fallback_json,
            provider_plan_evidence_json, required_outputs_json, permission_decisions_json,
            repair_budget_consumed, fallback_budget_consumed,
            repair_prompt_template_version, recovery_parser_version,
            policy_feature_flags_json, evidence_artifact_path, lease_id,
            evidence_version, projection_integrity, projection_stale_since,
            projection_schema_version, projection_rebuild_attempts,
            recorded_at, created_at, updated_at
        ) VALUES (
            ?, ?,
            ?, ?, ?, ?,
            ?, ?, ?, ?,
            ?, ?,
            ?, ?, ?, ?,
            ?, ?, ?,
            ?, ?, ?,
            ?, ?,
            ?, ?,
            ?, ?, ?,
            ?, ?, ?,
            ?, ?,
            ?, ?, ?
        )
        "#,
    )
    .bind(&event.repair_attempt_id)
    .bind(&event.schema_version)
    .bind(&event.run_id)
    .bind(&event.stage_execution_id)
    .bind(&event.agent_execution_id)
    .bind(&event.session_generation_id)
    .bind(&event.role)
    .bind(&event.provider_family)
    .bind(&event.adapter_family)
    .bind(&event.required_output_mode)
    .bind(&event.initial_failure_class)
    .bind(&event.initial_failure_subtype)
    .bind(&event.status)
    .bind(&event.presentation_category)
    .bind(&event.recommended_next_action)
    .bind(&event.final_output_settlement)
    .bind(&event.same_session_repair_json)
    .bind(&event.transcript_recovery_json)
    .bind(&event.provider_fallback_json)
    .bind(&event.provider_plan_evidence_json)
    .bind(&event.required_outputs_json)
    .bind(&event.permission_decisions_json)
    .bind(event.repair_budget_consumed as i64)
    .bind(event.fallback_budget_consumed as i64)
    .bind(&event.repair_prompt_template_version)
    .bind(&event.recovery_parser_version)
    .bind(&event.policy_feature_flags_json)
    .bind(&event.evidence_artifact_path)
    .bind(&event.lease_id)
    .bind(event.evidence_version)
    .bind(&event.projection_integrity)
    .bind(&event.projection_stale_since)
    .bind(&event.projection_schema_version)
    .bind(event.projection_rebuild_attempts)
    .bind(&event.recorded_at)
    .bind(&event.created_at)
    .bind(&event.updated_at)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO output_contract_repair_leases (
            lease_key, schema_version, repair_event_id, run_id, stage_execution_id,
            parent_agent_execution_id,
            lease_kind, lease_state, settled_result, reclamation_reason,
            frozen_fallback_policy_hash, idempotency_token,
            lease_owner_principal_id, lease_acquired_at, lease_expires_at, lease_seconds,
            dispatch_committed_at, version, infra_retry_count, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&lease.lease_key)
    .bind(&lease.schema_version)
    .bind(&lease.repair_event_id)
    .bind(&lease.run_id)
    .bind(&lease.stage_execution_id)
    .bind(&lease.parent_agent_execution_id)
    .bind(lease.lease_kind.to_string())
    .bind("reserved")
    .bind(&lease.settled_result)
    .bind(&lease.reclamation_reason)
    .bind(&lease.frozen_fallback_policy_hash)
    .bind(&lease.idempotency_token)
    .bind(&lease.lease_owner_principal_id)
    .bind(&lease.lease_acquired_at)
    .bind(&lease.lease_expires_at)
    .bind(lease.lease_seconds)
    .bind(&lease.dispatch_committed_at)
    .bind(lease.version)
    .bind(lease.infra_retry_count)
    .bind(&lease.created_at)
    .bind(&lease.updated_at)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Transition lease from 'reserved' -> 'prompt_sent' and set dispatch_committed_at.
/// This MUST complete before ACP bytes are transmitted (REL-r2-1).
pub async fn transition_lease_to_prompt_sent(
    pool: &SqlitePool,
    lease_key: &str,
    dispatch_committed_at: &str,
    updated_at: &str,
) -> Result<()> {
    let rows_affected = sqlx::query(
        r#"
        UPDATE output_contract_repair_leases
        SET lease_state = 'prompt_sent',
            dispatch_committed_at = ?,
            version = version + 1,
            updated_at = ?
        WHERE lease_key = ? AND lease_state = 'reserved'
        "#,
    )
    .bind(dispatch_committed_at)
    .bind(updated_at)
    .bind(lease_key)
    .execute(pool)
    .await?
    .rows_affected();

    if rows_affected == 0 {
        bail!(
            "transition_lease_to_prompt_sent: lease_key={lease_key} not found in 'reserved' state"
        );
    }
    Ok(())
}

/// Settle a lease terminally. Called in the same transaction as final output settlement.
/// The `dispatch_committed_at_override` parameter is used to satisfy the CHECK constraint
/// `dispatch_committed_at_set_on_send` for leases that transition directly from `reserved`
/// to `settled` (e.g. TTL sweep reclamation of a never-dispatched lease).  Pass the
/// settlement timestamp in that case; for `prompt_sent` leases the existing
/// `dispatch_committed_at` is already set and the COALESCE keeps it unchanged.
pub async fn settle_lease_tx(
    tx: &mut Transaction<'_, Sqlite>,
    lease_key: &str,
    settled_result: &str,
    reclamation_reason: Option<&str>,
    updated_at: &str,
) -> Result<()> {
    let rows_affected = sqlx::query(
        r#"
        UPDATE output_contract_repair_leases
        SET lease_state = 'settled',
            settled_result = ?,
            reclamation_reason = ?,
            -- Satisfy dispatch_committed_at_set_on_send CHECK for leases settled directly
            -- from 'reserved' (e.g. TTL reclamation before prompt was sent).
            -- For 'prompt_sent' leases dispatch_committed_at is already set; COALESCE
            -- preserves it. For 'reserved' leases we record the finalization time.
            dispatch_committed_at = COALESCE(dispatch_committed_at, ?),
            version = version + 1,
            updated_at = ?
        WHERE lease_key = ? AND lease_state != 'settled'
        "#,
    )
    .bind(settled_result)
    .bind(reclamation_reason)
    .bind(updated_at)
    .bind(updated_at)
    .bind(lease_key)
    .execute(&mut **tx)
    .await?
    .rows_affected();

    if rows_affected == 0 {
        bail!("settle_lease_tx: lease_key={lease_key} not found or already settled");
    }
    Ok(())
}

/// Atomically settle terminal status on the repair evidence row AND transition the lease
/// to settled in a single SQLite transaction (DEFECT-004 fix).
/// This is the correct path for terminal arms where the repair result is known.
pub async fn settle_terminal_event_and_lease(
    pool: &SqlitePool,
    repair_attempt_id: &str,
    lease_key: &str,
    status: &str,
    presentation_category: &str,
    recommended_next_action: &str,
    final_output_settlement: Option<&str>,
    lease_result: &str,
    updated_at: &str,
) -> Result<()> {
    let mut tx = pool.begin().await?;

    // 1. Update repair event to terminal status.
    let rows = sqlx::query(
        r#"
        UPDATE output_contract_repair_events
        SET status = ?,
            presentation_category = ?,
            recommended_next_action = ?,
            final_output_settlement = ?,
            evidence_version = evidence_version + 1,
            updated_at = ?
        WHERE repair_attempt_id = ?
        "#,
    )
    .bind(status)
    .bind(presentation_category)
    .bind(recommended_next_action)
    .bind(final_output_settlement)
    .bind(updated_at)
    .bind(repair_attempt_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    if rows == 0 {
        tx.rollback().await.ok();
        bail!(
            "settle_terminal_event_and_lease: no event row for repair_attempt_id={repair_attempt_id}"
        );
    }

    // 2. Settle the lease in the same transaction.
    settle_lease_tx(&mut tx, lease_key, lease_result, None, updated_at).await?;

    tx.commit().await?;
    Ok(())
}

/// Best-effort (non-transactional) lease settlement for use in repair result arms.
/// For atomic settlement with output truth, prefer `settle_terminal_event_and_lease`.
pub async fn settle_lease_pool(
    pool: &SqlitePool,
    lease_key: &str,
    settled_result: &str,
    reclamation_reason: Option<&str>,
    updated_at: &str,
) -> Result<()> {
    let rows_affected = sqlx::query(
        r#"
        UPDATE output_contract_repair_leases
        SET lease_state = 'settled',
            settled_result = ?,
            reclamation_reason = ?,
            dispatch_committed_at = COALESCE(dispatch_committed_at, ?),
            version = version + 1,
            updated_at = ?
        WHERE lease_key = ? AND lease_state != 'settled'
        "#,
    )
    .bind(settled_result)
    .bind(reclamation_reason)
    .bind(updated_at)
    .bind(updated_at)
    .bind(lease_key)
    .execute(pool)
    .await?
    .rows_affected();

    if rows_affected == 0 {
        bail!("settle_lease_pool: lease_key={lease_key} not found or already settled");
    }
    Ok(())
}

pub async fn get_lease_by_key(
    pool: &SqlitePool,
    lease_key: &str,
) -> Result<Option<OutputContractRepairLeaseRow>> {
    let row = sqlx::query(
        r#"
        SELECT lease_key, schema_version, repair_event_id, run_id, stage_execution_id,
               parent_agent_execution_id,
               lease_kind, lease_state, settled_result, reclamation_reason,
               frozen_fallback_policy_hash, idempotency_token,
               lease_owner_principal_id, lease_acquired_at, lease_expires_at, lease_seconds,
               dispatch_committed_at, version, infra_retry_count, created_at, updated_at
        FROM output_contract_repair_leases
        WHERE lease_key = ?
        "#,
    )
    .bind(lease_key)
    .fetch_optional(pool)
    .await?;

    row.map(map_lease_row).transpose()
}

/// Returns all leases for a given run. Used by the run-report readback to resolve
/// lease objects keyed by lease_key for each repair event.
pub async fn list_leases_for_run(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Vec<OutputContractRepairLeaseRow>> {
    let rows = sqlx::query(
        r#"
        SELECT lease_key, schema_version, repair_event_id, run_id, stage_execution_id,
               parent_agent_execution_id,
               lease_kind, lease_state, settled_result, reclamation_reason,
               frozen_fallback_policy_hash, idempotency_token,
               lease_owner_principal_id, lease_acquired_at, lease_expires_at, lease_seconds,
               dispatch_committed_at, version, infra_retry_count, created_at, updated_at
        FROM output_contract_repair_leases
        WHERE run_id = ?
        ORDER BY created_at ASC
        "#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
        result.push(map_lease_row(row)?);
    }
    Ok(result)
}

/// Returns all active (non-settled) leases with expires_at in the past.
/// Used by the reconciliation sweep (REL-r2-2).
pub async fn list_expired_active_leases(
    pool: &SqlitePool,
    now_iso: &str,
) -> Result<Vec<OutputContractRepairLeaseRow>> {
    let rows = sqlx::query(
        r#"
        SELECT lease_key, schema_version, repair_event_id, run_id, stage_execution_id,
               parent_agent_execution_id,
               lease_kind, lease_state, settled_result, reclamation_reason,
               frozen_fallback_policy_hash, idempotency_token,
               lease_owner_principal_id, lease_acquired_at, lease_expires_at, lease_seconds,
               dispatch_committed_at, version, infra_retry_count, created_at, updated_at
        FROM output_contract_repair_leases
        WHERE lease_state != 'settled' AND lease_expires_at < ?
        ORDER BY lease_expires_at ASC
        "#,
    )
    .bind(now_iso)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
        result.push(map_lease_row(row)?);
    }
    Ok(result)
}

fn map_lease_row(row: sqlx::sqlite::SqliteRow) -> Result<OutputContractRepairLeaseRow> {
    let lease_kind_str: String = row.get("lease_kind");
    let lease_state_str: String = row.get("lease_state");
    let lease_state = lease_state_str
        .parse::<LeaseState>()
        .map_err(|e| anyhow::anyhow!("invalid lease_state in DB: {e}"))?;
    Ok(OutputContractRepairLeaseRow {
        lease_key: row.get("lease_key"),
        schema_version: row.get("schema_version"),
        repair_event_id: row.get("repair_event_id"),
        run_id: row.get("run_id"),
        stage_execution_id: row.get("stage_execution_id"),
        parent_agent_execution_id: row.get("parent_agent_execution_id"),
        lease_kind: match lease_kind_str.as_str() {
            "fallback" => domain::output_contract_repair::LeaseKind::Fallback,
            _ => domain::output_contract_repair::LeaseKind::Repair,
        },
        lease_state,
        settled_result: row.get("settled_result"),
        reclamation_reason: row.get("reclamation_reason"),
        frozen_fallback_policy_hash: row.get("frozen_fallback_policy_hash"),
        idempotency_token: row.get("idempotency_token"),
        lease_owner_principal_id: row.get("lease_owner_principal_id"),
        lease_acquired_at: row.get("lease_acquired_at"),
        lease_expires_at: row.get("lease_expires_at"),
        lease_seconds: row.get("lease_seconds"),
        dispatch_committed_at: row.get("dispatch_committed_at"),
        version: row.get::<i64, _>("version"),
        infra_retry_count: row.get("infra_retry_count"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

// ---------------------------------------------------------------------------
// Fallback parent links
// ---------------------------------------------------------------------------

pub async fn insert_fallback_parent_link(
    pool: &SqlitePool,
    link: &FallbackParentLink,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO output_contract_repair_fallback_parent_links (
            fallback_agent_execution_id, parent_failed_agent_execution_id,
            repair_event_id, lease_key,
            fallback_packet_hash, fallback_principal_id, fallback_principal_capability_hash,
            fallback_result, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&link.fallback_agent_execution_id)
    .bind(&link.parent_failed_agent_execution_id)
    .bind(&link.repair_event_id)
    .bind(&link.lease_key)
    .bind(&link.fallback_packet_hash)
    .bind(&link.fallback_principal_id)
    .bind(&link.fallback_principal_capability_hash)
    .bind(&link.fallback_result)
    .bind(&link.created_at)
    .bind(&link.updated_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_fallback_link_by_parent(
    pool: &SqlitePool,
    parent_failed_agent_execution_id: &str,
) -> Result<Option<FallbackParentLink>> {
    let row = sqlx::query(
        r#"
        SELECT fallback_agent_execution_id, parent_failed_agent_execution_id,
               repair_event_id, lease_key,
               fallback_packet_hash, fallback_principal_id, fallback_principal_capability_hash,
               fallback_result, created_at, updated_at
        FROM output_contract_repair_fallback_parent_links
        WHERE parent_failed_agent_execution_id = ?
        "#,
    )
    .bind(parent_failed_agent_execution_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(map_fallback_link_row))
}

pub async fn get_fallback_link_by_fallback_execution(
    pool: &SqlitePool,
    fallback_agent_execution_id: &str,
) -> Result<Option<FallbackParentLink>> {
    let row = sqlx::query(
        r#"
        SELECT fallback_agent_execution_id, parent_failed_agent_execution_id,
               repair_event_id, lease_key,
               fallback_packet_hash, fallback_principal_id, fallback_principal_capability_hash,
               fallback_result, created_at, updated_at
        FROM output_contract_repair_fallback_parent_links
        WHERE fallback_agent_execution_id = ?
        "#,
    )
    .bind(fallback_agent_execution_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(map_fallback_link_row))
}

pub async fn update_fallback_link_result(
    pool: &SqlitePool,
    parent_failed_agent_execution_id: &str,
    fallback_result: &str,
    updated_at: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE output_contract_repair_fallback_parent_links
        SET fallback_result = ?, updated_at = ?
        WHERE parent_failed_agent_execution_id = ?
        "#,
    )
    .bind(fallback_result)
    .bind(updated_at)
    .bind(parent_failed_agent_execution_id)
    .execute(pool)
    .await?;
    Ok(())
}

fn map_fallback_link_row(row: sqlx::sqlite::SqliteRow) -> FallbackParentLink {
    FallbackParentLink {
        fallback_agent_execution_id: row.get("fallback_agent_execution_id"),
        parent_failed_agent_execution_id: row.get("parent_failed_agent_execution_id"),
        repair_event_id: row.get("repair_event_id"),
        lease_key: row.get("lease_key"),
        fallback_packet_hash: row.get("fallback_packet_hash"),
        fallback_principal_id: row.get("fallback_principal_id"),
        fallback_principal_capability_hash: row.get("fallback_principal_capability_hash"),
        fallback_result: row.get("fallback_result"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

// ---------------------------------------------------------------------------
// Lease TTL sweep reclamation (REL-r2-2)
// ---------------------------------------------------------------------------

/// Atomically reclaim an expired lease and settle the associated repair event.
///
/// Called by the reconciliation sweep when a lease's `lease_expires_at` is in
/// the past.  Budget rules per proposal:
///  - `reserved` → `unavailable`, budget NOT consumed, reason `ttl_expired_reserved`
///  - `prompt_sent` → `failed_transport`, budget IS consumed, reason `ttl_expired_prompt_sent`
///
/// `repair_budget_consumed` and `fallback_budget_consumed` must be set from the
/// lease kind: a `repair` lease in `prompt_sent` state sets only `repair_budget_consumed`,
/// and a `fallback` lease sets only `fallback_budget_consumed`.  Both are false for
/// `reserved` leases since budget is not consumed until a prompt has been dispatched.
///
/// Both the event status update and the lease settlement are committed in the
/// same SQLite transaction so there is no observable intermediate state.
pub async fn reclaim_expired_lease_and_event(
    pool: &SqlitePool,
    lease_key: &str,
    repair_event_id: &str,
    event_status: &str,
    event_presentation_category: &str,
    event_recommended_next_action: &str,
    reclamation_reason: &str,
    repair_budget_consumed: bool,
    fallback_budget_consumed: bool,
    updated_at: &str,
) -> Result<()> {
    let lease_result = match reclamation_reason {
        "ttl_expired_reserved" => "unavailable",
        "ttl_expired_prompt_sent" => "failed_transport",
        "cancellation" => "cancelled",
        "supersession" => "superseded_ignored",
        "principal_revoked" => "unavailable",
        other => bail!("reclaim_expired_lease_and_event: unknown reclamation_reason={other}"),
    };

    let mut tx = pool.begin().await?;

    // 1. Settle the event row.  Update each budget flag independently so a
    //    reclaimed repair lease never contaminates fallback_budget_consumed and
    //    vice versa (REL-r2-2 budget correctness requirement).
    let event_rows = sqlx::query(
        r#"
        UPDATE output_contract_repair_events
        SET status = ?,
            presentation_category = ?,
            recommended_next_action = ?,
            repair_budget_consumed = MAX(repair_budget_consumed, ?),
            fallback_budget_consumed = MAX(fallback_budget_consumed, ?),
            evidence_version = evidence_version + 1,
            updated_at = ?
        WHERE repair_attempt_id = ? AND status NOT IN ('recovered','blocked','skipped','cancelled','failed')
        "#,
    )
    .bind(event_status)
    .bind(event_presentation_category)
    .bind(event_recommended_next_action)
    .bind(repair_budget_consumed as i64)
    .bind(fallback_budget_consumed as i64)
    .bind(updated_at)
    .bind(repair_event_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    if event_rows == 0 {
        tx.rollback().await.ok();
        bail!(
            "reclaim_expired_lease_and_event: event {repair_event_id} not found or already terminal"
        );
    }

    // 2. Settle the lease with reclamation_reason.
    settle_lease_tx(&mut tx, lease_key, lease_result, Some(reclamation_reason), updated_at).await?;

    tx.commit().await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn make_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .connect(":memory:")
            .await
            .expect("in-memory pool");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrations");
        // Seed prerequisite rows to satisfy FK constraints.
        sqlx::query(
            "INSERT INTO ideas (id, title, body, status, created_at) \
             VALUES ('idea-test', 'Test Idea', 'Test body', 'active', '2026-05-30T10:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("seed test idea");
        sqlx::query(
            "INSERT INTO runs \
             (id, idea_id, status, workflow_id, workflow_title, workspace_root, artifact_root, started_at) \
             VALUES (?, 'idea-test', 'running', 'wf-test', 'Test Workflow', '/tmp', '/tmp/artifacts', '2026-05-30T10:00:00Z')",
        )
        .bind("run-test-001")
        .execute(&pool)
        .await
        .expect("seed test run");
        pool
    }

    fn sample_event_row(agent_execution_id: &str) -> OutputContractRepairEventRow {
        let now = "2026-05-30T10:00:00Z";
        OutputContractRepairEventRow {
            repair_attempt_id: format!("rep-{agent_execution_id}"),
            schema_version: "output_contract_repair.v1".into(),
            run_id: "run-test-001".into(),
            stage_execution_id: "stage-test-001".into(),
            agent_execution_id: agent_execution_id.into(),
            session_generation_id: "session-001".into(),
            role: "lead_orchestrator".into(),
            provider_family: "claude".into(),
            adapter_family: "claude".into(),
            required_output_mode: "strict_structured".into(),
            initial_failure_class: "missing_required_outputs".into(),
            initial_failure_subtype: None,
            status: "in_progress".into(),
            presentation_category: "informational".into(),
            recommended_next_action: "continue".into(),
            final_output_settlement: None,
            same_session_repair_json: None,
            transcript_recovery_json: None,
            provider_fallback_json: None,
            provider_plan_evidence_json: None,
            required_outputs_json: "[]".into(),
            permission_decisions_json: "[]".into(),
            repair_budget_consumed: false,
            fallback_budget_consumed: false,
            repair_prompt_template_version: Some("p079_repair_v1".into()),
            recovery_parser_version: Some("p079_recovery_v1".into()),
            policy_feature_flags_json: "[]".into(),
            evidence_artifact_path: None,
            lease_id: None,
            evidence_version: 1,
            projection_integrity: "fresh".into(),
            projection_stale_since: None,
            projection_schema_version: "output_contract_repair_events_v1".into(),
            projection_rebuild_attempts: 0,
            recorded_at: now.into(),
            created_at: now.into(),
            updated_at: now.into(),
        }
    }

    #[tokio::test]
    async fn proposal_079_insert_and_retrieve_repair_event() {
        let pool = make_pool().await;
        let row = sample_event_row("agent-001");
        insert_repair_event(&pool, &row).await.unwrap();

        let found = get_repair_event_by_agent_execution_id(&pool, "agent-001")
            .await
            .unwrap()
            .expect("should find event");
        assert_eq!(found.status, "in_progress");
        assert_eq!(found.projection_integrity, "fresh");
        assert_eq!(found.evidence_version, 1);
        assert_eq!(found.repair_attempt_id, "rep-agent-001");
    }

    #[tokio::test]
    async fn proposal_079_update_status_increments_version() {
        let pool = make_pool().await;
        let row = sample_event_row("agent-002");
        insert_repair_event(&pool, &row).await.unwrap();

        update_repair_event_status(
            &pool,
            "rep-agent-002",
            "recovered",
            "recovered",
            "continue",
            Some("valid_outputs_from_repair"),
            "2026-05-30T10:01:00Z",
        )
        .await
        .unwrap();

        let found = get_repair_event_by_repair_attempt_id(&pool, "rep-agent-002")
            .await
            .unwrap()
            .expect("should find event");
        assert_eq!(found.status, "recovered");
        assert_eq!(found.evidence_version, 2);
    }

    #[tokio::test]
    async fn proposal_079_list_for_run() {
        let pool = make_pool().await;
        insert_repair_event(&pool, &sample_event_row("agent-003"))
            .await
            .unwrap();
        insert_repair_event(&pool, &sample_event_row("agent-004"))
            .await
            .unwrap();

        let rows = list_repair_events_for_run(&pool, "run-test-001")
            .await
            .unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[tokio::test]
    async fn proposal_079_lease_reserved_to_prompt_sent() {
        use domain::output_contract_repair::{LeaseKind, LeaseState};
        let pool = make_pool().await;

        let event_row = sample_event_row("agent-005");
        insert_repair_event(&pool, &event_row).await.unwrap();

        let lease = OutputContractRepairLeaseRow {
            lease_key: "lk-001".into(),
            schema_version: "output_contract_repair_leases_v1".into(),
            repair_event_id: "rep-agent-005".into(),
            run_id: "run-test-001".into(),
            stage_execution_id: "stage-test-001".into(),
            parent_agent_execution_id: "agent-005".into(),
            lease_kind: LeaseKind::Repair,
            lease_state: LeaseState::Reserved,
            settled_result: None,
            reclamation_reason: None,
            frozen_fallback_policy_hash: None,
            idempotency_token: "idem-001".into(),
            lease_owner_principal_id: "principal-001".into(),
            lease_acquired_at: "2026-05-30T10:00:00Z".into(),
            lease_expires_at: "2026-05-30T10:03:00Z".into(),
            lease_seconds: 180,
            dispatch_committed_at: None,
            version: 0,
            infra_retry_count: 0,
            created_at: "2026-05-30T10:00:00Z".into(),
            updated_at: "2026-05-30T10:00:00Z".into(),
        };
        insert_lease(&pool, &lease).await.unwrap();

        let found = get_lease_by_key(&pool, "lk-001").await.unwrap().unwrap();
        assert_eq!(found.lease_state, LeaseState::Reserved);
        assert!(found.dispatch_committed_at.is_none());

        transition_lease_to_prompt_sent(
            &pool,
            "lk-001",
            "2026-05-30T10:00:01Z",
            "2026-05-30T10:00:01Z",
        )
        .await
        .unwrap();

        let found2 = get_lease_by_key(&pool, "lk-001").await.unwrap().unwrap();
        assert_eq!(found2.lease_state, LeaseState::PromptSent);
        assert_eq!(
            found2.dispatch_committed_at.as_deref(),
            Some("2026-05-30T10:00:01Z")
        );
        assert_eq!(found2.version, 1);
    }

    #[tokio::test]
    async fn proposal_079_expired_lease_enumeration() {
        use domain::output_contract_repair::{LeaseKind, LeaseState};
        let pool = make_pool().await;

        let event_row = sample_event_row("agent-006");
        insert_repair_event(&pool, &event_row).await.unwrap();

        let lease = OutputContractRepairLeaseRow {
            lease_key: "lk-002".into(),
            schema_version: "output_contract_repair_leases_v1".into(),
            repair_event_id: "rep-agent-006".into(),
            run_id: "run-test-001".into(),
            stage_execution_id: "stage-test-001".into(),
            parent_agent_execution_id: "agent-006".into(),
            lease_kind: LeaseKind::Repair,
            lease_state: LeaseState::Reserved,
            settled_result: None,
            reclamation_reason: None,
            frozen_fallback_policy_hash: None,
            idempotency_token: "idem-002".into(),
            lease_owner_principal_id: "principal-002".into(),
            lease_acquired_at: "2026-05-29T10:00:00Z".into(),
            lease_expires_at: "2026-05-29T10:03:00Z".into(),
            lease_seconds: 180,
            dispatch_committed_at: None,
            version: 0,
            infra_retry_count: 0,
            created_at: "2026-05-29T10:00:00Z".into(),
            updated_at: "2026-05-29T10:00:00Z".into(),
        };
        insert_lease(&pool, &lease).await.unwrap();

        // Now: 2026-05-30, lease expired yesterday
        let expired = list_expired_active_leases(&pool, "2026-05-30T00:00:00Z")
            .await
            .unwrap();
        assert!(expired.iter().any(|l| l.lease_key == "lk-002"));
    }

    #[tokio::test]
    async fn proposal_079_settle_lease_pool_accepted() {
        use domain::output_contract_repair::{LeaseKind, LeaseState};
        let pool = make_pool().await;

        let event_row = sample_event_row("agent-007");
        insert_repair_event(&pool, &event_row).await.unwrap();

        let lease = OutputContractRepairLeaseRow {
            lease_key: "lk-003".into(),
            schema_version: "output_contract_repair_leases_v1".into(),
            repair_event_id: "rep-agent-007".into(),
            run_id: "run-test-001".into(),
            stage_execution_id: "stage-test-001".into(),
            parent_agent_execution_id: "agent-007".into(),
            lease_kind: LeaseKind::Repair,
            lease_state: LeaseState::Reserved,
            settled_result: None,
            reclamation_reason: None,
            frozen_fallback_policy_hash: None,
            idempotency_token: "idem-003".into(),
            lease_owner_principal_id: "principal-003".into(),
            lease_acquired_at: "2026-05-30T10:00:00Z".into(),
            lease_expires_at: "2026-05-30T10:03:00Z".into(),
            lease_seconds: 180,
            dispatch_committed_at: None,
            version: 0,
            infra_retry_count: 0,
            created_at: "2026-05-30T10:00:00Z".into(),
            updated_at: "2026-05-30T10:00:00Z".into(),
        };
        insert_lease(&pool, &lease).await.unwrap();
        transition_lease_to_prompt_sent(&pool, "lk-003", "2026-05-30T10:00:01Z", "2026-05-30T10:00:01Z")
            .await
            .unwrap();
        settle_lease_pool(&pool, "lk-003", "accepted", None, "2026-05-30T10:01:00Z")
            .await
            .unwrap();

        let found = get_lease_by_key(&pool, "lk-003").await.unwrap().unwrap();
        assert_eq!(found.lease_state, LeaseState::Settled);
        assert_eq!(found.settled_result.as_deref(), Some("accepted"));
        assert_eq!(found.version, 2);

        // Second settle is idempotent-ish (rows_affected=0 -> error, but no panic).
        let second = settle_lease_pool(&pool, "lk-003", "accepted", None, "2026-05-30T10:02:00Z").await;
        assert!(second.is_err());
    }

    #[tokio::test]
    async fn proposal_079_reclaim_reserved_lease_no_budget() {
        use domain::output_contract_repair::{LeaseKind, LeaseState};
        let pool = make_pool().await;

        // Event row for the lease.
        let event_row = sample_event_row("agent-008");
        insert_repair_event(&pool, &event_row).await.unwrap();

        let lease = OutputContractRepairLeaseRow {
            lease_key: "lk-reclaim-reserved".into(),
            schema_version: "output_contract_repair_leases_v1".into(),
            repair_event_id: "rep-agent-008".into(),
            run_id: "run-test-001".into(),
            stage_execution_id: "stage-test-001".into(),
            parent_agent_execution_id: "agent-008".into(),
            lease_kind: LeaseKind::Repair,
            lease_state: LeaseState::Reserved,
            settled_result: None,
            reclamation_reason: None,
            frozen_fallback_policy_hash: None,
            idempotency_token: "idem-reclaim-001".into(),
            lease_owner_principal_id: "principal-001".into(),
            lease_acquired_at: "2026-05-29T10:00:00Z".into(),
            lease_expires_at: "2026-05-29T10:03:00Z".into(),
            lease_seconds: 180,
            dispatch_committed_at: None,
            version: 0,
            infra_retry_count: 0,
            created_at: "2026-05-29T10:00:00Z".into(),
            updated_at: "2026-05-29T10:00:00Z".into(),
        };
        insert_lease(&pool, &lease).await.unwrap();

        reclaim_expired_lease_and_event(
            &pool,
            "lk-reclaim-reserved",
            "rep-agent-008",
            "blocked",
            "blocked",
            "manual_investigation",
            "ttl_expired_reserved",
            false,
            false,
            "2026-05-30T10:00:00Z",
        )
        .await
        .unwrap();

        let found_lease = get_lease_by_key(&pool, "lk-reclaim-reserved").await.unwrap().unwrap();
        assert_eq!(found_lease.lease_state, LeaseState::Settled);
        assert_eq!(found_lease.settled_result.as_deref(), Some("unavailable"));
        assert_eq!(found_lease.reclamation_reason.as_deref(), Some("ttl_expired_reserved"));

        let found_event = get_repair_event_by_repair_attempt_id(&pool, "rep-agent-008")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found_event.status, "blocked");
        assert!(!found_event.repair_budget_consumed, "reserved lease must not consume repair budget");
        assert!(!found_event.fallback_budget_consumed, "reserved lease must not consume fallback budget");
    }

    #[tokio::test]
    async fn proposal_079_reclaim_prompt_sent_lease_consumes_budget() {
        use domain::output_contract_repair::{LeaseKind, LeaseState};
        let pool = make_pool().await;

        let event_row = sample_event_row("agent-009");
        insert_repair_event(&pool, &event_row).await.unwrap();

        let lease = OutputContractRepairLeaseRow {
            lease_key: "lk-reclaim-prompt-sent".into(),
            schema_version: "output_contract_repair_leases_v1".into(),
            repair_event_id: "rep-agent-009".into(),
            run_id: "run-test-001".into(),
            stage_execution_id: "stage-test-001".into(),
            parent_agent_execution_id: "agent-009".into(),
            lease_kind: LeaseKind::Repair,
            lease_state: LeaseState::Reserved,
            settled_result: None,
            reclamation_reason: None,
            frozen_fallback_policy_hash: None,
            idempotency_token: "idem-reclaim-002".into(),
            lease_owner_principal_id: "principal-002".into(),
            lease_acquired_at: "2026-05-29T10:00:00Z".into(),
            lease_expires_at: "2026-05-29T10:03:00Z".into(),
            lease_seconds: 180,
            dispatch_committed_at: None,
            version: 0,
            infra_retry_count: 0,
            created_at: "2026-05-29T10:00:00Z".into(),
            updated_at: "2026-05-29T10:00:00Z".into(),
        };
        insert_lease(&pool, &lease).await.unwrap();
        transition_lease_to_prompt_sent(
            &pool, "lk-reclaim-prompt-sent",
            "2026-05-29T10:00:01Z", "2026-05-29T10:00:01Z",
        )
        .await
        .unwrap();

        reclaim_expired_lease_and_event(
            &pool,
            "lk-reclaim-prompt-sent",
            "rep-agent-009",
            "failed",
            "failed",
            "manual_investigation",
            "ttl_expired_prompt_sent",
            true,
            false,
            "2026-05-30T10:00:00Z",
        )
        .await
        .unwrap();

        let found_lease = get_lease_by_key(&pool, "lk-reclaim-prompt-sent").await.unwrap().unwrap();
        assert_eq!(found_lease.lease_state, LeaseState::Settled);
        assert_eq!(found_lease.settled_result.as_deref(), Some("failed_transport"));
        assert_eq!(found_lease.reclamation_reason.as_deref(), Some("ttl_expired_prompt_sent"));

        let found_event = get_repair_event_by_repair_attempt_id(&pool, "rep-agent-009")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found_event.status, "failed");
        assert!(found_event.repair_budget_consumed, "prompt_sent repair lease must consume repair budget");
        assert!(!found_event.fallback_budget_consumed, "repair lease reclamation must not consume fallback budget");
    }

    #[tokio::test]
    async fn proposal_079_reclaim_fallback_prompt_sent_lease_consumes_fallback_budget_only() {
        // REL-r2-2 regression: a fallback lease reclaimed in prompt_sent state must set
        // fallback_budget_consumed=true and must NOT set repair_budget_consumed=true.
        use domain::output_contract_repair::{LeaseKind, LeaseState};
        let pool = make_pool().await;

        let event_row = sample_event_row("agent-010");
        insert_repair_event(&pool, &event_row).await.unwrap();

        let lease = OutputContractRepairLeaseRow {
            lease_key: "lk-fallback-prompt-sent".into(),
            schema_version: "output_contract_repair_leases_v1".into(),
            repair_event_id: "rep-agent-010".into(),
            run_id: "run-test-001".into(),
            stage_execution_id: "stage-test-001".into(),
            parent_agent_execution_id: "agent-010".into(),
            lease_kind: LeaseKind::Fallback,
            lease_state: LeaseState::Reserved,
            settled_result: None,
            reclamation_reason: None,
            frozen_fallback_policy_hash: Some("hash-abc123".into()),
            idempotency_token: "idem-fallback-001".into(),
            lease_owner_principal_id: "principal-fb-001".into(),
            lease_acquired_at: "2026-05-29T10:00:00Z".into(),
            lease_expires_at: "2026-05-29T10:03:00Z".into(),
            lease_seconds: 180,
            dispatch_committed_at: None,
            version: 0,
            infra_retry_count: 0,
            created_at: "2026-05-29T10:00:00Z".into(),
            updated_at: "2026-05-29T10:00:00Z".into(),
        };
        insert_lease(&pool, &lease).await.unwrap();
        transition_lease_to_prompt_sent(
            &pool, "lk-fallback-prompt-sent",
            "2026-05-29T10:00:01Z", "2026-05-29T10:00:01Z",
        )
        .await
        .unwrap();

        reclaim_expired_lease_and_event(
            &pool,
            "lk-fallback-prompt-sent",
            "rep-agent-010",
            "failed",
            "failed",
            "manual_investigation",
            "ttl_expired_prompt_sent",
            false,
            true,
            "2026-05-30T10:00:00Z",
        )
        .await
        .unwrap();

        let found_lease = get_lease_by_key(&pool, "lk-fallback-prompt-sent").await.unwrap().unwrap();
        assert_eq!(found_lease.lease_state, LeaseState::Settled);
        assert_eq!(found_lease.settled_result.as_deref(), Some("failed_transport"));

        let found_event = get_repair_event_by_repair_attempt_id(&pool, "rep-agent-010")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found_event.status, "failed");
        assert!(!found_event.repair_budget_consumed, "fallback lease reclamation must not consume repair budget");
        assert!(found_event.fallback_budget_consumed, "prompt_sent fallback lease must consume fallback budget");
    }

    // SEC-002 integration regression: the advisory-only fail-closed path must settle
    // with enum values that satisfy the migration CHECK constraints. This test calls
    // the real settle_terminal_event_and_lease and update_repair_event_status with
    // "manual_investigation" and "skipped_ineligible" against a migrated SQLite pool.
    // If the code uses an invalid value, SQLite will reject the UPDATE and the test fails.
    #[tokio::test]
    async fn proposal_079_advisory_failclosed_settle_satisfies_migration_check() {
        use domain::output_contract_repair::{LeaseKind, LeaseState};
        let pool = make_pool().await;

        let event_row = sample_event_row("agent-advisory-settle");
        insert_repair_event(&pool, &event_row).await.unwrap();

        let lease = OutputContractRepairLeaseRow {
            lease_key: "lk-advisory-settle".into(),
            schema_version: "output_contract_repair_leases_v1".into(),
            repair_event_id: "rep-agent-advisory-settle".into(),
            run_id: "run-test-001".into(),
            stage_execution_id: "stage-test-001".into(),
            parent_agent_execution_id: "agent-advisory-settle".into(),
            lease_kind: LeaseKind::Repair,
            lease_state: LeaseState::Reserved,
            settled_result: None,
            reclamation_reason: None,
            frozen_fallback_policy_hash: None,
            idempotency_token: "idem-advisory-settle".into(),
            lease_owner_principal_id: "principal-advisory".into(),
            lease_acquired_at: "2026-06-09T10:00:00Z".into(),
            lease_expires_at: "2026-06-09T10:03:00Z".into(),
            lease_seconds: 180,
            dispatch_committed_at: None,
            version: 0,
            infra_retry_count: 0,
            created_at: "2026-06-09T10:00:00Z".into(),
            updated_at: "2026-06-09T10:00:00Z".into(),
        };
        insert_lease(&pool, &lease).await.unwrap();

        // This is the exact call the advisory fail-closed path makes when a lease exists.
        // "manual_investigation" and "skipped_ineligible" must satisfy the migration CHECK.
        // final_output_settlement is None because "advisory_posture_not_enforceable" is not
        // a valid CHECK value; the reason is conveyed by recommended_next_action instead.
        settle_terminal_event_and_lease(
            &pool,
            "rep-agent-advisory-settle",
            "lk-advisory-settle",
            "skipped",
            "skipped",
            "manual_investigation",
            None,
            "skipped_ineligible",
            "2026-06-09T10:01:00Z",
        )
        .await
        .expect("advisory fail-closed settle must succeed against migrated SQLite CHECK constraints");

        let found = get_repair_event_by_repair_attempt_id(&pool, "rep-agent-advisory-settle")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.status, "skipped");
        assert_eq!(found.recommended_next_action, "manual_investigation");
        assert!(found.final_output_settlement.is_none());
        let found_lease = get_lease_by_key(&pool, "lk-advisory-settle").await.unwrap().unwrap();
        assert_eq!(found_lease.lease_state, LeaseState::Settled);
        assert_eq!(found_lease.settled_result.as_deref(), Some("skipped_ineligible"));
    }

    // SEC-002 integration regression: update_repair_event_status with "manual_investigation"
    // (the advisory no-lease path) must satisfy the migration CHECK constraint.
    #[tokio::test]
    async fn proposal_079_advisory_failclosed_update_satisfies_migration_check() {
        let pool = make_pool().await;
        let event_row = sample_event_row("agent-advisory-update");
        insert_repair_event(&pool, &event_row).await.unwrap();

        // final_output_settlement is None; "advisory_posture_not_enforceable" is not valid per CHECK.
        update_repair_event_status(
            &pool,
            "rep-agent-advisory-update",
            "skipped",
            "skipped",
            "manual_investigation",
            None,
            "2026-06-09T10:01:00Z",
        )
        .await
        .expect("advisory fail-closed update must succeed against migrated SQLite CHECK constraints");

        let found = get_repair_event_by_repair_attempt_id(&pool, "rep-agent-advisory-update")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.status, "skipped");
        assert_eq!(found.recommended_next_action, "manual_investigation");
    }

    // DEFECT-001 regression: Junie provider_mode_mismatch skip must use
    // "blocked_provider_mode_mismatch" (an allowed CHECK value), not the invalid
    // "provider_mode_mismatch_risk" string that causes SQLite CHECK failure.
    #[tokio::test]
    async fn proposal_079_junie_mismatch_settle_uses_valid_final_output_settlement() {
        use domain::output_contract_repair::{LeaseKind, LeaseState};
        let pool = make_pool().await;

        let event_row = sample_event_row("agent-junie-mismatch");
        insert_repair_event(&pool, &event_row).await.unwrap();

        let lease = OutputContractRepairLeaseRow {
            lease_key: "lk-junie-mismatch".into(),
            schema_version: "output_contract_repair_leases_v1".into(),
            repair_event_id: "rep-agent-junie-mismatch".into(),
            run_id: "run-test-001".into(),
            stage_execution_id: "stage-test-001".into(),
            parent_agent_execution_id: "agent-junie-mismatch".into(),
            lease_kind: LeaseKind::Repair,
            lease_state: LeaseState::Reserved,
            settled_result: None,
            reclamation_reason: None,
            frozen_fallback_policy_hash: None,
            idempotency_token: "idem-junie-mismatch".into(),
            lease_owner_principal_id: "principal-junie".into(),
            lease_acquired_at: "2026-06-09T10:00:00Z".into(),
            lease_expires_at: "2026-06-09T10:03:00Z".into(),
            lease_seconds: 180,
            dispatch_committed_at: None,
            version: 0,
            infra_retry_count: 0,
            created_at: "2026-06-09T10:00:00Z".into(),
            updated_at: "2026-06-09T10:00:00Z".into(),
        };
        insert_lease(&pool, &lease).await.unwrap();

        // The Junie mode-mismatch skip path must use "blocked_provider_mode_mismatch" and
        // "manual_investigation". No approval is pending on a mode-mismatch skip, so
        // operator_resolve_approval would mislead the operator.
        settle_terminal_event_and_lease(
            &pool,
            "rep-agent-junie-mismatch",
            "lk-junie-mismatch",
            "skipped",
            "skipped",
            "manual_investigation",
            Some("blocked_provider_mode_mismatch"),
            "skipped_ineligible",
            "2026-06-09T10:01:00Z",
        )
        .await
        .expect("Junie mode-mismatch skip must succeed: blocked_provider_mode_mismatch is a valid CHECK value");

        let found = get_repair_event_by_repair_attempt_id(&pool, "rep-agent-junie-mismatch")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.status, "skipped");
        assert_eq!(found.recommended_next_action, "manual_investigation");
        assert_eq!(
            found.final_output_settlement.as_deref(),
            Some("blocked_provider_mode_mismatch")
        );

        // Also verify the no-lease path (update_repair_event_status) accepts the same value.
        let event_row2 = sample_event_row("agent-junie-mismatch-nolease");
        insert_repair_event(&pool, &event_row2).await.unwrap();
        update_repair_event_status(
            &pool,
            "rep-agent-junie-mismatch-nolease",
            "skipped",
            "skipped",
            "manual_investigation",
            Some("blocked_provider_mode_mismatch"),
            "2026-06-09T10:01:00Z",
        )
        .await
        .expect("update_repair_event_status with blocked_provider_mode_mismatch must satisfy CHECK");

        let found2 = get_repair_event_by_repair_attempt_id(&pool, "rep-agent-junie-mismatch-nolease")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            found2.final_output_settlement.as_deref(),
            Some("blocked_provider_mode_mismatch")
        );
    }

    #[test]
    fn proposal_079_lease_reclamation_reason_roundtrip() {
        use domain::output_contract_repair::LeaseReclamationReason;
        use std::str::FromStr;

        let cases = [
            (LeaseReclamationReason::TtlExpiredReserved, "ttl_expired_reserved"),
            (LeaseReclamationReason::TtlExpiredPromptSent, "ttl_expired_prompt_sent"),
            (LeaseReclamationReason::Cancellation, "cancellation"),
            (LeaseReclamationReason::Supersession, "supersession"),
            (LeaseReclamationReason::PrincipalRevoked, "principal_revoked"),
        ];
        for (variant, s) in &cases {
            assert_eq!(variant.to_string(), *s, "Display mismatch for {variant:?}");
            assert_eq!(
                LeaseReclamationReason::from_str(s).unwrap(),
                *variant,
                "FromStr mismatch for {s}"
            );
        }
        assert!(LeaseReclamationReason::from_str("unknown_value").is_err());
    }
}
