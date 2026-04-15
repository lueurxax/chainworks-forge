use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GitReleaseError {
    #[error("worktree not found at: {path}")]
    WorktreeNotFound { path: String },
    #[error("push target '{0}' is not allowed")]
    UnsafeBranch(String),
    #[error("current branch '{actual}' does not match expected '{expected}'")]
    NotOnExpectedBranch { expected: String, actual: String },
    #[error("nothing to commit")]
    NothingToCommit,
    #[error("commit failed: {0}")]
    CommitFailed(String),
    #[error("push failed: {0}")]
    PushFailed(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReleaseManifest {
    pub commit_sha: String,
    pub branch: String,
    pub remote: String,
    pub commit_message: String,
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
    pub timestamp: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GitPushReceipt {
    pub commit_sha: String,
    pub remote: String,
    pub branch: String,
    pub status: String,
    pub failure_reason: Option<String>,
    pub timestamp: DateTime<Utc>,
}

pub struct GitReleaseService;

impl GitReleaseService {
    pub async fn commit_and_push(
        &self,
        worktree_root: &str,
        target_branch: &str,
        commit_message: &str,
    ) -> Result<(ReleaseManifest, GitPushReceipt)> {
        let worktree = Path::new(worktree_root);
        if !worktree.exists() {
            bail!(GitReleaseError::WorktreeNotFound {
                path: worktree_root.to_string(),
            });
        }

        if matches!(target_branch, "main" | "master") {
            bail!(GitReleaseError::UnsafeBranch(target_branch.to_string()));
        }

        let current_branch = self
            .run_git(&["rev-parse", "--abbrev-ref", "HEAD"], worktree)
            .await?
            .trim()
            .to_string();
        if current_branch != target_branch {
            bail!(GitReleaseError::NotOnExpectedBranch {
                expected: target_branch.to_string(),
                actual: current_branch,
            });
        }

        let status = self.run_git(&["status", "--porcelain"], worktree).await?;
        if status.trim().is_empty() {
            bail!(GitReleaseError::NothingToCommit);
        }

        let _ = self
            .run_git(
                &[
                    "-c",
                    "user.name=Chainworks Forge",
                    "-c",
                    "user.email=chainworks-forge@local",
                    "add",
                    "-A",
                ],
                worktree,
            )
            .await?;

        let commit_output = self
            .run_git(
                &[
                    "-c",
                    "user.name=Chainworks Forge",
                    "-c",
                    "user.email=chainworks-forge@local",
                    "commit",
                    "-m",
                    commit_message,
                ],
                worktree,
            )
            .await
            .map_err(|e| anyhow::anyhow!(GitReleaseError::CommitFailed(e.to_string())))?;
        if commit_output.contains("nothing to commit") {
            bail!(GitReleaseError::NothingToCommit);
        }

        let commit_sha = self
            .run_git(&["rev-parse", "HEAD"], worktree)
            .await?
            .trim()
            .to_string();
        let diff_stat = self
            .run_git(&["diff", "--stat", "HEAD~1..HEAD"], worktree)
            .await?;
        let (files_changed, insertions, deletions) = parse_diff_stat(&diff_stat);

        let remote = "origin";
        self.run_git(&["push", remote, target_branch], worktree)
            .await
            .map_err(|e| anyhow::anyhow!(GitReleaseError::PushFailed(e.to_string())))?;

        let now = Utc::now();
        let manifest = ReleaseManifest {
            commit_sha: commit_sha.clone(),
            branch: target_branch.to_string(),
            remote: remote.to_string(),
            commit_message: commit_message.to_string(),
            files_changed,
            insertions,
            deletions,
            timestamp: now,
        };
        let receipt = GitPushReceipt {
            commit_sha,
            remote: remote.to_string(),
            branch: target_branch.to_string(),
            status: "success".to_string(),
            failure_reason: None,
            timestamp: now,
        };

        Ok((manifest, receipt))
    }

    async fn run_git(&self, args: &[&str], directory: &Path) -> Result<String> {
        let output = Command::new("git")
            .current_dir(directory)
            .args(args)
            .output()
            .with_context(|| format!("spawn git {:?}", args))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(anyhow::anyhow!(
                "git {:?} failed: {}",
                args,
                if stderr.trim().is_empty() {
                    String::from_utf8_lossy(&output.stdout).to_string()
                } else {
                    stderr
                }
            ))
        }
    }
}

fn parse_diff_stat(stat: &str) -> (usize, usize, usize) {
    let mut files_changed = 0;
    let mut insertions = 0;
    let mut deletions = 0;

    for line in stat.lines().filter(|line| !line.trim().is_empty()) {
        let summary = line.trim();
        let parts: Vec<&str> = summary.split(',').collect();
        if parts.len() == 1 {
            continue;
        }
        for part in parts {
            let part = part.trim();
            if part.contains("file") {
                files_changed = part.split_whitespace().next().unwrap_or("0").parse().unwrap_or(0);
            } else if part.contains("insertion") {
                insertions = part.split_whitespace().next().unwrap_or("0").parse().unwrap_or(0);
            } else if part.contains("deletion") {
                deletions = part.split_whitespace().next().unwrap_or("0").parse().unwrap_or(0);
            }
        }
    }

    (files_changed, insertions, deletions)
}
