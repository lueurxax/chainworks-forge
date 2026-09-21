use engine::run_carry_forward::{InventoryLimits, PreviewOptions, WorkspacePlanner};
use std::{
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
};

fn git(root: &Path, args: &[&str]) -> Vec<u8> {
    let output = Command::new("git")
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(root)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{:?}: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-b", "main"]);
    git(dir.path(), &["config", "user.name", "P039 Test"]);
    git(
        dir.path(),
        &["config", "user.email", "p039@example.invalid"],
    );
    fs::write(dir.path().join("code.txt"), b"committed\n").unwrap();
    fs::write(dir.path().join("deleted.txt"), b"old\n").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-m", "fixture"]);
    dir
}

#[tokio::test]
async fn preview_preserves_source_and_binds_staged_unstaged_untracked_modes_and_deletions() {
    let repo = fixture();
    fs::write(repo.path().join("code.txt"), b"staged\n").unwrap();
    git(repo.path(), &["add", "code.txt"]);
    fs::write(repo.path().join("code.txt"), b"final\0binary\n").unwrap();
    fs::set_permissions(
        repo.path().join("code.txt"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    fs::remove_file(repo.path().join("deleted.txt")).unwrap();
    fs::write(repo.path().join("untracked.txt"), b"untracked\n").unwrap();
    let index = fs::read(repo.path().join(".git/index")).unwrap();
    let status = git(repo.path(), &["status", "--porcelain=v1", "-z"]);
    let first = WorkspacePlanner::preview(repo.path(), PreviewOptions::default())
        .await
        .unwrap();
    let second = WorkspacePlanner::preview(repo.path(), PreviewOptions::default())
        .await
        .unwrap();
    assert_eq!(first.digest(), second.digest());
    assert_eq!(fs::read(repo.path().join(".git/index")).unwrap(), index);
    assert_eq!(
        git(repo.path(), &["status", "--porcelain=v1", "-z"]),
        status
    );
    assert_eq!(
        fs::read(repo.path().join("code.txt")).unwrap(),
        b"final\0binary\n"
    );
    assert_eq!(first.entry("code.txt").unwrap().mode, 0o755);
    assert!(first.entry("deleted.txt").unwrap().deleted);
    assert!(first.entry("untracked.txt").is_some());
    assert!(!repo.path().join(".chainworks").exists());
    assert!(!repo.path().join(".git/index.lock").exists());
    fs::write(repo.path().join("code.txt"), b"another edit\n").unwrap();
    let changed = WorkspacePlanner::preview(repo.path(), PreviewOptions::default())
        .await
        .unwrap();
    assert_ne!(first.digest(), changed.digest());
}

#[tokio::test]
async fn index_only_change_invalidates_witness_even_when_working_bytes_are_identical() {
    let repo = fixture();
    fs::write(repo.path().join("code.txt"), b"changed\n").unwrap();
    let before = WorkspacePlanner::preview(repo.path(), PreviewOptions::default())
        .await
        .unwrap();
    git(repo.path(), &["add", "code.txt"]);
    let after = WorkspacePlanner::preview(repo.path(), PreviewOptions::default())
        .await
        .unwrap();
    assert_ne!(before.digest(), after.digest());
    assert_eq!(
        before.entry("code.txt").unwrap().sha256,
        after.entry("code.txt").unwrap().sha256
    );
}

#[tokio::test]
async fn external_symlink_is_rejected_without_reading_target_and_explicit_machine_exclusion_is_recorded(
) {
    let repo = fixture();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret"), b"not copied").unwrap();
    fs::create_dir(repo.path().join(".antigravitycli")).unwrap();
    symlink(
        outside.path().join("secret"),
        repo.path().join(".antigravitycli/config"),
    )
    .unwrap();
    assert!(
        WorkspacePlanner::preview(repo.path(), PreviewOptions::default())
            .await
            .is_err()
    );
    let mut options = PreviewOptions::default();
    options.exclusions.insert(
        ".antigravitycli/config".into(),
        "machine configuration".into(),
    );
    let preview = WorkspacePlanner::preview(repo.path(), options)
        .await
        .unwrap();
    let excluded = preview.entry(".antigravitycli/config").unwrap();
    assert!(excluded.excluded);
    assert!(excluded.sha256.is_none());
    assert_eq!(
        excluded.link_target,
        Some(outside.path().join("secret").to_string_lossy().into())
    );
}

#[tokio::test]
async fn product_files_cannot_be_excluded_and_limits_do_not_truncate() {
    let repo = fixture();
    let mut options = PreviewOptions::default();
    options
        .exclusions
        .insert("code.txt".into(), "discard changes".into());
    assert!(WorkspacePlanner::preview(repo.path(), options)
        .await
        .is_err());
    let options = PreviewOptions {
        limits: InventoryLimits {
            max_file_bytes: 2,
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(WorkspacePlanner::preview(repo.path(), options)
        .await
        .unwrap_err()
        .to_string()
        .contains("continuation_budget_exceeded"));
    assert_eq!(
        fs::read(repo.path().join("code.txt")).unwrap(),
        b"committed\n"
    );
}

#[tokio::test]
async fn active_filter_is_rejected_before_any_clean_filter_execution() {
    let repo = fixture();
    let marker: PathBuf = repo.path().join("filter-was-executed");
    fs::write(
        repo.path().join(".gitattributes"),
        b"code.txt filter=fixture\n",
    )
    .unwrap();
    git(
        repo.path(),
        &[
            "config",
            "filter.fixture.clean",
            &format!("touch '{}'; cat", marker.display()),
        ],
    );
    assert!(
        WorkspacePlanner::preview(repo.path(), PreviewOptions::default())
            .await
            .unwrap_err()
            .to_string()
            .contains("unsupported_frontier")
    );
    assert!(!marker.exists());
}

#[tokio::test]
async fn safe_relative_symlink_remains_a_link_in_inventory() {
    let repo = fixture();
    symlink("code.txt", repo.path().join("link.txt")).unwrap();
    let preview = WorkspacePlanner::preview(repo.path(), PreviewOptions::default())
        .await
        .unwrap();
    let link = preview.entry("link.txt").unwrap();
    assert_eq!(link.link_target.as_deref(), Some("code.txt"));
    assert_ne!(link.sha256, preview.entry("code.txt").unwrap().sha256);
}

#[tokio::test]
async fn staged_deletions_and_rename_origins_remain_in_the_inventory() {
    let repo = fixture();
    git(repo.path(), &["rm", "deleted.txt"]);
    git(repo.path(), &["mv", "code.txt", "renamed.txt"]);
    let preview = WorkspacePlanner::preview(repo.path(), PreviewOptions::default())
        .await
        .unwrap();
    assert!(preview.entry("deleted.txt").unwrap().deleted);
    assert!(preview.entry("code.txt").unwrap().deleted);
    assert!(!preview.entry("renamed.txt").unwrap().deleted);
}

#[tokio::test]
async fn symlink_targets_cannot_traverse_another_symlink() {
    let repo = fixture();
    fs::create_dir(repo.path().join("nested")).unwrap();
    fs::write(repo.path().join("nested/code.txt"), b"nested").unwrap();
    symlink("nested", repo.path().join("directory-link")).unwrap();
    symlink("directory-link/code.txt", repo.path().join("chained-link")).unwrap();
    let error = WorkspacePlanner::preview(repo.path(), PreviewOptions::default())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("unsafe_path: chained link"));
}

#[tokio::test]
async fn sparse_and_assume_unchanged_entries_are_held_instead_of_treated_as_deletions() {
    for flag in ["--skip-worktree", "--assume-unchanged"] {
        let repo = fixture();
        git(repo.path(), &["update-index", flag, "code.txt"]);
        fs::remove_file(repo.path().join("code.txt")).unwrap();
        let index = fs::read(repo.path().join(".git/index")).unwrap();
        let error = WorkspacePlanner::preview(repo.path(), PreviewOptions::default())
            .await
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("unsupported_frontier: sparse or hidden index"));
        assert_eq!(fs::read(repo.path().join(".git/index")).unwrap(), index);
    }
}

#[tokio::test]
async fn partial_clone_is_rejected_without_object_hydration() {
    let repo = fixture();
    git(repo.path(), &["config", "remote.origin.promisor", "true"]);
    git(
        repo.path(),
        &["config", "remote.origin.partialclonefilter", "blob:none"],
    );
    let error = WorkspacePlanner::preview(repo.path(), PreviewOptions::default())
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("unsupported_frontier: partial clone"));
}

#[tokio::test]
async fn split_index_is_rejected_instead_of_archiving_an_incomplete_index() {
    let repo = fixture();
    git(repo.path(), &["update-index", "--split-index"]);
    assert!(!git(repo.path(), &["rev-parse", "--shared-index-path"]).is_empty());
    let error = WorkspacePlanner::preview(repo.path(), PreviewOptions::default())
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("unsupported_frontier: split index"));
}

#[tokio::test]
async fn aggregate_budget_is_checked_before_hashing_the_next_file() {
    let repo = fixture();
    let index_size = fs::metadata(repo.path().join(".git/index")).unwrap().len();
    let options = PreviewOptions {
        limits: InventoryLimits {
            max_total_bytes: index_size + 5,
            max_file_bytes: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    let error = WorkspacePlanner::preview(repo.path(), options)
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "continuation_budget_exceeded: preservation"
    );
}
