/// P089 Temporary Artifact Inventory — scanner infrastructure.
///
/// Read-only, descriptor-relative no-follow traversal with permit guards,
/// cooperative cancellation, device-boundary enforcement, cycle detection,
/// and bounded partial errors.
/// No deletion, cleanup, persistence, migration, or mutation of any kind.
use std::collections::{HashMap, HashSet};
use std::ffi::{CString, OsStr};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::os::unix::io::RawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use chrono::Utc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use domain::temp_artifact_inventory::{
    compute_path_hash, compute_path_hash_short, DryRunRecommendation, InventoryErrorCode,
    InventoryStatus, LifecycleClassification, RootKind, SCAN_CANCEL_CHECK_INTERVAL_ENTRIES,
    SCAN_CANCEL_CHECK_INTERVAL_MS, SCAN_CONTEXT_PERMIT_MAX, SCAN_GLOBAL_PERMIT_MAX,
    SCAN_MAX_DIR_DEPTH, SCAN_MAX_PATH_BYTES, SCAN_MAX_VISITED_DIRS,
    SCAN_PARTIAL_ERRORS_MAX_PER_PAYLOAD, SCAN_PARTIAL_ERRORS_MAX_PER_ROW,
    TEMP_ARTIFACT_INVENTORY_SCHEMA_VERSION,
};

// ── Descriptor-relative no-follow helpers (SEC-P089-HIGH-001) ────────────────

const SCAN_MAX_TOTAL_ENTRIES: usize = 100_000;

struct OwnedFd(RawFd);

impl OwnedFd {
    fn new(fd: RawFd) -> Self {
        Self(fd)
    }

    fn raw(&self) -> RawFd {
        self.0
    }

    fn into_raw(self) -> RawFd {
        let fd = self.0;
        std::mem::forget(self);
        fd
    }
}

impl Drop for OwnedFd {
    fn drop(&mut self) {
        if self.0 >= 0 {
            unsafe { libc::close(self.0) };
        }
    }
}

/// RAII wrapper for a `*mut libc::DIR` obtained via `fdopendir`.
/// Calls `closedir` (which also closes the underlying fd) on drop.
struct OwnedDir(*mut libc::DIR);

impl Drop for OwnedDir {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { libc::closedir(self.0) };
        }
    }
}

// SAFETY: OwnedDir is used only on the thread that opened it; never shared.
unsafe impl Send for OwnedDir {}

struct DirStream {
    dir: OwnedDir,
}

impl DirStream {
    fn from_fd(fd: OwnedFd) -> io::Result<Self> {
        let raw_fd = fd.into_raw();
        let dir_ptr = unsafe { libc::fdopendir(raw_fd) };
        if dir_ptr.is_null() {
            unsafe { libc::close(raw_fd) };
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            dir: OwnedDir(dir_ptr),
        })
    }

    fn fd(&self) -> RawFd {
        unsafe { libc::dirfd(self.dir.0) }
    }

    fn next_name(&mut self) -> Option<Vec<u8>> {
        loop {
            let entry_ptr = unsafe { libc::readdir(self.dir.0) };
            if entry_ptr.is_null() {
                return None;
            }
            let name_bytes: Vec<u8> = unsafe {
                let name_ptr = (*entry_ptr).d_name.as_ptr() as *const u8;
                let mut len = 0usize;
                while *name_ptr.add(len) != 0 {
                    len += 1;
                }
                std::slice::from_raw_parts(name_ptr, len).to_vec()
            };
            if name_bytes == b"." || name_bytes == b".." {
                continue;
            }
            return Some(name_bytes);
        }
    }
}

fn os_str_to_cstring(os: &OsStr) -> io::Result<CString> {
    CString::new(os.as_bytes()).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))
}

fn bytes_to_cstring(bytes: &[u8]) -> io::Result<CString> {
    CString::new(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))
}

fn zeroed_stat() -> libc::stat {
    unsafe { std::mem::zeroed() }
}

fn fstat_fd(fd: RawFd) -> io::Result<libc::stat> {
    let mut stat = zeroed_stat();
    let rc = unsafe { libc::fstat(fd, &mut stat) };
    if rc == 0 {
        Ok(stat)
    } else {
        Err(io::Error::last_os_error())
    }
}

fn fstatat_child(parent_fd: RawFd, name: &[u8]) -> io::Result<libc::stat> {
    let c_name = bytes_to_cstring(name)?;
    let mut stat = zeroed_stat();
    let rc = unsafe {
        libc::fstatat(
            parent_fd,
            c_name.as_ptr(),
            &mut stat,
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if rc == 0 {
        Ok(stat)
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Follow-stat (metadata only, no fd opened, no content read) used exclusively to
/// classify a symlink *entry's target type* — directory vs anything else — so the
/// traversal_policy distinction between symlinked directories (bounded partial
/// error, not descended) and symlinked files (link metadata only row) can be made
/// without ever opening or reading the target. This is a single `fstatat` syscall
/// and never crosses into descending the target even when it is a directory.
fn fstatat_child_follow(parent_fd: RawFd, name: &[u8]) -> io::Result<libc::stat> {
    let c_name = bytes_to_cstring(name)?;
    let mut stat = zeroed_stat();
    let rc = unsafe { libc::fstatat(parent_fd, c_name.as_ptr(), &mut stat, 0) };
    if rc == 0 {
        Ok(stat)
    } else {
        Err(io::Error::last_os_error())
    }
}

fn stat_mode(stat: &libc::stat) -> libc::mode_t {
    stat.st_mode as libc::mode_t
}

fn stat_is_dir(stat: &libc::stat) -> bool {
    (stat_mode(stat) & libc::S_IFMT) == libc::S_IFDIR
}

fn stat_is_symlink(stat: &libc::stat) -> bool {
    (stat_mode(stat) & libc::S_IFMT) == libc::S_IFLNK
}

fn stat_size(stat: &libc::stat) -> u64 {
    if stat.st_size <= 0 {
        0
    } else {
        stat.st_size as u64
    }
}

fn stat_dev(stat: &libc::stat) -> u64 {
    stat.st_dev as u64
}

fn stat_ino(stat: &libc::stat) -> u64 {
    stat.st_ino as u64
}

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "linux",
    target_os = "android"
))]
fn stat_mtime(stat: &libc::stat) -> Option<String> {
    chrono::DateTime::<Utc>::from_timestamp(stat.st_mtime, stat.st_mtime_nsec as u32)
        .map(|dt| dt.to_rfc3339())
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "linux",
    target_os = "android"
)))]
fn stat_mtime(_stat: &libc::stat) -> Option<String> {
    None
}

fn open_child_dir_at(parent_fd: RawFd, name: &[u8]) -> io::Result<OwnedFd> {
    let c_name = bytes_to_cstring(name)?;
    // O_DIRECTORY: fail immediately with ENOTDIR if the target is not a directory,
    // closing the TOCTOU window where a special file (FIFO, device) could be swapped
    // in between fstatat and openat and cause the open to block.
    // O_NONBLOCK: additional defence-in-depth; open never blocks on special files.
    // SEC-P089-HIGH-001
    let fd = unsafe {
        libc::openat(
            parent_fd,
            c_name.as_ptr(),
            libc::O_RDONLY
                | libc::O_NOFOLLOW
                | libc::O_CLOEXEC
                | libc::O_DIRECTORY
                | libc::O_NONBLOCK,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let owned = OwnedFd::new(fd);
    let stat = fstat_fd(owned.raw())?;
    if stat_is_dir(&stat) {
        Ok(owned)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "opened child is not a directory",
        ))
    }
}

fn open_absolute_dir_nofollow(path: &Path) -> io::Result<OwnedFd> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "scan root must be absolute",
        ));
    }

    let root_path = CString::new("/").expect("literal has no nul");
    // O_DIRECTORY ensures the fd refers to a directory; O_NONBLOCK prevents blocking.
    // SEC-P089-HIGH-001
    let root_fd = unsafe {
        libc::open(
            root_path.as_ptr(),
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NONBLOCK,
        )
    };
    if root_fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut current = OwnedFd::new(root_fd);

    for component in path.components() {
        match component {
            std::path::Component::RootDir => {}
            std::path::Component::CurDir => {}
            std::path::Component::Normal(os) => {
                let c_name = os_str_to_cstring(os)?;
                // Explicit symlink check before openat so ELOOP is returned for symlink
                // path components, preserving the caller's symlink-escape detection logic.
                // This check is separate from the O_DIRECTORY|O_NONBLOCK protection below.
                // SEC-P089-HIGH-001
                match fstatat_child(current.raw(), os.as_bytes()) {
                    Ok(st) if stat_is_symlink(&st) => {
                        return Err(io::Error::from_raw_os_error(libc::ELOOP));
                    }
                    Ok(_) => {}
                    Err(e) => return Err(e),
                }
                // O_DIRECTORY | O_NONBLOCK: even if a special file (FIFO, device) is
                // swapped in between the fstatat above and this openat, the open fails
                // immediately (ENOTDIR) instead of blocking the worker indefinitely.
                // SEC-P089-HIGH-001
                let fd = unsafe {
                    libc::openat(
                        current.raw(),
                        c_name.as_ptr(),
                        libc::O_RDONLY
                            | libc::O_NOFOLLOW
                            | libc::O_CLOEXEC
                            | libc::O_DIRECTORY
                            | libc::O_NONBLOCK,
                    )
                };
                if fd < 0 {
                    return Err(io::Error::last_os_error());
                }
                let next = OwnedFd::new(fd);
                let stat = fstat_fd(next.raw())?;
                if !stat_is_dir(&stat) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "path component is not a directory",
                    ));
                }
                current = next;
            }
            std::path::Component::ParentDir | std::path::Component::Prefix(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid scan root component",
                ));
            }
        }
    }

    Ok(current)
}

fn logical_child_path(parent: &Path, name: &[u8]) -> PathBuf {
    parent.join(OsStr::from_bytes(name))
}

fn terminal_scan_status(deadline: Instant, cancelled: &AtomicBool) -> Option<InventoryStatus> {
    if Instant::now() >= deadline {
        Some(InventoryStatus::Timeout)
    } else if cancelled.load(Ordering::Relaxed) || GLOBAL_SHUTDOWN_REQUESTED.load(Ordering::Relaxed)
    {
        Some(InventoryStatus::Cancelled)
    } else {
        None
    }
}

fn admitted_device_ids(root_kind: RootKind, root_dev: u64) -> HashSet<u64> {
    let mut devices = HashSet::from([root_dev]);
    if root_kind != RootKind::DiagnosticTestRoot {
        return devices;
    }

    for configured_root in std::env::var("CHAINWORKS_TEMP_ARTIFACT_DIAGNOSTIC_TEST_ROOTS")
        .unwrap_or_default()
        .split(':')
        .filter(|value| !value.is_empty())
    {
        let path = Path::new(configured_root);
        if path.is_absolute() {
            if let Ok(metadata) = std::fs::metadata(path) {
                devices.insert(metadata.dev());
            }
        }
    }
    devices
}

// ── Permit infrastructure ─────────────────────────────────────────────────────

static GLOBAL_SCAN_SEMAPHORE: OnceLock<Arc<Semaphore>> = OnceLock::new();
static CONTEXT_SEMAPHORES: OnceLock<Mutex<HashMap<String, Arc<Semaphore>>>> = OnceLock::new();

// Process-wide shutdown flag. Set by `request_global_shutdown()` when the daemon
// is shutting down. All in-progress scans check this on their next cancel-check
// interval and return `Cancelled` status (SEC-P089-007).
static GLOBAL_SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
static SCAN_EXECUTION_TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

#[cfg(test)]
fn scan_execution_test_lock() -> std::sync::MutexGuard<'static, ()> {
    SCAN_EXECUTION_TEST_MUTEX
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Signals all in-progress scans to stop at their next cancellation check interval.
/// Called by the daemon's shutdown handler on SIGTERM / SIGINT. Scans will return
/// `Cancelled` status and release their permits promptly.
pub fn request_global_shutdown() {
    GLOBAL_SHUTDOWN_REQUESTED.store(true, Ordering::Relaxed);
}

/// True once `request_global_shutdown()` has been called. Lets callers that only
/// observe a scan's terminal `Cancelled` status (not the per-request `cancelled`
/// flag) distinguish daemon shutdown from an explicit cancel/supersede/transport
/// close when labeling the bounded `terminal_status`/`source` metric.
pub fn is_global_shutdown_requested() -> bool {
    GLOBAL_SHUTDOWN_REQUESTED.load(Ordering::Relaxed)
}

/// Waits until every admitted scan has released its global permit. Callers must
/// wrap this future in the daemon's bounded shutdown deadline. Acquiring all
/// permits is observational only: the permits are dropped together before this
/// function returns, and new acquisitions are rejected once shutdown is set.
pub async fn drain_global_scan_permits() -> Result<(), tokio::sync::AcquireError> {
    let permits = global_scan_semaphore()
        .acquire_many_owned(SCAN_GLOBAL_PERMIT_MAX as u32)
        .await?;
    drop(permits);
    Ok(())
}

fn global_scan_semaphore() -> Arc<Semaphore> {
    GLOBAL_SCAN_SEMAPHORE
        .get_or_init(|| Arc::new(Semaphore::new(SCAN_GLOBAL_PERMIT_MAX)))
        .clone()
}

/// Looks up (or creates) the context's semaphore and acquires one owned permit
/// from it, all under a single held lock. Acquiring while holding the lock
/// (both operations are non-blocking/synchronous, never `.await`) closes a
/// race where a concurrent caller could `retain()`-evict and replace an
/// idle-looking entry after it was returned but before its first permit was
/// acquired, letting two distinct semaphores govern the same context and
/// defeat the per-context cap (SR-MEDIUM-003).
fn get_or_create_context_semaphore_and_acquire(
    context_key: &str,
) -> Result<OwnedSemaphorePermit, ScanPermitError> {
    let pool = CONTEXT_SEMAPHORES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = pool.lock().unwrap_or_else(|e| e.into_inner());
    // Evict idle entries (all permits returned) before inserting a new one.
    // This bounds the map size to at most the number of concurrent active scans,
    // preventing unbounded memory growth from distinct valid-UUID run_ids (SEC-P089-008).
    map.retain(|_, sem| sem.available_permits() < SCAN_CONTEXT_PERMIT_MAX);
    let sem = map
        .entry(context_key.to_string())
        .or_insert_with(|| Arc::new(Semaphore::new(SCAN_CONTEXT_PERMIT_MAX)))
        .clone();
    sem.try_acquire_owned()
        .map_err(|_| ScanPermitError::ResourceExhausted)
}

/// RAII permit guard: holds one context permit and one global permit.
/// Both permits are released when this value is dropped.
pub struct ScanPermitGuard {
    _context_permit: OwnedSemaphorePermit,
    _global_permit: OwnedSemaphorePermit,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScanPermitError {
    ResourceExhausted,
}

impl ScanPermitGuard {
    /// Acquires context permit then global permit (non-blocking).
    /// Returns `ResourceExhausted` immediately if either semaphore is full.
    /// Acquisition order (context → global) is consistent across all callers.
    pub fn try_acquire(context_key: &str) -> Result<Self, ScanPermitError> {
        if is_global_shutdown_requested() {
            return Err(ScanPermitError::ResourceExhausted);
        }
        let context_permit = get_or_create_context_semaphore_and_acquire(context_key)?;
        let global_permit = global_scan_semaphore()
            .try_acquire_owned()
            .map_err(|_| ScanPermitError::ResourceExhausted)?;
        Ok(Self {
            _context_permit: context_permit,
            _global_permit: global_permit,
        })
    }
}

// ── Scan types ────────────────────────────────────────────────────────────────

pub struct ScanRootTarget {
    pub path: PathBuf,
    pub root_kind: RootKind,
}

pub struct ScanRow {
    pub path_display: String,
    pub path_hash: String,
    pub path_hash_short: String,
    pub correlation_key: String,
    pub root_kind: RootKind,
    pub lifecycle_classification: LifecycleClassification,
    pub dry_run_recommendation: Option<DryRunRecommendation>,
    pub estimated_size_bytes: String,
    pub last_touched_at: Option<String>,
    pub status_token: String,
    pub generated_at: String,
    pub partial_errors: Vec<String>,
}

pub struct ScanError {
    pub code: InventoryErrorCode,
    pub message: String,
    pub root_kind: Option<RootKind>,
    pub phase: Option<&'static str>,
}

pub struct ScanResult {
    pub status: InventoryStatus,
    pub generated_at: String,
    pub rows: Vec<ScanRow>,
    pub errors: Vec<ScanError>,
    pub summary_estimated_bytes: u64,
    pub truncated: bool,
    /// True count of bounded-error events encountered during the scan, including
    /// ones dropped once `errors` hit `SCAN_PARTIAL_ERRORS_MAX_PER_PAYLOAD`.
    /// `summary.scan_error_count` uses this instead of `errors.len()` so the
    /// aggregate count survives the emitted-payload cap (SR-LOW-002).
    pub error_count_observed: usize,
}

// ── Scan execution ────────────────────────────────────────────────────────────

/// Synchronous (blocking) scan of a diagnostic test root.
/// Called via `tokio::task::spawn_blocking` from async handlers.
///
/// Implements:
/// - lstat/no-follow (symlink_metadata) semantics throughout
/// - symlinked directories: reported as bounded partial errors, not descended
/// - device-boundary enforcement: entries on a different device produce bounded errors
/// - cycle detection via device + inode identity set
/// - cooperative cancellation every 128 entries or 100 ms
/// - bounded partial errors (max 100 per payload)
/// - no deletion, cleanup, persistence, or mutation of any kind
#[cfg(test)]
pub fn scan_diagnostic_test_root(
    target: &ScanRootTarget,
    limit: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> ScanResult {
    let _guard = scan_execution_test_lock();
    scan_diagnostic_test_root_impl(target, limit, deadline, cancelled)
}

#[cfg(not(test))]
pub fn scan_diagnostic_test_root(
    target: &ScanRootTarget,
    limit: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> ScanResult {
    scan_diagnostic_test_root_impl(target, limit, deadline, cancelled)
}

fn scan_diagnostic_test_root_impl(
    target: &ScanRootTarget,
    limit: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> ScanResult {
    let generated_at = Utc::now().to_rfc3339();
    let mut rows: Vec<ScanRow> = Vec::new();
    let mut errors: Vec<ScanError> = Vec::new();
    let mut total_bytes: u64 = 0;
    let mut entry_count: usize = 0;
    let mut last_check = Instant::now();
    let mut seen_short_hashes: Vec<String> = Vec::new();
    let mut error_count_observed: usize = 0;

    // Get root metadata only to classify a symlinked final component for the
    // external error code. The authoritative traversal opens every path
    // component below through openat(O_NOFOLLOW).
    let root_meta = match std::fs::symlink_metadata(&target.path) {
        Ok(m) => m,
        Err(_) => {
            errors.push(ScanError {
                code: InventoryErrorCode::RootUnreadable,
                message: "<redacted>".to_string(),
                root_kind: Some(target.root_kind),
                phase: None,
            });
            return ScanResult {
                status: InventoryStatus::Error,
                generated_at,
                error_count_observed: errors.len(),
                rows,
                errors,
                summary_estimated_bytes: 0,
                truncated: false,
            };
        }
    };
    // Reject a symlinked scan root — containment checks must be on the canonical target.
    if root_meta.file_type().is_symlink() {
        errors.push(ScanError {
            code: InventoryErrorCode::InvalidRootOverride,
            message: "<redacted>".to_string(),
            root_kind: Some(target.root_kind),
            phase: None,
        });
        return ScanResult {
            status: InventoryStatus::Error,
            generated_at,
            error_count_observed: errors.len(),
            rows,
            errors,
            summary_estimated_bytes: 0,
            truncated: false,
        };
    }
    let root_fd = match open_absolute_dir_nofollow(&target.path) {
        Ok(fd) => fd,
        Err(err) => {
            let symlink_escape = err.raw_os_error() == Some(libc::ELOOP);
            errors.push(ScanError {
                code: if symlink_escape {
                    InventoryErrorCode::InvalidRootOverride
                } else {
                    InventoryErrorCode::RootUnreadable
                },
                message: "<redacted>".to_string(),
                root_kind: Some(target.root_kind),
                phase: None,
            });
            return ScanResult {
                status: if symlink_escape {
                    InventoryStatus::Error
                } else {
                    InventoryStatus::Partial
                },
                generated_at,
                error_count_observed: errors.len(),
                rows,
                errors,
                summary_estimated_bytes: 0,
                truncated: false,
            };
        }
    };
    let root_stat = match fstat_fd(root_fd.raw()) {
        Ok(stat) => stat,
        Err(_) => {
            errors.push(ScanError {
                code: InventoryErrorCode::RootUnreadable,
                message: "<redacted>".to_string(),
                root_kind: Some(target.root_kind),
                phase: None,
            });
            return ScanResult {
                status: InventoryStatus::Partial,
                generated_at,
                error_count_observed: errors.len(),
                rows,
                errors,
                summary_estimated_bytes: 0,
                truncated: false,
            };
        }
    };

    let root_dev = stat_dev(&root_stat);
    let allowed_device_ids = admitted_device_ids(target.root_kind, root_dev);
    let mut visited_dirs: HashSet<(u64, u64)> =
        HashSet::from([(stat_dev(&root_stat), stat_ino(&root_stat))]);
    let mut reported_duplicate_dirs: HashSet<(u64, u64)> = HashSet::new();

    if limit == 0 {
        return ScanResult {
            status: InventoryStatus::Complete,
            generated_at,
            error_count_observed: errors.len(),
            rows,
            errors,
            summary_estimated_bytes: 0,
            truncated: false,
        };
    }

    let mut root_stream = match DirStream::from_fd(root_fd) {
        Ok(stream) => stream,
        Err(_) => {
            errors.push(ScanError {
                code: InventoryErrorCode::RootUnreadable,
                message: "<redacted>".to_string(),
                root_kind: Some(target.root_kind),
                phase: None,
            });
            return ScanResult {
                status: InventoryStatus::Partial,
                generated_at,
                error_count_observed: errors.len(),
                rows,
                errors,
                summary_estimated_bytes: 0,
                truncated: false,
            };
        }
    };

    let mut status = InventoryStatus::Complete;
    let now_ts = Utc::now().to_rfc3339();

    loop {
        if rows.len() >= limit {
            break;
        }
        if let Some(terminal) = terminal_scan_status(deadline, cancelled) {
            status = terminal;
            add_terminal_error(
                &mut errors,
                terminal,
                "enumeration",
                Some(target.root_kind),
                &mut error_count_observed,
            );
            break;
        }

        let Some(name) = root_stream.next_name() else {
            break;
        };

        entry_count += 1;
        if entry_count > SCAN_MAX_TOTAL_ENTRIES {
            add_bounded_error(
                &mut errors,
                InventoryErrorCode::SizeEstimationFailed,
                Some(target.root_kind),
                &mut status,
                &mut error_count_observed,
            );
            break;
        }
        if entry_count % SCAN_CANCEL_CHECK_INTERVAL_ENTRIES == 0
            || last_check.elapsed() >= Duration::from_millis(SCAN_CANCEL_CHECK_INTERVAL_MS)
        {
            last_check = Instant::now();
            if let Some(terminal) = terminal_scan_status(deadline, cancelled) {
                status = terminal;
                add_terminal_error(
                    &mut errors,
                    terminal,
                    "enumeration",
                    Some(target.root_kind),
                    &mut error_count_observed,
                );
                break;
            }
        }

        let path = logical_child_path(&target.path, &name);
        let mut stat = match fstatat_child(root_stream.fd(), &name) {
            Ok(stat) => stat,
            Err(_) => {
                add_bounded_error(
                    &mut errors,
                    InventoryErrorCode::InternalError,
                    Some(target.root_kind),
                    &mut status,
                    &mut error_count_observed,
                );
                continue;
            }
        };

        if stat_is_symlink(&stat) {
            // traversal_policy: symlinked directories are bounded partial errors and
            // are never descended; symlinked files are link metadata only rows, and
            // never have their target content read. The follow-stat below is a single
            // metadata-only syscall used solely to distinguish the two cases — it never
            // opens the target and a directory target is still never descended.
            let target_is_dir = fstatat_child_follow(root_stream.fd(), &name)
                .map(|target_stat| stat_is_dir(&target_stat))
                .unwrap_or(false);

            if target_is_dir {
                add_bounded_error(
                    &mut errors,
                    InventoryErrorCode::InternalError,
                    Some(target.root_kind),
                    &mut status,
                    &mut error_count_observed,
                );
                continue;
            }

            // Symlinked file (or dangling/non-directory link): emit a row from the
            // symlink's own lstat metadata only — its target content is never read.
            total_bytes = total_bytes.saturating_add(stat_size(&stat));
            let full_hash = compute_path_hash(path.as_os_str().as_bytes(), target.root_kind);
            let existing_refs: Vec<&str> = seen_short_hashes.iter().map(|s| s.as_str()).collect();
            let short_hash = compute_path_hash_short(&full_hash, &existing_refs);
            seen_short_hashes.push(short_hash.clone());
            let last_touched_at = stat_mtime(&stat);
            let (lifecycle_classification, dry_run_recommendation) =
                classify_row(last_touched_at.as_deref(), target.root_kind);
            rows.push(ScanRow {
                path_display: format!("<redacted:{}>", short_hash),
                path_hash: full_hash.clone(),
                path_hash_short: short_hash,
                correlation_key: full_hash,
                root_kind: target.root_kind,
                lifecycle_classification,
                dry_run_recommendation: Some(dry_run_recommendation),
                estimated_size_bytes: stat_size(&stat).to_string(),
                last_touched_at,
                status_token: InventoryStatus::Complete.as_str().to_string(),
                generated_at: now_ts.clone(),
                partial_errors: Vec::new(),
            });
            continue;
        }

        let mut child_dir_fd: Option<OwnedFd> = None;
        if stat_is_dir(&stat) {
            let fd = match open_child_dir_at(root_stream.fd(), &name) {
                Ok(fd) => fd,
                Err(_) => {
                    add_bounded_error(
                        &mut errors,
                        InventoryErrorCode::InternalError,
                        Some(target.root_kind),
                        &mut status,
                        &mut error_count_observed,
                    );
                    continue;
                }
            };
            stat = match fstat_fd(fd.raw()) {
                Ok(opened_stat) => opened_stat,
                Err(_) => {
                    add_bounded_error(
                        &mut errors,
                        InventoryErrorCode::InternalError,
                        Some(target.root_kind),
                        &mut status,
                        &mut error_count_observed,
                    );
                    continue;
                }
            };
            child_dir_fd = Some(fd);
        }

        let dev = stat_dev(&stat);
        if !allowed_device_ids.contains(&dev) {
            add_bounded_error(
                &mut errors,
                InventoryErrorCode::InternalError,
                Some(target.root_kind),
                &mut status,
                &mut error_count_observed,
            );
            continue;
        }

        let ino = stat_ino(&stat);
        if stat_is_dir(&stat) {
            let id = (dev, ino);
            if visited_dirs.len() >= SCAN_MAX_VISITED_DIRS {
                add_bounded_error(
                    &mut errors,
                    InventoryErrorCode::InternalError,
                    Some(target.root_kind),
                    &mut status,
                    &mut error_count_observed,
                );
                continue;
            }
            if !visited_dirs.insert(id) {
                if reported_duplicate_dirs.insert(id) {
                    add_bounded_error(
                        &mut errors,
                        InventoryErrorCode::InternalError,
                        Some(target.root_kind),
                        &mut status,
                        &mut error_count_observed,
                    );
                }
                continue;
            }
        }

        let row_error_start = errors.len();
        let size_bytes = if let Some(fd) = child_dir_fd {
            estimate_dir_size(
                fd,
                path.clone(),
                &allowed_device_ids,
                deadline,
                cancelled,
                &mut entry_count,
                &mut last_check,
                &mut visited_dirs,
                &mut reported_duplicate_dirs,
                &mut errors,
                &mut status,
                &mut error_count_observed,
                target.root_kind,
            )
        } else {
            stat_size(&stat)
        };

        if let Some(terminal) = terminal_scan_status(deadline, cancelled) {
            status = terminal;
            add_terminal_error(
                &mut errors,
                terminal,
                "size_estimation",
                Some(target.root_kind),
                &mut error_count_observed,
            );
            break;
        }

        total_bytes = total_bytes.saturating_add(size_bytes);

        // Compute path hash (HMAC-SHA256, process-scoped key) from the path's raw
        // OS bytes, not a lossy UTF-8 conversion, so distinct non-UTF-8 paths never
        // collapse onto the same correlation identity (SR-LOW-001).
        use std::os::unix::ffi::OsStrExt;
        let full_hash = compute_path_hash(path.as_os_str().as_bytes(), target.root_kind);
        let existing_refs: Vec<&str> = seen_short_hashes.iter().map(|s| s.as_str()).collect();
        let short_hash = compute_path_hash_short(&full_hash, &existing_refs);
        seen_short_hashes.push(short_hash.clone());

        let last_touched_at = stat_mtime(&stat);
        let (mut lifecycle_classification, mut dry_run_recommendation) =
            classify_row(last_touched_at.as_deref(), target.root_kind);
        let partial_errors: Vec<String> = errors[row_error_start..]
            .iter()
            .take(SCAN_PARTIAL_ERRORS_MAX_PER_ROW)
            .map(|error| error.code.as_str().to_string())
            .collect();
        if !partial_errors.is_empty() {
            lifecycle_classification = LifecycleClassification::ScanError;
            dry_run_recommendation = DryRunRecommendation::NeedsOperatorReview;
        }

        rows.push(ScanRow {
            // Use the full collision-resolved short hash (12-20 chars), not a fixed
            // 8-char prefix — truncating further than compute_path_hash_short already
            // did would throw away the collision-extension it computed and let two
            // distinct artifact trees render as the same operator-visible string.
            path_display: format!("<redacted:{}>", short_hash),
            path_hash: full_hash.clone(),
            path_hash_short: short_hash,
            // Proposal: use path_hash when present as the stable row correlation key.
            correlation_key: full_hash,
            root_kind: target.root_kind,
            lifecycle_classification,
            dry_run_recommendation: Some(dry_run_recommendation),
            estimated_size_bytes: size_bytes.to_string(),
            last_touched_at,
            status_token: if partial_errors.is_empty() {
                InventoryStatus::Complete
            } else {
                InventoryStatus::Partial
            }
            .as_str()
            .to_string(),
            generated_at: now_ts.clone(),
            partial_errors,
        });
    }

    let truncated = status == InventoryStatus::Complete && rows.len() >= limit;

    ScanResult {
        status,
        generated_at,
        rows,
        errors,
        summary_estimated_bytes: total_bytes,
        truncated,
        error_count_observed,
    }
}

// ── Advisory lifecycle classification (read-only, dry-run only) ──────────────
//
// Thresholds are deliberately coarse and age-based only: this slice has no
// process liveness signal, manifest state, or owner inference, so classification
// stays advisory and defaults to the most conservative (keep/review) bucket
// whenever evidence is ambiguous. No mutation of any kind is implied or possible.
const AGE_ACTIVE_SECS: i64 = 60 * 60; // 1 hour
const AGE_RECENT_SECS: i64 = 24 * 60 * 60; // 24 hours
const AGE_TERMINAL_SECS: i64 = 7 * 24 * 60 * 60; // 7 days

/// Classifies a scanned entry from its last-touched age and root kind.
/// Legacy roots always classify as `legacy_unmanaged` regardless of age, since
/// migration (not age) is the operative future action for that root kind.
fn classify_row(
    last_touched_at: Option<&str>,
    root_kind: RootKind,
) -> (LifecycleClassification, DryRunRecommendation) {
    if root_kind == RootKind::LegacyChainworksTmp {
        return (
            LifecycleClassification::LegacyUnmanaged,
            DryRunRecommendation::WouldMigrateLegacyManifestAfterFutureMigrationEnabled,
        );
    }
    let age_secs = last_touched_at
        .and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok())
        .map(|dt| (Utc::now() - dt.with_timezone(&Utc)).num_seconds());
    match age_secs {
        None => (
            LifecycleClassification::Unknown,
            DryRunRecommendation::NeedsOperatorReview,
        ),
        Some(secs) if secs < AGE_ACTIVE_SECS => (
            LifecycleClassification::ActiveOrRecent,
            DryRunRecommendation::WouldKeepActive,
        ),
        Some(secs) if secs < AGE_RECENT_SECS => (
            LifecycleClassification::ActiveOrRecent,
            DryRunRecommendation::WouldKeepRecent,
        ),
        Some(secs) if secs < AGE_TERMINAL_SECS => (
            LifecycleClassification::TerminalCandidate,
            DryRunRecommendation::NeedsOperatorReview,
        ),
        Some(_) => (
            LifecycleClassification::OrphanCandidate,
            DryRunRecommendation::WouldDeleteAfterFutureApproval,
        ),
    }
}

/// Iterative directory size estimation with descriptor-relative no-follow traversal.
///
/// Every directory in the traversal stack is already an open fd obtained through
/// `openat(O_NOFOLLOW)` from its parent fd. Children are enumerated via `readdir`
/// on that descriptor and opened/stat'ed with `openat`/`fstatat`, so mutable
/// path ancestors are not re-resolved during recursion (SEC-P089-HIGH-001).
///
/// The explicit stack holds at most one open `DirStream` per depth level of the
/// *currently active* path (ancestors of the entry being enumerated), never one
/// per sibling. A child directory's fd is opened only when it becomes the new
/// stack top, and it is closed (via `DirStream`'s `Drop`) as soon as its own
/// enumeration finishes and it is popped. This bounds simultaneously-open
/// descriptors to O(depth) instead of O(breadth), so a directory with many
/// entries at one level cannot exhaust `RLIMIT_NOFILE` (SR-HIGH-001).
///
/// Emits bounded partial errors for unreadable entries, symlinks, and cross-device entries
/// so aggregate scan_error_count is accurate (proposal bounded partial-error requirement).
///
/// Does not follow symlinks or cross device boundaries. Does not descend into directories
/// already in `visited_dirs`.
fn estimate_dir_size(
    root_fd: OwnedFd,
    root_path: PathBuf,
    allowed_device_ids: &HashSet<u64>,
    deadline: Instant,
    cancelled: &AtomicBool,
    entry_count: &mut usize,
    last_check: &mut Instant,
    visited_dirs: &mut HashSet<(u64, u64)>,
    reported_duplicate_dirs: &mut HashSet<(u64, u64)>,
    errors: &mut Vec<ScanError>,
    status: &mut InventoryStatus,
    error_count_observed: &mut usize,
    root_kind: RootKind,
) -> u64 {
    let mut total: u64 = 0;

    let root_stream = match DirStream::from_fd(root_fd) {
        Ok(stream) => stream,
        Err(_) => {
            add_bounded_error(
                errors,
                InventoryErrorCode::InternalError,
                Some(root_kind),
                status,
                error_count_observed,
            );
            return total;
        }
    };
    // Stack of (open stream, its logical path) for the current path from root to
    // the entry currently being enumerated. Only ancestors are ever open at once.
    let mut stack: Vec<(DirStream, PathBuf)> = vec![(root_stream, root_path)];

    while let Some((mut stream, current_path)) = stack.pop() {
        if terminal_scan_status(deadline, cancelled).is_some() {
            return total;
        }

        let Some(name) = stream.next_name() else {
            // This directory is exhausted; its fd closes here via Drop. Resume
            // the parent, which is now the new stack top.
            continue;
        };

        *entry_count += 1;
        if *entry_count > SCAN_MAX_TOTAL_ENTRIES {
            add_bounded_error(
                errors,
                InventoryErrorCode::SizeEstimationFailed,
                Some(root_kind),
                status,
                error_count_observed,
            );
            return total;
        }
        if *entry_count % SCAN_CANCEL_CHECK_INTERVAL_ENTRIES == 0
            || last_check.elapsed() >= Duration::from_millis(SCAN_CANCEL_CHECK_INTERVAL_MS)
        {
            *last_check = Instant::now();
            if terminal_scan_status(deadline, cancelled).is_some() {
                return total;
            }
        }

        let mut stat = match fstatat_child(stream.fd(), &name) {
            Ok(stat) => stat,
            Err(_) => {
                add_bounded_error(
                    errors,
                    InventoryErrorCode::InternalError,
                    Some(root_kind),
                    status,
                    error_count_observed,
                );
                stack.push((stream, current_path));
                continue;
            }
        };

        if stat_is_symlink(&stat) {
            add_bounded_error(
                errors,
                InventoryErrorCode::InternalError,
                Some(root_kind),
                status,
                error_count_observed,
            );
            stack.push((stream, current_path));
            continue;
        }

        let mut child_dir_fd: Option<OwnedFd> = None;
        if stat_is_dir(&stat) {
            let fd = match open_child_dir_at(stream.fd(), &name) {
                Ok(fd) => fd,
                Err(_) => {
                    add_bounded_error(
                        errors,
                        InventoryErrorCode::InternalError,
                        Some(root_kind),
                        status,
                        error_count_observed,
                    );
                    stack.push((stream, current_path));
                    continue;
                }
            };
            stat = match fstat_fd(fd.raw()) {
                Ok(opened_stat) => opened_stat,
                Err(_) => {
                    add_bounded_error(
                        errors,
                        InventoryErrorCode::InternalError,
                        Some(root_kind),
                        status,
                        error_count_observed,
                    );
                    stack.push((stream, current_path));
                    continue;
                }
            };
            child_dir_fd = Some(fd);
        }

        if !allowed_device_ids.contains(&stat_dev(&stat)) {
            add_bounded_error(
                errors,
                InventoryErrorCode::InternalError,
                Some(root_kind),
                status,
                error_count_observed,
            );
            stack.push((stream, current_path));
            continue;
        }

        if let Some(fd) = child_dir_fd {
            let id = (stat_dev(&stat), stat_ino(&stat));
            let child_path = logical_child_path(&current_path, &name);
            // Bound simultaneous ancestor depth (one open fd per stack level) and
            // cumulative logical path bytes before descending further, independent of
            // the distinct-directory-identity cap below (SR-MEDIUM-002): a long
            // singly-nested chain visits distinct identities but would otherwise grow
            // descriptor and PathBuf usage without limit. `fd` is dropped here (closed)
            // rather than descended into.
            if stack.len() >= SCAN_MAX_DIR_DEPTH
                || child_path.as_os_str().len() > SCAN_MAX_PATH_BYTES
            {
                add_bounded_error(
                    errors,
                    InventoryErrorCode::SizeEstimationFailed,
                    Some(root_kind),
                    status,
                    error_count_observed,
                );
                stack.push((stream, current_path));
            } else if visited_dirs.len() >= SCAN_MAX_VISITED_DIRS {
                add_bounded_error(
                    errors,
                    InventoryErrorCode::InternalError,
                    Some(root_kind),
                    status,
                    error_count_observed,
                );
                stack.push((stream, current_path));
            } else if visited_dirs.insert(id) {
                match DirStream::from_fd(fd) {
                    Ok(child_stream) => {
                        // Resume the parent after the child is fully drained.
                        stack.push((stream, current_path));
                        stack.push((child_stream, child_path));
                    }
                    Err(_) => {
                        add_bounded_error(
                            errors,
                            InventoryErrorCode::InternalError,
                            Some(root_kind),
                            status,
                            error_count_observed,
                        );
                        stack.push((stream, current_path));
                    }
                }
            } else {
                if reported_duplicate_dirs.insert(id) {
                    add_bounded_error(
                        errors,
                        InventoryErrorCode::InternalError,
                        Some(root_kind),
                        status,
                        error_count_observed,
                    );
                }
                stack.push((stream, current_path));
            }
        } else {
            total = total.saturating_add(stat_size(&stat));
            stack.push((stream, current_path));
        }
    }

    total
}

/// Adds a bounded partial error. Caps the emitted `errors` vec at
/// `SCAN_PARTIAL_ERRORS_MAX_PER_PAYLOAD`, but always increments
/// `observed_count` so the true aggregate error count survives the cap
/// (SR-LOW-002). Only downgrades status from Complete to Partial; never
/// upgrades from Error.
fn add_bounded_error(
    errors: &mut Vec<ScanError>,
    code: InventoryErrorCode,
    root_kind: Option<RootKind>,
    status: &mut InventoryStatus,
    observed_count: &mut usize,
) {
    *observed_count += 1;
    if errors.len() < SCAN_PARTIAL_ERRORS_MAX_PER_PAYLOAD {
        errors.push(ScanError {
            code,
            message: "<redacted>".to_string(),
            root_kind,
            phase: None,
        });
    }
    if *status == InventoryStatus::Complete {
        *status = InventoryStatus::Partial;
    }
}

fn add_terminal_error(
    errors: &mut Vec<ScanError>,
    status: InventoryStatus,
    phase: &'static str,
    root_kind: Option<RootKind>,
    observed_count: &mut usize,
) {
    let code = match status {
        InventoryStatus::Cancelled => InventoryErrorCode::Cancelled,
        InventoryStatus::Timeout => InventoryErrorCode::DeadlineExceeded,
        _ => return,
    };
    *observed_count += 1;
    if errors.len() < SCAN_PARTIAL_ERRORS_MAX_PER_PAYLOAD {
        errors.push(ScanError {
            code,
            message: "<redacted>".to_string(),
            root_kind,
            phase: Some(phase),
        });
    }
}

fn artifact_kind_for_root(root_kind: RootKind) -> &'static str {
    match root_kind {
        RootKind::RunMetaRoot => "run_output",
        RootKind::ControlPlaneCache => "control_plane_cache",
        RootKind::ProviderHomeCopy => "provider_home_copy",
        RootKind::LegacyChainworksTmp => "legacy_unmanaged",
        RootKind::DiagnosticTestRoot => "diagnostic_fixture",
        RootKind::Unknown => "unknown",
    }
}

/// Assembles the scan result into the canonical DTO JSON value.
#[allow(clippy::too_many_arguments)]
pub fn assemble_scan_dto(
    result: ScanResult,
    include_dry_run: bool,
    limit: i32,
    timeout_ms: i32,
    queue_wait_ms: u64,
    scan_deadline_at: Option<String>,
    mode: domain::temp_artifact_inventory::InventoryMode,
) -> serde_json::Value {
    use domain::temp_artifact_inventory::{EnabledState, MutationGuardStatus};
    let now = Utc::now().to_rfc3339();

    let rows: Vec<serde_json::Value> = result
        .rows
        .iter()
        .map(|r| {
            let mut v = serde_json::json!({
                "path_display": r.path_display,
                "path_hash": r.path_hash,
                "path_hash_short": r.path_hash_short,
                "correlation_key": r.correlation_key,
                "root_kind": r.root_kind.as_str(),
                "artifact_kind": artifact_kind_for_root(r.root_kind),
                "manifest_state": "unknown",
                "lifecycle_classification": r.lifecycle_classification.as_str(),
                "estimated_size_bytes": r.estimated_size_bytes,
                "active_process_evidence": null,
                "owner": null,
                "owner_inference": null,
                "status_token": r.status_token,
                "generated_at": r.generated_at,
                "partial_errors": r.partial_errors
            });
            if include_dry_run {
                match &r.dry_run_recommendation {
                    Some(rec) => {
                        v["dry_run_recommendation"] =
                            serde_json::Value::String(rec.as_str().to_string());
                    }
                    None => {
                        v["dry_run_recommendation"] = serde_json::Value::Null;
                    }
                }
            } else {
                v["dry_run_recommendation"] = serde_json::Value::Null;
            }
            if let Some(ts) = &r.last_touched_at {
                v["last_touched_at"] = serde_json::Value::String(ts.clone());
            } else {
                v["last_touched_at"] = serde_json::Value::Null;
            }
            v
        })
        .collect();

    let errors: Vec<serde_json::Value> = result
        .errors
        .iter()
        .map(|e| {
            let mut v = serde_json::json!({
                "code": e.code.as_str(),
                "message": e.message,
            });
            if let Some(rk) = e.root_kind {
                v["root_kind"] = serde_json::Value::String(rk.as_str().to_string());
            } else {
                v["root_kind"] = serde_json::Value::Null;
            }
            if let Some(phase) = e.phase {
                v["phase"] = serde_json::Value::String(phase.to_string());
            } else {
                v["phase"] = serde_json::Value::Null;
            }
            v
        })
        .collect();

    let row_count = rows.len();

    // Classification and recommendation summary counters, computed from the same
    // per-row classification the rows array already carries (single source of truth).
    let mut active_or_recent_count: i64 = 0;
    let mut terminal_candidate_count: i64 = 0;
    let mut orphan_candidate_count: i64 = 0;
    let mut legacy_unmanaged_count: i64 = 0;
    let mut dry_run_candidate_count: i64 = 0;
    let mut recommendation_counts = serde_json::Map::new();
    for r in &result.rows {
        match r.lifecycle_classification {
            LifecycleClassification::ActiveOrRecent => active_or_recent_count += 1,
            LifecycleClassification::TerminalCandidate => terminal_candidate_count += 1,
            LifecycleClassification::OrphanCandidate => orphan_candidate_count += 1,
            LifecycleClassification::LegacyUnmanaged => legacy_unmanaged_count += 1,
            LifecycleClassification::ScanError | LifecycleClassification::Unknown => {}
        }
        if include_dry_run {
            if let Some(rec) = r.dry_run_recommendation {
                let key = rec.as_str();
                let count = recommendation_counts
                    .entry(key)
                    .or_insert(serde_json::Value::from(0));
                *count = serde_json::Value::from(count.as_i64().unwrap_or(0) + 1);
                if !matches!(
                    rec,
                    DryRunRecommendation::WouldKeepActive
                        | DryRunRecommendation::WouldKeepRecent
                        | DryRunRecommendation::NoRecommendation
                        | DryRunRecommendation::Unknown
                ) {
                    dry_run_candidate_count += 1;
                }
            }
        }
    }

    let dry_run = if include_dry_run {
        serde_json::json!({
            "schema_version": "temp_artifact_dry_run_v1",
            "generated_at": result.generated_at,
            "recommendation_counts": recommendation_counts,
            "mutation_guard": {
                "status": MutationGuardStatus::Pass.as_str(),
                "checked_at": now
            }
        })
    } else {
        serde_json::Value::Null
    };

    serde_json::json!({
        "schema_version": TEMP_ARTIFACT_INVENTORY_SCHEMA_VERSION,
        "status": result.status.as_str(),
        "enabled_state": EnabledState::Enabled.as_str(),
        "mode": mode.as_str(),
        "disabled_reason_code": null,
        "generated_at": result.generated_at,
        "limits_applied": {
            "limit": limit,
            "timeout_ms": timeout_ms,
            "scan_deadline_at": scan_deadline_at,
            "queue_wait_ms": queue_wait_ms
        },
        "summary": {
            "artifact_tree_count": row_count,
            "estimated_bytes": result.summary_estimated_bytes.to_string(),
            "active_or_recent_count": active_or_recent_count,
            "terminal_candidate_count": terminal_candidate_count,
            "orphan_candidate_count": orphan_candidate_count,
            "legacy_unmanaged_count": legacy_unmanaged_count,
            "scan_error_count": result.error_count_observed.max(errors.len()),
            "dry_run_candidate_count": dry_run_candidate_count,
            "truncated": result.truncated,
            "queue_wait_ms": queue_wait_ms
        },
        "rows": rows,
        "errors": errors,
        "dry_run": dry_run,
        "mutation_guard": {
            "status": MutationGuardStatus::Pass.as_str(),
            "checked_at": now,
            "no_delete": true,
            "no_prune": true,
            "no_chmod": true,
            "no_persist": true,
            "no_retry": true
        }
    })
}

// ── Multi-root scan ───────────────────────────────────────────────────────────

/// Scans multiple roots sequentially and aggregates results into a single ScanResult.
/// Stops early if the row limit is reached, the deadline passes, or cancellation is requested.
#[cfg(test)]
pub fn scan_multi_root(
    targets: &[ScanRootTarget],
    limit: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> ScanResult {
    let _guard = scan_execution_test_lock();
    scan_multi_root_impl(targets, limit, deadline, cancelled)
}

#[cfg(not(test))]
pub fn scan_multi_root(
    targets: &[ScanRootTarget],
    limit: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> ScanResult {
    scan_multi_root_impl(targets, limit, deadline, cancelled)
}

fn scan_multi_root_impl(
    targets: &[ScanRootTarget],
    limit: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> ScanResult {
    let generated_at = Utc::now().to_rfc3339();
    let mut all_rows: Vec<ScanRow> = Vec::new();
    let mut all_errors: Vec<ScanError> = Vec::new();
    let mut total_bytes: u64 = 0;
    let mut worst_status = InventoryStatus::Complete;
    // Aggregate count of bounded-error events across every root, independent of
    // the payload-wide `all_errors` cap below (SR-LOW-002).
    let mut total_error_count_observed: usize = 0;

    for target in targets {
        if Instant::now() >= deadline {
            worst_status = InventoryStatus::Timeout;
            add_terminal_error(
                &mut all_errors,
                worst_status,
                "enumeration",
                None,
                &mut total_error_count_observed,
            );
            break;
        }
        if cancelled.load(Ordering::Relaxed) || GLOBAL_SHUTDOWN_REQUESTED.load(Ordering::Relaxed) {
            worst_status = InventoryStatus::Cancelled;
            add_terminal_error(
                &mut all_errors,
                worst_status,
                "enumeration",
                None,
                &mut total_error_count_observed,
            );
            break;
        }

        let remaining_limit = limit.saturating_sub(all_rows.len());
        if remaining_limit == 0 {
            break;
        }

        let sub = scan_diagnostic_test_root_impl(target, remaining_limit, deadline, cancelled);
        total_bytes = total_bytes.saturating_add(sub.summary_estimated_bytes);
        total_error_count_observed += sub.error_count_observed;
        all_rows.extend(sub.rows);
        // Each root independently caps at SCAN_PARTIAL_ERRORS_MAX_PER_PAYLOAD, but
        // concatenating those per-root caps across N roots can still exceed the
        // payload-wide cap. Enforce one payload-wide cap here too (SR-LOW-002).
        if all_errors.len() < SCAN_PARTIAL_ERRORS_MAX_PER_PAYLOAD {
            let remaining_error_capacity = SCAN_PARTIAL_ERRORS_MAX_PER_PAYLOAD - all_errors.len();
            all_errors.extend(sub.errors.into_iter().take(remaining_error_capacity));
        }

        // Track worst terminal status (Error > Timeout > Cancelled > Partial > Complete)
        match sub.status {
            InventoryStatus::Error => {
                worst_status = InventoryStatus::Error;
            }
            InventoryStatus::Timeout if worst_status != InventoryStatus::Error => {
                worst_status = InventoryStatus::Timeout;
            }
            InventoryStatus::Cancelled
                if !matches!(
                    worst_status,
                    InventoryStatus::Error | InventoryStatus::Timeout
                ) =>
            {
                worst_status = InventoryStatus::Cancelled;
            }
            InventoryStatus::Partial
                if !matches!(
                    worst_status,
                    InventoryStatus::Error | InventoryStatus::Timeout | InventoryStatus::Cancelled
                ) =>
            {
                worst_status = InventoryStatus::Partial;
            }
            _ => {}
        }
    }

    let truncated = all_rows.len() >= limit && worst_status == InventoryStatus::Complete;

    ScanResult {
        status: worst_status,
        generated_at,
        rows: all_rows,
        errors: all_errors,
        summary_estimated_bytes: total_bytes,
        truncated,
        error_count_observed: total_error_count_observed,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::sync::{Mutex, OnceLock};
    use tempfile::TempDir;

    // Serializes tests that set or are sensitive to GLOBAL_SHUTDOWN_REQUESTED.
    // Any test that calls request_global_shutdown() or stores into the flag must
    // hold this lock for its entire duration, including the reset store. Any test
    // that may be affected by the flag being true must also hold this lock.
    static SHUTDOWN_FLAG_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
    fn shutdown_flag_lock() -> std::sync::MutexGuard<'static, ()> {
        SHUTDOWN_FLAG_MUTEX
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    // ── classify_row ──────────────────────────────────────────────────────

    #[test]
    fn p089_classify_row_legacy_root_is_always_legacy_unmanaged() {
        let (classification, recommendation) = classify_row(None, RootKind::LegacyChainworksTmp);
        assert_eq!(classification, LifecycleClassification::LegacyUnmanaged);
        assert_eq!(
            recommendation,
            DryRunRecommendation::WouldMigrateLegacyManifestAfterFutureMigrationEnabled
        );
    }

    #[test]
    fn p089_classify_row_missing_timestamp_is_unknown() {
        let (classification, recommendation) = classify_row(None, RootKind::RunMetaRoot);
        assert_eq!(classification, LifecycleClassification::Unknown);
        assert_eq!(recommendation, DryRunRecommendation::NeedsOperatorReview);
    }

    #[test]
    fn p089_classify_row_recent_timestamp_is_active_or_recent_keep_active() {
        let ts = Utc::now().to_rfc3339();
        let (classification, recommendation) = classify_row(Some(&ts), RootKind::RunMetaRoot);
        assert_eq!(classification, LifecycleClassification::ActiveOrRecent);
        assert_eq!(recommendation, DryRunRecommendation::WouldKeepActive);
    }

    #[test]
    fn p089_classify_row_hours_old_is_active_or_recent_keep_recent() {
        let ts = (Utc::now() - chrono::Duration::hours(2)).to_rfc3339();
        let (classification, recommendation) = classify_row(Some(&ts), RootKind::RunMetaRoot);
        assert_eq!(classification, LifecycleClassification::ActiveOrRecent);
        assert_eq!(recommendation, DryRunRecommendation::WouldKeepRecent);
    }

    #[test]
    fn p089_classify_row_days_old_is_terminal_candidate() {
        let ts = (Utc::now() - chrono::Duration::days(2)).to_rfc3339();
        let (classification, recommendation) = classify_row(Some(&ts), RootKind::RunMetaRoot);
        assert_eq!(classification, LifecycleClassification::TerminalCandidate);
        assert_eq!(recommendation, DryRunRecommendation::NeedsOperatorReview);
    }

    #[test]
    fn p089_classify_row_weeks_old_is_orphan_candidate() {
        let ts = (Utc::now() - chrono::Duration::days(30)).to_rfc3339();
        let (classification, recommendation) = classify_row(Some(&ts), RootKind::RunMetaRoot);
        assert_eq!(classification, LifecycleClassification::OrphanCandidate);
        assert_eq!(
            recommendation,
            DryRunRecommendation::WouldDeleteAfterFutureApproval
        );
    }

    #[test]
    fn p089_hidden_readback_row_carries_real_classification_not_unknown() {
        let _guard = scan_execution_test_lock();
        let dir = TempDir::new().expect("temp dir");
        std::fs::write(dir.path().join("fresh.txt"), b"data").expect("write");
        let target = make_target(dir.path().to_path_buf());
        let result = scan_diagnostic_test_root_impl(&target, 10, far_deadline(), &not_cancelled());
        assert_eq!(result.rows.len(), 1);
        let row = &result.rows[0];
        assert_ne!(
            row.lifecycle_classification,
            LifecycleClassification::Unknown
        );
        assert_eq!(row.status_token, "complete");
        assert!(row.dry_run_recommendation.is_some());
        assert_ne!(
            row.dry_run_recommendation,
            Some(DryRunRecommendation::NoRecommendation)
        );
    }

    #[test]
    fn p089_assemble_scan_dto_summary_counts_reflect_row_classifications() {
        let _guard = scan_execution_test_lock();
        let dir = TempDir::new().expect("temp dir");
        std::fs::write(dir.path().join("fresh.txt"), b"data").expect("write");
        let target = make_target(dir.path().to_path_buf());
        let result = scan_diagnostic_test_root_impl(&target, 10, far_deadline(), &not_cancelled());
        let dto = assemble_scan_dto(
            result,
            true,
            500,
            5000,
            0,
            None,
            domain::temp_artifact_inventory::InventoryMode::HiddenReadback,
        );
        assert_eq!(dto["summary"]["active_or_recent_count"], 1);
        assert_eq!(dto["summary"]["dry_run_candidate_count"], 0);
        let row = &dto["rows"][0];
        assert_eq!(row["artifact_kind"], "diagnostic_fixture");
        assert_eq!(row["manifest_state"], "unknown");
        assert!(row["active_process_evidence"].is_null());
        assert!(row["owner"].is_null());
        assert!(row["owner_inference"].is_null());
        let counts = &dto["dry_run"]["recommendation_counts"];
        assert_eq!(counts["would_keep_active"], 1);
    }

    #[test]
    fn p089_nested_entry_error_is_bounded_on_its_row() {
        use std::os::unix::fs::symlink;

        let dir = TempDir::new().expect("temp dir");
        let outside = TempDir::new().expect("outside dir");
        let tree = dir.path().join("tree");
        std::fs::create_dir(&tree).expect("tree");
        symlink(outside.path(), tree.join("escape")).expect("symlink");

        let target = make_target(dir.path().to_path_buf());
        let result = scan_diagnostic_test_root(&target, 10, far_deadline(), &not_cancelled());

        assert_eq!(result.rows.len(), 1);
        assert_eq!(
            result.rows[0].lifecycle_classification,
            LifecycleClassification::ScanError
        );
        assert_eq!(result.rows[0].status_token, "partial");
        assert_eq!(result.rows[0].partial_errors, vec!["internal_error"]);
        assert!(result.rows[0].partial_errors.len() <= SCAN_PARTIAL_ERRORS_MAX_PER_ROW);
    }

    fn far_deadline() -> Instant {
        Instant::now() + Duration::from_secs(30)
    }

    fn past_deadline() -> Instant {
        Instant::now() - Duration::from_millis(1)
    }

    fn not_cancelled() -> Arc<AtomicBool> {
        Arc::new(AtomicBool::new(false))
    }

    fn make_target(path: PathBuf) -> ScanRootTarget {
        ScanRootTarget {
            path: std::fs::canonicalize(&path).unwrap_or(path),
            root_kind: RootKind::DiagnosticTestRoot,
        }
    }

    // ── Permit guard tests ────────────────────────────────────────────────────

    #[test]
    fn p089_scan_permit_guard_acquires_successfully() {
        let result = ScanPermitGuard::try_acquire("test-context-acquire");
        assert!(
            result.is_ok(),
            "permit guard must acquire when permits are available"
        );
    }

    #[test]
    fn p089_scan_permit_guard_releases_on_drop() {
        let key = "test-context-release";
        {
            let _guard = ScanPermitGuard::try_acquire(key).expect("first acquire");
            // guard drops here
        }
        // After drop, should be able to acquire again
        let result = ScanPermitGuard::try_acquire(key);
        assert!(result.is_ok(), "permit must be released after drop");
    }

    #[test]
    fn p089_scan_permit_guard_resource_exhausted_when_context_full() {
        // SCAN_CONTEXT_PERMIT_MAX = 1: a second acquire on the same context must fail
        let key = "test-context-exhausted";
        let _guard = ScanPermitGuard::try_acquire(key).expect("first acquire");
        let result = ScanPermitGuard::try_acquire(key);
        assert!(
            result.is_err(),
            "second acquire on same context must return Err"
        );
        assert_eq!(
            result.err(),
            Some(ScanPermitError::ResourceExhausted),
            "error must be ResourceExhausted"
        );
    }

    #[test]
    fn p089_scan_permit_guard_concurrent_first_acquire_never_exceeds_context_max() {
        // Regression for SR-MEDIUM-003: get-or-create-semaphore and first-acquire
        // must happen under one held lock. Previously they were separate steps, so a
        // concurrent caller could see the just-created semaphore evicted by
        // `retain()` (it still looked idle, since nobody had acquired from it yet)
        // and replaced with a second, independent semaphore for the same context —
        // letting more than SCAN_CONTEXT_PERMIT_MAX callers hold a permit for one
        // context at once. This stresses many callers racing on a brand-new key and
        // asserts the peak number of *simultaneous* holders never exceeds the cap.
        use std::sync::atomic::AtomicUsize;
        let key = "test-context-concurrent-race";
        let attempts = 32;
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(attempts));
        let concurrent = std::sync::Arc::new(AtomicUsize::new(0));
        let peak = std::sync::Arc::new(AtomicUsize::new(0));
        let handles: Vec<_> = (0..attempts)
            .map(|_| {
                let barrier = std::sync::Arc::clone(&barrier);
                let concurrent = std::sync::Arc::clone(&concurrent);
                let peak = std::sync::Arc::clone(&peak);
                std::thread::spawn(move || {
                    barrier.wait();
                    if let Ok(_guard) = ScanPermitGuard::try_acquire(key) {
                        let now = concurrent.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(now, Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(5));
                        concurrent.fetch_sub(1, Ordering::SeqCst);
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().expect("thread panicked");
        }
        assert!(
            peak.load(Ordering::SeqCst) <= SCAN_CONTEXT_PERMIT_MAX,
            "at most {SCAN_CONTEXT_PERMIT_MAX} callers may simultaneously hold a permit for \
             the same context; observed peak {}",
            peak.load(Ordering::SeqCst)
        );
    }

    #[tokio::test]
    async fn p089_permit_lease_survives_request_timeout_until_worker_actually_exits() {
        // Regression for SEC-P089-HIGH-001: a request that times out waiting on its
        // spawn_blocking scan worker must NOT free that worker's capacity lease
        // early. This directly exercises the same permit-into-closure +
        // `tokio::select!` pattern used by `execute_inventory_preview`/
        // `run_hidden_readback_scan`, with a synthetic "stuck" worker (an
        // unconditional sleep, independent of any cooperative deadline/cancellation
        // check) standing in for a filesystem syscall blocked on an unresponsive
        // mount.
        let key = "test-context-permit-lease-survives-timeout";
        let permit = ScanPermitGuard::try_acquire(key).expect("first acquire");

        let mut handle = tokio::task::spawn_blocking(move || {
            std::thread::sleep(Duration::from_millis(200));
            permit // dropped only when this closure returns — releasing capacity then
        });

        // Race the worker against a much shorter "request" deadline, exactly like
        // the production `tokio::select!` pattern.
        let timed_out = tokio::select! {
            _ = &mut handle => false,
            _ = tokio::time::sleep(Duration::from_millis(20)) => true,
        };
        assert!(
            timed_out,
            "the synthetic worker must still be running when the request times out"
        );

        // The request has already given up, but the worker has not: capacity for
        // this context must still be unavailable, proving the lease was not
        // reclaimed early — no replacement worker may start while the original is
        // still alive.
        assert_eq!(
            ScanPermitGuard::try_acquire(key).err(),
            Some(ScanPermitError::ResourceExhausted),
            "permit must remain held while its worker is still running, even after \
             the request that spawned it has already timed out"
        );

        // Once the synthetic worker actually finishes, its permit drops (as a
        // temporary, immediately after this statement) and capacity becomes
        // available again — proving the lease is not leaked forever either.
        handle.await.expect("worker must not panic");
        assert!(
            ScanPermitGuard::try_acquire(key).is_ok(),
            "permit must become available once the real worker actually exits"
        );
    }

    // ── Scanner traversal tests ───────────────────────────────────────────────

    #[test]
    fn p089_scan_empty_root_returns_zero_rows() {
        let dir = TempDir::new().unwrap();
        let target = make_target(dir.path().to_path_buf());
        let cancelled = not_cancelled();
        let result = scan_diagnostic_test_root(&target, 500, far_deadline(), &cancelled);
        assert_eq!(result.rows.len(), 0);
        assert_eq!(result.status, InventoryStatus::Complete);
        assert!(!result.truncated);
    }

    #[test]
    fn p089_scan_returns_rows_for_immediate_children() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("file1.txt"), b"hello").unwrap();
        std::fs::write(dir.path().join("file2.txt"), b"world").unwrap();
        std::fs::create_dir(dir.path().join("subdir")).unwrap();

        let target = make_target(dir.path().to_path_buf());
        let cancelled = not_cancelled();
        let result = scan_diagnostic_test_root(&target, 500, far_deadline(), &cancelled);
        assert_eq!(result.rows.len(), 3, "should have 3 immediate children");
        assert_eq!(result.status, InventoryStatus::Complete);
    }

    #[test]
    fn p089_scan_path_display_is_redacted_not_raw() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("secret.txt"), b"secret content").unwrap();
        let target = make_target(dir.path().to_path_buf());
        let cancelled = not_cancelled();
        let result = scan_diagnostic_test_root(&target, 500, far_deadline(), &cancelled);
        assert_eq!(result.rows.len(), 1);
        let row = &result.rows[0];
        // path_display must not contain the raw path
        let raw_path = dir.path().to_string_lossy();
        assert!(
            !row.path_display.contains(raw_path.as_ref()),
            "path_display must not contain raw path: got {:?}",
            row.path_display
        );
        // path_display must start with <redacted:
        assert!(
            row.path_display.starts_with("<redacted:"),
            "path_display must be redacted: got {:?}",
            row.path_display
        );
    }

    #[test]
    fn p089_scan_path_hash_is_64_hex_chars() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("f.txt"), b"x").unwrap();
        let target = make_target(dir.path().to_path_buf());
        let cancelled = not_cancelled();
        let result = scan_diagnostic_test_root(&target, 500, far_deadline(), &cancelled);
        let row = &result.rows[0];
        assert_eq!(
            row.path_hash.len(),
            64,
            "full path_hash must be 64 hex chars"
        );
        assert!(
            row.path_hash
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "path_hash must be lowercase hex"
        );
        assert!(
            row.path_hash_short.len() >= 12 && row.path_hash_short.len() <= 20,
            "path_hash_short must be 12-20 chars"
        );
    }

    #[test]
    fn p089_scan_symlinked_dir_not_descended() {
        let dir = TempDir::new().unwrap();
        let target_dir = TempDir::new().unwrap();
        std::fs::write(target_dir.path().join("secret.txt"), b"in linked dir").unwrap();

        // Create a symlink to the target dir inside our scan root
        let link_path = dir.path().join("link_to_dir");
        std::os::unix::fs::symlink(target_dir.path(), &link_path).unwrap();

        let target = make_target(dir.path().to_path_buf());
        let cancelled = not_cancelled();
        let result = scan_diagnostic_test_root(&target, 500, far_deadline(), &cancelled);

        // Symlink should NOT be descended — no rows for its contents
        // The symlink itself should produce a bounded partial error and 0 rows
        assert_eq!(
            result.rows.len(),
            0,
            "symlinked directory must not be descended"
        );
        assert!(
            !result.errors.is_empty(),
            "symlinked directory must produce a bounded partial error"
        );
        // Secret file inside the linked dir must not appear in any row
        for row in &result.rows {
            assert!(
                !row.path_display.contains("secret"),
                "raw path contents must not leak through symlink"
            );
        }
    }

    #[test]
    fn p089_scan_symlinked_file_produces_link_metadata_row() {
        // traversal_policy: symlinked files are treated as link metadata only, not
        // target contents — unlike a symlinked directory, they must produce exactly
        // one row (not a bounded error), and that row's size must reflect the
        // symlink's own lstat metadata, never the target file's real content/size.
        let dir = TempDir::new().unwrap();
        let target_dir = TempDir::new().unwrap();
        let target_file = target_dir.path().join("real_target.txt");
        std::fs::write(&target_file, b"target file content that must never be read").unwrap();

        let link_path = dir.path().join("link_to_file");
        std::os::unix::fs::symlink(&target_file, &link_path).unwrap();

        let target = make_target(dir.path().to_path_buf());
        let cancelled = not_cancelled();
        let result = scan_diagnostic_test_root(&target, 500, far_deadline(), &cancelled);

        assert_eq!(
            result.rows.len(),
            1,
            "symlinked file must produce exactly one link-metadata row"
        );
        let row = &result.rows[0];
        assert_eq!(row.status_token, "complete");
        assert!(row.partial_errors.is_empty());
        assert!(
            row.path_display.starts_with("<redacted:"),
            "symlinked file row must still be redacted"
        );
        // The symlink's own lstat size is small (a path string); the target file's
        // content is far larger. Asserting the row's size is not the target's real
        // size proves target content was never read to compute it.
        let target_real_size = std::fs::metadata(&target_file).unwrap().len();
        let row_size: u64 = row.estimated_size_bytes.parse().unwrap();
        assert_ne!(
            row_size, target_real_size,
            "symlinked file row must use link metadata, not target content size"
        );
    }

    #[test]
    fn p089_scan_dangling_symlink_produces_link_metadata_row() {
        // A dangling (broken-target) symlink is not a directory, so it is treated
        // like any other non-directory link: a link-metadata row, not an error.
        let dir = TempDir::new().unwrap();
        let missing_target = dir.path().join("does-not-exist");
        let link_path = dir.path().join("dangling_link");
        std::os::unix::fs::symlink(&missing_target, &link_path).unwrap();

        let target = make_target(dir.path().to_path_buf());
        let cancelled = not_cancelled();
        let result = scan_diagnostic_test_root(&target, 500, far_deadline(), &cancelled);

        assert_eq!(
            result.rows.len(),
            1,
            "dangling symlink must produce a link-metadata row, not an error"
        );
        assert_eq!(result.rows[0].status_token, "complete");
    }

    #[test]
    fn p089_scan_respects_row_limit() {
        let dir = TempDir::new().unwrap();
        for i in 0..10 {
            std::fs::write(dir.path().join(format!("f{i}.txt")), b"x").unwrap();
        }
        let target = make_target(dir.path().to_path_buf());
        let cancelled = not_cancelled();
        // Limit to 5
        let result = scan_diagnostic_test_root(&target, 5, far_deadline(), &cancelled);
        assert!(result.rows.len() <= 5, "rows must not exceed limit");
    }

    #[test]
    fn p089_scan_cancellation_respected() {
        let dir = TempDir::new().unwrap();
        // Create enough files that cancellation fires
        for i in 0..10 {
            std::fs::write(dir.path().join(format!("f{i}.txt")), b"x").unwrap();
        }
        let cancelled = Arc::new(AtomicBool::new(true)); // pre-cancelled
        let target = make_target(dir.path().to_path_buf());
        let result = scan_diagnostic_test_root(&target, 500, far_deadline(), &cancelled);
        // The walker checks `terminal_scan_status` at the top of its loop, before
        // fetching or processing any entry, so a pre-cancelled flag is observed
        // deterministically on the first iteration — this must never race to
        // Complete (audit/prepush: "tests explicitly allow Complete for
        // pre-cancelled... scans").
        assert_eq!(
            result.status,
            InventoryStatus::Cancelled,
            "pre-cancelled scan must deterministically return Cancelled"
        );
        assert!(result.errors.iter().any(|error| {
            error.code == InventoryErrorCode::Cancelled && error.phase == Some("enumeration")
        }));
    }

    #[test]
    fn p089_scan_past_deadline_returns_timeout() {
        let dir = TempDir::new().unwrap();
        // Add files to ensure the loop is entered
        for i in 0..5 {
            std::fs::write(dir.path().join(format!("f{i}.txt")), b"x").unwrap();
        }
        let cancelled = not_cancelled();
        let target = make_target(dir.path().to_path_buf());
        // Deadline already in the past
        let result = scan_diagnostic_test_root(&target, 500, past_deadline(), &cancelled);
        // The walker checks `terminal_scan_status` at the top of its loop, before
        // fetching or processing any entry, so an already-past deadline is
        // observed deterministically on the first iteration — this must never
        // race to Complete (audit/prepush: "tests explicitly allow Complete for
        // ...expired-deadline... scans").
        assert_eq!(
            result.status,
            InventoryStatus::Timeout,
            "past-deadline scan must deterministically return Timeout"
        );
        assert!(result.errors.iter().any(|error| {
            error.code == InventoryErrorCode::DeadlineExceeded && error.phase == Some("enumeration")
        }));
    }

    #[test]
    fn p089_scan_estimated_bytes_is_decimal_string() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("f.txt"), b"hello world").unwrap();
        let target = make_target(dir.path().to_path_buf());
        let cancelled = not_cancelled();
        let result = scan_diagnostic_test_root(&target, 500, far_deadline(), &cancelled);
        assert!(!result.rows.is_empty());
        let size_str = &result.rows[0].estimated_size_bytes;
        // Must be a valid decimal string (no leading zeros except "0", no negatives)
        assert!(
            size_str.chars().all(|c| c.is_ascii_digit()),
            "estimated_size_bytes must be decimal: got {size_str:?}"
        );
        assert!(
            size_str == "0" || !size_str.starts_with('0'),
            "estimated_size_bytes must not have leading zeros: got {size_str:?}"
        );
    }

    #[test]
    fn p089_scan_mutation_guard_no_mutations_in_result() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("f.txt"), b"data").unwrap();
        let target = make_target(dir.path().to_path_buf());
        let cancelled = not_cancelled();
        let result = scan_diagnostic_test_root(&target, 500, far_deadline(), &cancelled);

        // The scan result DTO must carry no_delete=true when assembled
        let dto = assemble_scan_dto(
            result,
            true,
            500,
            5000,
            0,
            None,
            domain::temp_artifact_inventory::InventoryMode::HiddenReadback,
        );
        let guard = &dto["mutation_guard"];
        assert_eq!(guard["no_delete"], true);
        assert_eq!(guard["no_prune"], true);
        assert_eq!(guard["no_chmod"], true);
        assert_eq!(guard["no_persist"], true);
        assert_eq!(guard["no_retry"], true);
    }

    #[test]
    fn p089_scan_assemble_dto_disabled_dry_run_null() {
        let dir = TempDir::new().unwrap();
        let target = make_target(dir.path().to_path_buf());
        let cancelled = not_cancelled();
        let result = scan_diagnostic_test_root(&target, 500, far_deadline(), &cancelled);
        let dto = assemble_scan_dto(
            result,
            false,
            500,
            5000,
            0,
            None,
            domain::temp_artifact_inventory::InventoryMode::HiddenReadback,
        );
        assert!(
            dto["dry_run"].is_null(),
            "dry_run must be null when include_dry_run=false"
        );
    }

    #[test]
    fn p089_scan_assemble_dto_schema_version() {
        let dir = TempDir::new().unwrap();
        let target = make_target(dir.path().to_path_buf());
        let cancelled = not_cancelled();
        let result = scan_diagnostic_test_root(&target, 500, far_deadline(), &cancelled);
        let dto = assemble_scan_dto(
            result,
            true,
            500,
            5000,
            0,
            None,
            domain::temp_artifact_inventory::InventoryMode::HiddenReadback,
        );
        assert_eq!(
            dto["schema_version"].as_str().unwrap_or(""),
            "temp_artifact_inventory_v1"
        );
    }

    #[test]
    fn p089_scan_nonexistent_root_returns_error_status() {
        let target = ScanRootTarget {
            path: PathBuf::from("/tmp/p089_nonexistent_test_root_xyz_abc"),
            root_kind: RootKind::DiagnosticTestRoot,
        };
        let cancelled = not_cancelled();
        let result = scan_diagnostic_test_root(&target, 500, far_deadline(), &cancelled);
        assert_eq!(result.status, InventoryStatus::Error);
        assert!(!result.errors.is_empty());
        assert_eq!(result.errors[0].code, InventoryErrorCode::RootUnreadable);
    }

    #[test]
    fn p089_scan_estimated_bytes_accounted() {
        let dir = TempDir::new().unwrap();
        // Write known content
        std::fs::write(dir.path().join("a.txt"), b"1234567890").unwrap(); // 10 bytes
        let target = make_target(dir.path().to_path_buf());
        let cancelled = not_cancelled();
        let result = scan_diagnostic_test_root(&target, 500, far_deadline(), &cancelled);
        // Summary should have non-zero estimated bytes
        assert!(
            result.summary_estimated_bytes > 0,
            "estimated bytes must be >0 for non-empty file"
        );
    }

    #[test]
    fn p089_scan_symlinked_root_returns_error() {
        // A symlinked scan root must be rejected with InvalidRootOverride, not followed.
        let real_dir = TempDir::new().unwrap();
        std::fs::write(real_dir.path().join("secret.txt"), b"content").unwrap();
        let container = TempDir::new().unwrap();
        let link_path = container.path().join("symlink_root");
        std::os::unix::fs::symlink(real_dir.path(), &link_path).unwrap();

        let target = ScanRootTarget {
            path: link_path,
            root_kind: RootKind::DiagnosticTestRoot,
        };
        let cancelled = not_cancelled();
        let result = scan_diagnostic_test_root(&target, 500, far_deadline(), &cancelled);
        assert_eq!(
            result.status,
            InventoryStatus::Error,
            "symlinked root must return Error status"
        );
        assert!(
            !result.errors.is_empty(),
            "symlinked root must produce an error entry"
        );
        assert_eq!(
            result.errors[0].code,
            InventoryErrorCode::InvalidRootOverride,
            "symlinked root error must use InvalidRootOverride code"
        );
        assert_eq!(result.rows.len(), 0, "no rows for symlinked root");
    }

    #[test]
    fn p089_scan_correlation_key_equals_path_hash() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("f.txt"), b"x").unwrap();
        let target = make_target(dir.path().to_path_buf());
        let cancelled = not_cancelled();
        let result = scan_diagnostic_test_root(&target, 500, far_deadline(), &cancelled);
        assert!(!result.rows.is_empty());
        let row = &result.rows[0];
        assert_eq!(
            row.correlation_key, row.path_hash,
            "correlation_key must equal path_hash when path is present"
        );
        assert_eq!(row.path_hash.len(), 64, "path_hash must be 64 chars");
    }

    // ── scan_multi_root tests ─────────────────────────────────────────────────

    #[test]
    fn p089_scan_multi_root_empty_targets_returns_complete() {
        let targets: Vec<ScanRootTarget> = vec![];
        let cancelled = not_cancelled();
        let result = scan_multi_root(&targets, 500, far_deadline(), &cancelled);
        assert_eq!(result.status, InventoryStatus::Complete);
        assert_eq!(result.rows.len(), 0);
        assert_eq!(result.summary_estimated_bytes, 0);
        assert!(!result.truncated);
    }

    #[test]
    fn p089_scan_multi_root_aggregates_rows_from_two_roots() {
        let _guard = shutdown_flag_lock(); // protect from concurrent global-shutdown tests
        let dir1 = TempDir::new().unwrap();
        let dir2 = TempDir::new().unwrap();
        std::fs::write(dir1.path().join("a.txt"), b"aaa").unwrap();
        std::fs::write(dir2.path().join("b.txt"), b"bbb").unwrap();

        let targets = vec![
            make_target(dir1.path().to_path_buf()),
            make_target(dir2.path().to_path_buf()),
        ];
        let cancelled = not_cancelled();
        let result = scan_multi_root(&targets, 500, far_deadline(), &cancelled);
        assert_eq!(result.status, InventoryStatus::Complete);
        assert_eq!(result.rows.len(), 2, "both roots must contribute rows");
        assert!(
            result.summary_estimated_bytes > 0,
            "total bytes must be aggregated"
        );
    }

    #[test]
    fn p089_scan_multi_root_respects_limit_across_roots() {
        let dir1 = TempDir::new().unwrap();
        let dir2 = TempDir::new().unwrap();
        for i in 0..5 {
            std::fs::write(dir1.path().join(format!("f{i}.txt")), b"x").unwrap();
            std::fs::write(dir2.path().join(format!("g{i}.txt")), b"x").unwrap();
        }

        let targets = vec![
            make_target(dir1.path().to_path_buf()),
            make_target(dir2.path().to_path_buf()),
        ];
        let cancelled = not_cancelled();
        let result = scan_multi_root(&targets, 6, far_deadline(), &cancelled);
        assert!(result.rows.len() <= 6, "total rows must not exceed limit");
    }

    #[test]
    fn p089_scan_multi_root_nonexistent_root_produces_partial_or_error() {
        let _guard = shutdown_flag_lock(); // protect from concurrent global-shutdown tests
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("good.txt"), b"x").unwrap();
        let targets = vec![
            make_target(dir.path().to_path_buf()),
            ScanRootTarget {
                path: PathBuf::from("/tmp/p089_multi_root_nonexistent_xyz"),
                root_kind: RootKind::DiagnosticTestRoot,
            },
        ];
        let cancelled = not_cancelled();
        let result = scan_multi_root(&targets, 500, far_deadline(), &cancelled);
        // The good root produces rows; the bad root produces an error
        assert!(
            !result.errors.is_empty(),
            "nonexistent root must produce errors"
        );
        assert!(
            matches!(
                result.status,
                InventoryStatus::Partial | InventoryStatus::Error
            ),
            "multi-root with a bad root must be Partial or Error"
        );
    }

    // ── SEC-P089-008 regression: context semaphore eviction ───────────────────

    #[test]
    fn p089_context_semaphore_map_evicts_idle_entries() {
        // Unique prefix to isolate from other tests running concurrently.
        let base_key = "p089-eviction-regression-";

        // Create and immediately release many distinct context semaphore entries.
        for i in 0..10 {
            let key = format!("{base_key}{i}");
            let _guard = ScanPermitGuard::try_acquire(&key).expect("should acquire unique key");
            // guard drops here, returning all permits — entry becomes idle
        }

        // Triggering a new lookup evicts idle entries before inserting the new one.
        let _trigger_guard = ScanPermitGuard::try_acquire(&format!("{base_key}trigger"));

        // Verify that the map no longer holds the 10 now-idle entries.
        let pool = CONTEXT_SEMAPHORES.get_or_init(|| Mutex::new(HashMap::new()));
        let map = pool.lock().unwrap_or_else(|e| e.into_inner());
        let live_test_entries: Vec<_> = map
            .keys()
            .filter(|k| k.starts_with(base_key))
            .cloned()
            .collect();

        // Only the trigger entry (still active) may remain with our prefix.
        assert!(
            live_test_entries.len() <= 1,
            "idle context semaphore entries must be evicted on next lookup; \
             found {:?} — SEC-P089-008",
            live_test_entries
        );
    }

    // ── SEC-P089-007 regression: global shutdown signal ───────────────────────

    #[test]
    fn p089_scan_global_shutdown_signal_stops_scan() {
        let _guard = shutdown_flag_lock(); // exclusive: other tests must not run while flag is true
        let _scan_guard = scan_execution_test_lock();
        // Reset any leftover state from prior test runs.
        GLOBAL_SHUTDOWN_REQUESTED.store(false, Ordering::SeqCst);

        let dir = TempDir::new().unwrap();
        // Enough files so the scan enters the entry loop.
        for i in 0..20 {
            std::fs::write(dir.path().join(format!("f{i}.txt")), b"x").unwrap();
        }

        // Set global shutdown BEFORE the scan starts (simulates daemon shutdown).
        GLOBAL_SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);

        let cancelled = not_cancelled();
        let target = make_target(dir.path().to_path_buf());
        let result = scan_diagnostic_test_root_impl(&target, 500, far_deadline(), &cancelled);

        // Reset immediately so concurrent tests are not affected.
        GLOBAL_SHUTDOWN_REQUESTED.store(false, Ordering::SeqCst);

        // Scan must have observed the shutdown flag and stopped early. The walker
        // checks `terminal_scan_status` (which also observes
        // `GLOBAL_SHUTDOWN_REQUESTED`) at the top of its loop before processing
        // any entry, so a pre-set shutdown flag is deterministic, not racy
        // (audit/prepush: "tests explicitly allow Complete for... shutdown
        // cases").
        assert_eq!(
            result.status,
            InventoryStatus::Cancelled,
            "global shutdown must deterministically stop scan; got {:?} — SEC-P089-007",
            result.status
        );
    }

    #[test]
    fn p089_request_global_shutdown_is_callable() {
        let _guard = shutdown_flag_lock(); // exclusive: reset immediately after to avoid affecting other tests
        let _scan_guard = scan_execution_test_lock();
        // Verify the public API exists and is callable without panicking.
        request_global_shutdown();
        GLOBAL_SHUTDOWN_REQUESTED.store(false, Ordering::SeqCst);
    }

    // ── SEC-P089-HIGH-001 regression: symlink-swap TOCTOU ────────────────────
    //
    // These tests verify that the O_NOFOLLOW fix closes the TOCTOU window between
    // lstat and read_dir. They test the static symlink case (symlink present at open
    // time), which exercises the same O_NOFOLLOW codepath that closes the race window.

    #[test]
    fn p089_scan_root_symlink_swap_rejected() {
        // Simulates the post-lstat TOCTOU window at the scan root:
        // - Initially a real directory exists and scan works normally.
        // - The directory is then replaced with a symlink (as in a swap race).
        // - O_NOFOLLOW open must reject the now-symlinked path with InvalidRootOverride.
        // SEC-P089-HIGH-001
        let real_dir = TempDir::new().unwrap();
        std::fs::write(real_dir.path().join("escaped.txt"), b"escaped content").unwrap();
        let container = TempDir::new().unwrap();

        // Phase 1: real directory at scan target — scan must succeed.
        let scan_target = container.path().join("scan_target");
        std::fs::create_dir(&scan_target).unwrap();
        std::fs::write(scan_target.join("benign.txt"), b"ok").unwrap();

        let target = make_target(scan_target.clone());
        let cancelled = not_cancelled();
        let result = scan_diagnostic_test_root(&target, 500, far_deadline(), &cancelled);
        assert_eq!(
            result.status,
            InventoryStatus::Complete,
            "real directory must scan successfully (phase 1 baseline)"
        );
        assert_eq!(result.rows.len(), 1, "one child in the real directory");

        // Phase 2: replace the real directory with a symlink (simulates post-lstat swap).
        std::fs::remove_dir_all(&scan_target).unwrap();
        std::os::unix::fs::symlink(real_dir.path(), &scan_target).unwrap();

        let target = ScanRootTarget {
            path: scan_target.clone(),
            root_kind: RootKind::DiagnosticTestRoot,
        };
        let cancelled = not_cancelled();
        let result = scan_diagnostic_test_root(&target, 500, far_deadline(), &cancelled);
        assert_eq!(
            result.status,
            InventoryStatus::Error,
            "symlink-swapped root must be rejected — SEC-P089-HIGH-001"
        );
        assert!(
            !result.errors.is_empty(),
            "symlink-swapped root must produce an error entry"
        );
        assert_eq!(
            result.errors[0].code,
            InventoryErrorCode::InvalidRootOverride,
            "symlink-swapped root error must use InvalidRootOverride — SEC-P089-HIGH-001"
        );
        assert_eq!(result.rows.len(), 0, "no rows for symlink-swapped root");
        // Contents of real_dir must not appear
        for row in &result.rows {
            assert!(
                !row.path_display.contains("escaped"),
                "escaped content must not leak through symlink — SEC-P089-HIGH-001"
            );
        }
    }

    #[test]
    fn p089_scan_nested_dir_symlink_swap_rejected() {
        // Simulates a symlink swap at a nested directory entry after lstat confirmation.
        // Phase 1: a sub-directory exists and is counted by size estimation.
        // Phase 2: the sub-directory is replaced with a symlink.
        // O_NOFOLLOW in estimate_dir_size must reject the swapped entry;
        // contents of the linked directory must not appear in any row.
        // SEC-P089-HIGH-001
        let real_dir = TempDir::new().unwrap();
        std::fs::write(real_dir.path().join("escaped.txt"), b"escaped content").unwrap();
        let container = TempDir::new().unwrap();

        // Phase 1: real sub-directory inside the scan root.
        let subdir = container.path().join("subdir");
        std::fs::create_dir(&subdir).unwrap();
        std::fs::write(subdir.join("child.txt"), b"child").unwrap();

        let target = make_target(container.path().to_path_buf());
        let cancelled = not_cancelled();
        let result = scan_diagnostic_test_root(&target, 500, far_deadline(), &cancelled);
        assert_eq!(
            result.status,
            InventoryStatus::Complete,
            "real sub-directory must scan successfully (phase 1 baseline)"
        );
        // The sub-directory is a child of the scan root and appears as a row.
        assert!(
            !result.rows.is_empty(),
            "real sub-directory must produce a row"
        );
        // estimated_size_bytes must be non-zero (child.txt is inside)
        let row_bytes: u64 = result
            .rows
            .iter()
            .map(|r| r.estimated_size_bytes.parse::<u64>().unwrap_or(0))
            .sum();
        assert!(row_bytes > 0, "estimated bytes must account for contents");

        // Phase 2: replace the sub-directory with a symlink (simulates post-lstat swap).
        std::fs::remove_dir_all(&subdir).unwrap();
        std::os::unix::fs::symlink(real_dir.path(), &subdir).unwrap();

        let target = make_target(container.path().to_path_buf());
        let cancelled = not_cancelled();
        let result = scan_diagnostic_test_root(&target, 500, far_deadline(), &cancelled);
        // The symlink is detected by lstat and emits a bounded error — not traversed.
        assert!(
            !result.errors.is_empty(),
            "symlink-swapped sub-directory must produce a bounded error — SEC-P089-HIGH-001"
        );
        // The escaped content must not appear in any row.
        for row in &result.rows {
            assert!(
                !row.path_display.contains("escaped"),
                "escaped content must not leak through symlink — SEC-P089-HIGH-001"
            );
        }
    }

    #[test]
    fn p089_scan_deeply_nested_chain_is_depth_bounded_not_descriptor_unbounded() {
        // Regression for SR-MEDIUM-002: a singly-nested chain deeper than
        // SCAN_MAX_DIR_DEPTH must stop descending (bounded partial error) rather than
        // opening an unbounded number of simultaneous directory descriptors — this is
        // a distinct failure mode from the distinct-directory-identity cap
        // (SCAN_MAX_VISITED_DIRS), which a long *chain* of always-novel identities does
        // not trip. The scan must still complete promptly rather than hang or panic.
        let container = TempDir::new().unwrap();
        let mut current = container.path().to_path_buf();
        let chain_len = SCAN_MAX_DIR_DEPTH + 32;
        for i in 0..chain_len {
            current = current.join(format!("d{i}"));
            std::fs::create_dir(&current).unwrap();
        }
        std::fs::write(current.join("leaf.txt"), b"leaf content").unwrap();

        let target = make_target(container.path().to_path_buf());
        let cancelled = not_cancelled();
        let result = scan_diagnostic_test_root(&target, 500, far_deadline(), &cancelled);

        assert!(
            result.error_count_observed > 0,
            "exceeding the depth budget must emit at least one bounded partial error"
        );
        assert!(
            matches!(
                result.status,
                InventoryStatus::Complete | InventoryStatus::Partial
            ),
            "a depth-bounded scan must still terminate with a definite status, not hang: got {:?}",
            result.status
        );
    }

    #[test]
    fn p089_scan_root_intermediate_symlink_component_rejected() {
        let outside = TempDir::new().unwrap();
        let escaped_leaf = outside.path().join("leaf");
        std::fs::create_dir(&escaped_leaf).unwrap();
        std::fs::write(escaped_leaf.join("escaped.txt"), b"escaped").unwrap();

        let container = TempDir::new().unwrap();
        let link = container.path().join("link_to_outside");
        std::os::unix::fs::symlink(outside.path(), &link).unwrap();

        let target = ScanRootTarget {
            path: link.join("leaf"),
            root_kind: RootKind::DiagnosticTestRoot,
        };
        let cancelled = not_cancelled();
        let result = scan_diagnostic_test_root(&target, 500, far_deadline(), &cancelled);

        assert_eq!(
            result.status,
            InventoryStatus::Error,
            "intermediate symlink components must be rejected by fd-chain openat traversal"
        );
        assert_eq!(
            result.rows.len(),
            0,
            "escaped leaf contents must not be listed"
        );
        assert!(
            !result
                .rows
                .iter()
                .any(|row| row.path_display.contains("escaped")),
            "escaped content must not leak through an intermediate symlink component"
        );
    }

    // ── SEC-P089-HIGH-001 regression: FIFO/device special-file swap ──────────
    //
    // Verifies that open_child_dir_at rejects a FIFO immediately (O_DIRECTORY flag)
    // rather than blocking the worker thread. The O_DIRECTORY|O_NONBLOCK flags close
    // the TOCTOU window where a special file is swapped in between fstatat and openat.
    #[test]
    fn p089_open_child_dir_at_rejects_fifo_immediately() {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let container = TempDir::new().unwrap();
        let fifo_name = container.path().join("not_a_dir");

        // Create a FIFO (named pipe) in place of a directory.
        let fifo_cstr =
            CString::new(fifo_name.as_os_str().as_bytes()).expect("no NUL in temp path");
        let ret = unsafe { libc::mkfifo(fifo_cstr.as_ptr(), 0o600) };
        assert_eq!(ret, 0, "mkfifo must succeed");

        // Open the parent directory fd for openat.
        let parent_cstr =
            CString::new(container.path().as_os_str().as_bytes()).expect("no NUL in temp path");
        let parent_fd = unsafe {
            libc::open(
                parent_cstr.as_ptr(),
                libc::O_RDONLY | libc::O_CLOEXEC | libc::O_DIRECTORY,
            )
        };
        assert!(parent_fd >= 0, "parent dir must open");
        let parent_owned = OwnedFd::new(parent_fd);

        // open_child_dir_at must fail immediately — not block.
        // With O_DIRECTORY, the FIFO is rejected with ENOTDIR before any I/O occurs.
        let result = open_child_dir_at(parent_owned.raw(), b"not_a_dir");
        assert!(
            result.is_err(),
            "open_child_dir_at must fail for a FIFO — SEC-P089-HIGH-001"
        );
        // Verify the error kind: ENOTDIR (from O_DIRECTORY) maps to ErrorKind::NotADirectory
        // on platforms that support it, or remains an OS error otherwise.
        let err = result.err().expect("checked is_err above");
        let raw = err.raw_os_error().unwrap_or(0);
        assert!(
            raw == libc::ENOTDIR || raw == libc::ENXIO || err.kind() == io::ErrorKind::Other
                || err.kind() == io::ErrorKind::InvalidInput
                || err.kind() == io::ErrorKind::NotFound,
            "FIFO open must fail with ENOTDIR or ENXIO, got raw={raw} kind={:?} — SEC-P089-HIGH-001",
            err.kind()
        );
    }

    #[test]
    fn p089_scan_child_fifo_produces_bounded_error_not_hang() {
        // Verifies that a FIFO planted as a child of the scan root produces a bounded
        // partial error rather than causing the scan worker to hang indefinitely.
        // This is the full end-to-end regression for the O_DIRECTORY fix.
        // SEC-P089-HIGH-001
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let container = TempDir::new().unwrap();
        // Plant a regular file and a FIFO side by side.
        std::fs::write(container.path().join("regular.txt"), b"ok").unwrap();
        let fifo_path = container.path().join("trap_fifo");
        let fifo_cstr =
            CString::new(fifo_path.as_os_str().as_bytes()).expect("no NUL in temp path");
        let ret = unsafe { libc::mkfifo(fifo_cstr.as_ptr(), 0o600) };
        assert_eq!(ret, 0, "mkfifo must succeed");

        let target = make_target(container.path().to_path_buf());
        let cancelled = not_cancelled();

        // The scan must complete within the deadline — not hang on the FIFO.
        let result = scan_diagnostic_test_root(&target, 500, far_deadline(), &cancelled);

        // FIFO produces a bounded error; the regular file is still enumerated.
        assert!(
            !result.errors.is_empty() || result.status == InventoryStatus::Complete,
            "scan must complete (not hang) when a FIFO is present — SEC-P089-HIGH-001"
        );
        // The scan result must not be an unrecoverable panic or hang.
        // Status is either Complete (FIFO seen as non-dir, skipped with error) or Partial.
        assert!(
            matches!(
                result.status,
                InventoryStatus::Complete | InventoryStatus::Partial
            ),
            "status must be Complete or Partial, not hung or panic — SEC-P089-HIGH-001; got {:?}",
            result.status
        );
    }
}
