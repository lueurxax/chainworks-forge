use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::process::Command;

pub const WORKTREE_FINGERPRINT_SCHEMA_VERSION: &str = "worktree_fingerprint_v1";
pub const WORKTREE_FINGERPRINT_CLASSIFIER_VERSION: &str = "worktree_fingerprint_classifier_v1";

#[derive(Clone, Debug)]
pub struct WorktreeFingerprintInput<'a> {
    pub worktree_root: PathBuf,
    pub run_id: String,
    pub stage_execution_id: String,
    pub agent_execution_id: String,
    pub session_generation_id: String,
    pub capture_phase: CapturePhase,
    pub active_proposal_id: Option<String>,
    pub baseline: Option<&'a WorktreeFingerprintV1>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapturePhase {
    PreOriginalPrompt,
    PostOriginalPrompt,
    PreCompletionRepair,
    PostCompletionRepair,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeFingerprintV1 {
    pub schema_version: String,
    pub run_id: String,
    pub stage_execution_id: String,
    pub agent_execution_id: String,
    pub session_generation_id: String,
    pub captured_at: DateTime<Utc>,
    pub capture_phase: CapturePhase,
    pub classifier_version: String,
    pub paths: Vec<WorktreeFingerprintPath>,
    pub summary: WorktreeFingerprintSummary,
}

impl WorktreeFingerprintV1 {
    pub fn artifact_sha256(&self) -> Result<String> {
        let mut value = serde_json::to_value(self)?;
        sort_json_object_keys(&mut value);
        let bytes = serde_json::to_vec(&value)?;
        Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeFingerprintPath {
    pub path: String,
    pub normalized_path: String,
    pub included: bool,
    pub include_or_exclude_reason: String,
    pub path_status: PathStatus,
    pub old_path: Option<String>,
    pub content_sha256: Option<String>,
    pub mode: Option<String>,
    pub size_bytes: Option<u64>,
    pub source: PathSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PathStatus {
    Clean,
    PreexistingDirty,
    NewAfterPrompt,
    ModifiedAfterPrompt,
    DeletedAfterPrompt,
    RenamedAfterPrompt,
    GeneratedMeta,
    ControlPlaneOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PathSource {
    GitDiff,
    Manifest,
    FilesystemSnapshot,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeFingerprintSummary {
    pub included_path_count: usize,
    pub excluded_path_count: usize,
    pub current_attempt_changed_path_count: usize,
    pub preexisting_dirty_path_count: usize,
    pub control_plane_only_path_count: usize,
    pub generated_artifact_only_path_count: usize,
    pub deleted_path_count: usize,
    pub renamed_path_count: usize,
    pub work_change_kind: WorkChangeKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkChangeKind {
    CurrentAttemptDiff,
    PreexistingDirtyWork,
    ControlPlaneOnlyManifest,
    GeneratedArtifactOnlyManifest,
    None,
}

pub async fn capture_worktree_fingerprint_v1(
    input: WorktreeFingerprintInput<'_>,
) -> Result<WorktreeFingerprintV1> {
    let status_entries = read_git_status(&input.worktree_root).await?;
    let baseline = input.baseline.map(baseline_by_path).unwrap_or_default();
    let mut paths = Vec::with_capacity(status_entries.len());

    for entry in status_entries {
        let normalized_path = normalize_repo_path(&entry.path);
        let classification = classify_path(&normalized_path, input.active_proposal_id.as_deref());
        let mut path_status = if classification.included {
            classify_included_status(input.capture_phase, &entry, baseline.get(&normalized_path))
        } else {
            classification.excluded_status
        };

        if !classification.included {
            path_status = classification.excluded_status;
        }

        let file_path = input.worktree_root.join(&normalized_path);
        let (content_sha256, mode, size_bytes) = read_existing_file_metadata(&file_path)?;

        paths.push(WorktreeFingerprintPath {
            path: entry.path,
            normalized_path,
            included: classification.included,
            include_or_exclude_reason: classification.reason,
            path_status,
            old_path: entry.old_path.map(|path| normalize_repo_path(&path)),
            content_sha256,
            mode,
            size_bytes,
            source: PathSource::GitDiff,
        });
    }

    paths.sort_by(|a, b| {
        a.normalized_path
            .as_bytes()
            .cmp(b.normalized_path.as_bytes())
    });
    let summary = derive_summary(&paths);

    Ok(WorktreeFingerprintV1 {
        schema_version: WORKTREE_FINGERPRINT_SCHEMA_VERSION.into(),
        run_id: input.run_id,
        stage_execution_id: input.stage_execution_id,
        agent_execution_id: input.agent_execution_id,
        session_generation_id: input.session_generation_id,
        captured_at: Utc::now(),
        capture_phase: input.capture_phase,
        classifier_version: WORKTREE_FINGERPRINT_CLASSIFIER_VERSION.into(),
        paths,
        summary,
    })
}

fn baseline_by_path(
    fingerprint: &WorktreeFingerprintV1,
) -> HashMap<String, &WorktreeFingerprintPath> {
    fingerprint
        .paths
        .iter()
        .map(|path| (path.normalized_path.clone(), path))
        .collect()
}

fn classify_included_status(
    capture_phase: CapturePhase,
    entry: &GitStatusEntry,
    baseline: Option<&&WorktreeFingerprintPath>,
) -> PathStatus {
    if matches!(
        capture_phase,
        CapturePhase::PreOriginalPrompt | CapturePhase::PreCompletionRepair
    ) {
        return PathStatus::PreexistingDirty;
    }

    if let Some(pre) = baseline {
        if pre.included
            && pre.content_sha256 == entry.content_sha256
            && pre.old_path
                == entry
                    .old_path
                    .as_ref()
                    .map(|path| normalize_repo_path(path))
        {
            return PathStatus::PreexistingDirty;
        }
        if entry.is_untracked {
            return PathStatus::NewAfterPrompt;
        }
        if entry.is_renamed {
            return PathStatus::RenamedAfterPrompt;
        }
        if entry.is_deleted {
            return PathStatus::DeletedAfterPrompt;
        }
        return PathStatus::ModifiedAfterPrompt;
    }

    if entry.is_renamed {
        return PathStatus::RenamedAfterPrompt;
    }
    if entry.is_deleted {
        return PathStatus::DeletedAfterPrompt;
    }

    if entry.is_untracked {
        PathStatus::NewAfterPrompt
    } else {
        PathStatus::ModifiedAfterPrompt
    }
}

fn classify_path(normalized_path: &str, active_proposal_id: Option<&str>) -> PathClassification {
    if normalized_path.starts_with(".chainworks/")
        || normalized_path.starts_with(".review-baselines/")
        || normalized_path.starts_with(".codex/")
    {
        return PathClassification::excluded(
            "control-plane generated metadata",
            PathStatus::ControlPlaneOnly,
        );
    }

    if is_generated_artifact_path(normalized_path, active_proposal_id) {
        return PathClassification::excluded(
            "generated artifact outside deterministic proposal-owned fixtures",
            PathStatus::GeneratedMeta,
        );
    }

    if is_implementation_owned_path(normalized_path, active_proposal_id) {
        return PathClassification::included("implementation-owned path");
    }

    PathClassification::excluded(
        "outside implementation-owned path set",
        PathStatus::GeneratedMeta,
    )
}

fn is_implementation_owned_path(normalized_path: &str, active_proposal_id: Option<&str>) -> bool {
    normalized_path.starts_with("Chainworks Forge/")
        || normalized_path.starts_with("Chainworks ForgeTests/")
        || normalized_path.starts_with("Chainworks ForgeUITests/")
        || normalized_path.starts_with("control-plane/")
        || normalized_path.starts_with("examples/workflows/")
        || normalized_path.starts_with("examples/agents/")
        || normalized_path.starts_with("scripts/")
        || normalized_path.starts_with("docs/reference/")
        || active_proposal_id
            .map(|proposal_id| {
                normalized_path.starts_with(&format!("docs/proposals/{proposal_id}-artifacts/"))
                    || normalized_path.starts_with(&format!("docs/evidence/{proposal_id}/"))
            })
            .unwrap_or(false)
}

fn is_generated_artifact_path(normalized_path: &str, active_proposal_id: Option<&str>) -> bool {
    if normalized_path
        == "docs/proposals/088-code-writer-completion-contract-and-output-freshness.md"
    {
        return true;
    }

    if normalized_path == "state/run-state.json"
        || normalized_path == "artifacts/active-index.json"
        || normalized_path == "review/implementation-summary.json"
        || normalized_path.contains("changed_files_manifest")
        || normalized_path.contains("runtime-receipt")
        || normalized_path.contains("runtime_receipt")
        || normalized_path.contains("validation_failure")
        || normalized_path.contains("failed-stage-evidence")
    {
        return !is_proposal_owned_evidence_fixture(normalized_path, active_proposal_id);
    }

    normalized_path.starts_with("docs/evidence/")
        && !is_proposal_owned_evidence_fixture(normalized_path, active_proposal_id)
}

fn is_proposal_owned_evidence_fixture(
    normalized_path: &str,
    active_proposal_id: Option<&str>,
) -> bool {
    active_proposal_id
        .map(|proposal_id| normalized_path.starts_with(&format!("docs/evidence/{proposal_id}/")))
        .unwrap_or(false)
}

fn derive_summary(paths: &[WorktreeFingerprintPath]) -> WorktreeFingerprintSummary {
    let included_path_count = paths.iter().filter(|path| path.included).count();
    let excluded_path_count = paths.len().saturating_sub(included_path_count);
    let current_attempt_changed_path_count = paths
        .iter()
        .filter(|path| path.included && is_current_attempt_status(path.path_status))
        .count();
    let preexisting_dirty_path_count = paths
        .iter()
        .filter(|path| path.included && path.path_status == PathStatus::PreexistingDirty)
        .count();
    let control_plane_only_path_count = paths
        .iter()
        .filter(|path| path.path_status == PathStatus::ControlPlaneOnly)
        .count();
    let generated_artifact_only_path_count = paths
        .iter()
        .filter(|path| path.path_status == PathStatus::GeneratedMeta)
        .count();
    let deleted_path_count = paths
        .iter()
        .filter(|path| path.path_status == PathStatus::DeletedAfterPrompt)
        .count();
    let renamed_path_count = paths
        .iter()
        .filter(|path| path.path_status == PathStatus::RenamedAfterPrompt)
        .count();

    let work_change_kind = if current_attempt_changed_path_count > 0 {
        WorkChangeKind::CurrentAttemptDiff
    } else if preexisting_dirty_path_count > 0 {
        WorkChangeKind::PreexistingDirtyWork
    } else if control_plane_only_path_count > 0 {
        WorkChangeKind::ControlPlaneOnlyManifest
    } else if generated_artifact_only_path_count > 0 {
        WorkChangeKind::GeneratedArtifactOnlyManifest
    } else {
        WorkChangeKind::None
    };

    WorktreeFingerprintSummary {
        included_path_count,
        excluded_path_count,
        current_attempt_changed_path_count,
        preexisting_dirty_path_count,
        control_plane_only_path_count,
        generated_artifact_only_path_count,
        deleted_path_count,
        renamed_path_count,
        work_change_kind,
    }
}

fn is_current_attempt_status(status: PathStatus) -> bool {
    matches!(
        status,
        PathStatus::NewAfterPrompt
            | PathStatus::ModifiedAfterPrompt
            | PathStatus::DeletedAfterPrompt
            | PathStatus::RenamedAfterPrompt
    )
}

#[derive(Clone, Debug)]
struct PathClassification {
    included: bool,
    reason: String,
    excluded_status: PathStatus,
}

impl PathClassification {
    fn included(reason: impl Into<String>) -> Self {
        Self {
            included: true,
            reason: reason.into(),
            excluded_status: PathStatus::Clean,
        }
    }

    fn excluded(reason: impl Into<String>, excluded_status: PathStatus) -> Self {
        Self {
            included: false,
            reason: reason.into(),
            excluded_status,
        }
    }
}

#[derive(Clone, Debug)]
struct GitStatusEntry {
    path: String,
    old_path: Option<String>,
    is_untracked: bool,
    is_deleted: bool,
    is_renamed: bool,
    content_sha256: Option<String>,
}

async fn read_git_status(root: &Path) -> Result<Vec<GitStatusEntry>> {
    let output = Command::new("git")
        .args(["status", "--porcelain=v1", "-z", "--untracked-files=all"])
        .current_dir(root)
        .output()
        .await
        .with_context(|| format!("running git status in {}", root.display()))?;
    if !output.status.success() {
        anyhow::bail!(
            "git status failed in {}: {}",
            root.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    parse_git_status(root, &output.stdout)
}

fn parse_git_status(root: &Path, bytes: &[u8]) -> Result<Vec<GitStatusEntry>> {
    let mut entries = Vec::new();
    let mut fields = bytes
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty());

    while let Some(field) = fields.next() {
        if field.len() < 4 {
            continue;
        }
        let code = std::str::from_utf8(&field[0..2]).context("git status code was not utf-8")?;
        let path = std::str::from_utf8(&field[3..])
            .context("git status path was not utf-8")?
            .to_string();
        let is_renamed = code.contains('R');
        let old_path = if is_renamed {
            fields
                .next()
                .map(|old| std::str::from_utf8(old).unwrap_or_default().to_string())
        } else {
            None
        };
        let normalized_path = normalize_repo_path(&path);
        let content_sha256 = read_file_sha256(&root.join(&normalized_path))?;
        entries.push(GitStatusEntry {
            path,
            old_path,
            is_untracked: code == "??",
            is_deleted: code.contains('D'),
            is_renamed,
            content_sha256,
        });
    }

    Ok(entries)
}

fn read_existing_file_metadata(
    path: &Path,
) -> Result<(Option<String>, Option<String>, Option<u64>)> {
    if !path.is_file() {
        return Ok((None, None, None));
    }
    let metadata = std::fs::metadata(path)
        .with_context(|| format!("reading metadata for {}", path.display()))?;
    Ok((
        read_file_sha256(path)?,
        Some(format_mode(&metadata)),
        Some(metadata.len()),
    ))
}

fn read_file_sha256(path: &Path) -> Result<Option<String>> {
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    Ok(Some(format!("sha256:{:x}", Sha256::digest(bytes))))
}

#[cfg(unix)]
fn format_mode(metadata: &std::fs::Metadata) -> String {
    use std::os::unix::fs::PermissionsExt;
    format!("{:o}", metadata.permissions().mode() & 0o7777)
}

#[cfg(not(unix))]
fn format_mode(_metadata: &std::fs::Metadata) -> String {
    "unknown".into()
}

fn normalize_repo_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_string()
}

fn sort_json_object_keys(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<_> = std::mem::take(map).into_iter().collect();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            for (key, mut child) in entries {
                sort_json_object_keys(&mut child);
                map.insert(key, child);
            }
        }
        Value::Array(items) => {
            for item in items {
                sort_json_object_keys(item);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tokio::process::Command;

    #[tokio::test]
    async fn proposal_088_inherited_dirty_work_stays_preexisting_when_post_prompt_unchanged() {
        let repo = temp_repo().await;
        write_file(
            repo.path(),
            "control-plane/crates/engine/src/lib.rs",
            "dirty\n",
        );

        let pre = capture_worktree_fingerprint_v1(WorktreeFingerprintInput {
            worktree_root: repo.path().to_path_buf(),
            run_id: "run".into(),
            stage_execution_id: "stage".into(),
            agent_execution_id: "agent".into(),
            session_generation_id: "generation".into(),
            capture_phase: CapturePhase::PreOriginalPrompt,
            active_proposal_id: Some("088".into()),
            baseline: None,
        })
        .await
        .unwrap();

        let post = capture_worktree_fingerprint_v1(WorktreeFingerprintInput {
            capture_phase: CapturePhase::PostOriginalPrompt,
            baseline: Some(&pre),
            ..input_for(repo.path())
        })
        .await
        .unwrap();

        assert_eq!(
            post.summary.work_change_kind,
            WorkChangeKind::PreexistingDirtyWork
        );
        assert_eq!(post.summary.preexisting_dirty_path_count, 1);
        assert_eq!(post.summary.current_attempt_changed_path_count, 0);
        assert_eq!(post.paths[0].path_status, PathStatus::PreexistingDirty);
    }

    #[tokio::test]
    async fn proposal_088_new_and_modified_paths_after_prompt_are_current_attempt_diff() {
        let repo = temp_repo().await;
        let pre = capture_worktree_fingerprint_v1(WorktreeFingerprintInput {
            capture_phase: CapturePhase::PreOriginalPrompt,
            baseline: None,
            ..input_for(repo.path())
        })
        .await
        .unwrap();

        write_file(
            repo.path(),
            "control-plane/crates/engine/src/worktree_fingerprint.rs",
            "modified\n",
        );
        write_file(repo.path(), "scripts/p088-proof.sh", "#!/bin/sh\n");

        let post = capture_worktree_fingerprint_v1(WorktreeFingerprintInput {
            capture_phase: CapturePhase::PostOriginalPrompt,
            baseline: Some(&pre),
            ..input_for(repo.path())
        })
        .await
        .unwrap();

        assert_eq!(
            post.summary.work_change_kind,
            WorkChangeKind::CurrentAttemptDiff
        );
        assert_eq!(post.summary.current_attempt_changed_path_count, 2);
        assert!(post
            .paths
            .iter()
            .any(|path| path.normalized_path == "scripts/p088-proof.sh"
                && path.path_status == PathStatus::NewAfterPrompt));
        assert!(post.paths.iter().any(|path| path.normalized_path
            == "control-plane/crates/engine/src/worktree_fingerprint.rs"
            && path.path_status == PathStatus::ModifiedAfterPrompt));
    }

    #[tokio::test]
    async fn proposal_088_inherited_deleted_and_renamed_paths_stay_preexisting_dirty() {
        let repo = temp_repo().await;
        write_file(repo.path(), "scripts/rename-me.sh", "rename\n");
        run_git(repo.path(), &["add", "."]).await;
        run_git(repo.path(), &["commit", "-m", "rename seed"]).await;
        std::fs::remove_file(
            repo.path()
                .join("control-plane/crates/engine/src/worktree_fingerprint.rs"),
        )
        .unwrap();
        run_git(
            repo.path(),
            &["mv", "scripts/rename-me.sh", "scripts/renamed.sh"],
        )
        .await;

        let pre = capture_worktree_fingerprint_v1(WorktreeFingerprintInput {
            capture_phase: CapturePhase::PreOriginalPrompt,
            baseline: None,
            ..input_for(repo.path())
        })
        .await
        .unwrap();
        let post = capture_worktree_fingerprint_v1(WorktreeFingerprintInput {
            capture_phase: CapturePhase::PostOriginalPrompt,
            baseline: Some(&pre),
            ..input_for(repo.path())
        })
        .await
        .unwrap();

        assert_eq!(
            post.summary.work_change_kind,
            WorkChangeKind::PreexistingDirtyWork
        );
        assert_eq!(post.summary.current_attempt_changed_path_count, 0);
        assert_eq!(post.summary.preexisting_dirty_path_count, 2);
        assert!(post.paths.iter().any(|path| path.normalized_path
            == "control-plane/crates/engine/src/worktree_fingerprint.rs"
            && path.path_status == PathStatus::PreexistingDirty));
        assert!(post
            .paths
            .iter()
            .any(|path| path.normalized_path == "scripts/renamed.sh"
                && path.path_status == PathStatus::PreexistingDirty));
    }

    #[tokio::test]
    async fn proposal_088_control_plane_and_generated_paths_are_excluded_and_counted() {
        let repo = temp_repo().await;
        write_file(
            repo.path(),
            ".chainworks/runs/run/state/run-state.json",
            "{}\n",
        );
        write_file(repo.path(), "artifacts/changed_files_manifest.json", "{}\n");

        let fingerprint = capture_worktree_fingerprint_v1(WorktreeFingerprintInput {
            capture_phase: CapturePhase::PostOriginalPrompt,
            baseline: None,
            ..input_for(repo.path())
        })
        .await
        .unwrap();

        assert_eq!(fingerprint.summary.included_path_count, 0);
        assert_eq!(fingerprint.summary.excluded_path_count, 2);
        assert_eq!(fingerprint.summary.control_plane_only_path_count, 1);
        assert_eq!(fingerprint.summary.generated_artifact_only_path_count, 1);
        assert_eq!(
            fingerprint.summary.work_change_kind,
            WorkChangeKind::ControlPlaneOnlyManifest
        );
        assert!(fingerprint
            .paths
            .iter()
            .all(|path| !path.included && path.content_sha256.is_some()));
    }

    #[tokio::test]
    async fn proposal_088_paths_are_deterministically_ordered_and_summary_is_derived() {
        let repo = temp_repo().await;
        write_file(repo.path(), "scripts/zeta.sh", "z\n");
        write_file(repo.path(), "scripts/alpha.sh", "a\n");
        write_file(
            repo.path(),
            "docs/evidence/tmp/runtime-receipt.json",
            "{}\n",
        );

        let fingerprint = capture_worktree_fingerprint_v1(WorktreeFingerprintInput {
            capture_phase: CapturePhase::PostOriginalPrompt,
            active_proposal_id: Some("088".into()),
            baseline: None,
            ..input_for(repo.path())
        })
        .await
        .unwrap();

        let paths: Vec<_> = fingerprint
            .paths
            .iter()
            .map(|path| path.normalized_path.as_str())
            .collect();
        assert_eq!(
            paths,
            vec![
                "docs/evidence/tmp/runtime-receipt.json",
                "scripts/alpha.sh",
                "scripts/zeta.sh"
            ]
        );
        assert_eq!(fingerprint.summary.included_path_count, 2);
        assert_eq!(fingerprint.summary.excluded_path_count, 1);
        assert_eq!(
            fingerprint.summary.current_attempt_changed_path_count,
            fingerprint
                .paths
                .iter()
                .filter(|path| matches!(
                    path.path_status,
                    PathStatus::NewAfterPrompt
                        | PathStatus::ModifiedAfterPrompt
                        | PathStatus::DeletedAfterPrompt
                        | PathStatus::RenamedAfterPrompt
                ))
                .count()
        );
    }

    fn input_for(root: &Path) -> WorktreeFingerprintInput<'_> {
        WorktreeFingerprintInput {
            worktree_root: root.to_path_buf(),
            run_id: "run".into(),
            stage_execution_id: "stage".into(),
            agent_execution_id: "agent".into(),
            session_generation_id: "generation".into(),
            capture_phase: CapturePhase::PostOriginalPrompt,
            active_proposal_id: Some("088".into()),
            baseline: None,
        }
    }

    async fn temp_repo() -> tempfile::TempDir {
        let repo = tempfile::tempdir().unwrap();
        run_git(repo.path(), &["init"]).await;
        run_git(repo.path(), &["config", "user.email", "test@example.com"]).await;
        run_git(repo.path(), &["config", "user.name", "Test User"]).await;
        write_file(
            repo.path(),
            "control-plane/crates/engine/src/worktree_fingerprint.rs",
            "initial\n",
        );
        run_git(repo.path(), &["add", "."]).await;
        run_git(repo.path(), &["commit", "-m", "initial"]).await;
        repo
    }

    fn write_file(root: &Path, relative: &str, contents: &str) {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    async fn run_git(dir: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .await
            .unwrap();
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
