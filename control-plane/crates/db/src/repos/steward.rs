use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};

use domain::steward::{
    StewardAnalysis, StewardAnalysisRunLink, StewardAnalysisStatus, StewardRecommendation,
};

#[derive(Clone, Debug, PartialEq)]
pub struct PendingConfigChange {
    pub config_hash: Option<String>,
    pub catalog_hash: Option<String>,
}

pub async fn insert_analysis(pool: &SqlitePool, analysis: &StewardAnalysis) -> Result<()> {
    crate::execute_repository_write!(
        pool,
        "steward.insert_analysis",
        sqlx::query(
            r#"
        INSERT INTO steward_analyses
          (id, created_at, window_start, window_end, run_count, cohort_keys_json, cohort_quality,
           status, degradation_count, improvement_count, workflow_snapshot_artifact_hash,
           agent_catalog_snapshot_hash, steward_config_snapshot_hash, metrics_snapshot_artifact_id,
           baseline_snapshot_artifact_id, agent_catalog_snapshot_artifact_id,
           workflow_snapshot_artifact_id, config_change_log_artifact_id, health_report_artifact_id,
           degradation_alert_artifact_id, agent_tuning_artifact_id, workflow_tuning_artifact_id,
           experiment_plan_artifact_id, audit_report_artifact_id, trigger_reason, error_summary)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)
        "#,
        )
        .bind(&analysis.id)
        .bind(analysis.created_at.to_rfc3339())
        .bind(analysis.window_start.to_rfc3339())
        .bind(analysis.window_end.to_rfc3339())
        .bind(analysis.run_count)
        .bind(&analysis.cohort_keys_json)
        .bind(analysis.cohort_quality.to_string())
        .bind(analysis.status.to_string())
        .bind(analysis.degradation_count)
        .bind(analysis.improvement_count)
        .bind(&analysis.workflow_snapshot_artifact_hash)
        .bind(&analysis.agent_catalog_snapshot_hash)
        .bind(&analysis.steward_config_snapshot_hash)
        .bind(&analysis.metrics_snapshot_artifact_id)
        .bind(&analysis.baseline_snapshot_artifact_id)
        .bind(&analysis.agent_catalog_snapshot_artifact_id)
        .bind(&analysis.workflow_snapshot_artifact_id)
        .bind(&analysis.config_change_log_artifact_id)
        .bind(&analysis.health_report_artifact_id)
        .bind(&analysis.degradation_alert_artifact_id)
        .bind(&analysis.agent_tuning_artifact_id)
        .bind(&analysis.workflow_tuning_artifact_id)
        .bind(&analysis.experiment_plan_artifact_id)
        .bind(&analysis.audit_report_artifact_id)
        .bind(&analysis.trigger_reason)
        .bind(&analysis.error_summary)
    )
    .context("insert steward analysis")?;
    Ok(())
}

pub async fn insert_run_link(pool: &SqlitePool, link: &StewardAnalysisRunLink) -> Result<()> {
    crate::execute_repository_write!(
        pool,
        "steward.insert_run_link",
        sqlx::query(
            r#"
        INSERT INTO steward_analysis_run_links (id, analysis_id, run_id, role)
        VALUES (?1, ?2, ?3, ?4)
        "#,
        )
        .bind(&link.id)
        .bind(&link.analysis_id)
        .bind(&link.run_id)
        .bind(&link.role)
    )
    .context("insert steward analysis run link")?;
    Ok(())
}

pub async fn insert_recommendation(
    pool: &SqlitePool,
    recommendation: &StewardRecommendation,
) -> Result<()> {
    crate::execute_repository_write!(
        pool,
        "steward.insert_recommendation",
        sqlx::query(
            r#"
        INSERT INTO steward_recommendations
          (id, analysis_id, created_at, category, summary, target_metric, confidence_level,
           status, source_artifact_name, decision_comment, decided_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
        )
        .bind(&recommendation.id)
        .bind(&recommendation.analysis_id)
        .bind(recommendation.created_at.to_rfc3339())
        .bind(&recommendation.category)
        .bind(&recommendation.summary)
        .bind(&recommendation.target_metric)
        .bind(&recommendation.confidence_level)
        .bind(&recommendation.status)
        .bind(&recommendation.source_artifact_name)
        .bind(&recommendation.decision_comment)
        .bind(recommendation.decided_at.map(|t| t.to_rfc3339()))
    )
    .context("insert steward recommendation")?;
    Ok(())
}

pub async fn find_analysis(pool: &SqlitePool, id: &str) -> Result<Option<StewardAnalysis>> {
    let row = sqlx::query(
        r#"
        SELECT id, created_at, window_start, window_end, run_count, cohort_keys_json, cohort_quality,
               status, degradation_count, improvement_count, workflow_snapshot_artifact_hash,
               agent_catalog_snapshot_hash, steward_config_snapshot_hash, metrics_snapshot_artifact_id,
               baseline_snapshot_artifact_id, agent_catalog_snapshot_artifact_id,
               workflow_snapshot_artifact_id, config_change_log_artifact_id, health_report_artifact_id,
               degradation_alert_artifact_id, agent_tuning_artifact_id, workflow_tuning_artifact_id,
               experiment_plan_artifact_id, audit_report_artifact_id, trigger_reason, error_summary
        FROM steward_analyses
        WHERE id = ?1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .context("find steward analysis")?;

    row.map(|r| parse_analysis_row(&r)).transpose()
}

pub async fn list_analyses(
    pool: &SqlitePool,
    limit: i64,
    status: Option<StewardAnalysisStatus>,
) -> Result<Vec<StewardAnalysis>> {
    let limit = limit.clamp(1, 200);
    let rows = if let Some(status) = status {
        sqlx::query(
            r#"
            SELECT id, created_at, window_start, window_end, run_count, cohort_keys_json, cohort_quality,
                   status, degradation_count, improvement_count, workflow_snapshot_artifact_hash,
                   agent_catalog_snapshot_hash, steward_config_snapshot_hash, metrics_snapshot_artifact_id,
                   baseline_snapshot_artifact_id, agent_catalog_snapshot_artifact_id,
                   workflow_snapshot_artifact_id, config_change_log_artifact_id, health_report_artifact_id,
                   degradation_alert_artifact_id, agent_tuning_artifact_id, workflow_tuning_artifact_id,
                   experiment_plan_artifact_id, audit_report_artifact_id, trigger_reason, error_summary
            FROM steward_analyses
            WHERE status = ?1
            ORDER BY created_at DESC
            LIMIT ?2
            "#,
        )
        .bind(status.to_string())
        .bind(limit)
        .fetch_all(pool)
        .await
        .context("list steward analyses by status")?
    } else {
        sqlx::query(
            r#"
            SELECT id, created_at, window_start, window_end, run_count, cohort_keys_json, cohort_quality,
                   status, degradation_count, improvement_count, workflow_snapshot_artifact_hash,
                   agent_catalog_snapshot_hash, steward_config_snapshot_hash, metrics_snapshot_artifact_id,
                   baseline_snapshot_artifact_id, agent_catalog_snapshot_artifact_id,
                   workflow_snapshot_artifact_id, config_change_log_artifact_id, health_report_artifact_id,
                   degradation_alert_artifact_id, agent_tuning_artifact_id, workflow_tuning_artifact_id,
                   experiment_plan_artifact_id, audit_report_artifact_id, trigger_reason, error_summary
            FROM steward_analyses
            ORDER BY created_at DESC
            LIMIT ?1
            "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await
        .context("list steward analyses")?
    };

    rows.iter().map(parse_analysis_row).collect()
}

pub async fn list_run_links(
    pool: &SqlitePool,
    analysis_id: &str,
) -> Result<Vec<StewardAnalysisRunLink>> {
    let rows = sqlx::query(
        r#"
        SELECT id, analysis_id, run_id, role
        FROM steward_analysis_run_links
        WHERE analysis_id = ?1
        ORDER BY role ASC, run_id ASC
        "#,
    )
    .bind(analysis_id)
    .fetch_all(pool)
    .await
    .context("list steward analysis run links")?;

    Ok(rows
        .into_iter()
        .map(|r| StewardAnalysisRunLink {
            id: r.get("id"),
            analysis_id: r.get("analysis_id"),
            run_id: r.get("run_id"),
            role: r.get("role"),
        })
        .collect())
}

pub async fn list_recommendations(
    pool: &SqlitePool,
    analysis_id: &str,
) -> Result<Vec<StewardRecommendation>> {
    let rows = sqlx::query(
        r#"
        SELECT id, analysis_id, created_at, category, summary, target_metric, confidence_level,
               status, source_artifact_name, decision_comment, decided_at
        FROM steward_recommendations
        WHERE analysis_id = ?1
        ORDER BY created_at ASC, id ASC
        "#,
    )
    .bind(analysis_id)
    .fetch_all(pool)
    .await
    .context("list steward recommendations")?;

    rows.iter().map(parse_recommendation_row).collect()
}

pub async fn get_runtime_state(pool: &SqlitePool, key: &str) -> Result<Option<String>> {
    let row = sqlx::query("SELECT value FROM steward_runtime_state WHERE key = ?1")
        .bind(key)
        .fetch_optional(pool)
        .await
        .context("get steward runtime state")?;
    Ok(row.map(|r| r.get("value")))
}

pub async fn set_runtime_state(pool: &SqlitePool, key: &str, value: &str) -> Result<()> {
    crate::execute_repository_write!(
        pool,
        "steward.set_runtime_state",
        sqlx::query(
            r#"
        INSERT INTO steward_runtime_state (key, value, updated_at)
        VALUES (?1, ?2, ?3)
        ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
        "#,
        )
        .bind(key)
        .bind(value)
        .bind(Utc::now().to_rfc3339())
    )
    .context("set steward runtime state")?;
    Ok(())
}

pub async fn mark_config_change_pending(
    pool: &SqlitePool,
    config_hash: Option<&str>,
    catalog_hash: Option<&str>,
) -> Result<()> {
    set_runtime_state(pool, "config_change_pending", "1").await?;
    if let Some(config_hash) = config_hash {
        set_runtime_state(pool, "pending_config_hash", config_hash).await?;
    }
    if let Some(catalog_hash) = catalog_hash {
        set_runtime_state(pool, "pending_catalog_hash", catalog_hash).await?;
    }
    Ok(())
}

pub async fn take_config_change_pending(pool: &SqlitePool) -> Result<Option<PendingConfigChange>> {
    let pending = get_runtime_state(pool, "config_change_pending").await?;
    if pending.as_deref() != Some("1") {
        return Ok(None);
    }
    let config_hash = get_runtime_state(pool, "pending_config_hash").await?;
    let catalog_hash = get_runtime_state(pool, "pending_catalog_hash").await?;
    set_runtime_state(pool, "config_change_pending", "0").await?;
    Ok(Some(PendingConfigChange {
        config_hash,
        catalog_hash,
    }))
}

pub async fn set_post_run_trigger_config(
    pool: &SqlitePool,
    enabled: bool,
    run_interval: usize,
) -> Result<()> {
    set_runtime_state(
        pool,
        "post_run_hook_enabled",
        if enabled { "1" } else { "0" },
    )
    .await?;
    set_runtime_state(
        pool,
        "post_run_hook_run_interval",
        &run_interval.to_string(),
    )
    .await?;
    Ok(())
}

pub async fn post_run_trigger_config(pool: &SqlitePool) -> Result<(bool, usize)> {
    let enabled = get_runtime_state(pool, "post_run_hook_enabled")
        .await?
        .as_deref()
        == Some("1");
    let run_interval = get_runtime_state(pool, "post_run_hook_run_interval")
        .await?
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(5)
        .max(1);
    Ok((enabled, run_interval))
}

pub async fn increment_completed_run_counter(pool: &SqlitePool) -> Result<usize> {
    let current = get_runtime_state(pool, "completed_runs_since_steward_analysis")
        .await?
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let next = current + 1;
    set_runtime_state(
        pool,
        "completed_runs_since_steward_analysis",
        &next.to_string(),
    )
    .await?;
    Ok(next)
}

pub async fn reset_completed_run_counter(pool: &SqlitePool) -> Result<()> {
    set_runtime_state(pool, "completed_runs_since_steward_analysis", "0").await
}

fn parse_analysis_row(r: &sqlx::sqlite::SqliteRow) -> Result<StewardAnalysis> {
    let status: String = r.get("status");
    let quality: String = r.get("cohort_quality");
    Ok(StewardAnalysis {
        id: r.get("id"),
        created_at: parse_dt(r.get("created_at"), "created_at")?,
        window_start: parse_dt(r.get("window_start"), "window_start")?,
        window_end: parse_dt(r.get("window_end"), "window_end")?,
        run_count: r.get("run_count"),
        cohort_keys_json: r.get("cohort_keys_json"),
        cohort_quality: quality.parse().map_err(|e: String| anyhow::anyhow!(e))?,
        status: status.parse().map_err(|e: String| anyhow::anyhow!(e))?,
        degradation_count: r.get("degradation_count"),
        improvement_count: r.get("improvement_count"),
        workflow_snapshot_artifact_hash: r.get("workflow_snapshot_artifact_hash"),
        agent_catalog_snapshot_hash: r.get("agent_catalog_snapshot_hash"),
        steward_config_snapshot_hash: r.get("steward_config_snapshot_hash"),
        metrics_snapshot_artifact_id: r.get("metrics_snapshot_artifact_id"),
        baseline_snapshot_artifact_id: r.get("baseline_snapshot_artifact_id"),
        agent_catalog_snapshot_artifact_id: r.get("agent_catalog_snapshot_artifact_id"),
        workflow_snapshot_artifact_id: r.get("workflow_snapshot_artifact_id"),
        config_change_log_artifact_id: r.get("config_change_log_artifact_id"),
        health_report_artifact_id: r.get("health_report_artifact_id"),
        degradation_alert_artifact_id: r.get("degradation_alert_artifact_id"),
        agent_tuning_artifact_id: r.get("agent_tuning_artifact_id"),
        workflow_tuning_artifact_id: r.get("workflow_tuning_artifact_id"),
        experiment_plan_artifact_id: r.get("experiment_plan_artifact_id"),
        audit_report_artifact_id: r.get("audit_report_artifact_id"),
        trigger_reason: r.get("trigger_reason"),
        error_summary: r.get("error_summary"),
    })
}

fn parse_recommendation_row(r: &sqlx::sqlite::SqliteRow) -> Result<StewardRecommendation> {
    Ok(StewardRecommendation {
        id: r.get("id"),
        analysis_id: r.get("analysis_id"),
        created_at: parse_dt(r.get("created_at"), "recommendation created_at")?,
        category: r.get("category"),
        summary: r.get("summary"),
        target_metric: r.get("target_metric"),
        confidence_level: r.get("confidence_level"),
        status: r.get("status"),
        source_artifact_name: r.get("source_artifact_name"),
        decision_comment: r.get("decision_comment"),
        decided_at: parse_optional_dt(r.get("decided_at"), "recommendation decided_at")?,
    })
}

fn parse_dt(s: String, field: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(&s)
        .with_context(|| format!("parse steward {field}"))
        .map(|dt| dt.with_timezone(&Utc))
}

fn parse_optional_dt(s: Option<String>, field: &str) -> Result<Option<DateTime<Utc>>> {
    s.map(|v| parse_dt(v, field)).transpose()
}
