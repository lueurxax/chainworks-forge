use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::release::coordinator::ReleaseResult;
use domain::run::{DeliveryConfiguration, Run};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeliveryReceipt {
    pub run_id: String,
    pub workflow_id: String,
    pub idea_title: String,
    pub delivery_config: DeliveryConfiguration,
    pub worktree_root: String,
    pub base_revision: Option<String>,
    pub release_result: Option<ReleaseResultSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rollout_contract_readback: Option<serde_json::Value>,
    pub implementation_review_status: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReleaseResultSummary {
    pub commit_sha: Option<String>,
    pub branch: Option<String>,
    pub remote: Option<String>,
    pub files_changed: Option<usize>,
    pub succeeded: bool,
    pub failure_stage: Option<String>,
    pub failure_reason: Option<String>,
}

pub struct DeliveryReceiptBuilder;

impl DeliveryReceiptBuilder {
    pub fn build_receipt(
        run: &Run,
        delivery_config: &DeliveryConfiguration,
        release_result: Option<&ReleaseResult>,
        rollout_contract_readback: Option<serde_json::Value>,
        idea_title: &str,
        review_status: Option<&str>,
    ) -> Option<DeliveryReceipt> {
        let worktree_root = run.worktree_root.as_ref()?.clone();
        let release_result = release_result?;

        let release_summary = Some(ReleaseResultSummary {
            commit_sha: release_result
                .git_manifest
                .as_ref()
                .map(|manifest| manifest.commit_sha.clone()),
            branch: release_result
                .git_manifest
                .as_ref()
                .map(|manifest| manifest.branch.clone()),
            remote: release_result
                .git_manifest
                .as_ref()
                .map(|manifest| manifest.remote.clone()),
            files_changed: release_result
                .git_manifest
                .as_ref()
                .map(|manifest| manifest.files_changed),
            succeeded: release_result.succeeded,
            failure_stage: release_result.failure_stage.clone(),
            failure_reason: release_result.failure_reason.clone(),
        });

        Some(DeliveryReceipt {
            run_id: run.id.to_string(),
            workflow_id: run.workflow_id.clone(),
            idea_title: idea_title.to_string(),
            delivery_config: delivery_config.clone(),
            worktree_root,
            base_revision: run.base_revision.clone(),
            release_result: release_summary,
            rollout_contract_readback,
            implementation_review_status: review_status.map(String::from),
            timestamp: Utc::now(),
        })
    }
}
