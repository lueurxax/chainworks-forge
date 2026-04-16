use std::collections::BTreeMap;

use domain::steward::CohortQuality;
use serde::Serialize;

use super::config::StewardThreshold;
use super::metrics::MetricsSnapshot;

#[derive(Debug, Serialize)]
pub struct DegradationSignal {
    #[serde(skip_serializing)]
    pub analysis_id: String,
    pub metric_name: String,
    pub metric_family: String,
    pub observed_value: f64,
    pub baseline_value: f64,
    pub delta_percentage: f64,
    pub threshold_used: f64,
    pub severity: String,
    pub confidence: String,
    pub implicated_run_ids: Vec<String>,
    pub likely_causes: Vec<String>,
    pub direction: String,
}

pub fn detect(
    analysis_id: &str,
    observation: &MetricsSnapshot,
    baseline: &MetricsSnapshot,
    thresholds: &BTreeMap<String, StewardThreshold>,
    minimum_window_size: usize,
    cohort_quality: CohortQuality,
    observation_run_ids: &[String],
) -> Vec<DegradationSignal> {
    if observation.run_count < minimum_window_size || baseline.run_count < minimum_window_size {
        return Vec::new();
    }

    let mut signals = Vec::new();
    compare_optional(
        &mut signals,
        analysis_id,
        "lead_time_median_seconds",
        "throughput",
        observation.lead_time_median_seconds,
        baseline.lead_time_median_seconds,
        thresholds,
        cohort_quality.clone(),
        observation_run_ids,
    );
    compare_optional(
        &mut signals,
        analysis_id,
        "approval_wait_median_seconds",
        "approvals",
        observation.approval_wait_median_seconds,
        baseline.approval_wait_median_seconds,
        thresholds,
        cohort_quality.clone(),
        observation_run_ids,
    );
    compare_scalar(
        &mut signals,
        analysis_id,
        "proposal_loop_mean",
        "loops",
        observation.proposal_loop_mean,
        baseline.proposal_loop_mean,
        thresholds,
        cohort_quality.clone(),
        observation_run_ids,
    );
    compare_scalar(
        &mut signals,
        analysis_id,
        "implementation_loop_mean",
        "loops",
        observation.implementation_loop_mean,
        baseline.implementation_loop_mean,
        thresholds,
        cohort_quality.clone(),
        observation_run_ids,
    );
    compare_scalar(
        &mut signals,
        analysis_id,
        "approval_rejection_rate",
        "approvals",
        observation.approval_rejection_rate,
        baseline.approval_rejection_rate,
        thresholds,
        cohort_quality.clone(),
        observation_run_ids,
    );
    compare_scalar(
        &mut signals,
        analysis_id,
        "failed_run_rate",
        "stability",
        observation.failed_run_rate,
        baseline.failed_run_rate,
        thresholds,
        cohort_quality.clone(),
        observation_run_ids,
    );
    compare_scalar(
        &mut signals,
        analysis_id,
        "blocked_run_rate",
        "stability",
        observation.blocked_run_rate,
        baseline.blocked_run_rate,
        thresholds,
        cohort_quality,
        observation_run_ids,
    );

    for (stage_id, observed) in &observation.stage_latency_medians {
        if let Some(baseline_value) = baseline.stage_latency_medians.get(stage_id) {
            compare_scalar(
                &mut signals,
                analysis_id,
                &format!("stage_latency_medians.{stage_id}"),
                "stage_latency",
                *observed,
                *baseline_value,
                thresholds,
                CohortQuality::Acceptable,
                observation_run_ids,
            );
        }
    }

    signals
}

fn compare_optional(
    signals: &mut Vec<DegradationSignal>,
    analysis_id: &str,
    metric_name: &str,
    metric_family: &str,
    observed: Option<f64>,
    baseline: Option<f64>,
    thresholds: &BTreeMap<String, StewardThreshold>,
    cohort_quality: CohortQuality,
    run_ids: &[String],
) {
    if let (Some(observed), Some(baseline)) = (observed, baseline) {
        compare_scalar(
            signals,
            analysis_id,
            metric_name,
            metric_family,
            observed,
            baseline,
            thresholds,
            cohort_quality,
            run_ids,
        );
    }
}

fn compare_scalar(
    signals: &mut Vec<DegradationSignal>,
    analysis_id: &str,
    metric_name: &str,
    metric_family: &str,
    observed: f64,
    baseline: f64,
    thresholds: &BTreeMap<String, StewardThreshold>,
    cohort_quality: CohortQuality,
    run_ids: &[String],
) {
    if baseline == 0.0 {
        return;
    }
    let Some(threshold_entry) = thresholds
        .get(metric_name)
        .or_else(|| thresholds.get(metric_family))
        .or_else(|| threshold_family_alias(metric_family).and_then(|alias| thresholds.get(alias)))
    else {
        return;
    };
    let threshold = threshold_entry.trigger;
    let delta_percentage = ((observed - baseline) / baseline) * 100.0;
    let threshold_percentage = threshold * 100.0;
    if delta_percentage.abs() < threshold_percentage {
        return;
    }
    let direction = if delta_percentage > 0.0 {
        "degradation"
    } else {
        "improvement"
    };
    signals.push(DegradationSignal {
        analysis_id: analysis_id.to_string(),
        metric_name: metric_name.to_string(),
        metric_family: metric_family.to_string(),
        observed_value: observed,
        baseline_value: baseline,
        delta_percentage,
        threshold_used: threshold,
        severity: if delta_percentage.abs() >= threshold_percentage * 2.0 {
            "high".into()
        } else {
            "medium".into()
        },
        confidence: match cohort_quality {
            CohortQuality::Strong => "high".into(),
            CohortQuality::Acceptable => "medium".into(),
            CohortQuality::Weak => "low".into(),
        },
        implicated_run_ids: run_ids.to_vec(),
        likely_causes: vec![format!("{metric_name} diverged from baseline")],
        direction: direction.into(),
    });
}

fn threshold_family_alias(metric_family: &str) -> Option<&'static str> {
    match metric_family {
        "throughput" | "stage_latency" => Some("timing"),
        "loops" => Some("rework"),
        "approvals" => Some("quality"),
        "stability" => Some("stability"),
        _ => None,
    }
}
