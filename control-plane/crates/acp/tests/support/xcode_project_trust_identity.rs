// Included as a private module so only the identity observation boundary is
// injected. Store locks, publication, parsing, and project validation stay real.
use super::*;
use crate::execution_root::resolve_execution_root;
use std::fs;

struct Fixture {
    _temporary: tempfile::TempDir,
    store: ProjectTrustStore,
    project: TrustedProject,
}
impl Fixture {
    fn new() -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().canonicalize().unwrap();
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
            _temporary: temporary,
            store: ProjectTrustStore::new(root.join("trust")),
            project,
        }
    }
}

#[test]
fn runtime_pins_accepted_current_boot_instead_of_reusing_the_grant_boot() {
    let f = Fixture::new();
    f.store.grant(&f.project, "operator").unwrap();
    let boot_a = |path: &Path, kind| {
        let mut identity = filesystem::observe(path, kind)?;
        identity.boot_session_uuid = "bff3189b-e088-45c2-88ee-c63941ba0ddb".into();
        Ok(identity)
    };
    let boot_b = |path: &Path, kind| {
        let mut identity = filesystem::observe(path, kind)?;
        identity.boot_session_uuid = "e561ac49-a1e6-45e9-bf99-dfae044a627a".into();
        Ok(identity)
    };
    assert!(f.store.check_observing(&f.project, &boot_a).is_ok());
    assert!(
        f.store.check_observing(&f.project, &boot_b).is_err(),
        "a running store cannot survive a boot transition"
    );
}

#[test]
fn unsupported_identity_never_creates_a_store_or_grant() {
    let f = Fixture::new();
    let unsupported = |_: &Path, _| Err(IdentityError::Unsupported);
    assert_eq!(
        f.store
            .grant_observing(&f.project, "operator", &unsupported)
            .unwrap_err()
            .to_string(),
        "persistent_identity_unsupported"
    );
    assert!(!f.store.directory.exists());
}

#[test]
fn current_observation_pin_rejects_same_boot_device_drift_after_cross_boot_acceptance() {
    let f = Fixture::new();
    let current = observe_project(&f.project, &filesystem::observe).unwrap();
    let mut historical = current.clone();
    for observation in [&mut historical.root, &mut historical.project] {
        observation.device += 1;
        observation.boot_session_uuid = "bff3189b-e088-45c2-88ee-c63941ba0ddb".into();
    }
    filesystem::accepts_observation(&historical.root, &current.root).unwrap();
    filesystem::accepts_observation(&historical.project, &current.project).unwrap();
    let name = durable_record_name(f.project.root(), f.project.key(), &current).unwrap();
    f.store.pin_current_observation(&name, &current).unwrap();
    for component in ["root", "project"] {
        let mut changed = current.clone();
        let observation = if component == "root" {
            &mut changed.root
        } else {
            &mut changed.project
        };
        observation.device += 2;
        // The historical predicate alone would wrongly allow this later drift.
        filesystem::accepts_observation(
            if component == "root" {
                &historical.root
            } else {
                &historical.project
            },
            observation,
        )
        .unwrap();
        assert_eq!(
            f.store
                .pin_current_observation(&name, &changed)
                .unwrap_err()
                .to_string(),
            "project_trust_identity_changed"
        );
        f.store.pin_current_observation(&name, &current).unwrap();
    }
}

#[test]
fn changed_identity_between_observations_never_publishes_a_grant() {
    let f = Fixture::new();
    let calls = std::cell::Cell::new(0);
    let observer = move |path: &Path, kind| {
        let mut identity = filesystem::observe(path, kind)?;
        let index = calls.get();
        calls.set(index + 1);
        if index >= 2 {
            identity.boot_session_uuid = "bff3189b-e088-45c2-88ee-c63941ba0ddb".into();
        }
        Ok(identity)
    };
    assert_eq!(
        f.store
            .grant_observing(&f.project, "operator", &observer)
            .unwrap_err()
            .to_string(),
        "project_trust_identity_changed"
    );
    assert_eq!(
        fs::read_dir(&f.store.directory).unwrap().count(),
        1,
        "only the lock may exist"
    );
}

#[test]
fn observation_binding_rejects_foreign_uid_and_root_or_project_device_mismatch() {
    for changed in ["uid", "root", "project"] {
        let f = Fixture::new();
        let observer = move |path: &Path, kind| {
            let mut identity = filesystem::observe(path, kind)?;
            if changed == "uid" {
                identity.uid += 1;
            }
            if (changed == "root" && path.ends_with("checkout"))
                || (changed == "project" && path.ends_with("App.xcodeproj"))
            {
                identity.device += 1;
            }
            Ok(identity)
        };
        assert_eq!(
            f.store
                .grant_observing(&f.project, "operator", &observer)
                .unwrap_err()
                .to_string(),
            "project_trust_identity_mismatch"
        );
        assert!(!f.store.directory.exists());
    }
}
