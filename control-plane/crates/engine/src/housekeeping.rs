//! Conservative generated-state cleanup for local daemon dogfood.
//!
//! This intentionally removes only rebuildable generated state. It never
//! removes worktrees, source files, run artifacts, SQLite databases, or active
//! run outputs.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime},
};

use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::{Row, SqlitePool};
use tracing::{debug, info, warn};

#[derive(Clone, Debug)]
pub struct GeneratedStateHousekeepingConfig {
    pub enabled: bool,
    pub interval: Duration,
    pub min_age: Duration,
}

impl GeneratedStateHousekeepingConfig {
    pub fn from_env() -> Self {
        Self {
            enabled: env_bool("CHAINWORKS_GENERATED_STATE_HOUSEKEEPING", true),
            interval: Duration::from_secs(env_u64(
                "CHAINWORKS_GENERATED_STATE_HOUSEKEEPING_INTERVAL_SECS",
                15 * 60,
            )),
            min_age: Duration::from_secs(env_u64(
                "CHAINWORKS_GENERATED_STATE_HOUSEKEEPING_MIN_AGE_SECS",
                6 * 60 * 60,
            )),
        }
    }
}

#[derive(Default, Debug)]
pub struct GeneratedStateHousekeepingReport {
    pub worktree_target_dirs_removed: usize,
    pub acp_runtime_dirs_removed: usize,
    pub git_temp_objects_removed: usize,
    pub bytes_reclaimed: u64,
}

#[derive(Clone, Debug)]
struct RunCleanupCandidate {
    run_id: String,
    status: String,
    workspace_root: PathBuf,
    worktree_root: Option<PathBuf>,
}

pub struct GeneratedStateHousekeeper;

impl GeneratedStateHousekeeper {
    pub async fn run_once(
        pool: &SqlitePool,
        config: &GeneratedStateHousekeepingConfig,
    ) -> Result<GeneratedStateHousekeepingReport> {
        if !config.enabled {
            return Ok(GeneratedStateHousekeepingReport::default());
        }

        let candidates = load_run_cleanup_candidates(pool).await?;
        let config = config.clone();
        let report = tokio::task::spawn_blocking(move || prune_generated_state(candidates, config))
            .await
            .context("join generated-state housekeeping task")??;

        // T19: prune terminal run-scoped Xcode roots under CHAINWORKS_TOOLCHAIN_HOME.
        if let Ok(toolchain_home) = std::env::var("CHAINWORKS_TOOLCHAIN_HOME") {
            let readback = sweep_xcode_toolchain_roots(pool, Path::new(&toolchain_home)).await;
            if let Ok(readback) = readback {
                let _ = db::repos::toolchain_cache_housekeeping::insert(pool, &readback).await;
            }
        }

        Ok(report)
    }
}

/// T19: prune terminal run-scoped Xcode roots from `CHAINWORKS_TOOLCHAIN_HOME/providers/xcode/`.
///
/// Terminal run dirs (completed/failed/cancelled) are removed in full, including any quarantine
/// subdirs left by the startup recovery sweep (T14). Active and unknown run dirs are not touched.
pub async fn sweep_xcode_toolchain_roots(
    pool: &SqlitePool,
    toolchain_home: &Path,
) -> Result<db::repos::toolchain_cache_housekeeping::ToolchainCacheHousekeepingReadback> {
    let xcode_dir = toolchain_home.join("providers").join("xcode");
    let now = Utc::now();
    let mut roots_pruned: i64 = 0;
    let mut prune_failures: i64 = 0;
    let mut oldest_eligible_root_age_days: Option<f64> = None;

    if let Ok(entries) = fs::read_dir(&xcode_dir) {
        for entry in entries.flatten() {
            let run_dir = entry.path();
            if !run_dir.is_dir() {
                continue;
            }
            let Some(run_id_str) = run_dir
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
            else {
                continue;
            };

            let status: Option<String> =
                sqlx::query_scalar("SELECT status FROM runs WHERE id = ?1")
                    .bind(&run_id_str)
                    .fetch_optional(pool)
                    .await
                    .unwrap_or(None);

            let is_terminal = status
                .as_deref()
                .map(|s| matches!(s, "completed" | "failed" | "cancelled"))
                .unwrap_or(false);

            if !is_terminal {
                continue;
            }

            // Track oldest eligible root age for the readback.
            if let Ok(meta) = fs::metadata(&run_dir) {
                if let Ok(modified) = meta.modified() {
                    if let Ok(age) = SystemTime::now().duration_since(modified) {
                        let age_days = age.as_secs_f64() / 86400.0;
                        oldest_eligible_root_age_days = Some(
                            oldest_eligible_root_age_days
                                .map_or(age_days, |prev: f64| prev.max(age_days)),
                        );
                    }
                }
            }

            match fs::remove_dir_all(&run_dir) {
                Ok(()) => {
                    roots_pruned += 1;
                    info!(
                        run_id = %run_id_str,
                        "Housekeeping pruned terminal run-scoped Xcode toolchain root"
                    );
                }
                Err(e) => {
                    prune_failures += 1;
                    warn!(
                        run_id = %run_id_str,
                        error = %e,
                        "Failed to prune terminal run-scoped Xcode toolchain root"
                    );
                }
            }
        }
    }

    Ok(
        db::repos::toolchain_cache_housekeeping::ToolchainCacheHousekeepingReadback {
            id: uuid::Uuid::new_v4().to_string(),
            last_sweep_started_at: now,
            run_scoped_roots_pruned: roots_pruned,
            run_scoped_prune_failures: prune_failures,
            oldest_eligible_root_age_days,
            disk_pressure_blocks: 0,
            quarantined_roots_created: 0,
            created_at: Utc::now(),
        },
    )
}

async fn load_run_cleanup_candidates(pool: &SqlitePool) -> Result<Vec<RunCleanupCandidate>> {
    let rows = sqlx::query(
        r#"SELECT id, status, workspace_root, worktree_root
           FROM runs
           WHERE workspace_root IS NOT NULL"#,
    )
    .fetch_all(pool)
    .await
    .context("load generated-state housekeeping candidates")?;

    Ok(rows
        .into_iter()
        .map(|row| RunCleanupCandidate {
            run_id: row.get("id"),
            status: row.get("status"),
            workspace_root: PathBuf::from(row.get::<String, _>("workspace_root")),
            worktree_root: row
                .get::<Option<String>, _>("worktree_root")
                .map(PathBuf::from),
        })
        .collect())
}

fn prune_generated_state(
    candidates: Vec<RunCleanupCandidate>,
    config: GeneratedStateHousekeepingConfig,
) -> Result<GeneratedStateHousekeepingReport> {
    let now = SystemTime::now();
    let live_command_lines = live_process_command_lines();
    let mut report = GeneratedStateHousekeepingReport::default();
    let mut workspace_roots = BTreeSet::new();

    for candidate in &candidates {
        workspace_roots.insert(candidate.workspace_root.clone());

        if !is_terminal_run_status(&candidate.status) {
            continue;
        }

        let Some(worktree_root) = candidate.worktree_root.as_deref() else {
            continue;
        };
        if !is_managed_worktree(&candidate.workspace_root, worktree_root) {
            warn!(
                run_id = %candidate.run_id,
                worktree_root = %worktree_root.display(),
                "Skipping generated-state cleanup outside managed worktree root"
            );
            continue;
        }

        for target_dir in generated_target_dirs(worktree_root) {
            if let Some(bytes) = remove_dir_if_old(&target_dir, now, config.min_age)? {
                report.bytes_reclaimed += bytes;
                report.worktree_target_dirs_removed += 1;
                info!(
                    run_id = %candidate.run_id,
                    target_dir = %target_dir.display(),
                    "Removed inactive run generated target directory"
                );
            }
        }
    }

    for workspace_root in workspace_roots {
        prune_stale_acp_runtime_homes(
            &workspace_root,
            now,
            config.min_age,
            &live_command_lines,
            &mut report,
        )?;
        prune_git_temp_objects(&workspace_root, now, config.min_age, &mut report)?;
    }

    if report.worktree_target_dirs_removed > 0
        || report.acp_runtime_dirs_removed > 0
        || report.git_temp_objects_removed > 0
    {
        info!(
            worktree_target_dirs_removed = report.worktree_target_dirs_removed,
            acp_runtime_dirs_removed = report.acp_runtime_dirs_removed,
            git_temp_objects_removed = report.git_temp_objects_removed,
            bytes_reclaimed = report.bytes_reclaimed,
            "Generated-state housekeeping complete"
        );
    } else {
        debug!("Generated-state housekeeping complete with nothing to prune");
    }

    Ok(report)
}

fn generated_target_dirs(worktree_root: &Path) -> Vec<PathBuf> {
    vec![
        worktree_root.join("control-plane").join("target"),
        worktree_root.join("target"),
    ]
}

fn prune_stale_acp_runtime_homes(
    workspace_root: &Path,
    now: SystemTime,
    min_age: Duration,
    live_command_lines: &[String],
    report: &mut GeneratedStateHousekeepingReport,
) -> Result<()> {
    let runtime_root = workspace_root.join(".forge-codex-acp");
    let Ok(entries) = fs::read_dir(&runtime_root) else {
        return Ok(());
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if is_path_referenced_by_live_process(&path, live_command_lines) {
            continue;
        }
        if let Some(bytes) = remove_dir_if_old(&path, now, min_age)? {
            report.bytes_reclaimed += bytes;
            report.acp_runtime_dirs_removed += 1;
            info!(
                runtime_home = %path.display(),
                "Removed stale Codex ACP runtime home"
            );
        }
    }

    Ok(())
}

fn prune_git_temp_objects(
    workspace_root: &Path,
    now: SystemTime,
    min_age: Duration,
    report: &mut GeneratedStateHousekeepingReport,
) -> Result<()> {
    let objects_dir = workspace_root.join(".git").join("objects");
    let Ok(entries) = fs::read_dir(&objects_dir) else {
        return Ok(());
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if !name.starts_with("tmp_obj_") || !is_old_enough(&path, now, min_age) {
            continue;
        }

        let bytes = fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
        match fs::remove_file(&path) {
            Ok(()) => {
                report.git_temp_objects_removed += 1;
                report.bytes_reclaimed += bytes;
                info!(path = %path.display(), "Removed stale git temporary object");
            }
            Err(err) => warn!(
                path = %path.display(),
                error = %err,
                "Failed to remove stale git temporary object"
            ),
        }
    }

    Ok(())
}

fn remove_dir_if_old(path: &Path, now: SystemTime, min_age: Duration) -> Result<Option<u64>> {
    if !path.is_dir() || !is_old_enough(path, now, min_age) {
        return Ok(None);
    }

    let bytes = directory_size(path);
    fs::remove_dir_all(path)
        .with_context(|| format!("remove generated directory {}", path.display()))?;
    Ok(Some(bytes))
}

fn is_old_enough(path: &Path, now: SystemTime, min_age: Duration) -> bool {
    let modified = fs::metadata(path)
        .and_then(|meta| meta.modified())
        .unwrap_or(now);
    now.duration_since(modified)
        .map(|age| age >= min_age)
        .unwrap_or(false)
}

fn is_terminal_run_status(status: &str) -> bool {
    matches!(status, "completed" | "failed" | "cancelled")
}

fn is_managed_worktree(workspace_root: &Path, worktree_root: &Path) -> bool {
    let managed_root = workspace_root.join(".chainworks").join("worktrees");
    worktree_root.starts_with(managed_root)
}

fn live_process_command_lines() -> Vec<String> {
    #[cfg(unix)]
    {
        Command::new("ps")
            .args(["-axo", "command"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| {
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .map(ToOwned::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }
    #[cfg(not(unix))]
    {
        Vec::new()
    }
}

fn is_path_referenced_by_live_process(path: &Path, live_command_lines: &[String]) -> bool {
    let path = path.to_string_lossy();
    live_command_lines
        .iter()
        .any(|command_line| command_line.contains(path.as_ref()))
}

fn directory_size(path: &Path) -> u64 {
    let mut total = 0;
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    for entry in entries.flatten() {
        let entry_path = entry.path();
        if let Ok(metadata) = entry.metadata() {
            if metadata.is_dir() {
                total += directory_size(&entry_path);
            } else {
                total += metadata.len();
            }
        }
    }
    total
}

fn env_bool(name: &str, default: bool) -> bool {
    match std::env::var(name) {
        Ok(value) => !matches!(
            value.to_ascii_lowercase().as_str(),
            "0" | "false" | "off" | "no"
        ),
        Err(_) => default,
    }
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|raw| raw.parse().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn terminal_statuses_are_the_only_cleanup_eligible_statuses() {
        assert!(is_terminal_run_status("completed"));
        assert!(is_terminal_run_status("failed"));
        assert!(is_terminal_run_status("cancelled"));
        assert!(!is_terminal_run_status("running"));
        assert!(!is_terminal_run_status("blocked"));
        assert!(!is_terminal_run_status("cancelling"));
    }

    #[test]
    fn managed_worktree_must_live_under_chainworks_worktrees() {
        let root = PathBuf::from("/repo");
        assert!(is_managed_worktree(
            &root,
            Path::new("/repo/.chainworks/worktrees/cw-test")
        ));
        assert!(!is_managed_worktree(&root, Path::new("/repo/other")));
        assert!(!is_managed_worktree(
            &root,
            Path::new("/repo/.chainworks/runs/cw-test")
        ));
    }

    #[test]
    fn stale_acp_home_is_not_removed_when_referenced_by_live_process() {
        let tmp = tempdir().unwrap();
        let home = tmp.path().join(".forge-codex-acp").join("session-1");
        fs::create_dir_all(&home).unwrap();

        let mut report = GeneratedStateHousekeepingReport::default();
        prune_stale_acp_runtime_homes(
            tmp.path(),
            SystemTime::now() + Duration::from_secs(60 * 60),
            Duration::from_secs(1),
            &[format!("codex-acp --home {}", home.display())],
            &mut report,
        )
        .unwrap();

        assert!(home.exists());
        assert_eq!(report.acp_runtime_dirs_removed, 0);
    }

    #[test]
    fn active_and_blocked_run_targets_are_preserved() {
        let tmp = tempdir().unwrap();
        let workspace = tmp.path();
        let active_target = workspace
            .join(".chainworks")
            .join("worktrees")
            .join("active-run")
            .join("control-plane")
            .join("target");
        let blocked_target = workspace
            .join(".chainworks")
            .join("worktrees")
            .join("blocked-run")
            .join("target");
        fs::create_dir_all(&active_target).unwrap();
        fs::create_dir_all(&blocked_target).unwrap();
        fs::write(active_target.join("build-output"), b"active").unwrap();
        fs::write(blocked_target.join("build-output"), b"blocked").unwrap();

        let report = prune_generated_state(
            vec![
                RunCleanupCandidate {
                    run_id: "active-run".into(),
                    status: "running".into(),
                    workspace_root: workspace.to_path_buf(),
                    worktree_root: Some(
                        workspace
                            .join(".chainworks")
                            .join("worktrees")
                            .join("active-run"),
                    ),
                },
                RunCleanupCandidate {
                    run_id: "blocked-run".into(),
                    status: "blocked".into(),
                    workspace_root: workspace.to_path_buf(),
                    worktree_root: Some(
                        workspace
                            .join(".chainworks")
                            .join("worktrees")
                            .join("blocked-run"),
                    ),
                },
            ],
            GeneratedStateHousekeepingConfig {
                enabled: true,
                interval: Duration::from_secs(1),
                min_age: Duration::from_secs(0),
            },
        )
        .unwrap();

        assert_eq!(report.worktree_target_dirs_removed, 0);
        assert!(active_target.exists());
        assert!(blocked_target.exists());
    }

    #[test]
    fn terminal_run_cleanup_preserves_worktree_sources_artifacts_and_databases() {
        let tmp = tempdir().unwrap();
        let workspace = tmp.path();
        let worktree = workspace
            .join(".chainworks")
            .join("worktrees")
            .join("done-run");
        let target = worktree.join("control-plane").join("target");
        let source_file = worktree.join("control-plane").join("src").join("main.rs");
        let artifact = workspace
            .join(".chainworks")
            .join("runs")
            .join("done-run")
            .join("artifact.json");
        let db_file = workspace.join(".chainworks").join("control-plane.db");

        fs::create_dir_all(&target).unwrap();
        fs::create_dir_all(source_file.parent().unwrap()).unwrap();
        fs::create_dir_all(artifact.parent().unwrap()).unwrap();
        fs::write(target.join("object.o"), b"generated").unwrap();
        fs::write(&source_file, b"fn main() {}").unwrap();
        fs::write(&artifact, b"{}").unwrap();
        fs::write(&db_file, b"sqlite").unwrap();

        let report = prune_generated_state(
            vec![RunCleanupCandidate {
                run_id: "done-run".into(),
                status: "completed".into(),
                workspace_root: workspace.to_path_buf(),
                worktree_root: Some(worktree.clone()),
            }],
            GeneratedStateHousekeepingConfig {
                enabled: true,
                interval: Duration::from_secs(1),
                min_age: Duration::from_secs(0),
            },
        )
        .unwrap();

        assert_eq!(report.worktree_target_dirs_removed, 1);
        assert!(!target.exists());
        assert!(worktree.exists(), "housekeeping must not delete worktrees");
        assert!(
            source_file.exists(),
            "housekeeping must not delete source files"
        );
        assert!(
            artifact.exists(),
            "housekeeping must not delete run artifacts"
        );
        assert!(
            db_file.exists(),
            "housekeeping must not delete SQLite database files"
        );
    }

    #[test]
    fn terminal_run_cleanup_skips_unmanaged_worktree_targets() {
        let tmp = tempdir().unwrap();
        let workspace = tmp.path();
        let unmanaged = workspace.join("manual-worktree");
        let target = unmanaged.join("control-plane").join("target");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("object.o"), b"generated").unwrap();

        let report = prune_generated_state(
            vec![RunCleanupCandidate {
                run_id: "manual-run".into(),
                status: "completed".into(),
                workspace_root: workspace.to_path_buf(),
                worktree_root: Some(unmanaged),
            }],
            GeneratedStateHousekeepingConfig {
                enabled: true,
                interval: Duration::from_secs(1),
                min_age: Duration::from_secs(0),
            },
        )
        .unwrap();

        assert_eq!(report.worktree_target_dirs_removed, 0);
        assert!(target.exists());
    }

    #[test]
    fn stale_unreferenced_acp_home_is_removed() {
        let tmp = tempdir().unwrap();
        let home = tmp.path().join(".forge-codex-acp").join("session-2");
        fs::create_dir_all(&home).unwrap();
        fs::write(home.join("state.json"), b"{}").unwrap();

        let mut report = GeneratedStateHousekeepingReport::default();
        prune_stale_acp_runtime_homes(
            tmp.path(),
            SystemTime::now(),
            Duration::from_secs(0),
            &[],
            &mut report,
        )
        .unwrap();

        assert!(!home.exists());
        assert_eq!(report.acp_runtime_dirs_removed, 1);
    }
}
