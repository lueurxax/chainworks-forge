use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use domain::agent::{AgentExecution, AgentStatus};
use domain::ids::{AgentExecutionId, RunId, StageExecutionId};
use domain::provider::ProviderFamily;
use domain::xcode_runtime::{XcodeRuntimeObservation, XcodeRuntimeObservationUpdate};

use crate::writer::begin_registered_immediate_transaction;

const SELECT_COLS: &str = r#"id, stage_execution_id, agent_id, provider, model, status, started_at, completed_at,
                owner_execution_lineage_id, session_lineage_id, session_generation_id, rehydrated_from_checkpoint_artifact_id,
                invocation_owner_key, session_reuse_scope, session_family_id,
                session_reuse_disposition, session_reset_reason,
                backend_profile_id, requested_mcp_extensions_json, predicted_mcp_extensions_json,
                predicted_mcp_runtime_ids_json, actual_mcp_extensions_json, actual_mcp_runtime_ids_json,
                denied_mcp_extensions_json, mcp_blocking_issues_json, actual_mcp_observation_json,
                actual_xcode_runtime_observation_json,
                mcp_session_startup_latency_ms,
                owner_kind, owner_id, lead_mediation_record_id, origin_stage_execution_id,
                total_cost_cents, input_tokens, output_tokens, cached_input_tokens,
                transcript_artifact_id,
                actual_toolchain_mapping_diagnostics_json,
                escalation_policy_id, escalation_policy_hash,
                escalation_tier_id, escalation_tier_kind_raw,
                escalation_trigger_raw, escalation_digest_version, escalation_ledger_id"#;

#[derive(Clone, Debug, PartialEq)]
pub struct RunningAgentExecution {
    pub id: AgentExecutionId,
    pub run_id: RunId,
    pub stage_id: String,
    pub stage_execution_id: StageExecutionId,
    pub provider_family: Option<String>,
    pub session_generation_id: Option<String>,
}

pub async fn insert(pool: &SqlitePool, exec: &AgentExecution) -> Result<()> {
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "agent_executions.insert").await?;
    insert_tx(&mut tx, exec).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn insert_tx(tx: &mut Transaction<'_, Sqlite>, exec: &AgentExecution) -> Result<()> {
    let owner_kind = exec.owner_kind.as_deref().unwrap_or("stage_execution");
    let stage_execution_id = exec.stage_execution_id.map(|id| id.to_string());
    let owner_id = exec.owner_id.clone().or_else(|| {
        (owner_kind == "stage_execution")
            .then(|| stage_execution_id.clone())
            .flatten()
    });
    sqlx::query(
        "INSERT INTO agent_executions
         (id, stage_execution_id, agent_id, provider, provider_family, model, status, started_at, completed_at,
          owner_execution_lineage_id, session_lineage_id, session_generation_id, rehydrated_from_checkpoint_artifact_id,
          invocation_owner_key, session_reuse_scope, session_family_id,
          session_reuse_disposition, session_reset_reason,
          backend_profile_id, requested_mcp_extensions_json, predicted_mcp_extensions_json,
          predicted_mcp_runtime_ids_json, actual_mcp_extensions_json, actual_mcp_runtime_ids_json,
          denied_mcp_extensions_json, mcp_blocking_issues_json, actual_mcp_observation_json,
          actual_xcode_runtime_observation_json,
          mcp_session_startup_latency_ms,
          owner_kind, owner_id, lead_mediation_record_id, origin_stage_execution_id,
          escalation_policy_id, escalation_policy_hash,
          escalation_tier_id, escalation_tier_kind_raw,
          escalation_trigger_raw, escalation_digest_version, escalation_ledger_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(exec.id.to_string())
    .bind(&stage_execution_id)
    .bind(&exec.agent_id)
    .bind(&exec.provider)
    .bind(ProviderFamily::canonicalize_known_alias(&exec.provider))
    .bind(&exec.model)
    .bind(exec.status.to_string())
    .bind(exec.started_at.to_rfc3339())
    .bind(exec.completed_at.map(|d| d.to_rfc3339()))
    .bind(&exec.owner_execution_lineage_id)
    .bind(&exec.session_lineage_id)
    .bind(&exec.session_generation_id)
    .bind(&exec.rehydrated_from_checkpoint_artifact_id)
    .bind(&exec.invocation_owner_key)
    .bind(&exec.session_reuse_scope)
    .bind(&exec.session_family_id)
    .bind(&exec.session_reuse_disposition)
    .bind(&exec.session_reset_reason)
    .bind(&exec.backend_profile_id)
    .bind(&exec.requested_mcp_extensions_json)
    .bind(&exec.predicted_mcp_extensions_json)
    .bind(&exec.predicted_mcp_runtime_ids_json)
    .bind(&exec.actual_mcp_extensions_json)
    .bind(&exec.actual_mcp_runtime_ids_json)
    .bind(&exec.denied_mcp_extensions_json)
    .bind(&exec.mcp_blocking_issues_json)
    .bind(&exec.actual_mcp_observation_json)
    .bind(&exec.actual_xcode_runtime_observation_json)
    .bind(exec.mcp_session_startup_latency_ms)
    .bind(owner_kind)
    .bind(&owner_id)
    .bind(&exec.lead_mediation_record_id)
    .bind(&exec.origin_stage_execution_id)
    .bind(&exec.escalation_policy_id)
    .bind(&exec.escalation_policy_hash)
    .bind(&exec.escalation_tier_id)
    .bind(&exec.escalation_tier_kind_raw)
    .bind(&exec.escalation_trigger_raw)
    .bind(&exec.escalation_digest_version)
    .bind(&exec.escalation_ledger_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn find_by_id(pool: &SqlitePool, id: AgentExecutionId) -> Result<Option<AgentExecution>> {
    let query = format!("SELECT {SELECT_COLS} FROM agent_executions WHERE id = ?");
    let row = sqlx::query(&query)
        .bind(id.to_string())
        .fetch_optional(pool)
        .await?;

    row.map(|row| parse_agent_execution_row(&row)).transpose()
}

pub async fn update_completed(
    pool: &SqlitePool,
    id: AgentExecutionId,
    status: AgentStatus,
    completed_at: DateTime<Utc>,
) -> Result<()> {
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "agent_executions.update_completed")
            .await?;
    update_completed_tx(&mut tx, id, status, completed_at).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn update_completed_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: AgentExecutionId,
    status: AgentStatus,
    completed_at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query("UPDATE agent_executions SET status = ?, completed_at = ? WHERE id = ?")
        .bind(status.to_string())
        .bind(completed_at.to_rfc3339())
        .bind(id.to_string())
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub async fn update_mcp_provenance(
    pool: &SqlitePool,
    id: AgentExecutionId,
    backend_profile_id: Option<&str>,
    requested_mcp_extensions_json: Option<&str>,
    predicted_mcp_extensions_json: Option<&str>,
    predicted_mcp_runtime_ids_json: Option<&str>,
    denied_mcp_extensions_json: Option<&str>,
    mcp_blocking_issues_json: Option<&str>,
) -> Result<()> {
    crate::execute_repository_write!(
        pool,
        "agent_executions.update_mcp_provenance",
        sqlx::query(
            "UPDATE agent_executions
         SET backend_profile_id = ?,
             requested_mcp_extensions_json = ?,
             predicted_mcp_extensions_json = ?,
             predicted_mcp_runtime_ids_json = ?,
             denied_mcp_extensions_json = ?,
             mcp_blocking_issues_json = ?
         WHERE id = ?",
        )
        .bind(backend_profile_id)
        .bind(requested_mcp_extensions_json)
        .bind(predicted_mcp_extensions_json)
        .bind(predicted_mcp_runtime_ids_json)
        .bind(denied_mcp_extensions_json)
        .bind(mcp_blocking_issues_json)
        .bind(id.to_string())
    )?;
    Ok(())
}

pub async fn update_mcp_actual(
    pool: &SqlitePool,
    id: AgentExecutionId,
    actual_mcp_extensions_json: Option<&str>,
    actual_mcp_runtime_ids_json: Option<&str>,
    actual_mcp_observation_json: Option<&str>,
    mcp_session_startup_latency_ms: Option<i64>,
) -> Result<()> {
    crate::execute_repository_write!(
        pool,
        "agent_executions.update_mcp_actual",
        sqlx::query(
            "UPDATE agent_executions
         SET actual_mcp_extensions_json = ?,
             actual_mcp_runtime_ids_json = ?,
             actual_mcp_observation_json = ?,
             mcp_session_startup_latency_ms = ?
         WHERE id = ?",
        )
        .bind(actual_mcp_extensions_json)
        .bind(actual_mcp_runtime_ids_json)
        .bind(actual_mcp_observation_json)
        .bind(mcp_session_startup_latency_ms)
        .bind(id.to_string())
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn update_session_policy(
    pool: &SqlitePool,
    id: AgentExecutionId,
    session_lineage_id: Option<&str>,
    session_generation_id: Option<&str>,
    rehydrated_from_checkpoint_artifact_id: Option<&str>,
    invocation_owner_key: Option<&str>,
    session_reuse_disposition: Option<&str>,
    session_reset_reason: Option<&str>,
) -> Result<()> {
    crate::execute_repository_write!(
        pool,
        "agent_executions.update_session_policy",
        sqlx::query(
            "UPDATE agent_executions
         SET session_lineage_id = ?1,
             session_generation_id = ?2,
             rehydrated_from_checkpoint_artifact_id = ?3,
             invocation_owner_key = ?4,
             session_reuse_disposition = ?5,
             session_reset_reason = ?6
         WHERE id = ?7",
        )
        .bind(session_lineage_id)
        .bind(session_generation_id)
        .bind(rehydrated_from_checkpoint_artifact_id)
        .bind(invocation_owner_key)
        .bind(session_reuse_disposition)
        .bind(session_reset_reason)
        .bind(id.to_string())
    )?;
    Ok(())
}

pub async fn append_xcode_runtime_observation(
    pool: &SqlitePool,
    id: AgentExecutionId,
    update: XcodeRuntimeObservationUpdate,
) -> Result<()> {
    for attempt in 0..3 {
        let mut tx = begin_registered_immediate_transaction(
            pool,
            crate::writer::class_a_operation(
                "agent_executions.append_xcode_runtime_observation",
                crate::write_class::WriteLane::CriticalBarrier,
                "agent_executions.append_xcode_runtime_observation",
            ),
            "agent_executions.append_xcode_runtime_observation",
        )
        .await?;
        let row = sqlx::query(
            "SELECT actual_xcode_runtime_observation_json FROM agent_executions WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&mut **tx)
        .await?;

        let Some(row) = row else {
            return Err(anyhow!("Agent execution not found: {id}"));
        };

        let current_json: Option<String> = row.get("actual_xcode_runtime_observation_json");
        let mut observation = current_json
            .as_deref()
            .map(|json| {
                serde_json::from_str::<XcodeRuntimeObservation>(json).unwrap_or_else(|error| {
                    tracing::error!(
                        agent_execution_id = %id,
                        error = %error,
                        "Recovering corrupt Xcode runtime observation JSON"
                    );
                    let mut observation = XcodeRuntimeObservation::default();
                    observation.record_corrupt_json_recovery(json.len());
                    observation
                })
            })
            .unwrap_or_default();

        observation.apply_update(update.clone());
        observation.apply_default_storage_bounds()?;
        let next_json = serde_json::to_string(&observation)?;

        let result = if let Some(current_json) = current_json.as_deref() {
            sqlx::query(
                "UPDATE agent_executions
                 SET actual_xcode_runtime_observation_json = ?
                 WHERE id = ? AND actual_xcode_runtime_observation_json = ?",
            )
            .bind(&next_json)
            .bind(id.to_string())
            .bind(current_json)
            .execute(&mut **tx)
            .await?
        } else {
            sqlx::query(
                "UPDATE agent_executions
                 SET actual_xcode_runtime_observation_json = ?
                 WHERE id = ? AND actual_xcode_runtime_observation_json IS NULL",
            )
            .bind(&next_json)
            .bind(id.to_string())
            .execute(&mut **tx)
            .await?
        };

        if result.rows_affected() == 1 {
            tx.commit().await?;
            return Ok(());
        }

        tx.rollback().await?;
        tracing::warn!(
            agent_execution_id = %id,
            attempt = attempt + 1,
            "Retrying contended Xcode runtime observation append"
        );
    }

    Err(anyhow!(
        "Failed to append Xcode runtime observation after optimistic retries: {id}"
    ))
}

pub async fn find_by_stage(
    pool: &SqlitePool,
    stage_execution_id: StageExecutionId,
) -> Result<Vec<AgentExecution>> {
    let query = format!(
        "SELECT {SELECT_COLS} FROM agent_executions WHERE stage_execution_id = ? ORDER BY started_at ASC"
    );
    let rows = sqlx::query(&query)
        .bind(stage_execution_id.to_string())
        .fetch_all(pool)
        .await?;

    rows.iter().map(parse_agent_execution_row).collect()
}

pub async fn find_by_stage_tx(
    tx: &mut Transaction<'_, Sqlite>,
    stage_execution_id: StageExecutionId,
) -> Result<Vec<AgentExecution>> {
    let query = format!(
        "SELECT {SELECT_COLS} FROM agent_executions WHERE stage_execution_id = ? ORDER BY started_at ASC"
    );
    let rows = sqlx::query(&query)
        .bind(stage_execution_id.to_string())
        .fetch_all(&mut **tx)
        .await?;

    rows.iter().map(parse_agent_execution_row).collect()
}

pub async fn list_by_run(pool: &SqlitePool, run_id: RunId) -> Result<Vec<AgentExecution>> {
    // P017: Include both stage-owned and mediation-owned executions for a run.
    let prefixed_select_cols = SELECT_COLS
        .split(',')
        .map(|col| format!("ae.{}", col.trim()))
        .collect::<Vec<_>>()
        .join(", ");
    let query = format!(
        "SELECT {prefixed_select_cols}
         FROM agent_executions ae
         LEFT JOIN stage_executions se ON se.id = ae.stage_execution_id
         LEFT JOIN lead_conflict_mediations lcm
             ON ae.owner_kind = 'lead_conflict_mediation'
             AND ae.lead_mediation_record_id = lcm.id
         WHERE se.run_id = ? OR lcm.run_id = ?
         ORDER BY ae.started_at ASC"
    );
    let rows = sqlx::query(&query)
        .bind(run_id.to_string())
        .bind(run_id.to_string())
        .fetch_all(pool)
        .await?;

    rows.iter().map(parse_agent_execution_row).collect()
}

pub async fn list_by_run_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
) -> Result<Vec<AgentExecution>> {
    // P017: Include both stage-owned and mediation-owned executions for a run.
    let prefixed_select_cols = SELECT_COLS
        .split(',')
        .map(|col| format!("ae.{}", col.trim()))
        .collect::<Vec<_>>()
        .join(", ");
    let query = format!(
        "SELECT {prefixed_select_cols}
         FROM agent_executions ae
         LEFT JOIN stage_executions se ON se.id = ae.stage_execution_id
         LEFT JOIN lead_conflict_mediations lcm
             ON ae.owner_kind = 'lead_conflict_mediation'
             AND ae.lead_mediation_record_id = lcm.id
         WHERE se.run_id = ? OR lcm.run_id = ?
         ORDER BY ae.started_at ASC"
    );
    let rows = sqlx::query(&query)
        .bind(run_id.to_string())
        .bind(run_id.to_string())
        .fetch_all(&mut **tx)
        .await?;

    rows.iter().map(parse_agent_execution_row).collect()
}

/// List every `agent_executions` row owned by the given mediation record,
/// ordered by `started_at` ASC (so callers can index attempts as 1..N).
///
/// API-001 (P017 R2 audit): the workflow-conflict mediation readback must
/// expose conflict-scoped execution attempts. This focused query avoids
/// the joinful `list_by_run` and returns only mediation-owned rows for a
/// single mediation, letting MCP/GraphQL build `execution_attempts`
/// without scanning the whole run.
pub async fn list_by_mediation_id(
    pool: &SqlitePool,
    mediation_id: &str,
) -> Result<Vec<AgentExecution>> {
    let query = format!(
        "SELECT {SELECT_COLS}
         FROM agent_executions
         WHERE owner_kind = 'lead_conflict_mediation'
           AND lead_mediation_record_id = ?
         ORDER BY started_at ASC"
    );
    let rows = sqlx::query(&query)
        .bind(mediation_id)
        .fetch_all(pool)
        .await
        .context("list agent_executions by mediation_id")?;
    rows.iter().map(parse_agent_execution_row).collect()
}

pub async fn cancel_running_by_run(
    pool: &SqlitePool,
    run_id: RunId,
    completed_at: DateTime<Utc>,
) -> Result<()> {
    // BLK-003: Cancel both stage-owned and mediation-owned executions for the run.
    crate::execute_repository_write!(
        pool,
        "agent_executions.cancel_running_by_run",
        sqlx::query(
            "UPDATE agent_executions
         SET status = ?, completed_at = ?
         WHERE status = ? AND (
             stage_execution_id IN (
                 SELECT id FROM stage_executions WHERE run_id = ?
             )
             OR (
                 owner_kind = 'lead_conflict_mediation'
                 AND lead_mediation_record_id IN (
                     SELECT id FROM lead_conflict_mediations WHERE run_id = ?
                 )
             )
         )",
        )
        .bind(AgentStatus::Cancelled.to_string())
        .bind(completed_at.to_rfc3339())
        .bind(AgentStatus::Running.to_string())
        .bind(run_id.to_string())
        .bind(run_id.to_string())
    )?;
    Ok(())
}

pub async fn cancel_running_by_run_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
    completed_at: DateTime<Utc>,
) -> Result<u64> {
    // BLK-003: Cancel both stage-owned and mediation-owned executions for the run.
    let result = sqlx::query(
        "UPDATE agent_executions
         SET status = ?, completed_at = ?
         WHERE status = ? AND (
             stage_execution_id IN (
                 SELECT id FROM stage_executions WHERE run_id = ?
             )
             OR (
                 owner_kind = 'lead_conflict_mediation'
                 AND lead_mediation_record_id IN (
                     SELECT id FROM lead_conflict_mediations WHERE run_id = ?
                 )
             )
         )",
    )
    .bind(AgentStatus::Cancelled.to_string())
    .bind(completed_at.to_rfc3339())
    .bind(AgentStatus::Running.to_string())
    .bind(run_id.to_string())
    .bind(run_id.to_string())
    .execute(&mut **tx)
    .await?;
    Ok(result.rows_affected())
}

pub async fn cancel_running_by_stage_tx(
    tx: &mut Transaction<'_, Sqlite>,
    stage_execution_id: StageExecutionId,
    completed_at: DateTime<Utc>,
) -> Result<u64> {
    let result = sqlx::query(
        "UPDATE agent_executions
         SET status = ?, completed_at = ?
         WHERE status = ? AND stage_execution_id = ?",
    )
    .bind(AgentStatus::Cancelled.to_string())
    .bind(completed_at.to_rfc3339())
    .bind(AgentStatus::Running.to_string())
    .bind(stage_execution_id.to_string())
    .execute(&mut **tx)
    .await?;
    Ok(result.rows_affected())
}

pub async fn list_running_across_interval(
    pool: &SqlitePool,
    started_at: DateTime<Utc>,
    ended_at: DateTime<Utc>,
) -> Result<Vec<RunningAgentExecution>> {
    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "agent_executions.list_running_across_interval",
    )
    .await?;
    let executions = list_running_across_interval_tx(&mut tx, started_at, ended_at).await?;
    tx.commit().await?;
    Ok(executions)
}

pub async fn list_running_across_interval_tx(
    tx: &mut Transaction<'_, Sqlite>,
    started_at: DateTime<Utc>,
    ended_at: DateTime<Utc>,
) -> Result<Vec<RunningAgentExecution>> {
    let rows = sqlx::query(
        r#"SELECT ae.id AS id,
                  se.run_id AS run_id,
                  se.stage_id AS stage_id,
                  ae.stage_execution_id AS stage_execution_id,
                  COALESCE(ae.provider_family, ae.provider) AS provider_family,
                  ae.session_generation_id AS session_generation_id
           FROM agent_executions ae
           INNER JOIN stage_executions se ON se.id = ae.stage_execution_id
           WHERE ae.status = ?1
             AND ae.started_at <= ?2
             AND (ae.completed_at IS NULL OR ae.completed_at >= ?3)
           ORDER BY ae.started_at ASC, ae.id ASC"#,
    )
    .bind(AgentStatus::Running.to_string())
    .bind(ended_at.to_rfc3339())
    .bind(started_at.to_rfc3339())
    .fetch_all(&mut **tx)
    .await?;

    rows.into_iter()
        .map(|row| parse_running_agent_execution_row(&row))
        .collect()
}

fn parse_running_agent_execution_row(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<RunningAgentExecution> {
    let id: String = row.get("id");
    let run_id: String = row.get("run_id");
    let stage_execution_id: String = row.get("stage_execution_id");
    let raw_provider_family: Option<String> = row.get("provider_family");
    Ok(RunningAgentExecution {
        id: id.parse().map_err(|e| anyhow::anyhow!("{}", e))?,
        run_id: run_id.parse().map_err(|e| anyhow::anyhow!("{}", e))?,
        stage_id: row.get("stage_id"),
        stage_execution_id: stage_execution_id
            .parse()
            .map_err(|e| anyhow::anyhow!("{}", e))?,
        provider_family: raw_provider_family
            .as_deref()
            .and_then(ProviderFamily::canonicalize_known_alias),
        session_generation_id: row.get("session_generation_id"),
    })
}

fn parse_agent_execution_row(row: &sqlx::sqlite::SqliteRow) -> Result<AgentExecution> {
    let id: String = row.get("id");
    let seid: Option<String> = row.get("stage_execution_id");
    let status_str: String = row.get("status");
    let started_str: String = row.get("started_at");
    let completed_str: Option<String> = row.get("completed_at");

    Ok(AgentExecution {
        id: id.parse().map_err(|e| anyhow::anyhow!("{}", e))?,
        stage_execution_id: seid
            .map(|id| id.parse().map_err(|e| anyhow::anyhow!("{}", e)))
            .transpose()?,
        agent_id: row.get("agent_id"),
        provider: row.get("provider"),
        model: row.get("model"),
        status: status_str.parse().map_err(|e| anyhow::anyhow!("{}", e))?,
        started_at: started_str.parse::<DateTime<Utc>>()?,
        completed_at: completed_str
            .map(|s| s.parse::<DateTime<Utc>>())
            .transpose()?,
        owner_execution_lineage_id: row.get("owner_execution_lineage_id"),
        session_lineage_id: row.get("session_lineage_id"),
        session_generation_id: row.get("session_generation_id"),
        rehydrated_from_checkpoint_artifact_id: row.get("rehydrated_from_checkpoint_artifact_id"),
        invocation_owner_key: row.get("invocation_owner_key"),
        session_reuse_scope: row.get("session_reuse_scope"),
        session_family_id: row.get("session_family_id"),
        session_reuse_disposition: row.get("session_reuse_disposition"),
        session_reset_reason: row.get("session_reset_reason"),
        backend_profile_id: row.get("backend_profile_id"),
        requested_mcp_extensions_json: row.get("requested_mcp_extensions_json"),
        predicted_mcp_extensions_json: row.get("predicted_mcp_extensions_json"),
        predicted_mcp_runtime_ids_json: row.get("predicted_mcp_runtime_ids_json"),
        actual_mcp_extensions_json: row.get("actual_mcp_extensions_json"),
        actual_mcp_runtime_ids_json: row.get("actual_mcp_runtime_ids_json"),
        denied_mcp_extensions_json: row.get("denied_mcp_extensions_json"),
        mcp_blocking_issues_json: row.get("mcp_blocking_issues_json"),
        actual_mcp_observation_json: row.get("actual_mcp_observation_json"),
        actual_xcode_runtime_observation_json: row.get("actual_xcode_runtime_observation_json"),
        mcp_session_startup_latency_ms: row.get("mcp_session_startup_latency_ms"),
        owner_kind: row.get("owner_kind"),
        owner_id: row.get("owner_id"),
        lead_mediation_record_id: row.get("lead_mediation_record_id"),
        origin_stage_execution_id: row.get("origin_stage_execution_id"),
        // P017 R4 / API-002: per-attempt cost & transcript attribution.
        // Migration 031 added these columns NULLABLE so existing rows
        // backfill as None and only freshly-completed attempts populate.
        total_cost_cents: row.get("total_cost_cents"),
        input_tokens: row.get("input_tokens"),
        output_tokens: row.get("output_tokens"),
        cached_input_tokens: row.get("cached_input_tokens"),
        transcript_artifact_id: row.get("transcript_artifact_id"),
        // P066: toolchain cache mapping diagnostics (nullable; legacy rows → None)
        actual_toolchain_mapping_diagnostics_json: row
            .try_get("actual_toolchain_mapping_diagnostics_json")
            .unwrap_or(None),
        // P058: escalation attribution (nullable; NULL for non-escalated executions)
        escalation_policy_id: row.try_get("escalation_policy_id").unwrap_or(None),
        escalation_policy_hash: row.try_get("escalation_policy_hash").unwrap_or(None),
        escalation_tier_id: row.try_get("escalation_tier_id").unwrap_or(None),
        escalation_tier_kind_raw: row.try_get("escalation_tier_kind_raw").unwrap_or(None),
        escalation_trigger_raw: row.try_get("escalation_trigger_raw").unwrap_or(None),
        escalation_digest_version: row.try_get("escalation_digest_version").unwrap_or(None),
        escalation_ledger_id: row.try_get("escalation_ledger_id").unwrap_or(None),
    })
}

/// P017 R4 / API-002: persist per-attempt cost and transcript attribution
/// for a completed `agent_executions` row. Used by the executor's
/// mediation-completion path so MCP/GraphQL `execution_attempts.cost`
/// and `transcript_ref` resolve to non-null values.
///
/// Idempotent at the row level — re-running with the same values
/// produces the same result. Stage-owned executions can also call this
/// when usage data is available.
pub async fn update_attempt_attribution_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: AgentExecutionId,
    total_cost_cents: Option<i64>,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    cached_input_tokens: Option<i64>,
    transcript_artifact_id: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "UPDATE agent_executions
         SET total_cost_cents = COALESCE(?, total_cost_cents),
             input_tokens = COALESCE(?, input_tokens),
             output_tokens = COALESCE(?, output_tokens),
             cached_input_tokens = COALESCE(?, cached_input_tokens),
             transcript_artifact_id = COALESCE(?, transcript_artifact_id)
         WHERE id = ?",
    )
    .bind(total_cost_cents)
    .bind(input_tokens)
    .bind(output_tokens)
    .bind(cached_input_tokens)
    .bind(transcript_artifact_id)
    .bind(id.to_string())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn update_attempt_attribution(
    pool: &SqlitePool,
    id: AgentExecutionId,
    total_cost_cents: Option<i64>,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    cached_input_tokens: Option<i64>,
    transcript_artifact_id: Option<&str>,
) -> Result<()> {
    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "agent_executions.update_attempt_attribution",
    )
    .await?;
    update_attempt_attribution_tx(
        &mut tx,
        id,
        total_cost_cents,
        input_tokens,
        output_tokens,
        cached_input_tokens,
        transcript_artifact_id,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// P066: Write the toolchain mapping diagnostics JSON document for a completed
/// execution attempt. Called by the adapter/host-executor path after mapping
/// setup completes (or fails). Idempotent — overwrites any prior value.
pub async fn update_toolchain_mapping_diagnostics(
    pool: &SqlitePool,
    id: AgentExecutionId,
    diagnostics_json: &str,
) -> Result<()> {
    crate::execute_repository_write!(
        pool,
        "agent_executions.update_toolchain_mapping_diagnostics",
        sqlx::query(
            "UPDATE agent_executions
         SET actual_toolchain_mapping_diagnostics_json = ?
         WHERE id = ?",
        )
        .bind(diagnostics_json)
        .bind(id.to_string())
    )?;
    Ok(())
}
