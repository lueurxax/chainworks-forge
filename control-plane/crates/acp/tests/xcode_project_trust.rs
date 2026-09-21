#![cfg(unix)]

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
