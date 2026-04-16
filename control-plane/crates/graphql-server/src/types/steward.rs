use async_graphql::*;
use domain::steward::{StewardAnalysis, StewardAnalysisRunLink, StewardRecommendation};

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlStewardAnalysis {
    pub id: ID,
    pub created_at: String,
    pub status: String,
    pub trigger_reason: String,
    pub cohort_quality: String,
    pub cohort_keys_json: String,
    pub run_count: i64,
    pub degradation_count: i64,
    pub improvement_count: i64,
    pub workflow_snapshot_artifact_hash: String,
    pub agent_catalog_snapshot_hash: String,
    pub steward_config_snapshot_hash: String,
    pub artifact_ids: Vec<ID>,
    pub error_summary: Option<String>,
    pub recommendations: Vec<GqlStewardRecommendation>,
    pub linked_runs: Vec<GqlStewardAnalysisRunLink>,
}

impl GqlStewardAnalysis {
    pub fn from_parts(
        analysis: StewardAnalysis,
        linked_runs: Vec<StewardAnalysisRunLink>,
        recommendations: Vec<StewardRecommendation>,
    ) -> Self {
        let artifact_ids = [
            analysis.metrics_snapshot_artifact_id.clone(),
            analysis.baseline_snapshot_artifact_id.clone(),
            analysis.agent_catalog_snapshot_artifact_id.clone(),
            analysis.workflow_snapshot_artifact_id.clone(),
            analysis.config_change_log_artifact_id.clone(),
            analysis.health_report_artifact_id.clone(),
            analysis.degradation_alert_artifact_id.clone(),
            analysis.agent_tuning_artifact_id.clone(),
            analysis.workflow_tuning_artifact_id.clone(),
            analysis.experiment_plan_artifact_id.clone(),
            analysis.audit_report_artifact_id.clone(),
        ]
        .into_iter()
        .flatten()
        .map(ID)
        .collect();

        Self {
            id: ID(analysis.id),
            created_at: analysis.created_at.to_rfc3339(),
            status: analysis.status.to_string(),
            trigger_reason: analysis.trigger_reason,
            cohort_quality: analysis.cohort_quality.to_string(),
            cohort_keys_json: analysis.cohort_keys_json,
            run_count: analysis.run_count,
            degradation_count: analysis.degradation_count,
            improvement_count: analysis.improvement_count,
            workflow_snapshot_artifact_hash: analysis.workflow_snapshot_artifact_hash,
            agent_catalog_snapshot_hash: analysis.agent_catalog_snapshot_hash,
            steward_config_snapshot_hash: analysis.steward_config_snapshot_hash,
            artifact_ids,
            error_summary: analysis.error_summary,
            recommendations: recommendations
                .into_iter()
                .map(GqlStewardRecommendation::from)
                .collect(),
            linked_runs: linked_runs
                .into_iter()
                .map(GqlStewardAnalysisRunLink::from)
                .collect(),
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlStewardAnalysisRunLink {
    pub id: ID,
    pub analysis_id: ID,
    pub run_id: ID,
    pub role: String,
}

impl From<StewardAnalysisRunLink> for GqlStewardAnalysisRunLink {
    fn from(value: StewardAnalysisRunLink) -> Self {
        Self {
            id: ID(value.id),
            analysis_id: ID(value.analysis_id),
            run_id: ID(value.run_id),
            role: value.role,
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlStewardRecommendation {
    pub id: ID,
    pub analysis_id: ID,
    pub created_at: String,
    pub category: String,
    pub summary: String,
    pub target_metric: String,
    pub confidence_level: String,
    pub status: String,
    pub source_artifact_name: Option<String>,
    pub decision_comment: Option<String>,
    pub decided_at: Option<String>,
}

impl From<StewardRecommendation> for GqlStewardRecommendation {
    fn from(value: StewardRecommendation) -> Self {
        Self {
            id: ID(value.id),
            analysis_id: ID(value.analysis_id),
            created_at: value.created_at.to_rfc3339(),
            category: value.category,
            summary: value.summary,
            target_metric: value.target_metric,
            confidence_level: value.confidence_level,
            status: value.status,
            source_artifact_name: value.source_artifact_name,
            decision_comment: value.decision_comment,
            decided_at: value.decided_at.map(|t| t.to_rfc3339()),
        }
    }
}
