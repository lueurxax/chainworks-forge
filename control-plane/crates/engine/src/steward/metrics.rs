use std::collections::{BTreeMap, HashMap};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use domain::run::{Run, RunStatus};
use serde::Serialize;
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

use super::cohort::CohortKey;

#[derive(Debug, Serialize)]
pub struct MetricsSnapshot {
    pub cohort: Option<BTreeMap<String, String>>,
    pub run_count: usize,
    pub run_ids: Vec<String>,
    pub window_start: Option<String>,
    pub window_end: Option<String>,
    pub lead_time_median_seconds: Option<f64>,
    pub stage_latency_medians: BTreeMap<String, f64>,
    pub approval_wait_median_seconds: Option<f64>,
    pub proposal_loop_mean: f64,
    pub implementation_loop_mean: f64,
    pub retries_per_stage_mean: BTreeMap<String, f64>,
    pub approval_rejection_rate: f64,
    pub audit_pass_rate: f64,
    pub cost_per_run_median_cents: Option<i64>,
    pub cost_by_stage_family: BTreeMap<String, i64>,
    pub failed_run_rate: f64,
    pub blocked_run_rate: f64,
    pub drift_event_count: usize,
    pub resumed_run_count: usize,
    pub legacy_pre_p049_excluded_count: usize,
}

#[derive(Debug)]
struct StageMetricRow {
    run_id: String,
    stage_id: String,
    stage_type: Option<String>,
    status: String,
    iteration: i64,
    attempt_number: i64,
    started_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
}

pub async fn collect_metrics(
    pool: &SqlitePool,
    runs: &[Run],
    cohort: Option<&CohortKey>,
    legacy_pre_p049_excluded_count: usize,
) -> Result<MetricsSnapshot> {
    let run_ids = runs
        .iter()
        .map(|run| run.id.to_string())
        .collect::<Vec<_>>();
    let stage_family_lookup = build_stage_family_lookup(runs);
    let stage_rows = load_stage_rows(pool, &run_ids).await?;
    let approval_waits = load_approval_wait_seconds(pool, &run_ids).await?;
    let approval_rejection_rate = load_approval_rejection_rate(pool, &run_ids).await?;
    let run_costs = load_run_costs(pool, &run_ids).await?;
    let cost_by_stage_family =
        load_cost_by_stage_family(pool, &run_ids, &stage_family_lookup).await?;
    let resumed_run_count = load_resumed_run_count(pool, &run_ids).await?;

    let lead_times = runs
        .iter()
        .filter_map(|run| {
            run.completed_at
                .map(|completed_at| (completed_at - run.started_at).num_seconds() as f64)
        })
        .collect::<Vec<_>>();
    let mut status_counts = BTreeMap::new();
    for run in runs {
        *status_counts
            .entry(run.status.to_string())
            .or_insert(0usize) += 1;
    }
    let mut stage_latencies: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut retries: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut proposal_iterations = Vec::new();
    let mut implementation_iterations = Vec::new();
    let mut audit_total = 0usize;
    let mut audit_passed = 0usize;
    for stage in &stage_rows {
        if let Some(completed_at) = stage.completed_at {
            stage_latencies
                .entry(stage.stage_id.clone())
                .or_default()
                .push((completed_at - stage.started_at).num_seconds() as f64);
        }
        retries
            .entry(stage.stage_id.clone())
            .or_default()
            .push((stage.attempt_number - 1).max(0) as f64);
        let family = stage_family_lookup
            .get(&(stage.run_id.clone(), stage.stage_id.clone()))
            .map(String::as_str)
            .unwrap_or_else(|| stage_family(&stage.stage_id, stage.stage_type.as_deref()));
        if family == "proposal" {
            proposal_iterations.push(stage.iteration as f64);
        } else if family == "implementation" {
            implementation_iterations.push(stage.iteration as f64);
        } else if family == "audit" {
            audit_total += 1;
            if stage.status == "completed" {
                audit_passed += 1;
            }
        }
    }

    let mut cohort_map = BTreeMap::new();
    if let Some(cohort) = cohort {
        cohort_map.insert("risk_class".to_string(), cohort.risk_class.clone());
        cohort_map.insert(
            "workflow_family".to_string(),
            cohort.workflow_family.clone(),
        );
    }

    Ok(MetricsSnapshot {
        cohort: (!cohort_map.is_empty()).then_some(cohort_map),
        run_count: runs.len(),
        run_ids,
        window_start: min_time(runs).map(|t| t.to_rfc3339()),
        window_end: max_time(runs).map(|t| t.to_rfc3339()),
        lead_time_median_seconds: median(lead_times),
        stage_latency_medians: stage_latencies
            .into_iter()
            .filter_map(|(stage_id, values)| median(values).map(|median| (stage_id, median)))
            .collect(),
        approval_wait_median_seconds: median(approval_waits),
        proposal_loop_mean: mean(&proposal_iterations).unwrap_or(0.0),
        implementation_loop_mean: mean(&implementation_iterations).unwrap_or(0.0),
        retries_per_stage_mean: retries
            .into_iter()
            .map(|(stage_id, values)| (stage_id, mean(&values).unwrap_or(0.0)))
            .collect(),
        approval_rejection_rate,
        audit_pass_rate: if audit_total == 0 {
            0.0
        } else {
            audit_passed as f64 / audit_total as f64
        },
        cost_per_run_median_cents: median_i64(run_costs.into_values().collect()),
        cost_by_stage_family,
        failed_run_rate: rate(runs, |status| status == RunStatus::Failed),
        blocked_run_rate: rate(runs, |status| status == RunStatus::Blocked),
        drift_event_count: runs
            .iter()
            .filter(|run| run.drift_detected_at.is_some())
            .count(),
        resumed_run_count,
        legacy_pre_p049_excluded_count,
    })
}

async fn load_stage_rows(pool: &SqlitePool, run_ids: &[String]) -> Result<Vec<StageMetricRow>> {
    if run_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut builder = QueryBuilder::<Sqlite>::new(
        "SELECT run_id, stage_id, stage_type, status, iteration, attempt_number, started_at, completed_at FROM stage_executions WHERE run_id IN (",
    );
    push_bind_list(&mut builder, run_ids);
    builder.push(")");
    let rows = builder
        .build()
        .fetch_all(pool)
        .await
        .context("load steward stage metrics")?;
    rows.into_iter()
        .map(|row| {
            Ok(StageMetricRow {
                run_id: row.get("run_id"),
                stage_id: row.get("stage_id"),
                stage_type: row.get("stage_type"),
                status: row.get("status"),
                iteration: row.get("iteration"),
                attempt_number: row.get("attempt_number"),
                started_at: parse_dt(row.get("started_at"), "stage started_at")?,
                completed_at: parse_optional_dt(row.get("completed_at"), "stage completed_at")?,
            })
        })
        .collect()
}

async fn load_approval_wait_seconds(pool: &SqlitePool, run_ids: &[String]) -> Result<Vec<f64>> {
    if run_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut builder = QueryBuilder::<Sqlite>::new(
        "SELECT requested_at, decided_at FROM approvals WHERE decided_at IS NOT NULL AND run_id IN (",
    );
    push_bind_list(&mut builder, run_ids);
    builder.push(")");
    let rows = builder
        .build()
        .fetch_all(pool)
        .await
        .context("load steward approval waits")?;
    rows.into_iter()
        .map(|row| {
            let requested_at = parse_dt(row.get("requested_at"), "approval requested_at")?;
            let decided_at = parse_dt(row.get("decided_at"), "approval decided_at")?;
            Ok((decided_at - requested_at).num_seconds() as f64)
        })
        .collect()
}

async fn load_approval_rejection_rate(pool: &SqlitePool, run_ids: &[String]) -> Result<f64> {
    if run_ids.is_empty() {
        return Ok(0.0);
    }
    let mut builder = QueryBuilder::<Sqlite>::new(
        "SELECT decision FROM approvals WHERE decision IN ('granted', 'rejected') AND run_id IN (",
    );
    push_bind_list(&mut builder, run_ids);
    builder.push(")");
    let rows = builder
        .build()
        .fetch_all(pool)
        .await
        .context("load steward approval decisions")?;
    if rows.is_empty() {
        return Ok(0.0);
    }
    let rejected = rows
        .iter()
        .filter(|row| row.get::<String, _>("decision") == "rejected")
        .count();
    Ok(rejected as f64 / rows.len() as f64)
}

async fn load_run_costs(pool: &SqlitePool, run_ids: &[String]) -> Result<BTreeMap<String, i64>> {
    if run_ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let mut builder = QueryBuilder::<Sqlite>::new(
        "SELECT se.run_id AS run_id, COALESCE(SUM(sg.cumulative_cost_cents), 0) AS cost
         FROM stage_executions se
         JOIN agent_executions ae ON ae.stage_execution_id = se.id
         LEFT JOIN session_generations sg ON sg.id = ae.session_generation_id
         WHERE se.run_id IN (",
    );
    push_bind_list(&mut builder, run_ids);
    builder.push(") GROUP BY se.run_id");
    let rows = builder
        .build()
        .fetch_all(pool)
        .await
        .context("load steward run costs")?;
    Ok(rows
        .into_iter()
        .map(|row| (row.get("run_id"), row.get("cost")))
        .collect())
}

async fn load_cost_by_stage_family(
    pool: &SqlitePool,
    run_ids: &[String],
    stage_family_lookup: &HashMap<(String, String), String>,
) -> Result<BTreeMap<String, i64>> {
    if run_ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let mut builder = QueryBuilder::<Sqlite>::new(
        "SELECT se.run_id AS run_id, se.stage_id AS stage_id, se.stage_type AS stage_type, COALESCE(SUM(sg.cumulative_cost_cents), 0) AS cost
         FROM stage_executions se
         JOIN agent_executions ae ON ae.stage_execution_id = se.id
         LEFT JOIN session_generations sg ON sg.id = ae.session_generation_id
         WHERE se.run_id IN (",
    );
    push_bind_list(&mut builder, run_ids);
    builder.push(") GROUP BY se.run_id, se.stage_id, se.stage_type");
    let rows = builder
        .build()
        .fetch_all(pool)
        .await
        .context("load steward cost by stage family")?;
    let mut costs = BTreeMap::new();
    for row in rows {
        let run_id: String = row.get("run_id");
        let stage_id: String = row.get("stage_id");
        let stage_type: Option<String> = row.get("stage_type");
        let family = stage_family_lookup
            .get(&(run_id, stage_id.clone()))
            .map(String::as_str)
            .unwrap_or_else(|| stage_family(&stage_id, stage_type.as_deref()));
        *costs.entry(family.to_string()).or_insert(0) += row.get::<i64, _>("cost");
    }
    Ok(costs)
}

async fn load_resumed_run_count(pool: &SqlitePool, run_ids: &[String]) -> Result<usize> {
    if run_ids.is_empty() {
        return Ok(0);
    }
    let mut builder = QueryBuilder::<Sqlite>::new(
        "SELECT DISTINCT se.run_id AS run_id
         FROM stage_executions se
         JOIN agent_executions ae ON ae.stage_execution_id = se.id
         WHERE (ae.session_reuse_disposition LIKE 'reused%' OR ae.session_reset_reason IS NOT NULL)
           AND se.run_id IN (",
    );
    push_bind_list(&mut builder, run_ids);
    builder.push(")");
    let rows = builder
        .build()
        .fetch_all(pool)
        .await
        .context("load steward resumed run count")?;
    Ok(rows.len())
}

fn push_bind_list<'args>(builder: &mut QueryBuilder<'args, Sqlite>, values: &'args [String]) {
    let mut separated = builder.separated(", ");
    for value in values {
        separated.push_bind(value);
    }
}

fn build_stage_family_lookup(runs: &[Run]) -> HashMap<(String, String), String> {
    let mut lookup = HashMap::new();
    for run in runs {
        let Some(snapshot_json) = run.workflow_snapshot_json.as_deref() else {
            continue;
        };
        let Ok(snapshot) = serde_json::from_str::<serde_json::Value>(snapshot_json) else {
            continue;
        };
        let Some(states) = snapshot.get("states").and_then(|states| states.as_object()) else {
            continue;
        };
        for (stage_id, state) in states {
            lookup.insert(
                (run.id.to_string(), stage_id.clone()),
                stage_family_from_snapshot_state(state).to_string(),
            );
        }
    }
    lookup
}

fn stage_family_from_snapshot_state(state: &serde_json::Value) -> &'static str {
    let state_type = state
        .get("state_type")
        .or_else(|| state.get("type"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if state_type == "manual_gate" || state.get("approval").is_some() {
        return "approval";
    }

    let mut haystack = String::new();
    append_value_text(state.get("label"), &mut haystack);
    append_value_text(state.get("owner"), &mut haystack);
    append_value_text(state.get("run"), &mut haystack);
    append_value_text(state.get("run_after_approval"), &mut haystack);
    let haystack = haystack.to_ascii_lowercase();
    if haystack.contains("proposal") {
        "proposal"
    } else if haystack.contains("implementation") || haystack.contains("code") {
        "implementation"
    } else if haystack.contains("audit") || haystack.contains("review") {
        "audit"
    } else {
        "other"
    }
}

fn append_value_text(value: Option<&serde_json::Value>, output: &mut String) {
    match value {
        Some(serde_json::Value::String(s)) => {
            output.push(' ');
            output.push_str(s);
        }
        Some(serde_json::Value::Array(items)) => {
            for item in items {
                append_value_text(Some(item), output);
            }
        }
        Some(serde_json::Value::Object(map)) => {
            for value in map.values() {
                append_value_text(Some(value), output);
            }
        }
        _ => {}
    }
}

fn stage_family(stage_id: &str, stage_type: Option<&str>) -> &'static str {
    let haystack = format!(
        "{} {}",
        stage_id.to_ascii_lowercase(),
        stage_type.unwrap_or_default().to_ascii_lowercase()
    );
    if haystack.contains("proposal") {
        "proposal"
    } else if haystack.contains("implementation") || haystack.contains("code") {
        "implementation"
    } else if haystack.contains("audit") || haystack.contains("review") {
        "audit"
    } else {
        "other"
    }
}

fn rate<F>(runs: &[Run], predicate: F) -> f64
where
    F: Fn(RunStatus) -> bool,
{
    if runs.is_empty() {
        return 0.0;
    }
    runs.iter()
        .filter(|run| predicate(run.status.clone()))
        .count() as f64
        / runs.len() as f64
}

fn min_time(runs: &[Run]) -> Option<DateTime<Utc>> {
    runs.iter()
        .filter_map(|run| run.completed_at.or(Some(run.started_at)))
        .min()
}

fn max_time(runs: &[Run]) -> Option<DateTime<Utc>> {
    runs.iter()
        .filter_map(|run| run.completed_at.or(Some(run.started_at)))
        .max()
}

fn median(mut values: Vec<f64>) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = values.len() / 2;
    if values.len() % 2 == 0 {
        Some((values[mid - 1] + values[mid]) / 2.0)
    } else {
        Some(values[mid])
    }
}

fn median_i64(mut values: Vec<i64>) -> Option<i64> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    Some(values[values.len() / 2])
}

fn mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

fn parse_dt(s: String, field: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(&s)
        .with_context(|| format!("parse steward {field}"))
        .map(|dt| dt.with_timezone(&Utc))
}

fn parse_optional_dt(s: Option<String>, field: &str) -> Result<Option<DateTime<Utc>>> {
    s.map(|v| parse_dt(v, field)).transpose()
}
