use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use domain::discovery::{
    LegacyBroadDiscoveryPolicy, LegacyDiscoveryOverride, LegacyDiscoveryOverrideInput,
    LegacyDiscoveryOverrideStatus,
};
use domain::ids::{RunId, StageExecutionId};

pub async fn create_for_pending_retry_tx(
    tx: &mut Transaction<'_, Sqlite>,
    input: &LegacyDiscoveryOverrideInput,
) -> Result<LegacyDiscoveryOverride> {
    validate_input(input)?;
    let pending_stage_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM stage_executions
           WHERE id = ?1 AND run_id = ?2 AND stage_id = ?3
             AND attempt_number = ?4 AND status = 'pending'"#,
    )
    .bind(input.target_stage_execution_id.to_string())
    .bind(input.run_id.to_string())
    .bind(&input.stage_id)
    .bind(input.target_attempt_number)
    .fetch_one(&mut **tx)
    .await?;
    if pending_stage_count != 1 {
        anyhow::bail!(
            "legacy discovery override target must be a pending retry attempt bound to run {}, stage {}, attempt {}",
            input.run_id,
            input.stage_id,
            input.target_attempt_number
        );
    }

    let existing_pending: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM legacy_discovery_overrides
           WHERE target_stage_execution_id = ?1
             AND target_attempt_number = ?2
             AND status = 'pending'"#,
    )
    .bind(input.target_stage_execution_id.to_string())
    .bind(input.target_attempt_number)
    .fetch_one(&mut **tx)
    .await?;
    if existing_pending != 0 {
        anyhow::bail!(
            "legacy discovery override already exists for stage execution {} attempt {}",
            input.target_stage_execution_id,
            input.target_attempt_number
        );
    }

    let now = Utc::now();
    let override_id = uuid::Uuid::new_v4().to_string();
    let idempotency_key = format!(
        "{}:{}:{}:{}",
        input.run_id,
        input.stage_id,
        input.target_attempt_number,
        policy_to_str(input.requested_policy)
    );
    sqlx::query(
        r#"INSERT INTO legacy_discovery_overrides
           (override_id, run_id, stage_id, workflow_id, target_stage_execution_id,
            target_attempt_number, actor_id, reason, created_at, expires_at_attempt,
            requested_policy, from_policy, approval_source, idempotency_key,
            consumed_at, status, journal_id)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, NULL, 'pending', ?15)"#,
    )
    .bind(&override_id)
    .bind(input.run_id.to_string())
    .bind(&input.stage_id)
    .bind(&input.workflow_id)
    .bind(input.target_stage_execution_id.to_string())
    .bind(input.target_attempt_number)
    .bind(&input.actor_id)
    .bind(input.reason.trim())
    .bind(now.to_rfc3339())
    .bind(input.target_attempt_number)
    .bind(policy_to_str(input.requested_policy))
    .bind(policy_to_str(input.from_policy))
    .bind(&input.approval_source)
    .bind(&idempotency_key)
    .bind(&input.journal_id)
    .execute(&mut **tx)
    .await?;

    Ok(LegacyDiscoveryOverride {
        override_id,
        run_id: input.run_id,
        stage_id: input.stage_id.clone(),
        workflow_id: input.workflow_id.clone(),
        target_stage_execution_id: input.target_stage_execution_id,
        target_attempt_number: input.target_attempt_number,
        actor_id: input.actor_id.clone(),
        reason: input.reason.trim().to_string(),
        created_at: now,
        expires_at_attempt: input.target_attempt_number,
        requested_policy: input.requested_policy,
        from_policy: input.from_policy,
        approval_source: input.approval_source.clone(),
        idempotency_key,
        consumed_at: None,
        status: LegacyDiscoveryOverrideStatus::Pending,
        journal_id: input.journal_id.clone(),
    })
}

pub async fn consume_pending_for_stage_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
    stage_id: &str,
    target_stage_execution_id: StageExecutionId,
    target_attempt_number: i64,
) -> Result<Option<LegacyDiscoveryOverride>> {
    let row = sqlx::query(
        r#"SELECT override_id, run_id, stage_id, workflow_id, target_stage_execution_id,
                  target_attempt_number, actor_id, reason, created_at, expires_at_attempt,
                  requested_policy, from_policy, approval_source, idempotency_key,
                  consumed_at, status, journal_id
           FROM legacy_discovery_overrides
           WHERE run_id = ?1
             AND stage_id = ?2
             AND target_stage_execution_id = ?3
             AND target_attempt_number = ?4
             AND status = 'pending'
           ORDER BY created_at ASC
           LIMIT 1"#,
    )
    .bind(run_id.to_string())
    .bind(stage_id)
    .bind(target_stage_execution_id.to_string())
    .bind(target_attempt_number)
    .fetch_optional(&mut **tx)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };
    let mut override_row = parse_row(&row)?;
    let now = Utc::now();
    if override_row.expires_at_attempt != target_attempt_number
        || override_row.requested_policy != LegacyBroadDiscoveryPolicy::WorkflowOptIn
    {
        sqlx::query(
            "UPDATE legacy_discovery_overrides SET status = ?1 WHERE override_id = ?2 AND status = 'pending'",
        )
        .bind(if override_row.expires_at_attempt != target_attempt_number {
            "expired"
        } else {
            "rejected"
        })
        .bind(&override_row.override_id)
        .execute(&mut **tx)
        .await?;
        return Ok(None);
    }

    let rows_affected = sqlx::query(
        r#"UPDATE legacy_discovery_overrides
           SET status = 'consumed', consumed_at = ?1
           WHERE override_id = ?2 AND status = 'pending'"#,
    )
    .bind(now.to_rfc3339())
    .bind(&override_row.override_id)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if rows_affected != 1 {
        anyhow::bail!(
            "legacy discovery override {} was not consumed",
            override_row.override_id
        );
    }
    override_row.consumed_at = Some(now);
    override_row.status = LegacyDiscoveryOverrideStatus::Consumed;
    Ok(Some(override_row))
}

pub async fn list_by_run(pool: &SqlitePool, run_id: RunId) -> Result<Vec<LegacyDiscoveryOverride>> {
    let rows = sqlx::query(
        r#"SELECT override_id, run_id, stage_id, workflow_id, target_stage_execution_id,
                  target_attempt_number, actor_id, reason, created_at, expires_at_attempt,
                  requested_policy, from_policy, approval_source, idempotency_key,
                  consumed_at, status, journal_id
           FROM legacy_discovery_overrides
           WHERE run_id = ?1
           ORDER BY created_at ASC"#,
    )
    .bind(run_id.to_string())
    .fetch_all(pool)
    .await?;
    rows.iter().map(parse_row).collect()
}

fn validate_input(input: &LegacyDiscoveryOverrideInput) -> Result<()> {
    if input.requested_policy != LegacyBroadDiscoveryPolicy::WorkflowOptIn {
        anyhow::bail!("legacy discovery override only supports workflow_opt_in");
    }
    if input.from_policy.allows_broad_discovery() {
        anyhow::bail!(
            "legacy discovery override is unnecessary when frozen policy already opts in"
        );
    }
    if input.reason.trim().is_empty() {
        anyhow::bail!("legacy discovery override reason is required");
    }
    if input.actor_id.trim().is_empty() {
        anyhow::bail!("legacy discovery override actor_id is required");
    }
    if input.approval_source.trim().is_empty() {
        anyhow::bail!("legacy discovery override approval_source is required");
    }
    if input.journal_id.trim().is_empty() {
        anyhow::bail!("legacy discovery override journal_id is required");
    }
    Ok(())
}

fn parse_row(row: &sqlx::sqlite::SqliteRow) -> Result<LegacyDiscoveryOverride> {
    let run_id: String = row.get("run_id");
    let target_stage_execution_id: String = row.get("target_stage_execution_id");
    let created_at: String = row.get("created_at");
    let consumed_at: Option<String> = row.get("consumed_at");
    let requested_policy: String = row.get("requested_policy");
    let from_policy: String = row.get("from_policy");
    let status: String = row.get("status");
    Ok(LegacyDiscoveryOverride {
        override_id: row.get("override_id"),
        run_id: run_id.parse().map_err(|e| anyhow!("{}", e))?,
        stage_id: row.get("stage_id"),
        workflow_id: row.get("workflow_id"),
        target_stage_execution_id: target_stage_execution_id
            .parse()
            .map_err(|e| anyhow!("{}", e))?,
        target_attempt_number: row.get("target_attempt_number"),
        actor_id: row.get("actor_id"),
        reason: row.get("reason"),
        created_at: DateTime::parse_from_rfc3339(&created_at)
            .context("parse legacy discovery override created_at")?
            .with_timezone(&Utc),
        expires_at_attempt: row.get("expires_at_attempt"),
        requested_policy: parse_policy(&requested_policy)?,
        from_policy: parse_policy(&from_policy)?,
        approval_source: row.get("approval_source"),
        idempotency_key: row.get("idempotency_key"),
        consumed_at: consumed_at
            .map(|value| DateTime::parse_from_rfc3339(&value).map(|dt| dt.with_timezone(&Utc)))
            .transpose()
            .context("parse legacy discovery override consumed_at")?,
        status: parse_status(&status)?,
        journal_id: row.get("journal_id"),
    })
}

fn parse_policy(value: &str) -> Result<LegacyBroadDiscoveryPolicy> {
    match value {
        "disabled" => Ok(LegacyBroadDiscoveryPolicy::Disabled),
        "workflow_opt_in" => Ok(LegacyBroadDiscoveryPolicy::WorkflowOptIn),
        _ => Err(anyhow!("unknown legacy discovery policy: {value}")),
    }
}

fn policy_to_str(policy: LegacyBroadDiscoveryPolicy) -> &'static str {
    match policy {
        LegacyBroadDiscoveryPolicy::Disabled => "disabled",
        LegacyBroadDiscoveryPolicy::WorkflowOptIn => "workflow_opt_in",
    }
}

fn parse_status(value: &str) -> Result<LegacyDiscoveryOverrideStatus> {
    match value {
        "pending" => Ok(LegacyDiscoveryOverrideStatus::Pending),
        "consumed" => Ok(LegacyDiscoveryOverrideStatus::Consumed),
        "rejected" => Ok(LegacyDiscoveryOverrideStatus::Rejected),
        "expired" => Ok(LegacyDiscoveryOverrideStatus::Expired),
        _ => Err(anyhow!("unknown legacy discovery override status: {value}")),
    }
}
