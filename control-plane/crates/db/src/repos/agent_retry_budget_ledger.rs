use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use domain::agent::AgentFailureKind;
use domain::ids::{AgentExecutionId, RunId, StageExecutionId};
use domain::mediation::OwnerKind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentRetryBudgetLedgerRow {
    pub id: String,
    pub run_id: RunId,
    pub owner_kind: OwnerKind,
    pub owner_id: String,
    pub stage_execution_id: Option<StageExecutionId>,
    pub agent_execution_id: AgentExecutionId,
    pub failure_kind: AgentFailureKind,
    pub retry_after: Option<DateTime<Utc>>,
    pub normal_budget_consumed: bool,
    pub early_retry_journal_id: Option<String>,
    pub idempotency_key: String,
    pub state: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderFamilyQuotaWait {
    pub ledger_id: String,
    pub run_id: RunId,
    pub stage_execution_id: Option<StageExecutionId>,
    pub agent_execution_id: AgentExecutionId,
    pub retry_after: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderFamilyQuotaConsumeResult {
    pub source_command_journal_id: String,
    pub consumed_count: u64,
}

pub async fn upsert_quota_failure(
    pool: &SqlitePool,
    run_id: RunId,
    stage_execution_id: StageExecutionId,
    agent_execution_id: AgentExecutionId,
    retry_after: Option<DateTime<Utc>>,
) -> Result<AgentRetryBudgetLedgerRow> {
    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "agent_retry_budget_ledger.upsert_quota_failure",
    )
    .await?;
    let row = upsert_quota_failure_tx(
        &mut tx,
        run_id,
        stage_execution_id,
        agent_execution_id,
        retry_after,
    )
    .await?;
    tx.commit().await?;
    Ok(row)
}

pub async fn upsert_quota_failure_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
    stage_execution_id: StageExecutionId,
    agent_execution_id: AgentExecutionId,
    retry_after: Option<DateTime<Utc>>,
) -> Result<AgentRetryBudgetLedgerRow> {
    upsert_quota_failure_for_owner_tx(
        tx,
        run_id,
        OwnerKind::StageExecution,
        stage_execution_id.to_string(),
        Some(stage_execution_id),
        agent_execution_id,
        retry_after,
    )
    .await
}

pub async fn upsert_quota_failure_for_owner_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
    owner_kind: OwnerKind,
    owner_id: String,
    stage_execution_id: Option<StageExecutionId>,
    agent_execution_id: AgentExecutionId,
    retry_after: Option<DateTime<Utc>>,
) -> Result<AgentRetryBudgetLedgerRow> {
    let failure_kind = AgentFailureKind::ProviderQuota;
    let retry_after_key = retry_after
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| "none".to_string());
    let idempotency_key = format!(
        "{}:{}:{}:{}:{}:{}",
        run_id, owner_kind, owner_id, agent_execution_id, retry_after_key, failure_kind
    );
    let now = Utc::now();
    let state = match retry_after {
        Some(dt) if dt > now => "waiting_for_reset",
        _ => "reset_elapsed",
    };

    sqlx::query(
        r#"INSERT INTO agent_retry_budget_ledger
           (id, run_id, owner_kind, owner_id, stage_execution_id, agent_execution_id, failure_kind, retry_after,
            normal_budget_consumed, early_retry_journal_id, idempotency_key, state, created_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, NULL, ?9, ?10, ?11, ?11)
           ON CONFLICT(idempotency_key) DO UPDATE SET
             retry_after = excluded.retry_after,
             state = excluded.state,
             updated_at = excluded.updated_at"#,
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(run_id.to_string())
    .bind(owner_kind.to_string())
    .bind(&owner_id)
    .bind(stage_execution_id.map(|id| id.to_string()))
    .bind(agent_execution_id.to_string())
    .bind(failure_kind.to_string())
    .bind(retry_after.map(|dt| dt.to_rfc3339()))
    .bind(&idempotency_key)
    .bind(state)
    .bind(now.to_rfc3339())
    .execute(&mut **tx)
    .await?;

    find_by_idempotency_key_tx(tx, &idempotency_key).await
}

pub async fn find_by_idempotency_key_tx(
    tx: &mut Transaction<'_, Sqlite>,
    idempotency_key: &str,
) -> Result<AgentRetryBudgetLedgerRow> {
    let row = sqlx::query(
        r#"SELECT id, run_id, stage_execution_id, agent_execution_id, failure_kind,
                  owner_kind, owner_id, retry_after, normal_budget_consumed, early_retry_journal_id,
                  idempotency_key, state, created_at, updated_at
           FROM agent_retry_budget_ledger
           WHERE idempotency_key = ?1"#,
    )
    .bind(idempotency_key)
    .fetch_one(&mut **tx)
    .await?;
    parse_row(&row)
}

pub async fn list_quota_for_stage_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
    stage_execution_id: StageExecutionId,
) -> Result<Vec<AgentRetryBudgetLedgerRow>> {
    list_quota_for_owner_tx(
        tx,
        run_id,
        OwnerKind::StageExecution,
        &stage_execution_id.to_string(),
    )
    .await
}

pub async fn list_quota_for_owner_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
    owner_kind: OwnerKind,
    owner_id: &str,
) -> Result<Vec<AgentRetryBudgetLedgerRow>> {
    let rows = sqlx::query(
        r#"SELECT id, run_id, stage_execution_id, agent_execution_id, failure_kind,
                  owner_kind, owner_id, retry_after, normal_budget_consumed, early_retry_journal_id,
                  idempotency_key, state, created_at, updated_at
           FROM agent_retry_budget_ledger
           WHERE run_id = ?1 AND owner_kind = ?2 AND owner_id = ?3 AND failure_kind = ?4
           ORDER BY created_at ASC"#,
    )
    .bind(run_id.to_string())
    .bind(owner_kind.to_string())
    .bind(owner_id)
    .bind(AgentFailureKind::ProviderQuota.to_string())
    .fetch_all(&mut **tx)
    .await?;
    rows.iter().map(parse_row).collect()
}

pub async fn mark_quota_reset_elapsed_tx(
    tx: &mut Transaction<'_, Sqlite>,
    ledger_id: &str,
) -> Result<AgentRetryBudgetLedgerRow> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"UPDATE agent_retry_budget_ledger
           SET state = 'reset_elapsed',
               updated_at = ?1
           WHERE id = ?2 AND normal_budget_consumed = 0"#,
    )
    .bind(&now)
    .bind(ledger_id)
    .execute(&mut **tx)
    .await?;
    find_by_id_tx(tx, ledger_id).await
}

pub async fn mark_elapsed_provider_quota_waits(
    pool: &SqlitePool,
    now: DateTime<Utc>,
) -> Result<u64> {
    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "agent_retry_budget_ledger.mark_elapsed_provider_quota_waits",
    )
    .await?;
    let updated = mark_elapsed_provider_quota_waits_tx(&mut tx, now).await?;
    tx.commit().await?;
    Ok(updated)
}

pub async fn mark_elapsed_provider_quota_waits_tx(
    tx: &mut Transaction<'_, Sqlite>,
    now: DateTime<Utc>,
) -> Result<u64> {
    let updated = sqlx::query(
        r#"UPDATE agent_retry_budget_ledger
           SET state = 'reset_elapsed',
               updated_at = ?1
           WHERE failure_kind = ?2
             AND normal_budget_consumed = 0
             AND state = 'waiting_for_reset'
             AND retry_after IS NOT NULL
             AND retry_after <= ?1"#,
    )
    .bind(now.to_rfc3339())
    .bind(AgentFailureKind::ProviderQuota.to_string())
    .execute(&mut **tx)
    .await?
    .rows_affected();
    Ok(updated)
}

pub async fn consume_early_quota_retry_tx(
    tx: &mut Transaction<'_, Sqlite>,
    ledger_id: &str,
    journal_id: &str,
) -> Result<AgentRetryBudgetLedgerRow> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"UPDATE agent_retry_budget_ledger
           SET normal_budget_consumed = 1,
               early_retry_journal_id = COALESCE(early_retry_journal_id, ?1),
               state = 'early_retry_consumed',
               updated_at = ?2
           WHERE id = ?3"#,
    )
    .bind(journal_id)
    .bind(&now)
    .bind(ledger_id)
    .execute(&mut **tx)
    .await?;
    find_by_id_tx(tx, ledger_id).await
}

pub async fn active_provider_family_quota_wait(
    pool: &SqlitePool,
    provider_family: &str,
    model: Option<&str>,
    now: DateTime<Utc>,
) -> Result<Option<ProviderFamilyQuotaWait>> {
    mark_elapsed_provider_quota_waits(pool, now).await?;

    let row = sqlx::query(
        r#"SELECT ledger.id, ledger.run_id, ledger.stage_execution_id,
                  ledger.agent_execution_id, ledger.retry_after
           FROM agent_retry_budget_ledger ledger
           INNER JOIN agent_executions ae ON ae.id = ledger.agent_execution_id
           WHERE ledger.failure_kind = ?1
             AND ledger.normal_budget_consumed = 0
             AND ledger.state = 'waiting_for_reset'
             AND ledger.retry_after IS NOT NULL
             AND ledger.retry_after > ?2
             AND COALESCE(ae.provider_family, ae.provider) = ?3
             AND (
               ae.model IS NULL
               OR trim(ae.model) = ''
               OR ?4 IS NULL
               OR trim(?4) = ''
               OR lower(ae.model) = lower(?4)
             )
           ORDER BY ledger.retry_after ASC, ledger.created_at ASC
           LIMIT 1"#,
    )
    .bind(AgentFailureKind::ProviderQuota.to_string())
    .bind(now.to_rfc3339())
    .bind(provider_family)
    .bind(model)
    .fetch_optional(pool)
    .await?;

    row.map(|row| parse_provider_family_quota_wait_row(&row))
        .transpose()
}

pub async fn consume_active_provider_family_quota_for_retry_target(
    pool: &SqlitePool,
    target_stage_execution_id: StageExecutionId,
    provider_family: &str,
    model: Option<&str>,
    now: DateTime<Utc>,
) -> Result<Option<ProviderFamilyQuotaConsumeResult>> {
    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "agent_retry_budget_ledger.consume_provider_family_quota_for_retry_target",
    )
    .await?;
    let result = consume_active_provider_family_quota_for_retry_target_tx(
        &mut tx,
        target_stage_execution_id,
        provider_family,
        model,
        now,
    )
    .await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn consume_active_provider_family_quota_for_retry_target_tx(
    tx: &mut Transaction<'_, Sqlite>,
    target_stage_execution_id: StageExecutionId,
    provider_family: &str,
    model: Option<&str>,
    now: DateTime<Utc>,
) -> Result<Option<ProviderFamilyQuotaConsumeResult>> {
    let authority = sqlx::query(
        r#"SELECT rsa.source_command_journal_id, cj.payload_json
           FROM retry_stage_execution_authorities rsa
           INNER JOIN command_journal cj ON cj.id = rsa.source_command_journal_id
           WHERE rsa.target_stage_execution_id = ?1
             AND rsa.authority_state = 'active'
           LIMIT 1"#,
    )
    .bind(target_stage_execution_id.to_string())
    .fetch_optional(&mut **tx)
    .await?;

    let Some(authority) = authority else {
        return Ok(None);
    };
    let source_command_journal_id: String = authority.get("source_command_journal_id");
    let payload_json: String = authority.get("payload_json");
    let payload: serde_json::Value = serde_json::from_str(&payload_json)?;
    let consume_quota_budget_now = payload
        .get("RetryStage")
        .and_then(|value| value.get("consume_quota_budget_now"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if !consume_quota_budget_now {
        return Ok(None);
    }

    let updated = sqlx::query(
        r#"UPDATE agent_retry_budget_ledger
           SET normal_budget_consumed = 1,
               early_retry_journal_id = COALESCE(early_retry_journal_id, ?1),
               state = 'early_retry_consumed',
               updated_at = ?2
           WHERE failure_kind = ?3
             AND normal_budget_consumed = 0
             AND state = 'waiting_for_reset'
             AND retry_after IS NOT NULL
             AND retry_after > ?2
             AND agent_execution_id IN (
               SELECT id FROM agent_executions
               WHERE COALESCE(provider_family, provider) = ?4
                 AND (
                   model IS NULL
                   OR trim(model) = ''
                   OR ?5 IS NULL
                   OR trim(?5) = ''
                   OR lower(model) = lower(?5)
                 )
             )"#,
    )
    .bind(&source_command_journal_id)
    .bind(now.to_rfc3339())
    .bind(AgentFailureKind::ProviderQuota.to_string())
    .bind(provider_family)
    .bind(model)
    .execute(&mut **tx)
    .await?
    .rows_affected();

    if updated == 0 {
        return Ok(None);
    }

    Ok(Some(ProviderFamilyQuotaConsumeResult {
        source_command_journal_id,
        consumed_count: updated,
    }))
}

pub async fn find_by_id_tx(
    tx: &mut Transaction<'_, Sqlite>,
    ledger_id: &str,
) -> Result<AgentRetryBudgetLedgerRow> {
    let row = sqlx::query(
        r#"SELECT id, run_id, stage_execution_id, agent_execution_id, failure_kind,
                  owner_kind, owner_id, retry_after, normal_budget_consumed, early_retry_journal_id,
                  idempotency_key, state, created_at, updated_at
           FROM agent_retry_budget_ledger
           WHERE id = ?1"#,
    )
    .bind(ledger_id)
    .fetch_one(&mut **tx)
    .await?;
    parse_row(&row)
}

fn parse_provider_family_quota_wait_row(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<ProviderFamilyQuotaWait> {
    let run_id: String = row.get("run_id");
    let stage_execution_id: Option<String> = row.get("stage_execution_id");
    let agent_execution_id: String = row.get("agent_execution_id");
    let retry_after: String = row.get("retry_after");
    Ok(ProviderFamilyQuotaWait {
        ledger_id: row.get("id"),
        run_id: run_id.parse().map_err(|e| anyhow::anyhow!("{e}"))?,
        stage_execution_id: stage_execution_id
            .map(|value| value.parse().map_err(|e| anyhow::anyhow!("{e}")))
            .transpose()?,
        agent_execution_id: agent_execution_id
            .parse()
            .map_err(|e| anyhow::anyhow!("{e}"))?,
        retry_after: DateTime::parse_from_rfc3339(&retry_after)?.with_timezone(&Utc),
    })
}

fn parse_row(row: &sqlx::sqlite::SqliteRow) -> Result<AgentRetryBudgetLedgerRow> {
    let run_id: String = row.get("run_id");
    let owner_kind: String = row.get("owner_kind");
    let stage_execution_id: Option<String> = row.get("stage_execution_id");
    let agent_execution_id: String = row.get("agent_execution_id");
    let failure_kind: String = row.get("failure_kind");
    let retry_after: Option<String> = row.get("retry_after");
    let created_at: String = row.get("created_at");
    let updated_at: String = row.get("updated_at");
    Ok(AgentRetryBudgetLedgerRow {
        id: row.get("id"),
        run_id: run_id.parse().map_err(|e| anyhow::anyhow!("{e}"))?,
        owner_kind: owner_kind.parse().map_err(anyhow::Error::msg)?,
        owner_id: row.get("owner_id"),
        stage_execution_id: stage_execution_id
            .map(|value| value.parse().map_err(|e| anyhow::anyhow!("{e}")))
            .transpose()?,
        agent_execution_id: agent_execution_id
            .parse()
            .map_err(|e| anyhow::anyhow!("{e}"))?,
        failure_kind: failure_kind.parse().unwrap_or(AgentFailureKind::Unknown),
        retry_after: retry_after
            .map(|raw| DateTime::parse_from_rfc3339(&raw).map(|dt| dt.with_timezone(&Utc)))
            .transpose()?,
        normal_budget_consumed: row.get::<i64, _>("normal_budget_consumed") != 0,
        early_retry_journal_id: row.get("early_retry_journal_id"),
        idempotency_key: row.get("idempotency_key"),
        state: row.get("state"),
        created_at: DateTime::parse_from_rfc3339(&created_at)?.with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&updated_at)?.with_timezone(&Utc),
    })
}
