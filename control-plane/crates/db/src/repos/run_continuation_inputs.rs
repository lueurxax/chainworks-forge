//! Installed carry-forward inputs are not provider artifacts or active outputs.
use anyhow::{ensure, Result};
use domain::{
    ids::{ArtifactId, RunId},
    run_carry_forward::{ContentDigest, EntryRole},
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Sqlite, SqlitePool, Transaction};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedInput {
    pub input_id: Uuid,
    pub ordinal: usize,
    pub role: EntryRole,
    pub source_kind: String,
    pub source_id: Uuid,
    pub source_run_id: RunId,
    pub original_artifact_id: ArtifactId,
    pub source_schema: String,
    pub content_sha256: ContentDigest,
    pub target_logical_name: String,
    pub target_relative_path: String,
    pub historical_relation: serde_json::Value,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct InstalledInput {
    pub input_id: String,
    pub operation_id: String,
    pub ordinal: i64,
    pub successor_run_id: String,
    pub role: String,
    pub source_kind: String,
    pub source_id: String,
    pub source_run_id: String,
    pub original_artifact_id: String,
    pub source_schema: String,
    pub content_sha256: String,
    pub target_logical_name: String,
    pub target_relative_path: String,
    pub historical_relation_json: String,
}

fn hex(digest: &ContentDigest) -> &str {
    digest
        .as_str()
        .strip_prefix("sha256:")
        .expect("validated digest")
}

pub async fn insert_tx(
    tx: &mut Transaction<'_, Sqlite>,
    op: &super::run_continuations::Continuation,
    inputs: &[PreparedInput],
) -> Result<()> {
    ensure!(
        op.phase == "prepared" && !op.worker_claimed,
        "effect_outcome_unknown"
    );
    ensure!(
        !inputs.is_empty() && inputs.len() <= 129,
        "artifact_provenance_invalid"
    );
    ensure!(
        inputs
            .iter()
            .filter(|i| i.role == EntryRole::ExecutionSeed)
            .count()
            == 1,
        "artifact_provenance_invalid"
    );
    let mut ids = BTreeSet::new();
    let mut targets = BTreeSet::new();
    for (ordinal, input) in inputs.iter().enumerate() {
        ensure!(
            input.ordinal == ordinal
                && ids.insert(input.input_id)
                && targets.insert(input.target_relative_path.clone()),
            "artifact_provenance_invalid"
        );
        ensure!(
            input.source_run_id.to_string() == op.source_run_id,
            "artifact_provenance_invalid"
        );
        let path = &input.target_relative_path;
        ensure!(
            !path.is_empty()
                && path.len() <= 4096
                && !path.chars().any(char::is_control)
                && !path.contains('\\')
                && path
                    .split('/')
                    .all(|p| !p.is_empty() && p != "." && p != ".."),
            "unsafe_path"
        );
        match input.role {
            EntryRole::ExecutionSeed => ensure!(
                input.target_logical_name == "proposal_current",
                "artifact_provenance_invalid"
            ),
            EntryRole::ReferenceOnly => ensure!(
                *path == format!("carry-forward/references/{}/content", input.input_id),
                "unsafe_path"
            ),
            _ => anyhow::bail!("artifact_provenance_invalid"),
        }
        let owned:bool=match input.source_kind.as_str() {
            "artifact"=>{
                ensure!(input.source_id==input.original_artifact_id.0,"artifact_provenance_invalid");
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM artifacts WHERE id=? AND run_id=? AND (checksum_sha256 IS NULL OR checksum_sha256=?))")
                    .bind(input.source_id.to_string()).bind(&op.source_run_id).bind(hex(&input.content_sha256)).fetch_one(&mut **tx).await?
            }
            "carried_input"=>sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM run_continuation_inputs i JOIN run_continuations c ON c.operation_id=i.operation_id WHERE i.input_id=? AND i.successor_run_id=? AND i.original_artifact_id=? AND i.content_sha256=? AND i.source_schema=? AND i.installed=1 AND c.phase='activated')")
                .bind(input.source_id.to_string()).bind(&op.source_run_id).bind(input.original_artifact_id.to_string()).bind(hex(&input.content_sha256)).bind(&input.source_schema).fetch_one(&mut **tx).await?,
            _=>false,
        };
        ensure!(owned, "artifact_provenance_invalid");
    }
    for input in inputs {
        sqlx::query("INSERT INTO run_continuation_inputs(input_id,operation_id,ordinal,role,source_kind,source_id,source_run_id,original_artifact_id,source_schema,content_sha256,target_logical_name,target_relative_path,historical_relation_json) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)")
            .bind(input.input_id.to_string()).bind(&op.operation_id).bind(input.ordinal as i64)
            .bind(if input.role==EntryRole::ExecutionSeed {"execution_seed"}else{"reference_only"}).bind(&input.source_kind).bind(input.source_id.to_string())
            .bind(input.source_run_id.to_string()).bind(input.original_artifact_id.to_string()).bind(&input.source_schema).bind(hex(&input.content_sha256))
            .bind(&input.target_logical_name).bind(&input.target_relative_path).bind(serde_json::to_string(&input.historical_relation)?)
            .execute(&mut **tx).await?;
    }
    Ok(())
}

pub async fn execution_seed(
    pool: &SqlitePool,
    run: RunId,
    logical_name: &str,
) -> Result<Option<InstalledInput>> {
    Ok(sqlx::query_as("SELECT i.* FROM run_continuation_inputs i JOIN run_continuations c ON c.operation_id=i.operation_id WHERE i.successor_run_id=? AND i.target_logical_name=? AND i.role='execution_seed' AND i.installed=1 AND c.phase='activated'")
        .bind(run.to_string()).bind(logical_name).fetch_optional(pool).await?)
}

pub async fn references(pool: &SqlitePool, run: RunId) -> Result<Vec<InstalledInput>> {
    Ok(sqlx::query_as("SELECT i.* FROM run_continuation_inputs i JOIN run_continuations c ON c.operation_id=i.operation_id WHERE i.successor_run_id=? AND i.role='reference_only' AND i.installed=1 AND c.phase='activated' ORDER BY i.ordinal LIMIT 128")
        .bind(run.to_string()).fetch_all(pool).await?)
}
