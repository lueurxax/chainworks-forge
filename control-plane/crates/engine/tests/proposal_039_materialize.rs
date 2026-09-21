use engine::run_carry_forward::{materialize, PreviewOptions, WorkspacePlanner};
use std::{
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    path::Path,
    process::Command,
};
use uuid::Uuid;

fn git(path: &Path, args: &[&str]) -> Vec<u8> {
    let output = Command::new("git")
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(path)
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

fn source(base: &Path) -> std::path::PathBuf {
    let source = base.join("source");
    fs::create_dir(&source).unwrap();
    git(&source, &["init", "-b", "main"]);
    git(&source, &["config", "user.name", "P039 Test"]);
    git(&source, &["config", "user.email", "p039@example.invalid"]);
    fs::write(source.join("code.txt"), b"original\n").unwrap();
    fs::write(source.join("deleted.txt"), b"old\n").unwrap();
    fs::write(source.join("rename.txt"), b"renamed bytes\n").unwrap();
    git(&source, &["add", "."]);
    git(&source, &["commit", "-m", "initial"]);
    source
}

#[tokio::test]
async fn first_preview_of_clean_materialized_checkout_never_refreshes_its_index() {
    let base = tempfile::tempdir().unwrap();
    let source = source(base.path());
    let preview = WorkspacePlanner::preview(&source, PreviewOptions::default())
        .await
        .unwrap();
    let receipt = materialize(&preview, base.path(), Uuid::new_v4())
        .await
        .unwrap();
    let index_path = std::path::PathBuf::from(
        String::from_utf8(git(
            &receipt.checkout_root,
            &["rev-parse", "--git-path", "index"],
        ))
        .unwrap()
        .trim(),
    );
    let before = fs::read(&index_path).unwrap();
    // read-tree populated the index before the overlay created the clean files.
    // The first inventory must not refresh the index it has just witnessed.
    let first = WorkspacePlanner::preview(&receipt.checkout_root, PreviewOptions::default())
        .await
        .unwrap();
    assert_eq!(fs::read(&index_path).unwrap(), before);
    assert!(git(
        &receipt.checkout_root,
        &[
            "-c",
            "diff.autoRefreshIndex=false",
            "diff",
            "--binary",
            "--no-ext-diff",
            "--no-textconv",
            "--"
        ]
    )
    .is_empty());
    let second = WorkspacePlanner::preview(&receipt.checkout_root, PreviewOptions::default())
        .await
        .unwrap();
    assert_eq!(fs::read(&index_path).unwrap(), before);
    assert_eq!(first.digest(), second.digest());
}

#[tokio::test]
async fn lfs_pointer_and_filter_are_rejected_without_hydration_or_filter_execution() {
    for configured in [false, true] {
        let base = tempfile::tempdir().unwrap();
        let source = source(base.path());
        let marker = base.path().join("filter-executed");
        fs::write(
            source.join(".gitattributes"),
            "*.bin filter=lfs diff=lfs merge=lfs -text\n",
        )
        .unwrap();
        let pointer = format!(
            "version https://git-lfs.github.com/spec/v1\noid sha256:{}\nsize 16777216\n",
            "a".repeat(64)
        );
        fs::write(source.join("asset.bin"), pointer.as_bytes()).unwrap();
        if configured {
            git(
                &source,
                &[
                    "config",
                    "filter.lfs.process",
                    &format!("touch '{}'", marker.display()),
                ],
            );
        }
        let index = fs::read(source.join(".git/index")).unwrap();
        let error = WorkspacePlanner::preview(&source, PreviewOptions::default())
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("unsupported_frontier"),
            "{error:#}"
        );
        assert!(!marker.exists());
        assert!(!source.join(".git/lfs").exists());
        assert_eq!(
            fs::read(source.join("asset.bin")).unwrap(),
            pointer.as_bytes()
        );
        assert_eq!(fs::read(source.join(".git/index")).unwrap(), index);
        assert!(git(&source, &["branch", "--list", "carry-forward-*"]).is_empty());
    }
}

#[tokio::test]
async fn staged_and_committed_submodules_are_rejected_without_initializing_them() {
    for committed in [false, true] {
        let base = tempfile::tempdir().unwrap();
        let source = source(base.path());
        let head = String::from_utf8(git(&source, &["rev-parse", "HEAD"])).unwrap();
        git(
            &source,
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("160000,{},vendor", head.trim()),
            ],
        );
        fs::write(
            source.join(".gitmodules"),
            "[submodule \"vendor\"]\n\tpath = vendor\n\turl = ../never-initialized\n",
        )
        .unwrap();
        if committed {
            git(&source, &["add", ".gitmodules"]);
            git(&source, &["commit", "-m", "fixture gitlink"]);
        }
        let index = fs::read(source.join(".git/index")).unwrap();
        let error = WorkspacePlanner::preview(&source, PreviewOptions::default())
            .await
            .unwrap_err();
        assert!(error.to_string().contains("submodule"), "{error:#}");
        assert!(!source.join("vendor").exists());
        assert!(!source.join(".git/modules").exists());
        assert_eq!(fs::read(source.join(".git/index")).unwrap(), index);
        assert!(git(&source, &["branch", "--list", "carry-forward-*"]).is_empty());
    }
}

#[tokio::test]
async fn materialized_checkout_is_independent_and_preserves_working_bytes_and_original_index() {
    for staged_deletion in [false, true] {
        let base = tempfile::tempdir().unwrap();
        let source = source(base.path());
        fs::write(source.join("code.txt"), b"staged\n").unwrap();
        git(&source, &["add", "code.txt"]);
        fs::write(source.join("code.txt"), b"final\0bytes\n").unwrap();
        fs::set_permissions(source.join("code.txt"), fs::Permissions::from_mode(0o755)).unwrap();
        if staged_deletion {
            git(&source, &["rm", "deleted.txt"]);
        } else {
            fs::remove_file(source.join("deleted.txt")).unwrap();
        }
        git(&source, &["mv", "rename.txt", "renamed.txt"]);
        fs::write(source.join("new.txt"), b"untracked\n").unwrap();
        symlink("code.txt", source.join("link.txt")).unwrap();
        let preview = WorkspacePlanner::preview(&source, PreviewOptions::default())
            .await
            .unwrap();
        let original_index = fs::read(source.join(".git/index")).unwrap();
        let receipt = materialize(&preview, base.path(), Uuid::new_v4())
            .await
            .unwrap();
        assert_ne!(receipt.checkout_root, source);
        assert_eq!(
            fs::read(receipt.checkout_root.join("code.txt")).unwrap(),
            b"final\0bytes\n"
        );
        assert_eq!(
            fs::read(receipt.checkout_root.join("new.txt")).unwrap(),
            b"untracked\n"
        );
        assert!(!receipt.checkout_root.join("deleted.txt").exists());
        assert!(!receipt.checkout_root.join("rename.txt").exists());
        assert_eq!(
            fs::read(receipt.checkout_root.join("renamed.txt")).unwrap(),
            b"renamed bytes\n"
        );
        assert_eq!(
            fs::read_link(receipt.checkout_root.join("link.txt"))
                .unwrap()
                .to_str(),
            Some("code.txt")
        );
        assert_eq!(
            fs::metadata(receipt.checkout_root.join("code.txt"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
        assert_eq!(
            git(&receipt.checkout_root, &["rev-parse", "HEAD"]),
            git(&source, &["rev-parse", "HEAD"])
        );
        assert!(git(&receipt.checkout_root, &["diff", "--cached", "--name-only"]).is_empty());
        assert_eq!(
            fs::read(receipt.preservation_root.join("index")).unwrap(),
            original_index
        );
        assert_eq!(fs::read(source.join(".git/index")).unwrap(), original_index);
        assert_eq!(
            preview.digest(),
            WorkspacePlanner::preview(&source, PreviewOptions::default())
                .await
                .unwrap()
                .digest()
        );
        fs::write(source.join("code.txt"), b"source later changes").unwrap();
        assert_eq!(
            fs::read(receipt.checkout_root.join("code.txt")).unwrap(),
            b"final\0bytes\n"
        );
        assert_eq!(
            fs::read(receipt.preservation_root.join("workspace/code.txt")).unwrap(),
            b"final\0bytes\n"
        );
        assert_eq!(
            fs::metadata(receipt.operation_root)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
}

#[tokio::test]
async fn source_change_is_rejected_before_creating_destination() {
    let base = tempfile::tempdir().unwrap();
    let source = source(base.path());
    let preview = WorkspacePlanner::preview(&source, PreviewOptions::default())
        .await
        .unwrap();
    fs::write(source.join("code.txt"), b"changed after preview").unwrap();
    let operation = Uuid::new_v4();
    assert!(materialize(&preview, base.path(), operation)
        .await
        .unwrap_err()
        .to_string()
        .contains("source_changed"));
    assert!(!base.path().join(operation.to_string()).exists());
}

#[tokio::test]
async fn occupied_destination_and_replay_never_overwrite_or_repeat_materialization() {
    let base = tempfile::tempdir().unwrap();
    let source = source(base.path());
    let preview = WorkspacePlanner::preview(&source, PreviewOptions::default())
        .await
        .unwrap();
    let operation = Uuid::new_v4();
    let receipt = materialize(&preview, base.path(), operation).await.unwrap();
    fs::write(receipt.checkout_root.join("marker"), b"keep").unwrap();
    assert!(materialize(&preview, base.path(), operation).await.is_err());
    assert_eq!(
        fs::read(receipt.checkout_root.join("marker")).unwrap(),
        b"keep"
    );
    assert!(materialize(&preview, &source, Uuid::new_v4())
        .await
        .is_err());
}

#[tokio::test]
async fn destination_cannot_be_inside_linked_worktree_git_metadata() {
    let base = tempfile::tempdir().unwrap();
    let main = source(base.path());
    let linked = base.path().join("linked");
    git(
        &main,
        &[
            "worktree",
            "add",
            "--detach",
            linked.to_str().unwrap(),
            "HEAD",
        ],
    );
    let preview = WorkspacePlanner::preview(&linked, PreviewOptions::default())
        .await
        .unwrap();
    let operation = Uuid::new_v4();
    let metadata = main.join(".git");
    let error = materialize(&preview, &metadata, operation)
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("unsafe_path: Git metadata destination"));
    assert!(!metadata.join(operation.to_string()).exists());
}

#[tokio::test]
async fn file_and_directory_replacements_preserve_the_final_tree_and_historical_deletions() {
    let base = tempfile::tempdir().unwrap();
    let source = source(base.path());
    fs::create_dir(source.join("container")).unwrap();
    fs::write(source.join("container/old.txt"), b"old leaf").unwrap();
    git(&source, &["add", "."]);
    git(&source, &["commit", "-m", "type change fixture"]);
    git(&source, &["rm", "code.txt", "container/old.txt"]);
    fs::create_dir(source.join("code.txt")).unwrap();
    fs::write(source.join("code.txt/new.txt"), b"new leaf").unwrap();
    fs::write(source.join("container"), b"new file").unwrap();
    git(&source, &["add", "."]);
    let preview = WorkspacePlanner::preview(&source, PreviewOptions::default())
        .await
        .unwrap();
    assert!(preview.entry("code.txt").unwrap().deleted);
    assert!(preview.entry("container/old.txt").unwrap().deleted);
    let receipt = materialize(&preview, base.path(), Uuid::new_v4())
        .await
        .unwrap();
    assert_eq!(
        fs::read(receipt.checkout_root.join("code.txt/new.txt")).unwrap(),
        b"new leaf"
    );
    assert_eq!(
        fs::read(receipt.checkout_root.join("container")).unwrap(),
        b"new file"
    );
}
