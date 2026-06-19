use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use sqlx::{Row, Sqlite, SqlitePool, Transaction};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Instant;

use domain::agent::{AgentOutputSettlement, ArtifactSourceClaimState};
use domain::artifact_contracts::{
    known_contract_id, normalize_contract_status, parse_implementation_self_assessment_v2,
    ActiveArtifactGenerationInput, ArtifactContractOverride, ArtifactContractOverrideInput,
    ArtifactSourceGenerationClaim, ArtifactSourceGenerationClaimKey, ContractParseContext,
    ImplementationSelfAssessmentStatus, ImplementationSelfAssessmentSummary,
    SourceGenerationImportDecision, ValidationIssue, IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH,
    IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
};
use domain::closeout_readiness::{
    CloseoutFingerprint, IMPLEMENTATION_CLOSEOUT_READINESS_V1_CONTRACT_ID,
};
use domain::ids::{AgentExecutionId, ArtifactId, RunId};
use domain::mediation::OwnerKind;
use domain::proposal_gate_result::PROPOSAL_GATE_RESULT_V1_CONTRACT_ID;

use crate::pool::log_write_transaction;
use crate::writer::begin_registered_immediate_transaction;

const ARTIFACT_CONTRACT_READBACK_MAX_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug)]
struct ArtifactReadRoots {
    run_id: RunId,
    chainworks_meta_root: Option<PathBuf>,
    artifact_root: PathBuf,
}

#[derive(Clone, Debug)]
pub struct RunStateProjectionRow {
    pub run_id: RunId,
    pub active_index_json: serde_json::Value,
    pub run_state_json: serde_json::Value,
    pub exported_active_index_path: Option<String>,
    pub exported_run_state_path: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActiveArtifactContractGenerationRow {
    pub generation_id: String,
    pub contract_id: String,
    pub canonical_path: String,
    pub raw_path: String,
    pub raw_status: String,
    pub canonical_status: String,
    pub output_settlement: String,
    pub source_generation_verified: bool,
    pub canonical_dimensions_json: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CanonicalContractField {
    Resolved(serde_json::Value),
    MissingControlled { contract_id: String },
    UncontrolledAlias,
}

pub async fn upsert_generation_and_rebuild(
    pool: &SqlitePool,
    input: ActiveArtifactGenerationInput,
) -> Result<()> {
    let run_id = input.run_id;
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "artifact_contracts.upsert_generation")
            .await?;
    upsert_generation_and_rebuild_tx(&mut tx, input).await?;
    tx.commit().await?;
    export_projection_files(pool, run_id).await
}

pub async fn upsert_verified_generation_and_rebuild(
    pool: &SqlitePool,
    input: ActiveArtifactGenerationInput,
) -> Result<()> {
    let run_id = input.run_id;
    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "artifact_contracts.upsert_verified_generation",
    )
    .await?;
    let read_roots = artifact_read_roots_for_run_tx(&mut tx, input.run_id).await?;
    record_p094_followup_seed_metric_if_needed(&input, &read_roots);
    let raw_status = effective_raw_status_for_generation(
        &read_roots,
        &input.contract_id,
        &input.raw_path,
        &input.raw_status,
    )?;
    let normalized =
        normalize_contract_status(&input.contract_id, &raw_status).map_err(anyhow::Error::msg)?;
    let mut warnings = input.warnings.clone();
    warnings.extend(normalized.warnings.clone());
    let warnings_json = serde_json::to_string(&warnings)?;
    let validation_errors_json = serde_json::to_string(&normalized.validation_errors)?;
    let now = Utc::now().to_rfc3339();
    let supersedes_generation_id = effective_supersedes_generation_id_tx(
        &mut tx,
        input.run_id,
        &input.contract_id,
        &input.generation_id,
        input.supersedes_generation_id.clone(),
        normalized.valid,
    )
    .await?;
    let mut verified_input = input;
    verified_input.supersedes_generation_id = supersedes_generation_id;
    insert_generation_in_tx(
        &mut tx,
        &verified_input,
        &normalized.canonical_status,
        normalized.valid,
        true,
        &warnings_json,
        &validation_errors_json,
        &now,
        verified_input.output_settlement.clone(),
    )
    .await?;
    if normalized.valid {
        sqlx::query(
            r#"INSERT INTO active_artifact_contracts (run_id, contract_id, generation_id, updated_at)
               VALUES (?1, ?2, ?3, ?4)
               ON CONFLICT(run_id, contract_id) DO UPDATE SET
                 generation_id = excluded.generation_id,
                 updated_at = excluded.updated_at"#,
        )
        .bind(verified_input.run_id.to_string())
        .bind(&verified_input.contract_id)
        .bind(&verified_input.generation_id)
        .bind(&now)
        .execute(&mut **tx)
        .await?;
    }
    rebuild_run_state_projection_tx(&mut tx, run_id).await?;
    tx.commit().await?;
    export_projection_files(pool, run_id).await
}

fn record_p094_followup_seed_metric_if_needed(
    input: &ActiveArtifactGenerationInput,
    roots: &ArtifactReadRoots,
) {
    if input.contract_id != "followup_proposal_seed_v1" {
        return;
    }
    let tail_class = read_confined_artifact_bytes(roots, &input.raw_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|value| {
            value
                .get("tail_class")
                .or_else(|| value.get("class"))
                .or_else(|| value.get("status"))
                .and_then(|field| field.as_str())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| "unspecified".to_string());
    crate::metrics::record_p094_followup_seed_created(&tail_class);
}

pub async fn upsert_generation_and_rebuild_tx(
    tx: &mut Transaction<'_, Sqlite>,
    input: ActiveArtifactGenerationInput,
) -> Result<()> {
    let read_roots = artifact_read_roots_for_run_tx(tx, input.run_id).await?;
    record_p094_followup_seed_metric_if_needed(&input, &read_roots);
    let raw_status = effective_raw_status_for_generation(
        &read_roots,
        &input.contract_id,
        &input.raw_path,
        &input.raw_status,
    )?;
    let normalized =
        normalize_contract_status(&input.contract_id, &raw_status).map_err(anyhow::Error::msg)?;
    let mut warnings = input.warnings.clone();
    warnings.extend(normalized.warnings.clone());
    let canonical_dimensions_json =
        canonical_dimensions_json_for_generation(&read_roots, &input.contract_id, &input.raw_path)?;
    let supersedes_generation_id = effective_supersedes_generation_id_tx(
        tx,
        input.run_id,
        &input.contract_id,
        &input.generation_id,
        input.supersedes_generation_id.clone(),
        normalized.valid,
    )
    .await?;
    let now = Utc::now().to_rfc3339();
    let run_id = input.run_id.to_string();
    let artifact_id = input.artifact_id.to_string();
    let warnings_json = serde_json::to_string(&warnings)?;
    let validation_errors_json = serde_json::to_string(&normalized.validation_errors)?;

    sqlx::query(
        r#"INSERT OR REPLACE INTO artifact_contract_generations
           (generation_id, run_id, artifact_id, contract_id, canonical_path, raw_path, raw_status,
            canonical_status, source_agent_execution_id, source_stage_execution_id,
            source_session_generation_id, source_work_item_id, supersedes_generation_id,
            output_settlement, source_generation_verified, valid, partial, warnings_json,
            validation_errors_json, canonical_dimensions_json, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)"#,
    )
    .bind(&input.generation_id)
    .bind(&run_id)
    .bind(&artifact_id)
    .bind(&input.contract_id)
    .bind(&input.canonical_path)
    .bind(&input.raw_path)
    .bind(&raw_status)
    .bind(&normalized.canonical_status)
    .bind(&input.source_agent_execution_id)
    .bind(&input.source_stage_execution_id)
    .bind(&input.source_session_generation_id)
    .bind(&input.source_work_item_id)
    .bind(&supersedes_generation_id)
    .bind(input.output_settlement.to_string())
    .bind(0_i64)
    .bind(if normalized.valid { 1_i64 } else { 0_i64 })
    .bind(if input.partial { 1_i64 } else { 0_i64 })
    .bind(&warnings_json)
    .bind(&validation_errors_json)
    .bind(&canonical_dimensions_json)
    .bind(&now)
    .execute(&mut **tx)
    .await?;

    if normalized.valid {
        sqlx::query(
            r#"INSERT INTO active_artifact_contracts (run_id, contract_id, generation_id, updated_at)
               VALUES (?1, ?2, ?3, ?4)
               ON CONFLICT(run_id, contract_id) DO UPDATE SET
                 generation_id = excluded.generation_id,
                 updated_at = excluded.updated_at"#,
        )
        .bind(&run_id)
        .bind(&input.contract_id)
        .bind(&input.generation_id)
        .bind(&now)
        .execute(&mut **tx)
        .await?;
    }
    rebuild_run_state_projection_tx(tx, input.run_id).await
}

pub async fn repair_contract_status_normalization_and_rebuild(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<u64> {
    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "artifact_contracts.repair_contract_status_normalization",
    )
    .await?;
    let repaired = repair_contract_status_normalization_tx(&mut tx, run_id).await?;
    rebuild_run_state_projection_tx(&mut tx, run_id).await?;
    tx.commit().await?;
    export_projection_files(pool, run_id).await?;
    Ok(repaired)
}

pub async fn repair_contract_status_normalization_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
) -> Result<u64> {
    let run_id_str = run_id.to_string();
    let read_roots = artifact_read_roots_for_run_tx(tx, run_id).await?;
    let rows = sqlx::query(
        r#"SELECT generation_id, contract_id, raw_path, raw_status, warnings_json,
                  source_generation_verified
           FROM artifact_contract_generations
           WHERE run_id = ?1 AND valid = 0"#,
    )
    .bind(&run_id_str)
    .fetch_all(&mut **tx)
    .await?;

    let mut repaired = 0_u64;
    for row in rows {
        let generation_id: String = row.get("generation_id");
        let contract_id: String = row.get("contract_id");
        let raw_path: String = row.get("raw_path");
        let raw_status: String = row.get("raw_status");
        let normalized =
            normalize_contract_status(&contract_id, &raw_status).map_err(anyhow::Error::msg)?;
        if !normalized.valid {
            continue;
        }

        let mut warnings: Vec<String> =
            serde_json::from_str(&row.get::<String, _>("warnings_json"))?;
        for warning in normalized.warnings {
            if !warnings.contains(&warning) {
                warnings.push(warning);
            }
        }
        let warnings_json = serde_json::to_string(&warnings)?;
        let validation_errors_json = serde_json::to_string(&normalized.validation_errors)?;
        let canonical_dimensions_json =
            canonical_dimensions_json_for_generation(&read_roots, &contract_id, &raw_path)?;
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"UPDATE artifact_contract_generations
               SET canonical_status = ?1,
                   valid = 1,
                   warnings_json = ?2,
                   validation_errors_json = ?3,
                   canonical_dimensions_json = ?4
               WHERE generation_id = ?5"#,
        )
        .bind(&normalized.canonical_status)
        .bind(&warnings_json)
        .bind(&validation_errors_json)
        .bind(&canonical_dimensions_json)
        .bind(&generation_id)
        .execute(&mut **tx)
        .await?;

        if row.get::<i64, _>("source_generation_verified") != 0 {
            sqlx::query(
                r#"INSERT INTO active_artifact_contracts (run_id, contract_id, generation_id, updated_at)
                   VALUES (?1, ?2, ?3, ?4)
                   ON CONFLICT(run_id, contract_id) DO UPDATE SET
                     generation_id = excluded.generation_id,
                     updated_at = excluded.updated_at"#,
            )
            .bind(&run_id_str)
            .bind(&contract_id)
            .bind(&generation_id)
            .bind(&now)
            .execute(&mut **tx)
            .await?;
        }
        repaired += 1;
    }

    Ok(repaired)
}

pub async fn record_run_state_advisory_tx(
    tx: &mut Transaction<'_, Sqlite>,
    input: ActiveArtifactGenerationInput,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let warnings_json = serde_json::to_string(&input.warnings)?;
    sqlx::query(
        r#"INSERT OR REPLACE INTO artifact_contract_advisories
           (advisory_id, run_id, artifact_id, contract_id, advisory_path, advisory_kind,
            superseded_by, source_agent_execution_id, source_stage_execution_id,
            source_session_generation_id, source_work_item_id, warnings_json, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"#,
    )
    .bind(&input.generation_id)
    .bind(input.run_id.to_string())
    .bind(input.artifact_id.to_string())
    .bind(&input.contract_id)
    .bind(&input.raw_path)
    .bind("agent_authored_run_state_superseded")
    .bind("sqlite_run_state_projection")
    .bind(&input.source_agent_execution_id)
    .bind(&input.source_stage_execution_id)
    .bind(&input.source_session_generation_id)
    .bind(&input.source_work_item_id)
    .bind(&warnings_json)
    .bind(&now)
    .execute(&mut **tx)
    .await?;
    rebuild_run_state_projection_tx(tx, input.run_id).await
}

pub async fn insert_source_generation_claim(
    pool: &SqlitePool,
    claim: ArtifactSourceGenerationClaim,
) -> Result<()> {
    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "artifact_contracts.insert_source_generation_claim",
    )
    .await?;
    insert_source_generation_claim_tx(&mut tx, claim).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn insert_source_generation_claim_tx(
    tx: &mut Transaction<'_, Sqlite>,
    claim: ArtifactSourceGenerationClaim,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO artifact_source_generation_claims
           (run_id, owner_kind, owner_id, stage_execution_id, agent_execution_id, source_work_item_id,
            current_session_generation_id, claim_state, superseding_work_item_id,
            superseded_by_agent_execution_id, supersession_journal_id, superseded_at,
            closed_at, created_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)"#,
    )
    .bind(claim.key.run_id.to_string())
    .bind(claim.key.owner_kind.to_string())
    .bind(&claim.key.owner_id)
    .bind(claim.key.stage_execution_id.map(|id| id.to_string()))
    .bind(claim.key.agent_execution_id.to_string())
    .bind(&claim.key.source_work_item_id)
    .bind(&claim.current_session_generation_id)
    .bind(claim.claim_state.to_string())
    .bind(&claim.superseding_work_item_id)
    .bind(&claim.superseded_by_agent_execution_id)
    .bind(&claim.supersession_journal_id)
    .bind(claim.superseded_at.map(|dt| dt.to_rfc3339()))
    .bind(claim.closed_at.map(|dt| dt.to_rfc3339()))
    .bind(claim.created_at.to_rfc3339())
    .bind(claim.updated_at.to_rfc3339())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn load_source_generation_claim(
    pool: &SqlitePool,
    key: &ArtifactSourceGenerationClaimKey,
) -> Result<Option<ArtifactSourceGenerationClaim>> {
    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "artifact_contracts.load_source_generation_claim",
    )
    .await?;
    let claim = load_source_generation_claim_tx(&mut tx, key).await?;
    tx.commit().await?;
    Ok(claim)
}

pub async fn load_source_generation_claim_tx(
    tx: &mut Transaction<'_, Sqlite>,
    key: &ArtifactSourceGenerationClaimKey,
) -> Result<Option<ArtifactSourceGenerationClaim>> {
    let row = sqlx::query(
        r#"SELECT run_id, owner_kind, owner_id, stage_execution_id, agent_execution_id, source_work_item_id,
                  current_session_generation_id, claim_state, superseding_work_item_id,
                  superseded_by_agent_execution_id, supersession_journal_id, superseded_at,
                  closed_at, created_at, updated_at
           FROM artifact_source_generation_claims
           WHERE run_id = ?1 AND owner_kind = ?2 AND owner_id = ?3 AND agent_execution_id = ?4
             AND source_work_item_id = ?5"#,
    )
    .bind(key.run_id.to_string())
    .bind(key.owner_kind.to_string())
    .bind(&key.owner_id)
    .bind(key.agent_execution_id.to_string())
    .bind(&key.source_work_item_id)
    .fetch_optional(&mut **tx)
    .await?;
    row.map(|row| parse_source_generation_claim_row(&row))
        .transpose()
}

pub async fn mark_claim_superseded_pending_retry(
    pool: &SqlitePool,
    key: &ArtifactSourceGenerationClaimKey,
    superseding_work_item_id: &str,
    supersession_journal_id: &str,
) -> Result<()> {
    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "artifact_contracts.mark_claim_superseded_pending_retry",
    )
    .await?;
    mark_claim_superseded_pending_retry_tx(
        &mut tx,
        key,
        superseding_work_item_id,
        supersession_journal_id,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn mark_claim_superseded_pending_retry_tx(
    tx: &mut Transaction<'_, Sqlite>,
    key: &ArtifactSourceGenerationClaimKey,
    superseding_work_item_id: &str,
    supersession_journal_id: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"UPDATE artifact_source_generation_claims
           SET claim_state = ?1,
               superseding_work_item_id = ?2,
               supersession_journal_id = ?3,
               superseded_at = ?4,
               updated_at = ?4
           WHERE run_id = ?5 AND owner_kind = ?6 AND owner_id = ?7 AND agent_execution_id = ?8
             AND source_work_item_id = ?9 AND claim_state = ?10"#,
    )
    .bind(ArtifactSourceClaimState::SupersededPendingRetry.to_string())
    .bind(superseding_work_item_id)
    .bind(supersession_journal_id)
    .bind(&now)
    .bind(key.run_id.to_string())
    .bind(key.owner_kind.to_string())
    .bind(&key.owner_id)
    .bind(key.agent_execution_id.to_string())
    .bind(&key.source_work_item_id)
    .bind(ArtifactSourceClaimState::Active.to_string())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn mark_active_claims_superseded_pending_retry_for_stage_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
    stage_execution_id: &str,
    superseding_work_item_id: &str,
    supersession_journal_id: &str,
) -> Result<u64> {
    mark_active_claims_superseded_pending_retry_for_owner_tx(
        tx,
        run_id,
        OwnerKind::StageExecution,
        stage_execution_id,
        superseding_work_item_id,
        supersession_journal_id,
    )
    .await
}

pub async fn mark_active_claims_superseded_pending_retry_for_owner_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
    owner_kind: OwnerKind,
    owner_id: &str,
    superseding_work_item_id: &str,
    supersession_journal_id: &str,
) -> Result<u64> {
    let now = Utc::now().to_rfc3339();
    let result = sqlx::query(
        r#"UPDATE artifact_source_generation_claims
           SET claim_state = ?1,
               superseding_work_item_id = ?2,
               supersession_journal_id = ?3,
               superseded_at = ?4,
               updated_at = ?4
           WHERE run_id = ?5 AND owner_kind = ?6 AND owner_id = ?7 AND claim_state = ?8"#,
    )
    .bind(ArtifactSourceClaimState::SupersededPendingRetry.to_string())
    .bind(superseding_work_item_id)
    .bind(supersession_journal_id)
    .bind(&now)
    .bind(run_id.to_string())
    .bind(owner_kind.to_string())
    .bind(owner_id)
    .bind(ArtifactSourceClaimState::Active.to_string())
    .execute(&mut **tx)
    .await?;
    Ok(result.rows_affected())
}

pub async fn finalize_pending_retry_supersession_tx(
    tx: &mut Transaction<'_, Sqlite>,
    superseding_work_item_id: &str,
    new_agent_execution_id: AgentExecutionId,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"UPDATE artifact_source_generation_claims
           SET claim_state = ?1,
               superseded_by_agent_execution_id = ?2,
               updated_at = ?3
           WHERE superseding_work_item_id = ?4 AND claim_state = ?5"#,
    )
    .bind(ArtifactSourceClaimState::Superseded.to_string())
    .bind(new_agent_execution_id.to_string())
    .bind(&now)
    .bind(superseding_work_item_id)
    .bind(ArtifactSourceClaimState::SupersededPendingRetry.to_string())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn finalize_pending_retry_supersession_for_work_item(
    pool: &SqlitePool,
    superseding_work_item_id: &str,
    new_agent_execution_id: AgentExecutionId,
) -> Result<()> {
    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "artifact_contracts.finalize_pending_retry_supersession_for_work_item",
    )
    .await?;
    finalize_pending_retry_supersession_tx(
        &mut tx,
        superseding_work_item_id,
        new_agent_execution_id,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn close_source_generation_claim(
    pool: &SqlitePool,
    key: &ArtifactSourceGenerationClaimKey,
) -> Result<()> {
    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "artifact_contracts.close_source_generation_claim",
    )
    .await?;
    close_source_generation_claim_tx(&mut tx, key).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn close_source_generation_claim_tx(
    tx: &mut Transaction<'_, Sqlite>,
    key: &ArtifactSourceGenerationClaimKey,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"UPDATE artifact_source_generation_claims
           SET claim_state = ?1, closed_at = ?2, updated_at = ?2
           WHERE run_id = ?3 AND owner_kind = ?4 AND owner_id = ?5 AND agent_execution_id = ?6
             AND source_work_item_id = ?7 AND claim_state = ?8"#,
    )
    .bind(ArtifactSourceClaimState::Closed.to_string())
    .bind(&now)
    .bind(key.run_id.to_string())
    .bind(key.owner_kind.to_string())
    .bind(&key.owner_id)
    .bind(key.agent_execution_id.to_string())
    .bind(&key.source_work_item_id)
    .bind(ArtifactSourceClaimState::Active.to_string())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn update_source_generation_claim_session(
    pool: &SqlitePool,
    key: &ArtifactSourceGenerationClaimKey,
    current_session_generation_id: Option<&str>,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    crate::execute_repository_write!(
        pool,
        "artifact_contracts.update_source_generation_claim_session",
        sqlx::query(
            r#"UPDATE artifact_source_generation_claims
           SET current_session_generation_id = ?1, updated_at = ?2
           WHERE run_id = ?3 AND owner_kind = ?4 AND owner_id = ?5 AND agent_execution_id = ?6
             AND source_work_item_id = ?7 AND claim_state = ?8"#,
        )
        .bind(current_session_generation_id)
        .bind(&now)
        .bind(key.run_id.to_string())
        .bind(key.owner_kind.to_string())
        .bind(&key.owner_id)
        .bind(key.agent_execution_id.to_string())
        .bind(&key.source_work_item_id)
        .bind(ArtifactSourceClaimState::Active.to_string())
    )?;
    Ok(())
}

pub async fn import_generation_with_claim_cas(
    pool: &SqlitePool,
    key: &ArtifactSourceGenerationClaimKey,
    source_session_generation_id: &str,
    input: ActiveArtifactGenerationInput,
) -> Result<SourceGenerationImportDecision> {
    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "artifact_contracts.import_generation_with_claim_cas",
    )
    .await?;
    let decision =
        import_generation_with_claim_cas_tx(&mut tx, key, source_session_generation_id, input)
            .await?;
    tx.commit().await?;
    export_projection_files(pool, key.run_id).await?;
    Ok(decision)
}

pub async fn import_generation_with_claim_cas_tx(
    tx: &mut Transaction<'_, Sqlite>,
    key: &ArtifactSourceGenerationClaimKey,
    source_session_generation_id: &str,
    input: ActiveArtifactGenerationInput,
) -> Result<SourceGenerationImportDecision> {
    let read_roots = artifact_read_roots_for_run_tx(tx, input.run_id).await?;
    let raw_status = effective_raw_status_for_generation(
        &read_roots,
        &input.contract_id,
        &input.raw_path,
        &input.raw_status,
    )?;
    let normalized =
        normalize_contract_status(&input.contract_id, &raw_status).map_err(anyhow::Error::msg)?;
    let mut warnings = input.warnings.clone();
    warnings.extend(normalized.warnings.clone());
    let warnings_json = serde_json::to_string(&warnings)?;
    let validation_errors_json = serde_json::to_string(&normalized.validation_errors)?;
    let now = Utc::now().to_rfc3339();
    let supersedes_generation_id = effective_supersedes_generation_id_tx(
        tx,
        key.run_id,
        &input.contract_id,
        &input.generation_id,
        input.supersedes_generation_id.clone(),
        normalized.valid,
    )
    .await?;
    let mut generation_input = input;
    generation_input.raw_status = raw_status;
    generation_input.supersedes_generation_id = supersedes_generation_id;

    let claim = sqlx::query(
        r#"SELECT claim_state, current_session_generation_id
           FROM artifact_source_generation_claims
           WHERE run_id = ?1 AND owner_kind = ?2 AND owner_id = ?3 AND agent_execution_id = ?4
             AND source_work_item_id = ?5"#,
    )
    .bind(key.run_id.to_string())
    .bind(key.owner_kind.to_string())
    .bind(&key.owner_id)
    .bind(key.agent_execution_id.to_string())
    .bind(&key.source_work_item_id)
    .fetch_optional(&mut **tx)
    .await?;

    let active_claim = claim.as_ref().is_some_and(|row| {
        row.get::<String, _>("claim_state") == ArtifactSourceClaimState::Active.to_string()
            && row
                .get::<Option<String>, _>("current_session_generation_id")
                .as_deref()
                == Some(source_session_generation_id)
    });
    if !active_claim {
        insert_generation_in_tx(
            tx,
            &generation_input,
            "invalid",
            false,
            false,
            &warnings_json,
            &validation_errors_json,
            &now,
            AgentOutputSettlement::IgnoredLateOutputs,
        )
        .await?;
        rebuild_run_state_projection_tx(tx, key.run_id).await?;
        return Ok(SourceGenerationImportDecision::IgnoredLateOutputs);
    }

    insert_generation_in_tx(
        tx,
        &generation_input,
        &normalized.canonical_status,
        normalized.valid,
        true,
        &warnings_json,
        &validation_errors_json,
        &now,
        generation_input.output_settlement.clone(),
    )
    .await?;
    if normalized.valid {
        sqlx::query(
            r#"INSERT INTO active_artifact_contracts (run_id, contract_id, generation_id, updated_at)
               VALUES (?1, ?2, ?3, ?4)
               ON CONFLICT(run_id, contract_id) DO UPDATE SET
                 generation_id = excluded.generation_id,
                 updated_at = excluded.updated_at"#,
        )
        .bind(key.run_id.to_string())
        .bind(&generation_input.contract_id)
        .bind(&generation_input.generation_id)
        .bind(&now)
        .execute(&mut **tx)
        .await?;
    }
    rebuild_run_state_projection_tx(tx, key.run_id).await?;
    Ok(if normalized.valid {
        SourceGenerationImportDecision::Activated
    } else {
        SourceGenerationImportDecision::InvalidRejected
    })
}

async fn insert_generation_in_tx(
    tx: &mut Transaction<'_, Sqlite>,
    input: &ActiveArtifactGenerationInput,
    canonical_status: &str,
    valid: bool,
    source_generation_verified: bool,
    warnings_json: &str,
    validation_errors_json: &str,
    now: &str,
    output_settlement: AgentOutputSettlement,
) -> Result<()> {
    let read_roots = artifact_read_roots_for_run_tx(tx, input.run_id).await?;
    let canonical_dimensions_json =
        canonical_dimensions_json_for_generation(&read_roots, &input.contract_id, &input.raw_path)?;
    sqlx::query(
        r#"INSERT OR REPLACE INTO artifact_contract_generations
           (generation_id, run_id, artifact_id, contract_id, canonical_path, raw_path, raw_status,
            canonical_status, source_agent_execution_id, source_stage_execution_id,
            source_session_generation_id, source_work_item_id, supersedes_generation_id,
            output_settlement, source_generation_verified, valid, partial, warnings_json,
            validation_errors_json, canonical_dimensions_json, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)"#,
    )
    .bind(&input.generation_id)
    .bind(input.run_id.to_string())
    .bind(input.artifact_id.to_string())
    .bind(&input.contract_id)
    .bind(&input.canonical_path)
    .bind(&input.raw_path)
    .bind(&input.raw_status)
    .bind(canonical_status)
    .bind(&input.source_agent_execution_id)
    .bind(&input.source_stage_execution_id)
    .bind(&input.source_session_generation_id)
    .bind(&input.source_work_item_id)
    .bind(&input.supersedes_generation_id)
    .bind(output_settlement.to_string())
    .bind(if source_generation_verified { 1_i64 } else { 0_i64 })
    .bind(if valid { 1_i64 } else { 0_i64 })
    .bind(if input.partial { 1_i64 } else { 0_i64 })
    .bind(warnings_json)
    .bind(validation_errors_json)
    .bind(&canonical_dimensions_json)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn create_override_and_rebuild(
    pool: &SqlitePool,
    input: ArtifactContractOverrideInput,
) -> Result<String> {
    let normalized = validate_override_input(&input)?;
    let override_id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let run_id = input.run_id.to_string();
    let source_artifacts_json = serde_json::to_string(&input.source_artifacts)?;
    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "artifact_contracts.create_override_and_rebuild",
    )
    .await?;
    sqlx::query(
        r#"INSERT INTO artifact_contract_overrides
           (override_id, run_id, contract_id, override_type, from_status, to_status, reason, owner,
            source_artifacts_json, expires_at_stage, journal_id, created_at, active)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 1)"#,
    )
    .bind(&override_id)
    .bind(&run_id)
    .bind(&input.contract_id)
    .bind(&input.override_type)
    .bind(&normalized.from_status)
    .bind(&normalized.to_status)
    .bind(&input.reason)
    .bind(&input.owner)
    .bind(&source_artifacts_json)
    .bind(&input.expires_at_stage)
    .bind(&input.journal_id)
    .bind(&now)
    .execute(&mut **tx)
    .await?;
    rebuild_run_state_projection_tx(&mut tx, input.run_id).await?;
    tx.commit().await?;
    export_projection_files(pool, input.run_id).await?;
    Ok(override_id)
}

pub async fn expire_overrides_for_stage(
    pool: &SqlitePool,
    run_id: RunId,
    stage_id: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let active_override_ids: Vec<String> = sqlx::query_scalar(
        "SELECT override_id FROM artifact_contract_overrides WHERE run_id = ?1 AND expires_at_stage = ?2 AND active = 1 ORDER BY created_at ASC",
    )
    .bind(run_id.to_string())
    .bind(stage_id)
    .fetch_all(pool)
    .await?;
    let expiry_journal_id = if active_override_ids.is_empty() {
        None
    } else {
        let journal_id = uuid::Uuid::new_v4().to_string();
        let payload = serde_json::json!({
            "run_id": run_id.to_string(),
            "stage_id": stage_id,
            "expired_override_ids": active_override_ids,
        });
        crate::repos::command_journal::record(
            pool,
            &journal_id,
            "ExpireArtifactContractOverrides",
            &serde_json::to_string(&payload)?,
            Some(&run_id.to_string()),
            Utc::now(),
            Some("engine"),
            Some("system"),
            Some("system"),
            Some("artifact_contract_override_expiry"),
            None,
            None,
            None,
            None,
        )
        .await?;
        Some(journal_id)
    };
    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "artifact_contracts.expire_overrides_for_stage",
    )
    .await?;
    let result: Result<()> = async {
        sqlx::query(
            "UPDATE artifact_contract_overrides SET active = 0, expired_at = ?1 WHERE run_id = ?2 AND expires_at_stage = ?3 AND active = 1",
        )
        .bind(&now)
        .bind(run_id.to_string())
        .bind(stage_id)
        .execute(&mut **tx)
        .await?;
        rebuild_run_state_projection_tx(&mut tx, run_id).await?;
        tx.commit().await?;
        Ok(())
    }
    .await;
    match result {
        Ok(()) => {
            if let Some(journal_id) = expiry_journal_id.as_deref() {
                crate::repos::command_journal::complete_entry(pool, journal_id, Utc::now()).await?;
            }
            export_projection_files(pool, run_id).await
        }
        Err(error) => {
            if let Some(journal_id) = expiry_journal_id.as_deref() {
                let _ = crate::repos::command_journal::fail_entry(
                    pool,
                    journal_id,
                    Utc::now(),
                    &error.to_string(),
                )
                .await;
            }
            Err(error)
        }
    }
}

pub async fn list_overrides(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<Vec<ArtifactContractOverride>> {
    let rows = sqlx::query(
        r#"SELECT override_id, run_id, contract_id, override_type, from_status, to_status,
                  reason, owner, source_artifacts_json, expires_at_stage, journal_id,
                  created_at, expired_at, active
           FROM artifact_contract_overrides WHERE run_id = ?1 ORDER BY created_at ASC"#,
    )
    .bind(run_id.to_string())
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(parse_override).collect()
}

pub async fn find_run_state_projection(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<Option<RunStateProjectionRow>> {
    let row = sqlx::query(
        r#"SELECT run_id, active_index_json, run_state_json, exported_active_index_path,
                  exported_run_state_path, updated_at
           FROM run_state_projections WHERE run_id = ?1"#,
    )
    .bind(run_id.to_string())
    .fetch_optional(pool)
    .await?;
    row.map(|r| {
        let run_id_str: String = r.get("run_id");
        let updated_at: String = r.get("updated_at");
        Ok(RunStateProjectionRow {
            run_id: run_id_str.parse::<uuid::Uuid>()?.into(),
            active_index_json: serde_json::from_str::<serde_json::Value>(
                &r.get::<String, _>("active_index_json"),
            )?,
            run_state_json: serde_json::from_str::<serde_json::Value>(
                &r.get::<String, _>("run_state_json"),
            )?,
            exported_active_index_path: r.get("exported_active_index_path"),
            exported_run_state_path: r.get("exported_run_state_path"),
            updated_at: DateTime::parse_from_rfc3339(&updated_at)?.with_timezone(&Utc),
        })
    })
    .transpose()
}

pub async fn p094_readback_json(pool: &SqlitePool, run_id: RunId) -> Result<serde_json::Value> {
    let proposal_decomposition_plan =
        p094_active_contract_readback_json(pool, run_id, "proposal_decomposition_plan_v1").await?;
    let quality_gate_blocker_assessment =
        p094_active_contract_readback_json(pool, run_id, "quality_gate_blocker_assessment_v1")
            .await?;
    let blocker_boundary_status =
        p094_active_contract_readback_json(pool, run_id, "blocker_boundary_status_v1").await?;
    let blocker_boundary_approval_request =
        p094_active_contract_readback_json(pool, run_id, "blocker_boundary_approval_request_v1")
            .await?;
    let blocker_boundary_human_decision =
        p094_active_contract_readback_json(pool, run_id, "blocker_boundary_human_decision_v1")
            .await?;
    let followup_proposal_seed =
        p094_active_contract_readback_json(pool, run_id, "followup_proposal_seed_v1").await?;
    Ok(serde_json::json!({
        "schema_version": "p094_boundary_readback_v1",
        "proposal_decomposition_plan": proposal_decomposition_plan,
        "quality_gate_blocker_assessment": quality_gate_blocker_assessment,
        "blocker_boundary_status": blocker_boundary_status,
        "blocker_boundary_approval_request": blocker_boundary_approval_request,
        "blocker_boundary_human_decision": blocker_boundary_human_decision,
        "followup_proposal_seeds": match followup_proposal_seed {
            serde_json::Value::Null => serde_json::json!([]),
            value => serde_json::json!([value]),
        },
    }))
}

async fn p094_active_contract_readback_json(
    pool: &SqlitePool,
    run_id: RunId,
    contract_id: &str,
) -> Result<serde_json::Value> {
    let Some(row) = find_active_generation_by_contract_id(pool, run_id, contract_id).await? else {
        return Ok(serde_json::Value::Null);
    };
    let roots = artifact_read_roots_for_run(pool, run_id).await?;
    let payload = read_confined_artifact_bytes(&roots, &row.raw_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .unwrap_or(serde_json::Value::Null);
    let mut object = match payload {
        serde_json::Value::Object(object) => object,
        value => {
            let mut object = serde_json::Map::new();
            object.insert("payload".to_string(), value);
            object
        }
    };
    object.insert(
        "generation_id".to_string(),
        serde_json::json!(row.generation_id),
    );
    object.insert(
        "contract_id".to_string(),
        serde_json::json!(row.contract_id),
    );
    object.insert(
        "canonical_path".to_string(),
        serde_json::json!(row.canonical_path),
    );
    object.insert(
        "source_generation_verified".to_string(),
        serde_json::json!(row.source_generation_verified),
    );
    object.insert(
        "active_index_summary".to_string(),
        serde_json::json!({
            "status": row.canonical_status,
            "output_settlement": row.output_settlement,
            "canonical_dimensions": row.canonical_dimensions_json,
            "created_at": row.created_at.to_rfc3339(),
        }),
    );
    Ok(serde_json::Value::Object(object))
}

pub async fn canonical_contract_field(
    pool: &SqlitePool,
    run_id: RunId,
    artifact_alias: &str,
    field_name: &str,
) -> Result<Option<serde_json::Value>> {
    Ok(
        match canonical_contract_field_result(pool, run_id, artifact_alias, field_name).await? {
            CanonicalContractField::Resolved(value) => Some(value),
            CanonicalContractField::MissingControlled { .. }
            | CanonicalContractField::UncontrolledAlias => None,
        },
    )
}

pub async fn active_contract_exists_result(
    pool: &SqlitePool,
    run_id: RunId,
    artifact_alias: &str,
) -> Result<CanonicalContractField> {
    let Some(contract_id) = contract_id_for_alias(artifact_alias) else {
        return Ok(CanonicalContractField::UncontrolledAlias);
    };
    let exists = sqlx::query_scalar::<_, i64>(
        r#"SELECT EXISTS(
               SELECT 1 FROM active_artifact_contracts
               WHERE run_id = ?1 AND contract_id = ?2
           )"#,
    )
    .bind(run_id.to_string())
    .bind(contract_id)
    .fetch_one(pool)
    .await?;
    if exists == 0 {
        Ok(CanonicalContractField::MissingControlled {
            contract_id: contract_id.to_string(),
        })
    } else {
        Ok(CanonicalContractField::Resolved(serde_json::Value::Bool(
            true,
        )))
    }
}

pub async fn find_active_generation_by_contract_id(
    pool: &SqlitePool,
    run_id: RunId,
    contract_id: &str,
) -> Result<Option<ActiveArtifactContractGenerationRow>> {
    let row = sqlx::query(
        r#"SELECT g.generation_id, g.contract_id, g.canonical_path, g.raw_path, g.raw_status,
                  g.canonical_status, g.output_settlement, g.source_generation_verified,
                  g.canonical_dimensions_json, g.created_at
           FROM active_artifact_contracts a
           JOIN artifact_contract_generations g ON g.generation_id = a.generation_id
           WHERE a.run_id = ?1 AND a.contract_id = ?2"#,
    )
    .bind(run_id.to_string())
    .bind(contract_id)
    .fetch_optional(pool)
    .await?;
    row.map(parse_active_generation_row).transpose()
}

/// P077: Read a canonical field from closeout_gate_generations for
/// proposal_gate_result_v1 and implementation_closeout_readiness_v1.
/// Returns MissingControlled when no active generation exists for the run.
async fn p077_canonical_field_result(
    pool: &SqlitePool,
    run_id: RunId,
    contract_id: &str,
    field_name: &str,
) -> Result<CanonicalContractField> {
    let row = sqlx::query(
        r#"SELECT status, decision FROM closeout_gate_generations
           WHERE run_id = ?1 AND contract_id = ?2 AND active = 1
           ORDER BY created_at DESC LIMIT 1"#,
    )
    .bind(run_id.to_string())
    .bind(contract_id)
    .fetch_optional(pool)
    .await
    .context("p077_canonical_field_result: query closeout_gate_generations")?;

    let Some(row) = row else {
        return Ok(CanonicalContractField::MissingControlled {
            contract_id: contract_id.to_string(),
        });
    };

    let value = match (contract_id, field_name) {
        (_, "status") => {
            let status: String = row.get("status");
            CanonicalContractField::Resolved(serde_json::json!(status))
        }
        (cid, "decision") if cid == IMPLEMENTATION_CLOSEOUT_READINESS_V1_CONTRACT_ID => {
            let decision: Option<String> = row.get("decision");
            match decision {
                Some(d) => CanonicalContractField::Resolved(serde_json::json!(d)),
                None => CanonicalContractField::MissingControlled {
                    contract_id: contract_id.to_string(),
                },
            }
        }
        _ => CanonicalContractField::MissingControlled {
            contract_id: contract_id.to_string(),
        },
    };
    Ok(value)
}

pub async fn canonical_contract_field_result(
    pool: &SqlitePool,
    run_id: RunId,
    artifact_alias: &str,
    field_name: &str,
) -> Result<CanonicalContractField> {
    let Some(contract_id) = contract_id_for_alias(artifact_alias) else {
        return Ok(CanonicalContractField::UncontrolledAlias);
    };
    if let Some(value) = active_override_field(pool, run_id, contract_id, field_name).await? {
        return Ok(CanonicalContractField::Resolved(value));
    }

    // P077: closeout_gate_generations is the authoritative store for
    // proposal_gate_result_v1 and implementation_closeout_readiness_v1.
    // These are never inserted into active_artifact_contracts.
    if contract_id == PROPOSAL_GATE_RESULT_V1_CONTRACT_ID
        || contract_id == IMPLEMENTATION_CLOSEOUT_READINESS_V1_CONTRACT_ID
    {
        return p077_canonical_field_result(pool, run_id, contract_id, field_name).await;
    }

    let row = sqlx::query(
        r#"SELECT g.contract_id, g.canonical_status, g.canonical_dimensions_json
           FROM active_artifact_contracts a
           JOIN artifact_contract_generations g ON g.generation_id = a.generation_id
           WHERE a.run_id = ?1 AND a.contract_id = ?2"#,
    )
    .bind(run_id.to_string())
    .bind(contract_id)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        return Ok(CanonicalContractField::MissingControlled {
            contract_id: contract_id.to_string(),
        });
    };
    let active_contract_id: String = row.get("contract_id");
    let status: String = row.get("canonical_status");
    let dimensions: serde_json::Value =
        serde_json::from_str(&row.get::<String, _>("canonical_dimensions_json"))?;
    Ok(match field_name {
        "status" | "implementation_status" => {
            CanonicalContractField::Resolved(serde_json::json!(status))
        }
        "requires_split" if active_contract_id == "proposal_decomposition_plan_v1" => {
            bool_dimension_field(&dimensions, contract_id, field_name)
        }
        "implementation_start_decision"
            if active_contract_id == "proposal_decomposition_plan_v1" =>
        {
            string_dimension_field(&dimensions, contract_id, field_name)
        }
        "followup_proposal_required"
        | "has_release_blocking_external_blockers"
        | "has_no_release_blocking_external_blockers"
            if active_contract_id == "blocker_boundary_status_v1" =>
        {
            bool_dimension_field(&dimensions, contract_id, field_name)
        }
        "projection_integrity" if active_contract_id == "blocker_boundary_status_v1" => {
            string_dimension_field(&dimensions, contract_id, field_name)
        }
        "primary_owner_class" | "workflow_route_hint"
            if active_contract_id == "blocker_boundary_status_v1" =>
        {
            string_dimension_field(&dimensions, contract_id, field_name)
        }
        "blocker_freshness" | "allowed_workflow_routes"
            if active_contract_id == "blocker_boundary_status_v1" =>
        {
            json_dimension_field(&dimensions, contract_id, field_name)
        }
        "release_evidence_status" if active_contract_id == "audit_report_v1" => {
            match dimensions
                .get("release_evidence_status")
                .and_then(|value| value.as_str())
            {
                Some(value) => CanonicalContractField::Resolved(serde_json::json!(value)),
                None => CanonicalContractField::MissingControlled {
                    contract_id: contract_id.to_string(),
                },
            }
        }
        "implementation_complete" | "verification_green"
            if active_contract_id == "implementation_self_assessment_v2" =>
        {
            match dimensions.get(field_name).and_then(|value| value.as_bool()) {
                Some(value) => CanonicalContractField::Resolved(serde_json::json!(value)),
                None => CanonicalContractField::MissingControlled {
                    contract_id: contract_id.to_string(),
                },
            }
        }
        "blocking_remaining_code_tasks"
        | "handoff_task_count"
        | "blocking_review_handoff_task_count"
            if active_contract_id == "implementation_self_assessment_v2" =>
        {
            match dimensions.get(field_name).and_then(|value| value.as_u64()) {
                Some(value) => CanonicalContractField::Resolved(serde_json::json!(value)),
                None => CanonicalContractField::MissingControlled {
                    contract_id: contract_id.to_string(),
                },
            }
        }
        _ => CanonicalContractField::MissingControlled {
            contract_id: contract_id.to_string(),
        },
    })
}

fn bool_dimension_field(
    dimensions: &serde_json::Value,
    contract_id: &str,
    field_name: &str,
) -> CanonicalContractField {
    match dimensions.get(field_name).and_then(|value| value.as_bool()) {
        Some(value) => CanonicalContractField::Resolved(serde_json::json!(value)),
        None => CanonicalContractField::MissingControlled {
            contract_id: contract_id.to_string(),
        },
    }
}

fn string_dimension_field(
    dimensions: &serde_json::Value,
    contract_id: &str,
    field_name: &str,
) -> CanonicalContractField {
    match dimensions.get(field_name).and_then(|value| value.as_str()) {
        Some(value) => CanonicalContractField::Resolved(serde_json::json!(value)),
        None => CanonicalContractField::MissingControlled {
            contract_id: contract_id.to_string(),
        },
    }
}

fn json_dimension_field(
    dimensions: &serde_json::Value,
    contract_id: &str,
    field_name: &str,
) -> CanonicalContractField {
    match dimensions.get(field_name) {
        Some(value) => CanonicalContractField::Resolved(value.clone()),
        None => CanonicalContractField::MissingControlled {
            contract_id: contract_id.to_string(),
        },
    }
}

pub async fn rebuild_run_state_projection(pool: &SqlitePool, run_id: RunId) -> Result<()> {
    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "artifact_contracts.rebuild_run_state_projection",
    )
    .await?;
    rebuild_run_state_projection_tx(&mut tx, run_id).await?;
    tx.commit().await?;
    export_projection_files(pool, run_id).await
}

struct NormalizedOverrideInput {
    from_status: String,
    to_status: String,
}

fn validate_override_input(
    input: &ArtifactContractOverrideInput,
) -> Result<NormalizedOverrideInput> {
    if !known_contract_id(&input.contract_id) {
        anyhow::bail!(
            "unknown artifact contract override target: {}",
            input.contract_id
        );
    }
    let field_contract_id = match (input.contract_id.as_str(), input.override_type.as_str()) {
        (_, "status") => input.contract_id.as_str(),
        ("audit_report_v1", "implementation_status") => "audit_report_v1",
        _ => anyhow::bail!(
            "unsupported artifact contract override type '{}' for {}",
            input.override_type,
            input.contract_id
        ),
    };
    let from = normalize_contract_status(field_contract_id, &input.from_status)
        .map_err(anyhow::Error::msg)?;
    if !from.valid {
        anyhow::bail!(
            "invalid override from_status '{}' for {}: {}",
            input.from_status,
            input.contract_id,
            from.validation_errors.join("; ")
        );
    }
    let to = normalize_contract_status(field_contract_id, &input.to_status)
        .map_err(anyhow::Error::msg)?;
    if !to.valid {
        anyhow::bail!(
            "invalid override to_status '{}' for {}: {}",
            input.to_status,
            input.contract_id,
            to.validation_errors.join("; ")
        );
    }
    Ok(NormalizedOverrideInput {
        from_status: from.canonical_status,
        to_status: to.canonical_status,
    })
}

async fn effective_supersedes_generation_id_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
    contract_id: &str,
    new_generation_id: &str,
    explicit_supersedes_generation_id: Option<String>,
    valid: bool,
) -> Result<Option<String>> {
    if explicit_supersedes_generation_id.is_some() || !valid {
        return Ok(explicit_supersedes_generation_id);
    }
    let previous: Option<String> = sqlx::query_scalar(
        "SELECT generation_id FROM active_artifact_contracts WHERE run_id = ?1 AND contract_id = ?2",
    )
    .bind(run_id.to_string())
    .bind(contract_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(previous.filter(|generation_id| generation_id != new_generation_id))
}

fn canonical_dimensions_json_for_generation(
    roots: &ArtifactReadRoots,
    contract_id: &str,
    raw_path: &str,
) -> Result<String> {
    let mut dimensions = serde_json::Map::new();
    if contract_id == "audit_report_v1" {
        if let Some(raw_status) = audit_release_evidence_status_from_path(roots, raw_path)? {
            dimensions.insert(
                "release_evidence_status".to_string(),
                serde_json::json!(normalize_audit_release_evidence_status(&raw_status)?),
            );
        }
    } else if contract_id == "implementation_self_assessment_v2" {
        if let Some(implementation) =
            implementation_self_assessment_summary_from_path(roots, raw_path)?
        {
            dimensions.insert(
                "implementation_complete".to_string(),
                serde_json::json!(implementation.implementation_complete),
            );
            dimensions.insert(
                "verification_green".to_string(),
                serde_json::json!(implementation.verification_green),
            );
            dimensions.insert(
                "blocking_remaining_code_tasks".to_string(),
                serde_json::json!(implementation.blocking_remaining_code_task_count),
            );
            dimensions.insert(
                "handoff_task_count".to_string(),
                serde_json::json!(implementation.handoff_task_count),
            );
            dimensions.insert(
                "blocking_review_handoff_task_count".to_string(),
                serde_json::json!(implementation.blocking_review_handoff_task_count),
            );
        }
    } else if contract_id == "proposal_decomposition_plan_v1" {
        if let Some(value) = p094_json_from_path(roots, raw_path)? {
            let valid_blocking_split_candidate_count =
                p094_valid_blocking_split_candidate_count(&value);
            if let Some(requires_split) = value
                .get("requires_split")
                .and_then(|value| value.as_bool())
            {
                if !requires_split || valid_blocking_split_candidate_count > 0 {
                    dimensions.insert(
                        "requires_split".to_string(),
                        serde_json::json!(requires_split),
                    );
                } else {
                    dimensions.insert(
                        "split_candidate_validation".to_string(),
                        serde_json::json!("requires_split_without_blocking_split_candidate"),
                    );
                }
            }
            if let Some(split_candidates) = value
                .get("split_candidates")
                .and_then(|value| value.as_array())
            {
                dimensions.insert(
                    "split_candidate_count".to_string(),
                    serde_json::json!(split_candidates.len() as u64),
                );
                dimensions.insert(
                    "blocking_split_candidate_count".to_string(),
                    serde_json::json!(valid_blocking_split_candidate_count as u64),
                );
            }
            if let Some(decision) = value
                .get("implementation_start_decision")
                .and_then(|value| value.as_str())
            {
                let requires_split = value
                    .get("requires_split")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false);
                if !requires_split || valid_blocking_split_candidate_count > 0 {
                    dimensions.insert(
                        "implementation_start_decision".to_string(),
                        serde_json::json!(decision),
                    );
                }
            }
        }
    } else if contract_id == "blocker_boundary_status_v1" {
        if let Some(value) = p094_json_from_path(roots, raw_path)? {
            for field_name in [
                "followup_proposal_required",
                "has_release_blocking_external_blockers",
                "has_no_release_blocking_external_blockers",
            ] {
                if let Some(field_value) = value.get(field_name).and_then(|value| value.as_bool()) {
                    dimensions.insert(field_name.to_string(), serde_json::json!(field_value));
                }
            }
            if let Some(projection_integrity) = value
                .get("projection_integrity")
                .and_then(|value| value.as_str())
            {
                dimensions.insert(
                    "projection_integrity".to_string(),
                    serde_json::json!(projection_integrity),
                );
            }
            if let Some(primary_owner_class) = value
                .get("primary_owner_class")
                .and_then(|value| value.as_str())
            {
                dimensions.insert(
                    "primary_owner_class".to_string(),
                    serde_json::json!(primary_owner_class),
                );
            }
            if let Some(workflow_route_hint) = value
                .get("workflow_route_hint")
                .and_then(|value| value.as_str())
            {
                dimensions.insert(
                    "workflow_route_hint".to_string(),
                    serde_json::json!(workflow_route_hint),
                );
            }
            if let Some(blocker_freshness) = value
                .get("blocker_freshness")
                .and_then(|value| value.as_str())
            {
                dimensions.insert(
                    "blocker_freshness".to_string(),
                    serde_json::json!(blocker_freshness),
                );
            }
            let blocker_freshness_details = value
                .get("blockers")
                .and_then(|value| value.as_array())
                .map(|blockers| {
                    blockers
                        .iter()
                        .filter_map(|blocker| {
                            Some(serde_json::json!({
                                "id": blocker.get("id")?.as_str()?,
                                "owner_class": blocker.get("owner_class")?.as_str()?,
                                "evidence_freshness": blocker
                                    .get("evidence_freshness")
                                    .or_else(|| blocker.get("freshness"))?
                                    .as_str()?,
                                "allowed_workflow_routes": blocker
                                    .get("allowed_workflow_routes")
                                    .cloned()
                                    .unwrap_or_else(|| serde_json::json!([])),
                            }))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            dimensions.insert(
                "blocker_freshness_details".to_string(),
                serde_json::json!(blocker_freshness_details),
            );
            if let Some(allowed_workflow_routes) = value.get("allowed_workflow_routes") {
                dimensions.insert(
                    "allowed_workflow_routes".to_string(),
                    allowed_workflow_routes.clone(),
                );
            }
        }
    }
    Ok(serde_json::to_string(&serde_json::Value::Object(
        dimensions,
    ))?)
}

fn effective_raw_status_for_generation(
    roots: &ArtifactReadRoots,
    contract_id: &str,
    raw_path: &str,
    fallback: &str,
) -> Result<String> {
    if contract_id == IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID {
        if let Some(summary) = implementation_self_assessment_summary_from_path(roots, raw_path)? {
            return Ok(summary.status.to_string());
        }
    } else if contract_id == "proposal_decomposition_plan_v1" {
        if let Some(value) = p094_json_from_path(roots, raw_path)? {
            if value
                .get("requires_split")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
            {
                if p094_valid_blocking_split_candidate_count(&value) > 0 {
                    return Ok("split_required".to_string());
                }
                return Ok("invalid".to_string());
            }
            if let Some(decision) = value
                .get("implementation_start_decision")
                .and_then(|value| value.as_str())
            {
                return Ok(decision.to_string());
            }
        }
    } else if contract_id == "blocker_boundary_status_v1" {
        if let Some(value) = p094_json_from_path(roots, raw_path)? {
            if let Some(status) = value.get("status").and_then(|value| value.as_str()) {
                return Ok(status.to_string());
            }
        }
    } else if contract_id == "blocker_boundary_human_decision_v1" {
        if let Some(value) = p094_json_from_path(roots, raw_path)? {
            if let Some(state) = value
                .get("canonical_approval_state")
                .and_then(|value| value.as_str())
            {
                return Ok(state.to_string());
            }
            if let Some(label) = value.get("decision_label").and_then(|value| value.as_str()) {
                return Ok(label.to_string());
            }
        }
    }
    Ok(fallback.to_string())
}

fn p094_valid_blocking_split_candidate_count(value: &serde_json::Value) -> usize {
    value
        .get("split_candidates")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter(|item| {
                    item.get("candidate_id")
                        .and_then(|value| value.as_str())
                        .is_some_and(|value| !value.trim().is_empty())
                        && item
                            .get("reason")
                            .and_then(|value| value.as_str())
                            .is_some_and(|value| !value.trim().is_empty())
                })
                .count()
        })
        .unwrap_or(0)
}

async fn artifact_read_roots_for_run(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<ArtifactReadRoots> {
    let row = sqlx::query("SELECT artifact_root, chainworks_meta_root FROM runs WHERE id = ?1")
        .bind(run_id.to_string())
        .fetch_optional(pool)
        .await?
        .context("run not found for confined artifact read")?;
    Ok(ArtifactReadRoots {
        run_id,
        chainworks_meta_root: row
            .get::<Option<String>, _>("chainworks_meta_root")
            .map(PathBuf::from),
        artifact_root: PathBuf::from(row.get::<String, _>("artifact_root")),
    })
}

async fn artifact_read_roots_for_run_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
) -> Result<ArtifactReadRoots> {
    let row = sqlx::query("SELECT artifact_root, chainworks_meta_root FROM runs WHERE id = ?1")
        .bind(run_id.to_string())
        .fetch_optional(&mut **tx)
        .await?
        .context("run not found for confined artifact read")?;
    Ok(ArtifactReadRoots {
        run_id,
        chainworks_meta_root: row
            .get::<Option<String>, _>("chainworks_meta_root")
            .map(PathBuf::from),
        artifact_root: PathBuf::from(row.get::<String, _>("artifact_root")),
    })
}

fn read_confined_artifact_bytes(roots: &ArtifactReadRoots, raw_path: &str) -> Result<Vec<u8>> {
    let Some((root, candidate)) = confined_artifact_candidate(roots, raw_path) else {
        anyhow::bail!(
            "artifact raw_path is outside run artifact roots for run {}",
            roots.run_id
        );
    };
    let root_meta = std::fs::symlink_metadata(&root)
        .with_context(|| format!("stat artifact root {}", root.display()))?;
    if !root_meta.is_dir() || root_meta.file_type().is_symlink() {
        anyhow::bail!("artifact root must be a real directory: {}", root.display());
    }
    let canonical_root = std::fs::canonicalize(&root)
        .with_context(|| format!("canonicalize artifact root {}", root.display()))?;
    if !candidate.starts_with(&root) {
        anyhow::bail!("artifact raw_path does not lexically remain inside artifact root");
    }
    let canonical_candidate = std::fs::canonicalize(&candidate)
        .with_context(|| format!("canonicalize artifact {}", candidate.display()))?;
    if !canonical_candidate.starts_with(&canonical_root) {
        anyhow::bail!("artifact raw_path canonical target escapes artifact root");
    }
    reject_symlink_components(&root, &candidate)?;

    let metadata = std::fs::symlink_metadata(&candidate)
        .with_context(|| format!("stat artifact {}", candidate.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        anyhow::bail!("artifact raw_path must be a regular non-symlink file");
    }
    if metadata.len() > ARTIFACT_CONTRACT_READBACK_MAX_BYTES {
        anyhow::bail!(
            "artifact raw_path exceeds readback cap: {} > {}",
            metadata.len(),
            ARTIFACT_CONTRACT_READBACK_MAX_BYTES
        );
    }
    let file = open_file_no_follow(&candidate)
        .with_context(|| format!("open artifact {}", candidate.display()))?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(ARTIFACT_CONTRACT_READBACK_MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > ARTIFACT_CONTRACT_READBACK_MAX_BYTES {
        anyhow::bail!(
            "artifact raw_path exceeds readback cap while reading: {}",
            candidate.display()
        );
    }
    Ok(bytes)
}

fn confined_artifact_candidate(
    roots: &ArtifactReadRoots,
    raw_path: &str,
) -> Option<(PathBuf, PathBuf)> {
    let raw = Path::new(raw_path);
    let candidate_roots = roots
        .chainworks_meta_root
        .iter()
        .chain(std::iter::once(&roots.artifact_root));
    for root in candidate_roots {
        let candidate = if raw.is_absolute() {
            raw.to_path_buf()
        } else {
            root.join(raw)
        };
        if candidate.starts_with(root) {
            return Some((root.clone(), candidate));
        }
    }
    None
}

fn reject_symlink_components(root: &Path, candidate: &Path) -> Result<()> {
    let relative = candidate
        .strip_prefix(root)
        .context("artifact path is not below canonical root")?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        let metadata = std::fs::symlink_metadata(&current)
            .with_context(|| format!("stat artifact path component {}", current.display()))?;
        if metadata.file_type().is_symlink() {
            anyhow::bail!(
                "artifact raw_path contains a symlink component: {}",
                current.display()
            );
        }
    }
    Ok(())
}

#[cfg(unix)]
fn open_file_no_follow(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
}

#[cfg(not(unix))]
fn open_file_no_follow(path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::File::open(path)
}

fn p094_json_from_path(roots: &ArtifactReadRoots, path: &str) -> Result<Option<serde_json::Value>> {
    let bytes = match read_confined_artifact_bytes(roots, path) {
        Ok(bytes) => bytes,
        Err(_) => return Ok(None),
    };
    Ok(Some(serde_json::from_slice(&bytes)?))
}

fn implementation_self_assessment_summary_from_path(
    roots: &ArtifactReadRoots,
    path: &str,
) -> Result<Option<ImplementationSelfAssessmentSummary>> {
    let bytes = match read_confined_artifact_bytes(roots, path) {
        Ok(bytes) => bytes,
        Err(_) => return Ok(None),
    };
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    Ok(Some(parse_implementation_self_assessment_v2(
        &value,
        ContractParseContext {
            declared_contract_id: Some(IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID.into()),
            canonical_artifact_path: IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH.into(),
            raw_artifact_path: Some(path.to_string()),
            ..ContractParseContext::default()
        },
    )))
}

fn normalize_audit_release_evidence_status(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    let normalized = match trimmed {
        "ready" | "Ready" => "ready",
        "blocked_pending_operator_evidence" => "blocked_pending_operator_evidence",
        "not_ready" | "Not Ready" => "not_ready",
        "not_applicable" | "N/A" => "not_applicable",
        "unknown" => "unknown",
        other => anyhow::bail!("unknown audit release_evidence_status value: {other}"),
    };
    Ok(normalized.to_string())
}

fn audit_release_evidence_status_from_path(
    roots: &ArtifactReadRoots,
    path: &str,
) -> Result<Option<String>> {
    let bytes = match read_confined_artifact_bytes(roots, path) {
        Ok(bytes) => bytes,
        Err(_) => return Ok(None),
    };
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    Ok(value
        .get("release_evidence_status")
        .and_then(|value| value.as_str())
        .map(str::to_string))
}

pub async fn rebuild_run_state_projection_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
) -> Result<()> {
    let run_id_str = run_id.to_string();
    let run_row =
        sqlx::query("SELECT status, current_state, chainworks_meta_root FROM runs WHERE id = ?1")
            .bind(&run_id_str)
            .fetch_optional(&mut **tx)
            .await?
            .context("run not found for run-state projection")?;
    let run_status: String = run_row.get("status");
    let current_state: Option<String> = run_row.get("current_state");
    let chainworks_meta_root: Option<String> = run_row.get("chainworks_meta_root");
    let active_rows = sqlx::query(
        r#"SELECT g.contract_id, g.canonical_path, g.raw_path, g.raw_status, g.canonical_status,
                  g.generation_id, g.source_agent_execution_id, g.source_stage_execution_id,
                  g.source_session_generation_id, g.source_work_item_id,
                  g.supersedes_generation_id, g.output_settlement,
                  g.source_generation_verified, g.valid, g.partial, g.warnings_json,
                  g.validation_errors_json, g.canonical_dimensions_json, g.created_at
           FROM active_artifact_contracts a
           JOIN artifact_contract_generations g ON g.generation_id = a.generation_id
           WHERE a.run_id = ?1 ORDER BY g.contract_id"#,
    )
    .bind(&run_id_str)
    .fetch_all(&mut **tx)
    .await?;

    // P077: Also fetch active closeout_gate_generations rows (proposal_gate_result_v1 and
    // implementation_closeout_readiness_v1). These are never inserted into
    // active_artifact_contracts but must appear in the exported projection so that
    // downstream consumers (run-state JSON, GraphQL, MCP runs.get) see P077 truth.
    let p077_rows = sqlx::query(
        r#"SELECT contract_id, status, decision, generation_id, fingerprint_json, created_at
           FROM closeout_gate_generations
           WHERE run_id = ?1 AND active = 1
           ORDER BY contract_id"#,
    )
    .bind(&run_id_str)
    .fetch_all(&mut **tx)
    .await?;

    let mut contracts = serde_json::Map::new();

    for p077_row in p077_rows {
        let contract_id: String = p077_row.get("contract_id");
        let status: String = p077_row.get("status");
        let decision: Option<String> = p077_row.get("decision");
        let generation_id: String = p077_row.get("generation_id");
        let fingerprint_json: Option<String> = p077_row.get("fingerprint_json");
        let created_at: String = p077_row.get("created_at");
        let fingerprint_hash: Option<String> = fingerprint_json
            .as_deref()
            .and_then(|j| serde_json::from_str::<CloseoutFingerprint>(j).ok())
            .map(|fp| fp.short_hash());
        contracts.insert(
            contract_id.clone(),
            serde_json::json!({
                "canonical_path": serde_json::Value::Null,
                "raw_path": serde_json::Value::Null,
                "raw_status": status,
                "status": status,
                "db_status": status,
                "decision": decision,
                "fingerprint_hash": fingerprint_hash,
                "status_overridden": false,
                "active_override_id": serde_json::Value::Null,
                "generation_id": generation_id,
                "artifact_generation_id": generation_id,
                "source_agent_execution_id": serde_json::Value::Null,
                "source_stage_execution_id": serde_json::Value::Null,
                "source_session_generation_id": serde_json::Value::Null,
                "source_work_item_id": serde_json::Value::Null,
                "supersedes_artifact_generation_id": serde_json::Value::Null,
                "supersedes": [],
                "output_settlement": "none",
                "source_generation_verified": false,
                "valid": true,
                "partial": false,
                "warnings": [],
                "validation_errors": [],
                "created_at": created_at,
                "p077": true,
            }),
        );
    }

    for row in active_rows {
        let contract_id: String = row.get("contract_id");
        let warnings: Vec<String> = serde_json::from_str(&row.get::<String, _>("warnings_json"))?;
        let validation_errors: Vec<String> =
            serde_json::from_str(&row.get::<String, _>("validation_errors_json"))?;
        let active_override =
            active_override_for_contract_in_tx(tx, &run_id_str, &contract_id).await?;
        let base_status: String = row.get("canonical_status");
        let raw_path: String = row.get("raw_path");
        let dimensions: serde_json::Value =
            serde_json::from_str(&row.get::<String, _>("canonical_dimensions_json"))?;
        let release_evidence_status = dimensions
            .get("release_evidence_status")
            .and_then(|value| value.as_str())
            .map(str::to_string);
        let effective_status = active_override
            .as_ref()
            .map(|ov| ov.to_status.clone())
            .unwrap_or_else(|| base_status.clone());
        let mut effective_warnings = warnings.clone();
        if let Some(ov) = &active_override {
            effective_warnings.push(format!(
                "operator override {} changed status from {} to {} until {}",
                ov.override_id, ov.from_status, ov.to_status, ov.expires_at_stage
            ));
        }
        let mut contract_json = serde_json::json!({
                "canonical_path": row.get::<String, _>("canonical_path"),
                "raw_path": raw_path,
                "raw_status": row.get::<String, _>("raw_status"),
                "status": effective_status,
                "implementation_status": if contract_id == "audit_report_v1" { serde_json::json!(effective_status.clone()) } else { serde_json::Value::Null },
                "release_evidence_status": release_evidence_status,
                "db_status": base_status,
                "status_overridden": active_override.is_some(),
                "active_override_id": active_override.as_ref().map(|ov| ov.override_id.clone()),
                "generation_id": row.get::<String, _>("generation_id"),
                "artifact_generation_id": row.get::<String, _>("generation_id"),
                "source_agent_execution_id": row.get::<Option<String>, _>("source_agent_execution_id"),
                "source_stage_execution_id": row.get::<Option<String>, _>("source_stage_execution_id"),
                "source_session_generation_id": row.get::<Option<String>, _>("source_session_generation_id"),
                "source_work_item_id": row.get::<Option<String>, _>("source_work_item_id"),
                "supersedes_artifact_generation_id": row.get::<Option<String>, _>("supersedes_generation_id"),
                "supersedes": row.get::<Option<String>, _>("supersedes_generation_id").map(|id| vec![id]).unwrap_or_default(),
                "output_settlement": row.get::<String, _>("output_settlement"),
                "source_generation_verified": row.get::<i64, _>("source_generation_verified") != 0,
                "valid": row.get::<i64, _>("valid") != 0,
                "partial": row.get::<i64, _>("partial") != 0,
                "warnings": effective_warnings,
                "validation_errors": validation_errors,
                "created_at": row.get::<String, _>("created_at"),
        });
        if let Some(contract_object) = contract_json.as_object_mut() {
            if let Some(dimensions_object) = dimensions.as_object() {
                for (key, value) in dimensions_object {
                    contract_object.insert(key.clone(), value.clone());
                }
            }
            contract_object.insert("canonical_dimensions".to_string(), dimensions);
        }
        contracts.insert(contract_id.clone(), contract_json);
    }

    let overrides = list_overrides_in_tx(tx, &run_id_str).await?;
    for ov in overrides.iter().filter(|ov| ov.active) {
        if !contracts.contains_key(&ov.contract_id) {
            contracts.insert(
                ov.contract_id.clone(),
                serde_json::json!({
                    "canonical_path": serde_json::Value::Null,
                    "raw_path": serde_json::Value::Null,
                    "raw_status": serde_json::Value::Null,
                    "status": ov.to_status,
                    "db_status": ov.from_status,
                    "status_overridden": true,
                    "active_override_id": ov.override_id,
                    "generation_id": serde_json::Value::Null,
                    "artifact_generation_id": serde_json::Value::Null,
                    "source_agent_execution_id": serde_json::Value::Null,
                    "source_stage_execution_id": serde_json::Value::Null,
                    "source_session_generation_id": serde_json::Value::Null,
                    "source_work_item_id": serde_json::Value::Null,
                    "supersedes_artifact_generation_id": serde_json::Value::Null,
                    "output_settlement": "none",
                    "source_generation_verified": false,
                    "valid": true,
                    "partial": false,
                    "warnings": [
                        format!(
                            "operator override {} supplies canonical transition truth without mutating raw artifacts",
                            ov.override_id
                        )
                    ],
                    "validation_errors": [],
                    "created_at": ov.created_at.to_rfc3339(),
                }),
            );
        }
    }
    let invalid_required_artifacts = latest_invalid_generations_in_tx(tx, &run_id_str).await?;
    let advisory_artifacts = list_advisories_in_tx(tx, &run_id_str).await?;
    let override_json = serde_json::to_value(&overrides)?;
    let projection_warnings =
        projection_warnings(&contracts, &invalid_required_artifacts, &advisory_artifacts);
    let (exported_active_index_path, exported_run_state_path) =
        projection_export_paths(chainworks_meta_root.as_deref());
    let active_index = serde_json::json!({
        "schema_version": "active-artifact-index.v1",
        "run_id": run_id.to_string(),
        "generated_at": Utc::now().to_rfc3339(),
        "owner": "sqlite",
        "contracts": contracts,
        "invalid_required_artifacts": invalid_required_artifacts,
        "advisory_artifacts": advisory_artifacts,
        "operator_overrides": override_json,
        "warnings": projection_warnings,
        "exported_path": exported_active_index_path,
    });
    let run_state = serde_json::json!({
        "schema_version": "run-state-projection.v1",
        "run_id": run_id.to_string(),
        "status": run_status,
        "current_state": current_state,
        "active_index_owner": "sqlite",
        "active_artifacts": active_index["contracts"].clone(),
        "operator_overrides": active_index["operator_overrides"].clone(),
        "invalid_required_artifacts": active_index["invalid_required_artifacts"].clone(),
        "advisory_artifacts": active_index["advisory_artifacts"].clone(),
        "warnings": active_index["warnings"].clone(),
        "exported_path": exported_run_state_path,
    });
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"INSERT INTO run_state_projections
           (run_id, active_index_json, run_state_json, exported_active_index_path, exported_run_state_path, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6)
           ON CONFLICT(run_id) DO UPDATE SET
             active_index_json = excluded.active_index_json,
             run_state_json = excluded.run_state_json,
             exported_active_index_path = excluded.exported_active_index_path,
             exported_run_state_path = excluded.exported_run_state_path,
             updated_at = excluded.updated_at"#,
    )
    .bind(run_id.to_string())
    .bind(serde_json::to_string(&active_index)?)
    .bind(serde_json::to_string(&run_state)?)
    .bind(exported_active_index_path.as_deref())
    .bind(exported_run_state_path.as_deref())
    .bind(&now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn export_projection_files(pool: &SqlitePool, run_id: RunId) -> Result<()> {
    let Some(row) = find_run_state_projection(pool, run_id).await? else {
        return Ok(());
    };
    if let Some(path) = &row.exported_active_index_path {
        atomic_write_json(Path::new(path), &row.active_index_json)?;
    }
    if let Some(path) = &row.exported_run_state_path {
        atomic_write_json(Path::new(path), &row.run_state_json)?;
    }
    Ok(())
}

pub async fn rebuild_projection_and_exports(pool: &SqlitePool, run_id: RunId) -> Result<()> {
    rebuild_run_state_projection(pool, run_id).await
}

async fn active_override_field(
    pool: &SqlitePool,
    run_id: RunId,
    contract_id: &str,
    field_name: &str,
) -> Result<Option<serde_json::Value>> {
    let row = sqlx::query(
        r#"SELECT to_status FROM artifact_contract_overrides
           WHERE run_id = ?1 AND contract_id = ?2 AND active = 1
             AND (override_type = ?3 OR (?3 = 'status' AND override_type = 'implementation_status'))
           ORDER BY created_at DESC LIMIT 1"#,
    )
    .bind(run_id.to_string())
    .bind(contract_id)
    .bind(field_name)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| serde_json::json!(r.get::<String, _>("to_status"))))
}

async fn active_override_for_contract_in_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
    contract_id: &str,
) -> Result<Option<ArtifactContractOverride>> {
    let row = sqlx::query(
        r#"SELECT override_id, run_id, contract_id, override_type, from_status, to_status,
                  reason, owner, source_artifacts_json, expires_at_stage, journal_id,
                  created_at, expired_at, active
           FROM artifact_contract_overrides
           WHERE run_id = ?1 AND contract_id = ?2 AND active = 1
           ORDER BY created_at DESC LIMIT 1"#,
    )
    .bind(run_id)
    .bind(contract_id)
    .fetch_optional(&mut **tx)
    .await?;
    row.map(parse_override).transpose()
}

async fn list_overrides_in_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
) -> Result<Vec<ArtifactContractOverride>> {
    let rows = sqlx::query(
        r#"SELECT override_id, run_id, contract_id, override_type, from_status, to_status,
                  reason, owner, source_artifacts_json, expires_at_stage, journal_id,
                  created_at, expired_at, active
           FROM artifact_contract_overrides WHERE run_id = ?1 ORDER BY created_at ASC"#,
    )
    .bind(run_id)
    .fetch_all(&mut **tx)
    .await?;
    rows.into_iter().map(parse_override).collect()
}

async fn latest_invalid_generations_in_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
) -> Result<Vec<serde_json::Value>> {
    let rows = sqlx::query(
        r#"SELECT g.contract_id, g.canonical_path, g.raw_path, g.raw_status,
                  g.validation_errors_json, g.output_settlement, g.created_at
           FROM artifact_contract_generations g
           WHERE g.run_id = ?1
             AND g.valid = 0
             AND NOT EXISTS (
               SELECT 1
               FROM active_artifact_contracts a
               JOIN artifact_contract_generations active_g
                 ON active_g.generation_id = a.generation_id
               WHERE a.run_id = g.run_id
                 AND a.contract_id = g.contract_id
                 AND active_g.valid = 1
                 AND (
                   active_g.created_at >= g.created_at
                   OR active_g.supersedes_generation_id = g.generation_id
                 )
             )
           ORDER BY g.created_at DESC"#,
    )
    .bind(run_id)
    .fetch_all(&mut **tx)
    .await?;
    rows.into_iter()
        .map(|row| {
            let validation_errors: Vec<String> =
                serde_json::from_str(&row.get::<String, _>("validation_errors_json"))?;
            Ok(serde_json::json!({
                "blocked": true,
                "reason": "invalid_required_artifact",
                "contract": row.get::<String, _>("contract_id"),
                "path": row.get::<String, _>("canonical_path"),
                "raw_path": row.get::<String, _>("raw_path"),
                "raw_status": row.get::<String, _>("raw_status"),
                "output_settlement": row.get::<String, _>("output_settlement"),
                "validation_errors": validation_errors,
                "created_at": row.get::<String, _>("created_at"),
            }))
        })
        .collect()
}

async fn list_advisories_in_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
) -> Result<Vec<serde_json::Value>> {
    let rows = sqlx::query(
        r#"SELECT advisory_id, contract_id, advisory_path, advisory_kind, superseded_by,
                  source_agent_execution_id, source_stage_execution_id,
                  source_session_generation_id, source_work_item_id, warnings_json, created_at
           FROM artifact_contract_advisories
           WHERE run_id = ?1 ORDER BY created_at ASC"#,
    )
    .bind(run_id)
    .fetch_all(&mut **tx)
    .await?;
    rows.into_iter()
        .map(|row| {
            let warnings: Vec<String> =
                serde_json::from_str(&row.get::<String, _>("warnings_json"))?;
            Ok(serde_json::json!({
                "advisory_id": row.get::<String, _>("advisory_id"),
                "contract_id": row.get::<String, _>("contract_id"),
                "advisory_path": row.get::<String, _>("advisory_path"),
                "advisory_kind": row.get::<String, _>("advisory_kind"),
                "superseded_by": row.get::<String, _>("superseded_by"),
                "source_agent_execution_id": row.get::<Option<String>, _>("source_agent_execution_id"),
                "source_stage_execution_id": row.get::<Option<String>, _>("source_stage_execution_id"),
                "source_session_generation_id": row.get::<Option<String>, _>("source_session_generation_id"),
                "source_work_item_id": row.get::<Option<String>, _>("source_work_item_id"),
                "warnings": warnings,
                "created_at": row.get::<String, _>("created_at"),
            }))
        })
        .collect()
}

fn projection_warnings(
    contracts: &serde_json::Map<String, serde_json::Value>,
    invalid_required_artifacts: &[serde_json::Value],
    advisory_artifacts: &[serde_json::Value],
) -> Vec<String> {
    let mut warnings = Vec::new();
    if !invalid_required_artifacts.is_empty() {
        warnings.push(
            "invalid_required_artifact evidence present; transition truth remains fail-closed"
                .to_string(),
        );
    }
    for (contract_id, value) in contracts {
        if value.get("partial").and_then(|v| v.as_bool()) == Some(true) {
            warnings.push(format!(
                "partial output warning: {contract_id} came from a partial execution"
            ));
        }
        if value.get("status_overridden").and_then(|v| v.as_bool()) == Some(true) {
            warnings.push(format!(
                "operator override active for {contract_id}; raw artifacts remain advisory"
            ));
        }
    }
    for advisory in advisory_artifacts {
        if advisory.get("contract_id").and_then(|value| value.as_str())
            == Some("run_state_projection_v1")
        {
            warnings.push(
                "agent-authored state/run-state.json was imported as advisory and superseded by sqlite projection"
                    .to_string(),
            );
        }
    }
    warnings
}

fn projection_export_paths(meta_root: Option<&str>) -> (Option<String>, Option<String>) {
    let Some(meta_root) = meta_root else {
        return (None, None);
    };
    let root = PathBuf::from(meta_root);
    (
        Some(
            root.join("artifacts")
                .join("active-index.json")
                .to_string_lossy()
                .into_owned(),
        ),
        Some(
            root.join("state")
                .join("run-state.json")
                .to_string_lossy()
                .into_owned(),
        ),
    )
}

fn atomic_write_json(path: &Path, value: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp_path = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("json")
    ));
    std::fs::write(&tmp_path, serde_json::to_vec_pretty(value)?)?;
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

fn parse_active_generation_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<ActiveArtifactContractGenerationRow> {
    let created_at: String = row.get("created_at");
    Ok(ActiveArtifactContractGenerationRow {
        generation_id: row.get("generation_id"),
        contract_id: row.get("contract_id"),
        canonical_path: row.get("canonical_path"),
        raw_path: row.get("raw_path"),
        raw_status: row.get("raw_status"),
        canonical_status: row.get("canonical_status"),
        output_settlement: row.get("output_settlement"),
        source_generation_verified: row.get::<i64, _>("source_generation_verified") != 0,
        canonical_dimensions_json: serde_json::from_str(
            &row.get::<String, _>("canonical_dimensions_json"),
        )?,
        created_at: DateTime::parse_from_rfc3339(&created_at)?.with_timezone(&Utc),
    })
}

fn parse_override(row: sqlx::sqlite::SqliteRow) -> Result<ArtifactContractOverride> {
    let run_id: String = row.get("run_id");
    let created_at: String = row.get("created_at");
    let expired_at: Option<String> = row.get("expired_at");
    Ok(ArtifactContractOverride {
        override_id: row.get("override_id"),
        run_id: run_id.parse::<uuid::Uuid>()?.into(),
        contract_id: row.get("contract_id"),
        override_type: row.get("override_type"),
        from_status: row.get("from_status"),
        to_status: row.get("to_status"),
        reason: row.get("reason"),
        owner: row.get("owner"),
        source_artifacts: serde_json::from_str(&row.get::<String, _>("source_artifacts_json"))?,
        expires_at_stage: row.get("expires_at_stage"),
        journal_id: row.get("journal_id"),
        created_at: DateTime::parse_from_rfc3339(&created_at)?.with_timezone(&Utc),
        expired_at: expired_at
            .map(|value| DateTime::parse_from_rfc3339(&value).map(|dt| dt.with_timezone(&Utc)))
            .transpose()?,
        active: row.get::<i64, _>("active") != 0,
    })
}

fn parse_source_generation_claim_row(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<ArtifactSourceGenerationClaim> {
    let run_id: String = row.get("run_id");
    let owner_kind: String = row.get("owner_kind");
    let stage_execution_id: Option<String> = row.get("stage_execution_id");
    let agent_execution_id: String = row.get("agent_execution_id");
    let claim_state: String = row.get("claim_state");
    let superseded_at: Option<String> = row.get("superseded_at");
    let closed_at: Option<String> = row.get("closed_at");
    let created_at: String = row.get("created_at");
    let updated_at: String = row.get("updated_at");
    Ok(ArtifactSourceGenerationClaim {
        key: ArtifactSourceGenerationClaimKey {
            run_id: run_id.parse::<uuid::Uuid>()?.into(),
            owner_kind: owner_kind.parse().map_err(anyhow::Error::msg)?,
            owner_id: row.get("owner_id"),
            stage_execution_id: stage_execution_id
                .map(|value| value.parse::<uuid::Uuid>().map(Into::into))
                .transpose()?,
            agent_execution_id: agent_execution_id.parse::<uuid::Uuid>()?.into(),
            source_work_item_id: row.get("source_work_item_id"),
        },
        current_session_generation_id: row.get("current_session_generation_id"),
        claim_state: claim_state
            .parse::<ArtifactSourceClaimState>()
            .map_err(anyhow::Error::msg)?,
        superseding_work_item_id: row.get("superseding_work_item_id"),
        superseded_by_agent_execution_id: row.get("superseded_by_agent_execution_id"),
        supersession_journal_id: row.get("supersession_journal_id"),
        superseded_at: superseded_at
            .map(|value| DateTime::parse_from_rfc3339(&value).map(|dt| dt.with_timezone(&Utc)))
            .transpose()?,
        closed_at: closed_at
            .map(|value| DateTime::parse_from_rfc3339(&value).map(|dt| dt.with_timezone(&Utc)))
            .transpose()?,
        created_at: DateTime::parse_from_rfc3339(&created_at)?.with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&updated_at)?.with_timezone(&Utc),
    })
}

pub fn contract_id_for_alias(alias: &str) -> Option<&'static str> {
    match alias {
        "prepush_review_report" => Some("prepush_review_v1"),
        "docs_report" => Some("docs_report_v1"),
        "audit_report" => Some("audit_report_v1"),
        "security_report" => Some("security_report_v1"),
        "implementation_review_summary" => Some("implementation_review_summary_v1"),
        "implementation_self_assessment_v2" => Some("implementation_self_assessment_v2"),
        "tests_result_v1" => Some("tests_result_v1"),
        // P077: closeout readiness artifacts stored in closeout_gate_generations.
        "implementation_closeout_readiness_v1" => Some("implementation_closeout_readiness_v1"),
        "proposal_gate_result_v1" => Some("proposal_gate_result_v1"),
        "proposal_decomposition_plan" => Some("proposal_decomposition_plan_v1"),
        "quality_gate_blocker_assessment" => Some("quality_gate_blocker_assessment_v1"),
        "blocker_boundary_status" => Some("blocker_boundary_status_v1"),
        "blocker_boundary_approval_request" => Some("blocker_boundary_approval_request_v1"),
        "blocker_boundary_human_decision" => Some("blocker_boundary_human_decision_v1"),
        "followup_proposal_seed" => Some("followup_proposal_seed_v1"),
        _ => None,
    }
}

include!("implementation_self_assessment_contract.rs");
