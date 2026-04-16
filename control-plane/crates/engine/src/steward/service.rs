use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use db::repos::{runs, steward};
use domain::run::Run;
use domain::steward::{
    CohortQuality, StewardAnalysis, StewardAnalysisRunLink, StewardAnalysisStatus,
    StewardRecommendation,
};
use serde::Serialize;
use sqlx::SqlitePool;
use tracing::warn;
use uuid::Uuid;

use super::anomaly;
use super::cohort::{classify_quality, is_p049_eligible, primary_cohort_key, runs_for_cohort};
use super::config::StewardRuntimeInputs;
use super::dossier;
use super::json::{canonical_hash, canonical_json, write_canonical_json};
use super::metrics;

#[derive(Clone, Debug)]
pub struct StewardAnalysisRequest {
    pub reason: String,
    pub artifact_base: PathBuf,
}

impl StewardAnalysisRequest {
    pub fn manual(artifact_base: impl Into<PathBuf>) -> Self {
        Self {
            reason: "manual".into(),
            artifact_base: artifact_base.into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct StewardAgentInvocation {
    pub agent_id: String,
    pub chainworks_meta_root: PathBuf,
}

#[async_trait]
pub trait StewardAgentExecutor: Send + Sync {
    async fn run_steward_agent(&self, invocation: StewardAgentInvocation) -> Result<()>;
}

#[derive(Debug, Serialize)]
struct WorkflowSnapshotIndex {
    snapshot_count: usize,
    primary_workflow_family: Option<String>,
    entries: Vec<WorkflowSnapshotEntry>,
}

#[derive(Debug, Serialize)]
struct WorkflowSnapshotEntry {
    workflow_snapshot_hash: String,
    workflow_family: Option<String>,
    run_ids: Vec<String>,
    workflow_snapshot_json: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct ConfigChangeLog {
    reason: String,
    previous_steward_config_hash: Option<String>,
    current_steward_config_hash: String,
    previous_agent_catalog_hash: Option<String>,
    current_agent_catalog_hash: String,
    changed_inputs: Vec<String>,
    trigger_pending_before_run: bool,
}

pub async fn run_steward_analysis(
    pool: &SqlitePool,
    runtime_inputs: &StewardRuntimeInputs,
    request: StewardAnalysisRequest,
) -> Result<StewardAnalysis> {
    run_steward_analysis_with_executor(pool, runtime_inputs, request, None).await
}

pub async fn run_steward_analysis_with_executor(
    pool: &SqlitePool,
    runtime_inputs: &StewardRuntimeInputs,
    request: StewardAnalysisRequest,
    agent_executor: Option<&dyn StewardAgentExecutor>,
) -> Result<StewardAnalysis> {
    let analysis_id = Uuid::new_v4().to_string();
    let created_at = Utc::now();
    match run_steward_analysis_inner(
        pool,
        runtime_inputs,
        request.clone(),
        agent_executor,
        analysis_id.clone(),
        created_at,
    )
    .await
    {
        Ok(analysis) => Ok(analysis),
        Err(err) => {
            if let Err(record_err) = persist_failed_analysis(
                pool,
                runtime_inputs,
                &request,
                &analysis_id,
                created_at,
                &err,
            )
            .await
            {
                warn!(
                    analysis_id = %analysis_id,
                    error = %record_err,
                    "Failed to persist failed Steward analysis row"
                );
            }
            Err(err)
        }
    }
}

async fn run_steward_analysis_inner(
    pool: &SqlitePool,
    runtime_inputs: &StewardRuntimeInputs,
    request: StewardAnalysisRequest,
    agent_executor: Option<&dyn StewardAgentExecutor>,
    analysis_id: String,
    created_at: DateTime<Utc>,
) -> Result<StewardAnalysis> {
    let artifact_root = request
        .artifact_base
        .join("steward")
        .join("analyses")
        .join(&analysis_id);
    let io_root = artifact_root.join("active-catalog-io");
    let steward_io_root = io_root.join("steward");

    let completed_runs = runs::list_completed(pool, 1000).await?;
    let eligible_runs = completed_runs
        .into_iter()
        .filter(|run| {
            within_max_age(
                run,
                runtime_inputs
                    .steward_config
                    .windows
                    .maximum_window_age_days,
            )
        })
        .collect::<Vec<_>>();
    let legacy_pre_p049_excluded_count = eligible_runs
        .iter()
        .filter(|run| !is_p049_eligible(run))
        .count();
    let primary_key = primary_cohort_key(&eligible_runs);
    let cohort_runs = primary_key
        .as_ref()
        .map(|key| runs_for_cohort(&eligible_runs, key))
        .unwrap_or_default();
    let quality = if cohort_runs.is_empty() {
        CohortQuality::Weak
    } else {
        classify_quality(&cohort_runs)
    };

    let observation_size = runtime_inputs
        .steward_config
        .windows
        .observation_window_size;
    let baseline_size = runtime_inputs.steward_config.windows.baseline_window_size;
    let minimum_window_size = runtime_inputs.steward_config.windows.minimum_window_size;
    let observation_runs = cohort_runs
        .iter()
        .take(observation_size)
        .cloned()
        .collect::<Vec<_>>();
    let baseline_runs = cohort_runs
        .iter()
        .skip(observation_runs.len())
        .take(baseline_size)
        .cloned()
        .collect::<Vec<_>>();

    let status = if observation_runs.len() < minimum_window_size
        || baseline_runs.len() < minimum_window_size
    {
        StewardAnalysisStatus::Inconclusive
    } else {
        StewardAnalysisStatus::Completed
    };

    let metrics_window = metrics::collect_metrics(
        pool,
        &observation_runs,
        primary_key.as_ref(),
        legacy_pre_p049_excluded_count,
    )
    .await?;
    let baseline_window =
        metrics::collect_metrics(pool, &baseline_runs, primary_key.as_ref(), 0).await?;
    let signals = if status == StewardAnalysisStatus::Completed {
        anomaly::detect(
            &analysis_id,
            &metrics_window,
            &baseline_window,
            &runtime_inputs.steward_config.thresholds,
            minimum_window_size,
            quality.clone(),
            &observation_runs
                .iter()
                .map(|run| run.id.to_string())
                .collect::<Vec<_>>(),
        )
    } else {
        Vec::new()
    };

    let metrics_path = steward_io_root.join("metrics-window.json");
    let baseline_path = steward_io_root.join("baseline-window.json");
    let workflow_snapshot_path = steward_io_root.join("workflow-snapshot.json");
    let catalog_snapshot_path = steward_io_root.join("catalog-snapshot.json");
    let config_change_log_path = steward_io_root.join("config-change-log.json");
    let deterministic_alert_path = artifact_root.join("degradation-alerts.json");
    let catalog_alert_path = steward_io_root
        .join("reports")
        .join("degradation-alert.json");

    write_canonical_json(&metrics_path, &metrics_window)?;
    write_canonical_json(&baseline_path, &baseline_window)?;
    let workflow_snapshot_index = build_workflow_snapshot_index(&cohort_runs)?;
    write_canonical_json(&workflow_snapshot_path, &workflow_snapshot_index)?;
    write_canonical_json(&catalog_snapshot_path, &runtime_inputs.agent_catalog_json)?;
    let config_change_log = build_config_change_log(&request, runtime_inputs);
    write_canonical_json(&config_change_log_path, &config_change_log)?;
    write_canonical_json(&deterministic_alert_path, &signals)?;
    write_canonical_json(&catalog_alert_path, &signals)?;

    let dossier_runs = if signals.is_empty() {
        observation_runs.iter().take(5).cloned().collect::<Vec<_>>()
    } else {
        observation_runs.clone()
    };
    for run in &dossier_runs {
        let dossier = dossier::build_dossier(pool, run).await?;
        let dossier_path = steward_io_root
            .join("dossiers")
            .join(format!("{}.json", run.id));
        write_canonical_json(&dossier_path, &dossier)?;
    }

    let mut health_report_artifact_id = None;
    let mut audit_report_artifact_id = None;
    let mut agent_tuning_artifact_id = None;
    let mut workflow_tuning_artifact_id = None;
    let mut experiment_plan_artifact_id = None;
    if let Some(executor) = agent_executor {
        if let Err(err) = executor
            .run_steward_agent(StewardAgentInvocation {
                agent_id: "system_steward".into(),
                chainworks_meta_root: io_root.clone(),
            })
            .await
        {
            warn!(error = %err, "Optional system_steward lane failed");
        }
        let health_report_path = steward_io_root.join("reports").join("health-report.json");
        if health_report_path.exists() {
            health_report_artifact_id = Some(artifact_id(&health_report_path));
            if let Err(err) = executor
                .run_steward_agent(StewardAgentInvocation {
                    agent_id: "steward_auditor".into(),
                    chainworks_meta_root: io_root.clone(),
                })
                .await
            {
                warn!(error = %err, "Optional steward_auditor lane failed");
            }
        }
        audit_report_artifact_id =
            existing_artifact_id(&steward_io_root.join("reports").join("audit-report.json"));
        agent_tuning_artifact_id =
            existing_artifact_id(&steward_io_root.join("proposals").join("agent-tuning.json"));
        workflow_tuning_artifact_id = existing_artifact_id(
            &steward_io_root
                .join("proposals")
                .join("workflow-tuning.json"),
        );
        experiment_plan_artifact_id = existing_artifact_id(
            &steward_io_root
                .join("proposals")
                .join("experiment-plan.json"),
        );
    }

    let degradation_count = signals
        .iter()
        .filter(|signal| signal.direction == "degradation")
        .count() as i64;
    let improvement_count = signals
        .iter()
        .filter(|signal| signal.direction == "improvement")
        .count() as i64;
    let cohort_keys_json = primary_key
        .as_ref()
        .map(canonical_json)
        .transpose()?
        .unwrap_or_else(|| "{}".into());
    let workflow_snapshot_artifact_hash = canonical_hash(&workflow_snapshot_index)?;
    let analysis = StewardAnalysis {
        id: analysis_id.clone(),
        created_at,
        window_start: min_completed_at(&observation_runs).unwrap_or(created_at),
        window_end: max_completed_at(&observation_runs).unwrap_or(created_at),
        run_count: observation_runs.len() as i64,
        cohort_keys_json,
        cohort_quality: quality,
        status,
        degradation_count,
        improvement_count,
        workflow_snapshot_artifact_hash,
        agent_catalog_snapshot_hash: runtime_inputs.agent_catalog_hash.clone(),
        steward_config_snapshot_hash: runtime_inputs.steward_config_hash.clone(),
        metrics_snapshot_artifact_id: Some(artifact_id(&metrics_path)),
        baseline_snapshot_artifact_id: Some(artifact_id(&baseline_path)),
        agent_catalog_snapshot_artifact_id: Some(artifact_id(&catalog_snapshot_path)),
        workflow_snapshot_artifact_id: Some(artifact_id(&workflow_snapshot_path)),
        config_change_log_artifact_id: Some(artifact_id(&config_change_log_path)),
        health_report_artifact_id,
        degradation_alert_artifact_id: Some(artifact_id(&catalog_alert_path)),
        agent_tuning_artifact_id,
        workflow_tuning_artifact_id,
        experiment_plan_artifact_id,
        audit_report_artifact_id,
        trigger_reason: request.reason.clone(),
        error_summary: None,
    };
    steward::insert_analysis(pool, &analysis).await?;

    let observation_role = if signals.is_empty() {
        "context"
    } else {
        "implicated"
    };
    for run in &dossier_runs {
        steward::insert_run_link(
            pool,
            &StewardAnalysisRunLink {
                id: Uuid::new_v4().to_string(),
                analysis_id: analysis_id.clone(),
                run_id: run.id.to_string(),
                role: observation_role.into(),
            },
        )
        .await?;
    }
    for run in &baseline_runs {
        steward::insert_run_link(
            pool,
            &StewardAnalysisRunLink {
                id: Uuid::new_v4().to_string(),
                analysis_id: analysis_id.clone(),
                run_id: run.id.to_string(),
                role: "baseline".into(),
            },
        )
        .await?;
    }
    for signal in signals
        .iter()
        .filter(|signal| signal.direction == "degradation")
    {
        steward::insert_recommendation(
            pool,
            &StewardRecommendation {
                id: Uuid::new_v4().to_string(),
                analysis_id: analysis_id.clone(),
                created_at,
                category: "degradation".into(),
                summary: format!(
                    "{} changed by {:.2}% versus baseline",
                    signal.metric_name, signal.delta_percentage
                ),
                target_metric: signal.metric_name.clone(),
                confidence_level: signal.confidence.clone(),
                status: "proposed".into(),
                source_artifact_name: Some("deterministic_signal".into()),
                decision_comment: None,
                decided_at: None,
            },
        )
        .await?;
    }

    Ok(analysis)
}

async fn persist_failed_analysis(
    pool: &SqlitePool,
    runtime_inputs: &StewardRuntimeInputs,
    request: &StewardAnalysisRequest,
    analysis_id: &str,
    created_at: DateTime<Utc>,
    error: &anyhow::Error,
) -> Result<()> {
    if steward::find_analysis(pool, analysis_id).await?.is_some() {
        return Ok(());
    }

    let error_summary = error.to_string();
    let failure_hash = canonical_hash(&serde_json::json!({
        "status": "failed",
        "error_summary": error_summary,
        "trigger_reason": request.reason,
    }))?;
    let analysis = StewardAnalysis {
        id: analysis_id.to_string(),
        created_at,
        window_start: created_at,
        window_end: created_at,
        run_count: 0,
        cohort_keys_json: "{}".into(),
        cohort_quality: CohortQuality::Weak,
        status: StewardAnalysisStatus::Failed,
        degradation_count: 0,
        improvement_count: 0,
        workflow_snapshot_artifact_hash: failure_hash,
        agent_catalog_snapshot_hash: runtime_inputs.agent_catalog_hash.clone(),
        steward_config_snapshot_hash: runtime_inputs.steward_config_hash.clone(),
        metrics_snapshot_artifact_id: None,
        baseline_snapshot_artifact_id: None,
        agent_catalog_snapshot_artifact_id: None,
        workflow_snapshot_artifact_id: None,
        config_change_log_artifact_id: None,
        health_report_artifact_id: None,
        degradation_alert_artifact_id: None,
        agent_tuning_artifact_id: None,
        workflow_tuning_artifact_id: None,
        experiment_plan_artifact_id: None,
        audit_report_artifact_id: None,
        trigger_reason: request.reason.clone(),
        error_summary: Some(error_summary),
    };
    steward::insert_analysis(pool, &analysis).await
}

fn within_max_age(run: &Run, maximum_window_age_days: usize) -> bool {
    let Some(completed_at) = run.completed_at else {
        return false;
    };
    completed_at >= Utc::now() - Duration::days(maximum_window_age_days as i64)
}

fn build_workflow_snapshot_index(runs: &[Run]) -> Result<WorkflowSnapshotIndex> {
    let mut snapshots: BTreeMap<String, (Option<String>, BTreeSet<String>, serde_json::Value)> =
        BTreeMap::new();
    for run in runs {
        let (Some(hash), Some(snapshot_json)) =
            (&run.workflow_snapshot_hash, &run.workflow_snapshot_json)
        else {
            continue;
        };
        let snapshot = serde_json::from_str::<serde_json::Value>(snapshot_json)
            .with_context(|| format!("parse workflow snapshot for run {}", run.id))?;
        let entry = snapshots.entry(hash.clone()).or_insert_with(|| {
            (
                run.workflow_family.clone(),
                BTreeSet::new(),
                snapshot.clone(),
            )
        });
        entry.1.insert(run.id.to_string());
    }

    Ok(WorkflowSnapshotIndex {
        snapshot_count: snapshots.len(),
        primary_workflow_family: runs.iter().find_map(|run| run.workflow_family.clone()),
        entries: snapshots
            .into_iter()
            .map(
                |(workflow_snapshot_hash, (workflow_family, run_ids, workflow_snapshot_json))| {
                    WorkflowSnapshotEntry {
                        workflow_snapshot_hash,
                        workflow_family,
                        run_ids: run_ids.into_iter().collect(),
                        workflow_snapshot_json,
                    }
                },
            )
            .collect(),
    })
}

fn build_config_change_log(
    request: &StewardAnalysisRequest,
    runtime_inputs: &StewardRuntimeInputs,
) -> ConfigChangeLog {
    let mut changed_inputs = Vec::new();
    if runtime_inputs
        .previous_steward_config_hash
        .as_ref()
        .is_some_and(|previous| previous != &runtime_inputs.steward_config_hash)
    {
        changed_inputs.push("steward_config".into());
    }
    if runtime_inputs
        .previous_agent_catalog_hash
        .as_ref()
        .is_some_and(|previous| previous != &runtime_inputs.agent_catalog_hash)
    {
        changed_inputs.push("agent_catalog".into());
    }
    ConfigChangeLog {
        reason: request.reason.clone(),
        previous_steward_config_hash: runtime_inputs.previous_steward_config_hash.clone(),
        current_steward_config_hash: runtime_inputs.steward_config_hash.clone(),
        previous_agent_catalog_hash: runtime_inputs.previous_agent_catalog_hash.clone(),
        current_agent_catalog_hash: runtime_inputs.agent_catalog_hash.clone(),
        changed_inputs,
        trigger_pending_before_run: runtime_inputs.config_change_analysis_scheduled,
    }
}

fn min_completed_at(runs: &[Run]) -> Option<DateTime<Utc>> {
    runs.iter().filter_map(|run| run.completed_at).min()
}

fn max_completed_at(runs: &[Run]) -> Option<DateTime<Utc>> {
    runs.iter().filter_map(|run| run.completed_at).max()
}

fn artifact_id(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn existing_artifact_id(path: &Path) -> Option<String> {
    path.exists().then(|| artifact_id(path))
}
