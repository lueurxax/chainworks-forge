//! Worktree provisioner — creates and manages dedicated git worktrees for
//! implementation agents (Proposal 007, ARCH-067).
//!
//! Matches Swift `WorktreeProvisioner`. One run = one dedicated worktree on
//! an isolated branch so implementation agents don't pollute the user's
//! working copy.

use anyhow::{Context, Result};
use domain::ids::RunId;
use std::path::Path;
use tracing::{info, warn};

/// Result of a successful worktree provisioning.
pub struct WorktreeProvisionResult {
    pub worktree_root: String,
    pub base_branch: String,
    pub base_revision: String,
    pub target_branch: String,
}

/// Creates and manages dedicated writable implementation worktrees.
pub struct WorktreeProvisioner;

impl WorktreeProvisioner {
    /// Provision a dedicated worktree for a repo-backed run.
    ///
    /// Steps (matching Swift WorktreeProvisioner §7.4):
    /// 1. Validate workspace is a git repo
    /// 2. Resolve base branch and record base revision
    /// 3. Create worktree with `git worktree add -b {branch} {path} {base}`
    /// 4. Return frozen worktree metadata
    ///
    /// Idempotent: if the worktree path already exists, returns existing data.
    pub async fn provision(
        workspace_root: &str,
        run_id: RunId,
        idea_title: &str,
        base_branch_override: Option<&str>,
    ) -> Result<WorktreeProvisionResult> {
        let ws = Path::new(workspace_root);

        // Step 1: Validate git repo
        let git_dir_output = run_git(&["rev-parse", "--git-dir"], ws).await?;
        if git_dir_output.trim().is_empty() {
            anyhow::bail!("workspace_root is not a git repository: {workspace_root}");
        }

        // Step 2: Resolve base branch
        let base_branch = base_branch_override.unwrap_or("main").to_string();
        let base_revision = run_git(&["rev-parse", &base_branch], ws)
            .await
            .with_context(|| format!("Base branch '{}' not found in {}", base_branch, workspace_root))?
            .trim()
            .to_string();

        if base_revision.is_empty() {
            anyhow::bail!("Could not resolve base branch '{}' to a commit", base_branch);
        }

        // Step 3: Compute naming
        let slug = sanitize_slug(idea_title);
        let short_id = &run_id.to_string()[..8];
        let worktree_dir_name = format!("cw-{slug}-{short_id}");
        let worktree_root = format!(
            "{workspace_root}/.chainworks/worktrees/{worktree_dir_name}"
        );
        let target_branch = format!("cw/{slug}/{short_id}");

        // Idempotent: if path exists, verify and return existing data
        if Path::new(&worktree_root).exists() {
            info!(
                worktree_root = %worktree_root,
                "Worktree already exists — returning existing data (idempotent)"
            );
            // Read the current branch name from the worktree
            let existing_branch = run_git(
                &["rev-parse", "--abbrev-ref", "HEAD"],
                Path::new(&worktree_root),
            )
            .await
            .unwrap_or_else(|_| target_branch.clone());

            return Ok(WorktreeProvisionResult {
                worktree_root,
                base_branch,
                base_revision,
                target_branch: existing_branch.trim().to_string(),
            });
        }

        // Step 4: Create parent directory
        let worktree_base = format!("{workspace_root}/.chainworks/worktrees");
        std::fs::create_dir_all(&worktree_base)
            .with_context(|| format!("creating worktree base dir: {worktree_base}"))?;

        // Step 5: Create worktree
        info!(
            workspace_root = %workspace_root,
            worktree_root = %worktree_root,
            target_branch = %target_branch,
            base_branch = %base_branch,
            "Provisioning worktree"
        );
        run_git(
            &["worktree", "add", "-b", &target_branch, &worktree_root, &base_branch],
            ws,
        )
        .await
        .with_context(|| {
            format!(
                "git worktree add failed: branch={target_branch} path={worktree_root} base={base_branch}"
            )
        })?;

        info!(
            worktree_root = %worktree_root,
            target_branch = %target_branch,
            base_revision = %base_revision,
            "Worktree provisioned successfully"
        );

        Ok(WorktreeProvisionResult {
            worktree_root,
            base_branch,
            base_revision,
            target_branch,
        })
    }

    /// Remove a worktree (cleanup after run completes or is cancelled).
    /// Best-effort: warns on failure instead of propagating errors.
    pub async fn cleanup(worktree_root: &str, workspace_root: &str) -> Result<()> {
        let ws = Path::new(workspace_root);
        if !Path::new(worktree_root).exists() {
            info!(worktree_root = %worktree_root, "Worktree already removed — skipping cleanup");
            return Ok(());
        }
        info!(worktree_root = %worktree_root, "Cleaning up worktree");
        match run_git(&["worktree", "remove", "--force", worktree_root], ws).await {
            Ok(_) => {
                info!(worktree_root = %worktree_root, "Worktree removed successfully");
                Ok(())
            }
            Err(e) => {
                warn!(
                    worktree_root = %worktree_root,
                    error = %e,
                    "Worktree cleanup failed — manual removal may be needed"
                );
                Err(e)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// SourceContextBuilder — gathers changed files manifest from worktree.
// Matches Swift `SourceContextBuilder`.
// ---------------------------------------------------------------------------

/// Source context gathered from the worktree for implementation agents.
pub struct SourceContext {
    pub changed_files: Vec<String>,
    pub diff_summary: String,
}

/// Build source context from the current worktree state.
/// Returns changed files and diff summary relative to the base branch.
pub async fn build_source_context(
    worktree_root: &str,
    base_branch: &str,
) -> Result<SourceContext> {
    let wt = Path::new(worktree_root);

    let changed_files_output = run_git(&["diff", "--name-only", base_branch], wt)
        .await
        .unwrap_or_default();
    let changed_files: Vec<String> = changed_files_output
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect();

    let diff_summary = run_git(&["diff", "--stat", base_branch], wt)
        .await
        .unwrap_or_default()
        .trim()
        .to_string();

    Ok(SourceContext {
        changed_files,
        diff_summary,
    })
}

// ---------------------------------------------------------------------------
// RepoSafetyGuard — validates worktree readiness and path boundaries.
// Matches Swift `RepoSafetyGuard` (Proposal 007 §7.7).
// ---------------------------------------------------------------------------

/// Validates safety invariants before write-enabled agent execution.
pub struct RepoSafetyGuard;

impl RepoSafetyGuard {
    /// Validate that a provisioned worktree exists and is ready for use.
    /// Called before launching a write-enabled agent session.
    pub fn validate_worktree_ready(worktree_root: Option<&str>) -> Result<()> {
        let wt = worktree_root
            .ok_or_else(|| anyhow::anyhow!("Write-enabled agent requires a provisioned worktree but none is set"))?;
        if wt.is_empty() {
            anyhow::bail!("Write-enabled agent has empty worktree_root");
        }
        if !Path::new(wt).exists() {
            anyhow::bail!(
                "Worktree root does not exist on disk: {wt}. \
                 It may have been cleaned up prematurely."
            );
        }
        if !Path::new(wt).is_dir() {
            anyhow::bail!("Worktree root is not a directory: {wt}");
        }
        Ok(())
    }

    /// Validate that a target path is within the allowed workspace or worktree boundary.
    /// Prevents write-enabled agents from accessing files outside their scope.
    pub fn validate_path_boundary(
        target_path: &str,
        workspace_root: &str,
        worktree_root: Option<&str>,
    ) -> Result<()> {
        let resolved = std::fs::canonicalize(target_path)
            .unwrap_or_else(|_| std::path::PathBuf::from(target_path));
        let resolved_str = resolved.to_string_lossy();

        // Check workspace boundary
        let ws_canon = std::fs::canonicalize(workspace_root)
            .unwrap_or_else(|_| std::path::PathBuf::from(workspace_root));
        let ws_str = ws_canon.to_string_lossy();
        if resolved_str.starts_with(ws_str.as_ref()) {
            return Ok(());
        }

        // Check worktree boundary
        if let Some(wt) = worktree_root {
            let wt_canon = std::fs::canonicalize(wt)
                .unwrap_or_else(|_| std::path::PathBuf::from(wt));
            let wt_str = wt_canon.to_string_lossy();
            if resolved_str.starts_with(wt_str.as_ref()) {
                return Ok(());
            }
        }

        anyhow::bail!(
            "Path '{}' is outside allowed boundaries (workspace: {}, worktree: {:?})",
            target_path,
            workspace_root,
            worktree_root,
        )
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Sanitize an idea title into a slug suitable for directory/branch names.
/// Lowercase, replace non-alphanumeric with `-`, collapse runs, truncate to 30 chars.
fn sanitize_slug(title: &str) -> String {
    let slug: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    // Collapse consecutive dashes
    let mut result = String::with_capacity(slug.len());
    let mut prev_dash = false;
    for c in slug.chars() {
        if c == '-' {
            if !prev_dash && !result.is_empty() {
                result.push(c);
            }
            prev_dash = true;
        } else {
            result.push(c);
            prev_dash = false;
        }
    }
    // Trim trailing dash and truncate
    let trimmed = result.trim_end_matches('-');
    if trimmed.len() > 30 {
        trimmed[..30].trim_end_matches('-').to_string()
    } else {
        trimmed.to_string()
    }
}

/// Run a git command in the given directory and return stdout.
async fn run_git(args: &[&str], dir: &Path) -> Result<String> {
    let output = tokio::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .await
        .with_context(|| format!("spawning git {}", args.join(" ")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::bail!(
            "git {} failed (exit {}): {}",
            args.join(" "),
            output.status,
            if stderr.trim().is_empty() { stdout.to_string() } else { stderr.to_string() }
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
