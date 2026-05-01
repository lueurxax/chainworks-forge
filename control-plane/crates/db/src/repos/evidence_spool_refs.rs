//! P075 EvidenceSpoolRef repository.
//!
//! Manages compact metadata pointers for evidence files written to the local filesystem.
//! Raw evidence bytes live in files; this repository stores only metadata (path, checksum,
//! size, kind, ownership) as Class C write targets through DbWriter (Phase 2+).
//!
//! # Phase 1
//!
//! The migration, types, and basic insert/query are present. DbWriter routing is
//! wired in Phase 2 (operation_name: "p075_evidence_spool_ref_insert").
//!
//! # File-before-metadata ordering
//!
//! Callers must complete: write → checksum → fsync(file) → atomic rename →
//! fsync(parent_dir) **before** calling `insert`. This makes metadata-without-bytes
//! impossible by construction (P075 §architecture.evidence_spooling.file_ordering).
//!
//! # Path rules
//!
//! `relative_path` must be relative to the artifact_root:
//! - No absolute paths.
//! - No `..` traversal segments.
//! - No empty path segments.
//! - Normalized to forward slashes.

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

/// Evidence kind enum matching the `kind` CHECK constraint in the migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EvidenceKind {
    Transcript,
    ToolTrace,
    Stdout,
    Stderr,
    Receipt,
    RuntimeEvent,
    ModelDelta,
    DeliveryReadback,
}

impl EvidenceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Transcript => "transcript",
            Self::ToolTrace => "tool_trace",
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
            Self::Receipt => "receipt",
            Self::RuntimeEvent => "runtime_event",
            Self::ModelDelta => "model_delta",
            Self::DeliveryReadback => "delivery_readback",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "transcript" => Some(Self::Transcript),
            "tool_trace" => Some(Self::ToolTrace),
            "stdout" => Some(Self::Stdout),
            "stderr" => Some(Self::Stderr),
            "receipt" => Some(Self::Receipt),
            "runtime_event" => Some(Self::RuntimeEvent),
            "model_delta" => Some(Self::ModelDelta),
            "delivery_readback" => Some(Self::DeliveryReadback),
            _ => None,
        }
    }
}

/// Reader status for an EvidenceSpoolRef.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceSpoolRefStatus {
    /// File present, checksum matches, readable.
    Available,
    /// Run predates P075; no spool metadata expected.
    LegacyAbsent,
    /// Metadata row exists but file is absent.
    MissingFile,
    /// File present but checksum does not match.
    ChecksumMismatch,
    /// File recovered by startup orphan sweep; metadata backfilled.
    RecoveredOrphan,
    /// Terminal-run file scheduled for deletion after grace period.
    PendingDelete,
}

impl EvidenceSpoolRefStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::LegacyAbsent => "legacy_absent",
            Self::MissingFile => "missing_file",
            Self::ChecksumMismatch => "checksum_mismatch",
            Self::RecoveredOrphan => "recovered_orphan",
            Self::PendingDelete => "pending_delete",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "available" => Some(Self::Available),
            "legacy_absent" => Some(Self::LegacyAbsent),
            "missing_file" => Some(Self::MissingFile),
            "checksum_mismatch" => Some(Self::ChecksumMismatch),
            "recovered_orphan" => Some(Self::RecoveredOrphan),
            "pending_delete" => Some(Self::PendingDelete),
            _ => None,
        }
    }
}

/// Compact metadata pointer for a single evidence file.
#[derive(Debug, Clone)]
pub struct EvidenceSpoolRef {
    pub id: String,
    pub metadata_version: i64,
    pub run_id: String,
    pub stage_execution_id: Option<String>,
    pub stage_id: Option<String>,
    pub agent_execution_id: Option<String>,
    pub agent_id: Option<String>,
    pub kind: EvidenceKind,
    /// Path relative to artifact_root. Forward-slash normalized.
    pub relative_path: String,
    pub size_bytes: i64,
    pub checksum_algorithm: String,
    pub checksum: String,
    pub producer_operation: String,
    pub content_type: Option<String>,
    /// Bounded summary JSON (max 8192 bytes). Must not contain raw evidence bytes.
    pub summary_json: Option<String>,
    pub created_at: DateTime<Utc>,
    pub status: EvidenceSpoolRefStatus,
}

/// Validate a relative path before insertion.
///
/// Rejects backslash separators before segment splitting so that mixed-separator
/// traversal such as `"foo\\..\bar"` cannot bypass the `".."` check. After
/// backslash rejection, normalize_path is a no-op and callers need not normalize
/// before validating.
pub fn validate_relative_path(path: &str) -> Result<()> {
    if path.is_empty() {
        bail!("relative_path must not be empty");
    }
    // Reject platform-specific separator ambiguity (P075 relative_path_rules).
    // Backslash rejection also blocks Windows-style absolute paths ("C:\...").
    if path.contains('\\') {
        bail!("relative_path must not contain backslash separators: {path}");
    }
    // Reject NUL bytes and ASCII control characters to prevent filesystem API
    // truncation and log injection in Phase 3+ readers (P075-SEC-004).
    if path.bytes().any(|b| b == 0 || b < 0x20) {
        bail!("relative_path must not contain NUL or control characters");
    }
    // Reject absolute paths.
    if std::path::Path::new(path).is_absolute() {
        bail!("relative_path must not be absolute: {path}");
    }
    // Validate each segment after splitting on the only accepted separator.
    for segment in path.split('/') {
        if segment.is_empty() {
            bail!("relative_path must not contain empty segments: {path}");
        }
        if segment == ".." {
            bail!("relative_path must not contain '..' traversal segments: {path}");
        }
        if segment == "." {
            bail!("relative_path must not contain '.' segments: {path}");
        }
    }
    Ok(())
}

/// Normalize a relative path to forward slashes.
pub fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

pub async fn insert(pool: &SqlitePool, spool_ref: &EvidenceSpoolRef) -> Result<()> {
    validate_relative_path(&spool_ref.relative_path).context("validate relative_path")?;
    let mut tx = pool.begin().await.context("begin insert evidence_spool_ref")?;
    insert_tx(&mut tx, spool_ref).await?;
    tx.commit().await.context("commit insert evidence_spool_ref")?;
    Ok(())
}

pub async fn insert_tx(tx: &mut Transaction<'_, Sqlite>, spool_ref: &EvidenceSpoolRef) -> Result<()> {
    validate_relative_path(&spool_ref.relative_path).context("validate relative_path")?;
    let created_at = spool_ref.created_at.to_rfc3339();
    let kind = spool_ref.kind.as_str();
    let status = spool_ref.status.as_str();
    let rel_path = normalize_path(&spool_ref.relative_path);

    sqlx::query(
        r#"
        INSERT INTO evidence_spool_refs (
            id, metadata_version, run_id,
            stage_execution_id, stage_id, agent_execution_id, agent_id,
            kind, relative_path, size_bytes,
            checksum_algorithm, checksum, producer_operation,
            content_type, summary_json, created_at, status
        ) VALUES (
            ?1, ?2, ?3,
            ?4, ?5, ?6, ?7,
            ?8, ?9, ?10,
            ?11, ?12, ?13,
            ?14, ?15, ?16, ?17
        )
        "#,
    )
    .bind(&spool_ref.id)
    .bind(spool_ref.metadata_version)
    .bind(&spool_ref.run_id)
    .bind(&spool_ref.stage_execution_id)
    .bind(&spool_ref.stage_id)
    .bind(&spool_ref.agent_execution_id)
    .bind(&spool_ref.agent_id)
    .bind(kind)
    .bind(&rel_path)
    .bind(spool_ref.size_bytes)
    .bind(&spool_ref.checksum_algorithm)
    .bind(&spool_ref.checksum)
    .bind(&spool_ref.producer_operation)
    .bind(&spool_ref.content_type)
    .bind(&spool_ref.summary_json)
    .bind(created_at)
    .bind(status)
    .execute(&mut **tx)
    .await
    .context("insert evidence_spool_ref")?;
    Ok(())
}

/// Atomic idempotent insert — uses ON CONFLICT(run_id, relative_path) DO NOTHING to
/// avoid the SELECT-then-INSERT TOCTOU race (P075-SEC-002, LIFT-REL-06).
///
/// Returns `Ok(true)` if the row was newly inserted, `Ok(false)` if an identical row
/// already existed (same checksum and size_bytes). Returns an error if a conflicting
/// row exists with a different checksum or size, signalling `evidence_metadata_conflict`.
pub async fn insert_idempotent(pool: &SqlitePool, spool_ref: &EvidenceSpoolRef) -> Result<bool> {
    validate_relative_path(&spool_ref.relative_path).context("validate relative_path")?;
    let rel_path = normalize_path(&spool_ref.relative_path);
    let created_at = spool_ref.created_at.to_rfc3339();
    let kind = spool_ref.kind.as_str();
    let status = spool_ref.status.as_str();

    let mut tx = pool.begin().await.context("begin insert_idempotent")?;

    // Single atomic statement: insert and skip silently on (run_id, relative_path) conflict.
    let result = sqlx::query(
        r#"
        INSERT INTO evidence_spool_refs (
            id, metadata_version, run_id,
            stage_execution_id, stage_id, agent_execution_id, agent_id,
            kind, relative_path, size_bytes,
            checksum_algorithm, checksum, producer_operation,
            content_type, summary_json, created_at, status
        ) VALUES (
            ?1, ?2, ?3,
            ?4, ?5, ?6, ?7,
            ?8, ?9, ?10,
            ?11, ?12, ?13,
            ?14, ?15, ?16, ?17
        ) ON CONFLICT(run_id, relative_path) DO NOTHING
        "#,
    )
    .bind(&spool_ref.id)
    .bind(spool_ref.metadata_version)
    .bind(&spool_ref.run_id)
    .bind(&spool_ref.stage_execution_id)
    .bind(&spool_ref.stage_id)
    .bind(&spool_ref.agent_execution_id)
    .bind(&spool_ref.agent_id)
    .bind(kind)
    .bind(&rel_path)
    .bind(spool_ref.size_bytes)
    .bind(&spool_ref.checksum_algorithm)
    .bind(&spool_ref.checksum)
    .bind(&spool_ref.producer_operation)
    .bind(&spool_ref.content_type)
    .bind(&spool_ref.summary_json)
    .bind(created_at)
    .bind(status)
    .execute(&mut *tx)
    .await
    .context("insert_idempotent execute")?;

    if result.rows_affected() == 1 {
        tx.commit().await.context("commit insert_idempotent")?;
        return Ok(true);
    }

    // rows_affected == 0: (run_id, relative_path) conflict — check the existing row.
    let existing = sqlx::query(
        r#"SELECT checksum, size_bytes FROM evidence_spool_refs
           WHERE run_id = ?1 AND relative_path = ?2"#,
    )
    .bind(&spool_ref.run_id)
    .bind(&rel_path)
    .fetch_one(&mut *tx)
    .await
    .context("fetch existing row after idempotent conflict")?;

    tx.commit().await.context("commit insert_idempotent check")?;

    let existing_checksum: String = existing.try_get("checksum").context("existing checksum")?;
    let existing_size: i64 = existing.try_get("size_bytes").context("existing size_bytes")?;

    if existing_checksum == spool_ref.checksum && existing_size == spool_ref.size_bytes {
        return Ok(false);
    }

    // Mismatch: hard error requiring manual reconcile (LIFT-REL-06, evidence_metadata_conflict_total).
    bail!(
        "evidence_metadata_conflict: run_id={} relative_path={} \
         existing checksum={} new checksum={}",
        spool_ref.run_id, rel_path, existing_checksum, spool_ref.checksum
    );
}

pub async fn find_by_id(pool: &SqlitePool, id: &str) -> Result<Option<EvidenceSpoolRef>> {
    let row = sqlx::query(
        r#"SELECT id, metadata_version, run_id,
                  stage_execution_id, stage_id, agent_execution_id, agent_id,
                  kind, relative_path, size_bytes,
                  checksum_algorithm, checksum, producer_operation,
                  content_type, summary_json, created_at, status
           FROM evidence_spool_refs WHERE id = ?1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .context("find evidence_spool_ref by id")?;

    row.map(parse_row).transpose()
}

pub async fn find_by_run_and_path(
    pool: &SqlitePool,
    run_id: &str,
    relative_path: &str,
) -> Result<Option<EvidenceSpoolRef>> {
    let rel_path = normalize_path(relative_path);
    let row = sqlx::query(
        r#"SELECT id, metadata_version, run_id,
                  stage_execution_id, stage_id, agent_execution_id, agent_id,
                  kind, relative_path, size_bytes,
                  checksum_algorithm, checksum, producer_operation,
                  content_type, summary_json, created_at, status
           FROM evidence_spool_refs WHERE run_id = ?1 AND relative_path = ?2"#,
    )
    .bind(run_id)
    .bind(&rel_path)
    .fetch_optional(pool)
    .await
    .context("find evidence_spool_ref by run_id + relative_path")?;

    row.map(parse_row).transpose()
}

pub async fn list_by_run_id(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Vec<EvidenceSpoolRef>> {
    let rows = sqlx::query(
        r#"SELECT id, metadata_version, run_id,
                  stage_execution_id, stage_id, agent_execution_id, agent_id,
                  kind, relative_path, size_bytes,
                  checksum_algorithm, checksum, producer_operation,
                  content_type, summary_json, created_at, status
           FROM evidence_spool_refs
           WHERE run_id = ?1
           ORDER BY created_at ASC"#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .context("list evidence_spool_refs by run_id")?;

    rows.into_iter().map(parse_row).collect()
}

/// Update status for a spool ref by id.
pub async fn update_status(
    pool: &SqlitePool,
    id: &str,
    status: EvidenceSpoolRefStatus,
) -> Result<()> {
    let status_str = status.as_str();
    sqlx::query("UPDATE evidence_spool_refs SET status = ?1 WHERE id = ?2")
        .bind(status_str)
        .bind(id)
        .execute(pool)
        .await
        .context("update evidence_spool_ref status")?;
    Ok(())
}

fn parse_row(row: sqlx::sqlite::SqliteRow) -> Result<EvidenceSpoolRef> {
    let kind_str: String = row.try_get("kind").context("kind")?;
    let kind = EvidenceKind::from_str(&kind_str)
        .ok_or_else(|| anyhow::anyhow!("unknown evidence kind: {kind_str}"))?;

    let status_str: String = row.try_get("status").context("status")?;
    let status = EvidenceSpoolRefStatus::from_str(&status_str)
        .ok_or_else(|| anyhow::anyhow!("unknown evidence status: {status_str}"))?;

    let created_at_str: String = row.try_get("created_at").context("created_at")?;
    let created_at = DateTime::parse_from_rfc3339(&created_at_str)
        .context("parse created_at")?
        .with_timezone(&Utc);

    Ok(EvidenceSpoolRef {
        id: row.try_get("id").context("id")?,
        metadata_version: row.try_get("metadata_version").context("metadata_version")?,
        run_id: row.try_get("run_id").context("run_id")?,
        stage_execution_id: row.try_get("stage_execution_id").context("stage_execution_id")?,
        stage_id: row.try_get("stage_id").context("stage_id")?,
        agent_execution_id: row.try_get("agent_execution_id").context("agent_execution_id")?,
        agent_id: row.try_get("agent_id").context("agent_id")?,
        kind,
        relative_path: row.try_get("relative_path").context("relative_path")?,
        size_bytes: row.try_get("size_bytes").context("size_bytes")?,
        checksum_algorithm: row.try_get("checksum_algorithm").context("checksum_algorithm")?,
        checksum: row.try_get("checksum").context("checksum")?,
        producer_operation: row.try_get("producer_operation").context("producer_operation")?,
        content_type: row.try_get("content_type").context("content_type")?,
        summary_json: row.try_get("summary_json").context("summary_json")?,
        created_at,
        status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::create_pool;
    use chrono::Utc;

    fn make_ref(id: &str, run_id: &str, relative_path: &str) -> EvidenceSpoolRef {
        EvidenceSpoolRef {
            id: id.to_string(),
            metadata_version: 1,
            run_id: run_id.to_string(),
            stage_execution_id: Some("stage-exec-1".to_string()),
            stage_id: Some("stage-1".to_string()),
            agent_execution_id: Some("agent-exec-1".to_string()),
            agent_id: Some("agent-1".to_string()),
            kind: EvidenceKind::Transcript,
            relative_path: relative_path.to_string(),
            size_bytes: 4096,
            checksum_algorithm: "sha256".to_string(),
            checksum: "abc123def456".to_string(),
            producer_operation: "p075_evidence_spool_ref_insert".to_string(),
            content_type: Some("text/plain".to_string()),
            summary_json: Some(r#"{"line_count":100}"#.to_string()),
            created_at: Utc::now(),
            status: EvidenceSpoolRefStatus::Available,
        }
    }

    #[tokio::test]
    async fn insert_and_find_by_id() {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        let spool_ref = make_ref("evsp_001", "run-1", "evidence/runs/run-1/transcript.jsonl");
        insert(&pool, &spool_ref).await.unwrap();
        let found = find_by_id(&pool, "evsp_001").await.unwrap().unwrap();
        assert_eq!(found.id, "evsp_001");
        assert_eq!(found.run_id, "run-1");
        assert_eq!(found.kind, EvidenceKind::Transcript);
        assert_eq!(found.status, EvidenceSpoolRefStatus::Available);
    }

    #[tokio::test]
    async fn insert_and_find_by_run_and_path() {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        let spool_ref = make_ref("evsp_002", "run-2", "evidence/runs/run-2/tool.jsonl");
        insert(&pool, &spool_ref).await.unwrap();
        let found = find_by_run_and_path(&pool, "run-2", "evidence/runs/run-2/tool.jsonl")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.id, "evsp_002");
    }

    #[tokio::test]
    async fn insert_idempotent_same_checksum() {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        let spool_ref = make_ref("evsp_003", "run-3", "evidence/runs/run-3/ts.jsonl");
        let inserted = insert_idempotent(&pool, &spool_ref).await.unwrap();
        assert!(inserted, "first insert should return true");
        let duplicate = insert_idempotent(&pool, &spool_ref).await.unwrap();
        assert!(!duplicate, "same-checksum re-insert should return false (idempotent)");
    }

    #[tokio::test]
    async fn insert_idempotent_checksum_mismatch_is_error() {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        let spool_ref = make_ref("evsp_004", "run-4", "evidence/runs/run-4/ts.jsonl");
        insert(&pool, &spool_ref).await.unwrap();

        let mut conflict = spool_ref.clone();
        conflict.id = "evsp_005".to_string();
        conflict.checksum = "different_checksum".to_string();
        let result = insert_idempotent(&pool, &conflict).await;
        assert!(result.is_err(), "checksum mismatch should return error");
    }

    #[tokio::test]
    async fn unique_constraint_on_run_id_relative_path() {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        let a = make_ref("evsp_010", "run-10", "evidence/runs/run-10/ts.jsonl");
        insert(&pool, &a).await.unwrap();
        let mut b = a.clone();
        b.id = "evsp_011".to_string(); // different id, same path
        let result = insert(&pool, &b).await;
        assert!(result.is_err(), "UNIQUE (run_id, relative_path) should prevent duplicate");
    }

    #[tokio::test]
    async fn check_constraint_rejects_invalid_kind() {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        let result = sqlx::query(
            r#"INSERT INTO evidence_spool_refs
               (id, metadata_version, run_id, kind, relative_path, size_bytes,
                checksum_algorithm, checksum, producer_operation, created_at, status)
               VALUES ('x','1','r','BAD_KIND','path/f',0,'sha256','abc','op','2025-01-01T00:00:00Z','available')"#,
        )
        .execute(&pool)
        .await;
        assert!(result.is_err(), "invalid kind should fail CHECK constraint");
    }

    #[tokio::test]
    async fn check_constraint_rejects_invalid_status() {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        let result = sqlx::query(
            r#"INSERT INTO evidence_spool_refs
               (id, metadata_version, run_id, kind, relative_path, size_bytes,
                checksum_algorithm, checksum, producer_operation, created_at, status)
               VALUES ('x2','1','r','transcript','path/g',0,'sha256','abc','op','2025-01-01T00:00:00Z','BAD_STATUS')"#,
        )
        .execute(&pool)
        .await;
        assert!(result.is_err(), "invalid status should fail CHECK constraint");
    }

    #[tokio::test]
    async fn check_constraint_rejects_negative_size() {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        let result = sqlx::query(
            r#"INSERT INTO evidence_spool_refs
               (id, metadata_version, run_id, kind, relative_path, size_bytes,
                checksum_algorithm, checksum, producer_operation, created_at, status)
               VALUES ('x3','1','r','transcript','path/h',-1,'sha256','abc','op','2025-01-01T00:00:00Z','available')"#,
        )
        .execute(&pool)
        .await;
        assert!(result.is_err(), "negative size_bytes should fail CHECK constraint");
    }

    #[tokio::test]
    async fn check_constraint_rejects_empty_relative_path() {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        let result = sqlx::query(
            r#"INSERT INTO evidence_spool_refs
               (id, metadata_version, run_id, kind, relative_path, size_bytes,
                checksum_algorithm, checksum, producer_operation, created_at, status)
               VALUES ('x4','1','r','transcript','',0,'sha256','abc','op','2025-01-01T00:00:00Z','available')"#,
        )
        .execute(&pool)
        .await;
        assert!(result.is_err(), "empty relative_path should fail CHECK constraint");
    }

    #[tokio::test]
    async fn check_constraint_rejects_summary_json_too_large() {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        // Build a summary_json exceeding 8192 bytes.
        let large_json = format!(r#"{{"data":"{}"}}"#, "x".repeat(8200));
        let result = sqlx::query(
            r#"INSERT INTO evidence_spool_refs
               (id, metadata_version, run_id, kind, relative_path, size_bytes,
                checksum_algorithm, checksum, producer_operation,
                content_type, summary_json, created_at, status)
               VALUES ('x5','1','r','transcript','path/j',0,'sha256','abc','op',
                       NULL, ?1, '2025-01-01T00:00:00Z','available')"#,
        )
        .bind(&large_json)
        .execute(&pool)
        .await;
        assert!(result.is_err(), "summary_json > 8192 bytes should fail CHECK constraint");
    }

    #[tokio::test]
    async fn update_status_changes_value() {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        let spool_ref = make_ref("evsp_020", "run-20", "evidence/runs/run-20/ts.jsonl");
        insert(&pool, &spool_ref).await.unwrap();
        update_status(&pool, "evsp_020", EvidenceSpoolRefStatus::PendingDelete)
            .await
            .unwrap();
        let found = find_by_id(&pool, "evsp_020").await.unwrap().unwrap();
        assert_eq!(found.status, EvidenceSpoolRefStatus::PendingDelete);
    }

    #[tokio::test]
    async fn list_by_run_id_returns_ordered_results() {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        let r1 = make_ref("evsp_030", "run-30", "evidence/runs/run-30/a.jsonl");
        let r2 = make_ref("evsp_031", "run-30", "evidence/runs/run-30/b.jsonl");
        let r3 = make_ref("evsp_032", "run-99", "evidence/runs/run-99/c.jsonl");
        insert(&pool, &r1).await.unwrap();
        insert(&pool, &r2).await.unwrap();
        insert(&pool, &r3).await.unwrap();
        let results = list_by_run_id(&pool, "run-30").await.unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.run_id == "run-30"));
    }

    #[test]
    fn validate_relative_path_rejects_absolute() {
        assert!(validate_relative_path("/absolute/path").is_err());
    }

    #[test]
    fn validate_relative_path_rejects_traversal() {
        assert!(validate_relative_path("foo/../bar").is_err());
    }

    #[test]
    fn validate_relative_path_rejects_empty() {
        assert!(validate_relative_path("").is_err());
    }

    #[test]
    fn validate_relative_path_rejects_empty_segment() {
        assert!(validate_relative_path("foo//bar").is_err());
    }

    #[test]
    fn validate_relative_path_accepts_valid() {
        assert!(validate_relative_path("evidence/runs/run-1/ts.jsonl").is_ok());
    }

    #[test]
    fn validate_relative_path_rejects_backslash_traversal() {
        // "foo\..\bar" passes the old '/' split but must now fail on backslash (SEC-001).
        assert!(validate_relative_path("foo\\..\\bar").is_err());
    }

    #[test]
    fn validate_relative_path_rejects_backslash_separator() {
        assert!(validate_relative_path("foo\\bar").is_err());
    }

    #[test]
    fn validate_relative_path_rejects_dot_segment() {
        assert!(validate_relative_path("foo/./bar").is_err());
    }

    #[test]
    fn validate_relative_path_rejects_nul_byte() {
        assert!(validate_relative_path("foo\0bar").is_err());
    }

    #[test]
    fn validate_relative_path_rejects_control_char() {
        assert!(validate_relative_path("foo\nbar").is_err());
    }

    #[test]
    fn normalize_path_converts_backslashes() {
        assert_eq!(normalize_path("foo\\bar\\baz"), "foo/bar/baz");
    }

    #[test]
    fn evidence_kind_roundtrip() {
        for kind in [
            EvidenceKind::Transcript,
            EvidenceKind::ToolTrace,
            EvidenceKind::Stdout,
            EvidenceKind::Stderr,
            EvidenceKind::Receipt,
            EvidenceKind::RuntimeEvent,
            EvidenceKind::ModelDelta,
            EvidenceKind::DeliveryReadback,
        ] {
            let s = kind.as_str();
            let back = EvidenceKind::from_str(s).expect("roundtrip");
            assert_eq!(kind, back, "roundtrip failed for {s}");
        }
    }

    #[test]
    fn evidence_status_roundtrip() {
        for status in [
            EvidenceSpoolRefStatus::Available,
            EvidenceSpoolRefStatus::LegacyAbsent,
            EvidenceSpoolRefStatus::MissingFile,
            EvidenceSpoolRefStatus::ChecksumMismatch,
            EvidenceSpoolRefStatus::RecoveredOrphan,
            EvidenceSpoolRefStatus::PendingDelete,
        ] {
            let s = status.as_str();
            let back = EvidenceSpoolRefStatus::from_str(s).expect("roundtrip");
            assert_eq!(status, back, "roundtrip failed for {s}");
        }
    }
}
