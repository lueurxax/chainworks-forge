#![cfg(target_os = "macos")]

use acp::xcode_coordinator::{CoordinatorError, FixtureJournalAuthority};
use acp::xcode_filesystem_identity::{observe, IdentityKind};
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::PathBuf;

const INVENTORY: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const CHANGED: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

struct Fixture {
    _temp: tempfile::TempDir,
    authority: PathBuf,
    database: PathBuf,
}
impl Fixture {
    fn new(drift: bool) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let database = root.join("journal.sqlite");
        fs::write(
            &database,
            b"fixture: unknown effects and holds remain unchanged",
        )
        .unwrap();
        fs::set_permissions(&database, fs::Permissions::from_mode(0o600)).unwrap();
        let authority = root.join("authority");
        drop(FixtureJournalAuthority::open(&authority, &database).unwrap());
        let f = Self {
            _temp: temp,
            authority,
            database,
        };
        if drift {
            let mut record: serde_json::Value = serde_json::from_slice(&f.record()).unwrap();
            record["database"]["device"] = (fs::metadata(&f.database).unwrap().dev() + 1).into();
            fs::write(
                f.authority.join("authority.json"),
                serde_json::to_vec(&record).unwrap(),
            )
            .unwrap();
        }
        f
    }
    fn record(&self) -> Vec<u8> {
        fs::read(self.authority.join("authority.json")).unwrap()
    }
    fn plan(&self) -> acp::xcode_coordinator::AuthorityRecoveryPlan {
        FixtureJournalAuthority::inspect_recovery(&self.authority, &self.database, INVENTORY)
            .unwrap()
    }
}

#[test]
fn legacy_inspection_is_read_only_even_while_authority_is_locked() {
    let f = Fixture::new(false);
    let record = f.record();
    let db = fs::read(&f.database).unwrap();
    let authority = FixtureJournalAuthority::open_existing(&f.authority, &f.database).unwrap();
    let before: Vec<_> = fs::read_dir(&f.authority)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    let plan = f.plan();
    assert_eq!(plan.continuity, "continuity_unproven");
    assert_eq!(plan.action, "enroll_current_database_as_new_authority");
    assert_eq!(f.record(), record);
    assert_eq!(fs::read(&f.database).unwrap(), db);
    let after: Vec<_> = fs::read_dir(&f.authority)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(before, after);
    authority.check().unwrap();
    assert!(matches!(
        FixtureJournalAuthority::enroll_current_database(
            &f.authority,
            &f.database,
            &plan.plan_digest,
            "operator",
            |_| Ok(INVENTORY.into())
        ),
        Err(CoordinatorError::AuthorityBusy)
    ));
}

#[test]
fn legacy_drift_never_silently_migrates_but_explicit_new_enrollment_preserves_original() {
    let f = Fixture::new(true);
    let previous = f.record();
    let database = fs::read(&f.database).unwrap();
    assert!(matches!(
        FixtureJournalAuthority::open_existing(&f.authority, &f.database),
        Err(CoordinatorError::AuthorityMismatch)
    ));
    let plan = f.plan();
    assert_eq!(plan.continuity, "continuity_unproven");
    assert_eq!(f.record(), previous);
    let mut calls = 0;
    let receipt = FixtureJournalAuthority::enroll_current_database(
        &f.authority,
        &f.database,
        &plan.plan_digest,
        "fixture-operator",
        |path| {
            calls += 1;
            assert_eq!(path, f.database);
            // The retained coordinator lock excludes both another maintenance
            // process and ordinary runtime opening during every proof callback.
            assert!(matches!(
                FixtureJournalAuthority::open_existing(&f.authority, &f.database),
                Err(CoordinatorError::AuthorityBusy)
            ));
            Ok(INVENTORY.into())
        },
    )
    .unwrap();
    assert_eq!(calls, 2);
    assert_eq!(receipt.operator, "fixture-operator");
    assert_eq!(receipt.plan.continuity, "continuity_unproven");
    assert_eq!(
        fs::read(f.authority.join(&receipt.backup_file)).unwrap(),
        previous
    );
    assert_eq!(fs::read(&f.database).unwrap(), database);
    let persisted: acp::xcode_coordinator::AuthorityRecoveryReceipt =
        serde_json::from_slice(&fs::read(f.authority.join(&receipt.receipt_file)).unwrap())
            .unwrap();
    assert_eq!(persisted, receipt);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&f.record()).unwrap()["version"],
        2
    );
    FixtureJournalAuthority::open_existing(&f.authority, &f.database)
        .unwrap()
        .check()
        .unwrap();
    let current = f.record();
    assert!(matches!(
        FixtureJournalAuthority::enroll_current_database(
            &f.authority,
            &f.database,
            &plan.plan_digest,
            "fixture-operator",
            |_| Ok(INVENTORY.into())
        ),
        Err(CoordinatorError::AuthorityMismatch)
    ));
    assert_eq!(f.record(), current);
}

#[test]
fn stale_plan_inventory_and_same_byte_authority_object_replacement_deny_before_publication() {
    for replacement in [
        "inventory",
        "authority.json",
        "coordinator.lock",
        "database",
    ] {
        let f = Fixture::new(true);
        let plan = f.plan();
        let original = f.record();
        if replacement != "inventory" {
            let path = if replacement == "database" {
                f.database.clone()
            } else {
                f.authority.join(replacement)
            };
            let bytes = fs::read(&path).unwrap();
            fs::rename(&path, path.with_extension("old")).unwrap();
            fs::write(&path, bytes).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        let result = FixtureJournalAuthority::enroll_current_database(
            &f.authority,
            &f.database,
            &plan.plan_digest,
            "operator",
            |_| {
                Ok(if replacement == "inventory" {
                    CHANGED.into()
                } else {
                    INVENTORY.into()
                })
            },
        );
        assert!(
            matches!(result, Err(CoordinatorError::AuthorityMismatch)),
            "{replacement}: {result:?}"
        );
        assert_eq!(f.record(), original);
    }
}

#[test]
fn rejected_initial_stop_proof_cannot_prepare_or_publish_any_authority() {
    let f = Fixture::new(true);
    let original = f.record();
    let plan = f.plan();
    let result = FixtureJournalAuthority::enroll_current_database(
        &f.authority,
        &f.database,
        &plan.plan_digest,
        "operator",
        |_| Err(CoordinatorError::LegacyTransitionUnproven),
    );
    assert_eq!(result, Err(CoordinatorError::LegacyTransitionUnproven));
    assert_eq!(f.record(), original);
    assert_eq!(fs::read_dir(&f.authority).unwrap().count(), 2);
}

#[test]
fn interrupted_prepare_leaves_legacy_blocked_and_byte_identical_retry_can_commit() {
    let f = Fixture::new(true);
    let original = f.record();
    let plan = f.plan();
    let mut calls = 0;
    let result = FixtureJournalAuthority::enroll_current_database(
        &f.authority,
        &f.database,
        &plan.plan_digest,
        "operator",
        |_| {
            calls += 1;
            if calls == 2 {
                Err(CoordinatorError::LegacyTransitionUnproven)
            } else {
                Ok(INVENTORY.into())
            }
        },
    );
    assert_eq!(result, Err(CoordinatorError::LegacyTransitionUnproven));
    assert_eq!(f.record(), original);
    assert!(matches!(
        FixtureJournalAuthority::open_existing(&f.authority, &f.database),
        Err(CoordinatorError::AuthorityMismatch)
    ));
    let receipt = FixtureJournalAuthority::enroll_current_database(
        &f.authority,
        &f.database,
        &plan.plan_digest,
        "operator",
        |_| Ok(INVENTORY.into()),
    )
    .unwrap();
    assert_eq!(
        fs::read(f.authority.join(receipt.backup_file)).unwrap(),
        original
    );
    FixtureJournalAuthority::open_existing(&f.authority, &f.database)
        .unwrap()
        .check()
        .unwrap();
}

#[test]
fn replacement_during_final_stop_proof_cannot_activate_prepared_authority() {
    for name in ["coordinator.lock", "authority.json", "database"] {
        let f = Fixture::new(true);
        let original = f.record();
        let plan = f.plan();
        let mut calls = 0;
        let result = FixtureJournalAuthority::enroll_current_database(
            &f.authority,
            &f.database,
            &plan.plan_digest,
            "operator",
            |_| {
                calls += 1;
                if calls == 2 {
                    let path = if name == "database" {
                        f.database.clone()
                    } else {
                        f.authority.join(name)
                    };
                    let bytes = fs::read(&path).unwrap();
                    fs::rename(&path, path.with_extension("old")).unwrap();
                    fs::write(&path, bytes).unwrap();
                    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
                }
                Ok(INVENTORY.into())
            },
        );
        assert!(result.is_err(), "{name}");
        assert_eq!(f.record(), original);
    }
}

#[test]
fn enrolled_runtime_requires_intact_durable_backup_and_receipt() {
    for which in ["backup", "receipt"] {
        let f = Fixture::new(true);
        let plan = f.plan();
        let receipt = FixtureJournalAuthority::enroll_current_database(
            &f.authority,
            &f.database,
            &plan.plan_digest,
            "operator",
            |_| Ok(INVENTORY.into()),
        )
        .unwrap();
        let authority = FixtureJournalAuthority::open_existing(&f.authority, &f.database).unwrap();
        let name = if which == "backup" {
            receipt.backup_file
        } else {
            receipt.receipt_file
        };
        fs::write(f.authority.join(name), b"tampered").unwrap();
        assert!(authority.check().is_err());
        drop(authority);
        assert!(FixtureJournalAuthority::open_existing(&f.authority, &f.database).is_err());
    }
}

#[test]
fn persisted_durable_witness_accepts_reboot_device_change_but_not_same_boot_or_foreign_volume() {
    for scenario in [
        "reboot",
        "same_boot",
        "foreign_volume",
        "replaced_birthtime",
    ] {
        let f = Fixture::new(false);
        let mut old = observe(&f.database, IdentityKind::PrivateFile).unwrap();
        old.device += 1;
        if scenario != "same_boot" {
            old.boot_session_uuid = "00000000-0000-4000-8000-000000000011".into();
        }
        if scenario == "foreign_volume" {
            old.persistent.volume_uuid = "00000000-0000-4000-8000-000000000012".into();
        }
        if scenario == "replaced_birthtime" {
            old.birth_sec += 1;
        }
        let record = serde_json::to_vec(
            &serde_json::json!({"version":2,"uid":old.uid,"database":old,"enrollment":null}),
        )
        .unwrap();
        fs::write(f.authority.join("authority.json"), &record).unwrap();
        let authority = FixtureJournalAuthority::open_existing(&f.authority, &f.database);
        if scenario == "reboot" {
            authority.unwrap().check().unwrap();
        } else {
            assert!(
                matches!(authority, Err(CoordinatorError::AuthorityMismatch)),
                "{scenario}"
            );
        }
        assert_eq!(f.record(), record);
    }
}

#[test]
fn abandoned_incomplete_pending_evidence_does_not_poison_exact_plan_retry() {
    let f = Fixture::new(true);
    let plan = f.plan();
    // Durable leftovers from a crash while writing an unpublished temporary
    // file cannot be interpreted as either the final original or audit receipt.
    for name in [".evidence-crashed.pending", ".authority-crashed.pending"] {
        let path = f.authority.join(name);
        fs::write(&path, b"{incomplete").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    let receipt = FixtureJournalAuthority::enroll_current_database(
        &f.authority,
        &f.database,
        &plan.plan_digest,
        "operator",
        |_| Ok(INVENTORY.into()),
    )
    .unwrap();
    assert_eq!(receipt.plan_digest, plan.plan_digest);
    FixtureJournalAuthority::open_existing(&f.authority, &f.database)
        .unwrap()
        .check()
        .unwrap();
}

#[test]
fn killed_maintenance_process_releases_lock_and_leaves_only_atomic_authority_states() {
    for phase in ["prepared", "committed"] {
        let f = Fixture::new(true);
        let original = f.record();
        let plan = f.plan();
        let marker = f.database.with_extension("marker");
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "crash_enrollment_child", "--nocapture"])
            .env("CW_RECOVERY_CHILD_ROOT", &f.authority)
            .env("CW_RECOVERY_CHILD_DATABASE", &f.database)
            .env("CW_RECOVERY_CHILD_PLAN", &plan.plan_digest)
            .env("CW_RECOVERY_CHILD_MARKER", &marker)
            .env("CW_RECOVERY_CHILD_PHASE", phase)
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !marker.exists() && std::time::Instant::now() < deadline {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        if !marker.exists() {
            let _ = child.kill();
            let output = child.wait().unwrap();
            panic!("recovery child did not reach {phase}: {output}");
        }
        if phase == "prepared" {
            assert_eq!(f.record(), original);
            assert!(matches!(
                FixtureJournalAuthority::enroll_current_database(
                    &f.authority,
                    &f.database,
                    &plan.plan_digest,
                    "operator",
                    |_| Ok(INVENTORY.into()),
                ),
                Err(CoordinatorError::AuthorityBusy)
            ));
        }
        child.kill().unwrap();
        assert!(!child.wait().unwrap().success());
        if phase == "prepared" {
            assert!(matches!(
                FixtureJournalAuthority::open_existing(&f.authority, &f.database),
                Err(CoordinatorError::AuthorityMismatch)
            ));
            let receipt = FixtureJournalAuthority::enroll_current_database(
                &f.authority,
                &f.database,
                &plan.plan_digest,
                "operator",
                |_| Ok(INVENTORY.into()),
            )
            .unwrap();
            assert_eq!(
                fs::read(f.authority.join(receipt.backup_file)).unwrap(),
                original
            );
        }
        FixtureJournalAuthority::open_existing(&f.authority, &f.database)
            .unwrap()
            .check()
            .unwrap();
    }
}

#[test]
fn crash_enrollment_child() {
    let Some(root) = std::env::var_os("CW_RECOVERY_CHILD_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let database = PathBuf::from(std::env::var_os("CW_RECOVERY_CHILD_DATABASE").unwrap());
    let marker = PathBuf::from(std::env::var_os("CW_RECOVERY_CHILD_MARKER").unwrap());
    let plan = std::env::var("CW_RECOVERY_CHILD_PLAN").unwrap();
    let phase = std::env::var("CW_RECOVERY_CHILD_PHASE").unwrap();
    let block_until_killed = || {
        fs::write(&marker, b"checkpoint reached").unwrap();
        loop {
            std::thread::park_timeout(std::time::Duration::from_secs(1));
        }
    };
    let mut checks = 0;
    FixtureJournalAuthority::enroll_current_database(&root, &database, &plan, "operator", |_| {
        checks += 1;
        if checks == 2 && phase == "prepared" {
            block_until_killed();
        }
        Ok(INVENTORY.into())
    })
    .unwrap();
    block_until_killed();
}
