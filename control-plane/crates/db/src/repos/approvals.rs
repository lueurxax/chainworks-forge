use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use domain::approval::{Approval, ApprovalDecision};
use domain::ids::{ApprovalId, RunId};

pub async fn insert(pool: &SqlitePool, approval: &Approval) -> Result<()> {
    let id = approval.id.to_string();
    let run_id = approval.run_id.to_string();
    let decision = approval.decision.to_string();
    let requested_at = approval.requested_at.to_rfc3339();
    let decided_at = approval.decided_at.map(|t| t.to_rfc3339());
    let expires_at = approval.expires_at.map(|t| t.to_rfc3339());

    crate::execute_repository_write!(pool, "approvals.insert", sqlx::query(
        r#"
        INSERT INTO approvals (id, run_id, stage_id, decision, requested_at, decided_at, comment, expires_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(id)
    .bind(run_id)
    .bind(&approval.stage_id)
    .bind(decision)
    .bind(requested_at)
    .bind(decided_at)
    .bind(&approval.comment)
    .bind(expires_at))
    .context("insert approval")?;
    Ok(())
}

pub async fn find_by_id(pool: &SqlitePool, id: ApprovalId) -> Result<Option<Approval>> {
    let id_str = id.to_string();
    let row = sqlx::query(
        r#"SELECT id, run_id, stage_id, decision, requested_at, decided_at, comment, expires_at
           FROM approvals WHERE id = ?1"#,
    )
    .bind(id_str)
    .fetch_optional(pool)
    .await
    .context("find approval by id")?;

    row.map(|r| {
        parse_approval_row(
            r.get("id"),
            r.get("run_id"),
            r.get("stage_id"),
            r.get("decision"),
            r.get("requested_at"),
            r.get("decided_at"),
            r.get("comment"),
            r.get("expires_at"),
        )
    })
    .transpose()
}

/// P072: Transaction-scoped lookup by approval_id for ResolveApproval.
pub async fn find_by_id_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: ApprovalId,
) -> Result<Option<Approval>> {
    let id_str = id.to_string();
    let row = sqlx::query(
        r#"SELECT id, run_id, stage_id, decision, requested_at, decided_at, comment, expires_at
           FROM approvals WHERE id = ?1"#,
    )
    .bind(id_str)
    .fetch_optional(&mut **tx)
    .await
    .context("find approval by id (tx)")?;

    row.map(|r| {
        parse_approval_row(
            r.get("id"),
            r.get("run_id"),
            r.get("stage_id"),
            r.get("decision"),
            r.get("requested_at"),
            r.get("decided_at"),
            r.get("comment"),
            r.get("expires_at"),
        )
    })
    .transpose()
}

/// Find the pending approval for a given run_id and stage_id.
/// Returns the most recently requested pending approval if multiple exist.
pub async fn find_pending_by_run_stage(
    pool: &SqlitePool,
    run_id: RunId,
    stage_id: &str,
) -> Result<Option<Approval>> {
    let run_id_str = run_id.to_string();
    let row = sqlx::query(
        r#"SELECT id, run_id, stage_id, decision, requested_at, decided_at, comment, expires_at
           FROM approvals
           WHERE run_id = ?1 AND stage_id = ?2 AND (decision = 'pending' OR decision = 'requested')
           ORDER BY requested_at DESC
           LIMIT 1"#,
    )
    .bind(run_id_str)
    .bind(stage_id)
    .fetch_optional(pool)
    .await
    .context("find pending approval by run and stage")?;

    row.map(|r| {
        parse_approval_row(
            r.get("id"),
            r.get("run_id"),
            r.get("stage_id"),
            r.get("decision"),
            r.get("requested_at"),
            r.get("decided_at"),
            r.get("comment"),
            r.get("expires_at"),
        )
    })
    .transpose()
}

pub async fn list_pending(pool: &SqlitePool) -> Result<Vec<Approval>> {
    let rows = sqlx::query(
        r#"SELECT id, run_id, stage_id, decision, requested_at, decided_at, comment, expires_at
           FROM approvals WHERE decision = 'pending' OR decision = 'requested'
           ORDER BY requested_at ASC"#,
    )
    .fetch_all(pool)
    .await
    .context("list pending approvals")?;

    rows.into_iter()
        .map(|r| {
            parse_approval_row(
                r.get("id"),
                r.get("run_id"),
                r.get("stage_id"),
                r.get("decision"),
                r.get("requested_at"),
                r.get("decided_at"),
                r.get("comment"),
                r.get("expires_at"),
            )
        })
        .collect()
}

pub async fn list_by_run(pool: &SqlitePool, run_id: RunId) -> Result<Vec<Approval>> {
    let run_id_str = run_id.to_string();
    let rows = sqlx::query(
        r#"SELECT id, run_id, stage_id, decision, requested_at, decided_at, comment, expires_at
           FROM approvals WHERE run_id = ?1 ORDER BY requested_at ASC"#,
    )
    .bind(run_id_str)
    .fetch_all(pool)
    .await
    .context("list approvals by run")?;

    rows.into_iter()
        .map(|r| {
            parse_approval_row(
                r.get("id"),
                r.get("run_id"),
                r.get("stage_id"),
                r.get("decision"),
                r.get("requested_at"),
                r.get("decided_at"),
                r.get("comment"),
                r.get("expires_at"),
            )
        })
        .collect()
}

pub async fn list_by_run_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
) -> Result<Vec<Approval>> {
    let run_id_str = run_id.to_string();
    let rows = sqlx::query(
        r#"SELECT id, run_id, stage_id, decision, requested_at, decided_at, comment, expires_at
           FROM approvals WHERE run_id = ?1 ORDER BY requested_at ASC"#,
    )
    .bind(run_id_str)
    .fetch_all(&mut **tx)
    .await
    .context("list approvals by run")?;

    rows.into_iter()
        .map(|r| {
            parse_approval_row(
                r.get("id"),
                r.get("run_id"),
                r.get("stage_id"),
                r.get("decision"),
                r.get("requested_at"),
                r.get("decided_at"),
                r.get("comment"),
                r.get("expires_at"),
            )
        })
        .collect()
}

pub async fn resolve(
    pool: &SqlitePool,
    id: ApprovalId,
    decision: ApprovalDecision,
    decided_at: DateTime<Utc>,
    comment: Option<String>,
) -> Result<()> {
    let id_str = id.to_string();
    let decision_str = decision.to_string();
    let decided_at_str = decided_at.to_rfc3339();

    crate::execute_repository_write!(pool, "approvals.resolve", sqlx::query(
        r#"UPDATE approvals SET decision = ?1, decided_at = ?2, comment = COALESCE(?3, comment) WHERE id = ?4"#,
    )
    .bind(decision_str)
    .bind(decided_at_str)
    .bind(comment)
    .bind(id_str))
    .context("resolve approval")?;
    Ok(())
}

pub async fn resolve_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: ApprovalId,
    decision: ApprovalDecision,
    decided_at: DateTime<Utc>,
    comment: Option<String>,
) -> Result<()> {
    let id_str = id.to_string();
    let decision_str = decision.to_string();
    let decided_at_str = decided_at.to_rfc3339();

    sqlx::query(
        r#"UPDATE approvals SET decision = ?1, decided_at = ?2, comment = COALESCE(?3, comment) WHERE id = ?4"#,
    )
    .bind(decision_str)
    .bind(decided_at_str)
    .bind(comment)
    .bind(id_str)
    .execute(&mut **tx)
    .await
    .context("resolve approval")?;
    Ok(())
}

fn parse_approval_row(
    id: String,
    run_id: String,
    stage_id: String,
    decision: String,
    requested_at: String,
    decided_at: Option<String>,
    comment: Option<String>,
    expires_at: Option<String>,
) -> Result<Approval> {
    let approval_id: ApprovalId = id
        .parse::<uuid::Uuid>()
        .context("parse approval id")?
        .into();
    let run_id_val: RunId = run_id
        .parse::<uuid::Uuid>()
        .context("parse approval run_id")?
        .into();
    let decision_val: ApprovalDecision =
        decision.parse().map_err(|e: String| anyhow::anyhow!(e))?;
    let requested_at_dt: DateTime<Utc> = DateTime::parse_from_rfc3339(&requested_at)
        .context("parse approval requested_at")?
        .with_timezone(&Utc);
    let decided_at_dt = decided_at
        .map(|s| {
            DateTime::parse_from_rfc3339(&s)
                .context("parse approval decided_at")
                .map(|dt| dt.with_timezone(&Utc))
        })
        .transpose()?;
    let expires_at_dt = expires_at
        .map(|s| {
            DateTime::parse_from_rfc3339(&s)
                .context("parse approval expires_at")
                .map(|dt| dt.with_timezone(&Utc))
        })
        .transpose()?;

    Ok(Approval {
        id: approval_id,
        run_id: run_id_val,
        stage_id,
        decision: decision_val,
        requested_at: requested_at_dt,
        decided_at: decided_at_dt,
        comment,
        expires_at: expires_at_dt,
    })
}
