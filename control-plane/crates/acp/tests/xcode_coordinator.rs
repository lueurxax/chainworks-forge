#![cfg(unix)]

use acp::xcode_coordinator::{
    AccessMode, CoordinatorError, CoordinatorLimits, FixtureJournalAuthority, ProjectHoldCheck,
    WorkspaceAccessCoordinator,
};
use domain::xcode_effect::ProjectKey;
use std::fs;
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;

#[derive(Default)]
struct Holds(AtomicU8);

#[async_trait::async_trait]
impl ProjectHoldCheck for Holds {
    async fn is_held(&self, _: &ProjectKey) -> Result<bool, CoordinatorError> {
        match self.0.load(Ordering::SeqCst) {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(CoordinatorError::HoldCheckUnavailable),
        }
    }
}

struct Fixture {
    _dir: tempfile::TempDir,
    authority: PathBuf,
    database: PathBuf,
    project: PathBuf,
    holds: Arc<Holds>,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let database = root.join("journal.sqlite");
        let project = root.join("Project.xcodeproj");
        fs::write(&database, b"disposable journal identity fixture").unwrap();
        fs::create_dir(&project).unwrap();
        Self {
            _dir: dir,
            authority: root.join("authority"),
            database,
            project,
            holds: Arc::default(),
        }
    }

    fn open(&self) -> Result<Arc<WorkspaceAccessCoordinator>, CoordinatorError> {
        self.open_with(CoordinatorLimits {
            queue_capacity: 2,
            queue_timeout: Duration::from_millis(100),
        })
    }

    fn open_authority(&self) -> Result<Arc<FixtureJournalAuthority>, CoordinatorError> {
        FixtureJournalAuthority::open(&self.authority, &self.database)
    }

    fn open_with(
        &self,
        limits: CoordinatorLimits,
    ) -> Result<Arc<WorkspaceAccessCoordinator>, CoordinatorError> {
        WorkspaceAccessCoordinator::new(limits, self.holds.clone())
    }
}

#[test]
fn kernel_lock_excludes_another_coordinator_and_is_never_unlinked() {
    let f = Fixture::new();
    let first = f.open_authority().unwrap();
    let lock = f.authority.join("coordinator.lock");
    let inode = fs::metadata(&lock).unwrap().ino();
    assert!(matches!(
        f.open_authority(),
        Err(CoordinatorError::AuthorityBusy)
    ));
    drop(first);
    assert_eq!(fs::metadata(&lock).unwrap().ino(), inode);
    let second = f.open_authority().unwrap();
    assert_eq!(fs::metadata(&lock).unwrap().ino(), inode);
    second.check().unwrap();
}

#[test]
fn bootstrap_persists_restrictive_authority_and_pins_database_identity() {
    let mut f = Fixture::new();
    drop(f.open_authority().unwrap());
    for (path, mode) in [
        (f.authority.clone(), 0o700),
        (f.authority.join("coordinator.lock"), 0o600),
        (f.authority.join("authority.json"), 0o600),
    ] {
        let metadata = fs::symlink_metadata(path).unwrap();
        assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
        assert_eq!(metadata.mode() & 0o777, mode);
    }
    f.database = f.database.with_file_name("other.sqlite");
    fs::write(&f.database, b"different database").unwrap();
    assert!(matches!(
        f.open_authority(),
        Err(CoordinatorError::AuthorityMismatch)
    ));
}

#[test]
fn database_alias_resolves_to_the_same_identity_but_path_replacement_does_not() {
    let mut f = Fixture::new();
    drop(f.open_authority().unwrap());
    let original = f.database.clone();
    f.database = original.with_file_name("database-alias");
    symlink(&original, &f.database).unwrap();
    drop(f.open_authority().unwrap());
    fs::rename(&original, original.with_extension("old")).unwrap();
    fs::write(&original, b"replacement").unwrap();
    assert!(matches!(
        f.open_authority(),
        Err(CoordinatorError::AuthorityMismatch)
    ));
}

#[test]
fn missing_or_corrupt_record_never_silently_bootstraps_again() {
    for corrupt in [false, true] {
        let f = Fixture::new();
        drop(f.open_authority().unwrap());
        let record = f.authority.join("authority.json");
        if corrupt {
            fs::write(record, b"{truncated").unwrap();
        } else {
            fs::remove_file(record).unwrap();
        }
        assert!(matches!(
            f.open_authority(),
            Err(CoordinatorError::AuthorityMissing | CoordinatorError::AuthorityCorrupt)
        ));
    }
}

#[test]
fn insecure_authority_nodes_are_rejected_without_repairing_them() {
    for node in ["", "coordinator.lock", "authority.json"] {
        let f = Fixture::new();
        drop(f.open_authority().unwrap());
        let path = f.authority.join(node);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(matches!(
            f.open_authority(),
            Err(CoordinatorError::InsecureAuthority)
        ));
        assert_eq!(fs::metadata(path).unwrap().mode() & 0o777, 0o777);
    }
}

#[test]
fn authority_symlinks_and_hardlinked_files_are_rejected() {
    for node in ["coordinator.lock", "authority.json"] {
        for hardlink in [false, true] {
            let f = Fixture::new();
            drop(f.open_authority().unwrap());
            let path = f.authority.join(node);
            let original = f.authority.with_file_name(format!("saved-{node}"));
            fs::rename(&path, &original).unwrap();
            if hardlink {
                fs::hard_link(&original, &path).unwrap();
            } else {
                symlink(&original, &path).unwrap();
            }
            assert!(matches!(
                f.open_authority(),
                Err(CoordinatorError::InsecureAuthority)
            ));
        }
    }
    let f = Fixture::new();
    let real = f.authority.with_file_name("real-authority");
    fs::create_dir(&real).unwrap();
    symlink(real, &f.authority).unwrap();
    assert!(matches!(
        f.open_authority(),
        Err(CoordinatorError::InsecureAuthority)
    ));
}

#[tokio::test]
async fn readers_share_but_writer_excludes_readers_across_shared_arc_clients() {
    let f = Fixture::new();
    let coordinator = f.open().unwrap();
    let other_client = coordinator.clone();
    let key = coordinator.project_key(&f.project).unwrap();
    let owner = coordinator.new_owner(Instant::now() + Duration::from_secs(5));
    let other = coordinator.new_owner(Instant::now() + Duration::from_secs(5));
    let a = coordinator
        .acquire(&owner, &key, AccessMode::Read)
        .await
        .unwrap();
    let b = other_client
        .acquire(&other, &key, AccessMode::Read)
        .await
        .unwrap();
    assert!(matches!(
        coordinator.acquire(&other, &key, AccessMode::Mutate).await,
        Err(CoordinatorError::QueueTimeout)
    ));
    drop((a, b));
    let writer = coordinator
        .acquire(&owner, &key, AccessMode::Mutate)
        .await
        .unwrap();
    assert!(matches!(
        other_client.acquire(&other, &key, AccessMode::Read).await,
        Err(CoordinatorError::QueueTimeout)
    ));
    drop(writer);
}

#[tokio::test]
async fn queued_writer_cannot_be_overtaken_and_queue_cancellation_frees_capacity() {
    let f = Fixture::new();
    let coordinator = f
        .open_with(CoordinatorLimits {
            queue_capacity: 2,
            queue_timeout: Duration::from_secs(5),
        })
        .unwrap();
    let key = coordinator.project_key(&f.project).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let a = coordinator.new_owner(deadline);
    let b = coordinator.new_owner(deadline);
    let c = coordinator.new_owner(deadline);
    let d = coordinator.new_owner(deadline);
    let first = coordinator
        .acquire(&a, &key, AccessMode::Read)
        .await
        .unwrap();
    let mut writer = Box::pin(coordinator.acquire(&b, &key, AccessMode::Mutate));
    assert!(tokio::time::timeout(Duration::from_millis(10), &mut writer)
        .await
        .is_err());
    let mut reader = Box::pin(coordinator.acquire(&c, &key, AccessMode::Read));
    assert!(tokio::time::timeout(Duration::from_millis(10), &mut reader)
        .await
        .is_err());
    assert!(matches!(
        coordinator.acquire(&d, &key, AccessMode::Read).await,
        Err(CoordinatorError::QueueFull)
    ));
    drop(reader);
    let mut last = Box::pin(coordinator.acquire(&d, &key, AccessMode::Read));
    assert!(tokio::time::timeout(Duration::from_millis(10), &mut last)
        .await
        .is_err());
    drop(first);
    let exclusive = writer.await.unwrap();
    assert!(tokio::time::timeout(Duration::from_millis(10), &mut last)
        .await
        .is_err());
    drop(exclusive);
    last.await.unwrap().check_dispatch().await.unwrap();
}

#[tokio::test]
async fn nested_permits_cannot_upgrade_or_outlive_owner_and_keep_exclusion_until_dropped() {
    let f = Fixture::new();
    let coordinator = f.open().unwrap();
    let key = coordinator.project_key(&f.project).unwrap();
    let owner = coordinator.new_owner(Instant::now() + Duration::from_secs(5));
    let read = coordinator
        .acquire(&owner, &key, AccessMode::Read)
        .await
        .unwrap();
    assert!(matches!(
        read.borrow_nested(AccessMode::Mutate).await,
        Err(CoordinatorError::UpgradeDenied)
    ));
    drop(read);
    let writer = coordinator
        .acquire(&owner, &key, AccessMode::Mutate)
        .await
        .unwrap();
    let nested = writer.borrow_nested(AccessMode::Read).await.unwrap();
    drop(writer);
    drop(owner);
    assert_eq!(
        nested.check_dispatch().await,
        Err(CoordinatorError::OwnerRevoked)
    );
    let fresh = coordinator.new_owner(Instant::now() + Duration::from_secs(5));
    assert!(matches!(
        coordinator.acquire(&fresh, &key, AccessMode::Read).await,
        Err(CoordinatorError::QueueTimeout)
    ));
    drop(nested);
    coordinator
        .acquire(&fresh, &key, AccessMode::Mutate)
        .await
        .unwrap();
}

#[tokio::test]
async fn explicit_revocation_and_foreign_owner_cannot_authorize_dispatch() {
    let f = Fixture::new();
    let coordinator = f.open().unwrap();
    let key = coordinator.project_key(&f.project).unwrap();
    let owner = coordinator.new_owner(Instant::now() + Duration::from_secs(5));
    let permit = coordinator
        .acquire(&owner, &key, AccessMode::Read)
        .await
        .unwrap();
    owner.revoke();
    assert_eq!(
        permit.check_dispatch().await,
        Err(CoordinatorError::OwnerRevoked)
    );
    let other = Fixture::new();
    let other_coordinator = other.open().unwrap();
    let foreign = other_coordinator.new_owner(Instant::now() + Duration::from_secs(5));
    assert!(matches!(
        coordinator.acquire(&foreign, &key, AccessMode::Read).await,
        Err(CoordinatorError::ForeignOwner)
    ));
}

#[tokio::test(start_paused = true)]
async fn nested_request_never_renews_deadline_and_expired_waiter_is_removed() {
    let f = Fixture::new();
    let coordinator = f.open().unwrap();
    let key = coordinator.project_key(&f.project).unwrap();
    let owner = coordinator.new_owner(Instant::now() + Duration::from_secs(1));
    let permit = coordinator
        .acquire(&owner, &key, AccessMode::Mutate)
        .await
        .unwrap();
    tokio::time::advance(Duration::from_millis(900)).await;
    let nested = permit.borrow_nested(AccessMode::Read).await.unwrap();
    tokio::time::advance(Duration::from_millis(101)).await;
    assert_eq!(
        nested.check_dispatch().await,
        Err(CoordinatorError::DeadlineExpired)
    );
    assert!(matches!(
        permit.borrow_nested(AccessMode::Read).await,
        Err(CoordinatorError::DeadlineExpired)
    ));
    drop((permit, nested));
    let expired = coordinator.new_owner(Instant::now());
    assert!(matches!(
        coordinator.acquire(&expired, &key, AccessMode::Read).await,
        Err(CoordinatorError::DeadlineExpired)
    ));
}

#[tokio::test]
async fn journal_hold_and_failed_hold_read_block_both_acquisition_and_existing_permits() {
    let f = Fixture::new();
    let coordinator = f.open().unwrap();
    let key = coordinator.project_key(&f.project).unwrap();
    let owner = coordinator.new_owner(Instant::now() + Duration::from_secs(5));
    let permit = coordinator
        .acquire(&owner, &key, AccessMode::Read)
        .await
        .unwrap();
    for (state, expected) in [
        (1, CoordinatorError::ProjectHeld),
        (2, CoordinatorError::HoldCheckUnavailable),
    ] {
        f.holds.0.store(state, Ordering::SeqCst);
        assert_eq!(permit.check_dispatch().await, Err(expected.clone()));
        assert!(coordinator
            .acquire(&owner, &key, AccessMode::Read)
            .await
            .is_err());
    }
}

#[tokio::test]
async fn path_replacement_and_inode_aliases_cannot_bypass_an_outstanding_permit() {
    let f = Fixture::new();
    let coordinator = f.open().unwrap();
    let key = coordinator.project_key(&f.project).unwrap();
    let owner = coordinator.new_owner(Instant::now() + Duration::from_secs(5));
    let permit = coordinator
        .acquire(&owner, &key, AccessMode::Mutate)
        .await
        .unwrap();
    let renamed = f.project.with_file_name("Renamed.xcodeproj");
    fs::rename(&f.project, &renamed).unwrap();
    let alias = coordinator.project_key(&renamed).unwrap();
    assert!(matches!(
        coordinator.acquire(&owner, &alias, AccessMode::Read).await,
        Err(CoordinatorError::ProjectAliasCollision)
    ));
    fs::create_dir(&f.project).unwrap();
    assert_eq!(
        permit.check_dispatch().await,
        Err(CoordinatorError::ProjectIdentityChanged)
    );
    let replacement = coordinator.project_key(&f.project).unwrap();
    assert!(matches!(
        coordinator
            .acquire(&owner, &replacement, AccessMode::Read)
            .await,
        Err(CoordinatorError::ProjectIdentityChanged)
    ));
}

#[test]
fn authority_tampering_after_open_is_detected_by_live_readback() {
    let f = Fixture::new();
    let authority = f.open_authority().unwrap();
    fs::remove_file(f.authority.join("authority.json")).unwrap();
    assert!(authority.check().is_err());
}

#[test]
fn missing_lock_or_abandoned_bootstrap_requires_explicit_operator_recovery() {
    for remove_record in [false, true] {
        let f = Fixture::new();
        drop(f.open_authority().unwrap());
        fs::remove_file(f.authority.join("coordinator.lock")).unwrap();
        if remove_record {
            fs::remove_file(f.authority.join("authority.json")).unwrap();
        }
        assert!(matches!(
            f.open_authority(),
            Err(CoordinatorError::AuthorityMissing)
        ));
        assert!(!f.authority.join("coordinator.lock").exists());
    }
}

#[test]
fn lock_is_exclusive_across_processes() {
    let f = Fixture::new();
    let _authority = f.open_authority().unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "fixture_child_authority_probe", "--nocapture"])
        .env("XCODE_COORDINATOR_TEST_ROOT", &f.authority)
        .env("XCODE_COORDINATOR_TEST_DATABASE", &f.database)
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn fixture_child_authority_probe() {
    let Some(root) = std::env::var_os("XCODE_COORDINATOR_TEST_ROOT") else {
        return;
    };
    let database = std::env::var_os("XCODE_COORDINATOR_TEST_DATABASE").unwrap();
    assert!(matches!(
        FixtureJournalAuthority::open(&PathBuf::from(root), &PathBuf::from(database)),
        Err(CoordinatorError::AuthorityBusy)
    ));
}

struct SlowHolds {
    slow: AtomicU8,
    released: tokio::sync::Notify,
}

#[async_trait::async_trait]
impl ProjectHoldCheck for SlowHolds {
    async fn is_held(&self, _: &ProjectKey) -> Result<bool, CoordinatorError> {
        if self.slow.load(Ordering::SeqCst) != 0 {
            self.released.notified().await;
        }
        Ok(false)
    }
}

#[tokio::test(start_paused = true)]
async fn hold_lookup_is_bounded_by_owner_deadline() {
    let f = Fixture::new();
    let holds = Arc::new(SlowHolds {
        slow: AtomicU8::new(1),
        released: tokio::sync::Notify::new(),
    });
    let coordinator = WorkspaceAccessCoordinator::new(
        CoordinatorLimits {
            queue_capacity: 2,
            queue_timeout: Duration::from_secs(1),
        },
        holds,
    )
    .unwrap();
    let key = coordinator.project_key(&f.project).unwrap();
    let owner = coordinator.new_owner(Instant::now() + Duration::from_millis(20));
    assert!(matches!(
        coordinator.acquire(&owner, &key, AccessMode::Read).await,
        Err(CoordinatorError::DeadlineExpired)
    ));
}

#[tokio::test]
async fn revocation_cancels_pending_hold_lookup_without_waiting_for_storage() {
    let f = Fixture::new();
    let holds = Arc::new(SlowHolds {
        slow: AtomicU8::new(0),
        released: tokio::sync::Notify::new(),
    });
    let coordinator = WorkspaceAccessCoordinator::new(
        CoordinatorLimits {
            queue_capacity: 2,
            queue_timeout: Duration::from_secs(1),
        },
        holds.clone(),
    )
    .unwrap();
    let key = coordinator.project_key(&f.project).unwrap();
    let owner = coordinator.new_owner(Instant::now() + Duration::from_secs(5));
    let permit = coordinator
        .acquire(&owner, &key, AccessMode::Read)
        .await
        .unwrap();
    holds.slow.store(1, Ordering::SeqCst);
    let mut dispatch = Box::pin(permit.check_dispatch());
    assert!(
        tokio::time::timeout(Duration::from_millis(10), &mut dispatch)
            .await
            .is_err()
    );
    owner.revoke();
    assert_eq!(
        tokio::time::timeout(Duration::from_millis(100), dispatch)
            .await
            .unwrap(),
        Err(CoordinatorError::OwnerRevoked)
    );
}

#[tokio::test]
async fn downgraded_nested_reader_cannot_regain_mutation_authority() {
    let f = Fixture::new();
    let coordinator = f.open().unwrap();
    let key = coordinator.project_key(&f.project).unwrap();
    let owner = coordinator.new_owner(Instant::now() + Duration::from_secs(5));
    let writer = coordinator
        .acquire(&owner, &key, AccessMode::Mutate)
        .await
        .unwrap();
    let reader = writer.borrow_nested(AccessMode::Read).await.unwrap();
    assert!(matches!(
        reader.borrow_nested(AccessMode::Mutate).await,
        Err(CoordinatorError::UpgradeDenied)
    ));
}

#[test]
fn failed_legacy_transition_proof_cannot_publish_authority() {
    let f = Fixture::new();
    let result =
        FixtureJournalAuthority::open_with_bootstrap_check(&f.authority, &f.database, |database| {
            assert_eq!(database, f.database);
            Err(CoordinatorError::LegacyTransitionUnproven)
        });
    assert!(matches!(
        result,
        Err(CoordinatorError::LegacyTransitionUnproven)
    ));
    assert!(!f.authority.join("authority.json").exists());
    assert!(matches!(
        f.open_authority(),
        Err(CoordinatorError::AuthorityMissing)
    ));
}

#[test]
fn legacy_transition_check_runs_only_for_bootstrap_and_under_the_kernel_lock() {
    let f = Fixture::new();
    let first =
        FixtureJournalAuthority::open_with_bootstrap_check(&f.authority, &f.database, |_| {
            assert!(matches!(
                f.open_authority(),
                Err(CoordinatorError::AuthorityBusy)
            ));
            Ok(())
        })
        .unwrap();
    drop(first);
    FixtureJournalAuthority::open_with_bootstrap_check(&f.authority, &f.database, |_| {
        Err(CoordinatorError::LegacyTransitionUnproven)
    })
    .unwrap()
    .check()
    .unwrap();
}

#[test]
fn symlinked_authority_parent_is_rejected_before_creating_a_lock() {
    let f = Fixture::new();
    let root = f.authority.parent().unwrap();
    let real = root.join("real-parent");
    let alias = root.join("alias-parent");
    fs::create_dir(&real).unwrap();
    symlink(&real, &alias).unwrap();
    assert!(matches!(
        FixtureJournalAuthority::open(&alias.join("authority"), &f.database),
        Err(CoordinatorError::InsecureAuthority)
    ));
    assert!(!real.join("authority").exists());
}

#[test]
fn ordinary_missing_authority_open_does_not_poison_later_explicit_bootstrap() {
    let f = Fixture::new();
    assert!(matches!(
        FixtureJournalAuthority::open_existing(&f.authority, &f.database),
        Err(CoordinatorError::LegacyTransitionUnproven)
    ));
    assert!(!f.authority.exists());
    let authority =
        FixtureJournalAuthority::open_with_bootstrap_check(&f.authority, &f.database, |_| Ok(()))
            .unwrap();
    authority.check().unwrap();
    drop(authority);
    FixtureJournalAuthority::open_existing(&f.authority, &f.database).unwrap();
}

#[test]
fn authority_readback_rejects_replaced_files_even_when_bytes_and_modes_match() {
    for name in ["coordinator.lock", "authority.json"] {
        let f = Fixture::new();
        let authority = f.open_authority().unwrap();
        let path = f.authority.join(name);
        let contents = fs::read(&path).unwrap();
        fs::rename(&path, f.authority.join(format!("previous-{name}"))).unwrap();
        fs::write(&path, contents).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(authority.check(), Err(CoordinatorError::InsecureAuthority));
    }
}

#[tokio::test]
async fn hardlinked_project_alias_cannot_bypass_an_outstanding_permit() {
    let f = Fixture::new();
    let project = f.project.join("project.pbxproj");
    let alias = f.project.join("alias.pbxproj");
    fs::write(&project, b"fixture project").unwrap();
    fs::hard_link(&project, &alias).unwrap();
    let coordinator = f.open().unwrap();
    let key = coordinator.project_key(&project).unwrap();
    let alias_key = coordinator.project_key(&alias).unwrap();
    let owner = coordinator.new_owner(Instant::now() + Duration::from_secs(5));
    let _permit = coordinator
        .acquire(&owner, &key, AccessMode::Mutate)
        .await
        .unwrap();
    assert!(matches!(
        coordinator
            .acquire(&owner, &alias_key, AccessMode::Read)
            .await,
        Err(CoordinatorError::ProjectAliasCollision)
    ));
}

#[tokio::test]
async fn independent_projects_do_not_share_a_global_mutation_lock() {
    let f = Fixture::new();
    let second_path = f.project.with_file_name("Independent.xcodeproj");
    fs::create_dir(&second_path).unwrap();
    let coordinator = f.open().unwrap();
    let first_key = coordinator.project_key(&f.project).unwrap();
    let second_key = coordinator.project_key(&second_path).unwrap();
    let owner = coordinator.new_owner(Instant::now() + Duration::from_secs(5));
    let other = coordinator.new_owner(Instant::now() + Duration::from_secs(5));
    let _first = coordinator
        .acquire(&owner, &first_key, AccessMode::Mutate)
        .await
        .unwrap();
    let second = coordinator
        .acquire(&other, &second_key, AccessMode::Mutate)
        .await
        .unwrap();
    second.check_dispatch().await.unwrap();
}
