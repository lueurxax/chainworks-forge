//! Cooperating workspace access and persistent per-user journal authority.
//!
//! Daemon wiring retains `JournalAuthority` and injects ONE coordinator Arc into
//! every live route. Fixture authority deliberately has a different Rust type.
//! An owner lives only for an invocation, not an idle provider session. Callers
//! retain permits until work finishes or a durable uncertainty hold replaces it;
//! dropping a permit does not prove an external operation stopped.

use domain::xcode_effect::ProjectKey;
use serde::{Deserialize, Serialize};
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
            inner: Authority::open(&production_root(false)?, database, false, |_| {
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
            inner: Authority::open(&production_root(true)?, database, true, check)?,
        }))
    }
}

impl FixtureJournalAuthority {
    pub fn open_existing(authority: &Path, database: &Path) -> Result<Arc<Self>, CoordinatorError> {
        Ok(Arc::new(Self {
            inner: Authority::open(authority, database, false, |_| {
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
            inner: Authority::open(authority, database, true, check)?,
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

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DatabaseIdentity {
    canonical_path: String,
    device: u64,
    inode: u64,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthorityRecord {
    version: u32,
    uid: u32,
    database: DatabaseIdentity,
}

struct Authority {
    root: PathBuf,
    directory: File,
    lock: File,
    record_file: File,
    expected: AuthorityRecord,
}

impl Authority {
    fn open(
        root: &Path,
        database: &Path,
        allow_bootstrap: bool,
        bootstrap_check: impl FnOnce(&Path) -> Result<(), CoordinatorError>,
    ) -> Result<Self, CoordinatorError> {
        let expected = AuthorityRecord {
            version: 1,
            uid: uid(),
            database: database_identity(database)?,
        };
        let fresh = if allow_bootstrap {
            create_private_directory(root)?
        } else {
            // Do not route this branch through any create-capable helper, even
            // after an existence check: a disappearing directory must not turn
            // an ordinary startup into a partial bootstrap.
            no_symlink_directories(root).map_err(existing_authority_error)?;
            false
        };
        no_symlink_directories(root)?;
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(root)
            .map_err(authority_io)?;
        check_node(root, &directory, true)?;
        let lock = open_at(&directory, "coordinator.lock", fresh, true)?;
        check_node(&root.join("coordinator.lock"), &lock, false)?;
        // flock is tied to this open file description. Closing releases the
        // kernel lock; no path is ever unlinked, including error paths.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            let error = std::io::Error::last_os_error();
            return Err(if error.raw_os_error() == Some(libc::EWOULDBLOCK) {
                CoordinatorError::AuthorityBusy
            } else {
                CoordinatorError::StorageUnavailable
            });
        }
        let record_file = if fresh {
            lock.sync_all().map_err(authority_io)?;
            directory.sync_all().map_err(authority_io)?;
            bootstrap_check(Path::new(&expected.database.canonical_path))?;
            if database_identity(database)? != expected.database {
                return Err(CoordinatorError::AuthorityMismatch);
            }
            let mut file = open_at(&directory, "authority.json", true, true)?;
            let bytes =
                serde_json::to_vec(&expected).map_err(|_| CoordinatorError::AuthorityCorrupt)?;
            file.write_all(&bytes).map_err(authority_io)?;
            file.sync_all().map_err(authority_io)?;
            directory.sync_all().map_err(authority_io)?;
            file
        } else {
            open_at(&directory, "authority.json", false, false)?
        };
        let authority = Self {
            root: root.to_owned(),
            directory,
            lock,
            record_file,
            expected,
        };
        authority.check()?;
        Ok(authority)
    }

    fn check(&self) -> Result<(), CoordinatorError> {
        no_symlink_directories(&self.root)?;
        check_node(&self.root, &self.directory, true)?;
        check_node(&self.root.join("coordinator.lock"), &self.lock, false)?;
        check_node(&self.root.join("authority.json"), &self.record_file, false)?;
        let length = self.record_file.metadata().map_err(authority_io)?.len();
        if length == 0 || length > 8192 {
            return Err(CoordinatorError::AuthorityCorrupt);
        }
        let mut bytes = vec![0; length as usize];
        self.record_file
            .read_exact_at(&mut bytes, 0)
            .map_err(|_| CoordinatorError::AuthorityCorrupt)?;
        let record: AuthorityRecord =
            serde_json::from_slice(&bytes).map_err(|_| CoordinatorError::AuthorityCorrupt)?;
        if record.version != 1 || record.uid != uid() {
            return Err(CoordinatorError::AuthorityCorrupt);
        }
        if record != self.expected {
            return Err(CoordinatorError::AuthorityMismatch);
        }
        if database_identity(Path::new(&record.database.canonical_path))? != record.database {
            return Err(CoordinatorError::AuthorityMismatch);
        }
        Ok(())
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
