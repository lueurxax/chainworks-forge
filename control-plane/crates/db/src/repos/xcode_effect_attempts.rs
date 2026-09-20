//! Standalone durable effect journal; no host bytes, permissions or endpoint.
//!
//! Mutations consume a single-purpose, writer-admitted IMMEDIATE transaction and
//! commit it before returning. The future engine integration must register its
//! Class A barrier operations, check caller authority and stop admission during
//! startup recovery. Accepting QueuedTransaction preserves P075 admission,
//! cancellation, deadlines and commit handling without adding an unregistered
//! production write route or falling back to a raw pool transaction.

use chrono::{DateTime, Utc};
use domain::xcode_effect::*;
use serde::{de::DeserializeOwned, Serialize};
use sqlx::{sqlite::SqliteRow, Row, SqliteConnection, SqlitePool};
use uuid::Uuid;

use crate::writer::QueuedTransaction;

pub type Result<T> = std::result::Result<T, JournalError>;

#[derive(Debug, thiserror::Error)]
pub enum JournalError {
    #[error(transparent)]
    InvalidInput(#[from] InputError),
    #[error("xcode_effect_intent_conflict")]
    IntentConflict,
    #[error("xcode_effect_project_held")]
    ProjectHeld,
    #[error("xcode_effect_revision_conflict")]
    RevisionConflict,
    #[error("xcode_effect_invalid_transition")]
    InvalidTransition,
    #[error("xcode_effect_not_found")]
    NotFound,
    #[error("xcode_effect_corrupt_record")]
    CorruptRecord,
    #[error("xcode_effect_storage_failure")]
    Storage(#[source] anyhow::Error),
}

impl From<sqlx::Error> for JournalError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage(error.into())
    }
}

impl From<anyhow::Error> for JournalError {
    fn from(error: anyhow::Error) -> Self {
        Self::Storage(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DispatchDecision {
    /// The caller may send only after this committed decision, subject to its
    /// still-valid engine/coordinator authority. This is not a permission grant.
    Dispatch(StoredAttempt),
    /// Never resend: map the historical/in-progress/unknown state at the endpoint.
    Existing(StoredAttempt),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttemptPage {
    pub items: Vec<StoredAttempt>,
    pub next_cursor: Option<String>,
}

pub async fn prepare(
    mut tx: QueuedTransaction,
    intent: &NormalizedIntent,
    now: DateTime<Utc>,
) -> Result<StoredAttempt> {
    intent.validate()?;
    validate_time(now, None)?;
    let project = encode(&intent.project_key)?;
    let existing = sqlx::query("SELECT * FROM xcode_effect_attempts WHERE owner_lineage = ? AND project_key = ? AND operation_key = ?")
        .bind(&intent.owner_lineage).bind(&project).bind(intent.operation_key.to_string())
        .fetch_optional(&mut **tx).await?;
    if let Some(row) = existing {
        let existing = parse(row)?;
        if !existing.intent.same_operation(intent) {
            return Err(JournalError::IntentConflict);
        }
        tx.commit().await?;
        return Ok(existing);
    }
    ensure_unheld(&mut tx, &intent.project_key).await?;
    let id = Uuid::new_v4().to_string();
    let row = sqlx::query("INSERT INTO xcode_effect_attempts (attempt_id, nonce, owner_lineage, project_key, operation_key, intent_json, state, revision, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, 'prepared', 0, ?, ?) RETURNING *")
        .bind(&id).bind(&id).bind(&intent.owner_lineage).bind(&project)
        .bind(intent.operation_key.to_string()).bind(encode(intent)?)
        .bind(now.to_rfc3339()).bind(now.to_rfc3339()).fetch_one(&mut **tx).await?;
    let attempt = parse(row)?;
    tx.commit().await?;
    Ok(attempt)
}

pub async fn dispatch(
    mut tx: QueuedTransaction,
    owner_lineage: &str,
    nonce: Uuid,
    request_digest: &str,
    expected_revision: i64,
    now: DateTime<Utc>,
) -> Result<DispatchDecision> {
    validate_digest("request_digest", request_digest)?;
    validate_revision(expected_revision)?;
    let mut attempt = load(&mut tx, owner_lineage, nonce).await?;
    if attempt.intent.request_digest != request_digest {
        return Err(JournalError::IntentConflict);
    }
    if attempt.state != AttemptState::Prepared {
        tx.commit().await?;
        return Ok(DispatchDecision::Existing(attempt));
    }
    check_transition(&attempt, expected_revision, AttemptState::Prepared, now)?;
    let project = encode(&attempt.intent.project_key)?;
    ensure_unheld(&mut tx, &attempt.intent.project_key).await?;
    attempt.state = AttemptState::Dispatched;
    attempt.dispatched_at = Some(now);
    persist(&mut tx, &mut attempt, expected_revision, now).await?;
    sqlx::query("INSERT INTO xcode_project_holds (project_key, attempt_id, uid, canonical_path, device, inode) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(project).bind(attempt.attempt_id.to_string())
        .bind(i64::from(attempt.intent.project_key.uid))
        .bind(&attempt.intent.project_key.canonical_path)
        .bind(&attempt.intent.project_key.device).bind(&attempt.intent.project_key.inode)
        .execute(&mut **tx).await?;
    tx.commit().await?;
    Ok(DispatchDecision::Dispatch(attempt))
}

pub async fn complete(
    mut tx: QueuedTransaction,
    owner_lineage: &str,
    attempt_id: Uuid,
    expected_revision: i64,
    completion: &Completion,
    now: DateTime<Utc>,
) -> Result<StoredAttempt> {
    let mut attempt = load(&mut tx, owner_lineage, attempt_id).await?;
    check_transition(&attempt, expected_revision, AttemptState::Dispatched, now)?;
    require_hold(&mut tx, &attempt).await?;
    match completion {
        Completion::Succeeded { outcome, proof } | Completion::Failed { outcome, proof } => {
            outcome.validate(&attempt.intent.result_schema)?;
            proof.validate()?;
            if matches!(completion, Completion::Succeeded { .. }) {
                outcome.require_result()?;
                attempt.state = AttemptState::Succeeded;
            } else {
                outcome.require_error(EffectErrorCode::TerminalFailure)?;
                attempt.state = AttemptState::Failed;
            }
            attempt.completed_at = Some(now);
            attempt.outcome = Some(outcome.clone());
            attempt.result_digest = Some(outcome.result_digest.clone());
            attempt.settlement_proof = Some(proof.clone());
        }
        Completion::Unknown { reason } => {
            attempt.state = AttemptState::Unknown;
            attempt.uncertainty_reason = Some(*reason);
        }
    }
    persist(&mut tx, &mut attempt, expected_revision, now).await?;
    if attempt.state != AttemptState::Unknown {
        release_hold(&mut tx, &attempt).await?;
    }
    tx.commit().await?;
    Ok(attempt)
}

pub async fn cancel(
    mut tx: QueuedTransaction,
    owner_lineage: &str,
    attempt_id: Uuid,
    expected_revision: i64,
    outcome: &HistoricalResult,
    now: DateTime<Utc>,
) -> Result<StoredAttempt> {
    let mut attempt = load(&mut tx, owner_lineage, attempt_id).await?;
    check_transition(&attempt, expected_revision, AttemptState::Prepared, now)?;
    outcome.validate(&attempt.intent.result_schema)?;
    outcome.require_error(EffectErrorCode::CancelledBeforeDispatch)?;
    attempt.state = AttemptState::Failed;
    attempt.completed_at = Some(now);
    attempt.outcome = Some(outcome.clone());
    attempt.result_digest = Some(outcome.result_digest.clone());
    persist(&mut tx, &mut attempt, expected_revision, now).await?;
    tx.commit().await?;
    Ok(attempt)
}

/// Internal startup inventory, not owner-facing readback. Admission must stay
/// closed until every dispatched attempt has been converted to an unknown hold.
pub async fn list_dispatched_revisions(
    pool: &SqlitePool,
    limit: u32,
) -> Result<Vec<AttemptRevision>> {
    if !(1..=100).contains(&limit) {
        return Err(InputError::new("limit", InputReasonCode::InvalidLimit).into());
    }
    let rows = sqlx::query(
        "SELECT * FROM xcode_effect_attempts WHERE state = 'dispatched' ORDER BY sequence LIMIT ?",
    )
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            let attempt = parse(row)?;
            Ok(AttemptRevision {
                attempt_id: attempt.attempt_id,
                revision: attempt.revision,
            })
        })
        .collect()
}

/// Recovery is an explicit bounded CAS batch. The startup coordinator
/// enumerates dispatched attempts under closed admission and calls this until
/// none remain. It must not switch databases to discard uncertainty.
pub async fn recover_dispatched(
    mut tx: QueuedTransaction,
    expected: &[AttemptRevision],
    now: DateTime<Utc>,
) -> Result<Vec<StoredAttempt>> {
    if expected.is_empty() || expected.len() > 100 {
        return Err(InputError::new("recovery_batch", InputReasonCode::InvalidLimit).into());
    }
    let mut recovered = Vec::with_capacity(expected.len());
    for expected in expected {
        validate_uuid("attempt_id", expected.attempt_id)?;
        let row = sqlx::query("SELECT * FROM xcode_effect_attempts WHERE attempt_id = ?")
            .bind(expected.attempt_id.to_string())
            .fetch_optional(&mut **tx)
            .await?
            .ok_or(JournalError::NotFound)?;
        let mut attempt = parse(row)?;
        check_transition(&attempt, expected.revision, AttemptState::Dispatched, now)?;
        require_hold(&mut tx, &attempt).await?;
        attempt.state = AttemptState::Unknown;
        attempt.uncertainty_reason = Some(UncertaintyReason::Restart);
        persist(&mut tx, &mut attempt, expected.revision, now).await?;
        recovered.push(attempt);
    }
    tx.commit().await?;
    Ok(recovered)
}

pub async fn reconcile(
    mut tx: QueuedTransaction,
    owner_lineage: &str,
    attempt_id: Uuid,
    expected_revision: i64,
    reconciliation: &Reconciliation,
    now: DateTime<Utc>,
) -> Result<StoredAttempt> {
    let mut attempt = load(&mut tx, owner_lineage, attempt_id).await?;
    check_transition(&attempt, expected_revision, AttemptState::Unknown, now)?;
    reconciliation.validate(&attempt.intent.result_schema)?;
    require_hold(&mut tx, &attempt).await?;
    attempt.state = AttemptState::Reconciled;
    attempt.completed_at = Some(now);
    attempt.outcome = Some(reconciliation.outcome.clone());
    attempt.result_digest = Some(reconciliation.outcome.result_digest.clone());
    attempt.settlement_proof = Some(reconciliation.proof.clone());
    attempt.reconciliation = Some(reconciliation.clone());
    attempt.reconciliation_evidence_ref = Some(reconciliation.proof.evidence_ref.clone());
    persist(&mut tx, &mut attempt, expected_revision, now).await?;
    release_hold(&mut tx, &attempt).await?;
    tx.commit().await?;
    Ok(attempt)
}

pub async fn get(
    pool: &SqlitePool,
    owner_lineage: &str,
    attempt_id: Uuid,
) -> Result<Option<StoredAttempt>> {
    validate_text("owner_lineage", owner_lineage, 256)?;
    validate_uuid("attempt_id", attempt_id)?;
    sqlx::query("SELECT * FROM xcode_effect_attempts WHERE owner_lineage = ? AND attempt_id = ?")
        .bind(owner_lineage)
        .bind(attempt_id.to_string())
        .fetch_optional(pool)
        .await?
        .map(parse)
        .transpose()
}

pub async fn list(
    pool: &SqlitePool,
    owner_lineage: &str,
    limit: u32,
    cursor: Option<&str>,
) -> Result<AttemptPage> {
    validate_text("owner_lineage", owner_lineage, 256)?;
    if !(1..=100).contains(&limit) {
        return Err(InputError::new("limit", InputReasonCode::InvalidLimit).into());
    }
    // A cursor is a retained attempt identity, resolved only inside this owner.
    let after = if let Some(cursor) = cursor {
        let invalid = || InputError::new("cursor", InputReasonCode::InvalidCursor);
        let id = Uuid::parse_str(cursor).map_err(|_| invalid())?;
        if id.to_string() != cursor {
            return Err(invalid().into());
        }
        get(pool, owner_lineage, id)
            .await?
            .ok_or_else(invalid)?
            .sequence
    } else {
        0
    };
    let rows = sqlx::query("SELECT * FROM xcode_effect_attempts WHERE owner_lineage = ? AND sequence > ? ORDER BY sequence LIMIT ?")
        .bind(owner_lineage).bind(after).bind(i64::from(limit) + 1).fetch_all(pool).await?;
    let mut items = rows.into_iter().map(parse).collect::<Result<Vec<_>>>()?;
    let more = items.len() > limit as usize;
    items.truncate(limit as usize);
    let next_cursor = if more {
        items.last().map(|a| a.attempt_id.to_string())
    } else {
        None
    };
    Ok(AttemptPage { items, next_cursor })
}

async fn load(
    conn: &mut SqliteConnection,
    owner_lineage: &str,
    attempt_id: Uuid,
) -> Result<StoredAttempt> {
    validate_text("owner_lineage", owner_lineage, 256)?;
    validate_uuid("attempt_id", attempt_id)?;
    let row = sqlx::query(
        "SELECT * FROM xcode_effect_attempts WHERE owner_lineage = ? AND attempt_id = ?",
    )
    .bind(owner_lineage)
    .bind(attempt_id.to_string())
    .fetch_optional(conn)
    .await?
    .ok_or(JournalError::NotFound)?;
    parse(row)
}

fn check_transition(
    attempt: &StoredAttempt,
    revision: i64,
    from: AttemptState,
    now: DateTime<Utc>,
) -> Result<()> {
    validate_revision(revision)?;
    if attempt.revision != revision {
        return Err(JournalError::RevisionConflict);
    }
    if attempt.state != from {
        return Err(JournalError::InvalidTransition);
    }
    validate_time(now, Some(attempt.updated_at))?;
    Ok(())
}

async fn ensure_unheld(conn: &mut SqliteConnection, project: &ProjectKey) -> Result<()> {
    let held: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM xcode_project_holds WHERE uid = ? AND (canonical_path = ? OR (device = ? AND inode = ?)))")
        .bind(i64::from(project.uid)).bind(&project.canonical_path)
        .bind(&project.device).bind(&project.inode).fetch_one(conn).await?;
    if held {
        Err(JournalError::ProjectHeld)
    } else {
        Ok(())
    }
}

pub async fn project_is_held(pool: &SqlitePool, project: &ProjectKey) -> Result<bool> {
    project.validate()?;
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM xcode_project_holds WHERE uid = ? AND (canonical_path = ? OR (device = ? AND inode = ?)))")
        .bind(i64::from(project.uid)).bind(&project.canonical_path).bind(&project.device).bind(&project.inode)
        .fetch_one(pool).await.map_err(JournalError::from)
}

async fn require_hold(conn: &mut SqliteConnection, attempt: &StoredAttempt) -> Result<()> {
    let held: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM xcode_project_holds WHERE project_key = ? AND attempt_id = ?)",
    )
    .bind(encode(&attempt.intent.project_key)?)
    .bind(attempt.attempt_id.to_string())
    .fetch_one(conn)
    .await?;
    if held {
        Ok(())
    } else {
        Err(JournalError::CorruptRecord)
    }
}

async fn release_hold(conn: &mut SqliteConnection, attempt: &StoredAttempt) -> Result<()> {
    let count =
        sqlx::query("DELETE FROM xcode_project_holds WHERE project_key = ? AND attempt_id = ?")
            .bind(encode(&attempt.intent.project_key)?)
            .bind(attempt.attempt_id.to_string())
            .execute(conn)
            .await?
            .rows_affected();
    if count != 1 {
        return Err(JournalError::CorruptRecord);
    }
    Ok(())
}

async fn persist(
    conn: &mut SqliteConnection,
    attempt: &mut StoredAttempt,
    revision: i64,
    now: DateTime<Utc>,
) -> Result<()> {
    let count = sqlx::query("UPDATE xcode_effect_attempts SET state = ?, revision = revision + 1, updated_at = ?, dispatched_at = ?, completed_at = ?, outcome_json = ?, result_digest = ?, uncertainty_reason = ?, settlement_proof_json = ?, reconciliation_json = ?, reconciliation_evidence_ref = ? WHERE attempt_id = ? AND revision = ?")
        .bind(attempt.state.as_str()).bind(now.to_rfc3339())
        .bind(attempt.dispatched_at.map(|t| t.to_rfc3339())).bind(attempt.completed_at.map(|t| t.to_rfc3339()))
        .bind(attempt.outcome.as_ref().map(encode).transpose()?).bind(&attempt.result_digest)
        .bind(attempt.uncertainty_reason.map(|v| enum_token(&v)).transpose()?)
        .bind(attempt.settlement_proof.as_ref().map(encode).transpose()?)
        .bind(attempt.reconciliation.as_ref().map(encode).transpose()?).bind(&attempt.reconciliation_evidence_ref)
        .bind(attempt.attempt_id.to_string()).bind(revision).execute(conn).await?.rows_affected();
    if count != 1 {
        return Err(JournalError::RevisionConflict);
    }
    attempt.revision = revision + 1;
    attempt.updated_at = now;
    Ok(())
}

fn encode<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(|_| JournalError::CorruptRecord)
}

fn decode<T: DeserializeOwned>(value: &str) -> Result<T> {
    serde_json::from_str(value).map_err(|_| JournalError::CorruptRecord)
}

fn enum_token<T: Serialize>(value: &T) -> Result<String> {
    match serde_json::to_value(value).map_err(|_| JournalError::CorruptRecord)? {
        serde_json::Value::String(s) => Ok(s),
        _ => Err(JournalError::CorruptRecord),
    }
}

fn parse(row: SqliteRow) -> Result<StoredAttempt> {
    let dt = |s: &str| {
        DateTime::parse_from_rfc3339(s)
            .map(|t| t.with_timezone(&Utc))
            .map_err(|_| JournalError::CorruptRecord)
    };
    let id = |s: &str| Uuid::parse_str(s).map_err(|_| JournalError::CorruptRecord);
    let attempt = StoredAttempt {
        sequence: row.try_get("sequence")?,
        attempt_id: id(row.try_get("attempt_id")?)?,
        nonce: id(row.try_get("nonce")?)?,
        intent: decode(row.try_get("intent_json")?)?,
        state: decode(&encode(&row.try_get::<String, _>("state")?)?)?,
        revision: row.try_get("revision")?,
        created_at: dt(row.try_get("created_at")?)?,
        updated_at: dt(row.try_get("updated_at")?)?,
        dispatched_at: row
            .try_get::<Option<String>, _>("dispatched_at")?
            .as_deref()
            .map(dt)
            .transpose()?,
        completed_at: row
            .try_get::<Option<String>, _>("completed_at")?
            .as_deref()
            .map(dt)
            .transpose()?,
        outcome: row
            .try_get::<Option<String>, _>("outcome_json")?
            .as_deref()
            .map(decode)
            .transpose()?,
        result_digest: row.try_get("result_digest")?,
        uncertainty_reason: row
            .try_get::<Option<String>, _>("uncertainty_reason")?
            .map(|s| decode(&encode(&s)?))
            .transpose()?,
        settlement_proof: row
            .try_get::<Option<String>, _>("settlement_proof_json")?
            .as_deref()
            .map(decode)
            .transpose()?,
        reconciliation: row
            .try_get::<Option<String>, _>("reconciliation_json")?
            .as_deref()
            .map(decode)
            .transpose()?,
        reconciliation_evidence_ref: row.try_get("reconciliation_evidence_ref")?,
    };
    attempt
        .intent
        .validate()
        .map_err(|_| JournalError::CorruptRecord)?;
    if let Some(outcome) = &attempt.outcome {
        outcome
            .validate(&attempt.intent.result_schema)
            .map_err(|_| JournalError::CorruptRecord)?;
    }
    if let Some(proof) = &attempt.settlement_proof {
        proof.validate().map_err(|_| JournalError::CorruptRecord)?;
    }
    if let Some(reconciliation) = &attempt.reconciliation {
        reconciliation
            .validate(&attempt.intent.result_schema)
            .map_err(|_| JournalError::CorruptRecord)?;
    }
    Ok(attempt)
}
