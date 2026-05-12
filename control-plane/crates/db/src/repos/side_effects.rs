use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use domain::side_effect::{
    SideEffect, SideEffectAttempt, SideEffectAttemptId, SideEffectId, SideEffectSettlementId,
    SideEffectStatus,
};

// ── Parse helpers ─────────────────────────────────────────────────────────────

fn parse_dt(s: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(s)
        .with_context(|| format!("parse datetime: {s}"))?
        .with_timezone(&Utc))
}

fn parse_dt_opt(s: Option<String>) -> Result<Option<DateTime<Utc>>> {
    s.map(|v| parse_dt(&v)).transpose()
}

fn parse_side_effect(row: sqlx::sqlite::SqliteRow) -> Result<SideEffect> {
    use domain::ids::{AgentExecutionId, RunId, StageExecutionId};
    use std::str::FromStr;

    let effect_kind: String = row.try_get("effect_kind")?;
    let status: String = row.try_get("status")?;
    let run_id: String = row.try_get("run_id")?;
    let stage_execution_id: String = row.try_get("stage_execution_id")?;
    let agent_execution_id: Option<String> = row.try_get("agent_execution_id")?;
    let external_write_attempted: i64 = row.try_get("external_write_attempted")?;

    Ok(SideEffect {
        id: SideEffectId::from_str(row.try_get::<&str, _>("id")?),
        run_id: RunId::from_str(&run_id).with_context(|| format!("parse run_id: {run_id}"))?,
        stage_execution_id: StageExecutionId::from_str(&stage_execution_id)
            .with_context(|| format!("parse stage_execution_id: {stage_execution_id}"))?,
        agent_execution_id: agent_execution_id
            .as_deref()
            .map(AgentExecutionId::from_str)
            .transpose()
            .with_context(|| "parse agent_execution_id")?,
        effect_kind: effect_kind.parse().map_err(|e: String| anyhow!("{}", e))?,
        target_key: row.try_get("target_key")?,
        idempotency_key: row.try_get("idempotency_key")?,
        idempotency_key_version: row.try_get("idempotency_key_version")?,
        request_fingerprint: row.try_get("request_fingerprint")?,
        request_fingerprint_version: row.try_get("request_fingerprint_version")?,
        status: status.parse().map_err(|e: String| anyhow!("{}", e))?,
        owner_instance_id: row.try_get("owner_instance_id")?,
        lease_acquired_at: parse_dt_opt(row.try_get("lease_acquired_at")?)?,
        lease_renewed_at: parse_dt_opt(row.try_get("lease_renewed_at")?)?,
        lease_expires_at: parse_dt_opt(row.try_get("lease_expires_at")?)?,
        deadline_at: parse_dt_opt(row.try_get("deadline_at")?)?,
        external_write_started_at: parse_dt_opt(row.try_get("external_write_started_at")?)?,
        external_write_attempted: external_write_attempted != 0,
        attempt_budget_remaining: row.try_get("attempt_budget_remaining")?,
        expected_evidence_json: row.try_get("expected_evidence_json")?,
        observed_evidence_summary_json: row.try_get("observed_evidence_summary_json")?,
        evidence_root: row.try_get("evidence_root")?,
        last_error_kind: row.try_get("last_error_kind")?,
        last_error: row.try_get("last_error")?,
        settlement_txn_id: row.try_get("settlement_txn_id")?,
        created_at: parse_dt(row.try_get("created_at")?)?,
        updated_at: parse_dt(row.try_get("updated_at")?)?,
    })
}

// ── Insert ────────────────────────────────────────────────────────────────────

pub async fn insert(pool: &SqlitePool, effect: &SideEffect) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO side_effects (
               id, run_id, stage_execution_id, agent_execution_id,
               effect_kind, target_key,
               idempotency_key, idempotency_key_version,
               request_fingerprint, request_fingerprint_version,
               status, owner_instance_id,
               lease_acquired_at, lease_renewed_at, lease_expires_at,
               deadline_at, external_write_started_at, external_write_attempted,
               attempt_budget_remaining, expected_evidence_json,
               observed_evidence_summary_json, evidence_root,
               last_error_kind, last_error, settlement_txn_id,
               created_at, updated_at
           ) VALUES (
               ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
               ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
               ?21, ?22, ?23, ?24, ?25, ?26, ?27
           )"#,
    )
    .bind(effect.id.as_ref())
    .bind(effect.run_id.to_string())
    .bind(effect.stage_execution_id.to_string())
    .bind(effect.agent_execution_id.as_ref().map(|a| a.to_string()))
    .bind(effect.effect_kind.to_string())
    .bind(&effect.target_key)
    .bind(&effect.idempotency_key)
    .bind(effect.idempotency_key_version)
    .bind(&effect.request_fingerprint)
    .bind(effect.request_fingerprint_version)
    .bind(effect.status.to_string())
    .bind(&effect.owner_instance_id)
    .bind(effect.lease_acquired_at.map(|t| t.to_rfc3339()))
    .bind(effect.lease_renewed_at.map(|t| t.to_rfc3339()))
    .bind(effect.lease_expires_at.map(|t| t.to_rfc3339()))
    .bind(effect.deadline_at.map(|t| t.to_rfc3339()))
    .bind(effect.external_write_started_at.map(|t| t.to_rfc3339()))
    .bind(if effect.external_write_attempted {
        1i64
    } else {
        0i64
    })
    .bind(effect.attempt_budget_remaining)
    .bind(&effect.expected_evidence_json)
    .bind(&effect.observed_evidence_summary_json)
    .bind(&effect.evidence_root)
    .bind(&effect.last_error_kind)
    .bind(&effect.last_error)
    .bind(&effect.settlement_txn_id)
    .bind(effect.created_at.to_rfc3339())
    .bind(effect.updated_at.to_rfc3339())
    .execute(pool)
    .await
    .context("insert side_effect")?;
    Ok(())
}

// ── Find ──────────────────────────────────────────────────────────────────────

pub async fn find_by_id(pool: &SqlitePool, id: &SideEffectId) -> Result<Option<SideEffect>> {
    let row = sqlx::query(
        r#"SELECT id, run_id, stage_execution_id, agent_execution_id,
                  effect_kind, target_key,
                  idempotency_key, idempotency_key_version,
                  request_fingerprint, request_fingerprint_version,
                  status, owner_instance_id,
                  lease_acquired_at, lease_renewed_at, lease_expires_at,
                  deadline_at, external_write_started_at, external_write_attempted,
                  attempt_budget_remaining, expected_evidence_json,
                  observed_evidence_summary_json, evidence_root,
                  last_error_kind, last_error, settlement_txn_id,
                  created_at, updated_at
           FROM side_effects
           WHERE id = ?1"#,
    )
    .bind(id.as_ref())
    .fetch_optional(pool)
    .await
    .context("find side_effect by id")?;

    row.map(parse_side_effect).transpose()
}

pub async fn find_by_idempotency_key(pool: &SqlitePool, key: &str) -> Result<Option<SideEffect>> {
    let row = sqlx::query(
        r#"SELECT id, run_id, stage_execution_id, agent_execution_id,
                  effect_kind, target_key,
                  idempotency_key, idempotency_key_version,
                  request_fingerprint, request_fingerprint_version,
                  status, owner_instance_id,
                  lease_acquired_at, lease_renewed_at, lease_expires_at,
                  deadline_at, external_write_started_at, external_write_attempted,
                  attempt_budget_remaining, expected_evidence_json,
                  observed_evidence_summary_json, evidence_root,
                  last_error_kind, last_error, settlement_txn_id,
                  created_at, updated_at
           FROM side_effects
           WHERE idempotency_key = ?1"#,
    )
    .bind(key)
    .fetch_optional(pool)
    .await
    .context("find side_effect by idempotency_key")?;

    row.map(parse_side_effect).transpose()
}

// ── List unresolved ───────────────────────────────────────────────────────────

pub async fn list_unresolved_for_run(pool: &SqlitePool, run_id: &str) -> Result<Vec<SideEffect>> {
    let rows = sqlx::query(
        r#"SELECT id, run_id, stage_execution_id, agent_execution_id,
                  effect_kind, target_key,
                  idempotency_key, idempotency_key_version,
                  request_fingerprint, request_fingerprint_version,
                  status, owner_instance_id,
                  lease_acquired_at, lease_renewed_at, lease_expires_at,
                  deadline_at, external_write_started_at, external_write_attempted,
                  attempt_budget_remaining, expected_evidence_json,
                  observed_evidence_summary_json, evidence_root,
                  last_error_kind, last_error, settlement_txn_id,
                  created_at, updated_at
           FROM side_effects
           WHERE run_id = ?1
             AND status IN (
               'prepared','executing','externally_observed',
               'needs_reconciliation','conflict','unrecoverable'
             )
           ORDER BY created_at ASC"#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .context("list unresolved side_effects for run")?;

    rows.into_iter().map(parse_side_effect).collect()
}

pub async fn list_unresolved_for_stage(
    pool: &SqlitePool,
    stage_execution_id: &str,
) -> Result<Vec<SideEffect>> {
    let rows = sqlx::query(
        r#"SELECT id, run_id, stage_execution_id, agent_execution_id,
                  effect_kind, target_key,
                  idempotency_key, idempotency_key_version,
                  request_fingerprint, request_fingerprint_version,
                  status, owner_instance_id,
                  lease_acquired_at, lease_renewed_at, lease_expires_at,
                  deadline_at, external_write_started_at, external_write_attempted,
                  attempt_budget_remaining, expected_evidence_json,
                  observed_evidence_summary_json, evidence_root,
                  last_error_kind, last_error, settlement_txn_id,
                  created_at, updated_at
           FROM side_effects
           WHERE stage_execution_id = ?1
             AND status IN (
               'prepared','executing','externally_observed',
               'needs_reconciliation','conflict','unrecoverable'
             )
           ORDER BY created_at ASC"#,
    )
    .bind(stage_execution_id)
    .fetch_all(pool)
    .await
    .context("list unresolved side_effects for stage")?;

    rows.into_iter().map(parse_side_effect).collect()
}

/// Transaction-scoped variant of `list_unresolved_for_stage`. Callers that already
/// hold an open `BEGIN IMMEDIATE` transaction on a single-connection pool must use
/// this to avoid deadlocking against themselves on pool acquire.
pub async fn list_unresolved_for_stage_tx(
    tx: &mut Transaction<'_, Sqlite>,
    stage_execution_id: &str,
) -> Result<Vec<SideEffect>> {
    let rows = sqlx::query(
        r#"SELECT id, run_id, stage_execution_id, agent_execution_id,
                  effect_kind, target_key,
                  idempotency_key, idempotency_key_version,
                  request_fingerprint, request_fingerprint_version,
                  status, owner_instance_id,
                  lease_acquired_at, lease_renewed_at, lease_expires_at,
                  deadline_at, external_write_started_at, external_write_attempted,
                  attempt_budget_remaining, expected_evidence_json,
                  observed_evidence_summary_json, evidence_root,
                  last_error_kind, last_error, settlement_txn_id,
                  created_at, updated_at
           FROM side_effects
           WHERE stage_execution_id = ?1
             AND status IN (
               'prepared','executing','externally_observed',
               'needs_reconciliation','conflict','unrecoverable'
             )
           ORDER BY created_at ASC"#,
    )
    .bind(stage_execution_id)
    .fetch_all(&mut **tx)
    .await
    .context("list unresolved side_effects for stage")?;

    rows.into_iter().map(parse_side_effect).collect()
}

/// List unresolved effects for a run+target_key pair (cross-stage version-cutover check).
/// Used to enforce that no two stages create concurrent intent for the same target_key.
pub async fn list_unresolved_by_run_and_target_key(
    pool: &SqlitePool,
    run_id: &str,
    target_key: &str,
) -> Result<Vec<SideEffect>> {
    let rows = sqlx::query(
        r#"SELECT id, run_id, stage_execution_id, agent_execution_id,
                  effect_kind, target_key,
                  idempotency_key, idempotency_key_version,
                  request_fingerprint, request_fingerprint_version,
                  status, owner_instance_id,
                  lease_acquired_at, lease_renewed_at, lease_expires_at,
                  deadline_at, external_write_started_at, external_write_attempted,
                  attempt_budget_remaining, expected_evidence_json,
                  observed_evidence_summary_json, evidence_root,
                  last_error_kind, last_error, settlement_txn_id,
                  created_at, updated_at
           FROM side_effects
           WHERE run_id = ?1
             AND target_key = ?2
             AND status IN (
               'prepared','executing','externally_observed',
               'needs_reconciliation','conflict','unrecoverable'
             )
           ORDER BY created_at ASC"#,
    )
    .bind(run_id)
    .bind(target_key)
    .fetch_all(pool)
    .await
    .context("list unresolved side_effects by run+target_key")?;

    rows.into_iter().map(parse_side_effect).collect()
}

/// List up to `limit` unresolved effects across all runs (for GraphQL projection).
pub async fn list_unresolved(pool: &SqlitePool, limit: u32) -> Result<Vec<SideEffect>> {
    let limit = limit.min(100).max(1) as i64;
    let rows = sqlx::query(
        r#"SELECT id, run_id, stage_execution_id, agent_execution_id,
                  effect_kind, target_key,
                  idempotency_key, idempotency_key_version,
                  request_fingerprint, request_fingerprint_version,
                  status, owner_instance_id,
                  lease_acquired_at, lease_renewed_at, lease_expires_at,
                  deadline_at, external_write_started_at, external_write_attempted,
                  attempt_budget_remaining, expected_evidence_json,
                  observed_evidence_summary_json, evidence_root,
                  last_error_kind, last_error, settlement_txn_id,
                  created_at, updated_at
           FROM side_effects
           WHERE status IN (
               'prepared','executing','externally_observed',
               'needs_reconciliation','conflict','unrecoverable'
             )
           ORDER BY created_at ASC
           LIMIT ?1"#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .context("list unresolved side_effects")?;

    rows.into_iter().map(parse_side_effect).collect()
}

// ── CAS: executor start ───────────────────────────────────────────────────────

pub struct ExecutorStartCasParams<'a> {
    pub effect_id: &'a SideEffectId,
    pub owner_instance_id: &'a str,
    pub attempt_id: &'a SideEffectAttemptId,
    pub lease_acquired_at: DateTime<Utc>,
    pub lease_expires_at: DateTime<Utc>,
    pub deadline_at: Option<DateTime<Utc>>,
    pub now: DateTime<Utc>,
}

/// Atomically transition prepared → executing and insert the attempt row.
/// Returns Ok(true) if exactly one row was updated (CAS won), Ok(false) if
/// the CAS predicate did not match (row was raced or already executing).
pub async fn executor_start_cas(
    pool: &SqlitePool,
    params: &ExecutorStartCasParams<'_>,
) -> Result<bool> {
    let mut tx =
        crate::pool::begin_immediate_with_retry(pool, "side_effects.executor_start_cas").await?;

    let rows_updated: u64 = sqlx::query(
        r#"UPDATE side_effects
           SET status = 'executing',
               owner_instance_id = ?1,
               lease_acquired_at = ?2,
               lease_renewed_at = ?2,
               lease_expires_at = ?3,
               deadline_at = COALESCE(?4, deadline_at),
               updated_at = ?5
           WHERE id = ?6
             AND status = 'prepared'
             AND (lease_expires_at IS NULL OR lease_expires_at < ?5)
             AND external_write_attempted = 0"#,
    )
    .bind(params.owner_instance_id)
    .bind(params.lease_acquired_at.to_rfc3339())
    .bind(params.lease_expires_at.to_rfc3339())
    .bind(params.deadline_at.map(|t| t.to_rfc3339()))
    .bind(params.now.to_rfc3339())
    .bind(params.effect_id.as_ref())
    .execute(&mut *tx)
    .await
    .context("executor_start_cas update")?
    .rows_affected();

    if rows_updated != 1 {
        // CAS lost — do not insert attempt row, do not commit
        return Ok(false);
    }

    // Insert attempt row inside the same transaction
    sqlx::query(
        r#"INSERT INTO side_effect_attempts
               (id, side_effect_id, attempt_number, attempt_kind, owner_instance_id, started_at, deadline_at)
           SELECT ?1, ?2, COALESCE(MAX(attempt_number), 0) + 1, 'initial', ?3, ?4, ?5
           FROM side_effect_attempts
           WHERE side_effect_id = ?2"#,
    )
    .bind(params.attempt_id.as_ref())
    .bind(params.effect_id.as_ref())
    .bind(params.owner_instance_id)
    .bind(params.now.to_rfc3339())
    .bind(params.deadline_at.map(|t| t.to_rfc3339()))
    .execute(&mut *tx)
    .await
    .context("executor_start_cas insert attempt")?;

    tx.commit().await.context("executor_start_cas commit")?;
    Ok(true)
}

// ── CAS: lease renew ──────────────────────────────────────────────────────────

/// Returns Ok(true) if lease was renewed successfully.
pub async fn lease_renew_cas(
    pool: &SqlitePool,
    effect_id: &SideEffectId,
    owner_instance_id: &str,
    new_expires_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<bool> {
    let rows_updated: u64 = sqlx::query(
        r#"UPDATE side_effects
           SET lease_renewed_at = ?1,
               lease_expires_at = ?2,
               updated_at = ?1
           WHERE id = ?3
             AND status = 'executing'
             AND owner_instance_id = ?4
             AND lease_expires_at >= ?1"#,
    )
    .bind(now.to_rfc3339())
    .bind(new_expires_at.to_rfc3339())
    .bind(effect_id.as_ref())
    .bind(owner_instance_id)
    .execute(pool)
    .await
    .context("lease_renew_cas")?
    .rows_affected();

    Ok(rows_updated == 1)
}

// ── CAS: executor settle ──────────────────────────────────────────────────────

pub struct ExecutorSettleCasParams<'a> {
    pub effect_id: &'a SideEffectId,
    pub owner_instance_id: &'a str,
    pub settlement_attempt_id: &'a SideEffectAttemptId,
    pub observed_lease_renewed_at: DateTime<Utc>,
    pub new_status: SideEffectStatus,
    pub observed_evidence_summary_json: Option<&'a str>,
    pub settlement_txn_id: &'a str,
    pub last_error_kind: Option<&'a str>,
    pub last_error: Option<&'a str>,
    pub now: DateTime<Utc>,
    pub settlement_source: &'a str,
    pub receipt_artifact_id: Option<&'a str>,
    pub decision_json: Option<&'a str>,
    pub decision_json_hash: Option<&'a str>,
    pub disposition_id: Option<&'a str>,
}

/// Settle an executing effect with strict CAS predicate.
/// Returns Ok(true) if settlement was applied, Ok(false) if CAS lost.
pub async fn executor_settle_cas(
    pool: &SqlitePool,
    params: &ExecutorSettleCasParams<'_>,
) -> Result<bool> {
    let mut tx =
        crate::pool::begin_immediate_with_retry(pool, "side_effects.executor_settle_cas").await?;
    let settled = executor_settle_cas_tx(&mut tx, params).await?;
    if !settled {
        return Ok(false);
    }
    tx.commit().await.context("executor_settle_cas commit")?;
    Ok(true)
}

pub async fn executor_settle_cas_tx(
    tx: &mut Transaction<'_, Sqlite>,
    params: &ExecutorSettleCasParams<'_>,
) -> Result<bool> {
    let settlement_id = SideEffectSettlementId::new();
    let new_status_str = params.new_status.to_string();

    // Predicate is status-sensitive per proposal:
    // - executing: require matching owner, live lease (>= now), and matching lease_renewed_at
    // - externally_observed: require matching owner only (lease may have lapsed after external observation)
    let rows_updated: u64 = sqlx::query(
        r#"UPDATE side_effects
           SET status = ?1,
               observed_evidence_summary_json = COALESCE(?2, observed_evidence_summary_json),
               last_error_kind = ?3,
               last_error = ?4,
               settlement_txn_id = ?5,
               updated_at = ?6
           WHERE id = ?7
             AND owner_instance_id = ?8
             AND NOT EXISTS (
               SELECT 1 FROM side_effect_settlements WHERE side_effect_id = ?7
             )
             AND (
               (status = 'executing'
                AND lease_expires_at >= ?6
                AND lease_renewed_at = ?9)
               OR
               (status = 'externally_observed')
             )"#,
    )
    .bind(&new_status_str)
    .bind(params.observed_evidence_summary_json)
    .bind(params.last_error_kind)
    .bind(params.last_error)
    .bind(params.settlement_txn_id)
    .bind(params.now.to_rfc3339())
    .bind(params.effect_id.as_ref())
    .bind(params.owner_instance_id)
    .bind(params.observed_lease_renewed_at.to_rfc3339())
    .execute(&mut **tx)
    .await
    .context("executor_settle_cas update")?
    .rows_affected();

    if rows_updated != 1 {
        return Ok(false);
    }

    // Update attempt row completion — verify the attempt belongs to this effect.
    // If zero rows are affected the attempt_id is wrong; roll back rather than
    // leaving the effect settled with no matching attempt record.
    let attempt_rows: u64 = sqlx::query(
        r#"UPDATE side_effect_attempts
           SET completed_at = ?1,
               exit_status = ?2,
               observed_evidence_summary_json = ?3,
               error_kind = ?4,
               error = ?5
           WHERE id = ?6
             AND side_effect_id = ?7"#,
    )
    .bind(params.now.to_rfc3339())
    .bind(if params.new_status == SideEffectStatus::Settled {
        "succeeded"
    } else {
        "failed"
    })
    .bind(params.observed_evidence_summary_json)
    .bind(params.last_error_kind)
    .bind(params.last_error)
    .bind(params.settlement_attempt_id.as_ref())
    .bind(params.effect_id.as_ref())
    .execute(&mut **tx)
    .await
    .context("executor_settle_cas update attempt")?
    .rows_affected();

    if attempt_rows != 1 {
        // Attempt row does not match this effect — do not commit.
        return Ok(false);
    }

    // Insert settlement row
    sqlx::query(
        r#"INSERT INTO side_effect_settlements
               (id, side_effect_id, settlement_status, settlement_source,
                receipt_artifact_id, applied_at, applied_by,
                disposition_id, settlement_attempt_id, decision_json, decision_json_hash)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"#,
    )
    .bind(settlement_id.as_ref())
    .bind(params.effect_id.as_ref())
    .bind(&new_status_str)
    .bind(params.settlement_source)
    .bind(params.receipt_artifact_id)
    .bind(params.now.to_rfc3339())
    .bind(params.owner_instance_id)
    .bind(params.disposition_id)
    .bind(params.settlement_attempt_id.as_ref())
    .bind(params.decision_json)
    .bind(params.decision_json_hash)
    .execute(&mut **tx)
    .await
    .context("executor_settle_cas insert settlement")?;

    Ok(true)
}

// ── CAS: executor failure (needs_reconciliation) ─────────────────────────────

/// Parameters for transitioning an executing effect to needs_reconciliation after
/// a release operation failure. Unlike executor_settle_cas, this does NOT insert
/// into side_effect_settlements because needs_reconciliation is not a terminal state
/// and is excluded from the settlement_status CHECK constraint.
pub struct ExecutorFailCasParams<'a> {
    pub effect_id: &'a SideEffectId,
    pub owner_instance_id: &'a str,
    pub attempt_id: &'a SideEffectAttemptId,
    pub observed_lease_renewed_at: DateTime<Utc>,
    pub last_error_kind: &'a str,
    pub last_error: &'a str,
    pub now: DateTime<Utc>,
}

/// Transition an executing effect to needs_reconciliation after a release failure.
/// CAS predicate: status=executing AND owner matches AND lease_renewed_at matches.
/// Returns Ok(true) on success, Ok(false) if CAS lost (reaper already transitioned),
/// Err(e) on DB error.
pub async fn executor_fail_cas(
    pool: &SqlitePool,
    params: &ExecutorFailCasParams<'_>,
) -> Result<bool> {
    let mut tx =
        crate::pool::begin_immediate_with_retry(pool, "side_effects.executor_fail_cas").await?;
    let transitioned = executor_fail_cas_tx(&mut tx, params).await?;
    if !transitioned {
        return Ok(false);
    }
    tx.commit().await.context("executor_fail_cas commit")?;
    Ok(true)
}

pub async fn executor_fail_cas_tx(
    tx: &mut Transaction<'_, Sqlite>,
    params: &ExecutorFailCasParams<'_>,
) -> Result<bool> {
    let rows_updated: u64 = sqlx::query(
        r#"UPDATE side_effects
           SET status = 'needs_reconciliation',
               last_error_kind = ?1,
               last_error = ?2,
               updated_at = ?3
           WHERE id = ?4
             AND status = 'executing'
             AND owner_instance_id = ?5
             AND COALESCE(lease_renewed_at, lease_acquired_at) = ?6"#,
    )
    .bind(params.last_error_kind)
    .bind(params.last_error)
    .bind(params.now.to_rfc3339())
    .bind(params.effect_id.as_ref())
    .bind(params.owner_instance_id)
    .bind(params.observed_lease_renewed_at.to_rfc3339())
    .execute(&mut **tx)
    .await
    .context("executor_fail_cas update")?
    .rows_affected();

    if rows_updated != 1 {
        return Ok(false);
    }

    // Mark the attempt record as failed.
    sqlx::query(
        r#"UPDATE side_effect_attempts
           SET completed_at = ?1,
               exit_status = 'failed',
               error_kind = ?2,
               error = ?3
           WHERE id = ?4
             AND side_effect_id = ?5"#,
    )
    .bind(params.now.to_rfc3339())
    .bind(params.last_error_kind)
    .bind(params.last_error)
    .bind(params.attempt_id.as_ref())
    .bind(params.effect_id.as_ref())
    .execute(&mut **tx)
    .await
    .context("executor_fail_cas update attempt")?;

    Ok(true)
}

// ── CAS: reaper transition ────────────────────────────────────────────────────

pub struct ReaperTransitionCasParams<'a> {
    pub effect_id: &'a SideEffectId,
    pub observed_status: SideEffectStatus,
    pub observed_owner: Option<&'a str>,
    pub observed_lease_renewed_at: Option<DateTime<Utc>>,
    /// Observed updated_at from the loaded row — used as CAS predicate for
    /// prepared/externally_observed transitions to prevent reaper/executor races.
    pub observed_updated_at: DateTime<Utc>,
    pub now: DateTime<Utc>,
    pub last_error_kind: &'a str,
    pub last_error: &'a str,
}

/// Transition a stale/expired effect to needs_reconciliation via watchdog.
/// Returns Ok(true) if transition committed, Ok(false) if CAS predicate failed.
pub async fn reaper_transition_cas(
    pool: &SqlitePool,
    params: &ReaperTransitionCasParams<'_>,
) -> Result<bool> {
    let mut tx =
        crate::pool::begin_immediate_with_retry(pool, "side_effects.reaper_transition_cas").await?;

    let rows_updated: u64 = match params.observed_status {
        SideEffectStatus::Executing => sqlx::query(
            r#"UPDATE side_effects
                   SET status = 'needs_reconciliation',
                       last_error_kind = ?1,
                       last_error = ?2,
                       updated_at = ?3
                   WHERE id = ?4
                     AND status = 'executing'
                     AND owner_instance_id = ?5
                     AND COALESCE(lease_renewed_at, lease_acquired_at) = ?6
                     AND (lease_expires_at < ?3 OR deadline_at < ?3)"#,
        )
        .bind(params.last_error_kind)
        .bind(params.last_error)
        .bind(params.now.to_rfc3339())
        .bind(params.effect_id.as_ref())
        .bind(params.observed_owner)
        .bind(params.observed_lease_renewed_at.map(|t| t.to_rfc3339()))
        .execute(&mut *tx)
        .await
        .context("reaper_transition_cas executing")?
        .rows_affected(),
        SideEffectStatus::Prepared => sqlx::query(
            r#"UPDATE side_effects
                   SET status = 'needs_reconciliation',
                       last_error_kind = ?1,
                       last_error = ?2,
                       updated_at = ?3
                   WHERE id = ?4
                     AND status = 'prepared'
                     AND updated_at = ?5
                     AND deadline_at IS NOT NULL
                     AND deadline_at < ?3"#,
        )
        .bind(params.last_error_kind)
        .bind(params.last_error)
        .bind(params.now.to_rfc3339())
        .bind(params.effect_id.as_ref())
        .bind(params.observed_updated_at.to_rfc3339())
        .execute(&mut *tx)
        .await
        .context("reaper_transition_cas prepared")?
        .rows_affected(),
        SideEffectStatus::ExternallyObserved => sqlx::query(
            r#"UPDATE side_effects
                   SET status = 'needs_reconciliation',
                       last_error_kind = ?1,
                       last_error = ?2,
                       updated_at = ?3
                   WHERE id = ?4
                     AND status = 'externally_observed'
                     AND updated_at = ?5
                     AND deadline_at IS NOT NULL
                     AND deadline_at < ?3"#,
        )
        .bind(params.last_error_kind)
        .bind(params.last_error)
        .bind(params.now.to_rfc3339())
        .bind(params.effect_id.as_ref())
        .bind(params.observed_updated_at.to_rfc3339())
        .execute(&mut *tx)
        .await
        .context("reaper_transition_cas externally_observed")?
        .rows_affected(),
        _ => {
            return Err(anyhow!(
                "reaper_transition_cas: invalid source status {}",
                params.observed_status
            ));
        }
    };

    if rows_updated != 1 {
        return Ok(false);
    }

    tx.commit().await.context("reaper_transition_cas commit")?;
    Ok(true)
}

// ── MCP operator disposition ──────────────────────────────────────────────────

/// Apply an operator disposition to a needs_reconciliation/conflict/unrecoverable effect.
/// Enforces disposition_id idempotency: same id + same payload → idempotent success.
pub async fn apply_operator_disposition(
    pool: &SqlitePool,
    effect_id: &SideEffectId,
    new_status: SideEffectStatus,
    settlement_source: &str,
    disposition_id: &str,
    decision_json: &str,
    decision_json_hash: &str,
    applied_by: &str,
    now: DateTime<Utc>,
) -> Result<DispositionOutcome> {
    // Check for existing settlement with this disposition_id (globally, not scoped to effect).
    // Defense-in-depth: compare both hash and text to eliminate hash collision.
    // HIGH-001 fix: only return AlreadyApplied when the settlement is for THIS effect_id.
    // A disposition used for a different effect returns PayloadMismatch to prevent
    // a cross-effect replay from falsely claiming idempotent success.
    let existing: Option<(String, String, String, Option<String>)> = sqlx::query_as(
        r#"SELECT side_effect_id, settlement_status, decision_json_hash, decision_json
           FROM side_effect_settlements
           WHERE disposition_id = ?1"#,
    )
    .bind(disposition_id)
    .fetch_optional(pool)
    .await
    .context("check existing disposition")?;

    if let Some((existing_side_effect_id, _, existing_hash, existing_text)) = existing {
        if existing_side_effect_id != effect_id.as_ref() {
            // Disposition was already used for a different effect — reject as payload mismatch.
            return Ok(DispositionOutcome::PayloadMismatch);
        }
        let hash_matches = existing_hash == decision_json_hash;
        let text_matches = existing_text.as_deref() == Some(decision_json);
        if hash_matches && text_matches {
            return Ok(DispositionOutcome::AlreadyApplied);
        } else {
            return Ok(DispositionOutcome::PayloadMismatch);
        }
    }

    // Check if a settlement already exists for this effect (with a DIFFERENT disposition_id).
    // The UNIQUE index on side_effect_id would cause an INSERT failure, so we detect this
    // upfront and return NotApplicable rather than an opaque constraint error.
    let existing_for_effect: Option<String> = sqlx::query_scalar(
        r#"SELECT disposition_id FROM side_effect_settlements WHERE side_effect_id = ?1"#,
    )
    .bind(effect_id.as_ref())
    .fetch_optional(pool)
    .await
    .context("check existing settlement for effect")?;

    if let Some(existing_disp) = existing_for_effect {
        if existing_disp == disposition_id {
            // Same disposition_id — already handled by the check above; should not reach here.
            return Ok(DispositionOutcome::AlreadyApplied);
        }
        // Settlement exists for this effect with a different disposition_id.
        return Ok(DispositionOutcome::NotApplicable);
    }

    let settlement_id = SideEffectSettlementId::new();
    let new_status_str = new_status.to_string();

    let mut tx =
        crate::pool::begin_immediate_with_retry(pool, "side_effects.apply_operator_disposition")
            .await?;

    let rows_updated: u64 = sqlx::query(
        r#"UPDATE side_effects
           SET status = ?1,
               updated_at = ?2
           WHERE id = ?3
             AND status IN ('needs_reconciliation', 'conflict', 'unrecoverable')"#,
    )
    .bind(&new_status_str)
    .bind(now.to_rfc3339())
    .bind(effect_id.as_ref())
    .execute(&mut *tx)
    .await
    .context("apply_operator_disposition update")?
    .rows_affected();

    if rows_updated != 1 {
        return Ok(DispositionOutcome::NotApplicable);
    }

    sqlx::query(
        r#"INSERT INTO side_effect_settlements
               (id, side_effect_id, settlement_status, settlement_source,
                applied_at, applied_by, disposition_id, decision_json, decision_json_hash)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
    )
    .bind(settlement_id.as_ref())
    .bind(effect_id.as_ref())
    .bind(&new_status_str)
    .bind(settlement_source)
    .bind(now.to_rfc3339())
    .bind(applied_by)
    .bind(disposition_id)
    .bind(decision_json)
    .bind(decision_json_hash)
    .execute(&mut *tx)
    .await
    .context("apply_operator_disposition insert settlement")?;

    tx.commit()
        .await
        .context("apply_operator_disposition commit")?;
    Ok(DispositionOutcome::Applied)
}

#[derive(Debug, Eq, PartialEq)]
pub enum DispositionOutcome {
    Applied,
    AlreadyApplied,
    PayloadMismatch,
    NotApplicable,
}

// ── Attempt helpers ───────────────────────────────────────────────────────────

pub async fn list_attempts(
    pool: &SqlitePool,
    effect_id: &SideEffectId,
) -> Result<Vec<SideEffectAttempt>> {
    let rows = sqlx::query(
        r#"SELECT id, side_effect_id, attempt_number, attempt_kind,
                  owner_instance_id, started_at, completed_at, deadline_at,
                  exit_status, stdout_path, stderr_path, trace_path,
                  observed_evidence_summary_json, error_kind, error
           FROM side_effect_attempts
           WHERE side_effect_id = ?1
           ORDER BY attempt_number ASC"#,
    )
    .bind(effect_id.as_ref())
    .fetch_all(pool)
    .await
    .context("list attempts")?;

    rows.into_iter()
        .map(|row| {
            Ok(SideEffectAttempt {
                id: SideEffectAttemptId::from_string(row.try_get::<String, _>("id")?),
                side_effect_id: SideEffectId::from_str(row.try_get::<&str, _>("side_effect_id")?),
                attempt_number: row.try_get("attempt_number")?,
                attempt_kind: row.try_get("attempt_kind")?,
                owner_instance_id: row.try_get("owner_instance_id")?,
                started_at: parse_dt(row.try_get("started_at")?)?,
                completed_at: parse_dt_opt(row.try_get("completed_at")?)?,
                deadline_at: parse_dt_opt(row.try_get("deadline_at")?)?,
                exit_status: row.try_get("exit_status")?,
                stdout_path: row.try_get("stdout_path")?,
                stderr_path: row.try_get("stderr_path")?,
                trace_path: row.try_get("trace_path")?,
                observed_evidence_summary_json: row.try_get("observed_evidence_summary_json")?,
                error_kind: row.try_get("error_kind")?,
                error: row.try_get("error")?,
            })
        })
        .collect()
}

// ── Count helpers ─────────────────────────────────────────────────────────────

/// Returns true if there are any external-write attempts for this effect.
pub async fn has_external_write_attempt(
    pool: &SqlitePool,
    effect_id: &SideEffectId,
) -> Result<bool> {
    let count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM side_effects
           WHERE id = ?1 AND external_write_attempted = 1"#,
    )
    .bind(effect_id.as_ref())
    .fetch_one(pool)
    .await
    .context("check external_write_attempted")?;
    Ok(count > 0)
}

/// Mark the effect as having started an external write.
/// Requires: status='executing', matching owner_instance_id, external_write_attempted=0,
/// and no existing settlement. Returns Ok(false) on CAS miss (caller must abort).
pub async fn mark_external_write_started(
    pool: &SqlitePool,
    effect_id: &SideEffectId,
    owner_instance_id: &str,
    started_at: DateTime<Utc>,
) -> Result<bool> {
    let rows_updated: u64 = sqlx::query(
        r#"UPDATE side_effects
           SET external_write_started_at = ?1,
               external_write_attempted = 1,
               updated_at = ?1
           WHERE id = ?2
             AND status = 'executing'
             AND owner_instance_id = ?3
             AND external_write_attempted = 0
             AND NOT EXISTS (
               SELECT 1 FROM side_effect_settlements WHERE side_effect_id = ?2
             )"#,
    )
    .bind(started_at.to_rfc3339())
    .bind(effect_id.as_ref())
    .bind(owner_instance_id)
    .execute(pool)
    .await
    .context("mark_external_write_started")?
    .rows_affected();
    Ok(rows_updated == 1)
}

/// Fetch side effects that need startup/watchdog recovery.
///
/// This intentionally covers every crash window P078 cares about:
/// - `prepared`: intent was recorded but executor never acquired/finished work
/// - `executing`: executor lease/deadline expired
/// - `externally_observed`: external write happened but settlement did not complete
/// - `settled`: readback must verify evidence durability before the run can be trusted
pub async fn list_watchdog_recovery_candidates(
    pool: &SqlitePool,
    now: DateTime<Utc>,
    limit: i64,
) -> Result<Vec<SideEffect>> {
    let rows = sqlx::query(
        r#"SELECT id, run_id, stage_execution_id, agent_execution_id,
                  effect_kind, target_key,
                  idempotency_key, idempotency_key_version,
                  request_fingerprint, request_fingerprint_version,
                  status, owner_instance_id,
                  lease_acquired_at, lease_renewed_at, lease_expires_at,
                  deadline_at, external_write_started_at, external_write_attempted,
                  attempt_budget_remaining, expected_evidence_json,
                  observed_evidence_summary_json, evidence_root,
                  last_error_kind, last_error, settlement_txn_id,
                  created_at, updated_at
           FROM side_effects
           WHERE (
               status = 'executing'
               AND (lease_expires_at < ?1 OR deadline_at < ?1)
             )
              OR (
               status IN ('prepared', 'externally_observed')
               AND deadline_at IS NOT NULL
               AND deadline_at < ?1
             )
              OR (
               status = 'settled'
               AND observed_evidence_summary_json IS NOT NULL
             )
           ORDER BY updated_at ASC
           LIMIT ?2"#,
    )
    .bind(now.to_rfc3339())
    .bind(limit)
    .fetch_all(pool)
    .await
    .context("list watchdog recovery side_effect candidates")?;

    rows.into_iter().map(parse_side_effect).collect()
}

/// Backward-compatible test/helper entry point for the original stale-executing
/// watchdog query.
pub async fn list_expired_executing(
    pool: &SqlitePool,
    now: DateTime<Utc>,
    limit: i64,
) -> Result<Vec<SideEffect>> {
    let candidates = list_watchdog_recovery_candidates(pool, now, limit).await?;
    Ok(candidates
        .into_iter()
        .filter(|effect| effect.status == SideEffectStatus::Executing)
        .collect())
}

/// Mark a previously settled side effect as needing operator reconciliation
/// because its evidence manifest or referenced files failed startup verification.
pub async fn mark_settled_evidence_failed(
    pool: &SqlitePool,
    effect_id: &SideEffectId,
    observed_updated_at: DateTime<Utc>,
    last_error_kind: &str,
    last_error: &str,
    now: DateTime<Utc>,
) -> Result<bool> {
    let rows_updated = sqlx::query(
        r#"UPDATE side_effects
           SET status = 'needs_reconciliation',
               last_error_kind = ?1,
               last_error = ?2,
               updated_at = ?3
           WHERE id = ?4
             AND status = 'settled'
             AND updated_at = ?5"#,
    )
    .bind(last_error_kind)
    .bind(last_error)
    .bind(now.to_rfc3339())
    .bind(effect_id.as_ref())
    .bind(observed_updated_at.to_rfc3339())
    .execute(pool)
    .await
    .context("mark settled side_effect evidence failed")?
    .rows_affected();

    Ok(rows_updated == 1)
}
