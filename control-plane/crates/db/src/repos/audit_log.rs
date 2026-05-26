//! P081 Phase 1: audit_log repository.
//!
//! Provides transactional append (`append_tx`) for allowed mutating paths already
//! inside a BEGIN IMMEDIATE write unit, and a bounded standalone append path
//! (`append`) for deny-only durable audit writes.
//!
//! Row-hash, prev-hash linkage, and payload truncation envelope construction are
//! owned here so each caller does not reimplement audit serialization rules.
//! Bearer tokens are never logged.
//!
//! Hash chain:
//!   prev_hash = row_hash of the most recently committed audit_log row in this DB.
//!   row_hash  = sha256(canonical fields || "\x1f" || prev_hash_or_empty).
//!   checkpoint_hash = sha256(previous_checkpoint_hash || covered row_hash sequence
//!                            || checkpoint metadata) — computed by the caller who
//!                     writes checkpoints (repo validates the format on insert).

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use sqlx::{Sqlite, SqlitePool, Transaction};

/// Lightweight description of one audit log entry passed to append functions.
/// The caller must NOT supply `row_hash`, `prev_hash`, or `payload_sha256`; the repo
/// computes and owns these to guarantee tamper-evidence consistency.
/// SEC-P081-004: payload_sha256 is always computed by the repo from bytes — never trusted
/// from caller input. For truncated payloads (`diagnostic_truncated=true`), supply
/// `original_payload_bytes` (the raw pre-truncation content) so the repo can compute
/// `sha256(original)` independently without trusting any caller-supplied digest.
pub struct AuditEntry<'a> {
    pub id: &'a str,
    pub request_id: &'a str,
    pub timestamp_ms: i64,
    pub event_type: &'a str,
    pub principal_id: Option<&'a str>,
    pub principal_class: Option<&'a str>,
    pub caller_class: Option<&'a str>,
    pub token_id: Option<&'a str>,
    pub transport: &'a str,
    pub action_attempted: &'a str,
    pub decision: &'a str,
    pub denial_reason_code: Option<&'a str>,
    pub row_id: Option<&'a str>,
    pub env_gate_state: Option<&'a str>,
    pub source_ip_hash_or_local_process_id: Option<&'a str>,
    pub boundary_policy_mode: &'a str,
    pub fixture_version: &'a str,
    /// Canonical JSON payload capped at 16 KiB. When `diagnostic_truncated=true`
    /// this must be the truncation envelope built by `build_envelope`.
    pub payload: &'a str,
    /// The raw pre-truncation payload bytes, supplied only when `diagnostic_truncated=true`.
    /// The repo computes sha256 from these bytes so no caller-supplied digest is trusted.
    /// SEC-P081-M003: use this instead of a caller-supplied hash to prevent forgery.
    pub original_payload_bytes: Option<&'a str>,
    pub diagnostic_truncated: bool,
    pub checkpoint_id: Option<&'a str>,
    pub created_at_ms: i64,
}

/// Build a canonical truncation envelope for oversized audit payloads.
///
/// SEC-P081-006: Centralised here so every caller routes through the same logic
/// and does not reimplement payload-cap, sha256, or envelope construction.
///
/// Returns `(stored_payload, original_sha256_hex, diagnostic_truncated)`.
/// - If `raw` fits within 16 KiB, returns `(raw.to_string(), sha256(raw), false)`.
/// - Otherwise returns a compact JSON truncation envelope and `true`.
pub fn build_envelope(raw: &str) -> (String, String, bool) {
    use sha2::{Digest, Sha256};
    let original_sha256 = {
        let mut h = Sha256::new();
        h.update(raw.as_bytes());
        format!("{:x}", h.finalize())
    };
    if raw.len() <= MAX_PAYLOAD_BYTES {
        (raw.to_string(), original_sha256, false)
    } else {
        // allowed_keys is empty because the full payload is replaced by this envelope;
        // no original payload keys are preserved in the truncation output.
        let envelope = serde_json::json!({
            "diagnostic_truncated": true,
            "payload_sha256": original_sha256,
            "original_size_bytes": raw.len(),
            "allowed_keys": [],
        })
        .to_string();
        (envelope, original_sha256, true)
    }
}

/// Lightweight description of one checkpoint entry.
pub struct CheckpointEntry<'a> {
    pub checkpoint_id: &'a str,
    pub checkpoint_seq: i64,
    pub covered_start_id: &'a str,
    pub covered_end_id: &'a str,
    pub covered_row_count: i64,
    pub previous_checkpoint_hash: Option<&'a str>,
    pub checkpoint_hash: &'a str,
    pub created_at_ms: i64,
}

/// Bounded health snapshot for operator readback. Does not expose raw rows.
#[derive(Debug, Clone)]
pub struct AuditLogHealthSnapshot {
    pub row_count: i64,
    pub latest_row_id: Option<String>,
    pub latest_checkpoint_seq: Option<i64>,
    pub latest_checkpoint_hash: Option<String>,
    pub writable: bool,
    pub last_write_ok_at_ms: Option<i64>,
    pub consecutive_failures: i64,
    pub cumulative_failures: i64,
    pub retention_min_days: i64,
    pub cleanup_state: String,
    pub cleanup_eligible_row_count: i64,
    pub cleanup_protected_row_count: i64,
    pub budget_bytes: i64,
    pub used_bytes: i64,
    pub payload_budget_bytes: i64,
    pub payload_used_bytes: i64,
    pub payload_budget_state: String,
    pub payload_budget_used_percent: i64,
    pub half_open_probe_success_count: i64,
    pub shadow_coverage_report_ref: String,
}

/// SEC-M-004: Integrity state from startup checkpoint verification.
///
/// `NoCheckpoints`: table exists but no checkpoints yet (normal for fresh installs).
/// `Verified`: latest checkpoint hash recomputed from covered rows matches stored hash.
/// `Degraded`: checkpoint window rows cannot be read or hash inputs are incomplete.
/// `TamperSuspected`: recomputed hash does not match stored hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditIntegrityState {
    NoCheckpoints,
    Verified,
    Degraded,
    TamperSuspected,
}

impl AuditIntegrityState {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditIntegrityState::NoCheckpoints => "no_checkpoints",
            AuditIntegrityState::Verified => "verified",
            AuditIntegrityState::Degraded => "degraded",
            AuditIntegrityState::TamperSuspected => "tamper_suspected",
        }
    }
}

// ── Internal hash computation ────────────────────────────────────────────

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// SEC-P081-001: Reject fields that contain the 0x1F delimiter byte used in
/// the canonical hash form. Called for free-text fields before hashing so that
/// a crafted field value cannot shift subsequent fields in the canonical string
/// and produce a hash-chain collision.
fn assert_no_delimiter(field_name: &str, value: &str) -> anyhow::Result<()> {
    if value.contains('\x1f') {
        anyhow::bail!(
            "audit_log field '{}' contains illegal ASCII Unit Separator (0x1F) byte; \
             field rejected before hash-chain computation",
            field_name
        );
    }
    Ok(())
}

/// Compute the canonical row hash for an audit entry.
///
/// Canonical form: all fields in column order joined with ASCII Unit Separator
/// (0x1F), followed by the prev_hash (or empty string if this is the first row).
/// sha256 of the UTF-8 encoded canonical string produces the row_hash.
/// Free-text fields are validated by the caller via assert_no_delimiter before
/// this function is invoked.
/// SEC-P081-004: payload_sha256 is passed in as a parameter computed by the repo,
/// never read from the entry struct, so callers cannot influence the hash chain.
///
/// Note: checkpoint_id is intentionally excluded from the canonical row hash.
/// It is a backfill annotation stamped on rows after checkpoint_write_if_needed
/// runs; including it would cause verify_latest_checkpoint to report
/// TamperSuspected for every row that has been checkpointed.
fn compute_row_hash(entry: &AuditEntry<'_>, payload_sha256: &str, prev_hash: &str) -> String {
    // Canonical form: audit_log columns in INSERT order, excluding row_hash, prev_hash,
    // and checkpoint_id (which is a post-hoc backfill — see note above).
    // payload_schema_version is always 1 in the current schema and is included as a constant.
    // 23 fields separated by 22 unit-separator bytes.
    let canonical = format!(
        "{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}",
        entry.id,
        entry.request_id,
        entry.timestamp_ms,
        entry.event_type,
        entry.principal_id.unwrap_or(""),
        entry.principal_class.unwrap_or(""),
        entry.caller_class.unwrap_or(""),
        entry.token_id.unwrap_or(""),
        entry.transport,
        entry.action_attempted,
        entry.decision,
        entry.denial_reason_code.unwrap_or(""),
        entry.row_id.unwrap_or(""),
        entry.env_gate_state.unwrap_or(""),
        entry.source_ip_hash_or_local_process_id.unwrap_or(""),
        entry.boundary_policy_mode,
        entry.fixture_version,
        1u8, // payload_schema_version — always 1 in current schema
        entry.payload,
        payload_sha256,
        entry.diagnostic_truncated as u8,
        entry.created_at_ms,
        prev_hash,
    );
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    bytes_to_hex(&hasher.finalize())
}

/// Fetch the most recent row_hash from `audit_log` inside the given transaction.
/// Returns `None` if there are no prior rows (first entry in the chain).
async fn fetch_prev_hash<'c>(tx: &mut Transaction<'c, Sqlite>) -> Result<Option<String>> {
    let result: Option<String> =
        sqlx::query_scalar("SELECT row_hash FROM audit_log ORDER BY rowid DESC LIMIT 1")
            .fetch_optional(&mut **tx)
            .await
            .context("fetch prev_hash for audit chain")?;
    Ok(result)
}

// ── Public API ───────────────────────────────────────────────────────────

/// Append one audit log row inside an existing caller-owned BEGIN IMMEDIATE
/// transaction. Used by allowed mutating paths that commit audit evidence
/// in the same write unit as command_journal, idempotency, and approval
/// settlement.
///
/// The repo computes `prev_hash` and `row_hash` internally; callers must not
/// supply these values. This ensures the hash chain cannot be broken by a
/// buggy or malicious caller.
/// Maximum audit log payload size. Enforced in Rust before row-hash computation
/// so callers receive a typed error rather than an opaque SQL CHECK failure.
const MAX_PAYLOAD_BYTES: usize = 16_384;

pub async fn append_tx<'c>(tx: &mut Transaction<'c, Sqlite>, entry: &AuditEntry<'_>) -> Result<()> {
    // SEC-002: Guard payload size before hash-chain computation. The SQL CHECK
    // constraint also enforces this limit, but a Rust-side check returns a typed
    // error that callers can map to E_AUDIT_UNAVAILABLE rather than an opaque
    // SQL error. Caller must supply a truncated envelope with diagnostic_truncated=1
    // when the original payload exceeds this limit.
    if entry.payload.len() > MAX_PAYLOAD_BYTES {
        anyhow::bail!(
            "audit_log payload exceeds {} byte limit ({} bytes supplied); \
             caller must truncate with diagnostic_truncated=1 before calling append_tx",
            MAX_PAYLOAD_BYTES,
            entry.payload.len()
        );
    }
    // SEC-P081-001: Validate every free-text field that participates in
    // compute_row_hash. Enum-constrained fields are also checked here so that
    // a future relaxation of DB constraints cannot silently reopen the gap.
    assert_no_delimiter("id", entry.id)?;
    assert_no_delimiter("request_id", entry.request_id)?;
    assert_no_delimiter("event_type", entry.event_type)?;
    assert_no_delimiter("transport", entry.transport)?;
    assert_no_delimiter("action_attempted", entry.action_attempted)?;
    assert_no_delimiter("decision", entry.decision)?;
    assert_no_delimiter("boundary_policy_mode", entry.boundary_policy_mode)?;
    assert_no_delimiter("fixture_version", entry.fixture_version)?;
    assert_no_delimiter("payload", entry.payload)?;
    if let Some(v) = entry.principal_id {
        assert_no_delimiter("principal_id", v)?;
    }
    if let Some(v) = entry.principal_class {
        assert_no_delimiter("principal_class", v)?;
    }
    if let Some(v) = entry.caller_class {
        assert_no_delimiter("caller_class", v)?;
    }
    if let Some(v) = entry.token_id {
        assert_no_delimiter("token_id", v)?;
    }
    if let Some(v) = entry.denial_reason_code {
        assert_no_delimiter("denial_reason_code", v)?;
    }
    if let Some(v) = entry.row_id {
        assert_no_delimiter("row_id", v)?;
    }
    if let Some(v) = entry.env_gate_state {
        assert_no_delimiter("env_gate_state", v)?;
    }
    if let Some(v) = entry.source_ip_hash_or_local_process_id {
        assert_no_delimiter("source_ip_hash_or_local_process_id", v)?;
    }

    // SEC-P081-004/M003: Compute payload_sha256 exclusively from bytes — never trust a
    // caller-supplied digest. For truncated entries, hash the original pre-truncation
    // bytes when supplied; otherwise hash the stored truncation envelope.
    let computed_payload_sha256: String = if entry.diagnostic_truncated {
        match entry.original_payload_bytes {
            Some(raw) => {
                let mut h = Sha256::new();
                h.update(raw.as_bytes());
                format!("{:x}", h.finalize())
            }
            None => {
                let mut h = Sha256::new();
                h.update(entry.payload.as_bytes());
                format!("{:x}", h.finalize())
            }
        }
    } else {
        let mut h = Sha256::new();
        h.update(entry.payload.as_bytes());
        format!("{:x}", h.finalize())
    };

    let prev_hash = fetch_prev_hash(tx).await?;
    let prev_hash_str = prev_hash.as_deref().unwrap_or("");
    let row_hash = compute_row_hash(entry, &computed_payload_sha256, prev_hash_str);

    sqlx::query(
        r#"INSERT INTO audit_log (
            id, request_id, timestamp_ms, event_type,
            principal_id, principal_class, caller_class, token_id,
            transport, action_attempted, decision, denial_reason_code,
            row_id, env_gate_state, source_ip_hash_or_local_process_id,
            boundary_policy_mode, fixture_version,
            payload_schema_version, payload, payload_sha256, diagnostic_truncated,
            prev_hash, row_hash, checkpoint_id, created_at_ms
        ) VALUES (
            ?1, ?2, ?3, ?4,
            ?5, ?6, ?7, ?8,
            ?9, ?10, ?11, ?12,
            ?13, ?14, ?15,
            ?16, ?17,
            1, ?18, ?19, ?20,
            ?21, ?22, ?23, ?24
        )"#,
    )
    .bind(entry.id)
    .bind(entry.request_id)
    .bind(entry.timestamp_ms)
    .bind(entry.event_type)
    .bind(entry.principal_id)
    .bind(entry.principal_class)
    .bind(entry.caller_class)
    .bind(entry.token_id)
    .bind(entry.transport)
    .bind(entry.action_attempted)
    .bind(entry.decision)
    .bind(entry.denial_reason_code)
    .bind(entry.row_id)
    .bind(entry.env_gate_state)
    .bind(entry.source_ip_hash_or_local_process_id)
    .bind(entry.boundary_policy_mode)
    .bind(entry.fixture_version)
    .bind(entry.payload)
    .bind(&computed_payload_sha256)
    .bind(entry.diagnostic_truncated as i64)
    .bind(prev_hash.as_deref())
    .bind(&row_hash)
    .bind(entry.checkpoint_id)
    .bind(entry.created_at_ms)
    .execute(&mut **tx)
    .await
    .context("append audit_log entry (tx)")?;
    Ok(())
}

/// Append one audit log row in a bounded standalone BEGIN IMMEDIATE transaction.
/// Used by deny-only non-command paths that must open their own audit write
/// unit. Commits before returning so durable evidence is guaranteed before
/// the denial is sent to the caller.
pub async fn append(pool: &SqlitePool, entry: &AuditEntry<'_>) -> Result<()> {
    let mut tx = crate::pool::begin_immediate_with_retry(pool, "audit_log.append").await?;
    append_tx(&mut tx, entry).await?;
    tx.commit()
        .await
        .context("commit standalone audit_log entry")?;
    Ok(())
}

/// Append one checkpoint row inside an existing caller-owned transaction.
/// Checkpoints are written every 1000 rows and at clean shutdown when the
/// open window is non-empty.
pub async fn append_checkpoint_tx<'c>(
    tx: &mut Transaction<'c, Sqlite>,
    entry: &CheckpointEntry<'_>,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO audit_log_checkpoints (
            checkpoint_id, checkpoint_seq,
            covered_start_id, covered_end_id, covered_row_count,
            previous_checkpoint_hash, checkpoint_hash, created_at_ms
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
    )
    .bind(entry.checkpoint_id)
    .bind(entry.checkpoint_seq)
    .bind(entry.covered_start_id)
    .bind(entry.covered_end_id)
    .bind(entry.covered_row_count)
    .bind(entry.previous_checkpoint_hash)
    .bind(entry.checkpoint_hash)
    .bind(entry.created_at_ms)
    .execute(&mut **tx)
    .await
    .context("append audit_log_checkpoints entry")?;
    Ok(())
}

/// SEC-M-004: Verify the latest audit checkpoint window at startup.
///
/// For each row in the covered window, re-derives the row_hash from the stored
/// canonical field values (including a recomputed payload_sha256 from the stored
/// payload bytes). A mismatch between recomputed and stored row_hash detects
/// payload tampering even when the attacker only changes field values and leaves
/// row_hash unchanged. The checkpoint hash is then verified against the
/// (now-validated) stored row_hashes.
///
/// This is bounded: it reads at most the rows in a single checkpoint window
/// (up to 1000 rows per the P081 contract) plus the checkpoint row itself.
pub async fn verify_latest_checkpoint(pool: &SqlitePool) -> AuditIntegrityState {
    // Read the latest checkpoint: covered range + stored hashes + metadata needed to
    // recompute the checkpoint_hash using the full write-time formula.
    let checkpoint: Option<(
        String,
        String,
        String,
        Option<String>,
        String,
        i64,
        i64,
        i64,
    )> = sqlx::query_as(
        "SELECT checkpoint_id, covered_start_id, covered_end_id, previous_checkpoint_hash, \
             checkpoint_hash, checkpoint_seq, covered_row_count, created_at_ms \
             FROM audit_log_checkpoints ORDER BY checkpoint_seq DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    let (
        checkpoint_id,
        covered_start_id,
        covered_end_id,
        prev_checkpoint_hash,
        stored_hash,
        checkpoint_seq,
        covered_row_count,
        created_at_ms,
    ) = match checkpoint {
        Some(row) => row,
        None => return AuditIntegrityState::NoCheckpoints,
    };

    // Fetch ALL canonical fields for the covered window so we can re-derive each
    // row's row_hash from content rather than trusting stored row_hash values.
    // SEC-H-001: Using stored row_hash values directly would leave payload
    // tampering undetected when row_hash is not also updated by the attacker.
    // payload_sha256 is fetched so that truncated rows can use the stored original-bytes
    // hash rather than hashing the truncation envelope (which would always mismatch).
    let rows: Vec<VerifiableAuditRow> = sqlx::query_as(
        "SELECT id, request_id, timestamp_ms, event_type, \
                principal_id, principal_class, caller_class, token_id, \
                transport, action_attempted, decision, denial_reason_code, \
                row_id, env_gate_state, source_ip_hash_or_local_process_id, \
                boundary_policy_mode, fixture_version, payload, payload_sha256, \
                diagnostic_truncated, created_at_ms, prev_hash, row_hash \
         FROM audit_log \
         WHERE rowid >= (SELECT rowid FROM audit_log WHERE id = ?1 LIMIT 1) \
           AND rowid <= (SELECT rowid FROM audit_log WHERE id = ?2 LIMIT 1) \
         ORDER BY rowid ASC",
    )
    .bind(&covered_start_id)
    .bind(&covered_end_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    if rows.is_empty() {
        return AuditIntegrityState::Degraded;
    }

    // Re-derive each row_hash from canonical fields and compare against stored.
    // Any mismatch indicates a field was modified without updating the hash chain.
    let mut verified_row_hashes: Vec<String> = Vec::with_capacity(rows.len());
    for row in &rows {
        let recomputed_row_hash = compute_row_hash_for_verification(row);
        if recomputed_row_hash != row.stored_row_hash {
            return AuditIntegrityState::TamperSuspected;
        }
        verified_row_hashes.push(row.stored_row_hash.clone());
    }

    // Recompute checkpoint_hash = sha256(prev_checkpoint_hash || row_hash_sequence ||
    // metadata) matching the full write-time formula used by write_checkpoint_inner.
    // SEC-M-004: must include checkpoint_seq, covered_start_id, covered_end_id,
    // covered_row_count, and created_at_ms so restarts don't produce false tamper_suspected.
    let mut hasher = Sha256::new();
    hasher.update(prev_checkpoint_hash.as_deref().unwrap_or("").as_bytes());
    for rh in &verified_row_hashes {
        hasher.update(rh.as_bytes());
    }
    let meta = format!(
        "{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}",
        checkpoint_id,
        checkpoint_seq,
        covered_start_id,
        covered_end_id,
        covered_row_count,
        created_at_ms
    );
    hasher.update(meta.as_bytes());
    let recomputed = bytes_to_hex(&hasher.finalize());

    if recomputed == stored_hash {
        AuditIntegrityState::Verified
    } else {
        AuditIntegrityState::TamperSuspected
    }
}

/// Internal struct for checkpoint verification. Fields match the audit_log schema
/// columns needed to recompute row_hash from canonical field values.
#[derive(sqlx::FromRow)]
struct VerifiableAuditRow {
    id: String,
    request_id: String,
    timestamp_ms: i64,
    event_type: String,
    principal_id: Option<String>,
    principal_class: Option<String>,
    caller_class: Option<String>,
    token_id: Option<String>,
    transport: String,
    action_attempted: String,
    decision: String,
    denial_reason_code: Option<String>,
    row_id: Option<String>,
    env_gate_state: Option<String>,
    source_ip_hash_or_local_process_id: Option<String>,
    boundary_policy_mode: String,
    fixture_version: String,
    payload: String,
    /// Stored sha256 written by append_tx. For truncated rows this is sha256 of the
    /// original pre-truncation bytes (not the envelope); needed by
    /// compute_row_hash_for_verification to reproduce the write-time hash.
    payload_sha256: String,
    diagnostic_truncated: i64,
    created_at_ms: i64,
    prev_hash: Option<String>,
    #[sqlx(rename = "row_hash")]
    stored_row_hash: String,
}

/// Re-derive the row_hash from canonical stored field values.
///
/// For non-truncated rows: recomputes payload_sha256 from stored payload bytes so
/// that payload tampering (without also updating payload_sha256 and row_hash) is
/// detected independently.
///
/// For truncated rows (diagnostic_truncated != 0): uses the stored payload_sha256
/// directly because that column holds sha256 of the original pre-truncation bytes —
/// the bytes that were hashed at write time. Hashing the stored truncation envelope
/// instead would always produce a mismatch and report TamperSuspected for every
/// legitimately truncated row.
fn compute_row_hash_for_verification(row: &VerifiableAuditRow) -> String {
    let recomputed_payload_sha256 = if row.diagnostic_truncated != 0 {
        // Trust the stored sha256 for truncated rows: it was computed from the original
        // bytes in append_tx and is part of the tamper-evidenced canonical string itself.
        row.payload_sha256.clone()
    } else {
        let mut h = Sha256::new();
        h.update(row.payload.as_bytes());
        format!("{:x}", h.finalize())
    };
    let prev_hash_str = row.prev_hash.as_deref().unwrap_or("");
    // checkpoint_id is excluded for the same reason as in compute_row_hash:
    // it is a post-hoc backfill and was not present when the row_hash was written.
    let canonical = format!(
        "{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}",
        row.id,
        row.request_id,
        row.timestamp_ms,
        row.event_type,
        row.principal_id.as_deref().unwrap_or(""),
        row.principal_class.as_deref().unwrap_or(""),
        row.caller_class.as_deref().unwrap_or(""),
        row.token_id.as_deref().unwrap_or(""),
        row.transport,
        row.action_attempted,
        row.decision,
        row.denial_reason_code.as_deref().unwrap_or(""),
        row.row_id.as_deref().unwrap_or(""),
        row.env_gate_state.as_deref().unwrap_or(""),
        row.source_ip_hash_or_local_process_id.as_deref().unwrap_or(""),
        row.boundary_policy_mode,
        row.fixture_version,
        1u8, // payload_schema_version — always 1 in current schema
        row.payload,
        recomputed_payload_sha256,
        row.diagnostic_truncated,
        row.created_at_ms,
        prev_hash_str,
    );
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    bytes_to_hex(&hasher.finalize())
}

/// Write a checkpoint covering any unchecked audit_log rows, even if fewer
/// than 1000. Called at clean shutdown to flush the open window per P081 contract.
/// Returns `true` if a checkpoint was written, `false` if the window is empty.
pub async fn write_checkpoint_if_non_empty(pool: &SqlitePool) -> Result<bool> {
    write_checkpoint_inner(pool, 1).await
}

/// Write a checkpoint for the next uncovered window of 1000 audit_log rows.
///
/// Called by the background checkpoint task every interval. Returns `true`
/// when a checkpoint was written, `false` when fewer than 1000 unchecked
/// rows exist (no checkpoint needed yet).
///
/// Also updates the covered rows to set `checkpoint_id` so the retention
/// cleanup (`delete_old_rows`) can identify checkpointed rows.
pub async fn write_checkpoint_if_needed(pool: &SqlitePool) -> Result<bool> {
    write_checkpoint_inner(pool, 1000).await
}

async fn write_checkpoint_inner(pool: &SqlitePool, min_unchecked: i64) -> Result<bool> {
    // Find the rowid of the last checkpoint's covered_end row (0 if no checkpoints).
    let last_covered_rowid: i64 = {
        let end_id: Option<String> = sqlx::query_scalar(
            "SELECT covered_end_id FROM audit_log_checkpoints \
             ORDER BY checkpoint_seq DESC LIMIT 1",
        )
        .fetch_optional(pool)
        .await
        .context("checkpoint covered_end_id lookup")?;

        match end_id {
            None => 0i64,
            Some(id) => {
                sqlx::query_scalar::<_, i64>("SELECT rowid FROM audit_log WHERE id = ? LIMIT 1")
                    .bind(&id)
                    .fetch_optional(pool)
                    .await
                    .context("covered_end rowid lookup")?
                    .unwrap_or(0)
            }
        }
    };

    // Count unchecked rows (written after the last covered row).
    let unchecked: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_log WHERE rowid > ?")
        .bind(last_covered_rowid)
        .fetch_one(pool)
        .await
        .context("unchecked rows count")?;

    if unchecked < min_unchecked {
        return Ok(false);
    }

    // Fetch the first 1000 unchecked rows: id, row_hash, rowid.
    let rows: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT id, row_hash, rowid FROM audit_log \
         WHERE rowid > ? \
         ORDER BY rowid ASC LIMIT 1000",
    )
    .bind(last_covered_rowid)
    .fetch_all(pool)
    .await
    .context("fetch checkpoint window")?;

    if rows.is_empty() {
        return Ok(false);
    }

    let first_id = rows.first().unwrap().0.clone();
    let last_id = rows.last().unwrap().0.clone();
    let last_rowid = rows.last().unwrap().2;
    let row_count = rows.len() as i64;

    // Get the previous checkpoint hash and next sequence number.
    let prev_cp: Option<(String, i64)> = sqlx::query_as(
        "SELECT checkpoint_hash, checkpoint_seq \
         FROM audit_log_checkpoints ORDER BY checkpoint_seq DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .context("prev checkpoint hash/seq")?;
    let (prev_cp_hash, prev_cp_seq) = match prev_cp {
        Some((h, s)) => (Some(h), s),
        None => (None, 0),
    };
    let next_seq = prev_cp_seq + 1;

    // Compute checkpoint_hash = sha256(prev_hash || row_hash_sequence || checkpoint metadata).
    // Checkpoint metadata includes: checkpoint_id, checkpoint_seq, covered_start_id,
    // covered_end_id, covered_row_count, and created_at_ms — all separated by 0x1F.
    // This binds the hash to the specific window covered and prevents reuse.
    let cp_id = uuid::Uuid::now_v7().to_string();
    let now_ms = chrono::Utc::now().timestamp_millis();
    let checkpoint_hash = {
        let mut h = Sha256::new();
        h.update(prev_cp_hash.as_deref().unwrap_or("").as_bytes());
        for (_, rh, _) in &rows {
            h.update(rh.as_bytes());
        }
        // Checkpoint metadata fields separated by unit separator (0x1F)
        let meta = format!(
            "{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}",
            cp_id, next_seq, first_id, last_id, row_count, now_ms
        );
        h.update(meta.as_bytes());
        bytes_to_hex(&h.finalize())
    };
    let mut tx = crate::pool::begin_immediate_with_retry(pool, "audit_checkpoint_write").await?;

    let cp = CheckpointEntry {
        checkpoint_id: &cp_id,
        checkpoint_seq: next_seq,
        covered_start_id: &first_id,
        covered_end_id: &last_id,
        covered_row_count: row_count,
        previous_checkpoint_hash: prev_cp_hash.as_deref(),
        checkpoint_hash: &checkpoint_hash,
        created_at_ms: now_ms,
    };
    append_checkpoint_tx(&mut tx, &cp).await?;

    // Stamp checkpoint_id on the covered rows so retention cleanup can identify them.
    sqlx::query(
        "UPDATE audit_log SET checkpoint_id = ? \
         WHERE rowid > ? AND rowid <= ?",
    )
    .bind(&cp_id)
    .bind(last_covered_rowid)
    .bind(last_rowid)
    .execute(&mut *tx)
    .await
    .context("stamp checkpoint_id on covered rows")?;

    tx.commit().await.context("commit audit checkpoint")?;
    Ok(true)
}

/// Bounded health snapshot for operator diagnostics. Never exposes raw audit
/// rows or unbounded query surfaces.
///
/// For startup tamper detection use `verify_latest_checkpoint` separately.
pub async fn health_snapshot(pool: &SqlitePool) -> Result<AuditLogHealthSnapshot> {
    let row_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_log")
        .fetch_one(pool)
        .await
        .context("audit_log row count")?;

    let latest_row: Option<(String, i64)> = sqlx::query_as(
        "SELECT id, created_at_ms FROM audit_log ORDER BY created_at_ms DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .context("audit_log latest row")?;
    let (latest_row_id, last_write_ok_at_ms) = latest_row
        .map(|(id, ts)| (Some(id), Some(ts)))
        .unwrap_or((None, None));

    let checkpoint_row: Option<(i64, String)> = sqlx::query_as(
        "SELECT checkpoint_seq, checkpoint_hash FROM audit_log_checkpoints ORDER BY checkpoint_seq DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .context("audit_log latest checkpoint")?;

    let (latest_checkpoint_seq, latest_checkpoint_hash) = match checkpoint_row {
        Some((seq, hash)) => (Some(seq), Some(hash)),
        None => (None, None),
    };

    let cutoff_ms = chrono::Utc::now().timestamp_millis() - RETENTION_MIN_MS;
    let cleanup_eligible_row_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log \
         WHERE created_at_ms < ?1 \
           AND checkpoint_id IS NOT NULL",
    )
    .bind(cutoff_ms)
    .fetch_one(pool)
    .await
    .context("audit_log cleanup eligible row count")?;
    let cleanup_protected_row_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log \
         WHERE created_at_ms < ?1 \
           AND checkpoint_id IS NULL",
    )
    .bind(cutoff_ms)
    .fetch_one(pool)
    .await
    .context("audit_log cleanup protected row count")?;
    let payload_used_bytes: i64 =
        sqlx::query_scalar("SELECT COALESCE(SUM(LENGTH(payload)), 0) FROM audit_log")
            .fetch_one(pool)
            .await
            .context("audit_log payload used bytes")?;
    let writable = sqlx::query_scalar::<_, i64>(
        "SELECT CASE WHEN EXISTS (SELECT 1 FROM sqlite_master WHERE type='table' AND name='audit_log') THEN 1 ELSE 0 END",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0)
        == 1;
    let cleanup_state = if cleanup_eligible_row_count > 0 {
        "cleanup_due"
    } else if cleanup_protected_row_count > 0 {
        "waiting_for_checkpoint"
    } else {
        "healthy"
    }
    .to_string();

    let payload_budget_bytes = row_count.saturating_mul(MAX_PAYLOAD_BYTES as i64);
    let payload_budget_used_percent = if payload_budget_bytes > 0 {
        ((payload_used_bytes.saturating_mul(100)) / payload_budget_bytes).clamp(0, 100)
    } else {
        0
    };
    let payload_budget_state = if payload_budget_used_percent >= 95 {
        crate::metrics::record_p081_audit_log_rate_limited("audit_log", "AUDIT_BUDGET_EXHAUSTED");
        "read_only_safe_mode"
    } else if payload_budget_used_percent >= 80 {
        "warning"
    } else {
        "healthy"
    }
    .to_string();

    let half_open_probe_success_count =
        if writable && payload_budget_state == "healthy" && payload_budget_used_percent < 80 {
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM (
                    SELECT id FROM audit_log
                    WHERE event_type = 'audit_budget_half_open_probe'
                      AND decision = 'allow'
                    ORDER BY created_at_ms DESC
                    LIMIT 3
                )",
            )
            .fetch_one(pool)
            .await
            .context("audit_log half-open probe success count")?
        } else {
            0
        };

    Ok(AuditLogHealthSnapshot {
        row_count,
        latest_row_id,
        latest_checkpoint_seq,
        latest_checkpoint_hash,
        writable,
        last_write_ok_at_ms,
        consecutive_failures: if writable { 0 } else { 1 },
        cumulative_failures: if writable { 0 } else { 1 },
        retention_min_days: RETENTION_MIN_DAYS,
        cleanup_state,
        cleanup_eligible_row_count,
        cleanup_protected_row_count,
        budget_bytes: payload_budget_bytes,
        used_bytes: payload_used_bytes,
        payload_budget_bytes,
        payload_used_bytes,
        payload_budget_state,
        payload_budget_used_percent,
        half_open_probe_success_count,
        shadow_coverage_report_ref: "docs/evidence/boundary-policy-shadow-coverage/report.json"
            .to_string(),
    })
}

/// Minimum local retention period: 90 days expressed in milliseconds.
pub const RETENTION_MIN_DAYS: i64 = 90;
const RETENTION_MIN_MS: i64 = RETENTION_MIN_DAYS * 24 * 60 * 60 * 1000;

/// Delete audit_log rows that are older than the retention window and whose
/// covering checkpoint has been recorded (i.e. `checkpoint_id IS NOT NULL`).
///
/// Safety: only rows with a recorded `checkpoint_id` are removed — orphaned
/// rows without a checkpoint are preserved until their window is closed and
/// recorded. Rows whose `created_at_ms > older_than_ms` are never touched.
///
/// Run outside request-handling transactions (e.g. from a periodic background
/// task). Returns the number of rows deleted.
///
/// The cleanup does NOT delete the covering `audit_log_checkpoints` rows; those
/// act as a compact proof-of-existence record and can be pruned independently
/// once the operator has taken an external backup.
pub async fn delete_old_rows(pool: &SqlitePool) -> Result<u64> {
    let started = std::time::Instant::now();
    let cutoff_ms = chrono::Utc::now().timestamp_millis() - RETENTION_MIN_MS;
    let result = sqlx::query(
        "DELETE FROM audit_log \
         WHERE created_at_ms < ?1 \
           AND checkpoint_id IS NOT NULL",
    )
    .bind(cutoff_ms)
    .execute(pool)
    .await
    .context("audit_log retention cleanup")?;
    crate::metrics::record_p081_audit_budget_cleanup_duration(started.elapsed());
    let health = health_snapshot(pool).await?;
    if health.writable
        && health.payload_budget_state == "healthy"
        && health.payload_budget_used_percent < 80
        && health.half_open_probe_success_count < 3
    {
        append_half_open_recovery_probes(pool, 3 - health.half_open_probe_success_count).await?;
    }
    Ok(result.rows_affected())
}

pub async fn audit_budget_requires_safe_mode(pool: &SqlitePool) -> Result<bool> {
    Ok(health_snapshot(pool).await?.payload_budget_state == "read_only_safe_mode")
}

async fn append_half_open_recovery_probes(pool: &SqlitePool, missing: i64) -> Result<()> {
    let missing = missing.clamp(0, 3);
    for idx in 0..missing {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let id = format!("audit-budget-half-open-{}", uuid::Uuid::now_v7());
        let request_id = format!("audit-budget-half-open-probe-{idx}");
        let payload = serde_json::json!({
            "schema_version": "audit_budget_half_open_probe_v1",
            "probe_index": idx + 1,
            "target_probe_count": 3
        })
        .to_string();
        let entry = AuditEntry {
            id: &id,
            request_id: &request_id,
            timestamp_ms: now_ms,
            event_type: "audit_budget_half_open_probe",
            principal_id: None,
            principal_class: None,
            caller_class: None,
            token_id: None,
            transport: "mcp_tools_call",
            action_attempted: "half_open_probe",
            decision: "allow",
            denial_reason_code: None,
            row_id: Some("p081.audit_budget.recovery.half_open_probe"),
            env_gate_state: None,
            source_ip_hash_or_local_process_id: None,
            boundary_policy_mode: "read_only_safe_mode",
            fixture_version: "p081-boundary-matrix-v1",
            payload: &payload,
            original_payload_bytes: None,
            diagnostic_truncated: false,
            checkpoint_id: None,
            created_at_ms: now_ms,
        };
        append(pool, &entry).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::create_pool;
    use std::sync::Mutex;

    static METRICS_TEST_MUTEX: Mutex<()> = Mutex::new(());

    fn metrics_test_lock() -> std::sync::MutexGuard<'static, ()> {
        METRICS_TEST_MUTEX.lock().unwrap()
    }

    async fn test_pool() -> SqlitePool {
        create_pool("sqlite::memory:").await.unwrap()
    }

    fn make_entry<'a>(id: &'a str, request_id: &'a str) -> AuditEntry<'a> {
        AuditEntry {
            id,
            request_id,
            timestamp_ms: 1_000_000,
            event_type: "boundary_decision",
            principal_id: Some("default-operator"),
            principal_class: Some("operator"),
            caller_class: Some("ui_operator"),
            token_id: None,
            transport: "graphql_query",
            action_attempted: "runs.get",
            decision: "allow",
            denial_reason_code: None,
            row_id: Some("p081.ui_operator.graphql_query.read"),
            env_gate_state: None,
            source_ip_hash_or_local_process_id: None,
            boundary_policy_mode: "shadow",
            fixture_version: "p081-v1",
            payload: r#"{"event":"boundary_decision"}"#,
            original_payload_bytes: None,
            diagnostic_truncated: false,
            checkpoint_id: None,
            created_at_ms: 1_000_000,
        }
    }

    #[tokio::test]
    async fn append_and_health_roundtrip() {
        let _metrics_lock = metrics_test_lock();
        crate::metrics::reset_for_tests();
        let pool = test_pool().await;
        let entry = make_entry("entry-001", "req-001");
        append(&pool, &entry).await.unwrap();
        let snap = health_snapshot(&pool).await.unwrap();
        assert_eq!(snap.row_count, 1);
        assert_eq!(snap.latest_row_id.as_deref(), Some("entry-001"));
        assert!(snap.latest_checkpoint_seq.is_none());
        assert!(snap.writable);
        assert_eq!(snap.retention_min_days, 90);
        assert_eq!(snap.cleanup_state, "waiting_for_checkpoint");
        assert_eq!(snap.cleanup_eligible_row_count, 0);
        assert_eq!(snap.cleanup_protected_row_count, 1);
        assert!(snap.payload_budget_bytes >= snap.payload_used_bytes);
        assert_eq!(
            snap.shadow_coverage_report_ref,
            "docs/evidence/boundary-policy-shadow-coverage/report.json"
        );
    }

    #[tokio::test]
    async fn p081_audit_budget_warning_and_safe_mode_emit_runtime_readback_and_metrics() {
        let _metrics_lock = metrics_test_lock();
        crate::metrics::reset_for_tests();
        let pool = test_pool().await;

        let warning_payload = "w".repeat(14_000);
        let mut warning = make_entry("entry-budget-warning", "req-budget-warning");
        warning.payload = &warning_payload;
        append(&pool, &warning).await.unwrap();

        let warning_snap = health_snapshot(&pool).await.unwrap();
        assert_eq!(warning_snap.payload_budget_state, "warning");
        assert!(warning_snap.payload_budget_used_percent >= 80);
        assert_eq!(
            crate::metrics::get_counter("audit_log_rate_limited_total"),
            0,
            "warning at 80 percent must not be reported as rate-limited"
        );

        let safe_pool = test_pool().await;
        let safe_payload = "s".repeat(16_100);
        let mut safe_mode = make_entry("entry-budget-safe-mode", "req-budget-safe-mode");
        safe_mode.payload = &safe_payload;
        append(&safe_pool, &safe_mode).await.unwrap();

        let safe_snap = health_snapshot(&safe_pool).await.unwrap();
        assert_eq!(safe_snap.payload_budget_state, "read_only_safe_mode");
        assert!(safe_snap.payload_budget_used_percent >= 95);
        assert_eq!(
            safe_snap.half_open_probe_success_count, 0,
            "budget recovery starts with zero half-open writes until cleanup lowers usage"
        );
        assert!(
            crate::metrics::get_counter("audit_log_rate_limited_total") > 0,
            "crossing the 95 percent audit budget must emit production rate-limit telemetry"
        );
    }

    #[tokio::test]
    async fn p081_audit_budget_recovery_exits_after_cleanup_and_half_open_probes() {
        let _metrics_lock = metrics_test_lock();
        crate::metrics::reset_for_tests();
        let pool = test_pool().await;

        let safe_payload = "s".repeat(16_100);
        let mut safe_mode = make_entry("entry-budget-recover", "req-budget-recover");
        safe_mode.payload = &safe_payload;
        append(&pool, &safe_mode).await.unwrap();
        sqlx::query(
            "UPDATE audit_log \
             SET checkpoint_id = 'cp-recover', created_at_ms = ?1 \
             WHERE id = 'entry-budget-recover'",
        )
        .bind(chrono::Utc::now().timestamp_millis() - RETENTION_MIN_MS - 1_000)
        .execute(&pool)
        .await
        .unwrap();

        let safe_snap = health_snapshot(&pool).await.unwrap();
        assert_eq!(safe_snap.payload_budget_state, "read_only_safe_mode");
        assert_eq!(safe_snap.half_open_probe_success_count, 0);

        let deleted = delete_old_rows(&pool).await.unwrap();
        assert_eq!(deleted, 1);

        let recovered = health_snapshot(&pool).await.unwrap();
        assert_eq!(recovered.payload_budget_state, "healthy");
        assert!(recovered.payload_budget_used_percent < 80);
        assert_eq!(
            recovered.half_open_probe_success_count, 3,
            "P081 safe-mode exit requires three successful half-open audit probes"
        );
        assert_eq!(
            crate::metrics::get_hot_read_sample_count("audit_budget_cleanup_duration_ms"),
            1,
            "cleanup progress must emit the P081 cleanup duration metric"
        );
    }

    #[tokio::test]
    async fn append_tx_inside_transaction() {
        let pool = test_pool().await;
        let entry = AuditEntry {
            id: "tx-entry-001",
            request_id: "req-tx-001",
            timestamp_ms: 2_000_000,
            event_type: "boundary_decision",
            principal_id: None,
            principal_class: None,
            caller_class: Some("agent_operator"),
            token_id: None,
            transport: "mcp_tools_call",
            action_attempted: "runs.start",
            decision: "deny",
            denial_reason_code: Some("CAPABILITY_OUT_OF_SCOPE"),
            row_id: Some("p081.agent_operator.mcp_tools_call.command"),
            env_gate_state: None,
            source_ip_hash_or_local_process_id: None,
            boundary_policy_mode: "enforce",
            fixture_version: "p081-v1",
            payload: r#"{"event":"deny"}"#,
            original_payload_bytes: None,
            diagnostic_truncated: false,
            checkpoint_id: None,
            created_at_ms: 2_000_000,
        };

        let mut tx = crate::pool::begin_immediate_with_retry(&pool, "test_tx")
            .await
            .unwrap();
        append_tx(&mut tx, &entry).await.unwrap();
        tx.commit().await.unwrap();

        let snap = health_snapshot(&pool).await.unwrap();
        assert_eq!(snap.row_count, 1);
    }

    #[tokio::test]
    async fn append_checkpoint_tx_roundtrip() {
        let pool = test_pool().await;
        let row_entry = AuditEntry {
            id: "cp-entry-001",
            request_id: "req-cp-001",
            timestamp_ms: 3_000_000,
            event_type: "boundary_decision",
            principal_id: None,
            principal_class: None,
            caller_class: Some("ui_operator"),
            token_id: None,
            transport: "graphql_mutation",
            action_attempted: "approveApproval",
            decision: "allow",
            denial_reason_code: None,
            row_id: Some("p081.ui_operator.graphql_mutation.approval_action"),
            env_gate_state: None,
            source_ip_hash_or_local_process_id: None,
            boundary_policy_mode: "enforce",
            fixture_version: "p081-v1",
            payload: r#"{"event":"allow"}"#,
            original_payload_bytes: None,
            diagnostic_truncated: false,
            checkpoint_id: Some("cp-001"),
            created_at_ms: 3_000_000,
        };

        let checkpoint_hash = "9".repeat(64);
        let cp = CheckpointEntry {
            checkpoint_id: "cp-001",
            checkpoint_seq: 1,
            covered_start_id: "cp-entry-001",
            covered_end_id: "cp-entry-001",
            covered_row_count: 1,
            previous_checkpoint_hash: None,
            checkpoint_hash: &checkpoint_hash,
            created_at_ms: 3_000_001,
        };

        let mut tx = crate::pool::begin_immediate_with_retry(&pool, "test_cp")
            .await
            .unwrap();
        append_tx(&mut tx, &row_entry).await.unwrap();
        append_checkpoint_tx(&mut tx, &cp).await.unwrap();
        tx.commit().await.unwrap();

        let snap = health_snapshot(&pool).await.unwrap();
        assert_eq!(snap.row_count, 1);
        assert_eq!(snap.latest_checkpoint_seq, Some(1));
        assert_eq!(
            snap.latest_checkpoint_hash.as_deref(),
            Some(&checkpoint_hash as &str)
        );
    }

    // ── SEC-006 regression: repo owns hash chain, callers cannot break it ──

    #[tokio::test]
    async fn hash_chain_is_computed_internally() {
        let pool = test_pool().await;

        // First entry: prev_hash should be None (first row), row_hash computed by repo.
        let entry1 = make_entry("chain-001", "req-chain-001");
        append(&pool, &entry1).await.unwrap();

        // Second entry: prev_hash should equal first row's row_hash, chained by repo.
        let entry2 = make_entry("chain-002", "req-chain-002");
        append(&pool, &entry2).await.unwrap();

        // Verify both rows exist and the chain is non-trivial (row_hash non-empty, not equal).
        let row1_hash: Option<String> =
            sqlx::query_scalar("SELECT row_hash FROM audit_log WHERE id = 'chain-001'")
                .fetch_optional(&pool)
                .await
                .unwrap();
        let row2_hash: Option<String> =
            sqlx::query_scalar("SELECT row_hash FROM audit_log WHERE id = 'chain-002'")
                .fetch_optional(&pool)
                .await
                .unwrap();
        // Use Option<String> decode to handle the nullable prev_hash column.
        let row2_prev: Option<Option<String>> = sqlx::query_scalar::<_, Option<String>>(
            "SELECT prev_hash FROM audit_log WHERE id = 'chain-002'",
        )
        .fetch_optional(&pool)
        .await
        .unwrap();

        let h1 = row1_hash.unwrap();
        let h2 = row2_hash.unwrap();
        // row2_prev is Some(Some(hash)) since it should have a non-null prev_hash.
        let p2 = row2_prev
            .expect("chain-002 row must exist")
            .expect("chain-002 prev_hash must not be NULL");

        // row_hash must be 64-char hex SHA-256.
        assert_eq!(h1.len(), 64, "row1 hash must be 64 hex chars");
        assert_eq!(h2.len(), 64, "row2 hash must be 64 hex chars");
        // Chain: row2.prev_hash == row1.row_hash.
        assert_eq!(p2, h1, "row2 prev_hash must equal row1 row_hash");
        // Hashes must differ (different inputs).
        assert_ne!(h1, h2, "consecutive row hashes must differ");
    }

    #[tokio::test]
    async fn first_row_has_null_prev_hash() {
        let pool = test_pool().await;
        let entry = make_entry("first-row", "req-first");
        append(&pool, &entry).await.unwrap();

        // Use SQL IS NULL check since sqlx cannot decode a NULL column as String.
        let is_null: i64 = sqlx::query_scalar(
            "SELECT CASE WHEN prev_hash IS NULL THEN 1 ELSE 0 END \
             FROM audit_log WHERE id = 'first-row'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(is_null, 1, "first row prev_hash must be NULL");
    }

    // ── SEC-P081-001 regression: free-text fields must not contain 0x1F delimiter ──

    #[tokio::test]
    async fn rejects_field_with_delimiter_byte_in_action_attempted() {
        let pool = test_pool().await;
        let mut entry = make_entry("sec-p081-001", "req-sec-001");
        // Inject the 0x1F delimiter into a free-text field.
        let malicious_action = "runs.get\x1ftransport_hacked";
        entry.action_attempted = malicious_action;
        let result = append(&pool, &entry).await;
        assert!(
            result.is_err(),
            "append must reject action_attempted containing 0x1F delimiter byte"
        );
        // DB must remain clean — no row inserted.
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_log")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            count, 0,
            "no row should be inserted when field contains delimiter"
        );
    }

    #[tokio::test]
    async fn rejects_field_with_delimiter_byte_in_payload() {
        let pool = test_pool().await;
        let mut entry = make_entry("sec-p081-001b", "req-sec-001b");
        entry.payload = "{\"event\":\"\x1fpayload_injection\"}";
        let result = append(&pool, &entry).await;
        assert!(
            result.is_err(),
            "append must reject payload containing 0x1F delimiter byte"
        );
    }

    // ── SEC-P081-001 extended: all remaining free-text hash fields ──

    #[tokio::test]
    async fn rejects_delimiter_in_transport() {
        let pool = test_pool().await;
        let mut entry = make_entry("sec-transport", "req-transport");
        entry.transport = "graphql_query\x1finject";
        assert!(append(&pool, &entry).await.is_err());
    }

    #[tokio::test]
    async fn rejects_delimiter_in_decision() {
        let pool = test_pool().await;
        let mut entry = make_entry("sec-decision", "req-decision");
        entry.decision = "allow\x1finject";
        assert!(append(&pool, &entry).await.is_err());
    }

    #[tokio::test]
    async fn rejects_delimiter_in_boundary_policy_mode() {
        let pool = test_pool().await;
        let mut entry = make_entry("sec-mode", "req-mode");
        entry.boundary_policy_mode = "shadow\x1finject";
        assert!(append(&pool, &entry).await.is_err());
    }

    #[tokio::test]
    async fn rejects_delimiter_in_fixture_version() {
        let pool = test_pool().await;
        let mut entry = make_entry("sec-fixture", "req-fixture");
        entry.fixture_version = "p081-v1\x1finject";
        assert!(append(&pool, &entry).await.is_err());
    }

    #[tokio::test]
    async fn rejects_delimiter_in_principal_id() {
        let pool = test_pool().await;
        let mut entry = make_entry("sec-principal", "req-principal");
        entry.principal_id = Some("op\x1finject");
        assert!(append(&pool, &entry).await.is_err());
    }

    #[tokio::test]
    async fn rejects_delimiter_in_row_id() {
        let pool = test_pool().await;
        let mut entry = make_entry("sec-rowid", "req-rowid");
        entry.row_id = Some("p081.ui_operator.graphql_query.read\x1finject");
        assert!(append(&pool, &entry).await.is_err());
    }

    // ── SEC-M-004: verify_latest_checkpoint correctness ─────────────────

    #[tokio::test]
    async fn verify_latest_checkpoint_returns_no_checkpoints_when_empty() {
        let pool = test_pool().await;
        let state = verify_latest_checkpoint(&pool).await;
        assert_eq!(state, AuditIntegrityState::NoCheckpoints);
    }

    #[tokio::test]
    async fn verify_latest_checkpoint_returns_tamper_suspected_for_wrong_hash() {
        let pool = test_pool().await;
        // Append a row and a checkpoint with a deliberately wrong hash.
        let row_entry = make_entry("vcp-entry-001", "req-vcp-001");
        let mut tx = crate::pool::begin_immediate_with_retry(&pool, "test_vcp")
            .await
            .unwrap();
        append_tx(&mut tx, &row_entry).await.unwrap();
        let cp = CheckpointEntry {
            checkpoint_id: "vcp-cp-001",
            checkpoint_seq: 1,
            covered_start_id: "vcp-entry-001",
            covered_end_id: "vcp-entry-001",
            covered_row_count: 1,
            previous_checkpoint_hash: None,
            checkpoint_hash: &"f".repeat(64), // deliberately wrong
            created_at_ms: 4_000_000,
        };
        append_checkpoint_tx(&mut tx, &cp).await.unwrap();
        tx.commit().await.unwrap();

        let state = verify_latest_checkpoint(&pool).await;
        assert_eq!(
            state,
            AuditIntegrityState::TamperSuspected,
            "wrong stored hash must be reported as tamper_suspected"
        );
    }

    // SEC-H-001: Verify that tampering with a row's payload (without updating row_hash)
    // is detected by verify_latest_checkpoint after re-deriving row hashes from content.
    #[tokio::test]
    async fn verify_detects_payload_tamper_unchanged_row_hash() {
        let pool = test_pool().await;
        // Write a valid row + checkpoint.
        let row_entry = make_entry("tamper-entry-001", "req-tamper-001");
        let mut tx = crate::pool::begin_immediate_with_retry(&pool, "test_tamper_setup")
            .await
            .unwrap();
        append_tx(&mut tx, &row_entry).await.unwrap();
        tx.commit().await.unwrap();

        // Retrieve the stored row_hash.
        let row_hash: String =
            sqlx::query_scalar("SELECT row_hash FROM audit_log WHERE id = 'tamper-entry-001'")
                .fetch_one(&pool)
                .await
                .unwrap();

        // Compute a correct checkpoint hash using the full write-time metadata formula.
        let expected_cp_hash = {
            use sha2::{Digest, Sha256};
            let mut h = Sha256::new();
            h.update(b"");
            h.update(row_hash.as_bytes());
            let meta = format!(
                "{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}",
                "tamper-cp-001", 1i64, "tamper-entry-001", "tamper-entry-001", 1i64, 5_000_000i64
            );
            h.update(meta.as_bytes());
            bytes_to_hex(&h.finalize())
        };
        let mut tx = crate::pool::begin_immediate_with_retry(&pool, "test_tamper_cp")
            .await
            .unwrap();
        let cp = CheckpointEntry {
            checkpoint_id: "tamper-cp-001",
            checkpoint_seq: 1,
            covered_start_id: "tamper-entry-001",
            covered_end_id: "tamper-entry-001",
            covered_row_count: 1,
            previous_checkpoint_hash: None,
            checkpoint_hash: &expected_cp_hash,
            created_at_ms: 5_000_000,
        };
        append_checkpoint_tx(&mut tx, &cp).await.unwrap();
        tx.commit().await.unwrap();

        // Sanity: verify reports Verified before tampering.
        assert_eq!(
            verify_latest_checkpoint(&pool).await,
            AuditIntegrityState::Verified
        );

        // Simulate payload tampering: directly update the payload column WITHOUT
        // updating row_hash or payload_sha256. This is the attack scenario where
        // the attacker modifies an audit decision record in the DB.
        sqlx::query(
            "UPDATE audit_log SET payload = '{\"event\":\"tampered\",\"decision\":\"allow\"}' \
             WHERE id = 'tamper-entry-001'",
        )
        .execute(&pool)
        .await
        .unwrap();

        // After tampering, verify must detect TamperSuspected because the re-derived
        // row_hash (from the new payload) will not match the stored row_hash.
        assert_eq!(
            verify_latest_checkpoint(&pool).await,
            AuditIntegrityState::TamperSuspected,
            "payload tamper with unchanged row_hash must be detected"
        );
    }

    // SEC-M-004: Verify that a checkpoint covering a truncated audit row reports
    // Verified rather than TamperSuspected. This is the regression test for the
    // bug where compute_row_hash_for_verification always hashed the stored payload
    // bytes (the truncation envelope) instead of the original pre-truncation sha256.
    #[tokio::test]
    async fn verify_checkpoint_with_truncated_payload_row_reports_verified() {
        let pool = test_pool().await;

        // Build a large payload that exceeds MAX_PAYLOAD_BYTES so build_envelope truncates it.
        let large_payload = "x".repeat(MAX_PAYLOAD_BYTES + 1);
        let (envelope, original_sha256, truncated) = build_envelope(&large_payload);
        assert!(truncated, "payload must trigger truncation for this test");

        let entry = AuditEntry {
            id: "trunc-entry-001",
            request_id: "req-trunc-001",
            timestamp_ms: 9_000_000,
            event_type: "boundary_decision",
            principal_id: Some("default-operator"),
            principal_class: Some("operator"),
            caller_class: Some("ui_operator"),
            token_id: None,
            transport: "graphql_query",
            action_attempted: "runs.get",
            decision: "allow",
            denial_reason_code: None,
            row_id: Some("p081.ui_operator.graphql_query.read"),
            env_gate_state: None,
            source_ip_hash_or_local_process_id: None,
            boundary_policy_mode: "enforce",
            fixture_version: "p081-v1",
            // payload is the truncation envelope; original_payload_bytes triggers
            // sha256(original) to be stored in the DB instead of sha256(envelope).
            payload: &envelope,
            original_payload_bytes: Some(&large_payload),
            diagnostic_truncated: truncated,
            checkpoint_id: None,
            created_at_ms: 9_000_000,
        };
        let _ = original_sha256; // used indirectly via append_tx

        let mut tx = crate::pool::begin_immediate_with_retry(&pool, "test_trunc")
            .await
            .unwrap();
        append_tx(&mut tx, &entry).await.unwrap();
        tx.commit().await.unwrap();

        // Read back the row_hash that append_tx wrote.
        let row_hash: String =
            sqlx::query_scalar("SELECT row_hash FROM audit_log WHERE id = 'trunc-entry-001'")
                .fetch_one(&pool)
                .await
                .unwrap();

        // Compute the expected checkpoint hash using the full write-time metadata formula.
        let expected_cp_hash = {
            use sha2::{Digest, Sha256};
            let mut h = Sha256::new();
            h.update(b"");
            h.update(row_hash.as_bytes());
            let meta = format!(
                "{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}",
                "trunc-cp-001", 1i64, "trunc-entry-001", "trunc-entry-001", 1i64, 9_000_001i64
            );
            h.update(meta.as_bytes());
            bytes_to_hex(&h.finalize())
        };

        let mut tx = crate::pool::begin_immediate_with_retry(&pool, "test_trunc_cp")
            .await
            .unwrap();
        let cp = CheckpointEntry {
            checkpoint_id: "trunc-cp-001",
            checkpoint_seq: 1,
            covered_start_id: "trunc-entry-001",
            covered_end_id: "trunc-entry-001",
            covered_row_count: 1,
            previous_checkpoint_hash: None,
            checkpoint_hash: &expected_cp_hash,
            created_at_ms: 9_000_001,
        };
        append_checkpoint_tx(&mut tx, &cp).await.unwrap();
        tx.commit().await.unwrap();

        assert_eq!(
            verify_latest_checkpoint(&pool).await,
            AuditIntegrityState::Verified,
            "checkpoint covering a truncated row must report Verified, not TamperSuspected"
        );
    }

    #[tokio::test]
    async fn verify_latest_checkpoint_returns_verified_for_correct_hash() {
        let pool = test_pool().await;
        // Append a row, retrieve its row_hash, compute the expected checkpoint hash,
        // store the checkpoint with the correct hash, and verify.
        let row_entry = make_entry("vcp2-entry-001", "req-vcp2-001");
        let mut tx = crate::pool::begin_immediate_with_retry(&pool, "test_vcp2")
            .await
            .unwrap();
        append_tx(&mut tx, &row_entry).await.unwrap();
        tx.commit().await.unwrap();

        // Read back the row_hash written by the repo.
        let row_hash: String =
            sqlx::query_scalar("SELECT row_hash FROM audit_log WHERE id = 'vcp2-entry-001'")
                .fetch_one(&pool)
                .await
                .unwrap();

        // Recompute the expected checkpoint hash using the full write-time metadata formula:
        // sha256(prev_hash_or_empty || row_hash_sequence || "{cp_id}\x1f{seq}\x1f{start}\x1f{end}\x1f{count}\x1f{ts}")
        let expected_cp_hash = {
            use sha2::{Digest, Sha256};
            let mut h = Sha256::new();
            h.update(b""); // no previous checkpoint
            h.update(row_hash.as_bytes());
            let meta = format!(
                "{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}",
                "vcp2-cp-001", 1i64, "vcp2-entry-001", "vcp2-entry-001", 1i64, 4_000_001i64
            );
            h.update(meta.as_bytes());
            bytes_to_hex(&h.finalize())
        };

        let cp = CheckpointEntry {
            checkpoint_id: "vcp2-cp-001",
            checkpoint_seq: 1,
            covered_start_id: "vcp2-entry-001",
            covered_end_id: "vcp2-entry-001",
            covered_row_count: 1,
            previous_checkpoint_hash: None,
            checkpoint_hash: &expected_cp_hash,
            created_at_ms: 4_000_001,
        };
        let mut tx = crate::pool::begin_immediate_with_retry(&pool, "test_vcp2_cp")
            .await
            .unwrap();
        append_checkpoint_tx(&mut tx, &cp).await.unwrap();
        tx.commit().await.unwrap();

        let state = verify_latest_checkpoint(&pool).await;
        assert_eq!(
            state,
            AuditIntegrityState::Verified,
            "correctly-hashed checkpoint must be verified"
        );
    }
}
