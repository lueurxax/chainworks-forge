//! Cooperating workspace access and persistent per-user journal authority.
//!
//! Daemon wiring retains `JournalAuthority` and injects ONE coordinator Arc into
//! every live route. Fixture authority deliberately has a different Rust type.
//! An owner lives only for an invocation, not an idle provider session. Callers
//! retain permits until work finishes or a durable uncertainty hold replaces it;
//! dropping a permit does not prove an external operation stopped.

use crate::xcode_filesystem_identity::{
    self as filesystem_identity, FilesystemObservation, IdentityError, IdentityKind,
};
use domain::xcode_effect::ProjectKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::ffi::{CStr, CString, OsStr};
use std::fs::{self, DirBuilder, File, Metadata, OpenOptions};
use std::io::Write;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, FileExt, MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;
use tokio::sync::Notify;
use tokio::time::Instant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessMode {
    Read,
    Mutate,
}

#[derive(Clone, Copy, Debug)]
pub struct CoordinatorLimits {
    pub queue_capacity: usize,
    pub queue_timeout: Duration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoordinatorError {
    AuthorityBusy,
    InsecureAuthority,
    AuthorityMissing,
    AuthorityCorrupt,
    AuthorityMismatch,
    InvalidProject,
    ProjectIdentityChanged,
    ProjectAliasCollision,
    QueueFull,
    QueueTimeout,
    DeadlineExpired,
    OwnerRevoked,
    ForeignOwner,
    UpgradeDenied,
    ProjectHeld,
    HoldCheckUnavailable,
    FixtureOnly,
    InvalidLimits,
    StorageUnavailable,
    LegacyTransitionUnproven,
    PersistentIdentityUnavailable,
}

impl std::fmt::Display for CoordinatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let code = match self {
            Self::AuthorityBusy => "authority_busy",
            Self::InsecureAuthority => "insecure_authority",
            Self::AuthorityMissing => "authority_missing",
            Self::AuthorityCorrupt => "authority_corrupt",
            Self::AuthorityMismatch => "authority_mismatch",
            Self::InvalidProject => "invalid_project",
            Self::ProjectIdentityChanged => "project_identity_changed",
            Self::ProjectAliasCollision => "project_alias_collision",
            Self::QueueFull => "queue_full",
            Self::QueueTimeout => "queue_timeout",
            Self::DeadlineExpired => "deadline_expired",
            Self::OwnerRevoked => "owner_revoked",
            Self::ForeignOwner => "foreign_owner",
            Self::UpgradeDenied => "upgrade_denied",
            Self::ProjectHeld => "project_held",
            Self::HoldCheckUnavailable => "hold_check_unavailable",
            Self::FixtureOnly => "fixture_only",
            Self::InvalidLimits => "invalid_limits",
            Self::StorageUnavailable => "storage_unavailable",
            Self::LegacyTransitionUnproven => "legacy_transition_unproven",
            Self::PersistentIdentityUnavailable => "persistent_identity_unavailable",
        };
        f.write_str(code)
    }
}

impl std::error::Error for CoordinatorError {}

/// Native journal truth, including crash recovery, not an observation cache.
/// There is no permissive default. Errors and hung reads deny dispatch.
/// Match UID AND (canonical path OR device/inode), not full serialized-key
/// equality: durable holds must survive aliases and path replacement. Admission
/// stays disabled until startup recovery has restored those holds.
#[async_trait::async_trait]
pub trait ProjectHoldCheck: Send + Sync {
    async fn is_held(&self, project: &ProjectKey) -> Result<bool, CoordinatorError>;
}

pub struct WorkspaceAccessCoordinator {
    limits: CoordinatorLimits,
    holds: Arc<dyn ProjectHoldCheck>,
    state: Mutex<QueueState>,
    changed: Notify,
}

/// Production authority, obtainable only at the fixed host-user location.
pub struct JournalAuthority {
    inner: Authority,
}

/// Separate type: cannot satisfy a live runtime's `Arc<JournalAuthority>` input.
pub struct FixtureJournalAuthority {
    inner: Authority,
}

/// Not Clone or serializable. Permits keep only a weak reference to this owner.
pub struct OwnerToken {
    state: Arc<OwnerState>,
}

pub struct WorkspacePermit {
    lease: Arc<Lease>,
    mode: AccessMode,
}

impl JournalAuthority {
    /// Reopen existing authority. First setup requires explicit legacy proof.
    pub fn open(database: &Path) -> Result<Arc<Self>, CoordinatorError> {
        Ok(Arc::new(Self {
            inner: Authority::open(&production_root(false)?, database, false, true, |_| {
                Err(CoordinatorError::LegacyTransitionUnproven)
            })?,
        }))
    }

    pub fn check(&self) -> Result<(), CoordinatorError> {
        self.inner.check()
    }

    /// The synchronous check runs under the kernel lock before first publication.
    /// It must establish legacy-runtime quiescence and absence of unresolved
    /// legacy effects. A failed/interrupted bootstrap is not silently retried.
    pub fn open_with_bootstrap_check(
        database: &Path,
        check: impl FnOnce(&Path) -> Result<(), CoordinatorError>,
    ) -> Result<Arc<Self>, CoordinatorError> {
        Ok(Arc::new(Self {
            inner: Authority::open(&production_root(true)?, database, true, true, check)?,
        }))
    }
}

impl FixtureJournalAuthority {
    pub fn open_existing(authority: &Path, database: &Path) -> Result<Arc<Self>, CoordinatorError> {
        Ok(Arc::new(Self {
            inner: Authority::open(authority, database, false, false, |_| {
                Err(CoordinatorError::LegacyTransitionUnproven)
            })?,
        }))
    }

    pub fn open(authority: &Path, database: &Path) -> Result<Arc<Self>, CoordinatorError> {
        Self::open_with_bootstrap_check(authority, database, |_| Ok(()))
    }

    pub fn check(&self) -> Result<(), CoordinatorError> {
        self.inner.check()
    }

    pub fn open_with_bootstrap_check(
        authority: &Path,
        database: &Path,
        check: impl FnOnce(&Path) -> Result<(), CoordinatorError>,
    ) -> Result<Arc<Self>, CoordinatorError> {
        Ok(Arc::new(Self {
            inner: Authority::open(authority, database, true, false, check)?,
        }))
    }
}

impl WorkspaceAccessCoordinator {
    pub fn new(
        limits: CoordinatorLimits,
        holds: Arc<dyn ProjectHoldCheck>,
    ) -> Result<Arc<Self>, CoordinatorError> {
        if limits.queue_capacity == 0
            || limits.queue_timeout.is_zero()
            || Instant::now().checked_add(limits.queue_timeout).is_none()
        {
            return Err(CoordinatorError::InvalidLimits);
        }
        Ok(Arc::new(Self {
            limits,
            holds,
            state: Mutex::new(QueueState::default()),
            changed: Notify::new(),
        }))
    }

    pub fn project_key(&self, path: &Path) -> Result<ProjectKey, CoordinatorError> {
        let canonical = fs::canonicalize(path).map_err(|_| CoordinatorError::InvalidProject)?;
        let metadata = fs::metadata(&canonical).map_err(|_| CoordinatorError::InvalidProject)?;
        if !metadata.is_dir() && !metadata.is_file() {
            return Err(CoordinatorError::InvalidProject);
        }
        let key = ProjectKey {
            version: 1,
            uid: uid(),
            canonical_path: canonical
                .to_str()
                .ok_or(CoordinatorError::InvalidProject)?
                .into(),
            device: metadata.dev().to_string(),
            inode: metadata.ino().to_string(),
        };
        key.validate()
            .map_err(|_| CoordinatorError::InvalidProject)?;
        Ok(key)
    }

    pub fn new_owner(self: &Arc<Self>, deadline: Instant) -> OwnerToken {
        OwnerToken {
            state: Arc::new(OwnerState {
                coordinator: Arc::downgrade(self),
                deadline,
                revoked: AtomicBool::new(false),
                cancelled: Notify::new(),
            }),
        }
    }

    pub async fn acquire(
        self: &Arc<Self>,
        owner: &OwnerToken,
        project: &ProjectKey,
        mode: AccessMode,
    ) -> Result<WorkspacePermit, CoordinatorError> {
        if !Weak::ptr_eq(&owner.state.coordinator, &Arc::downgrade(self)) {
            return Err(CoordinatorError::ForeignOwner);
        }
        owner.state.check()?;
        validate_project(project)?;
        let queue_deadline = Instant::now()
            .checked_add(self.limits.queue_timeout)
            .ok_or(CoordinatorError::InvalidLimits)?
            .min(owner.state.deadline);
        match tokio::time::timeout_at(queue_deadline, self.acquire_inner(owner, project, mode))
            .await
        {
            Ok(result) => result,
            Err(_) if Instant::now() >= owner.state.deadline => {
                Err(CoordinatorError::DeadlineExpired)
            }
            Err(_) => Err(CoordinatorError::QueueTimeout),
        }
    }

    async fn acquire_inner(
        self: &Arc<Self>,
        owner: &OwnerToken,
        project: &ProjectKey,
        mode: AccessMode,
    ) -> Result<WorkspacePermit, CoordinatorError> {
        let id = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| CoordinatorError::StorageUnavailable)?;
            for other in state.active.values().chain(state.waiting.iter()) {
                reject_alias_collision(project, &other.project)?;
            }
            if state.waiting.len() >= self.limits.queue_capacity {
                return Err(CoordinatorError::QueueFull);
            }
            state.next_id = state
                .next_id
                .checked_add(1)
                .ok_or(CoordinatorError::StorageUnavailable)?;
            let id = state.next_id;
            state.waiting.push_back(Request {
                id,
                project: project.clone(),
                mode,
            });
            id
        };
        let mut queued = Queued {
            coordinator: self.clone(),
            id,
            armed: true,
        };
        loop {
            // Register before checking the state: release/revoke cannot be lost
            // between the state check and parking this waiter.
            let notified = self.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            owner.state.check()?;
            validate_project(project)?;
            let admitted = {
                let mut state = self
                    .state
                    .lock()
                    .map_err(|_| CoordinatorError::StorageUnavailable)?;
                let index = state
                    .waiting
                    .iter()
                    .position(|r| r.id == id)
                    .ok_or(CoordinatorError::StorageUnavailable)?;
                let request = &state.waiting[index];
                let blocked = state.active.values().any(|r| r.conflicts(request))
                    || state
                        .waiting
                        .iter()
                        .take(index)
                        .any(|r| r.conflicts(request));
                if blocked {
                    false
                } else {
                    let request = state
                        .waiting
                        .remove(index)
                        .ok_or(CoordinatorError::StorageUnavailable)?;
                    state.active.insert(id, request);
                    queued.armed = false;
                    true
                }
            };
            if admitted {
                let permit = WorkspacePermit {
                    lease: Arc::new(Lease {
                        coordinator: self.clone(),
                        owner: Arc::downgrade(&owner.state),
                        id,
                        project: project.clone(),
                    }),
                    mode,
                };
                permit.check_dispatch().await?;
                return Ok(permit);
            }
            notified.await;
        }
    }
}

impl OwnerToken {
    pub fn revoke(&self) {
        self.state.revoked.store(true, Ordering::Release);
        self.state.cancelled.notify_waiters();
        if let Some(coordinator) = self.state.coordinator.upgrade() {
            coordinator.changed.notify_waiters();
        }
    }
}

impl Drop for OwnerToken {
    fn drop(&mut self) {
        self.revoke();
    }
}

impl WorkspacePermit {
    /// Borrow the same lease, never its owner's liveness or a new deadline.
    pub async fn borrow_nested(&self, mode: AccessMode) -> Result<Self, CoordinatorError> {
        if self.mode == AccessMode::Read && mode == AccessMode::Mutate {
            return Err(CoordinatorError::UpgradeDenied);
        }
        self.check_dispatch().await?;
        Ok(Self {
            lease: self.lease.clone(),
            mode,
        })
    }

    /// Revalidate immediately before EACH backend dispatch, including nested work.
    /// The caller also checks its production JournalAuthority and binding policy.
    pub async fn check_dispatch(&self) -> Result<(), CoordinatorError> {
        let owner = self
            .lease
            .owner
            .upgrade()
            .ok_or(CoordinatorError::OwnerRevoked)?;
        let cancelled = owner.cancelled.notified();
        tokio::pin!(cancelled);
        cancelled.as_mut().enable();
        owner.check()?;
        validate_project(&self.lease.project)?;
        let held = tokio::select! {
            biased;
            _ = &mut cancelled => return Err(CoordinatorError::OwnerRevoked),
            _ = tokio::time::sleep_until(owner.deadline) => return Err(CoordinatorError::DeadlineExpired),
            result = self.lease.coordinator.holds.is_held(&self.lease.project) => {
                result.map_err(|_| CoordinatorError::HoldCheckUnavailable)?
            }
        };
        owner.check()?;
        validate_project(&self.lease.project)?;
        if held {
            Err(CoordinatorError::ProjectHeld)
        } else {
            Ok(())
        }
    }
}

struct OwnerState {
    coordinator: Weak<WorkspaceAccessCoordinator>,
    deadline: Instant,
    revoked: AtomicBool,
    cancelled: Notify,
}

impl OwnerState {
    fn check(&self) -> Result<(), CoordinatorError> {
        if self.revoked.load(Ordering::Acquire) {
            return Err(CoordinatorError::OwnerRevoked);
        }
        if Instant::now() >= self.deadline {
            return Err(CoordinatorError::DeadlineExpired);
        }
        Ok(())
    }
}

#[derive(Default)]
struct QueueState {
    next_id: u64,
    waiting: VecDeque<Request>,
    active: HashMap<u64, Request>,
}

struct Request {
    id: u64,
    project: ProjectKey,
    mode: AccessMode,
}

impl Request {
    fn conflicts(&self, other: &Self) -> bool {
        self.project.uid == other.project.uid
            && self.project.canonical_path == other.project.canonical_path
            && (self.mode == AccessMode::Mutate || other.mode == AccessMode::Mutate)
    }
}

struct Queued {
    coordinator: Arc<WorkspaceAccessCoordinator>,
    id: u64,
    armed: bool,
}

impl Drop for Queued {
    fn drop(&mut self) {
        if self.armed {
            if let Ok(mut state) = self.coordinator.state.lock() {
                state.waiting.retain(|r| r.id != self.id);
            }
            self.coordinator.changed.notify_waiters();
        }
    }
}

struct Lease {
    coordinator: Arc<WorkspaceAccessCoordinator>,
    owner: Weak<OwnerState>,
    id: u64,
    project: ProjectKey,
}

impl Drop for Lease {
    fn drop(&mut self) {
        if let Ok(mut state) = self.coordinator.state.lock() {
            state.active.remove(&self.id);
        }
        self.coordinator.changed.notify_waiters();
    }
}

fn validate_project(key: &ProjectKey) -> Result<(), CoordinatorError> {
    key.validate()
        .map_err(|_| CoordinatorError::InvalidProject)?;
    if key.uid != uid() {
        return Err(CoordinatorError::InvalidProject);
    }
    let path = Path::new(&key.canonical_path);
    let canonical = fs::canonicalize(path).map_err(|_| CoordinatorError::ProjectIdentityChanged)?;
    let metadata = fs::metadata(path).map_err(|_| CoordinatorError::ProjectIdentityChanged)?;
    if canonical != path
        || metadata.dev().to_string() != key.device
        || metadata.ino().to_string() != key.inode
        || (!metadata.is_file() && !metadata.is_dir())
    {
        return Err(CoordinatorError::ProjectIdentityChanged);
    }
    Ok(())
}

fn reject_alias_collision(key: &ProjectKey, other: &ProjectKey) -> Result<(), CoordinatorError> {
    if key.uid == other.uid {
        let same_inode = key.device == other.device && key.inode == other.inode;
        if key.canonical_path == other.canonical_path && !same_inode {
            return Err(CoordinatorError::ProjectIdentityChanged);
        }
        if key.canonical_path != other.canonical_path && same_inode {
            return Err(CoordinatorError::ProjectAliasCollision);
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DatabaseIdentity {
    canonical_path: String,
    device: u64,
    inode: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyAuthorityRecord {
    version: u32,
    uid: u32,
    database: DatabaseIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnrollmentEvidence {
    plan_digest: String,
    previous_record_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DurableAuthorityRecord {
    version: u32,
    uid: u32,
    database: FilesystemObservation,
    enrollment: Option<EnrollmentEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
enum AuthorityRecord {
    Legacy(LegacyAuthorityRecord),
    Durable(DurableAuthorityRecord),
}

impl AuthorityRecord {
    fn parse(bytes: &[u8]) -> Result<Self, CoordinatorError> {
        let record: Self =
            serde_json::from_slice(bytes).map_err(|_| CoordinatorError::AuthorityCorrupt)?;
        match &record {
            Self::Legacy(record) => {
                if record.version != 1
                    || record.uid != uid()
                    || record.database.device == 0
                    || record.database.inode == 0
                    || !valid_canonical_path(&record.database.canonical_path)
                {
                    return Err(CoordinatorError::AuthorityCorrupt);
                }
            }
            Self::Durable(record) => {
                if record.version != 2 || record.uid != uid() || record.uid != record.database.uid {
                    return Err(CoordinatorError::AuthorityCorrupt);
                }
                record
                    .database
                    .validate()
                    .map_err(|_| CoordinatorError::AuthorityCorrupt)?;
                if let Some(evidence) = &record.enrollment {
                    validate_digest(&evidence.plan_digest)?;
                    validate_digest(&evidence.previous_record_sha256)?;
                }
            }
        }
        Ok(record)
    }

    fn database_path(&self) -> &str {
        match self {
            Self::Legacy(record) => &record.database.canonical_path,
            Self::Durable(record) => &record.database.canonical_path,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityNodeWitness {
    pub name: String,
    pub device: u64,
    pub inode: u64,
    pub uid: u32,
    pub mode: u32,
    pub links: u64,
    pub birth_sec: u64,
    pub birth_nsec: u32,
}

/// Bounded read-only plan. The digest binds the original record bytes, current
/// native identity, and the caller's locked grant/hold/writer inventory.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityRecoveryPlan {
    pub version: u32,
    pub authority_root: String,
    pub authority_nodes: Vec<AuthorityNodeWitness>,
    pub action: String,
    pub continuity: String,
    pub previous_record_sha256: String,
    pub previous_record: serde_json::Value,
    pub current: FilesystemObservation,
    pub inventory_digest: String,
    pub plan_digest: String,
}

/// This prepared receipt becomes committed only when authority.json has exactly
/// new_record_sha256. It is fsynced before publication; an interrupted prepare
/// cannot grant authority. The previous record is retained byte for byte.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityRecoveryReceipt {
    pub version: u32,
    pub plan: AuthorityRecoveryPlan,
    pub action: String,
    pub operator: String,
    pub plan_digest: String,
    pub previous_record_sha256: String,
    pub new_record_sha256: String,
    pub backup_file: String,
    pub receipt_file: String,
    pub current: FilesystemObservation,
}

impl JournalAuthority {
    pub fn inspect_recovery(
        database: &Path,
        inventory_digest: &str,
    ) -> Result<AuthorityRecoveryPlan, CoordinatorError> {
        inspect_recovery(&production_root(false)?, database, inventory_digest)
    }

    /// Explicit NEW enrollment, never a claim of legacy continuity. The caller
    /// authenticates the global operator and retains singleton, trust inventory,
    /// and database writer locks for the entire call. The callback must prove
    /// stopped legacy writers and return the exact inventory digest, twice.
    pub fn enroll_current_database(
        database: &Path,
        approved_plan_digest: &str,
        operator: &str,
        maintenance: impl FnMut(&Path) -> Result<String, CoordinatorError>,
    ) -> Result<AuthorityRecoveryReceipt, CoordinatorError> {
        enroll_current_database(
            &production_root(false)?,
            database,
            approved_plan_digest,
            operator,
            maintenance,
        )
    }
}

impl FixtureJournalAuthority {
    pub fn inspect_recovery(
        authority: &Path,
        database: &Path,
        inventory_digest: &str,
    ) -> Result<AuthorityRecoveryPlan, CoordinatorError> {
        inspect_recovery(authority, database, inventory_digest)
    }

    pub fn enroll_current_database(
        authority: &Path,
        database: &Path,
        approved_plan_digest: &str,
        operator: &str,
        maintenance: impl FnMut(&Path) -> Result<String, CoordinatorError>,
    ) -> Result<AuthorityRecoveryReceipt, CoordinatorError> {
        enroll_current_database(
            authority,
            database,
            approved_plan_digest,
            operator,
            maintenance,
        )
    }
}

struct AuthorityFiles {
    root: PathBuf,
    directory: File,
    lock: File,
    record_file: File,
    record_bytes: Vec<u8>,
}

impl AuthorityFiles {
    fn read(root: &Path, exclusive: bool) -> Result<Self, CoordinatorError> {
        no_symlink_directories(root).map_err(existing_authority_error)?;
        let directory = open_directory(root)?;
        let lock = open_at(&directory, "coordinator.lock", false, false)?;
        check_node(&root.join("coordinator.lock"), &lock, false)?;
        if exclusive {
            lock_exclusive(&lock)?;
        }
        let record_file = open_at(&directory, "authority.json", false, false)?;
        check_node(&root.join("authority.json"), &record_file, false)?;
        let record_bytes = read_bounded(&record_file, 8192)?;
        let files = Self {
            root: root.into(),
            directory,
            lock,
            record_file,
            record_bytes,
        };
        files.check()?;
        Ok(files)
    }

    fn check(&self) -> Result<(), CoordinatorError> {
        no_symlink_directories(&self.root)?;
        check_node(&self.root, &self.directory, true)?;
        check_node(&self.root.join("coordinator.lock"), &self.lock, false)?;
        check_node(&self.root.join("authority.json"), &self.record_file, false)?;
        if read_bounded(&self.record_file, 8192)? != self.record_bytes {
            return Err(CoordinatorError::AuthorityMismatch);
        }
        Ok(())
    }

    fn plan(
        &self,
        database: &Path,
        inventory_digest: &str,
    ) -> Result<AuthorityRecoveryPlan, CoordinatorError> {
        validate_digest(inventory_digest)?;
        self.check()?;
        let record = AuthorityRecord::parse(&self.record_bytes)?;
        // A different path needs a separate future enrollment contract. This
        // operation is scoped to the reviewed database at the pinned path.
        if database != Path::new(record.database_path()) {
            return Err(CoordinatorError::AuthorityMismatch);
        }
        let current = filesystem_identity::observe(database, IdentityKind::PrivateFile)
            .map_err(identity_error)?;
        let continuity = match record {
            AuthorityRecord::Durable(ref pinned)
                if filesystem_identity::accepts_observation(&pinned.database, &current).is_ok() =>
            {
                "persistent_identity_verified"
            }
            _ => "continuity_unproven",
        };
        let mut plan = AuthorityRecoveryPlan {
            version: 1,
            authority_root: self
                .root
                .to_str()
                .ok_or(CoordinatorError::InsecureAuthority)?
                .into(),
            authority_nodes: vec![
                node_witness(".", &self.directory)?,
                node_witness("coordinator.lock", &self.lock)?,
                node_witness("authority.json", &self.record_file)?,
            ],
            action: "enroll_current_database_as_new_authority".into(),
            continuity: continuity.into(),
            previous_record_sha256: digest(&self.record_bytes),
            previous_record: serde_json::from_slice(&self.record_bytes)
                .map_err(|_| CoordinatorError::AuthorityCorrupt)?,
            current,
            inventory_digest: inventory_digest.into(),
            plan_digest: String::new(),
        };
        plan.plan_digest = digest(&json_bytes(&plan)?);
        self.check()?;
        if filesystem_identity::observe(database, IdentityKind::PrivateFile)
            .map_err(identity_error)?
            != plan.current
        {
            return Err(CoordinatorError::AuthorityMismatch);
        }
        Ok(plan)
    }
}

fn inspect_recovery(
    root: &Path,
    database: &Path,
    inventory_digest: &str,
) -> Result<AuthorityRecoveryPlan, CoordinatorError> {
    // No create, flock, database connection, or metadata mutation on inspection.
    AuthorityFiles::read(root, false)?.plan(database, inventory_digest)
}

fn enroll_current_database(
    root: &Path,
    database: &Path,
    approved_plan_digest: &str,
    operator: &str,
    mut maintenance: impl FnMut(&Path) -> Result<String, CoordinatorError>,
) -> Result<AuthorityRecoveryReceipt, CoordinatorError> {
    validate_digest(approved_plan_digest)?;
    if operator.is_empty() || operator.len() > 256 || operator.chars().any(char::is_control) {
        return Err(CoordinatorError::InsecureAuthority);
    }
    let files = AuthorityFiles::read(root, true)?;
    let inventory = maintenance(database)?;
    let plan = files.plan(database, &inventory)?;
    if plan.plan_digest != approved_plan_digest {
        return Err(CoordinatorError::AuthorityMismatch);
    }
    let new_record = AuthorityRecord::Durable(DurableAuthorityRecord {
        version: 2,
        uid: uid(),
        database: plan.current.clone(),
        enrollment: Some(EnrollmentEvidence {
            plan_digest: plan.plan_digest.clone(),
            previous_record_sha256: plan.previous_record_sha256.clone(),
        }),
    });
    let bytes = json_bytes(&new_record)?;
    let receipt = AuthorityRecoveryReceipt {
        plan: plan.clone(),
        version: 1,
        action: plan.action.clone(),
        operator: operator.into(),
        plan_digest: plan.plan_digest.clone(),
        previous_record_sha256: plan.previous_record_sha256.clone(),
        new_record_sha256: digest(&bytes),
        backup_file: backup_name(&plan.previous_record_sha256),
        receipt_file: receipt_name(&plan.plan_digest),
        current: plan.current.clone(),
    };
    // Original and audit receipt are durable BEFORE the sole activation rename.
    // A retry may reuse only byte-identical preparations; never overwrite evidence.
    preserve_file(&files, &receipt.backup_file, &files.record_bytes)?;
    preserve_file(&files, &receipt.receipt_file, &json_bytes(&receipt)?)?;
    files.directory.sync_all().map_err(authority_io)?;
    let temporary_name = format!(".authority-{}.pending", uuid::Uuid::new_v4());
    let mut pending = open_at(&files.directory, &temporary_name, true, true)?;
    pending.write_all(&bytes).map_err(authority_io)?;
    pending.sync_all().map_err(authority_io)?;
    check_node(&root.join(&temporary_name), &pending, false)?;
    // External stop proof and inventories are rechecked after preparation,
    // while all cooperating writer locks and the authority lock remain held.
    let final_inventory = maintenance(database)?;
    if files.plan(database, &final_inventory)? != plan {
        return Err(CoordinatorError::AuthorityMismatch);
    }
    check_node(&root.join(&temporary_name), &pending, false)?;
    if read_bounded(&pending, 8192)? != bytes {
        return Err(CoordinatorError::AuthorityCorrupt);
    }
    verify_recovery_evidence(&files.directory, root, &new_record, &bytes)?;
    files.check()?;
    let from = CString::new(temporary_name).map_err(|_| CoordinatorError::InsecureAuthority)?;
    let to = c"authority.json";
    if unsafe {
        libc::renameat(
            files.directory.as_raw_fd(),
            from.as_ptr(),
            files.directory.as_raw_fd(),
            to.as_ptr(),
        )
    } != 0
    {
        return Err(authority_io(std::io::Error::last_os_error()));
    }
    files.directory.sync_all().map_err(authority_io)?;
    check_node(&root.join("authority.json"), &pending, false)?;
    if read_bounded(&pending, 8192)? != bytes
        || filesystem_identity::observe(database, IdentityKind::PrivateFile)
            .map_err(identity_error)?
            != plan.current
    {
        return Err(CoordinatorError::AuthorityMismatch);
    }
    verify_recovery_evidence(&files.directory, root, &new_record, &bytes)?;
    Ok(receipt)
}

fn preserve_file(files: &AuthorityFiles, name: &str, bytes: &[u8]) -> Result<(), CoordinatorError> {
    match open_at(&files.directory, name, false, false) {
        Ok(existing) => {
            check_node(&files.root.join(name), &existing, false)?;
            if read_bounded(&existing, 32768)? != bytes {
                return Err(CoordinatorError::AuthorityCorrupt);
            }
            existing.sync_all().map_err(authority_io)?;
            return Ok(());
        }
        Err(CoordinatorError::AuthorityMissing) => {}
        Err(error) => return Err(error),
    }
    // Never expose a half-written final evidence file. Crash leftovers have a
    // unique pending name and do not poison an exact-plan retry. Publication
    // must not replace any previously preserved evidence.
    let temporary = format!(".evidence-{}.pending", uuid::Uuid::new_v4());
    let mut file = open_at(&files.directory, &temporary, true, true)?;
    file.write_all(bytes).map_err(authority_io)?;
    file.sync_all().map_err(authority_io)?;
    check_node(&files.root.join(&temporary), &file, false)?;
    if read_bounded(&file, 32768)? != bytes {
        return Err(CoordinatorError::AuthorityCorrupt);
    }
    files.check()?;
    publish_evidence_exclusive(&files.directory, &temporary, name)?;
    files.directory.sync_all().map_err(authority_io)?;
    check_node(&files.root.join(name), &file, false)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn publish_evidence_exclusive(
    directory: &File,
    temporary: &str,
    name: &str,
) -> Result<(), CoordinatorError> {
    let from = CString::new(temporary).map_err(|_| CoordinatorError::InsecureAuthority)?;
    let to = CString::new(name).map_err(|_| CoordinatorError::InsecureAuthority)?;
    if unsafe {
        libc::renameatx_np(
            directory.as_raw_fd(),
            from.as_ptr(),
            directory.as_raw_fd(),
            to.as_ptr(),
            libc::RENAME_EXCL,
        )
    } != 0
    {
        return Err(authority_io(std::io::Error::last_os_error()));
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn publish_evidence_exclusive(_: &File, _: &str, _: &str) -> Result<(), CoordinatorError> {
    Err(CoordinatorError::PersistentIdentityUnavailable)
}

fn node_witness(name: &str, file: &File) -> Result<AuthorityNodeWitness, CoordinatorError> {
    let metadata = file.metadata().map_err(authority_io)?;
    let birth = metadata
        .created()
        .map_err(authority_io)?
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| CoordinatorError::InsecureAuthority)?;
    Ok(AuthorityNodeWitness {
        name: name.into(),
        device: metadata.dev(),
        inode: metadata.ino(),
        uid: metadata.uid(),
        mode: metadata.mode(),
        links: if metadata.is_dir() {
            0
        } else {
            metadata.nlink()
        },
        birth_sec: birth.as_secs(),
        birth_nsec: birth.subsec_nanos(),
    })
}
fn backup_name(digest: &str) -> String {
    format!("authority-{digest}.original.json")
}
fn receipt_name(digest: &str) -> String {
    format!("enrollment-{digest}.json")
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn validate_digest(value: &str) -> Result<(), CoordinatorError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(CoordinatorError::AuthorityCorrupt);
    }
    Ok(())
}
fn valid_canonical_path(value: &str) -> bool {
    value.len() <= 4096
        && value.starts_with('/')
        && !value.ends_with('/')
        && !value.chars().any(char::is_control)
        && !value[1..]
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
}
fn json_bytes(value: &impl Serialize) -> Result<Vec<u8>, CoordinatorError> {
    serde_json::to_vec(value).map_err(|_| CoordinatorError::AuthorityCorrupt)
}
fn read_bounded(file: &File, maximum: u64) -> Result<Vec<u8>, CoordinatorError> {
    let length = file.metadata().map_err(authority_io)?.len();
    if length == 0 || length > maximum {
        return Err(CoordinatorError::AuthorityCorrupt);
    }
    let mut bytes = vec![0; length as usize];
    file.read_exact_at(&mut bytes, 0)
        .map_err(|_| CoordinatorError::AuthorityCorrupt)?;
    if file.metadata().map_err(authority_io)?.len() != length {
        return Err(CoordinatorError::AuthorityCorrupt);
    }
    Ok(bytes)
}
fn identity_error(error: IdentityError) -> CoordinatorError {
    match error {
        IdentityError::Unsupported | IdentityError::Unavailable => {
            CoordinatorError::PersistentIdentityUnavailable
        }
        IdentityError::Insecure => CoordinatorError::InsecureAuthority,
        IdentityError::Changed => CoordinatorError::AuthorityMismatch,
        IdentityError::Invalid => CoordinatorError::AuthorityCorrupt,
    }
}
fn open_directory(root: &Path) -> Result<File, CoordinatorError> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(root)
        .map_err(authority_io)?;
    check_node(root, &file, true)?;
    Ok(file)
}
fn lock_exclusive(lock: &File) -> Result<(), CoordinatorError> {
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        let error = std::io::Error::last_os_error();
        return Err(if error.raw_os_error() == Some(libc::EWOULDBLOCK) {
            CoordinatorError::AuthorityBusy
        } else {
            CoordinatorError::StorageUnavailable
        });
    }
    Ok(())
}
fn verify_recovery_evidence(
    directory: &File,
    root: &Path,
    record: &AuthorityRecord,
    bytes: &[u8],
) -> Result<(), CoordinatorError> {
    let AuthorityRecord::Durable(record) = record else {
        return Ok(());
    };
    let Some(evidence) = &record.enrollment else {
        return Ok(());
    };
    let backup_name = backup_name(&evidence.previous_record_sha256);
    let backup = open_at(directory, &backup_name, false, false)?;
    check_node(&root.join(&backup_name), &backup, false)?;
    if digest(&read_bounded(&backup, 8192)?) != evidence.previous_record_sha256 {
        return Err(CoordinatorError::AuthorityCorrupt);
    }
    let receipt_name = receipt_name(&evidence.plan_digest);
    let receipt = open_at(directory, &receipt_name, false, false)?;
    check_node(&root.join(&receipt_name), &receipt, false)?;
    let receipt: AuthorityRecoveryReceipt = serde_json::from_slice(&read_bounded(&receipt, 32768)?)
        .map_err(|_| CoordinatorError::AuthorityCorrupt)?;
    let mut approved_plan = receipt.plan.clone();
    approved_plan.plan_digest.clear();
    if digest(&json_bytes(&approved_plan)?) != receipt.plan_digest
        || receipt.plan.plan_digest != receipt.plan_digest
        || receipt.plan.current != record.database
        || receipt.plan.previous_record_sha256 != receipt.previous_record_sha256
        || receipt.plan.action != receipt.action
        || receipt.version != 1
        || receipt.action != "enroll_current_database_as_new_authority"
        || receipt.operator.is_empty()
        || receipt.operator.len() > 256
        || receipt.operator.chars().any(char::is_control)
        || receipt.plan_digest != evidence.plan_digest
        || receipt.previous_record_sha256 != evidence.previous_record_sha256
        || receipt.new_record_sha256 != digest(bytes)
        || receipt.backup_file != backup_name
        || receipt.receipt_file != receipt_name
        || receipt.current != record.database
    {
        return Err(CoordinatorError::AuthorityCorrupt);
    }
    Ok(())
}

struct Authority {
    files: AuthorityFiles,
    expected: AuthorityRecord,
    // Pin current boot too: an older persisted boot must not excuse a second
    // device change while this process is running.
    opened_observation: Option<FilesystemObservation>,
}

impl Authority {
    fn open(
        root: &Path,
        database: &Path,
        allow_bootstrap: bool,
        durable_bootstrap: bool,
        bootstrap_check: impl FnOnce(&Path) -> Result<(), CoordinatorError>,
    ) -> Result<Self, CoordinatorError> {
        let legacy_identity = database_identity(database)?;
        let fresh = if allow_bootstrap {
            create_private_directory(root)?
        } else {
            no_symlink_directories(root).map_err(existing_authority_error)?;
            false
        };
        if fresh {
            no_symlink_directories(root)?;
            let directory = open_directory(root)?;
            let lock = open_at(&directory, "coordinator.lock", true, true)?;
            check_node(&root.join("coordinator.lock"), &lock, false)?;
            lock_exclusive(&lock)?;
            lock.sync_all().map_err(authority_io)?;
            directory.sync_all().map_err(authority_io)?;
            // The original zero-uncertainty bootstrap proof is unchanged.
            bootstrap_check(Path::new(&legacy_identity.canonical_path))?;
            if database_identity(database)? != legacy_identity {
                return Err(CoordinatorError::AuthorityMismatch);
            }
            let expected = if durable_bootstrap {
                AuthorityRecord::Durable(DurableAuthorityRecord {
                    version: 2,
                    uid: uid(),
                    enrollment: None,
                    database: filesystem_identity::observe(
                        Path::new(&legacy_identity.canonical_path),
                        IdentityKind::PrivateFile,
                    )
                    .map_err(identity_error)?,
                })
            } else {
                // Portable fixtures retain v1 and cannot produce live authority.
                AuthorityRecord::Legacy(LegacyAuthorityRecord {
                    version: 1,
                    uid: uid(),
                    database: legacy_identity,
                })
            };
            let record_bytes = json_bytes(&expected)?;
            let mut record_file = open_at(&directory, "authority.json", true, true)?;
            record_file.write_all(&record_bytes).map_err(authority_io)?;
            record_file.sync_all().map_err(authority_io)?;
            directory.sync_all().map_err(authority_io)?;
            let files = AuthorityFiles {
                root: root.into(),
                directory,
                lock,
                record_file,
                record_bytes,
            };
            return Self::from_files(files, database);
        }
        Self::from_files(AuthorityFiles::read(root, true)?, database)
    }

    fn from_files(files: AuthorityFiles, database: &Path) -> Result<Self, CoordinatorError> {
        let expected = AuthorityRecord::parse(&files.record_bytes)?;
        let current_legacy = database_identity(database)?;
        if current_legacy.canonical_path != expected.database_path() {
            return Err(CoordinatorError::AuthorityMismatch);
        }
        let opened_observation = match &expected {
            AuthorityRecord::Legacy(record) => {
                if record.database != current_legacy {
                    return Err(CoordinatorError::AuthorityMismatch);
                }
                None
            }
            AuthorityRecord::Durable(record) => {
                let current = filesystem_identity::observe(
                    Path::new(expected.database_path()),
                    IdentityKind::PrivateFile,
                )
                .map_err(identity_error)?;
                filesystem_identity::accepts_observation(&record.database, &current)
                    .map_err(identity_error)?;
                Some(current)
            }
        };
        let authority = Self {
            files,
            expected,
            opened_observation,
        };
        authority.check()?;
        Ok(authority)
    }

    fn check(&self) -> Result<(), CoordinatorError> {
        self.files.check()?;
        match &self.expected {
            AuthorityRecord::Legacy(record) => {
                if database_identity(Path::new(&record.database.canonical_path))? != record.database
                {
                    return Err(CoordinatorError::AuthorityMismatch);
                }
            }
            AuthorityRecord::Durable(record) => {
                let current = filesystem_identity::observe(
                    Path::new(&record.database.canonical_path),
                    IdentityKind::PrivateFile,
                )
                .map_err(identity_error)?;
                if self.opened_observation.as_ref() != Some(&current) {
                    return Err(CoordinatorError::AuthorityMismatch);
                }
                filesystem_identity::accepts_observation(&record.database, &current)
                    .map_err(identity_error)?;
            }
        }
        verify_recovery_evidence(
            &self.files.directory,
            &self.files.root,
            &self.expected,
            &self.files.record_bytes,
        )
    }
}

fn uid() -> u32 {
    unsafe { libc::geteuid() }
}

fn database_identity(path: &Path) -> Result<DatabaseIdentity, CoordinatorError> {
    if !path.is_absolute() {
        return Err(CoordinatorError::AuthorityMismatch);
    }
    let canonical = fs::canonicalize(path).map_err(authority_io)?;
    let metadata = fs::symlink_metadata(&canonical).map_err(authority_io)?;
    if !metadata.is_file() || metadata.nlink() != 1 || metadata.uid() != uid() {
        return Err(CoordinatorError::InsecureAuthority);
    }
    let canonical_path = canonical
        .to_str()
        .ok_or(CoordinatorError::AuthorityMismatch)?
        .to_owned();
    if canonical_path.len() > 4096 {
        return Err(CoordinatorError::AuthorityMismatch);
    }
    Ok(DatabaseIdentity {
        canonical_path,
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

fn authority_io(error: std::io::Error) -> CoordinatorError {
    match error.raw_os_error() {
        Some(libc::ELOOP) | Some(libc::ENOTDIR) => CoordinatorError::InsecureAuthority,
        Some(libc::ENOENT) => CoordinatorError::AuthorityMissing,
        Some(libc::EEXIST) => CoordinatorError::AuthorityCorrupt,
        _ => CoordinatorError::StorageUnavailable,
    }
}

pub(crate) fn no_symlink_directories(path: &Path) -> Result<(), CoordinatorError> {
    if !path.is_absolute() {
        return Err(CoordinatorError::InsecureAuthority);
    }
    let mut prefix = PathBuf::new();
    for component in path.components() {
        match component {
            Component::RootDir | Component::Normal(_) => prefix.push(component.as_os_str()),
            _ => return Err(CoordinatorError::InsecureAuthority),
        }
        if !fs::symlink_metadata(&prefix)
            .map_err(authority_io)?
            .is_dir()
        {
            return Err(CoordinatorError::InsecureAuthority);
        }
    }
    Ok(())
}

fn same_inode(a: &Metadata, b: &Metadata) -> bool {
    a.dev() == b.dev() && a.ino() == b.ino()
}

pub(crate) fn check_node(
    path: &Path,
    file: &File,
    directory: bool,
) -> Result<(), CoordinatorError> {
    let named = fs::symlink_metadata(path).map_err(authority_io)?;
    let opened = file.metadata().map_err(authority_io)?;
    for metadata in [&named, &opened] {
        let valid_kind = if directory {
            metadata.is_dir()
        } else {
            metadata.is_file() && metadata.nlink() == 1
        };
        let mode = if directory { 0o700 } else { 0o600 };
        if !valid_kind || metadata.uid() != uid() || metadata.mode() & 0o7777 != mode {
            return Err(CoordinatorError::InsecureAuthority);
        }
    }
    if !same_inode(&named, &opened) {
        return Err(CoordinatorError::InsecureAuthority);
    }
    Ok(())
}

pub(crate) fn create_private_directory(path: &Path) -> Result<bool, CoordinatorError> {
    let parent = path.parent().ok_or(CoordinatorError::InsecureAuthority)?;
    no_symlink_directories(parent)?;
    match DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {
            File::open(parent)
                .and_then(|f| f.sync_all())
                .map_err(authority_io)?;
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(authority_io(error)),
    }
}

pub(crate) fn open_at(
    directory: &File,
    name: &str,
    fresh: bool,
    write: bool,
) -> Result<File, CoordinatorError> {
    let name = CString::new(name).map_err(|_| CoordinatorError::InsecureAuthority)?;
    let mut flags = libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK;
    flags |= if write { libc::O_RDWR } else { libc::O_RDONLY };
    if fresh {
        flags |= libc::O_CREAT | libc::O_EXCL;
    }
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            flags,
            0o600 as libc::c_uint,
        )
    };
    if fd < 0 {
        return Err(authority_io(std::io::Error::last_os_error()));
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn existing_authority_error(error: CoordinatorError) -> CoordinatorError {
    if error == CoordinatorError::AuthorityMissing {
        CoordinatorError::LegacyTransitionUnproven
    } else {
        error
    }
}

fn production_root(allow_bootstrap: bool) -> Result<PathBuf, CoordinatorError> {
    // HOME, DEVELOPER_DIR, daemon port and database configuration must not choose
    // this location. Resolve the effective UID's home through the OS account DB.
    let mut buffer = vec![0u8; 16 * 1024];
    let home = loop {
        let mut entry: libc::passwd = unsafe { std::mem::zeroed() };
        let mut result = std::ptr::null_mut();
        let code = unsafe {
            libc::getpwuid_r(
                uid(),
                &mut entry,
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut result,
            )
        };
        if code == libc::ERANGE && buffer.len() < 1024 * 1024 {
            buffer.resize(buffer.len() * 2, 0);
            continue;
        }
        if code != 0 || result.is_null() || entry.pw_dir.is_null() {
            return Err(CoordinatorError::StorageUnavailable);
        }
        let bytes = unsafe { CStr::from_ptr(entry.pw_dir) }.to_bytes();
        break PathBuf::from(OsStr::from_bytes(bytes));
    };
    no_symlink_directories(&home)?;
    if fs::metadata(&home).map_err(authority_io)?.uid() != uid() {
        return Err(CoordinatorError::InsecureAuthority);
    }
    let mut parent = home;
    for name in ["Library", "Application Support", "Chainworks Forge"] {
        parent.push(name);
        if allow_bootstrap {
            create_private_directory(&parent)?;
            no_symlink_directories(&parent)?;
        } else {
            no_symlink_directories(&parent).map_err(existing_authority_error)?;
        }
        let metadata = fs::metadata(&parent).map_err(authority_io)?;
        if metadata.uid() != uid() || metadata.mode() & 0o022 != 0 {
            return Err(CoordinatorError::InsecureAuthority);
        }
    }
    Ok(parent.join("xcode-runtime"))
}

#[cfg(all(test, target_os = "macos"))]
mod authority_identity_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn runtime_recheck_uses_current_open_pin_even_when_persisted_boot_would_accept_drift() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let database = root.join("fixture.sqlite");
        fs::write(&database, b"isolated fixture").unwrap();
        fs::set_permissions(&database, fs::Permissions::from_mode(0o600)).unwrap();
        let directory = root.join("authority");
        drop(Authority::open(&directory, &database, true, false, |_| Ok(())).unwrap());
        let current = filesystem_identity::observe(&database, IdentityKind::PrivateFile).unwrap();
        let mut older_boot = current.clone();
        older_boot.device += 1;
        older_boot.boot_session_uuid = "00000000-0000-4000-8000-000000000011".into();
        fs::write(
            directory.join("authority.json"),
            json_bytes(&AuthorityRecord::Durable(DurableAuthorityRecord {
                version: 2,
                uid: uid(),
                database: older_boot.clone(),
                enrollment: None,
            }))
            .unwrap(),
        )
        .unwrap();
        let mut authority =
            Authority::open(&directory, &database, false, false, |_| unreachable!()).unwrap();
        assert_eq!(authority.opened_observation.as_ref(), Some(&current));
        authority.check().unwrap();
        // Synthetic current-boot device observations model drift without remounting
        // the host. The persisted old boot would accept either observation.
        authority.opened_observation.as_mut().unwrap().device += 2;
        filesystem_identity::accepts_observation(&older_boot, &current).unwrap();
        filesystem_identity::accepts_observation(
            &older_boot,
            authority.opened_observation.as_ref().unwrap(),
        )
        .unwrap();
        assert_eq!(authority.check(), Err(CoordinatorError::AuthorityMismatch));
    }
}
