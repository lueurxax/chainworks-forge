//! Explicit operator project trust. Runtime reads never create grants.

use crate::xcode_coordinator::{
    check_node, create_private_directory, no_symlink_directories, open_at, CoordinatorError,
};
use crate::xcode_headless_host::TrustedProject;
use crate::xcode_headless_runtime::HeadlessProjectTrust;
use anyhow::{bail, ensure, Result};
use chrono::{DateTime, Utc};
use domain::{
    execution_root::ResolvedExecutionRoot,
    xcode_contract as contract,
    xcode_effect::{validate_text, ProjectKey},
};
use serde::{Deserialize, Serialize};
use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileExt, OpenOptionsExt};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
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
}

pub struct ProjectTrustStore {
    directory: PathBuf,
}

impl ProjectTrustStore {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    pub fn grant(&self, project: &TrustedProject, operator_id: &str) -> Result<String> {
        validate_project(project)?;
        validate_text("operator_id", operator_id, 256)?;
        let store = self.open(true)?;
        let name = record_name(project)?;
        // An explicit grant may replace a valid grant, never repair a malformed
        // or insecure record. A new grant identity invalidates earlier bindings.
        if let Some(record) = store.read(&name)? {
            validate_record(&record, project)?;
        }
        let record = TrustRecord {
            version: 1,
            grant_id: Uuid::new_v4(),
            operator_id: operator_id.to_owned(),
            granted_at: Utc::now(),
            root: project.root().clone(),
            project_key: project.key().clone(),
            trust_policy_id: contract::TRUST_POLICY_ID.to_owned(),
            trust_policy_digest: contract::trust_policy_digest()?,
            revoked_by: None,
            revoked_at: None,
        };
        validate_project(project)?;
        store.write(&name, &record)?;
        record_digest(&record)
    }

    pub fn revoke(&self, project: &TrustedProject, operator_id: &str) -> Result<()> {
        validate_project(project)?;
        validate_text("operator_id", operator_id, 256)?;
        let store = self.open_existing(true)?;
        let name = record_name(project)?;
        let mut record = store
            .read(&name)?
            .ok_or_else(|| anyhow::anyhow!("project_trust_required"))?;
        validate_record(&record, project)?;
        record.revoked_by = Some(operator_id.to_owned());
        record.revoked_at = Some(Utc::now());
        store.write(&name, &record)
    }

    fn open(&self, create: bool) -> Result<LockedStore> {
        let fresh = create && create_private_directory(&self.directory)?;
        self.open_inner(fresh, create)
    }

    fn open_existing(&self, write: bool) -> Result<LockedStore> {
        self.open_inner(false, write)
    }

    fn open_inner(&self, fresh: bool, write: bool) -> Result<LockedStore> {
        no_symlink_directories(&self.directory)?;
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
        validate_project(project)?;
        let store = self.open_existing(false)?;
        let record = store
            .read(&record_name(project)?)?
            .ok_or_else(|| anyhow::anyhow!("project_trust_required"))?;
        validate_record(&record, project)?;
        ensure!(record.revoked_at.is_none(), "project_trust_revoked");
        validate_project(project)?;
        record_digest(&record)
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
        let record = serde_json::from_value(contract::parse_unique_json(&bytes)?)?;
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

fn record_name(project: &TrustedProject) -> Result<String> {
    Ok(format!(
        "{}.json",
        contract::canonical_digest(
            "cw.xcode.project-trust-key.v1",
            &serde_json::json!({"root": project.root(), "project_key": project.key()})
        )?
    ))
}

fn record_digest(record: &TrustRecord) -> Result<String> {
    Ok(contract::canonical_digest(
        "cw.xcode.project-trust.v1",
        &serde_json::to_value(record)?,
    )?)
}

fn validate_record(record: &TrustRecord, project: &TrustedProject) -> Result<()> {
    ensure!(
        record.version == 1 && !record.grant_id.is_nil(),
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
    if record.root != *project.root()
        || record.project_key != *project.key()
        || record.trust_policy_id != contract::TRUST_POLICY_ID
        || record.trust_policy_digest != contract::trust_policy_digest()?
    {
        bail!("project_trust_mismatch");
    }
    Ok(())
}
