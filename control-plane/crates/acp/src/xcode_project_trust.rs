//! Explicit operator project trust. Runtime reads never create grants.

use crate::xcode_coordinator::{
    check_node, create_private_directory, no_symlink_directories, open_at, CoordinatorError,
};
use crate::xcode_filesystem_identity::{
    self as filesystem, FilesystemObservation, IdentityError, IdentityKind,
};
use crate::xcode_headless_host::TrustedProject;
use crate::xcode_headless_runtime::HeadlessProjectTrust;
use anyhow::{bail, ensure, Result};
use chrono::{DateTime, Utc};
use domain::{
    execution_root::{ExecutionRootKind, ResolvedExecutionRoot},
    xcode_contract as contract,
    xcode_effect::{validate_text, ProjectKey},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TrustRecord {
    version: u32,
    grant_id: Uuid,
    operator_id: String,
    granted_at: DateTime<Utc>,
    root: ResolvedExecutionRoot,
    project_key: ProjectKey,
    trust_policy_id: String,
    trust_policy_digest: String,
    revoked_by: Option<String>,
    revoked_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    filesystem: Option<TrustFilesystemIdentity>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TrustFilesystemIdentity {
    root: FilesystemObservation,
    project: FilesystemObservation,
}

type Observer =
    dyn Fn(&Path, IdentityKind) -> std::result::Result<FilesystemObservation, IdentityError>;

pub struct ProjectTrustStore {
    directory: PathBuf,
    // Durable grant evidence may come from an earlier boot. Once accepted, the
    // current observations must stay exact for this running store's lifetime.
    accepted: Mutex<HashMap<String, TrustFilesystemIdentity>>,
}

impl ProjectTrustStore {
    pub fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            accepted: Mutex::new(HashMap::new()),
        }
    }

    pub fn grant(&self, project: &TrustedProject, operator_id: &str) -> Result<String> {
        self.grant_observing(project, operator_id, &filesystem::observe)
    }

    fn grant_observing(
        &self,
        project: &TrustedProject,
        operator_id: &str,
        observer: &Observer,
    ) -> Result<String> {
        validate_project(project)?;
        validate_text("operator_id", operator_id, 256)?;
        // Unsupported identity fails before creating any store or grant.
        let identity = observe_project(project, observer)?;
        let store = self.open(true)?;
        let name = durable_record_name(project.root(), project.key(), &identity)?;
        if let Some(record) = store.read(&name)? {
            validate_durable_record(&record, project, &identity)?;
        }
        // Explicit enrollment is a new decision, not migration. Check exact old
        // slots for damage, but leave their decisions and bytes unchanged.
        legacy_records(&store, project)?;
        let record = new_record(project, operator_id, identity.clone())?;
        ensure!(
            observe_project(project, observer)? == identity,
            "project_trust_identity_changed"
        );
        store.write(&name, &record)?;
        record_digest(&record)
    }

    pub fn revoke(&self, project: &TrustedProject, operator_id: &str) -> Result<()> {
        self.revoke_observing(project, operator_id, &filesystem::observe)
    }

    fn revoke_observing(
        &self,
        project: &TrustedProject,
        operator_id: &str,
        observer: &Observer,
    ) -> Result<()> {
        validate_project(project)?;
        validate_text("operator_id", operator_id, 256)?;
        let store = self.open_existing(true)?;
        let identity = observe_project(project, observer)?;
        let name = durable_record_name(project.root(), project.key(), &identity)?;
        let current = store.read(&name)?;
        if let Some(record) = &current {
            validate_durable_record(record, project, &identity)?;
        }
        let legacy = legacy_records(&store, project)?;
        ensure!(
            current.is_some() || !legacy.is_empty(),
            "project_trust_required"
        );
        // A legacy-only revoke creates a fresh denial, never an inherited grant.
        let mut record = match current {
            Some(record) => record,
            None => new_record(project, operator_id, identity.clone())?,
        };
        let revoked_at = Utc::now();
        ensure!(
            observe_project(project, observer)? == identity,
            "project_trust_identity_changed"
        );
        // Revoke discoverable exact legacy slots first, then publish the v3
        // tombstone. Old-device historical slots are not inferred or scanned.
        for (name, mut legacy_record) in legacy {
            legacy_record.revoked_by = Some(operator_id.to_owned());
            legacy_record.revoked_at = Some(revoked_at);
            store.write(&name, &legacy_record)?;
        }
        record.revoked_by = Some(operator_id.to_owned());
        record.revoked_at = Some(revoked_at);
        store.write(&name, &record)
    }

    fn check_observing(&self, project: &TrustedProject, observer: &Observer) -> Result<String> {
        validate_project(project)?;
        let store = self.open_existing(false)?;
        let identity = observe_project(project, observer)?;
        let name = durable_record_name(project.root(), project.key(), &identity)?;
        // All legacy grants require explicit v3 enrollment, even when device
        // still matches. Falling back would bypass v3 volume/replacement checks.
        let record = store
            .read(&name)?
            .ok_or_else(|| anyhow::anyhow!("project_trust_required"))?;
        validate_durable_record(&record, project, &identity)?;
        ensure!(record.revoked_at.is_none(), "project_trust_revoked");
        ensure!(
            observe_project(project, observer)? == identity,
            "project_trust_identity_changed"
        );
        self.pin_current_observation(&name, &identity)?;
        record_digest(&record)
    }

    fn pin_current_observation(&self, name: &str, current: &TrustFilesystemIdentity) -> Result<()> {
        let mut accepted = self
            .accepted
            .lock()
            .map_err(|_| anyhow::anyhow!("project_trust_identity_changed"))?;
        if let Some(previous) = accepted.get(name) {
            ensure!(previous == current, "project_trust_identity_changed");
        } else {
            accepted.insert(name.to_owned(), current.clone());
        }
        Ok(())
    }

    fn open(&self, create: bool) -> Result<LockedStore> {
        let fresh = create && create_private_directory(&self.directory)?;
        self.open_inner(fresh, create)
    }

    fn open_existing(&self, write: bool) -> Result<LockedStore> {
        self.open_inner(false, write)
    }

    fn open_inner(&self, fresh: bool, write: bool) -> Result<LockedStore> {
        // Only an absent store path means trust has not been admitted. Missing
        // files inside an existing store must still fail closed as store damage.
        no_symlink_directories(&self.directory).map_err(|error| match error {
            CoordinatorError::AuthorityMissing => anyhow::anyhow!("project_trust_required"),
            error => error.into(),
        })?;
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&self.directory)?;
        check_node(&self.directory, &directory, true)?;
        let lock = open_at(&directory, "trust.lock", fresh, write)?;
        check_node(&self.directory.join("trust.lock"), &lock, false)?;
        let mode = if write { libc::LOCK_EX } else { libc::LOCK_SH };
        ensure!(
            unsafe { libc::flock(lock.as_raw_fd(), mode | libc::LOCK_NB) } == 0,
            "project_trust_busy"
        );
        if fresh {
            lock.sync_all()?;
            directory.sync_all()?;
        }
        let store = LockedStore {
            path: self.directory.clone(),
            directory,
            lock,
        };
        store.check()?;
        Ok(store)
    }
}

#[async_trait::async_trait]
impl HeadlessProjectTrust for ProjectTrustStore {
    async fn check(&self, project: &TrustedProject) -> Result<String> {
        self.check_observing(project, &filesystem::observe)
    }
}

struct LockedStore {
    path: PathBuf,
    directory: File,
    lock: File,
}

impl LockedStore {
    fn check(&self) -> Result<()> {
        no_symlink_directories(&self.path)?;
        check_node(&self.path, &self.directory, true)?;
        check_node(&self.path.join("trust.lock"), &self.lock, false)?;
        Ok(())
    }

    fn read(&self, name: &str) -> Result<Option<TrustRecord>> {
        self.check()?;
        let file = match open_at(&self.directory, name, false, false) {
            Ok(file) => file,
            Err(CoordinatorError::AuthorityMissing) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        check_node(&self.path.join(name), &file, false)?;
        let length = file.metadata()?.len();
        ensure!((1..=16384).contains(&length), "project_trust_corrupt");
        let mut bytes = vec![0; length as usize];
        file.read_exact_at(&mut bytes, 0)?;
        let value = contract::parse_unique_json(&bytes)?;
        // An optional v3 field must not relax the exact historical schemas:
        // even an explicit null was an unknown field in v1/v2.
        ensure!(
            !matches!(
                value.get("version").and_then(serde_json::Value::as_u64),
                Some(1 | 2)
            ) || value.get("filesystem").is_none(),
            "project_trust_corrupt"
        );
        let record = serde_json::from_value(value)?;
        check_node(&self.path.join(name), &file, false)?;
        self.check()?;
        Ok(Some(record))
    }

    fn write(&self, name: &str, record: &TrustRecord) -> Result<()> {
        self.check()?;
        let bytes = serde_json::to_vec(record)?;
        ensure!(bytes.len() <= 16384, "project_trust_record_too_large");
        let temporary = format!(".{}.tmp", Uuid::new_v4());
        let mut file = open_at(&self.directory, &temporary, true, true)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        check_node(&self.path.join(&temporary), &file, false)?;
        self.check()?;
        let source = CString::new(temporary)?;
        let destination = CString::new(name)?;
        ensure!(
            unsafe {
                libc::renameat(
                    self.directory.as_raw_fd(),
                    source.as_ptr(),
                    self.directory.as_raw_fd(),
                    destination.as_ptr(),
                )
            } == 0,
            "project_trust_publish_failed"
        );
        self.directory.sync_all()?;
        let stored = self
            .read(name)?
            .ok_or_else(|| anyhow::anyhow!("project_trust_publish_failed"))?;
        ensure!(
            record_digest(&stored)? == record_digest(record)?,
            "project_trust_publish_failed"
        );
        Ok(())
    }
}

fn validate_project(project: &TrustedProject) -> Result<()> {
    ensure!(
        project.key().uid == unsafe { libc::geteuid() },
        "project_trust_foreign_uid"
    );
    project.revalidate()
}

fn shared_worktree_identity(root: &ResolvedExecutionRoot) -> bool {
    root.kind == ExecutionRootKind::Worktree
        && matches!(
            root.strategy.as_deref(),
            Some("dedicated" | "shared_implementation_worktree")
        )
}

fn new_record(
    project: &TrustedProject,
    operator_id: &str,
    identity: TrustFilesystemIdentity,
) -> Result<TrustRecord> {
    Ok(TrustRecord {
        version: 3,
        grant_id: Uuid::new_v4(),
        operator_id: operator_id.to_owned(),
        granted_at: Utc::now(),
        root: project.root().clone(),
        project_key: project.key().clone(),
        trust_policy_id: contract::TRUST_POLICY_ID.to_owned(),
        trust_policy_digest: contract::trust_policy_digest()?,
        revoked_by: None,
        revoked_at: None,
        filesystem: Some(identity),
    })
}

fn observe_project(
    project: &TrustedProject,
    observer: &Observer,
) -> Result<TrustFilesystemIdentity> {
    validate_project(project)?;
    let identity = TrustFilesystemIdentity {
        root: observer(
            Path::new(&project.root().effective),
            IdentityKind::Directory,
        )?,
        project: observer(
            Path::new(&project.key().canonical_path),
            IdentityKind::Directory,
        )?,
    };
    validate_observation_binding(project.root(), project.key(), &identity)?;
    validate_project(project)?;
    Ok(identity)
}

fn validate_observation_binding(
    root: &ResolvedExecutionRoot,
    project: &ProjectKey,
    identity: &TrustFilesystemIdentity,
) -> Result<()> {
    identity.root.validate()?;
    identity.project.validate()?;
    ensure!(
        identity.root.canonical_path == root.effective
            && identity.root.uid == project.uid
            && identity.root.device.to_string() == root.device
            && identity.root.inode.to_string() == root.inode
            && identity.project.canonical_path == project.canonical_path
            && identity.project.uid == project.uid
            && identity.project.device.to_string() == project.device
            && identity.project.inode.to_string() == project.inode
            && identity.root.boot_session_uuid == identity.project.boot_session_uuid,
        "project_trust_identity_mismatch"
    );
    Ok(())
}

fn durable_record_name(
    root: &ResolvedExecutionRoot,
    project: &ProjectKey,
    identity: &TrustFilesystemIdentity,
) -> Result<String> {
    validate_observation_binding(root, project, identity)?;
    let strategy = if shared_worktree_identity(root) {
        None
    } else {
        root.strategy.as_deref()
    };
    // Device and boot UUID are admission observations, not persistent key parts.
    // All semantic root bindings and both durable physical identities remain.
    let durable = |observation: &FilesystemObservation| {
        serde_json::json!({
            "canonical_path": observation.canonical_path,
            "uid": observation.uid,
            "inode": observation.inode,
            "birth_sec": observation.birth_sec,
            "birth_nsec": observation.birth_nsec,
            "persistent": observation.persistent,
        })
    };
    Ok(format!(
        "{}.json",
        contract::canonical_digest(
            "cw.xcode.project-trust-key.v3",
            &serde_json::json!({
                "root_version": root.version,
                "root_kind": root.kind,
                "repository": root.repository,
                "shared_worktree_identity": shared_worktree_identity(root),
                "strategy": strategy,
                "project_version": project.version,
                "root": durable(&identity.root),
                "project": durable(&identity.project),
            })
        )?
    ))
}

fn validate_durable_record(
    record: &TrustRecord,
    project: &TrustedProject,
    current: &TrustFilesystemIdentity,
) -> Result<()> {
    validate_record_metadata(record, 3)?;
    let pinned = record
        .filesystem
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("project_trust_corrupt"))?;
    ensure!(
        durable_record_name(&record.root, &record.project_key, pinned)?
            == durable_record_name(project.root(), project.key(), current)?,
        "project_trust_mismatch"
    );
    filesystem::accepts_observation(&pinned.root, &current.root)?;
    filesystem::accepts_observation(&pinned.project, &current.project)?;
    Ok(())
}

fn legacy_records(
    store: &LockedStore,
    project: &TrustedProject,
) -> Result<Vec<(String, TrustRecord)>> {
    if shared_worktree_identity(project.root()) {
        let mut records = legacy_worktree_records(store, project)?;
        let mut root = project.root().clone();
        root.strategy = None;
        let name = format!(
            "{}.json",
            contract::canonical_digest(
                "cw.xcode.project-trust-key.v2",
                &serde_json::json!({"root": root, "project_key": project.key()})
            )?
        );
        if let Some(record) = store.read(&name)? {
            validate_record(&record, project.root(), project.key(), 2)?;
            records.push((name, record));
        }
        Ok(records)
    } else {
        let name = legacy_record_name(project.root(), project.key())?;
        match store.read(&name)? {
            Some(record) => {
                validate_record(&record, project.root(), project.key(), 1)?;
                Ok(vec![(name, record)])
            }
            None => Ok(Vec::new()),
        }
    }
}

fn legacy_record_name(root: &ResolvedExecutionRoot, project_key: &ProjectKey) -> Result<String> {
    Ok(format!(
        "{}.json",
        contract::canonical_digest(
            "cw.xcode.project-trust-key.v1",
            &serde_json::json!({"root": root, "project_key": project_key})
        )?
    ))
}

fn legacy_worktree_records(
    store: &LockedStore,
    project: &TrustedProject,
) -> Result<Vec<(String, TrustRecord)>> {
    let mut records = Vec::new();
    // The finite aliases are derived from the exact physical identity, never
    // discovered by scanning grants or inherited from a repository/parent root.
    for strategy in ["dedicated", "shared_implementation_worktree"] {
        let mut root = project.root().clone();
        root.strategy = Some(strategy.to_owned());
        let name = legacy_record_name(&root, project.key())?;
        if let Some(record) = store.read(&name)? {
            validate_record(&record, &root, project.key(), 1)?;
            records.push((name, record));
        }
    }
    Ok(records)
}

fn record_digest(record: &TrustRecord) -> Result<String> {
    Ok(contract::canonical_digest(
        match record.version {
            3 => "cw.xcode.project-trust.v3",
            2 => "cw.xcode.project-trust.v2",
            _ => "cw.xcode.project-trust.v1",
        },
        &serde_json::to_value(record)?,
    )?)
}

fn validate_record(
    record: &TrustRecord,
    root: &ResolvedExecutionRoot,
    project_key: &ProjectKey,
    version: u32,
) -> Result<()> {
    validate_record_metadata(record, version)?;
    ensure!(record.filesystem.is_none(), "project_trust_corrupt");
    let root_matches = if version == 2 {
        let mut admitted = record.root.clone();
        let mut requested = root.clone();
        let supported = shared_worktree_identity(&admitted) && shared_worktree_identity(&requested);
        admitted.strategy = None;
        requested.strategy = None;
        supported && admitted == requested
    } else {
        record.root == *root
    };
    if !root_matches
        || record.project_key != *project_key
        || record.trust_policy_id != contract::TRUST_POLICY_ID
        || record.trust_policy_digest != contract::trust_policy_digest()?
    {
        bail!("project_trust_mismatch");
    }
    Ok(())
}

fn validate_record_metadata(record: &TrustRecord, version: u32) -> Result<()> {
    ensure!(
        record.version == version && !record.grant_id.is_nil(),
        "project_trust_corrupt"
    );
    validate_text("operator_id", &record.operator_id, 256)?;
    if let Some(operator) = &record.revoked_by {
        validate_text("operator_id", operator, 256)?;
    }
    ensure!(
        record.revoked_by.is_some() == record.revoked_at.is_some(),
        "project_trust_corrupt"
    );
    ensure!(
        record.trust_policy_id == contract::TRUST_POLICY_ID
            && record.trust_policy_digest == contract::trust_policy_digest()?,
        "project_trust_mismatch"
    );
    Ok(())
}

#[cfg(all(test, target_os = "macos"))]
#[path = "../tests/support/xcode_project_trust_identity.rs"]
mod identity_tests;
