#![cfg(target_os = "macos")]

use acp::{
    execution_root::resolve_execution_root, xcode_headless_host::TrustedProject,
    xcode_headless_runtime::HeadlessProjectTrust, xcode_project_trust::ProjectTrustStore,
};
use std::fs;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
use std::path::PathBuf;

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    directory: PathBuf,
    project: TrustedProject,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let checkout = root.join("checkout");
        fs::create_dir(&checkout).unwrap();
        fs::create_dir(checkout.join("App.xcodeproj")).unwrap();
        fs::write(checkout.join("App.xcodeproj/project.pbxproj"), b"fixture").unwrap();
        let resolved =
            resolve_execution_root(checkout.to_str().unwrap(), None, false, None).unwrap();
        let project =
            TrustedProject::resolve(resolved, Some("App.xcodeproj"), unsafe { libc::geteuid() })
                .unwrap();
        Self {
            _dir: dir,
            directory: root.join("trust"),
            root,
            project,
        }
    }

    fn store(&self) -> ProjectTrustStore {
        ProjectTrustStore::new(self.directory.clone())
    }

    fn record(&self) -> PathBuf {
        fs::read_dir(&self.directory)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| p.extension().is_some_and(|e| e == "json"))
            .unwrap()
    }

    fn worktree_project(&self, strategy: &str) -> TrustedProject {
        let checkout = self.root.join("checkout");
        let tree = self.root.join("worktree");
        fs::create_dir_all(tree.join("App.xcodeproj")).unwrap();
        fs::write(tree.join("App.xcodeproj/project.pbxproj"), b"fixture").unwrap();
        let root = resolve_execution_root(
            checkout.to_str().unwrap(),
            Some(tree.to_str().unwrap()),
            true,
            Some(strategy),
        )
        .unwrap();
        TrustedProject::resolve(root, Some("App.xcodeproj"), self.project.key().uid).unwrap()
    }

    // Construct shipped legacy formats directly, independently of v3 grants.
    fn legacy_grant(&self, project: &TrustedProject, revoked: bool) -> (PathBuf, String) {
        self.legacy_grant_version(project, revoked, 1)
    }

    fn legacy_grant_version(
        &self,
        project: &TrustedProject,
        revoked: bool,
        version: u32,
    ) -> (PathBuf, String) {
        assert!([1, 2].contains(&version));
        if !self.directory.exists() {
            fs::create_dir(&self.directory).unwrap();
            fs::set_permissions(&self.directory, fs::Permissions::from_mode(0o700)).unwrap();
            let lock = self.directory.join("trust.lock");
            fs::write(&lock, b"").unwrap();
            fs::set_permissions(lock, fs::Permissions::from_mode(0o600)).unwrap();
        }
        let record = serde_json::json!({
            "version": version,
            "grant_id": uuid::Uuid::new_v4(),
            "operator_id": "legacy-operator",
            "granted_at": "2026-09-27T10:00:00Z",
            "root": project.root(),
            "project_key": project.key(),
            "trust_policy_id": "trusted_local_project_v1",
            "trust_policy_digest": domain::xcode_contract::trust_policy_digest().unwrap(),
            "revoked_by": revoked.then_some("legacy-operator"),
            "revoked_at": revoked.then_some("2026-09-27T11:00:00Z"),
        });
        let mut key_root = project.root().clone();
        if version == 2 {
            key_root.strategy = None;
        }
        let name = domain::xcode_contract::canonical_digest(
            if version == 2 {
                "cw.xcode.project-trust-key.v2"
            } else {
                "cw.xcode.project-trust-key.v1"
            },
            &serde_json::json!({"root": key_root, "project_key": project.key()}),
        )
        .unwrap();
        let path = self.directory.join(format!("{name}.json"));
        fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let digest = domain::xcode_contract::canonical_digest(
            if version == 2 {
                "cw.xcode.project-trust.v2"
            } else {
                "cw.xcode.project-trust.v1"
            },
            &record,
        )
        .unwrap();
        (path, digest)
    }

    fn snapshot(&self) -> Vec<(PathBuf, Vec<u8>)> {
        let mut files = fs::read_dir(&self.directory)
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                let bytes = fs::read(&path).unwrap();
                (path, bytes)
            })
            .collect::<Vec<_>>();
        files.sort();
        files
    }
}

#[tokio::test]
async fn legacy_worktree_grants_require_explicit_v3_enrollment_without_changing_files() {
    for version in [1, 2] {
        for strategy in ["dedicated", "shared_implementation_worktree"] {
            let f = Fixture::new();
            let writer = f.worktree_project("dedicated");
            let reader = f.worktree_project("shared_implementation_worktree");
            let granted = f.worktree_project(strategy);
            f.legacy_grant_version(&granted, false, version);
            let before = f.snapshot();
            for project in [&writer, &reader] {
                assert_eq!(
                    f.store().check(project).await.unwrap_err().to_string(),
                    "project_trust_required"
                );
            }
            assert_eq!(f.snapshot(), before, "legacy lookup must be read-only");
            let digest = f.store().grant(&granted, "operator").unwrap();
            assert_eq!(f.store().check(&writer).await.unwrap(), digest);
            assert_eq!(f.store().check(&reader).await.unwrap(), digest);
            for (path, bytes) in before {
                assert_eq!(fs::read(path).unwrap(), bytes);
            }
        }
    }
}

#[tokio::test]
async fn new_worktree_grant_and_revoke_have_one_identity_across_writer_and_reader() {
    let f = Fixture::new();
    let writer = f.worktree_project("dedicated");
    let reader = f.worktree_project("shared_implementation_worktree");
    let digest = f.store().grant(&writer, "operator").unwrap();
    assert_eq!(f.store().check(&reader).await.unwrap(), digest);
    f.store().revoke(&reader, "operator").unwrap();
    for project in [&writer, &reader] {
        assert_eq!(
            f.store().check(project).await.unwrap_err().to_string(),
            "project_trust_revoked"
        );
    }
    let renewed = f.store().grant(&reader, "operator").unwrap();
    assert_ne!(renewed, digest);
    assert_eq!(f.store().check(&writer).await.unwrap(), renewed);
    assert_eq!(
        fs::read_dir(&f.directory).unwrap().count(),
        2,
        "one record plus lock"
    );
}

#[tokio::test]
async fn independent_legacy_grants_require_explicit_regrant_instead_of_arbitrary_selection() {
    let f = Fixture::new();
    let writer = f.worktree_project("dedicated");
    let reader = f.worktree_project("shared_implementation_worktree");
    f.legacy_grant(&writer, false);
    f.legacy_grant(&reader, false);
    let before = f.snapshot();
    for project in [&writer, &reader] {
        assert_eq!(
            f.store().check(project).await.unwrap_err().to_string(),
            "project_trust_required"
        );
    }
    assert_eq!(f.snapshot(), before);
    let digest = f.store().grant(&reader, "operator").unwrap();
    assert_eq!(f.store().check(&writer).await.unwrap(), digest);
    assert_eq!(f.store().check(&reader).await.unwrap(), digest);
    for (path, bytes) in before {
        assert_eq!(
            fs::read(path).unwrap(),
            bytes,
            "regrant does not rewrite history"
        );
    }
}

#[tokio::test]
async fn either_legacy_revocation_blocks_both_strategies_until_explicit_regrant() {
    for revoked_strategy in ["dedicated", "shared_implementation_worktree"] {
        let f = Fixture::new();
        let writer = f.worktree_project("dedicated");
        let reader = f.worktree_project("shared_implementation_worktree");
        f.legacy_grant(&writer, revoked_strategy == "dedicated");
        f.legacy_grant(
            &reader,
            revoked_strategy == "shared_implementation_worktree",
        );
        let before = f.snapshot();
        for project in [&writer, &reader] {
            assert_eq!(
                f.store().check(project).await.unwrap_err().to_string(),
                "project_trust_required"
            );
        }
        assert_eq!(f.snapshot(), before);
        let digest = f.store().grant(&reader, "operator").unwrap();
        assert_eq!(f.store().check(&writer).await.unwrap(), digest);
    }
}

#[tokio::test]
async fn legacy_alias_corruption_cannot_be_ignored_or_repaired_by_grant() {
    for damaged_strategy in ["dedicated", "shared_implementation_worktree"] {
        let f = Fixture::new();
        let writer = f.worktree_project("dedicated");
        let reader = f.worktree_project("shared_implementation_worktree");
        let (writer_path, _) = f.legacy_grant(&writer, false);
        let (reader_path, _) = f.legacy_grant(&reader, false);
        let damaged = if damaged_strategy == "dedicated" {
            writer_path
        } else {
            reader_path
        };
        fs::write(damaged, b"{broken").unwrap();
        let before = f.snapshot();
        for project in [&writer, &reader] {
            assert!(f.store().check(project).await.is_err());
            assert!(f.store().grant(project, "operator").is_err());
            assert!(f.store().revoke(project, "operator").is_err());
        }
        assert_eq!(f.snapshot(), before);
    }
}

#[tokio::test]
async fn cross_strategy_revoke_revokes_legacy_files_before_publishing_shared_tombstone() {
    for strategy in ["dedicated", "shared_implementation_worktree"] {
        let f = Fixture::new();
        let writer = f.worktree_project("dedicated");
        let reader = f.worktree_project("shared_implementation_worktree");
        let (writer_path, _) = f.legacy_grant(&writer, false);
        let (reader_path, _) = f.legacy_grant(&reader, false);
        let target = f.worktree_project(strategy);
        f.store().revoke(&target, "revoking-operator").unwrap();
        for project in [&writer, &reader] {
            assert_eq!(
                f.store().check(project).await.unwrap_err().to_string(),
                "project_trust_revoked"
            );
        }
        // A v1 binary reads only its original slot and checks revoked_at.
        for path in [writer_path, reader_path] {
            let record: serde_json::Value =
                serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
            assert_eq!(record["version"], 1);
            assert_eq!(record["revoked_by"], "revoking-operator");
            assert!(
                record["revoked_at"].is_string(),
                "old readers must also reject"
            );
        }
    }
}

#[tokio::test]
async fn authoritative_worktree_record_never_falls_back_to_an_active_legacy_grant() {
    for corrupt in [false, true] {
        let f = Fixture::new();
        let writer = f.worktree_project("dedicated");
        let reader = f.worktree_project("shared_implementation_worktree");
        let (legacy_path, _) = f.legacy_grant(&writer, false);
        let legacy_bytes = fs::read(&legacy_path).unwrap();
        f.store().grant(&reader, "operator").unwrap();
        if corrupt {
            let current = f
                .snapshot()
                .into_iter()
                .find(|(path, _)| {
                    path != &legacy_path && path.extension().is_some_and(|ext| ext == "json")
                })
                .unwrap()
                .0;
            fs::write(current, b"{broken").unwrap();
        } else {
            f.store().revoke(&reader, "operator").unwrap();
        }
        // Simulate a stale v1 writer restoring the old active slot. The v3
        // tombstone/corrupt record remains authoritative to upgraded readers.
        fs::write(legacy_path, legacy_bytes).unwrap();
        for project in [&writer, &reader] {
            assert!(f.store().check(project).await.is_err());
        }
    }
}

#[tokio::test]
async fn worktree_equivalence_does_not_cover_other_strategies_or_repository_roots() {
    let f = Fixture::new();
    let writer = f.worktree_project("dedicated");
    f.store().grant(&writer, "operator").unwrap();
    for strategy in ["meta_only", "unknown", ""] {
        let other = f.worktree_project(strategy);
        assert_eq!(other.key(), writer.key());
        assert_eq!(
            f.store().check(&other).await.unwrap_err().to_string(),
            "project_trust_required"
        );
    }
    let mut root = writer.root().clone();
    root.strategy = None;
    let legacy = TrustedProject::resolve(root, Some("App.xcodeproj"), writer.key().uid).unwrap();
    assert!(f.store().check(&legacy).await.is_err());

    let repository = f.root.join("checkout");
    let mut root = resolve_execution_root(repository.to_str().unwrap(), None, false, None).unwrap();
    root.strategy = Some("dedicated".into());
    let repository_writer =
        TrustedProject::resolve(root.clone(), Some("App.xcodeproj"), writer.key().uid).unwrap();
    f.store().grant(&repository_writer, "operator").unwrap();
    root.strategy = Some("shared_implementation_worktree".into());
    let repository_reader =
        TrustedProject::resolve(root, Some("App.xcodeproj"), writer.key().uid).unwrap();
    assert!(f.store().check(&repository_reader).await.is_err());
}

#[tokio::test]
async fn shared_worktree_identity_still_pins_repository_and_root_inode() {
    let f = Fixture::new();
    let writer = f.worktree_project("dedicated");
    f.store().grant(&writer, "operator").unwrap();
    let other_repository = f.root.join("other-repository");
    fs::create_dir(&other_repository).unwrap();
    let root = resolve_execution_root(
        other_repository.to_str().unwrap(),
        Some(&writer.root().effective),
        false,
        Some("shared_implementation_worktree"),
    )
    .unwrap();
    let other = TrustedProject::resolve(root, Some("App.xcodeproj"), writer.key().uid).unwrap();
    assert_eq!(other.key(), writer.key());
    assert!(f.store().check(&other).await.is_err());

    let tree = PathBuf::from(&writer.root().effective);
    let saved = f.root.join("old-worktree");
    fs::rename(&tree, &saved).unwrap();
    fs::create_dir(&tree).unwrap();
    fs::rename(saved.join("App.xcodeproj"), tree.join("App.xcodeproj")).unwrap();
    let replaced = f.worktree_project("shared_implementation_worktree");
    assert_eq!(replaced.key(), writer.key(), "project inode was retained");
    assert_ne!(replaced.root().inode, writer.root().inode);
    assert!(f.store().check(&replaced).await.is_err());
}

#[tokio::test]
async fn revoking_a_single_legacy_alias_never_synthesizes_a_peer_legacy_grant() {
    let f = Fixture::new();
    let writer = f.worktree_project("dedicated");
    let reader = f.worktree_project("shared_implementation_worktree");
    f.legacy_grant(&writer, false);
    f.store().revoke(&reader, "operator").unwrap();
    assert_eq!(fs::read_dir(&f.directory).unwrap().count(), 3);
    for project in [&writer, &reader] {
        assert_eq!(
            f.store().check(project).await.unwrap_err().to_string(),
            "project_trust_revoked"
        );
    }
}

#[tokio::test]
async fn lookup_never_bootstraps_or_implicitly_grants_project_trust() {
    for missing_parent in [false, true] {
        let f = Fixture::new();
        let directory = if missing_parent {
            f.root.join("absent-parent").join("trust")
        } else {
            f.directory.clone()
        };
        let store = ProjectTrustStore::new(directory.clone());
        let error = store.check(&f.project).await.unwrap_err();
        assert_eq!(error.to_string(), "project_trust_required");
        assert!(!directory.exists());
        assert!(!f.root.join("absent-parent").exists());
        assert_eq!(fs::read_dir(&f.root).unwrap().count(), 1);
    }
}

#[tokio::test]
async fn explicit_grant_is_durable_exact_and_revocation_invalidates_its_digest() {
    let f = Fixture::new();
    assert_eq!(
        f.store().check(&f.project).await.unwrap_err().to_string(),
        "project_trust_required"
    );
    let digest = f.store().grant(&f.project, "operator-a").unwrap();
    assert_eq!(digest.len(), 64);
    assert_eq!(f.store().check(&f.project).await.unwrap(), digest);
    f.store().revoke(&f.project, "operator-b").unwrap();
    assert_eq!(
        f.store().check(&f.project).await.unwrap_err().to_string(),
        "project_trust_revoked"
    );
    let renewed = f.store().grant(&f.project, "operator-a").unwrap();
    assert_ne!(renewed, digest);
    assert_eq!(f.store().check(&f.project).await.unwrap(), renewed);
}

#[test]
fn trust_records_are_private_and_bind_operator_policy_root_and_project() {
    let f = Fixture::new();
    f.store().grant(&f.project, "operator-a").unwrap();
    let path = f.record();
    assert_eq!(fs::metadata(&f.directory).unwrap().mode() & 0o777, 0o700);
    assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
    assert_eq!(fs::metadata(&path).unwrap().uid(), unsafe {
        libc::geteuid()
    });
    let record: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(record["operator_id"], "operator-a");
    assert_eq!(record["trust_policy_id"], "trusted_local_project_v1");
    assert_eq!(
        record["root"],
        serde_json::to_value(f.project.root()).unwrap()
    );
    assert_eq!(
        record["project_key"],
        serde_json::to_value(f.project.key()).unwrap()
    );
}

#[tokio::test]
async fn same_project_under_another_execution_root_is_not_trusted() {
    let f = Fixture::new();
    f.store().grant(&f.project, "operator").unwrap();
    let root = resolve_execution_root(f.root.to_str().unwrap(), None, false, None).unwrap();
    let other =
        TrustedProject::resolve(root, Some("checkout/App.xcodeproj"), f.project.key().uid).unwrap();
    assert_eq!(other.key(), f.project.key());
    assert_eq!(
        f.store().check(&other).await.unwrap_err().to_string(),
        "project_trust_required"
    );
}

#[tokio::test]
async fn equivalent_project_in_another_checkout_requires_its_own_grant() {
    let f = Fixture::new();
    let other = Fixture::new();
    let digest = f.store().grant(&f.project, "operator").unwrap();
    assert_eq!(
        f.store()
            .check(&other.project)
            .await
            .unwrap_err()
            .to_string(),
        "project_trust_required"
    );
    assert_eq!(f.store().check(&f.project).await.unwrap(), digest);
}

#[tokio::test]
async fn project_replacement_does_not_inherit_existing_trust() {
    let f = Fixture::new();
    f.store().grant(&f.project, "operator").unwrap();
    let path = PathBuf::from(&f.project.key().canonical_path);
    fs::rename(&path, path.with_extension("previous")).unwrap();
    fs::create_dir(&path).unwrap();
    fs::write(path.join("project.pbxproj"), b"replacement").unwrap();
    assert!(f.store().check(&f.project).await.is_err());
    let replacement = TrustedProject::resolve(
        f.project.root().clone(),
        Some("App.xcodeproj"),
        f.project.key().uid,
    )
    .unwrap();
    assert!(f.store().check(&replacement).await.is_err());
}

#[tokio::test]
async fn corrupt_policy_or_unknown_fields_fail_closed() {
    for field in ["trust_policy_id", "unexpected"] {
        let f = Fixture::new();
        f.store().grant(&f.project, "operator").unwrap();
        let path = f.record();
        let mut record: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        record[field] = serde_json::json!("not-an-approved-policy");
        fs::write(path, serde_json::to_vec(&record).unwrap()).unwrap();
        assert!(f.store().check(&f.project).await.is_err());
    }
}

#[tokio::test]
async fn trust_symlinks_hardlinks_and_permissive_modes_are_rejected() {
    for mode in ["symlink", "hardlink", "permissions"] {
        let f = Fixture::new();
        f.store().grant(&f.project, "operator").unwrap();
        let path = f.record();
        if mode == "permissions" {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        } else {
            let saved = f.root.join("record-backup");
            fs::rename(&path, &saved).unwrap();
            if mode == "symlink" {
                symlink(saved, &path).unwrap();
            } else {
                fs::hard_link(saved, &path).unwrap();
            }
        }
        assert!(f.store().check(&f.project).await.is_err());
        assert!(f.store().grant(&f.project, "operator").is_err());
    }
}

#[test]
fn grant_rejects_foreign_uid_and_never_repairs_insecure_store_permissions() {
    let f = Fixture::new();
    let foreign = TrustedProject::resolve(
        f.project.root().clone(),
        Some("App.xcodeproj"),
        f.project.key().uid + 1,
    )
    .unwrap();
    assert!(f.store().grant(&foreign, "operator").is_err());
    fs::create_dir(&f.directory).unwrap();
    fs::set_permissions(&f.directory, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(f.store().grant(&f.project, "operator").is_err());
    assert_eq!(fs::metadata(&f.directory).unwrap().mode() & 0o777, 0o755);
}

#[tokio::test]
async fn stable_kernel_lock_serializes_grant_revoke_and_lookup() {
    let f = Fixture::new();
    f.store().grant(&f.project, "operator").unwrap();
    let path = f.directory.join("trust.lock");
    let inode = fs::metadata(&path).unwrap().ino();
    let lock = fs::File::open(&path).unwrap();
    assert_eq!(
        unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
    assert_eq!(
        f.store().check(&f.project).await.unwrap_err().to_string(),
        "project_trust_busy"
    );
    assert!(f.store().grant(&f.project, "operator").is_err());
    assert!(f.store().revoke(&f.project, "operator").is_err());
    drop(lock);
    f.store().revoke(&f.project, "operator").unwrap();
    f.store().grant(&f.project, "operator").unwrap();
    assert_eq!(fs::metadata(path).unwrap().ino(), inode);
}

#[test]
fn missing_revoke_and_symlinked_parent_never_create_a_trust_store() {
    let f = Fixture::new();
    assert!(f.store().revoke(&f.project, "operator").is_err());
    assert!(!f.directory.exists());
    let actual = f.root.join("actual");
    fs::create_dir(&actual).unwrap();
    let alias = f.root.join("alias");
    symlink(&actual, &alias).unwrap();
    let store = ProjectTrustStore::new(alias.join("trust"));
    assert!(store.grant(&f.project, "operator").is_err());
    assert!(!actual.join("trust").exists());
}

#[tokio::test]
async fn unsafe_store_paths_are_not_classified_as_missing_trust() {
    for kind in [
        "symlinked-parent",
        "dangling-symlink",
        "file",
        "permissions",
    ] {
        let f = Fixture::new();
        let directory = match kind {
            "symlinked-parent" => {
                let actual = f.root.join("actual");
                fs::create_dir(&actual).unwrap();
                let alias = f.root.join("alias");
                symlink(&actual, &alias).unwrap();
                alias.join("trust")
            }
            "dangling-symlink" => {
                symlink(f.root.join("missing-target"), &f.directory).unwrap();
                f.directory.clone()
            }
            "file" => {
                fs::write(&f.directory, b"not a directory").unwrap();
                f.directory.clone()
            }
            "permissions" => {
                f.store().grant(&f.project, "operator").unwrap();
                fs::set_permissions(&f.directory, fs::Permissions::from_mode(0o755)).unwrap();
                f.directory.clone()
            }
            _ => unreachable!(),
        };
        let error = ProjectTrustStore::new(directory)
            .check(&f.project)
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), "insecure_authority", "{kind}");
        assert!(!f.root.join("actual/trust").exists());
        assert!(!f.root.join("missing-target").exists());
    }
}

#[tokio::test]
async fn missing_or_hardlinked_trust_lock_is_not_repaired() {
    for hardlink in [false, true] {
        let f = Fixture::new();
        f.store().grant(&f.project, "operator").unwrap();
        let path = f.directory.join("trust.lock");
        if hardlink {
            fs::hard_link(&path, f.root.join("lock-alias")).unwrap();
        } else {
            fs::remove_file(&path).unwrap();
        }
        let error = f.store().check(&f.project).await.unwrap_err();
        assert_ne!(error.to_string(), "project_trust_required");
        assert!(f.store().grant(&f.project, "operator").is_err());
        assert_eq!(path.exists(), hardlink);
    }
}

// These tests alter only persisted fixture evidence. Admission still observes
// real APFS directories through the production observer and public API.
fn rewrite_record(f: &Fixture, edit: impl FnOnce(&mut serde_json::Value)) {
    let path = f.record();
    let mut record: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    edit(&mut record);
    fs::write(path, serde_json::to_vec(&record).unwrap()).unwrap();
}

fn previous_device(record: &mut serde_json::Value, component: &str, cross_boot: bool) {
    let observation = &mut record["filesystem"][component];
    let historical_device = observation["device"].as_u64().unwrap() + 1;
    observation["device"] = historical_device.into();
    if cross_boot {
        observation["boot_session_uuid"] = "bff3189b-e088-45c2-88ee-c63941ba0ddb".into();
    }
    if component == "root" {
        record["root"]["device"] = historical_device.to_string().into();
    } else {
        record["project_key"]["device"] = historical_device.to_string().into();
    }
}

#[tokio::test]
async fn v3_native_apfs_grant_accepts_only_cross_boot_device_drift_without_rewriting() {
    for cross_boot in [false, true] {
        let f = Fixture::new();
        f.store().grant(&f.project, "operator").unwrap();
        rewrite_record(&f, |record| {
            assert_eq!(record["version"], 3);
            for component in ["root", "project"] {
                assert_eq!(
                    record["filesystem"][component]["persistent"]["filesystem"],
                    "apfs"
                );
                assert!(
                    record["filesystem"][component]["birth_sec"]
                        .as_i64()
                        .unwrap()
                        > 0
                );
                previous_device(record, component, cross_boot);
            }
        });
        let before = f.snapshot();
        let result = f.store().check(&f.project).await;
        if cross_boot {
            assert!(result.is_ok(), "{result:?}");
        } else {
            assert_eq!(
                result.unwrap_err().to_string(),
                "persistent_identity_changed"
            );
        }
        assert_eq!(
            f.snapshot(),
            before,
            "runtime reads never update historical evidence"
        );
    }
}

#[tokio::test]
async fn v3_root_and_project_foreign_volume_replacement_and_unsupported_evidence_fail_closed() {
    for component in ["root", "project"] {
        for changed in ["volume", "birth", "unsupported", "missing", "boot_mismatch"] {
            let f = Fixture::new();
            f.store().grant(&f.project, "operator").unwrap();
            rewrite_record(&f, |record| {
                let observation = &mut record["filesystem"][component];
                match changed {
                    "volume" => {
                        observation["persistent"]["volume_uuid"] =
                            "bff3189b-e088-45c2-88ee-c63941ba0ddb".into()
                    }
                    "birth" => {
                        observation["birth_nsec"] =
                            ((observation["birth_nsec"].as_i64().unwrap() + 1) % 1_000_000_000)
                                .into()
                    }
                    "unsupported" => observation["persistent"]["filesystem"] = "hfs".into(),
                    "missing" => {
                        observation.as_object_mut().unwrap().remove("persistent");
                    }
                    "boot_mismatch" => {
                        observation["boot_session_uuid"] =
                            "bff3189b-e088-45c2-88ee-c63941ba0ddb".into()
                    }
                    _ => unreachable!(),
                }
            });
            let before = f.snapshot();
            assert!(
                f.store().check(&f.project).await.is_err(),
                "{component}/{changed}"
            );
            assert!(
                f.store().grant(&f.project, "operator").is_err(),
                "{component}/{changed}"
            );
            assert_eq!(
                f.snapshot(),
                before,
                "damaged evidence cannot be repaired by grant"
            );
        }
    }
}

#[tokio::test]
async fn legacy_v2_exact_corruption_blocks_enrollment_and_revocation() {
    let f = Fixture::new();
    let writer = f.worktree_project("dedicated");
    let reader = f.worktree_project("shared_implementation_worktree");
    let (path, _) = f.legacy_grant_version(&writer, false, 2);
    fs::write(path, b"{broken").unwrap();
    let before = f.snapshot();
    assert!(f.store().grant(&reader, "operator").is_err());
    assert!(f.store().revoke(&reader, "operator").is_err());
    assert_eq!(f.snapshot(), before);
}

#[tokio::test]
async fn legacy_v2_revocation_publishes_denial_and_explicit_enrollment_preserves_legacy_bytes() {
    let f = Fixture::new();
    let writer = f.worktree_project("dedicated");
    let reader = f.worktree_project("shared_implementation_worktree");
    let (path, _) = f.legacy_grant_version(&writer, false, 2);
    f.store().revoke(&reader, "operator").unwrap();
    let bytes = fs::read(&path).unwrap();
    let revoked: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(revoked["version"], 2);
    assert!(revoked["revoked_at"].is_string());
    assert_eq!(
        f.store().check(&writer).await.unwrap_err().to_string(),
        "project_trust_revoked"
    );
    f.store().grant(&writer, "operator").unwrap();
    assert!(f.store().check(&reader).await.is_ok());
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[tokio::test]
async fn legacy_repository_slots_never_authorize_and_old_device_slots_are_not_scanned_or_rewritten()
{
    for old_device in [false, true] {
        let f = Fixture::new();
        let (mut path, _) = f.legacy_grant(&f.project, false);
        if old_device {
            let mut record: serde_json::Value =
                serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            for field in ["root", "project_key"] {
                record[field]["device"] = (record[field]["device"]
                    .as_str()
                    .unwrap()
                    .parse::<u64>()
                    .unwrap()
                    + 1)
                .to_string()
                .into();
            }
            let old_name = domain::xcode_contract::canonical_digest(
                "cw.xcode.project-trust-key.v1",
                &serde_json::json!({"root": record["root"], "project_key": record["project_key"]}),
            )
            .unwrap();
            let old_path = f.directory.join(format!("{old_name}.json"));
            fs::rename(path, &old_path).unwrap();
            fs::write(&old_path, serde_json::to_vec(&record).unwrap()).unwrap();
            path = old_path;
        }
        let before = f.snapshot();
        let legacy_bytes = fs::read(&path).unwrap();
        assert_eq!(
            f.store().check(&f.project).await.unwrap_err().to_string(),
            "project_trust_required"
        );
        if old_device {
            assert_eq!(
                f.store()
                    .revoke(&f.project, "operator")
                    .unwrap_err()
                    .to_string(),
                "project_trust_required"
            );
        }
        assert_eq!(f.snapshot(), before);
        f.store().grant(&f.project, "operator").unwrap();
        assert_eq!(
            fs::read(&path).unwrap(),
            legacy_bytes,
            "grant preserves every historical slot"
        );
        assert!(f.store().check(&f.project).await.is_ok());
        f.store().revoke(&f.project, "operator").unwrap();
        assert_eq!(
            f.store().check(&f.project).await.unwrap_err().to_string(),
            "project_trust_revoked"
        );
        if old_device {
            assert_eq!(
                fs::read(&path).unwrap(),
                legacy_bytes,
                "revoke never scans old-device slots"
            );
        } else {
            let legacy: serde_json::Value =
                serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            assert!(
                legacy["revoked_at"].is_string(),
                "discoverable exact legacy slots are revoked"
            );
        }
    }
}

#[tokio::test]
async fn v3_evidence_is_mandatory_and_duplicate_identity_fields_are_rejected() {
    for damage in ["missing", "legacy_version", "duplicate"] {
        let f = Fixture::new();
        f.store().grant(&f.project, "operator").unwrap();
        let path = f.record();
        let mut record: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        match damage {
            "missing" => {
                record.as_object_mut().unwrap().remove("filesystem");
            }
            "legacy_version" => {
                record["version"] = 1.into();
                record.as_object_mut().unwrap().remove("filesystem");
            }
            "duplicate" => (),
            _ => unreachable!(),
        }
        let mut bytes = serde_json::to_string(&record).unwrap();
        if damage == "duplicate" {
            bytes = bytes.replacen("\"birth_sec\":", "\"birth_sec\":1,\"birth_sec\":", 1);
        }
        fs::write(path, bytes).unwrap();
        let before = f.snapshot();
        assert!(f.store().check(&f.project).await.is_err(), "{damage}");
        assert!(f.store().grant(&f.project, "operator").is_err(), "{damage}");
        assert!(
            f.store().revoke(&f.project, "operator").is_err(),
            "{damage}"
        );
        assert_eq!(f.snapshot(), before);
    }
}

#[test]
fn legacy_schema_does_not_accept_a_null_v3_identity_extension() {
    for version in [1, 2] {
        let f = Fixture::new();
        let writer = f.worktree_project("dedicated");
        let (path, _) = f.legacy_grant_version(&writer, false, version);
        let mut record: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        record["filesystem"] = serde_json::Value::Null;
        fs::write(path, serde_json::to_vec(&record).unwrap()).unwrap();
        let before = f.snapshot();
        assert!(
            f.store().grant(&writer, "operator").is_err(),
            "legacy {version} accepted a field outside its schema"
        );
        assert!(f.store().revoke(&writer, "operator").is_err());
        assert_eq!(f.snapshot(), before);
    }
}
