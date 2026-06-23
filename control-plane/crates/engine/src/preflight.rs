use std::fs;
use std::path::Path;
use std::process::Command;
use std::process::Stdio;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use domain::run::DeliveryConfiguration;

const DELIVERY_PREFLIGHT_GIT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeliveryPreflightResult {
    pub checks: Vec<PreflightCheck>,
    pub passed: bool,
    pub timestamp: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreflightCheck {
    pub id: String,
    pub label: String,
    pub passed: bool,
    pub detail: Option<String>,
}

pub fn missing_delivery_configuration_preflight() -> DeliveryPreflightResult {
    DeliveryPreflightResult {
        checks: vec![PreflightCheck {
            id: "delivery_configuration_present".into(),
            label: "Delivery configuration is present".into(),
            passed: false,
            detail: Some("release workflows require frozen delivery_configuration_json".into()),
        }],
        passed: false,
        timestamp: Utc::now(),
    }
}

pub fn run_delivery_preflight(config: &DeliveryConfiguration) -> DeliveryPreflightResult {
    let mut checks = Vec::new();
    checks.push(check_repo_root_exists(&config.repo_root));
    checks.push(check_git_repository(&config.repo_root));
    checks.push(check_base_branch_exists(
        &config.repo_root,
        &config.base_branch,
    ));
    checks.push(check_worktree_base_writable(&config.worktree_base_path));
    checks.push(non_empty_check(
        "release_target_id",
        "Release target identifier is non-empty",
        config.release_target_id.as_deref().unwrap_or_default(),
    ));
    checks.push(non_empty_check(
        "repo_identifier",
        "Repository identifier is non-empty",
        &config.repo_identifier,
    ));
    let passed = checks.iter().all(|check| check.passed);
    DeliveryPreflightResult {
        checks,
        passed,
        timestamp: Utc::now(),
    }
}

fn check_repo_root_exists(repo_root: &str) -> PreflightCheck {
    let path = Path::new(repo_root);
    PreflightCheck {
        id: "repo_root_exists".into(),
        label: "Repository root exists".into(),
        passed: path.is_dir(),
        detail: (!path.is_dir()).then(|| format!("{repo_root} is not a directory")),
    }
}

fn check_git_repository(repo_root: &str) -> PreflightCheck {
    let status = run_git_with_timeout(
        &["-C", repo_root, "rev-parse", "--is-inside-work-tree"],
        DELIVERY_PREFLIGHT_GIT_TIMEOUT,
    );
    let passed = status == PreflightCommandStatus::Success;
    PreflightCheck {
        id: "git_repository_valid".into(),
        label: "Repository root is a git repository".into(),
        passed,
        detail: (!passed).then(|| status.detail("git rev-parse failed")),
    }
}

fn check_base_branch_exists(repo_root: &str, base_branch: &str) -> PreflightCheck {
    let branch_ref_status = run_git_with_timeout(
        &[
            "-C",
            repo_root,
            "rev-parse",
            "--verify",
            "--quiet",
            base_branch,
        ],
        DELIVERY_PREFLIGHT_GIT_TIMEOUT,
    );
    let branch_ref_exists = branch_ref_status == PreflightCommandStatus::Success;
    let passed = !base_branch.trim().is_empty() && branch_ref_exists;
    PreflightCheck {
        id: "base_branch_exists".into(),
        label: "Base branch exists".into(),
        passed,
        detail: (!passed).then(|| format!("base branch {base_branch:?} was not found")),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PreflightCommandStatus {
    Success,
    Failed,
    TimedOut,
    SpawnFailed,
}

impl PreflightCommandStatus {
    fn detail(self, fallback: &str) -> String {
        match self {
            Self::Success => fallback.to_string(),
            Self::Failed => fallback.to_string(),
            Self::TimedOut => format!("{fallback}: command timed out"),
            Self::SpawnFailed => format!("{fallback}: command could not be started"),
        }
    }
}

fn run_git_with_timeout(args: &[&str], timeout: Duration) -> PreflightCommandStatus {
    run_command_with_timeout("git", args, timeout)
}

fn run_command_with_timeout(
    program: &str,
    args: &[&str],
    timeout: Duration,
) -> PreflightCommandStatus {
    let mut child = match Command::new(program)
        .current_dir("/")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return PreflightCommandStatus::SpawnFailed,
    };
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    PreflightCommandStatus::Success
                } else {
                    PreflightCommandStatus::Failed
                };
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return PreflightCommandStatus::TimedOut;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return PreflightCommandStatus::Failed;
            }
        }
    }
}

fn check_worktree_base_writable(worktree_base_path: &str) -> PreflightCheck {
    let path = Path::new(worktree_base_path);
    let probe = path.join(format!(".chainworks-write-probe-{}", uuid::Uuid::new_v4()));
    let passed = path.is_dir()
        && fs::write(&probe, b"probe")
            .and_then(|_| fs::remove_file(&probe))
            .is_ok();
    PreflightCheck {
        id: "worktree_base_writable".into(),
        label: "Worktree base path is writable".into(),
        passed,
        detail: (!passed).then(|| format!("{worktree_base_path} is not writable")),
    }
}

fn non_empty_check(id: &str, label: &str, value: &str) -> PreflightCheck {
    let passed = !value.trim().is_empty();
    PreflightCheck {
        id: id.into(),
        label: label.into(),
        passed,
        detail: (!passed).then(|| format!("{id} is empty")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preflight_command_timeout_kills_hung_child() {
        let status = run_command_with_timeout("sh", &["-c", "sleep 60"], Duration::from_millis(50));

        assert_eq!(status, PreflightCommandStatus::TimedOut);
    }
}
