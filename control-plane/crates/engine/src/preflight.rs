use std::fs;
use std::path::Path;
use std::process::Command;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use domain::run::DeliveryConfiguration;

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
    let output = Command::new("git")
        .args(["-C", repo_root, "rev-parse", "--is-inside-work-tree"])
        .output();
    let passed = output
        .as_ref()
        .map(|output| output.status.success())
        .unwrap_or(false);
    PreflightCheck {
        id: "git_repository_valid".into(),
        label: "Repository root is a git repository".into(),
        passed,
        detail: (!passed).then(|| "git rev-parse failed".to_string()),
    }
}

fn check_base_branch_exists(repo_root: &str, base_branch: &str) -> PreflightCheck {
    let current_branch_matches = Command::new("git")
        .args(["-C", repo_root, "symbolic-ref", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            output
                .status
                .success()
                .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
        })
        .map(|branch| branch == base_branch)
        .unwrap_or(false);
    let branch_ref_exists = Command::new("git")
        .args([
            "-C",
            repo_root,
            "rev-parse",
            "--verify",
            "--quiet",
            base_branch,
        ])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    let passed = !base_branch.trim().is_empty() && (current_branch_matches || branch_ref_exists);
    PreflightCheck {
        id: "base_branch_exists".into(),
        label: "Base branch exists".into(),
        passed,
        detail: (!passed).then(|| format!("base branch {base_branch:?} was not found")),
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
