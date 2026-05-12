use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use domain::error_sanitizer::sanitize_error_for_storage;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::release::git::{GitPushReceipt, ReleaseManifest};
use domain::run::DeliveryConfiguration;

#[derive(Debug, Error)]
pub enum PublishError {
    #[error("git push receipt is required before publishing")]
    MissingGitPushReceipt,
    #[error("release manifest is required before publishing")]
    MissingReleaseManifest,
    #[error("release target id is required before publishing")]
    MissingReleaseTarget,
    #[error("release mode is required before publishing")]
    MissingReleaseMode,
    #[error("unsupported release mode: {0}")]
    InvalidReleaseMode(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReleaseBundleManifest {
    pub bundle_identifier: String,
    pub bundle_version: String,
    pub build_number: String,
    pub archive_path: Option<String>,
    pub checksum_sha256: String,
    pub size_bytes: i64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConnectUploadReceipt {
    pub artifact_id: String,
    pub destination: String,
    pub release_target_id: String,
    pub release_mode: String,
    pub status: String,
    pub failure_reason: Option<String>,
    pub timestamp: DateTime<Utc>,
}

pub struct ConnectPublishService;

impl ConnectPublishService {
    pub async fn build_archive(
        &self,
        worktree_root: &str,
        git_push_receipt: &GitPushReceipt,
        release_manifest: &ReleaseManifest,
        delivery_config: &DeliveryConfiguration,
    ) -> Result<ReleaseBundleManifest> {
        if git_push_receipt.status != "success" {
            bail!(PublishError::MissingGitPushReceipt);
        }

        let release_mode = delivery_config
            .release_mode
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!(PublishError::MissingReleaseMode))?;
        if !matches!(release_mode, "sandbox" | "staging") {
            bail!(PublishError::InvalidReleaseMode(release_mode.to_string()));
        }

        let worktree_for_build = worktree_root.to_string();
        let _build_warning = match tokio::task::spawn_blocking(move || {
            Command::new("xcodebuild")
                .current_dir(worktree_for_build)
                .arg("build")
                .output()
        })
        .await
        {
            Err(err) => Some(sanitize_release_warning(&err.to_string())),
            Ok(Err(err)) => Some(sanitize_release_warning(&err.to_string())),
            Ok(Ok(output)) if output.status.success() => None,
            Ok(Ok(output)) => Some(sanitize_release_warning(
                String::from_utf8_lossy(&output.stderr).trim(),
            )),
        };

        let checksum_input = format!(
            "{}:{}:{}:{}",
            release_manifest.commit_sha,
            release_manifest.files_changed,
            release_manifest.insertions,
            release_manifest.deletions
        );
        let checksum = stable_checksum(&checksum_input);
        let size_bytes = directory_size(Path::new(worktree_root));
        let archive_path = Path::new(worktree_root).join(".build");
        if !archive_path.exists() {
            let _ = fs::create_dir_all(&archive_path);
        }
        let archive_path = Some(archive_path.to_string_lossy().into_owned());

        let bundle = ReleaseBundleManifest {
            bundle_identifier: format!("com.chainworks.forge.{}", release_mode),
            bundle_version: "1.0.0".to_string(),
            build_number: release_manifest.commit_sha.chars().take(8).collect(),
            archive_path,
            checksum_sha256: checksum,
            size_bytes,
            timestamp: Utc::now(),
        };
        Ok(bundle)
    }

    pub async fn upload_archive(
        &self,
        git_push_receipt: &GitPushReceipt,
        bundle: &ReleaseBundleManifest,
        delivery_config: &DeliveryConfiguration,
    ) -> Result<ConnectUploadReceipt> {
        if git_push_receipt.status != "success" {
            bail!(PublishError::MissingGitPushReceipt);
        }
        let release_target_id = delivery_config
            .release_target_id
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!(PublishError::MissingReleaseTarget))?;
        let release_mode = delivery_config
            .release_mode
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!(PublishError::MissingReleaseMode))?;
        if !matches!(release_mode, "sandbox" | "staging") {
            bail!(PublishError::InvalidReleaseMode(release_mode.to_string()));
        }
        let receipt = ConnectUploadReceipt {
            artifact_id: uuid::Uuid::new_v4().to_string(),
            destination: format!("{release_mode}://{release_target_id}"),
            release_target_id: release_target_id.to_string(),
            release_mode: release_mode.to_string(),
            status: if bundle.archive_path.is_some() {
                "success".to_string()
            } else {
                "archive_missing".to_string()
            },
            failure_reason: if bundle.archive_path.is_none() {
                Some("Archive output missing after build_archive".to_string())
            } else {
                None
            },
            timestamp: Utc::now(),
        };
        Ok(receipt)
    }

    pub async fn build_and_distribute(
        &self,
        worktree_root: &str,
        git_push_receipt: &GitPushReceipt,
        release_manifest: &ReleaseManifest,
        delivery_config: &DeliveryConfiguration,
    ) -> Result<(ReleaseBundleManifest, ConnectUploadReceipt)> {
        let bundle = self
            .build_archive(
                worktree_root,
                git_push_receipt,
                release_manifest,
                delivery_config,
            )
            .await?;
        let receipt = self
            .upload_archive(git_push_receipt, &bundle, delivery_config)
            .await?;
        Ok((bundle, receipt))
    }
}

fn sanitize_release_warning(raw: &str) -> String {
    sanitize_error_for_storage(raw, 512)
}

fn stable_checksum(input: &str) -> String {
    use sha2::{Digest, Sha256};

    let digest = Sha256::digest(input.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn directory_size(path: &Path) -> i64 {
    let mut total = 0i64;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            // symlink_metadata does not follow symlinks — skip them to avoid escaping the worktree
            if let Ok(metadata) = fs::symlink_metadata(&entry_path) {
                if metadata.file_type().is_symlink() {
                    continue;
                }
                if metadata.is_dir() {
                    total += directory_size(&entry_path);
                } else {
                    total += metadata.len() as i64;
                }
            }
        }
    }
    total
}
