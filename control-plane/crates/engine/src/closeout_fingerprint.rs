use chrono::Utc;
use domain::closeout_readiness::CloseoutFingerprint;
use domain::run::Run;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::process::Command as TokioCommand;
use tokio::time::timeout;

pub const CLOSEOUT_FINGERPRINT_LATENCY_BUDGET_MS: u64 = 5_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloseoutWorktreeTruth {
    pub worktree_head: String,
    pub dirty_or_changed_file_digest: String,
    pub latency_ms: u64,
    pub unavailable: bool,
    pub latency_exceeded: bool,
    pub diagnostic_reason: Option<String>,
}

impl CloseoutWorktreeTruth {
    fn unavailable(reason: impl Into<String>, latency_ms: u64) -> Self {
        let reason = reason.into();
        Self {
            worktree_head: unavailable_digest("worktree-head", &reason),
            dirty_or_changed_file_digest: unavailable_digest("worktree-dirty", &reason),
            latency_ms,
            unavailable: true,
            latency_exceeded: latency_ms >= CLOSEOUT_FINGERPRINT_LATENCY_BUDGET_MS,
            diagnostic_reason: Some(reason),
        }
    }
}

pub fn build_closeout_fingerprint(
    run: &Run,
    stage_id: &str,
    worktree_head: impl Into<String>,
    dirty_or_changed_file_digest: impl Into<String>,
    upstream_active_generation_ids: Vec<String>,
    latency_ms: u64,
) -> CloseoutFingerprint {
    let workflow_digest = run
        .workflow_snapshot_hash
        .clone()
        .or_else(|| {
            run.workflow_id
                .strip_prefix("sha256:")
                .map(|_| run.workflow_id.clone())
        })
        .unwrap_or_else(|| "sha256:unknown-workflow".into());
    let proposal_or_freeze_digest = run
        .workflow_snapshot_hash
        .clone()
        .or_else(|| run.catalog_snapshot_hash.clone())
        .or_else(|| run.base_revision.clone())
        .unwrap_or_else(|| "sha256:unknown-proposal".into());

    CloseoutFingerprint {
        proposal_or_freeze_digest,
        run_id: run.id.to_string(),
        stage_id: stage_id.to_string(),
        workflow_digest,
        worktree_head: worktree_head.into(),
        dirty_or_changed_file_digest: dirty_or_changed_file_digest.into(),
        upstream_active_generation_ids,
        contract_version: "implementation_closeout_readiness_v1".into(),
        computed_at: Utc::now(),
        latency_ms,
    }
}

pub async fn resolve_closeout_worktree_truth(run: &Run) -> CloseoutWorktreeTruth {
    let started = Instant::now();
    let Some(root) = worktree_root(run) else {
        return CloseoutWorktreeTruth::unavailable("closeout worktree root unavailable", 0);
    };
    match timeout(
        Duration::from_millis(CLOSEOUT_FINGERPRINT_LATENCY_BUDGET_MS),
        read_worktree_truth(root),
    )
    .await
    {
        Ok(Ok(mut truth)) => {
            truth.latency_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
            truth.latency_exceeded = truth.latency_ms > CLOSEOUT_FINGERPRINT_LATENCY_BUDGET_MS;
            truth
        }
        Ok(Err(error)) => CloseoutWorktreeTruth::unavailable(
            format!("closeout worktree truth unavailable: {error}"),
            started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        ),
        Err(_) => CloseoutWorktreeTruth::unavailable(
            "closeout worktree truth timed out",
            CLOSEOUT_FINGERPRINT_LATENCY_BUDGET_MS,
        ),
    }
}

fn worktree_root(run: &Run) -> Option<PathBuf> {
    run.worktree_root
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            let root = run.workspace_root.trim();
            (!root.is_empty()).then_some(root)
        })
        .map(PathBuf::from)
}

async fn read_worktree_truth(root: PathBuf) -> anyhow::Result<CloseoutWorktreeTruth> {
    if !root.is_dir() {
        anyhow::bail!("{} is not a directory", root.display());
    }
    let head = run_git_text(&root, ["rev-parse", "HEAD"]).await?;
    let status = run_git_bytes(
        &root,
        ["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )
    .await?;
    let diff = run_git_bytes(&root, ["diff", "--binary", "HEAD"]).await?;
    let mut hasher = Sha256::new();
    hasher.update(b"git-status-porcelain-v1-z\0");
    hasher.update(&status);
    hasher.update(b"git-diff-binary-head\0");
    hasher.update(&diff);

    Ok(CloseoutWorktreeTruth {
        worktree_head: head.trim().to_string(),
        dirty_or_changed_file_digest: format!("sha256:{:x}", hasher.finalize()),
        latency_ms: 0,
        unavailable: false,
        latency_exceeded: false,
        diagnostic_reason: None,
    })
}

async fn run_git_text<const N: usize>(root: &Path, args: [&str; N]) -> anyhow::Result<String> {
    let bytes = run_git_bytes(root, args).await?;
    String::from_utf8(bytes).map_err(Into::into)
}

async fn run_git_bytes<const N: usize>(root: &Path, args: [&str; N]) -> anyhow::Result<Vec<u8>> {
    let output = TokioCommand::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .await?;
    if !output.status.success() {
        anyhow::bail!(
            "git command failed in {} with status {:?}",
            root.display(),
            output.status.code()
        );
    }
    Ok(output.stdout)
}

fn unavailable_digest(kind: &str, reason: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(kind.as_bytes());
    hasher.update(b"\0");
    hasher.update(reason.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain::ids::{IdeaId, RunId};
    use domain::run::{Run, RunStatus};

    fn run() -> Run {
        Run {
            id: RunId::new(),
            idea_id: IdeaId::new(),
            status: RunStatus::Running,
            workflow_id: "workflow".into(),
            workflow_title: "Workflow".into(),
            workspace_root: "/tmp/repo".into(),
            artifact_root: ".chainworks/run".into(),
            started_at: Utc::now(),
            completed_at: None,
            cancellation_requested_at: None,
            cancellation_settled_at: None,
            cancellation_settlement_log: None,
            current_state: Some("state_9_implementation_reviewed".into()),
            workflow_yaml_path: None,
            agent_catalog_yaml_path: None,
            worktree_root: Some("/tmp/repo/.chainworks/worktrees/run".into()),
            base_branch: Some("main".into()),
            base_revision: Some("sha256:base".into()),
            target_branch: Some("cw/p077".into()),
            delivery_configuration_json: None,
            delivery_preflight_json: None,
            workflow_family: None,
            project_key: None,
            risk_class: None,
            stack: None,
            workflow_snapshot_hash: Some("sha256:workflow".into()),
            catalog_snapshot_hash: Some("sha256:catalog".into()),
            workflow_snapshot_json: None,
            catalog_snapshot_json: None,
            drift_detected_at: None,
            drift_details_json: None,
            chainworks_meta_root: None,
            review_routing_json: None,
            closeout_readiness_mode: Some("enforcement".into()),
        }
    }

    #[test]
    fn closeout_fingerprint_uses_run_and_generation_truth() {
        let run = run();
        let fingerprint = build_closeout_fingerprint(
            &run,
            "state_9_implementation_reviewed",
            "abcdef",
            "sha256:dirty",
            vec!["gen-a".into(), "gen-b".into()],
            12,
        );

        assert_eq!(fingerprint.run_id, run.id.to_string());
        assert_eq!(fingerprint.stage_id, "state_9_implementation_reviewed");
        assert_eq!(fingerprint.workflow_digest, "sha256:workflow");
        assert_eq!(fingerprint.proposal_or_freeze_digest, "sha256:workflow");
        assert_eq!(fingerprint.worktree_head, "abcdef");
        assert_eq!(fingerprint.dirty_or_changed_file_digest, "sha256:dirty");
        assert_eq!(
            fingerprint.upstream_active_generation_ids,
            vec!["gen-a", "gen-b"]
        );
        assert_eq!(fingerprint.latency_ms, 12);
        assert_eq!(fingerprint.short_hash().len(), 8);
    }

    #[tokio::test]
    async fn closeout_worktree_truth_uses_live_head_and_dirty_digest() {
        let temp = tempfile::tempdir().unwrap();
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(temp.path())
            .output()
            .unwrap();
        std::fs::write(temp.path().join("tracked.txt"), "one\n").unwrap();
        std::process::Command::new("git")
            .args(["add", "tracked.txt"])
            .current_dir(temp.path())
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args([
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "commit",
                "-m",
                "init",
            ])
            .current_dir(temp.path())
            .output()
            .unwrap();

        let mut run = run();
        run.worktree_root = Some(temp.path().display().to_string());
        let clean = resolve_closeout_worktree_truth(&run).await;
        assert!(!clean.unavailable);
        assert_eq!(clean.worktree_head.len(), 40);
        assert!(clean.dirty_or_changed_file_digest.starts_with("sha256:"));

        std::fs::write(temp.path().join("tracked.txt"), "two\n").unwrap();
        let dirty = resolve_closeout_worktree_truth(&run).await;
        assert_ne!(
            dirty.dirty_or_changed_file_digest,
            clean.dirty_or_changed_file_digest
        );
    }
}
