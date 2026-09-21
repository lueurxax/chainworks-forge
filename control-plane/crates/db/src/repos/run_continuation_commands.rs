//! Immutable command outcomes, committed with their continuation mutation.
//! Boundary authorization must be rechecked before every lookup, including replay.
use anyhow::{ensure, Result};
use domain::run_carry_forward_api::RunContinuationCommandResultV1;
use sqlx::{Sqlite, SqlitePool, Transaction};

#[derive(Debug, thiserror::Error)]
#[error("continuation command commit outcome unknown")]
pub struct CommitOutcomeUnknown;

pub fn unknown_commit(error: anyhow::Error) -> anyhow::Error {
    error.context(CommitOutcomeUnknown)
}

pub struct CommandIdentity<'a> {
    pub caller_fingerprint: &'a str,
    pub caller_request_id: &'a str,
    pub command: &'a str,
    pub intent_sha256: &'a str,
}

fn decode(
    row: Option<(String, String, String)>,
    command: &str,
    intent: &str,
) -> Result<Option<RunContinuationCommandResultV1>> {
    match row {
        Some((stored_command, stored_intent, json)) => {
            ensure!(
                stored_command == command && stored_intent == intent,
                "idempotency_conflict"
            );
            let result: RunContinuationCommandResultV1 = serde_json::from_str(&json)?;
            result.validate()?;
            Ok(Some(result))
        }
        None => Ok(None),
    }
}

pub async fn find(
    pool: &SqlitePool,
    caller: &str,
    request: &str,
    command: &str,
    intent: &str,
) -> Result<Option<RunContinuationCommandResultV1>> {
    let row=sqlx::query_as("SELECT command,intent_sha256,result_json FROM run_continuation_commands WHERE caller_fingerprint=? AND caller_request_id=?")
        .bind(caller).bind(request).fetch_optional(pool).await?;
    decode(row, command, intent)
}

pub async fn find_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &CommandIdentity<'_>,
) -> Result<Option<RunContinuationCommandResultV1>> {
    let row=sqlx::query_as("SELECT command,intent_sha256,result_json FROM run_continuation_commands WHERE caller_fingerprint=? AND caller_request_id=?")
        .bind(id.caller_fingerprint).bind(id.caller_request_id).fetch_optional(&mut **tx).await?;
    decode(row, id.command, id.intent_sha256)
}

pub async fn record_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &CommandIdentity<'_>,
    source: &str,
    result: &RunContinuationCommandResultV1,
) -> Result<()> {
    result.validate()?;
    ensure!(
        result.caller_request_id.as_uuid().to_string() == id.caller_request_id,
        "idempotency_conflict"
    );
    let json = serde_json::to_string(result)?;
    ensure!(json.len() <= 65536, "continuation result bound");
    if let Some(previous) = find_tx(tx, id).await? {
        ensure!(previous == *result, "idempotency_conflict");
        return Ok(());
    }
    sqlx::query("INSERT INTO run_continuation_commands(caller_fingerprint,caller_request_id,command,intent_sha256,source_run_id,operation_id,result_json,created_at) VALUES (?,?,?,?,?,?,?,?)")
        .bind(id.caller_fingerprint).bind(id.caller_request_id).bind(id.command).bind(id.intent_sha256).bind(source)
        .bind(result.operation.as_ref().map(|op|op.operation_id.as_uuid().to_string())).bind(json).bind(chrono::Utc::now().to_rfc3339())
        .execute(&mut **tx).await?;
    Ok(())
}

pub fn accepted(
    op: &super::run_continuations::Continuation,
    request: &str,
    journal: &str,
) -> Result<RunContinuationCommandResultV1> {
    Ok(serde_json::from_value(serde_json::json!({
        "schema_version":"run_continuation_command_result_v1","status":"accepted","caller_request_id":request,
        "journal_id":journal,"operation":{"operation_id":op.operation_id,"phase":op.phase,"version":op.version,"successor_run_id":op.successor_run_id},
        "denial":null,"next_action":if op.phase=="preparing" {"await_worker"} else {"read_operation"},"replay_of":null
    }))?)
}

pub async fn record_denial(
    pool: &SqlitePool,
    id: &CommandIdentity<'_>,
    source: &str,
    operation: Option<&super::run_continuations::Continuation>,
    code: domain::run_carry_forward_api::ContinuationReasonCode,
) -> Result<RunContinuationCommandResultV1> {
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "run_continuation_commands.deny").await?;
    if let Some(previous) = find_tx(&mut tx, id).await? {
        return Ok(previous.replayed()?);
    }
    if let Some(op) = operation {
        ensure!(op.source_run_id == source, "artifact_provenance_invalid");
    }
    let message = serde_json::to_value(code)?;
    let result = serde_json::from_value(serde_json::json!({
        "schema_version":"run_continuation_command_result_v1","status":"denied","caller_request_id":id.caller_request_id,
        "journal_id":operation.map(|op|&op.journal_id),
        "operation":operation.map(|op|serde_json::json!({"operation_id":op.operation_id,"phase":op.phase,"version":op.version,"successor_run_id":op.successor_run_id})),
        "denial":{"code":code,"message":message},"next_action":"resolve_hold","replay_of":null
    }))?;
    record_tx(&mut tx, id, source, &result).await?;
    tx.commit().await.map_err(unknown_commit)?;
    Ok(result)
}
