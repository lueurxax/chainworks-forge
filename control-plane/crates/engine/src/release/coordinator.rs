use anyhow::{bail, Result};

use crate::release::connect::{ConnectPublishService, ConnectUploadReceipt, ReleaseBundleManifest};
use crate::release::git::{GitPushReceipt, GitReleaseService, ReleaseManifest};
use domain::run::DeliveryConfiguration;

#[derive(Clone, Debug)]
pub struct ReleaseResult {
    pub git_manifest: Option<ReleaseManifest>,
    pub git_receipt: Option<GitPushReceipt>,
    pub bundle_manifest: Option<ReleaseBundleManifest>,
    pub upload_receipt: Option<ConnectUploadReceipt>,
    pub succeeded: bool,
    pub failure_stage: Option<String>,
    pub failure_reason: Option<String>,
}

pub struct ReleaseOpsCoordinator {
    git_service: GitReleaseService,
    publish_service: ConnectPublishService,
}

impl ReleaseOpsCoordinator {
    pub fn new() -> Self {
        Self {
            git_service: GitReleaseService,
            publish_service: ConnectPublishService,
        }
    }

    pub async fn execute_release(
        &self,
        delivery_config: &DeliveryConfiguration,
        worktree_root: &str,
        commit_message: &str,
    ) -> Result<ReleaseResult> {
        let target_branch = delivery_config
            .target_branch
            .as_str();
        if target_branch.trim().is_empty() {
            bail!("delivery configuration is missing a target branch");
        }

        let (git_manifest, git_receipt) = match self
            .git_service
            .commit_and_push(worktree_root, target_branch, commit_message)
            .await
        {
            Ok(result) => result,
            Err(error) => {
                return Ok(ReleaseResult {
                    git_manifest: None,
                    git_receipt: None,
                    bundle_manifest: None,
                    upload_receipt: None,
                    succeeded: false,
                    failure_stage: Some("commit_and_push".to_string()),
                    failure_reason: Some(error.to_string()),
                });
            }
        };

        match self
            .publish_service
            .build_and_distribute(worktree_root, &git_receipt, &git_manifest, delivery_config)
            .await
        {
            Ok((bundle_manifest, upload_receipt)) => Ok(ReleaseResult {
                git_manifest: Some(git_manifest),
                git_receipt: Some(git_receipt),
                bundle_manifest: Some(bundle_manifest),
                upload_receipt: Some(upload_receipt),
                succeeded: true,
                failure_stage: None,
                failure_reason: None,
            }),
            Err(error) => Ok(ReleaseResult {
                git_manifest: Some(git_manifest),
                git_receipt: Some(git_receipt),
                bundle_manifest: None,
                upload_receipt: None,
                succeeded: false,
                failure_stage: Some("build_archive_and_push".to_string()),
                failure_reason: Some(error.to_string()),
            }),
        }
    }
}
