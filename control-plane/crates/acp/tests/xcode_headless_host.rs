use acp::{execution_root::resolve_execution_root, xcode_headless_host::TrustedProject};
use domain::execution_root::ResolvedExecutionRoot;
use std::{
    fs,
    path::{Path, PathBuf},
};

fn bundle(root: &Path, name: &str) -> PathBuf {
    let path = root.join(name);
    fs::create_dir_all(&path).unwrap();
    fs::write(
        path.join(if name.ends_with(".xcworkspace") {
            "contents.xcworkspacedata"
        } else {
            "project.pbxproj"
        }),
        b"fixture",
    )
    .unwrap();
    path.canonicalize().unwrap()
}

fn root(path: &Path) -> ResolvedExecutionRoot {
    resolve_execution_root(path.to_str().unwrap(), None, false, None).unwrap()
}

#[test]
fn explicit_project_pins_canonical_root_and_identity() {
    let temp = tempfile::tempdir().unwrap();
    let project = bundle(temp.path(), "A.xcodeproj");
    let trusted = TrustedProject::resolve(root(temp.path()), Some("A.xcodeproj"), 501).unwrap();
    let legacy =
        TrustedProject::resolve(root(temp.path()), Some("workspace:A.xcodeproj"), 501).unwrap();
    assert_eq!(legacy.key(), trusted.key());
    assert_eq!(trusted.key().canonical_path, project.to_str().unwrap());
    assert_eq!(trusted.key().uid, 501);
    assert_eq!(
        trusted.root().effective,
        temp.path().canonicalize().unwrap().to_str().unwrap()
    );
    trusted.revalidate().unwrap();
    fs::write(project.join("project.pbxproj"), b"ordinary project edit").unwrap();
    trusted.revalidate().unwrap();
}

#[test]
fn immediate_discovery_requires_one_bundle_and_never_descends() {
    let temp = tempfile::tempdir().unwrap();
    assert!(TrustedProject::resolve(root(temp.path()), None, 501).is_err());
    bundle(temp.path(), "nested/Hidden.xcodeproj");
    assert!(TrustedProject::resolve(root(temp.path()), None, 501).is_err());
    let a = bundle(temp.path(), "A.xcodeproj");
    bundle(&a, "project.xcworkspace");
    assert_eq!(
        TrustedProject::resolve(root(temp.path()), None, 501)
            .unwrap()
            .key()
            .canonical_path,
        a.to_str().unwrap()
    );
    bundle(temp.path(), "A.xcworkspace");
    assert!(TrustedProject::resolve(root(temp.path()), None, 501).is_err());
    TrustedProject::resolve(root(temp.path()), Some("A.xcworkspace"), 501).unwrap();
}

#[test]
fn worktree_remaps_exact_legacy_repository_path_without_checkout_fallback() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let tree = temp.path().join("tree");
    fs::create_dir(&repo).unwrap();
    fs::create_dir(&tree).unwrap();
    let original = bundle(&repo, "nested/A.xcodeproj");
    let effective = bundle(&tree, "nested/A.xcodeproj");
    let resolved = resolve_execution_root(
        repo.to_str().unwrap(),
        Some(tree.to_str().unwrap()),
        false,
        Some("shared_implementation_worktree"),
    )
    .unwrap();
    let hint = format!("workspace:{}", original.display());
    let selected = TrustedProject::resolve(resolved.clone(), Some(&hint), 501).unwrap();
    assert_eq!(selected.key().canonical_path, effective.to_str().unwrap());
    let direct = format!("workspace:{}", effective.display());
    assert_eq!(
        TrustedProject::resolve(resolved.clone(), Some(&direct), 501)
            .unwrap()
            .key(),
        selected.key()
    );
    fs::remove_dir_all(effective).unwrap();
    assert!(TrustedProject::resolve(resolved, Some(&hint), 501).is_err());
    assert!(original.exists());
}

#[test]
fn relative_legacy_workspace_selector_uses_effective_root_without_checkout_fallback() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let tree = temp.path().join("tree");
    fs::create_dir(&repo).unwrap();
    fs::create_dir(&tree).unwrap();
    let original = bundle(&repo, "Chainworks.xcworkspace");
    let effective = bundle(&tree, "Chainworks.xcworkspace");
    let hint = Some("workspace:Chainworks.xcworkspace");
    let repository_project = TrustedProject::resolve(root(&repo), hint, 501).unwrap();
    assert_eq!(
        repository_project.key().canonical_path,
        original.to_str().unwrap()
    );
    for strategy in ["dedicated", "shared_implementation_worktree"] {
        let resolved = resolve_execution_root(
            repo.to_str().unwrap(),
            Some(tree.to_str().unwrap()),
            false,
            Some(strategy),
        )
        .unwrap();
        let selected = TrustedProject::resolve(resolved.clone(), hint, 501).unwrap();
        assert_eq!(selected.key().canonical_path, effective.to_str().unwrap());
        selected.revalidate().unwrap();
        fs::rename(&effective, tree.join("hidden-workspace")).unwrap();
        assert!(TrustedProject::resolve(resolved, hint, 501).is_err());
        assert!(original.exists());
        fs::rename(tree.join("hidden-workspace"), &effective).unwrap();
    }
}

#[test]
fn malformed_missing_pid_and_escape_selectors_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    bundle(temp.path(), "A.xcodeproj");
    fs::create_dir(temp.path().join("Empty.xcworkspace")).unwrap();
    fs::write(temp.path().join("File.xcodeproj"), b"not directory").unwrap();
    for selector in [
        "",
        "pid:123",
        "123",
        "workspace:123",
        "workspace:",
        "workspace:pid:123",
        "workspace:../A.xcodeproj",
        "workspace:./A.xcodeproj",
        "workspace:nested/../A.xcodeproj",
        "workspace:Empty.xcworkspace",
        "workspace:File.xcodeproj",
        "workspace:Missing.xcodeproj",
        "workspace:A.xcodeproj/project.pbxproj",
        "../A.xcodeproj",
        "nested/../A.xcodeproj",
        "/outside/A.xcodeproj",
        "Empty.xcworkspace",
        "File.xcodeproj",
        "Missing.xcodeproj",
        "A.xcodeproj/project.pbxproj",
    ] {
        assert!(
            TrustedProject::resolve(root(temp.path()), Some(selector), 501).is_err(),
            "{selector}"
        );
    }
    fs::create_dir(temp.path().join("Bad.xcodeproj")).unwrap();
    fs::create_dir(temp.path().join("Bad.xcodeproj/project.pbxproj")).unwrap();
    assert!(TrustedProject::resolve(root(temp.path()), Some("Bad.xcodeproj"), 501).is_err());
}

#[test]
fn replaced_root_and_replaced_project_invalidate_existing_guard() {
    let temp = tempfile::tempdir().unwrap();
    let repository = temp.path().join("repo");
    fs::create_dir(&repository).unwrap();
    let path = bundle(&repository, "A.xcodeproj");
    let trusted = TrustedProject::resolve(root(&repository), None, 501).unwrap();
    fs::rename(&path, repository.join("old-project")).unwrap();
    bundle(&repository, "A.xcodeproj");
    assert!(trusted.revalidate().is_err());
    let trusted = TrustedProject::resolve(root(&repository), None, 501).unwrap();
    fs::rename(&repository, temp.path().join("old-root")).unwrap();
    fs::create_dir(&repository).unwrap();
    bundle(&repository, "A.xcodeproj");
    assert!(trusted.revalidate().is_err());
    assert!(TrustedProject::resolve(trusted.root().clone(), None, 501).is_err());
}

#[cfg(unix)]
#[test]
fn symlinked_package_parent_or_metadata_cannot_escape_root() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let external = bundle(outside.path(), "External.xcodeproj");
    symlink(&external, temp.path().join("Link.xcodeproj")).unwrap();
    symlink(outside.path(), temp.path().join("nested")).unwrap();
    let internal = bundle(temp.path(), "Internal.xcodeproj");
    fs::remove_file(internal.join("project.pbxproj")).unwrap();
    symlink(
        external.join("project.pbxproj"),
        internal.join("project.pbxproj"),
    )
    .unwrap();
    for name in [
        "Link.xcodeproj",
        "nested/External.xcodeproj",
        "Internal.xcodeproj",
    ] {
        assert!(
            TrustedProject::resolve(root(temp.path()), Some(name), 501).is_err(),
            "{name}"
        );
        let legacy = format!("workspace:{name}");
        assert!(
            TrustedProject::resolve(root(temp.path()), Some(&legacy), 501).is_err(),
            "{legacy}"
        );
    }
}

#[test]
fn discovery_has_a_bounded_directory_budget() {
    let temp = tempfile::tempdir().unwrap();
    bundle(temp.path(), "A.xcodeproj");
    for i in 0..4096 {
        fs::write(temp.path().join(format!("entry-{i}")), b"").unwrap();
    }
    assert!(TrustedProject::resolve(root(temp.path()), None, 501).is_err());
    TrustedProject::resolve(root(temp.path()), Some("A.xcodeproj"), 501).unwrap();
}
