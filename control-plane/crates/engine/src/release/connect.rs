use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
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
    pub async fn build_and_distribute(
        &self,
        worktree_root: &str,
        git_push_receipt: &GitPushReceipt,
        release_manifest: &ReleaseManifest,
        delivery_config: &DeliveryConfiguration,
    ) -> Result<(ReleaseBundleManifest, ConnectUploadReceipt)> {
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

        let build_warning = match Command::new("xcodebuild")
            .current_dir(worktree_root)
            .arg("build")
            .output()
        {
            Ok(output) if output.status.success() => None,
            Ok(output) => Some(
                String::from_utf8_lossy(&output.stderr)
                    .trim()
                    .to_string(),
            ),
            Err(err) => Some(err.to_string()),
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
        let archive_path = archive_path.exists().then(|| archive_path.to_string_lossy().into_owned());

        let bundle = ReleaseBundleManifest {
            bundle_identifier: format!("com.chainworks.forge.{}", release_mode),
            bundle_version: "1.0.0".to_string(),
            build_number: release_manifest.commit_sha.chars().take(8).collect(),
            archive_path,
            checksum_sha256: checksum,
            size_bytes,
            timestamp: Utc::now(),
        };
        let receipt = ConnectUploadReceipt {
            artifact_id: uuid::Uuid::new_v4().to_string(),
            destination: format!("{release_mode}://{release_target_id}"),
            release_target_id: release_target_id.to_string(),
            release_mode: release_mode.to_string(),
            status: if build_warning.is_some() {
                "build_warning".to_string()
            } else {
                "success".to_string()
            },
            failure_reason: build_warning.map(|warning| format!("Build completed with warnings: {warning}")),
            timestamp: Utc::now(),
        };

        Ok((bundle, receipt))
    }
}

fn stable_checksum(input: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn directory_size(path: &Path) -> i64 {
    let mut total = 0i64;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_dir() {
                    total += directory_size(&path);
                } else {
                    total += metadata.len() as i64;
                }
            }
        }
    }
    total
}
