// P086: DB-backed raw attach receipt storage and access audit log.
//
// Raw provider_session_attach_receipt_v2 bodies are stored in the DB (migration 085)
// rather than the filesystem. This prevents same-UID ACP subprocess path traversal:
// DATABASE_URL is never passed to child processes (env_clear + allowlist), so child
// processes cannot reach the raw receipt even though they run as the same UID.
//
// Access is tracked in p086_receipt_access_audit per the proposal access matrix:
//   Operator (run-scoped) → full raw body
//   Reviewer               → redacted projection (session ids hashed, pids absent)
//   Guest/unauthenticated  → minimal projection (existence + resurrection_phase only)
//   Wrong-run Operator     → denied (auth_failure, no existence oracle)

use anyhow::Result;
use domain::continuation::ResurrectionPhase;
use sqlx::{Row, SqlitePool};

pub struct RawReceiptRow {
    pub continuation_id: String,
    pub raw_receipt_json: String,
    pub written_at: String,
}

pub struct ReceiptReadbackContext {
    pub run_id: String,
    pub attach_receipt_artifact_id: Option<String>,
    pub resurrection_phase: Option<ResurrectionPhase>,
}

/// Persist or replace the raw receipt JSON for a resurrection continuation.
pub async fn upsert(
    pool: &SqlitePool,
    continuation_id: &str,
    raw_receipt_json: &str,
    written_at: &str,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO p086_resurrection_raw_receipts
           (continuation_id, raw_receipt_json, written_at)
           VALUES (?1, ?2, ?3)
           ON CONFLICT(continuation_id) DO UPDATE SET
             raw_receipt_json = excluded.raw_receipt_json,
             written_at = excluded.written_at"#,
    )
    .bind(continuation_id)
    .bind(raw_receipt_json)
    .bind(written_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Fetch the raw receipt for a continuation.  Returns None if not found.
pub async fn find_by_continuation_id(
    pool: &SqlitePool,
    continuation_id: &str,
) -> Result<Option<RawReceiptRow>> {
    let row = sqlx::query(
        "SELECT continuation_id, raw_receipt_json, written_at \
         FROM p086_resurrection_raw_receipts \
         WHERE continuation_id = ?1",
    )
    .bind(continuation_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| RawReceiptRow {
        continuation_id: r.get("continuation_id"),
        raw_receipt_json: r.get("raw_receipt_json"),
        written_at: r.get("written_at"),
    }))
}

pub struct ReceiptAccessAuditRow {
    pub id: String,
    pub principal_id: String,
    pub principal_class: String,
    pub continuation_id: String,
    pub run_id: String,
    pub requested_at: String,
    pub source_channel: String,
    pub outcome: String,
    pub denial_reason: Option<String>,
}

/// Record a single receipt access event (raw read, reviewer projection, or denial).
pub async fn record_access_audit(pool: &SqlitePool, row: &ReceiptAccessAuditRow) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO p086_receipt_access_audit
           (id, principal_id, principal_class, continuation_id, run_id,
            requested_at, source_channel, outcome, denial_reason, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
    )
    .bind(&row.id)
    .bind(&row.principal_id)
    .bind(&row.principal_class)
    .bind(&row.continuation_id)
    .bind(&row.run_id)
    .bind(&row.requested_at)
    .bind(&row.source_channel)
    .bind(&row.outcome)
    .bind(&row.denial_reason)
    .bind(&row.requested_at) // created_at = requested_at
    .execute(pool)
    .await?;
    Ok(())
}

/// Check whether a continuation_id belongs to a given run_id.
/// Returns Some(run_id) if found in agent_work_continuations, None if not.
/// Used for run-scoped authorization before raw receipt access.
pub async fn continuation_run_id(
    pool: &SqlitePool,
    continuation_id: &str,
) -> Result<Option<String>> {
    let row = sqlx::query("SELECT run_id FROM agent_work_continuations WHERE id = ?1")
        .bind(continuation_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.get::<String, _>("run_id")))
}

/// Fetch the run-scoping and public-artifact context needed to shape
/// `agents.attach_receipt.get` responses without exposing raw receipt fields.
pub async fn continuation_receipt_readback_context(
    pool: &SqlitePool,
    continuation_id: &str,
) -> Result<Option<ReceiptReadbackContext>> {
    let row = sqlx::query(
        "SELECT run_id, attach_receipt_artifact_id, resurrection_phase \
         FROM agent_work_continuations \
         WHERE id = ?1",
    )
    .bind(continuation_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| {
        let resurrection_phase = r
            .get::<Option<String>, _>("resurrection_phase")
            .map(|phase| {
                phase
                    .parse::<ResurrectionPhase>()
                    .expect("database resurrection_phase must match domain enum")
            });
        ReceiptReadbackContext {
            run_id: r.get("run_id"),
            attach_receipt_artifact_id: r.get("attach_receipt_artifact_id"),
            resurrection_phase,
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn p086_raw_receipt_stored_in_db_not_filesystem() {
        // P086 SEC-HIGH-001: raw receipt bodies are written to and read from the DB,
        // not the filesystem, preventing same-UID ACP subprocess path traversal.
        let pool = test_pool().await;
        let raw = r#"{"schema_version":"provider_session_attach_receipt_v2"}"#;
        upsert(&pool, "cont-id-1", raw, "2026-01-01T00:00:00Z")
            .await
            .unwrap();

        let found = find_by_continuation_id(&pool, "cont-id-1")
            .await
            .unwrap()
            .expect("receipt must be stored");
        assert_eq!(found.continuation_id, "cont-id-1");
        assert_eq!(found.raw_receipt_json, raw);
        assert_eq!(found.written_at, "2026-01-01T00:00:00Z");

        let missing = find_by_continuation_id(&pool, "no-such-id").await.unwrap();
        assert!(missing.is_none(), "non-existent id must return None");
    }

    #[tokio::test]
    async fn p086_raw_receipt_upsert_replaces_existing() {
        let pool = test_pool().await;
        upsert(&pool, "cont-id-1", r#"{"v":1}"#, "2026-01-01T00:00:00Z")
            .await
            .unwrap();
        upsert(&pool, "cont-id-1", r#"{"v":2}"#, "2026-01-02T00:00:00Z")
            .await
            .unwrap();

        let found = find_by_continuation_id(&pool, "cont-id-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.raw_receipt_json, r#"{"v":2}"#);
    }

    #[tokio::test]
    async fn p086_receipt_access_audit_records_and_retrieves() {
        let pool = test_pool().await;
        let audit_row = ReceiptAccessAuditRow {
            id: "audit-id-1".to_string(),
            principal_id: "op-principal".to_string(),
            principal_class: "operator".to_string(),
            continuation_id: "cont-id-1".to_string(),
            run_id: "run-id-1".to_string(),
            requested_at: "2026-01-01T00:00:00Z".to_string(),
            source_channel: "mcp".to_string(),
            outcome: "raw_read".to_string(),
            denial_reason: None,
        };
        record_access_audit(&pool, &audit_row).await.unwrap();

        let row = sqlx::query(
            "SELECT outcome, denial_reason FROM p086_receipt_access_audit WHERE id = ?1",
        )
        .bind("audit-id-1")
        .fetch_one(&pool)
        .await
        .unwrap();
        let outcome: String = row.get("outcome");
        let denial: Option<String> = row.get("denial_reason");
        assert_eq!(outcome, "raw_read");
        assert!(denial.is_none());
    }

    #[tokio::test]
    async fn p086_receipt_access_audit_records_denial_reason() {
        let pool = test_pool().await;
        let audit_row = ReceiptAccessAuditRow {
            id: "audit-id-2".to_string(),
            principal_id: "rev-principal".to_string(),
            principal_class: "observer".to_string(),
            continuation_id: "cont-id-1".to_string(),
            run_id: "run-id-1".to_string(),
            requested_at: "2026-01-01T00:01:00Z".to_string(),
            source_channel: "mcp".to_string(),
            outcome: "denied".to_string(),
            denial_reason: Some("wrong_run".to_string()),
        };
        record_access_audit(&pool, &audit_row).await.unwrap();

        let row = sqlx::query(
            "SELECT outcome, denial_reason FROM p086_receipt_access_audit WHERE id = ?1",
        )
        .bind("audit-id-2")
        .fetch_one(&pool)
        .await
        .unwrap();
        let outcome: String = row.get("outcome");
        let denial: Option<String> = row.get("denial_reason");
        assert_eq!(outcome, "denied");
        assert_eq!(denial.as_deref(), Some("wrong_run"));
    }
}
