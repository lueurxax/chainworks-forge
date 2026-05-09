use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};

use domain::ids::{ArtifactId, RunId, StageExecutionId};
use domain::validation::{ValidationFailureClass, ValidationFailureRecord};

pub async fn insert(pool: &SqlitePool, record: &ValidationFailureRecord) -> Result<()> {
    crate::execute_repository_write!(
        pool,
        "validation.insert",
        sqlx::query(
            r#"
        INSERT INTO validation_failure_records
            (id, artifact_id, run_id, stage_id, agent_id, stage_execution_id, agent_execution_id,
             timestamp, failure_class, failure_summary, record_json, recovery_action)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
        "#,
        )
        .bind(&record.id)
        .bind(record.artifact_id.to_string())
        .bind(record.run_id.to_string())
        .bind(&record.stage_id)
        .bind(&record.agent_id)
        .bind(record.stage_execution_id.to_string())
        .bind(record.agent_execution_id.to_string())
        .bind(record.timestamp.to_rfc3339())
        .bind(validation_failure_class_to_str(&record.failure_class))
        .bind(&record.failure_summary)
        .bind(serde_json::to_string(record).context("serialize ValidationFailureRecord")?)
        .bind(&record.recovery_recommendation.action)
    )
    .context("insert validation_failure_record")?;
    Ok(())
}

pub async fn find_by_artifact_id(
    pool: &SqlitePool,
    artifact_id: ArtifactId,
) -> Result<Option<ValidationFailureRecord>> {
    let row = sqlx::query(
        r#"SELECT record_json
           FROM validation_failure_records
           WHERE artifact_id = ?1"#,
    )
    .bind(artifact_id.to_string())
    .fetch_optional(pool)
    .await
    .context("find validation_failure_record by artifact_id")?;

    row.map(|r| {
        let raw: String = r.get("record_json");
        serde_json::from_str(&raw).context("decode ValidationFailureRecord")
    })
    .transpose()
}

pub async fn list_by_run(pool: &SqlitePool, run_id: RunId) -> Result<Vec<ValidationFailureRecord>> {
    let rows = sqlx::query(
        r#"SELECT record_json
           FROM validation_failure_records
           WHERE run_id = ?1
           ORDER BY timestamp ASC"#,
    )
    .bind(run_id.to_string())
    .fetch_all(pool)
    .await
    .context("list validation_failure_records by run")?;

    rows.into_iter()
        .map(|r| {
            let raw: String = r.get("record_json");
            serde_json::from_str(&raw).context("decode ValidationFailureRecord")
        })
        .collect()
}

pub async fn list_by_stage_execution(
    pool: &SqlitePool,
    stage_execution_id: StageExecutionId,
) -> Result<Vec<ValidationFailureRecord>> {
    let rows = sqlx::query(
        r#"SELECT record_json
           FROM validation_failure_records
           WHERE stage_execution_id = ?1
           ORDER BY timestamp ASC"#,
    )
    .bind(stage_execution_id.to_string())
    .fetch_all(pool)
    .await
    .context("list validation_failure_records by stage_execution_id")?;

    rows.into_iter()
        .map(|r| {
            let raw: String = r.get("record_json");
            serde_json::from_str(&raw).context("decode ValidationFailureRecord")
        })
        .collect()
}

fn validation_failure_class_to_str(class: &ValidationFailureClass) -> &'static str {
    match class {
        ValidationFailureClass::OutputContractMismatch => "output_contract_mismatch",
        ValidationFailureClass::NoOutputProduced => "no_output_produced",
        ValidationFailureClass::EmptyOutput => "empty_output",
        ValidationFailureClass::PersistenceFailure => "persistence_failure",
    }
}

pub fn parse_timestamp(raw: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(raw)
        .context("parse validation timestamp")?
        .with_timezone(&Utc))
}
