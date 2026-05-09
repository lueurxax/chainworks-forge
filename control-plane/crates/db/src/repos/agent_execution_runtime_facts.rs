use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use domain::agent::{
    AgentExecutionRuntimeFacts, AgentFailureKind, AgentOutputSettlement, OperatorActionHint,
};
use domain::ids::{AgentExecutionId, RunId};

pub async fn upsert(pool: &SqlitePool, facts: &AgentExecutionRuntimeFacts) -> Result<()> {
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "agent_execution_runtime_facts.upsert")
            .await?;
    upsert_tx(&mut tx, facts).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn upsert_tx(
    tx: &mut Transaction<'_, Sqlite>,
    facts: &AgentExecutionRuntimeFacts,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO agent_execution_runtime_facts
           (agent_execution_id, failure_kind, failure_kind_raw_debug, failure_kind_version,
            failure_message_redacted, failure_message_redaction_version, retry_after,
            operator_action_hint, provider_exit_status, transport_error_code,
            supervision_classification, output_settlement, valid_required_outputs,
            late_output_count, ignored_late_output_count, session_reuse_reason,
            quota_ledger_id, created_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
           ON CONFLICT(agent_execution_id) DO UPDATE SET
             failure_kind = excluded.failure_kind,
             failure_kind_raw_debug = excluded.failure_kind_raw_debug,
             failure_kind_version = excluded.failure_kind_version,
             failure_message_redacted = excluded.failure_message_redacted,
             failure_message_redaction_version = excluded.failure_message_redaction_version,
             retry_after = excluded.retry_after,
             operator_action_hint = excluded.operator_action_hint,
             provider_exit_status = excluded.provider_exit_status,
             transport_error_code = excluded.transport_error_code,
             supervision_classification = excluded.supervision_classification,
             output_settlement = excluded.output_settlement,
             valid_required_outputs = excluded.valid_required_outputs,
             late_output_count = excluded.late_output_count,
             ignored_late_output_count = excluded.ignored_late_output_count,
             session_reuse_reason = COALESCE(excluded.session_reuse_reason, agent_execution_runtime_facts.session_reuse_reason),
             quota_ledger_id = COALESCE(excluded.quota_ledger_id, agent_execution_runtime_facts.quota_ledger_id),
             updated_at = excluded.updated_at"#,
    )
    .bind(facts.agent_execution_id.to_string())
    .bind(facts.failure_kind.as_ref().map(ToString::to_string))
    .bind(&facts.failure_kind_raw_debug)
    .bind(facts.failure_kind_version)
    .bind(&facts.failure_message_redacted)
    .bind(facts.failure_message_redaction_version)
    .bind(facts.retry_after.map(|dt| dt.to_rfc3339()))
    .bind(facts.operator_action_hint.as_ref().map(ToString::to_string))
    .bind(facts.provider_exit_status)
    .bind(&facts.transport_error_code)
    .bind(&facts.supervision_classification)
    .bind(facts.output_settlement.to_string())
    .bind(if facts.valid_required_outputs { 1_i64 } else { 0_i64 })
    .bind(facts.late_output_count)
    .bind(facts.ignored_late_output_count)
    .bind(&facts.session_reuse_reason)
    .bind(&facts.quota_ledger_id)
    .bind(facts.created_at.to_rfc3339())
    .bind(facts.updated_at.to_rfc3339())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn find_by_execution_id(
    pool: &SqlitePool,
    agent_execution_id: AgentExecutionId,
) -> Result<Option<AgentExecutionRuntimeFacts>> {
    let row = sqlx::query(
        r#"SELECT agent_execution_id, failure_kind, failure_kind_raw_debug,
                  failure_kind_version, failure_message_redacted,
                  failure_message_redaction_version, retry_after, operator_action_hint,
                  provider_exit_status, transport_error_code, supervision_classification,
                  output_settlement, valid_required_outputs, late_output_count,
                  ignored_late_output_count, session_reuse_reason, quota_ledger_id,
                  created_at, updated_at
           FROM agent_execution_runtime_facts WHERE agent_execution_id = ?1"#,
    )
    .bind(agent_execution_id.to_string())
    .fetch_optional(pool)
    .await?;
    row.map(|row| parse_runtime_facts_row(&row)).transpose()
}

pub async fn list_by_run(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<Vec<AgentExecutionRuntimeFacts>> {
    // P017: Include both stage-owned executions (via stage_executions join)
    // and mediation-owned executions (via lead_conflict_mediations join).
    let rows = sqlx::query(
        r#"SELECT arf.agent_execution_id, arf.failure_kind, arf.failure_kind_raw_debug,
                  arf.failure_kind_version, arf.failure_message_redacted,
                  arf.failure_message_redaction_version, arf.retry_after,
                  arf.operator_action_hint, arf.provider_exit_status,
                  arf.transport_error_code, arf.supervision_classification,
                  arf.output_settlement, arf.valid_required_outputs,
                  arf.late_output_count, arf.ignored_late_output_count,
                  arf.session_reuse_reason, arf.quota_ledger_id, arf.created_at, arf.updated_at
           FROM agent_execution_runtime_facts arf
           INNER JOIN agent_executions ae ON ae.id = arf.agent_execution_id
           LEFT JOIN stage_executions se ON se.id = ae.stage_execution_id
           LEFT JOIN lead_conflict_mediations lcm
               ON ae.owner_kind = 'lead_conflict_mediation'
               AND ae.lead_mediation_record_id = lcm.id
           WHERE se.run_id = ?1 OR lcm.run_id = ?1
           ORDER BY ae.started_at ASC"#,
    )
    .bind(run_id.to_string())
    .fetch_all(pool)
    .await?;
    rows.iter().map(parse_runtime_facts_row).collect()
}

fn parse_runtime_facts_row(row: &sqlx::sqlite::SqliteRow) -> Result<AgentExecutionRuntimeFacts> {
    let agent_execution_id: String = row.get("agent_execution_id");
    let failure_kind_raw: Option<String> = row.get("failure_kind");
    let parsed_failure_kind = failure_kind_raw
        .as_deref()
        .map(|s| s.parse::<AgentFailureKind>());
    let failure_kind = parsed_failure_kind
        .as_ref()
        .map(|result| result.clone().unwrap_or(AgentFailureKind::Unknown));
    let stored_failure_kind_raw_debug: Option<String> = row.get("failure_kind_raw_debug");
    let failure_kind_raw_debug = match (&failure_kind_raw, &parsed_failure_kind) {
        (Some(raw), Some(Err(_))) => Some(
            stored_failure_kind_raw_debug
                .clone()
                .unwrap_or_else(|| raw.clone()),
        ),
        _ => stored_failure_kind_raw_debug,
    };
    let action_hint_raw: Option<String> = row.get("operator_action_hint");
    let output_settlement_raw: String = row.get("output_settlement");
    let retry_after_raw: Option<String> = row.get("retry_after");
    let created_at_raw: String = row.get("created_at");
    let updated_at_raw: String = row.get("updated_at");
    Ok(AgentExecutionRuntimeFacts {
        agent_execution_id: agent_execution_id
            .parse()
            .map_err(|e| anyhow::anyhow!("{e}"))?,
        failure_kind,
        failure_kind_raw_debug,
        failure_kind_version: row.get("failure_kind_version"),
        failure_message_redacted: row.get("failure_message_redacted"),
        failure_message_redaction_version: row.get("failure_message_redaction_version"),
        retry_after: retry_after_raw
            .map(|s| DateTime::parse_from_rfc3339(&s).map(|dt| dt.with_timezone(&Utc)))
            .transpose()?,
        operator_action_hint: action_hint_raw
            .as_deref()
            .map(|s| s.parse::<OperatorActionHint>())
            .transpose()
            .map_err(anyhow::Error::msg)?,
        provider_exit_status: row.get("provider_exit_status"),
        transport_error_code: row.get("transport_error_code"),
        supervision_classification: row.get("supervision_classification"),
        output_settlement: output_settlement_raw
            .parse::<AgentOutputSettlement>()
            .unwrap_or(AgentOutputSettlement::None),
        valid_required_outputs: row.get::<i64, _>("valid_required_outputs") != 0,
        late_output_count: row.get("late_output_count"),
        ignored_late_output_count: row.get("ignored_late_output_count"),
        session_reuse_reason: row.get("session_reuse_reason"),
        quota_ledger_id: row.get("quota_ledger_id"),
        created_at: DateTime::parse_from_rfc3339(&created_at_raw)?.with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&updated_at_raw)?.with_timezone(&Utc),
    })
}
