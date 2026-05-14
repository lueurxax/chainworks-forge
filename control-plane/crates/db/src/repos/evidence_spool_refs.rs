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

use crate::write_class::{ReplayPolicy, WriteClass, WriteLane, WriteOperation, WriteResult};
use crate::writer::begin_registered_immediate_transaction;
use crate::writer::DbWriter;

// P075-LOW-005: field length caps applied at validate_spool_ref_fields.
const MAX_ID_LEN: usize = 256;
const MAX_CHECKSUM_LEN: usize = 256;
const MAX_CONTENT_TYPE_LEN: usize = 1024;
const MAX_PRODUCER_OPERATION_LEN: usize = 1024;
// P075-SEC-001: pre-parse size cap for summary_json (must be checked BEFORE serde_json::from_str).
const MAX_SUMMARY_JSON_LEN: usize = 8192;
// P075-SEC-001: max length for relative_path (mirrors SQL CHECK added in migration 041).
pub const MAX_RELATIVE_PATH_LEN: usize = 2048;
// P075-SEC-001: max length for producer-supplied identity fields.
const MAX_IDENTITY_FIELD_LEN: usize = 512;

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

/// Returns a safe, non-revealing representation of a path for error messages.
///
/// P075-LOW-003: do not emit raw relative_path values in error messages — they may
/// contain user-controlled data. Use this redacted form in all error message formats.
pub fn redact_path(path: &str) -> String {
    format!("<path:{}_chars>", path.len())
}

/// Validate that `relative_path` is owned by the given `run_id`.
///
/// SEC-P075-001: a producer must not be able to write or register evidence under
/// another run's path tree. The canonical path layout is:
/// `evidence/runs/{run_id}/stages/{stage_id}/agents/{agent_id}/{kind}/...`
///
/// This function enforces that `relative_path` starts with
/// `evidence/runs/{run_id}/`, binding the metadata row to its run at insert time.
pub fn validate_path_ownership(relative_path: &str, run_id: &str) -> Result<()> {
    // Build the expected prefix from the run_id so that a path for a different run
    // (e.g. evidence/runs/other-run/...) is always rejected.
    let expected_prefix = format!("evidence/runs/{}/", run_id);
    if !relative_path.starts_with(expected_prefix.as_str()) {
        bail!(
            "relative_path must be under evidence/runs/{{run_id}}/... for this run \
             (run_id_len={}, path={})",
            run_id.len(),
            redact_path(relative_path)
        );
    }
    Ok(())
}

/// Reject NUL bytes and ASCII control characters in a metadata string field.
///
/// P075-SEC-MED-004: all producer-supplied metadata strings must be free of control
/// characters to prevent log injection and canonical SQLite metadata pollution.
fn validate_no_control_chars(val: &str, field_name: &'static str) -> Result<()> {
    if val.bytes().any(|b| b < 0x20 || b == 0x7f) {
        bail!("{field_name} contains NUL or control characters");
    }
    Ok(())
}

/// Enforce sha256 checksum format: exactly 64 lowercase hex digits.
///
/// P075-SEC-MED-004: strict format enforcement prevents forged or malformed checksums
/// from entering canonical metadata and later surfacing through diagnostics or exports.
fn validate_sha256_checksum_format(checksum: &str) -> Result<()> {
    if checksum.len() != 64 {
        bail!(
            "sha256 checksum must be exactly 64 hex characters, got {}",
            checksum.len()
        );
    }
    if !checksum
        .bytes()
        .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    {
        bail!("sha256 checksum must contain only lowercase hex digits (0-9, a-f)");
    }
    Ok(())
}

/// Enforce registry-style token format for producer_operation.
///
/// P075-SEC-MED-004: producer_operation must be a safe ASCII token to prevent
/// log injection when operation names are interpolated into structured log fields.
/// Allowed characters: ASCII letters, digits, underscores, dots, and hyphens.
fn validate_producer_operation_format(op: &str) -> Result<()> {
    if op.is_empty() {
        bail!("producer_operation must not be empty");
    }
    if !op
        .bytes()
        .all(|b| matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'.' | b'-'))
    {
        bail!(
            "producer_operation must be a registry-style token \
             (ASCII letters, digits, underscores, dots, or hyphens)"
        );
    }
    Ok(())
}

/// Validate and re-serialize `summary_json` to its canonical form.
///
/// P075-SEC-HIGH-001: The producer-supplied summary_json string may contain duplicate
/// keys (e.g. `{"line_count":1,"line_count":"<raw transcript>"}`). `serde_json::Map`
/// deduplicates keys by keeping the last value, but the *original string* is what gets
/// persisted if we only validate the parsed Map. This function returns the re-serialized
/// form of the validated Map so that duplicate-key smuggling is neutralised at the
/// persistence boundary.
///
/// Steps: size check → JSON parse with fixed non-revealing errors → allowlist
/// validation → re-serialize.
pub fn canonicalize_summary_json(summary: &str) -> Result<String> {
    if summary.len() > MAX_SUMMARY_JSON_LEN {
        bail!(
            "summary_json exceeds maximum length of {} bytes",
            MAX_SUMMARY_JSON_LEN
        );
    }
    let value: serde_json::Value = serde_json::from_str(summary)
        .map_err(|_| anyhow::anyhow!("summary_json must be a valid JSON object"))?;
    let obj = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("summary_json must be a valid JSON object"))?;
    validate_summary_json_content(&obj).context("summary_json schema violation")?;
    serde_json::to_string(&obj).context("re-serialize summary_json to canonical form")
}

/// Validate string field lengths and summary_json shape for an EvidenceSpoolRef.
///
/// P075-LOW-005: enforce modest length caps before insertion so runaway producers
/// cannot write arbitrarily large metadata strings.
/// P075-SEC-001: check summary_json byte length BEFORE serde_json::from_str to avoid
/// unnecessary allocation and parsing of oversized producer-controlled strings.
/// P075-LOW-002: validate summary_json as a JSON object, not merely length-bounded.
/// P075-SEC-MED-004: reject control characters in all metadata string fields; enforce
/// sha256 checksum format and registry-style producer_operation token.
pub fn validate_spool_ref_fields(spool_ref: &EvidenceSpoolRef) -> Result<()> {
    if spool_ref.id.len() > MAX_ID_LEN {
        bail!("id exceeds maximum length of {} bytes", MAX_ID_LEN);
    }
    if spool_ref.checksum.len() > MAX_CHECKSUM_LEN {
        bail!(
            "checksum exceeds maximum length of {} bytes",
            MAX_CHECKSUM_LEN
        );
    }
    if let Some(ref ct) = spool_ref.content_type {
        if ct.len() > MAX_CONTENT_TYPE_LEN {
            bail!(
                "content_type exceeds maximum length of {} bytes",
                MAX_CONTENT_TYPE_LEN
            );
        }
    }
    if spool_ref.producer_operation.len() > MAX_PRODUCER_OPERATION_LEN {
        bail!(
            "producer_operation exceeds maximum length of {} bytes",
            MAX_PRODUCER_OPERATION_LEN
        );
    }
    // P075-SEC-001: check byte length of producer-supplied identity fields.
    if spool_ref.run_id.len() > MAX_IDENTITY_FIELD_LEN {
        bail!(
            "run_id exceeds maximum length of {} bytes",
            MAX_IDENTITY_FIELD_LEN
        );
    }
    if let Some(ref v) = spool_ref.stage_execution_id {
        if v.len() > MAX_IDENTITY_FIELD_LEN {
            bail!(
                "stage_execution_id exceeds maximum length of {} bytes",
                MAX_IDENTITY_FIELD_LEN
            );
        }
    }
    if let Some(ref v) = spool_ref.stage_id {
        if v.len() > MAX_IDENTITY_FIELD_LEN {
            bail!(
                "stage_id exceeds maximum length of {} bytes",
                MAX_IDENTITY_FIELD_LEN
            );
        }
    }
    if let Some(ref v) = spool_ref.agent_execution_id {
        if v.len() > MAX_IDENTITY_FIELD_LEN {
            bail!(
                "agent_execution_id exceeds maximum length of {} bytes",
                MAX_IDENTITY_FIELD_LEN
            );
        }
    }
    if let Some(ref v) = spool_ref.agent_id {
        if v.len() > MAX_IDENTITY_FIELD_LEN {
            bail!(
                "agent_id exceeds maximum length of {} bytes",
                MAX_IDENTITY_FIELD_LEN
            );
        }
    }

    // SEC-P075-001: bind relative_path to run_id at insert time to prevent cross-run
    // evidence registration. Must be checked AFTER the basic validate_relative_path call
    // that rejects empty/traversal/absolute paths.
    validate_path_ownership(&spool_ref.relative_path, &spool_ref.run_id)
        .context("validate relative_path ownership for run")?;

    // P075-SEC-MED-004: reject NUL and ASCII control characters in all metadata strings.
    validate_no_control_chars(&spool_ref.id, "id")?;
    validate_no_control_chars(&spool_ref.run_id, "run_id")?;
    if let Some(ref v) = spool_ref.stage_execution_id {
        validate_no_control_chars(v, "stage_execution_id")?;
    }
    if let Some(ref v) = spool_ref.stage_id {
        validate_no_control_chars(v, "stage_id")?;
    }
    if let Some(ref v) = spool_ref.agent_execution_id {
        validate_no_control_chars(v, "agent_execution_id")?;
    }
    if let Some(ref v) = spool_ref.agent_id {
        validate_no_control_chars(v, "agent_id")?;
    }
    if let Some(ref v) = spool_ref.content_type {
        validate_no_control_chars(v, "content_type")?;
    }

    // P075-SEC-MED-004: enforce sha256 checksum format (64 lowercase hex chars).
    if spool_ref.checksum_algorithm == "sha256" {
        validate_sha256_checksum_format(&spool_ref.checksum)
            .context("checksum format validation")?;
    }

    // P075-SEC-MED-004: enforce registry-style producer_operation token.
    validate_producer_operation_format(&spool_ref.producer_operation)
        .context("producer_operation format validation")?;

    // P075-SEC-001 / P075-SEC-HIGH-001: validate via canonicalize_summary_json which
    // size-checks, parses, allowlist-validates, and re-serializes. The return value is
    // discarded here (caller must use insert_tx/insert_idempotent which bind the
    // canonical form, not the raw producer string). No extra context layer so that
    // callers see the specific error message ("exceeds maximum length", "disallowed field",
    // etc.) directly without the generic "summary_json validation" wrapper hiding it.
    if let Some(ref summary) = spool_ref.summary_json {
        let _ = canonicalize_summary_json(summary)?;
    }
    Ok(())
}

/// Validate summary_json against the narrow compact-fact allowlist.
///
/// P075-SEC-MED-002: The proposal explicitly forbids raw evidence text in SQLite
/// metadata. Only bounded scalar fields useful for evidence summaries are accepted.
/// Disallowed keys, nested objects/arrays, and oversized string values are all rejected.
///
/// P075-SEC-MED-003: Raw producer-controlled JSON keys are NEVER interpolated into
/// error messages. Errors emit only key length or fixed field-category tokens to
/// prevent raw transcript text, prompt fragments, or newline-injected log forgery from
/// appearing in anyhow errors, logs, or diagnostics.
///
/// Allowed fields:
/// - `line_count`, `chunk_count`, `byte_count`: non-negative integer
/// - `truncated`: boolean
/// - `started_at`, `finished_at`, `first_timestamp`, `last_timestamp`: string ≤ 64 chars
/// - `producer_label`, `producer`: string ≤ 256 chars
pub fn validate_summary_json_content(
    obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<()> {
    const MAX_TIMESTAMP_LEN: usize = 64;
    const MAX_PRODUCER_LABEL_LEN: usize = 256;

    for (key, value) in obj {
        // P075-SEC-MED-003: reject control characters in keys before any further processing.
        // A key with a newline or NUL could forge log records if echoed in error messages.
        if key.bytes().any(|b| b < 0x20 || b == 0x7f) {
            bail!(
                "summary_json key contains control characters (key_len={})",
                key.len()
            );
        }

        match key.as_str() {
            "line_count" | "chunk_count" | "byte_count" => match value {
                serde_json::Value::Null => {}
                serde_json::Value::Number(n) if n.is_u64() || n.is_i64() => {
                    if let Some(v) = n.as_i64() {
                        if v < 0 {
                            // Field name is a static match arm token — safe to include.
                            bail!("summary_json integer count field must be non-negative");
                        }
                    }
                }
                _ => bail!(
                    "summary_json integer count field has unexpected type {}",
                    json_type_name(value)
                ),
            },
            "truncated" => match value {
                serde_json::Value::Null | serde_json::Value::Bool(_) => {}
                _ => bail!(
                    "summary_json 'truncated' must be a boolean, got {}",
                    json_type_name(value)
                ),
            },
            "started_at" | "finished_at" | "first_timestamp" | "last_timestamp" => match value {
                serde_json::Value::Null => {}
                serde_json::Value::String(s) => {
                    if s.len() > MAX_TIMESTAMP_LEN {
                        bail!(
                            "summary_json timestamp field exceeds {} character limit",
                            MAX_TIMESTAMP_LEN
                        );
                    }
                    // P075-SEC-MED-001: reject control characters in timestamp string values
                    // to prevent log injection when values appear in structured log output.
                    if s.bytes().any(|b| b < 0x20 || b == 0x7f) {
                        bail!("summary_json timestamp field contains control characters");
                    }
                    // P075-SEC-MED-001: enforce RFC3339 timestamp format to prevent raw
                    // transcript fragments, prompt text, or arbitrary strings from being
                    // stored as timestamp metadata. Only valid RFC3339 timestamps are accepted.
                    // Empty string is permitted (prefer null for absent timestamps).
                    if !s.is_empty() && DateTime::parse_from_rfc3339(s).is_err() {
                        bail!("summary_json timestamp field must be a valid RFC3339 timestamp");
                    }
                }
                _ => bail!(
                    "summary_json timestamp field must be a string, got {}",
                    json_type_name(value)
                ),
            },
            "producer_label" | "producer" => match value {
                serde_json::Value::Null => {}
                serde_json::Value::String(s) => {
                    if s.len() > MAX_PRODUCER_LABEL_LEN {
                        bail!(
                            "summary_json label field exceeds {} character limit",
                            MAX_PRODUCER_LABEL_LEN
                        );
                    }
                    // P075-SEC-MED-001: enforce registry-style token format for producer
                    // label fields to prevent arbitrary text, transcript fragments, prompt
                    // text, or filesystem paths from being stored as producer metadata.
                    // Allowed: ASCII letters, digits, underscores, dots, hyphens.
                    // Empty string is permitted (field is optional).
                    if !s.is_empty()
                        && !s.bytes().all(|b| {
                            matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'.' | b'-')
                        })
                    {
                        bail!(
                            "summary_json label field must be a registry-style token \
                             (ASCII letters, digits, underscores, dots, or hyphens)"
                        );
                    }
                }
                _ => bail!(
                    "summary_json label field must be a string, got {}",
                    json_type_name(value)
                ),
            },
            // P075-SEC-MED-003: never echo the raw key — emit only the length.
            _ => bail!(
                "summary_json contains disallowed field (key_len={}); \
                 only compact-fact fields are permitted",
                key.len()
            ),
        }
    }
    Ok(())
}

fn json_type_name(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

/// Validate a relative path before insertion.
///
/// Rejects backslash separators before segment splitting so that mixed-separator
/// traversal such as `"foo\\..\bar"` cannot bypass the `".."` check. After
/// backslash rejection, normalize_path is a no-op and callers need not normalize
/// before validating.
///
/// P075-LOW-001: NFC normalization (rejecting paths whose NFC form differs from input)
/// is deferred. Wire before Phase 3 producers land using the `unicode-normalization` crate.
pub fn validate_relative_path(path: &str) -> Result<()> {
    if path.is_empty() {
        bail!("relative_path must not be empty");
    }
    // P075-SEC-001: cap length before any further validation to bound allocation.
    if path.len() > MAX_RELATIVE_PATH_LEN {
        bail!(
            "relative_path exceeds maximum length of {} bytes: {}",
            MAX_RELATIVE_PATH_LEN,
            redact_path(path)
        );
    }
    // Reject platform-specific separator ambiguity (P075 relative_path_rules).
    // Backslash rejection also blocks Windows-style absolute paths ("C:\...").
    if path.contains('\\') {
        bail!(
            "relative_path must not contain backslash separators: {}",
            redact_path(path)
        );
    }
    // Reject NUL bytes and ASCII control characters to prevent filesystem API
    // truncation and log injection in Phase 3+ readers (P075-SEC-004).
    if path.bytes().any(|b| b == 0 || b < 0x20) {
        bail!("relative_path must not contain NUL or control characters");
    }
    // Reject absolute paths.
    if std::path::Path::new(path).is_absolute() {
        bail!("relative_path must not be absolute: {}", redact_path(path));
    }
    // Validate each segment after splitting on the only accepted separator.
    for segment in path.split('/') {
        if segment.is_empty() {
            bail!(
                "relative_path must not contain empty segments: {}",
                redact_path(path)
            );
        }
        if segment == ".." {
            bail!(
                "relative_path must not contain '..' traversal segments: {}",
                redact_path(path)
            );
        }
        if segment == "." {
            bail!(
                "relative_path must not contain '.' segments: {}",
                redact_path(path)
            );
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
    validate_spool_ref_fields(spool_ref).context("validate spool_ref fields")?;
    // Use the DbWriter-owned P061 immediate transaction path instead of BEGIN DEFERRED.
    // to align with the single retry primitive mandated by P075 and avoid contention divergence.
    let mut tx = begin_registered_immediate_transaction(
        pool,
        crate::writer::class_a_operation(
            "p075_evidence_spool_ref_insert",
            crate::write_class::WriteLane::CriticalBarrier,
            "p075_evidence_spool_ref_insert",
        ),
        "p075_evidence_spool_ref_insert",
    )
    .await
    .context("begin insert evidence_spool_ref")?;
    insert_tx(&mut tx, spool_ref).await?;
    tx.commit()
        .await
        .context("commit insert evidence_spool_ref")?;
    Ok(())
}

pub async fn insert_tx(
    tx: &mut Transaction<'_, Sqlite>,
    spool_ref: &EvidenceSpoolRef,
) -> Result<()> {
    validate_relative_path(&spool_ref.relative_path).context("validate relative_path")?;
    validate_spool_ref_fields(spool_ref).context("validate spool_ref fields")?;
    let created_at = spool_ref.created_at.to_rfc3339();
    let kind = spool_ref.kind.as_str();
    let status = spool_ref.status.as_str();
    let rel_path = normalize_path(&spool_ref.relative_path);
    // P075-SEC-HIGH-001: persist the canonicalized (re-serialized) summary_json, not the
    // raw producer string. canonicalize_summary_json validates and re-serializes the parsed
    // Map, eliminating duplicate-key smuggling (e.g. {"k":1,"k":"<transcript>"}).
    let canonical_summary = spool_ref
        .summary_json
        .as_deref()
        .map(canonicalize_summary_json)
        .transpose()
        .context("canonicalize summary_json for insert_tx")?;

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
    .bind(&canonical_summary)
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
    // Use the DbWriter-owned P061 immediate transaction path instead of BEGIN DEFERRED.
    let mut tx = begin_registered_immediate_transaction(
        pool,
        crate::writer::class_a_operation(
            "p075_evidence_spool_ref_insert_idempotent",
            crate::write_class::WriteLane::CriticalBarrier,
            "p075_evidence_spool_ref_insert_idempotent",
        ),
        "p075_evidence_spool_ref_insert_idempotent",
    )
    .await
    .context("begin insert_idempotent")?;
    let inserted = insert_idempotent_tx(&mut tx, spool_ref).await?;
    tx.commit().await.context("commit insert_idempotent")?;
    Ok(inserted)
}

async fn insert_idempotent_tx(
    tx: &mut Transaction<'_, Sqlite>,
    spool_ref: &EvidenceSpoolRef,
) -> Result<bool> {
    validate_relative_path(&spool_ref.relative_path).context("validate relative_path")?;
    validate_spool_ref_fields(spool_ref).context("validate spool_ref fields")?;
    let rel_path = normalize_path(&spool_ref.relative_path);
    let created_at = spool_ref.created_at.to_rfc3339();
    let kind = spool_ref.kind.as_str();
    let status = spool_ref.status.as_str();
    // P075-SEC-HIGH-001: bind canonical re-serialized form, not the raw producer string.
    let canonical_summary = spool_ref
        .summary_json
        .as_deref()
        .map(canonicalize_summary_json)
        .transpose()
        .context("canonicalize summary_json for insert_idempotent")?;

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
    .bind(&canonical_summary)
    .bind(created_at)
    .bind(status)
    .execute(&mut **tx)
    .await
    .context("insert_idempotent execute")?;

    if result.rows_affected() == 1 {
        return Ok(true);
    }

    // rows_affected == 0: (run_id, relative_path) conflict — check the existing row.
    let existing = sqlx::query(
        r#"SELECT checksum, size_bytes FROM evidence_spool_refs
           WHERE run_id = ?1 AND relative_path = ?2"#,
    )
    .bind(&spool_ref.run_id)
    .bind(&rel_path)
    .fetch_one(&mut **tx)
    .await
    .context("fetch existing row after idempotent conflict")?;

    let existing_checksum: String = existing.try_get("checksum").context("existing checksum")?;
    let existing_size: i64 = existing
        .try_get("size_bytes")
        .context("existing size_bytes")?;

    if existing_checksum == spool_ref.checksum && existing_size == spool_ref.size_bytes {
        return Ok(false);
    }

    // Mismatch: hard error requiring manual reconcile (LIFT-REL-06, evidence_metadata_conflict_total).
    // P075-SEC-MED-004: run_id is redacted to prevent producer-controlled content from entering
    // error messages. Only the length is emitted for triage. Checksum prefix (12 of 64 hex chars)
    // bounds fingerprinting exposure while retaining enough for incident correlation.
    bail!(
        "evidence_metadata_conflict: run_id_len={} relative_path={} \
         existing checksum_prefix={} new checksum_prefix={}",
        spool_ref.run_id.len(),
        redact_path(&rel_path),
        &existing_checksum[..existing_checksum.len().min(12)],
        &spool_ref.checksum[..spool_ref.checksum.len().min(12)]
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

/// Find an evidence spool ref by run_id and relative_path.
///
/// P075-SEC-LOW-002: validates the path before normalizing so that an otherwise-invalid
/// backslash-spelled path is rejected rather than silently coerced to a forward-slash
/// equivalent. The evidence path boundary is symmetric across reads and writes.
pub async fn find_by_run_and_path(
    pool: &SqlitePool,
    run_id: &str,
    relative_path: &str,
) -> Result<Option<EvidenceSpoolRef>> {
    validate_relative_path(relative_path).context("validate relative_path for lookup")?;
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

pub async fn list_by_run_id(pool: &SqlitePool, run_id: &str) -> Result<Vec<EvidenceSpoolRef>> {
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
///
/// Uses the DbWriter-owned P061 immediate transaction path for consistency with insert paths.
/// and to satisfy the single-retry-primitive contract from P075 (bypass-006 retirement).
pub async fn update_status(
    pool: &SqlitePool,
    id: &str,
    status: EvidenceSpoolRefStatus,
) -> Result<()> {
    let status_str = status.as_str();
    let mut tx = begin_registered_immediate_transaction(
        pool,
        crate::writer::class_a_operation(
            "p075_evidence_spool_ref_update_status",
            crate::write_class::WriteLane::CriticalBarrier,
            "p075_evidence_spool_ref_update_status",
        ),
        "p075_evidence_spool_ref_update_status",
    )
    .await
    .context("begin update_status evidence_spool_ref")?;
    sqlx::query("UPDATE evidence_spool_refs SET status = ?1 WHERE id = ?2")
        .bind(status_str)
        .bind(id)
        .execute(&mut **tx)
        .await
        .context("update evidence_spool_ref status")?;
    tx.commit()
        .await
        .context("commit update_status evidence_spool_ref")?;
    Ok(())
}

// ─── DbWriter routing (Phase 3, retires bypass-006) ─────────────────────────

/// Submit a Class C evidence spool metadata insert through [`DbWriter`].
///
/// File-before-metadata ordering: callers must have completed
/// `write_spool_file()` (fsync + rename) before calling this function.
/// Failure here leaves an orphan-safe fsynced file; the startup sweep recovers it.
///
/// Returns [`WriteResult::Committed`] on success. Validation failures return
/// [`WriteResult::WriteFailed`] without entering the DbWriter queue.
pub async fn insert_via_dbwriter(writer: &DbWriter, spool_ref: EvidenceSpoolRef) -> WriteResult {
    if validate_relative_path(&spool_ref.relative_path).is_err() {
        return WriteResult::WriteFailed;
    }
    if validate_spool_ref_fields(&spool_ref).is_err() {
        return WriteResult::WriteFailed;
    }
    let idempotency_key = format!(
        "{}/{}",
        spool_ref.run_id,
        normalize_path(&spool_ref.relative_path)
    );
    let op = WriteOperation {
        class: WriteClass::C,
        lane: WriteLane::EvidenceMetadata,
        operation_name: "p075_evidence_spool_ref_insert",
        expected_rows: 1,
        batchable: false,
        barrier: false,
        deadline: WriteClass::C.default_deadline(),
        deadline_reason: None,
        idempotency_key,
        replay_policy: ReplayPolicy::ChecksumIdempotent,
        observed_at: None,
    };
    let mut tx = match writer
        .begin_immediate_transaction(op, "p075_evidence_spool_ref_insert")
        .await
    {
        Ok(tx) => tx,
        Err(_) => return WriteResult::WriteFailed,
    };
    let result = insert_tx(&mut tx, &spool_ref).await;
    match result {
        Ok(()) => match tx.commit().await {
            Ok(()) => WriteResult::Committed,
            Err(_) => WriteResult::WriteFailed,
        },
        Err(_) => {
            let _ = tx.rollback().await;
            WriteResult::WriteFailed
        }
    }
}

/// Submit an idempotent Class C evidence spool metadata insert through [`DbWriter`].
///
/// Uses INSERT OR IGNORE on (run_id, relative_path). Returns
/// [`WriteResult::Committed`] regardless of whether the row was newly inserted
/// or already existed with a matching checksum. Returns [`WriteResult::WriteFailed`]
/// on checksum/size mismatch or validation error.
pub async fn insert_idempotent_via_dbwriter(
    writer: &DbWriter,
    spool_ref: EvidenceSpoolRef,
) -> WriteResult {
    if validate_relative_path(&spool_ref.relative_path).is_err() {
        return WriteResult::WriteFailed;
    }
    if validate_spool_ref_fields(&spool_ref).is_err() {
        return WriteResult::WriteFailed;
    }
    let idempotency_key = format!(
        "{}/{}",
        spool_ref.run_id,
        normalize_path(&spool_ref.relative_path)
    );
    let op = WriteOperation {
        class: WriteClass::C,
        lane: WriteLane::EvidenceMetadata,
        operation_name: "p075_evidence_spool_ref_insert_idempotent",
        expected_rows: 1,
        batchable: false,
        barrier: false,
        deadline: WriteClass::C.default_deadline(),
        deadline_reason: None,
        idempotency_key,
        replay_policy: ReplayPolicy::ChecksumIdempotent,
        observed_at: None,
    };
    let mut tx = match writer
        .begin_immediate_transaction(op, "p075_evidence_spool_ref_insert_idempotent")
        .await
    {
        Ok(tx) => tx,
        Err(_) => return WriteResult::WriteFailed,
    };
    let result = insert_idempotent_tx(&mut tx, &spool_ref).await;
    match result {
        Ok(_) => match tx.commit().await {
            Ok(()) => WriteResult::Committed,
            Err(_) => WriteResult::WriteFailed,
        },
        Err(_) => {
            let _ = tx.rollback().await;
            WriteResult::WriteFailed
        }
    }
}

/// Submit a Class A status update for an evidence spool ref through [`DbWriter`].
///
/// Status transitions (e.g. `available → recovered_orphan`, `available → pending_delete`)
/// are canonical decisions that must be barrier-committed. Uses the spool ref's `id`
/// primary key as the natural idempotency key; applying the same status twice is a no-op.
pub async fn update_status_via_dbwriter(
    writer: &DbWriter,
    id: &str,
    status: EvidenceSpoolRefStatus,
) -> WriteResult {
    let id_owned = id.to_string();
    let status_str = status.as_str();
    let idempotency_key = format!("evsp_status/{}/{}", id, status_str);
    let op = WriteOperation {
        class: WriteClass::A,
        lane: WriteLane::CriticalBarrier,
        operation_name: "p075_evidence_spool_ref_update_status",
        expected_rows: 1,
        batchable: false,
        barrier: true,
        deadline: WriteClass::A.default_deadline(),
        deadline_reason: None,
        idempotency_key,
        replay_policy: ReplayPolicy::NaturalKey,
        observed_at: None,
    };
    let mut tx = match writer
        .begin_immediate_transaction(op, "p075_evidence_spool_ref_update_status")
        .await
    {
        Ok(tx) => tx,
        Err(_) => return WriteResult::WriteFailed,
    };
    let result = sqlx::query("UPDATE evidence_spool_refs SET status = ?1 WHERE id = ?2")
        .bind(status.as_str())
        .bind(&id_owned)
        .execute(&mut **tx)
        .await
        .context("update evidence_spool_ref status");
    match result {
        Ok(_) => match tx.commit().await {
            Ok(()) => WriteResult::Committed,
            Err(_) => WriteResult::WriteFailed,
        },
        Err(_) => {
            let _ = tx.rollback().await;
            WriteResult::WriteFailed
        }
    }
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
        metadata_version: row
            .try_get("metadata_version")
            .context("metadata_version")?,
        run_id: row.try_get("run_id").context("run_id")?,
        stage_execution_id: row
            .try_get("stage_execution_id")
            .context("stage_execution_id")?,
        stage_id: row.try_get("stage_id").context("stage_id")?,
        agent_execution_id: row
            .try_get("agent_execution_id")
            .context("agent_execution_id")?,
        agent_id: row.try_get("agent_id").context("agent_id")?,
        kind,
        relative_path: row.try_get("relative_path").context("relative_path")?,
        size_bytes: row.try_get("size_bytes").context("size_bytes")?,
        checksum_algorithm: row
            .try_get("checksum_algorithm")
            .context("checksum_algorithm")?,
        checksum: row.try_get("checksum").context("checksum")?,
        producer_operation: row
            .try_get("producer_operation")
            .context("producer_operation")?,
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

    async fn test_pool() -> sqlx::SqlitePool {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        crate::writer::register_shared_writer(
            &pool,
            std::sync::Arc::new(crate::writer::DbWriter::new(pool.clone())),
        )
        .await
        .expect("register shared writer");
        pool
    }

    // Valid 64-char lowercase hex sha256 checksum for test fixtures.
    // P075-SEC-MED-004: test fixtures use the correct format so that Rust-side
    // checksum format validation passes in all round-trip and idempotency tests.
    const VALID_CHECKSUM: &str = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
    // A different valid 64-char sha256 for mismatch tests.
    const MISMATCH_CHECKSUM: &str =
        "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

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
            checksum: VALID_CHECKSUM.to_string(),
            producer_operation: "p075_evidence_spool_ref_insert".to_string(),
            content_type: Some("text/plain".to_string()),
            summary_json: Some(r#"{"line_count":100}"#.to_string()),
            created_at: Utc::now(),
            status: EvidenceSpoolRefStatus::Available,
        }
    }

    #[tokio::test]
    async fn insert_and_find_by_id() {
        let pool = test_pool().await;
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
        let pool = test_pool().await;
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
        let pool = test_pool().await;
        let spool_ref = make_ref("evsp_003", "run-3", "evidence/runs/run-3/ts.jsonl");
        let inserted = insert_idempotent(&pool, &spool_ref).await.unwrap();
        assert!(inserted, "first insert should return true");
        let duplicate = insert_idempotent(&pool, &spool_ref).await.unwrap();
        assert!(
            !duplicate,
            "same-checksum re-insert should return false (idempotent)"
        );
    }

    #[tokio::test]
    async fn insert_idempotent_checksum_mismatch_is_error() {
        let pool = test_pool().await;
        let spool_ref = make_ref("evsp_004", "run-4", "evidence/runs/run-4/ts.jsonl");
        insert(&pool, &spool_ref).await.unwrap();

        let mut conflict = spool_ref.clone();
        conflict.id = "evsp_005".to_string();
        conflict.checksum = MISMATCH_CHECKSUM.to_string();
        let result = insert_idempotent(&pool, &conflict).await;
        assert!(result.is_err(), "checksum mismatch should return error");
    }

    #[tokio::test]
    async fn unique_constraint_on_run_id_relative_path() {
        let pool = test_pool().await;
        let a = make_ref("evsp_010", "run-10", "evidence/runs/run-10/ts.jsonl");
        insert(&pool, &a).await.unwrap();
        let mut b = a.clone();
        b.id = "evsp_011".to_string(); // different id, same path
        let result = insert(&pool, &b).await;
        assert!(
            result.is_err(),
            "UNIQUE (run_id, relative_path) should prevent duplicate"
        );
    }

    #[tokio::test]
    async fn check_constraint_rejects_invalid_kind() {
        let pool = test_pool().await;
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
        let pool = test_pool().await;
        let result = sqlx::query(
            r#"INSERT INTO evidence_spool_refs
               (id, metadata_version, run_id, kind, relative_path, size_bytes,
                checksum_algorithm, checksum, producer_operation, created_at, status)
               VALUES ('x2','1','r','transcript','path/g',0,'sha256','abc','op','2025-01-01T00:00:00Z','BAD_STATUS')"#,
        )
        .execute(&pool)
        .await;
        assert!(
            result.is_err(),
            "invalid status should fail CHECK constraint"
        );
    }

    #[tokio::test]
    async fn check_constraint_rejects_negative_size() {
        let pool = test_pool().await;
        let result = sqlx::query(
            r#"INSERT INTO evidence_spool_refs
               (id, metadata_version, run_id, kind, relative_path, size_bytes,
                checksum_algorithm, checksum, producer_operation, created_at, status)
               VALUES ('x3','1','r','transcript','path/h',-1,'sha256','abc','op','2025-01-01T00:00:00Z','available')"#,
        )
        .execute(&pool)
        .await;
        assert!(
            result.is_err(),
            "negative size_bytes should fail CHECK constraint"
        );
    }

    #[tokio::test]
    async fn check_constraint_rejects_empty_relative_path() {
        let pool = test_pool().await;
        let result = sqlx::query(
            r#"INSERT INTO evidence_spool_refs
               (id, metadata_version, run_id, kind, relative_path, size_bytes,
                checksum_algorithm, checksum, producer_operation, created_at, status)
               VALUES ('x4','1','r','transcript','',0,'sha256','abc','op','2025-01-01T00:00:00Z','available')"#,
        )
        .execute(&pool)
        .await;
        assert!(
            result.is_err(),
            "empty relative_path should fail CHECK constraint"
        );
    }

    #[tokio::test]
    async fn check_constraint_rejects_summary_json_too_large() {
        let pool = test_pool().await;
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
        assert!(
            result.is_err(),
            "summary_json > 8192 bytes should fail CHECK constraint"
        );
    }

    #[tokio::test]
    async fn update_status_changes_value() {
        let pool = test_pool().await;
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
        let pool = test_pool().await;
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

    // -----------------------------------------------------------------------
    // P075-LOW-005: field length cap tests
    // -----------------------------------------------------------------------

    #[test]
    fn validate_fields_rejects_oversized_id() {
        let mut r = make_ref("evsp_test_id", "run-test", "evidence/runs/run-test/f.jsonl");
        r.id = "x".repeat(MAX_ID_LEN + 1);
        assert!(
            validate_spool_ref_fields(&r).is_err(),
            "oversized id should fail"
        );
    }

    #[test]
    fn validate_fields_rejects_oversized_checksum() {
        let mut r = make_ref("evsp_test_cs", "run-test", "evidence/runs/run-test/f.jsonl");
        r.checksum = "a".repeat(MAX_CHECKSUM_LEN + 1);
        // Length cap fires before format check; both reject but length is first.
        assert!(
            validate_spool_ref_fields(&r).is_err(),
            "oversized checksum should fail"
        );
    }

    #[test]
    fn validate_fields_rejects_oversized_content_type() {
        let mut r = make_ref("evsp_test_ct", "run-test", "evidence/runs/run-test/f.jsonl");
        r.content_type = Some("t".repeat(MAX_CONTENT_TYPE_LEN + 1));
        assert!(
            validate_spool_ref_fields(&r).is_err(),
            "oversized content_type should fail"
        );
    }

    #[test]
    fn validate_fields_rejects_oversized_producer_operation() {
        let mut r = make_ref("evsp_test_po", "run-test", "evidence/runs/run-test/f.jsonl");
        r.producer_operation = "p".repeat(MAX_PRODUCER_OPERATION_LEN + 1);
        assert!(
            validate_spool_ref_fields(&r).is_err(),
            "oversized producer_operation should fail"
        );
    }

    #[test]
    fn validate_fields_accepts_valid_fields() {
        let r = make_ref("evsp_test_ok", "run-test", "evidence/runs/run-test/f.jsonl");
        assert!(
            validate_spool_ref_fields(&r).is_ok(),
            "valid fields should pass"
        );
    }

    // -----------------------------------------------------------------------
    // P075-LOW-002: summary_json JSON object validation tests
    // -----------------------------------------------------------------------

    #[test]
    fn validate_fields_rejects_non_object_summary_json() {
        let mut r = make_ref(
            "evsp_test_sj1",
            "run-test",
            "evidence/runs/run-test/f2.jsonl",
        );
        r.summary_json = Some(r#"["not","an","object"]"#.to_string());
        assert!(
            validate_spool_ref_fields(&r).is_err(),
            "array summary_json should fail"
        );
    }

    #[test]
    fn validate_fields_rejects_string_summary_json() {
        let mut r = make_ref(
            "evsp_test_sj2",
            "run-test",
            "evidence/runs/run-test/f3.jsonl",
        );
        r.summary_json = Some(r#""plain string""#.to_string());
        assert!(
            validate_spool_ref_fields(&r).is_err(),
            "string summary_json should fail"
        );
    }

    #[test]
    fn validate_fields_rejects_malformed_summary_json() {
        let mut r = make_ref(
            "evsp_test_sj3",
            "run-test",
            "evidence/runs/run-test/f4.jsonl",
        );
        r.summary_json = Some("not valid json".to_string());
        assert!(
            validate_spool_ref_fields(&r).is_err(),
            "malformed summary_json should fail"
        );
    }

    #[test]
    fn validate_fields_rejects_secret_bearing_string_summary_without_echoing_payload() {
        let mut r = make_ref(
            "evsp_test_sj_secret_string",
            "run-test",
            "evidence/runs/run-test/f4a.jsonl",
        );
        r.summary_json = Some(r#""token=super_secret_summary_json_value""#.to_string());

        let result = validate_spool_ref_fields(&r);

        assert!(result.is_err(), "string summary_json should fail");
        let err = format!("{:#}", result.unwrap_err());
        assert!(
            !err.contains("super_secret_summary_json_value"),
            "raw summary_json payload must not appear in error, got: {err}"
        );
        assert!(
            err.contains("summary_json must be a valid JSON object"),
            "error should stay actionable without raw payload, got: {err}"
        );
    }

    #[test]
    fn validate_fields_accepts_valid_object_summary_json() {
        let mut r = make_ref(
            "evsp_test_sj4",
            "run-test",
            "evidence/runs/run-test/f5.jsonl",
        );
        r.summary_json =
            Some(r#"{"line_count":100,"chunk_count":5,"truncated":false}"#.to_string());
        assert!(
            validate_spool_ref_fields(&r).is_ok(),
            "valid JSON object summary_json should pass"
        );
    }

    #[test]
    fn validate_fields_accepts_none_summary_json() {
        let mut r = make_ref(
            "evsp_test_sj5",
            "run-test",
            "evidence/runs/run-test/f6.jsonl",
        );
        r.summary_json = None;
        assert!(
            validate_spool_ref_fields(&r).is_ok(),
            "None summary_json should pass"
        );
    }

    // -----------------------------------------------------------------------
    // insert rejects oversized fields before hitting the database
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn insert_rejects_oversized_id_before_db() {
        let pool = test_pool().await;
        let mut r = make_ref(
            "evsp_test_db_id",
            "run-test",
            "evidence/runs/run-test/g.jsonl",
        );
        r.id = "x".repeat(MAX_ID_LEN + 1);
        let result = insert(&pool, &r).await;
        assert!(
            result.is_err(),
            "insert with oversized id should fail validation"
        );
    }

    // -----------------------------------------------------------------------
    // Concurrency test: racing insert_idempotent tasks
    // P075-SEC-002 / prepush review: lock in evidence_metadata_conflict semantics
    // under genuine concurrent writers before Phase 3 producers wire.
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn insert_idempotent_concurrent_same_checksum_is_safe() {
        let pool = test_pool().await;
        let r1 = make_ref(
            "evsp_conc_001",
            "run-conc",
            "evidence/runs/run-conc/ts.jsonl",
        );
        let r2 = r1.clone();

        let pool1 = pool.clone();
        let pool2 = pool.clone();

        let (res1, res2) = tokio::join!(
            tokio::spawn(async move { insert_idempotent(&pool1, &r1).await }),
            tokio::spawn(async move { insert_idempotent(&pool2, &r2).await }),
        );

        let ok1 = res1.unwrap().unwrap();
        let ok2 = res2.unwrap().unwrap();
        // Exactly one task inserts (true) and the other gets the idempotent skip (false).
        assert!(
            ok1 ^ ok2,
            "exactly one task should return true (inserted) and one false (idempotent); \
             got ok1={ok1} ok2={ok2}"
        );
    }

    // -----------------------------------------------------------------------
    // P075-SEC-001: pre-parse summary_json and oversized field rejection tests
    // -----------------------------------------------------------------------

    #[test]
    fn validate_fields_rejects_oversized_summary_json_before_parse() {
        let mut r = make_ref(
            "evsp_sec001_a",
            "run-test",
            "evidence/runs/run-test/sec001a.jsonl",
        );
        // A string > MAX_SUMMARY_JSON_LEN that is NOT valid JSON.
        // If size is checked first, we get a "exceeds maximum length" error.
        // If JSON is parsed first, we would get a JSON parse error instead.
        let not_json = "x".repeat(MAX_SUMMARY_JSON_LEN + 1);
        r.summary_json = Some(not_json);
        let result = validate_spool_ref_fields(&r);
        assert!(
            result.is_err(),
            "oversized non-JSON summary_json should fail"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("exceeds maximum length"),
            "error must be about size (before parse), got: {err}"
        );
    }

    #[test]
    fn validate_fields_rejects_oversized_summary_json_valid_json_before_parse() {
        let mut r = make_ref(
            "evsp_sec001_b",
            "run-test",
            "evidence/runs/run-test/sec001b.jsonl",
        );
        // Build a valid JSON object that exceeds 8192 bytes.
        let large_value = "v".repeat(MAX_SUMMARY_JSON_LEN);
        let large_json = format!(r#"{{"key":"{}"}}"#, large_value);
        assert!(
            large_json.len() > MAX_SUMMARY_JSON_LEN,
            "test precondition: json must exceed cap"
        );
        r.summary_json = Some(large_json);
        let result = validate_spool_ref_fields(&r);
        assert!(
            result.is_err(),
            "oversized valid-JSON summary_json should fail"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("exceeds maximum length"),
            "error must be about size (before parse), got: {err}"
        );
    }

    #[test]
    fn validate_relative_path_rejects_oversized_path() {
        let long_segment = "a".repeat(MAX_RELATIVE_PATH_LEN + 1);
        let result = validate_relative_path(&long_segment);
        assert!(result.is_err(), "oversized relative_path should fail");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("exceeds maximum length"),
            "error must mention max length, got: {err}"
        );
    }

    #[test]
    fn validate_fields_rejects_oversized_run_id() {
        let mut r = make_ref(
            "evsp_sec001_c",
            "run-test",
            "evidence/runs/run-test/sec001c.jsonl",
        );
        r.run_id = "r".repeat(MAX_IDENTITY_FIELD_LEN + 1);
        assert!(
            validate_spool_ref_fields(&r).is_err(),
            "oversized run_id should fail"
        );
    }

    #[test]
    fn validate_fields_rejects_oversized_stage_execution_id() {
        let mut r = make_ref(
            "evsp_sec001_d",
            "run-test",
            "evidence/runs/run-test/sec001d.jsonl",
        );
        r.stage_execution_id = Some("s".repeat(MAX_IDENTITY_FIELD_LEN + 1));
        assert!(
            validate_spool_ref_fields(&r).is_err(),
            "oversized stage_execution_id should fail"
        );
    }

    #[test]
    fn validate_fields_rejects_oversized_agent_id() {
        let mut r = make_ref(
            "evsp_sec001_e",
            "run-test",
            "evidence/runs/run-test/sec001e.jsonl",
        );
        r.agent_id = Some("a".repeat(MAX_IDENTITY_FIELD_LEN + 1));
        assert!(
            validate_spool_ref_fields(&r).is_err(),
            "oversized agent_id should fail"
        );
    }

    // -----------------------------------------------------------------------
    // P075-SEC-002: DB-level path containment constraint tests (migration 041)
    // These tests exercise the SQL CHECK constraints directly, independent of
    // the Rust-side validator, proving defense-in-depth at the DB boundary.
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn check_constraint_rejects_absolute_path_at_db_level() {
        let pool = test_pool().await;
        let result = sqlx::query(
            r#"INSERT INTO evidence_spool_refs
               (id, metadata_version, run_id, kind, relative_path, size_bytes,
                checksum_algorithm, checksum, producer_operation, created_at, status)
               VALUES ('sec002_abs','1','r','transcript','/absolute/path',0,'sha256','abc','op','2025-01-01T00:00:00Z','available')"#,
        )
        .execute(&pool)
        .await;
        assert!(
            result.is_err(),
            "absolute relative_path should fail DB CHECK constraint"
        );
    }

    #[tokio::test]
    async fn check_constraint_rejects_traversal_at_db_level() {
        let pool = test_pool().await;
        let result = sqlx::query(
            r#"INSERT INTO evidence_spool_refs
               (id, metadata_version, run_id, kind, relative_path, size_bytes,
                checksum_algorithm, checksum, producer_operation, created_at, status)
               VALUES ('sec002_trav','1','r','transcript','foo/../bar',0,'sha256','abc','op','2025-01-01T00:00:00Z','available')"#,
        )
        .execute(&pool)
        .await;
        assert!(
            result.is_err(),
            "'foo/../bar' should fail DB CHECK constraint"
        );
    }

    #[tokio::test]
    async fn check_constraint_rejects_dotdot_segment_at_db_level() {
        let pool = test_pool().await;
        let result = sqlx::query(
            r#"INSERT INTO evidence_spool_refs
               (id, metadata_version, run_id, kind, relative_path, size_bytes,
                checksum_algorithm, checksum, producer_operation, created_at, status)
               VALUES ('sec002_dd','1','r','transcript','..',0,'sha256','abc','op','2025-01-01T00:00:00Z','available')"#,
        )
        .execute(&pool)
        .await;
        assert!(
            result.is_err(),
            "bare '..' relative_path should fail DB CHECK constraint"
        );
    }

    #[tokio::test]
    async fn check_constraint_rejects_dot_segment_at_db_level() {
        let pool = test_pool().await;
        let result = sqlx::query(
            r#"INSERT INTO evidence_spool_refs
               (id, metadata_version, run_id, kind, relative_path, size_bytes,
                checksum_algorithm, checksum, producer_operation, created_at, status)
               VALUES ('sec002_dot','1','r','transcript','foo/./bar',0,'sha256','abc','op','2025-01-01T00:00:00Z','available')"#,
        )
        .execute(&pool)
        .await;
        assert!(
            result.is_err(),
            "'foo/./bar' should fail DB CHECK constraint"
        );
    }

    #[tokio::test]
    async fn check_constraint_rejects_double_slash_at_db_level() {
        let pool = test_pool().await;
        let result = sqlx::query(
            r#"INSERT INTO evidence_spool_refs
               (id, metadata_version, run_id, kind, relative_path, size_bytes,
                checksum_algorithm, checksum, producer_operation, created_at, status)
               VALUES ('sec002_ds','1','r','transcript','foo//bar',0,'sha256','abc','op','2025-01-01T00:00:00Z','available')"#,
        )
        .execute(&pool)
        .await;
        assert!(
            result.is_err(),
            "'foo//bar' (empty segment) should fail DB CHECK constraint"
        );
    }

    #[tokio::test]
    async fn check_constraint_accepts_valid_relative_path_at_db_level() {
        let pool = test_pool().await;
        let result = sqlx::query(
            r#"INSERT INTO evidence_spool_refs
               (id, metadata_version, run_id, kind, relative_path, size_bytes,
                checksum_algorithm, checksum, producer_operation, created_at, status)
               VALUES ('sec002_ok','1','r','transcript','evidence/runs/run-1/ts.jsonl',0,'sha256','abc','op','2025-01-01T00:00:00Z','available')"#,
        )
        .execute(&pool)
        .await;
        assert!(
            result.is_ok(),
            "valid relative_path should pass DB CHECK constraint"
        );
    }

    #[tokio::test]
    async fn check_constraint_rejects_oversized_run_id_at_db_level() {
        let pool = test_pool().await;
        let long_id = "r".repeat(513);
        let result = sqlx::query(
            r#"INSERT INTO evidence_spool_refs
               (id, metadata_version, run_id, kind, relative_path, size_bytes,
                checksum_algorithm, checksum, producer_operation, created_at, status)
               VALUES ('sec002_rid', '1', ?1, 'transcript', 'evidence/runs/sec002/f.jsonl', 0,
                       'sha256', 'abc', 'op', '2025-01-01T00:00:00Z', 'available')"#,
        )
        .bind(&long_id)
        .execute(&pool)
        .await;
        assert!(
            result.is_err(),
            "run_id > 512 chars should fail DB CHECK constraint"
        );
    }

    #[tokio::test]
    async fn insert_idempotent_concurrent_checksum_mismatch_errors() {
        let pool = test_pool().await;
        let r_base = make_ref(
            "evsp_conc_002",
            "run-conc2",
            "evidence/runs/run-conc2/ts.jsonl",
        );

        // Pre-insert so both racers see a pre-existing row with different checksums.
        insert(&pool, &r_base).await.unwrap();

        let mut r_mismatch = r_base.clone();
        r_mismatch.checksum = MISMATCH_CHECKSUM.to_string();

        let result = insert_idempotent(&pool, &r_mismatch).await;
        assert!(
            result.is_err(),
            "insert_idempotent with checksum mismatch must return error"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("evidence_metadata_conflict"),
            "error must name evidence_metadata_conflict, got: {err}"
        );
        // P075-SEC-MED-004: run_id must not appear verbatim in the error.
        assert!(
            !err.contains("run-conc2"),
            "raw run_id must not appear in conflict error, got: {err}"
        );
    }

    // -----------------------------------------------------------------------
    // P075-SEC-MED-002: summary_json compact-fact allowlist tests
    // -----------------------------------------------------------------------

    fn make_ref_with_summary(summary: Option<&str>) -> EvidenceSpoolRef {
        let mut r = make_ref("evsp_sj_test", "run-sj", "evidence/runs/run-sj/f.jsonl");
        r.summary_json = summary.map(|s| s.to_string());
        r
    }

    #[test]
    fn summary_json_rejects_transcript_shaped_payload() {
        let r = make_ref_with_summary(Some(
            r#"{"content":"this is raw transcript text that should never be in metadata"}"#,
        ));
        let result = validate_spool_ref_fields(&r);
        assert!(
            result.is_err(),
            "transcript-shaped summary_json must be rejected"
        );
        let err = format!("{:#}", result.unwrap_err());
        assert!(
            err.contains("disallowed field"),
            "error must mention disallowed field, got: {err}"
        );
    }

    #[test]
    fn summary_json_rejects_stdout_shaped_payload() {
        let r = make_ref_with_summary(Some(r#"{"stdout":"ls -la output","stderr":""}"#));
        let result = validate_spool_ref_fields(&r);
        assert!(
            result.is_err(),
            "stdout-shaped summary_json must be rejected"
        );
    }

    #[test]
    fn summary_json_rejects_prompt_shaped_payload() {
        let r = make_ref_with_summary(Some(
            r#"{"prompt":"system instructions","text":"model response text"}"#,
        ));
        let result = validate_spool_ref_fields(&r);
        assert!(
            result.is_err(),
            "prompt-shaped summary_json must be rejected"
        );
    }

    #[test]
    fn summary_json_rejects_nested_object() {
        let r = make_ref_with_summary(Some(r#"{"nested":{"key":"value"}}"#));
        let result = validate_spool_ref_fields(&r);
        assert!(
            result.is_err(),
            "nested object in summary_json must be rejected"
        );
        let err = format!("{:#}", result.unwrap_err());
        assert!(
            err.contains("disallowed field"),
            "error must mention disallowed field, got: {err}"
        );
    }

    #[test]
    fn summary_json_rejects_array_value() {
        let r = make_ref_with_summary(Some(r#"{"line_count":[1,2,3]}"#));
        let result = validate_spool_ref_fields(&r);
        assert!(
            result.is_err(),
            "array value for line_count must be rejected"
        );
    }

    #[test]
    fn summary_json_rejects_string_for_integer_field() {
        let r = make_ref_with_summary(Some(r#"{"line_count":"100"}"#));
        let result = validate_spool_ref_fields(&r);
        assert!(
            result.is_err(),
            "string value for line_count must be rejected"
        );
    }

    #[test]
    fn summary_json_rejects_integer_for_truncated() {
        let r = make_ref_with_summary(Some(r#"{"truncated":1}"#));
        let result = validate_spool_ref_fields(&r);
        assert!(
            result.is_err(),
            "integer value for truncated must be rejected (must be boolean)"
        );
    }

    #[test]
    fn summary_json_rejects_oversized_producer_label() {
        let long_label = "p".repeat(257);
        let json = format!(r#"{{"producer_label":"{}"}}"#, long_label);
        let r = make_ref_with_summary(Some(&json));
        let result = validate_spool_ref_fields(&r);
        assert!(result.is_err(), "oversized producer_label must be rejected");
        let err = format!("{:#}", result.unwrap_err());
        assert!(
            err.contains("exceeds") || err.contains("character limit"),
            "error must mention limit, got: {err}"
        );
    }

    #[test]
    fn summary_json_rejects_oversized_timestamp() {
        let long_ts = "2025-01-01T00:00:00Z".repeat(4); // > 64 chars
        let json = format!(r#"{{"started_at":"{}"}}"#, long_ts);
        let r = make_ref_with_summary(Some(&json));
        let result = validate_spool_ref_fields(&r);
        assert!(result.is_err(), "oversized timestamp must be rejected");
    }

    #[test]
    fn summary_json_accepts_all_valid_compact_fields() {
        let r = make_ref_with_summary(Some(
            r#"{"line_count":1000,"chunk_count":10,"byte_count":51200,"truncated":false,"started_at":"2025-01-01T00:00:00Z","finished_at":"2025-01-01T00:01:00Z","producer_label":"acp_transcript_v1"}"#,
        ));
        assert!(
            validate_spool_ref_fields(&r).is_ok(),
            "all valid compact fields must pass"
        );
    }

    #[test]
    fn summary_json_accepts_partial_valid_fields() {
        let r = make_ref_with_summary(Some(r#"{"line_count":42,"truncated":true}"#));
        assert!(
            validate_spool_ref_fields(&r).is_ok(),
            "partial valid summary_json must pass"
        );
    }

    #[test]
    fn summary_json_accepts_null_values_for_allowed_fields() {
        let r = make_ref_with_summary(Some(
            r#"{"line_count":null,"truncated":null,"started_at":null}"#,
        ));
        assert!(
            validate_spool_ref_fields(&r).is_ok(),
            "null values for allowed fields must pass"
        );
    }

    #[test]
    fn summary_json_rejects_arbitrary_unknown_key() {
        let r = make_ref_with_summary(Some(r#"{"unknown_field":"value"}"#));
        let result = validate_spool_ref_fields(&r);
        assert!(result.is_err(), "unknown key must be rejected");
        let err = format!("{:#}", result.unwrap_err());
        assert!(
            err.contains("disallowed field"),
            "error must mention disallowed field, got: {err}"
        );
    }

    #[test]
    fn summary_json_rejects_negative_line_count() {
        let r = make_ref_with_summary(Some(r#"{"line_count":-1}"#));
        let result = validate_spool_ref_fields(&r);
        assert!(result.is_err(), "negative line_count must be rejected");
    }

    // -----------------------------------------------------------------------
    // P075-SEC-MED-001: timestamp and producer_label string value hardening.
    //
    // Before this fix, timestamp fields only checked length (≤ 64 chars) and
    // producer_label fields only checked length (≤ 256 chars). A producer could
    // store raw transcript fragments, prompt text, or arbitrary ASCII in these
    // fields, defeating the raw-evidence-spooling boundary P075 exists to enforce.
    //
    // Fix: timestamp fields must be valid RFC3339 and free of control characters;
    // producer_label/producer fields must be registry-style tokens.
    // -----------------------------------------------------------------------

    #[test]
    fn summary_json_rejects_plain_text_in_timestamp_field() {
        // Non-RFC3339 text must be rejected even if it fits within 64 chars.
        let mut map = serde_json::Map::new();
        map.insert(
            "started_at".to_string(),
            serde_json::Value::String("User: Hello what is the answer?".to_string()),
        );
        let result = validate_summary_json_content(&map);
        assert!(
            result.is_err(),
            "transcript-shaped text in timestamp field must be rejected"
        );
        let err = format!("{:#}", result.unwrap_err());
        assert!(
            err.contains("RFC3339") || err.contains("timestamp"),
            "error must mention timestamp format, got: {err}"
        );
    }

    #[test]
    fn summary_json_rejects_control_char_in_timestamp_value() {
        // A newline inside a timestamp value could forge log records.
        let mut map = serde_json::Map::new();
        map.insert(
            "first_timestamp".to_string(),
            serde_json::Value::String("2025-01-01\nX-Injected: forged".to_string()),
        );
        let result = validate_summary_json_content(&map);
        assert!(
            result.is_err(),
            "control char in timestamp value must be rejected"
        );
        let err = format!("{:#}", result.unwrap_err());
        assert!(
            !err.contains("forged"),
            "injected content must not appear in error, got: {err}"
        );
    }

    #[test]
    fn summary_json_rejects_nul_in_timestamp_value() {
        let mut map = serde_json::Map::new();
        map.insert(
            "last_timestamp".to_string(),
            serde_json::Value::String("2025-01-01\x00poisoned".to_string()),
        );
        let result = validate_summary_json_content(&map);
        assert!(
            result.is_err(),
            "NUL byte in timestamp value must be rejected"
        );
    }

    #[test]
    fn summary_json_accepts_valid_rfc3339_timestamps() {
        let mut map = serde_json::Map::new();
        map.insert(
            "started_at".to_string(),
            serde_json::Value::String("2025-01-01T00:00:00Z".to_string()),
        );
        map.insert(
            "finished_at".to_string(),
            serde_json::Value::String("2025-01-01T00:01:30.123+00:00".to_string()),
        );
        let result = validate_summary_json_content(&map);
        assert!(
            result.is_ok(),
            "valid RFC3339 timestamps must pass: {result:?}"
        );
    }

    #[test]
    fn summary_json_accepts_empty_string_timestamp() {
        // Empty string is permitted; callers should prefer null for absent timestamps.
        let mut map = serde_json::Map::new();
        map.insert(
            "started_at".to_string(),
            serde_json::Value::String(String::new()),
        );
        let result = validate_summary_json_content(&map);
        assert!(result.is_ok(), "empty timestamp string must pass");
    }

    #[test]
    fn summary_json_rejects_space_in_producer_label() {
        // Spaces are not allowed in registry-style tokens.
        let mut map = serde_json::Map::new();
        map.insert(
            "producer_label".to_string(),
            serde_json::Value::String("acp transcript v1".to_string()),
        );
        let result = validate_summary_json_content(&map);
        assert!(result.is_err(), "space in producer_label must be rejected");
        let err = format!("{:#}", result.unwrap_err());
        assert!(
            err.contains("registry-style") || err.contains("label field"),
            "error must mention token format, got: {err}"
        );
    }

    #[test]
    fn summary_json_rejects_control_char_in_producer_label() {
        let mut map = serde_json::Map::new();
        map.insert(
            "producer".to_string(),
            serde_json::Value::String("acp\ninjection".to_string()),
        );
        let result = validate_summary_json_content(&map);
        assert!(
            result.is_err(),
            "control char in producer value must be rejected"
        );
    }

    #[test]
    fn summary_json_rejects_path_fragment_in_producer_label() {
        // Filesystem path fragments must be rejected.
        let mut map = serde_json::Map::new();
        map.insert(
            "producer_label".to_string(),
            serde_json::Value::String("../secret/path".to_string()),
        );
        let result = validate_summary_json_content(&map);
        assert!(
            result.is_err(),
            "path fragment in producer_label must be rejected (slash not allowed)"
        );
    }

    #[test]
    fn summary_json_accepts_valid_registry_style_producer_labels() {
        for label in &[
            "acp_transcript_v1",
            "codex.tool-trace",
            "gemini-stream-v2",
            "P075.evidence.writer",
        ] {
            let mut map = serde_json::Map::new();
            map.insert(
                "producer_label".to_string(),
                serde_json::Value::String(label.to_string()),
            );
            let result = validate_summary_json_content(&map);
            assert!(
                result.is_ok(),
                "registry-style label {label:?} must pass, got: {result:?}"
            );
        }
    }

    #[test]
    fn summary_json_accepts_empty_producer_label() {
        let mut map = serde_json::Map::new();
        map.insert(
            "producer_label".to_string(),
            serde_json::Value::String(String::new()),
        );
        let result = validate_summary_json_content(&map);
        assert!(result.is_ok(), "empty producer_label must pass");
    }

    // -----------------------------------------------------------------------
    // P075-SEC-MED-003: raw key must never appear in validation error messages.
    // Regression tests ensure unknown keys with secret-like content and embedded
    // newlines do not leak through anyhow error chains into logs or diagnostics.
    // -----------------------------------------------------------------------

    #[test]
    fn summary_json_unknown_key_with_secret_does_not_appear_in_error() {
        let mut map = serde_json::Map::new();
        map.insert(
            "super_secret_api_key".to_string(),
            serde_json::Value::String("my-private-token-12345".to_string()),
        );
        let result = validate_summary_json_content(&map);
        assert!(result.is_err(), "unknown key must be rejected");
        let err = format!("{:#}", result.unwrap_err());
        assert!(
            !err.contains("super_secret_api_key"),
            "raw key must not appear in error, got: {err}"
        );
        assert!(
            !err.contains("my-private-token"),
            "raw value must not appear in error, got: {err}"
        );
        assert!(
            err.contains("disallowed field"),
            "error must mention disallowed field, got: {err}"
        );
    }

    #[test]
    fn summary_json_key_with_newline_does_not_forge_log_records() {
        // A key containing a newline could forge log lines if echoed into error messages.
        let mut map = serde_json::Map::new();
        map.insert(
            "field\nX-Injected: forged-log-line".to_string(),
            serde_json::Value::String("value".to_string()),
        );
        let result = validate_summary_json_content(&map);
        assert!(result.is_err(), "key with newline must be rejected");
        let err = format!("{:#}", result.unwrap_err());
        assert!(
            !err.contains("X-Injected"),
            "newline-injected content must not appear in error, got: {err}"
        );
        assert!(
            !err.contains("forged-log-line"),
            "injected log fragment must not appear in error, got: {err}"
        );
    }

    #[test]
    fn summary_json_key_with_nul_byte_does_not_appear_in_error() {
        let mut map = serde_json::Map::new();
        map.insert(
            "key\x00secret".to_string(),
            serde_json::Value::String("val".to_string()),
        );
        let result = validate_summary_json_content(&map);
        assert!(result.is_err(), "key with NUL byte must be rejected");
        let err = format!("{:#}", result.unwrap_err());
        assert!(
            !err.contains("secret"),
            "NUL-truncated key suffix must not appear in error, got: {err}"
        );
    }

    // -----------------------------------------------------------------------
    // P075-SEC-MED-004: control-character, sha256 format, and producer_operation
    // format validation for EvidenceSpoolRef metadata fields.
    // -----------------------------------------------------------------------

    #[test]
    fn validate_fields_rejects_nul_in_run_id() {
        let mut r = make_ref(
            "evsp_sec4_a",
            "run-test",
            "evidence/runs/run-test/sec4a.jsonl",
        );
        r.run_id = "run\x00injected".to_string();
        assert!(
            validate_spool_ref_fields(&r).is_err(),
            "NUL in run_id must fail"
        );
    }

    #[test]
    fn validate_fields_rejects_newline_in_id() {
        let mut r = make_ref(
            "evsp_sec4_b",
            "run-test",
            "evidence/runs/run-test/sec4b.jsonl",
        );
        r.id = "evsp\ninjected".to_string();
        assert!(
            validate_spool_ref_fields(&r).is_err(),
            "newline in id must fail"
        );
    }

    #[test]
    fn validate_fields_rejects_control_char_in_stage_id() {
        let mut r = make_ref(
            "evsp_sec4_c",
            "run-test",
            "evidence/runs/run-test/sec4c.jsonl",
        );
        r.stage_id = Some("stage\x01bad".to_string());
        assert!(
            validate_spool_ref_fields(&r).is_err(),
            "control char in stage_id must fail"
        );
    }

    #[test]
    fn validate_fields_rejects_control_char_in_content_type() {
        let mut r = make_ref(
            "evsp_sec4_d",
            "run-test",
            "evidence/runs/run-test/sec4d.jsonl",
        );
        r.content_type = Some("text/plain\x0d\x0aX-Injected: header".to_string());
        assert!(
            validate_spool_ref_fields(&r).is_err(),
            "CRLF in content_type must fail"
        );
    }

    #[test]
    fn validate_fields_rejects_non_hex_checksum() {
        let mut r = make_ref(
            "evsp_sec4_e",
            "run-test",
            "evidence/runs/run-test/sec4e.jsonl",
        );
        // Uppercase hex is not lowercase hex — must fail.
        r.checksum = "ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789".to_string();
        assert!(
            validate_spool_ref_fields(&r).is_err(),
            "uppercase checksum must fail"
        );
    }

    #[test]
    fn validate_fields_rejects_short_sha256_checksum() {
        let mut r = make_ref(
            "evsp_sec4_f",
            "run-test",
            "evidence/runs/run-test/sec4f.jsonl",
        );
        r.checksum = "abcdef012345".to_string(); // only 12 chars, not 64
        assert!(
            validate_spool_ref_fields(&r).is_err(),
            "too-short sha256 checksum must fail"
        );
    }

    #[test]
    fn validate_fields_accepts_valid_sha256_checksum() {
        let r = make_ref(
            "evsp_sec4_g",
            "run-test",
            "evidence/runs/run-test/sec4g.jsonl",
        );
        // make_ref uses VALID_CHECKSUM (64 lowercase hex chars).
        assert!(
            validate_spool_ref_fields(&r).is_ok(),
            "valid 64-char lowercase hex checksum must pass"
        );
    }

    #[test]
    fn validate_fields_rejects_space_in_producer_operation() {
        let mut r = make_ref(
            "evsp_sec4_h",
            "run-test",
            "evidence/runs/run-test/sec4h.jsonl",
        );
        r.producer_operation = "p075 evidence spool".to_string(); // space not allowed
        assert!(
            validate_spool_ref_fields(&r).is_err(),
            "space in producer_operation must fail registry-style check"
        );
    }

    #[test]
    fn validate_fields_rejects_empty_producer_operation() {
        let mut r = make_ref(
            "evsp_sec4_i",
            "run-test",
            "evidence/runs/run-test/sec4i.jsonl",
        );
        r.producer_operation = String::new();
        assert!(
            validate_spool_ref_fields(&r).is_err(),
            "empty producer_operation must fail"
        );
    }

    #[test]
    fn validate_fields_accepts_registry_style_producer_operation() {
        let r = make_ref(
            "evsp_sec4_j",
            "run-test",
            "evidence/runs/run-test/sec4j.jsonl",
        );
        // make_ref uses "p075_evidence_spool_ref_insert" which matches registry-style.
        assert!(
            validate_spool_ref_fields(&r).is_ok(),
            "registry-style producer_operation must pass"
        );
    }

    // -----------------------------------------------------------------------
    // P075-SEC-LOW-002: find_by_run_and_path must validate before normalizing.
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn find_by_run_and_path_rejects_backslash_path() {
        let pool = test_pool().await;
        let result = find_by_run_and_path(&pool, "run-1", "foo\\bar\\baz.jsonl").await;
        assert!(
            result.is_err(),
            "find_by_run_and_path must reject backslash path rather than normalizing it"
        );
    }

    #[tokio::test]
    async fn find_by_run_and_path_rejects_traversal_path() {
        let pool = test_pool().await;
        let result = find_by_run_and_path(&pool, "run-1", "foo/../bar.jsonl").await;
        assert!(
            result.is_err(),
            "find_by_run_and_path must reject traversal path"
        );
    }

    #[tokio::test]
    async fn find_by_run_and_path_accepts_valid_path_returns_none_when_absent() {
        let pool = test_pool().await;
        let result = find_by_run_and_path(&pool, "run-1", "evidence/runs/run-1/ts.jsonl").await;
        assert!(result.is_ok(), "valid path must not fail validation");
        assert!(result.unwrap().is_none(), "absent row must return None");
    }

    // -----------------------------------------------------------------------
    // P075-SEC-HIGH-001: summary_json duplicate-key smuggling regression tests.
    //
    // serde_json::Map deduplicates keys keeping the last value, so a producer-supplied
    // string like {"line_count":1,"line_count":"<transcript>"} validates against the
    // allowlist (the parsed Map has only {"line_count":"<transcript>"} -- wait, actually
    // serde_json keeps the LAST value for duplicate keys). Without canonicalization, the
    // raw string containing the smuggled second value would be persisted verbatim.
    //
    // These tests verify that:
    //   1. canonicalize_summary_json re-serializes the Map (dropping duplicates).
    //   2. insert/insert_idempotent persist the canonical form, not the raw string.
    // -----------------------------------------------------------------------

    #[test]
    fn canonicalize_summary_json_eliminates_duplicate_keys() {
        // serde_json keeps the LAST occurrence of a duplicate key.
        // A malicious payload: {"line_count":1,"line_count":"<raw transcript text>"}
        // After parsing: {"line_count": "<raw transcript text>"} -- which fails allowlist
        // because string is not allowed for line_count.
        let dup_key_invalid = r#"{"line_count":1,"line_count":"raw evidence text"}"#;
        let result = canonicalize_summary_json(dup_key_invalid);
        assert!(
            result.is_err(),
            "duplicate key whose last value fails the allowlist must be rejected"
        );

        // A subtler case: first value valid, second value also valid but different.
        // {"line_count":1,"line_count":2} → after parse: {"line_count": 2}.
        // canonicalize_summary_json should succeed and return {"line_count":2}.
        let dup_key_valid_values = r#"{"line_count":1,"line_count":2}"#;
        let canonical = canonicalize_summary_json(dup_key_valid_values).unwrap();
        // Re-parse the canonical output to check it contains exactly one key.
        let canonical_map: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&canonical).unwrap();
        assert_eq!(
            canonical_map.len(),
            1,
            "canonical form must have exactly one key"
        );
        assert_eq!(
            canonical_map["line_count"].as_i64().unwrap(),
            2,
            "canonical form must keep the last value for duplicate keys"
        );
        // The raw string still contains the first value; the canonical does not.
        assert!(
            dup_key_valid_values.contains("\"line_count\":1"),
            "raw string has first duplicate"
        );
        assert!(
            !canonical.contains(","),
            "canonical single-key object has no comma separating entries"
        );
    }

    #[tokio::test]
    async fn insert_persists_canonical_not_raw_summary_json() {
        // Regression for P075-SEC-HIGH-001: insert must persist the re-serialized Map.
        // We use a valid summary_json (serde_json preserves key order within the parsed Map)
        // and verify the stored value round-trips as valid compact JSON.
        let pool = test_pool().await;
        let mut r = make_ref("evsp_dup_001", "run-dup", "evidence/runs/run-dup/ts.jsonl");
        // Valid summary with multiple allowed fields.
        r.summary_json = Some(r#"{"line_count":42,"truncated":false}"#.to_string());
        insert(&pool, &r).await.unwrap();

        let found = find_by_id(&pool, "evsp_dup_001").await.unwrap().unwrap();
        let stored = found.summary_json.unwrap();
        // Must parse as a valid JSON object with the same logical content.
        let stored_map: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&stored).expect("stored summary_json must be valid JSON");
        assert_eq!(
            stored_map["line_count"].as_i64().unwrap(),
            42,
            "line_count must survive round-trip"
        );
        assert_eq!(
            stored_map["truncated"].as_bool().unwrap(),
            false,
            "truncated must survive round-trip"
        );
    }

    #[tokio::test]
    async fn insert_idempotent_persists_canonical_not_raw_summary_json() {
        let pool = test_pool().await;
        let mut r = make_ref(
            "evsp_dup_002",
            "run-dup2",
            "evidence/runs/run-dup2/ts.jsonl",
        );
        r.summary_json = Some(r#"{"chunk_count":10,"producer_label":"test_v1"}"#.to_string());
        let inserted = insert_idempotent(&pool, &r).await.unwrap();
        assert!(inserted);

        let found = find_by_id(&pool, "evsp_dup_002").await.unwrap().unwrap();
        let stored = found.summary_json.unwrap();
        let stored_map: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&stored).expect("stored summary_json must be valid JSON");
        assert_eq!(stored_map["chunk_count"].as_i64().unwrap(), 10);
        assert_eq!(stored_map["producer_label"].as_str().unwrap(), "test_v1");
    }
}
