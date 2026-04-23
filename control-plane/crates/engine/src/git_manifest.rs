use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::time::timeout;

use crate::contracts::DeclaredOutput;

const CHANGED_FILES_MANIFEST_OUTPUT: &str = "changed_files_manifest";
const GIT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitManifestStatus {
    Available,
    Timeout,
    NotGitRepository,
    CommandFailed,
    NotDeclared,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangedFilesManifest {
    pub schema_version: String,
    pub status: GitManifestStatus,
    pub files: Vec<String>,
    pub staged_changes: Vec<String>,
    pub unstaged_changes: Vec<String>,
    pub deleted_files: Vec<String>,
    pub renamed_files: Vec<String>,
    pub conflicted_files: Vec<String>,
    pub untracked_files: Vec<String>,
    pub diff_stat_text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preserved_agent_manifest_path: Option<String>,
    pub diagnostics: Vec<String>,
}

pub async fn generate_changed_files_manifest_if_declared(
    declared_outputs: &[DeclaredOutput],
    worktree_root: Option<&str>,
    worktree_write_enabled: bool,
) -> Result<Option<GitManifestStatus>> {
    let Some(declared) = declared_outputs
        .iter()
        .find(|declared| declared.output_name == CHANGED_FILES_MANIFEST_OUTPUT)
    else {
        return Ok(Some(GitManifestStatus::NotDeclared));
    };

    let target_path = Path::new(&declared.target_path);
    let Some(worktree_root) = worktree_root.filter(|_| worktree_write_enabled) else {
        write_manifest(
            target_path,
            ChangedFilesManifest::with_status(
                GitManifestStatus::NotGitRepository,
                vec!["changed_files_manifest requires a write-enabled worktree".to_string()],
            ),
        )?;
        return Ok(Some(GitManifestStatus::NotGitRepository));
    };

    let result = GitManifestRunner::new(PathBuf::from(worktree_root))
        .run()
        .await;
    let mut manifest = match result {
        Ok(manifest) => manifest,
        Err(error) => ChangedFilesManifest::with_status(
            GitManifestStatus::CommandFailed,
            vec![error.to_string()],
        ),
    };
    manifest.preserved_agent_manifest_path = preserve_agent_manifest(target_path)?;
    let status = manifest.status.clone();
    write_manifest(target_path, manifest)?;
    Ok(Some(status))
}

#[derive(Clone, Debug)]
pub struct GitManifestRunner {
    worktree_root: PathBuf,
    timeout: Duration,
}

impl GitManifestRunner {
    pub fn new(worktree_root: PathBuf) -> Self {
        Self {
            worktree_root,
            timeout: GIT_TIMEOUT,
        }
    }

    pub async fn run(&self) -> Result<ChangedFilesManifest> {
        if !self.worktree_root.is_dir() {
            return Ok(ChangedFilesManifest::with_status(
                GitManifestStatus::NotGitRepository,
                vec![format!(
                    "worktree root does not exist: {}",
                    self.worktree_root.display()
                )],
            ));
        }

        let inside_worktree = match self
            .run_git_text(&["rev-parse", "--is-inside-work-tree"])
            .await
        {
            Ok(output) => output.trim() == "true",
            Err(GitCommandError::NotGitRepository(_)) => {
                return Ok(ChangedFilesManifest::with_status(
                    GitManifestStatus::NotGitRepository,
                    vec![format!(
                        "not a git repository: {}",
                        self.worktree_root.display()
                    )],
                ));
            }
            Err(GitCommandError::Timeout) => {
                return Ok(ChangedFilesManifest::with_status(
                    GitManifestStatus::Timeout,
                    vec!["git rev-parse timed out".to_string()],
                ));
            }
            Err(GitCommandError::CommandFailed(message)) => {
                return Ok(ChangedFilesManifest::with_status(
                    GitManifestStatus::CommandFailed,
                    vec![message],
                ));
            }
        };
        if !inside_worktree {
            return Ok(ChangedFilesManifest::with_status(
                GitManifestStatus::NotGitRepository,
                vec![format!(
                    "git rev-parse reported outside worktree: {}",
                    self.worktree_root.display()
                )],
            ));
        }
        let git_top_level = match self.run_git_text(&["rev-parse", "--show-toplevel"]).await {
            Ok(output) => output.trim().to_string(),
            Err(GitCommandError::Timeout) => {
                return Ok(ChangedFilesManifest::with_status(
                    GitManifestStatus::Timeout,
                    vec!["git rev-parse --show-toplevel timed out".to_string()],
                ));
            }
            Err(GitCommandError::NotGitRepository(message)) => {
                return Ok(ChangedFilesManifest::with_status(
                    GitManifestStatus::NotGitRepository,
                    vec![message],
                ));
            }
            Err(GitCommandError::CommandFailed(message)) => {
                return Ok(ChangedFilesManifest::with_status(
                    GitManifestStatus::CommandFailed,
                    vec![message],
                ));
            }
        };
        let requested_root = std::fs::canonicalize(&self.worktree_root)
            .unwrap_or_else(|_| self.worktree_root.clone());
        let repository_root =
            std::fs::canonicalize(&git_top_level).unwrap_or_else(|_| PathBuf::from(&git_top_level));
        if repository_root != requested_root {
            return Ok(ChangedFilesManifest::with_status(
                GitManifestStatus::NotGitRepository,
                vec![format!(
                    "git repository root {} does not match requested worktree root {}",
                    repository_root.display(),
                    requested_root.display()
                )],
            ));
        }

        let status_output = match self
            .run_git_bytes(&["status", "--porcelain=v1", "-z", "--untracked-files=all"])
            .await
        {
            Ok(output) => output,
            Err(GitCommandError::Timeout) => {
                return Ok(ChangedFilesManifest::with_status(
                    GitManifestStatus::Timeout,
                    vec!["git status timed out".to_string()],
                ));
            }
            Err(GitCommandError::NotGitRepository(message)) => {
                return Ok(ChangedFilesManifest::with_status(
                    GitManifestStatus::NotGitRepository,
                    vec![message],
                ));
            }
            Err(GitCommandError::CommandFailed(message)) => {
                return Ok(ChangedFilesManifest::with_status(
                    GitManifestStatus::CommandFailed,
                    vec![message],
                ));
            }
        };

        let diff_names = match self.run_git_bytes(&["diff", "--name-only", "-z"]).await {
            Ok(output) => parse_nul_paths(&output),
            Err(GitCommandError::Timeout) => {
                return Ok(ChangedFilesManifest::with_status(
                    GitManifestStatus::Timeout,
                    vec!["git diff --name-only timed out".to_string()],
                ));
            }
            Err(GitCommandError::NotGitRepository(message)) => {
                return Ok(ChangedFilesManifest::with_status(
                    GitManifestStatus::NotGitRepository,
                    vec![message],
                ));
            }
            Err(GitCommandError::CommandFailed(message)) => {
                return Ok(ChangedFilesManifest::with_status(
                    GitManifestStatus::CommandFailed,
                    vec![message],
                ));
            }
        };

        let diff_stat_text = match self.run_git_text(&["diff", "--stat"]).await {
            Ok(output) => output,
            Err(GitCommandError::Timeout) => {
                return Ok(ChangedFilesManifest::with_status(
                    GitManifestStatus::Timeout,
                    vec!["git diff --stat timed out".to_string()],
                ));
            }
            Err(GitCommandError::NotGitRepository(message)) => {
                return Ok(ChangedFilesManifest::with_status(
                    GitManifestStatus::NotGitRepository,
                    vec![message],
                ));
            }
            Err(GitCommandError::CommandFailed(message)) => {
                return Ok(ChangedFilesManifest::with_status(
                    GitManifestStatus::CommandFailed,
                    vec![message],
                ));
            }
        };

        let mut manifest = parse_porcelain_status(&status_output);
        for path in diff_names {
            push_unique(&mut manifest.files, path);
        }
        manifest.diff_stat_text = diff_stat_text;
        Ok(manifest)
    }

    async fn run_git_text(&self, args: &[&str]) -> Result<String, GitCommandError> {
        let bytes = self.run_git_bytes(args).await?;
        Ok(String::from_utf8_lossy(&bytes).to_string())
    }

    async fn run_git_bytes(&self, args: &[&str]) -> Result<Vec<u8>, GitCommandError> {
        let child = Command::new("git")
            .args(args)
            .current_dir(&self.worktree_root)
            .kill_on_drop(true)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|error| GitCommandError::CommandFailed(error.to_string()))?;

        let output = match timeout(self.timeout, child.wait_with_output()).await {
            Ok(output) => {
                output.map_err(|error| GitCommandError::CommandFailed(error.to_string()))?
            }
            Err(_) => return Err(GitCommandError::Timeout),
        };

        if output.status.success() {
            return Ok(output.stdout);
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let message = if stderr.is_empty() { stdout } else { stderr };
        if message.contains("not a git repository") {
            Err(GitCommandError::NotGitRepository(message))
        } else {
            Err(GitCommandError::CommandFailed(format!(
                "git {} failed (exit {}): {}",
                args.join(" "),
                output.status,
                message
            )))
        }
    }
}

#[derive(Debug)]
enum GitCommandError {
    Timeout,
    NotGitRepository(String),
    CommandFailed(String),
}

impl ChangedFilesManifest {
    fn available() -> Self {
        Self {
            schema_version: "changed_files_manifest_v1".to_string(),
            status: GitManifestStatus::Available,
            files: Vec::new(),
            staged_changes: Vec::new(),
            unstaged_changes: Vec::new(),
            deleted_files: Vec::new(),
            renamed_files: Vec::new(),
            conflicted_files: Vec::new(),
            untracked_files: Vec::new(),
            diff_stat_text: String::new(),
            preserved_agent_manifest_path: None,
            diagnostics: Vec::new(),
        }
    }

    fn with_status(status: GitManifestStatus, diagnostics: Vec<String>) -> Self {
        Self {
            status,
            diagnostics,
            ..Self::available()
        }
    }
}

fn parse_porcelain_status(bytes: &[u8]) -> ChangedFilesManifest {
    let mut manifest = ChangedFilesManifest::available();
    let entries: Vec<&[u8]> = bytes
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .collect();
    let mut index = 0usize;
    while index < entries.len() {
        let entry = entries[index];
        if entry.len() < 4 {
            index += 1;
            continue;
        }
        let x = entry[0] as char;
        let y = entry[1] as char;
        let path = String::from_utf8_lossy(&entry[3..]).to_string();
        if path.is_empty() {
            index += 1;
            continue;
        }

        push_unique(&mut manifest.files, path.clone());
        if x == '?' && y == '?' {
            push_unique(&mut manifest.untracked_files, path);
            index += 1;
            continue;
        }
        if is_conflicted_status(x, y) {
            push_unique(&mut manifest.conflicted_files, path.clone());
        }
        if x != ' ' && x != '?' {
            push_unique(&mut manifest.staged_changes, path.clone());
        }
        if y != ' ' && y != '?' {
            push_unique(&mut manifest.unstaged_changes, path.clone());
        }
        if x == 'D' || y == 'D' {
            push_unique(&mut manifest.deleted_files, path.clone());
        }
        if x == 'R' || y == 'R' {
            let old_path = entries
                .get(index + 1)
                .map(|old| String::from_utf8_lossy(old).to_string())
                .filter(|old| !old.is_empty());
            let rename = old_path
                .map(|old| format!("{old} -> {path}"))
                .unwrap_or(path.clone());
            push_unique(&mut manifest.renamed_files, rename);
            if entries.get(index + 1).is_some() {
                index += 1;
            }
        }
        index += 1;
    }
    manifest
}

fn parse_nul_paths(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .map(|entry| String::from_utf8_lossy(entry).to_string())
        .collect()
}

fn is_conflicted_status(x: char, y: char) -> bool {
    matches!(
        (x, y),
        ('D', 'D') | ('A', 'U') | ('U', 'D') | ('U', 'A') | ('D', 'U') | ('A', 'A') | ('U', 'U')
    )
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn preserve_agent_manifest(target_path: &Path) -> Result<Option<String>> {
    if !target_path.exists() {
        return Ok(None);
    }
    let preserved_path = target_path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join("changed_files_manifest.agent.json");
    std::fs::copy(target_path, &preserved_path).with_context(|| {
        format!(
            "preserving existing agent-authored changed_files_manifest at {}",
            preserved_path.display()
        )
    })?;
    Ok(Some(preserved_path.to_string_lossy().into_owned()))
}

fn write_manifest(target_path: &Path, manifest: ChangedFilesManifest) -> Result<()> {
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "creating changed-files manifest parent {}",
                parent.display()
            )
        })?;
    }
    let bytes = serde_json::to_vec_pretty(&manifest)?;
    std::fs::write(target_path, bytes)
        .with_context(|| format!("writing changed-files manifest {}", target_path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn proposal_053_git_manifest_runner_reports_worktree_changes() {
        let repo = tempfile::tempdir().unwrap();
        run_git_for_test(repo.path(), &["init"]).await;
        std::fs::write(repo.path().join("tracked.txt"), "before\n").unwrap();
        run_git_for_test(repo.path(), &["add", "tracked.txt"]).await;
        std::fs::write(repo.path().join("tracked.txt"), "after\n").unwrap();
        std::fs::write(repo.path().join("untracked.txt"), "new\n").unwrap();

        let manifest = GitManifestRunner::new(repo.path().to_path_buf())
            .run()
            .await
            .unwrap();

        assert_eq!(manifest.status, GitManifestStatus::Available);
        assert!(manifest.files.contains(&"tracked.txt".to_string()));
        assert!(manifest.files.contains(&"untracked.txt".to_string()));
        assert!(manifest.staged_changes.contains(&"tracked.txt".to_string()));
        assert!(manifest
            .unstaged_changes
            .contains(&"tracked.txt".to_string()));
        assert!(manifest
            .untracked_files
            .contains(&"untracked.txt".to_string()));
        assert!(manifest.diff_stat_text.contains("tracked.txt"));
    }

    #[tokio::test]
    async fn proposal_053_declared_manifest_preserves_agent_authored_file() {
        let repo = tempfile::tempdir().unwrap();
        run_git_for_test(repo.path(), &["init"]).await;
        std::fs::write(repo.path().join("source.txt"), "source\n").unwrap();
        let target_path = repo.path().join("out/changed-files.json");
        std::fs::create_dir_all(target_path.parent().unwrap()).unwrap();
        std::fs::write(&target_path, br#"{"files":["agent"]}"#).unwrap();
        let declared = DeclaredOutput {
            output_name: CHANGED_FILES_MANIFEST_OUTPUT.to_string(),
            target_path: target_path.to_string_lossy().into_owned(),
            schema: None,
            reuse_policy: None,
            companion_output_name: None,
            companion_path: None,
        };

        let status = generate_changed_files_manifest_if_declared(
            &[declared],
            Some(repo.path().to_string_lossy().as_ref()),
            true,
        )
        .await
        .unwrap();

        assert_eq!(status, Some(GitManifestStatus::Available));
        let preserved = target_path
            .parent()
            .unwrap()
            .join("changed_files_manifest.agent.json");
        assert_eq!(
            std::fs::read_to_string(preserved).unwrap(),
            r#"{"files":["agent"]}"#
        );
        let manifest: ChangedFilesManifest =
            serde_json::from_slice(&std::fs::read(&target_path).unwrap()).unwrap();
        assert_eq!(manifest.status, GitManifestStatus::Available);
        assert!(manifest.untracked_files.contains(&"source.txt".to_string()));
        assert!(manifest.preserved_agent_manifest_path.is_some());
    }

    #[tokio::test]
    async fn proposal_053_git_manifest_runner_reports_not_git_repository() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = GitManifestRunner::new(dir.path().to_path_buf())
            .run()
            .await
            .unwrap();

        assert_eq!(manifest.status, GitManifestStatus::NotGitRepository);
        assert!(!manifest.diagnostics.is_empty());
    }

    async fn run_git_for_test(dir: &Path, args: &[&str]) {
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
