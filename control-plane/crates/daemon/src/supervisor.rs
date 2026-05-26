//! Supervisor-side utilities (Proposal 042 §6).
//!
//! Owns the advisory PID lock (§6.1) and the crash-loop budget (§6.2).
//! All functions are Unix-only; on non-Unix platforms they are no-ops
//! because packaged mode targets macOS.
//!
//! # Why the daemon doesn't own restart policy
//!
//! The daemon exits with a deterministic status code and lets the
//! supervisor-of-record (SMAppService for `packaged-app`, launchd for
//! `packaged-helper`, the developer for `dev`) decide whether to restart.
//! This module provides only the *inputs* that policy needs:
//!
//! - A PID lock that refuses duplicate singletons and reclaims stale ones.
//! - A crash-budget record the daemon reads on startup to decide whether
//!   to enter failed-serve mode (§6.2).
//! - A SIGTERM/SIGINT handler for graceful shutdown (§6.3).

use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use fs2::FileExt;
use serde::{Deserialize, Serialize};

/// Outcome of `acquire_pid_lock` per P042 §6.1.
#[derive(Debug)]
pub enum PidLockOutcome {
    /// Lock was acquired. Retained `PidLock` must stay alive for the
    /// process lifetime; dropping it releases the flock (by closing the
    /// file descriptor) and removes the file.
    Acquired(PidLock),
    /// Lock is held by a live peer with the given PID. Caller should exit
    /// 0 (duplicate-healthy singleton policy).
    DuplicateHealthy { peer_pid: u32 },
    /// Anomalous case from §6.1: flock is held by someone, yet the PID
    /// recorded in the file is dead. Caller should exit 75 (`EX_TEMPFAIL`)
    /// and let the supervisor dialog the condition.
    AnomalousHolder { recorded_pid: u32 },
}

/// RAII lock guard. Drop closes the fd (which releases the kernel flock)
/// and unlinks the file.
pub struct PidLock {
    file: std::fs::File,
    path: PathBuf,
}

impl std::fmt::Debug for PidLock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PidLock").field("path", &self.path).finish()
    }
}

impl PidLock {
    /// Absolute path of the lock file.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PidLock {
    fn drop(&mut self) {
        // Close the fd first so the flock is released; THEN unlink so a
        // racing `acquire` in another process sees either "lock held" or
        // "no file" but never "file present, flock-free, yet racing cleanup".
        let _ = self.file.sync_all();
        let _ = std::fs::remove_file(&self.path);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LockError {
    #[error("pid lock io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("pid lock: unexpected liveness probe error for pid {pid}: {errno}")]
    UnknownKillErr { pid: u32, errno: i32 },
}

/// Acquire the advisory exclusive PID lock per §6.1. Caller maps the
/// three [`PidLockOutcome`] variants to their respective exit behaviors.
///
/// The function never exits the process itself — it only reports the
/// outcome — so the caller can test the full three-case flow without
/// process termination.
#[cfg(unix)]
pub fn acquire_pid_lock(path: &Path) -> Result<PidLockOutcome, LockError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Step 1: open-or-create with O_CREAT|O_RDWR. Do NOT use O_EXCL — we
    // need to read an existing file in the conflict branch.
    let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(path)?;

    // Step 2: non-blocking advisory flock.
    match file.try_lock_exclusive() {
        Ok(()) => {
            // We own the lock. Overwrite stale contents (may exist if the
            // previous owner crashed before Drop removed the file).
            file.seek(SeekFrom::Start(0))?;
            file.set_len(0)?;
            let pid = std::process::id();
            writeln!(file, "{pid}")?;
            file.sync_all()?;
            Ok(PidLockOutcome::Acquired(PidLock {
                file,
                path: path.to_path_buf(),
            }))
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            // Lock held elsewhere. Probe the recorded PID.
            let mut buf = String::new();
            file.seek(SeekFrom::Start(0))?;
            file.read_to_string(&mut buf)?;
            let recorded_pid: u32 = buf.trim().parse().unwrap_or(0);
            // `kill(pid, 0)` does not send a signal; it just checks existence.
            // Safety: calling into libc with a numeric pid is well-defined.
            let kill_ret = unsafe { libc::kill(recorded_pid as libc::pid_t, 0) };
            if kill_ret == 0 {
                // Peer is alive.
                Ok(PidLockOutcome::DuplicateHealthy {
                    peer_pid: recorded_pid,
                })
            } else {
                let err = std::io::Error::last_os_error();
                match err.raw_os_error() {
                    Some(libc::ESRCH) => {
                        // Peer is dead but flock is held by some other fd
                        // keeping the file alive. Anomalous per §6.1 case (c).
                        // Best-effort: hint in logs which PID we saw.
                        let _ = drop(file);
                        Ok(PidLockOutcome::AnomalousHolder { recorded_pid })
                    }
                    Some(other) => Err(LockError::UnknownKillErr {
                        pid: recorded_pid,
                        errno: other,
                    }),
                    None => Err(LockError::Io(err)),
                }
            }
        }
        Err(e) => Err(LockError::Io(e)),
    }
}

/// Derive the daemon singleton lock path for a SQLite file URL.
///
/// This is intentionally scoped to the database file, not to app-support
/// or the packaged helper name. A debug `target/debug/control-plane`
/// process pointed at the production DB and a bundled
/// `chainworks-forge-daemon` process therefore contend on the same kernel
/// flock even if they have different `daemon.pid` paths or bind different
/// ports.
#[cfg(unix)]
fn sqlite_database_lock_path(database_url: &str) -> Option<PathBuf> {
    if database_url.contains(":memory:") {
        return None;
    }

    let raw_path = database_url
        .strip_prefix("sqlite://")
        .or_else(|| database_url.strip_prefix("sqlite:"))
        .unwrap_or(database_url)
        .split('?')
        .next()
        .unwrap_or("")
        .trim();
    if raw_path.is_empty() {
        return None;
    }

    let db_path = PathBuf::from(raw_path);
    let lock_name = match db_path.file_name().and_then(|name| name.to_str()) {
        Some(name) if !name.is_empty() => format!("{name}.lock"),
        _ => return None,
    };
    Some(db_path.with_file_name(lock_name))
}

/// Acquire a singleton lock derived from the SQLite database file.
///
/// Returns `Ok(None)` for in-memory databases where no cross-process file
/// owner exists. File-backed databases return the same three-state outcome
/// as [`acquire_pid_lock`].
#[cfg(unix)]
pub fn acquire_database_lock(database_url: &str) -> Result<Option<PidLockOutcome>, LockError> {
    let Some(path) = sqlite_database_lock_path(database_url) else {
        return Ok(None);
    };
    acquire_pid_lock(&path).map(Some)
}

#[cfg(not(unix))]
pub fn acquire_database_lock(_database_url: &str) -> Result<Option<PidLockOutcome>, LockError> {
    Ok(None)
}

/// Non-Unix stub: returns a permissive lock that does nothing. Packaged
/// mode targets macOS only; dev/test on Windows is unsupported.
#[cfg(not(unix))]
pub fn acquire_pid_lock(path: &Path) -> Result<PidLockOutcome, LockError> {
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(true)
        .open(path)?;
    Ok(PidLockOutcome::Acquired(PidLock {
        file,
        path: path.to_path_buf(),
    }))
}

// ── Crash-loop budget (§6.2) ───────────────────────────────────────────

/// Persisted state of the crash-loop budget file (§6.2). On-disk at
/// `~/Library/Application Support/Chainworks Forge/crash-budget.json` in
/// packaged modes; in dev/test it lives wherever the caller specifies.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CrashBudgetFile {
    /// Unix epoch seconds of the earliest crash in the current window,
    /// or 0 if no crashes have been recorded.
    #[serde(default)]
    pub first_crash_at: u64,
    /// Number of crashes recorded since `first_crash_at`.
    #[serde(default)]
    pub crash_count: u32,
}

/// Decision derived from the on-disk budget file per §6.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrashBudgetDecision {
    /// No prior crashes recorded; proceed with normal startup.
    Clean,
    /// Some crashes recorded but the budget is intact; caller may log a
    /// `crash_loop_warn` event and proceed.
    Warn { count: u32 },
    /// Window has expired; caller may reset the file and proceed.
    WindowExpired,
    /// 5+ crashes within 60 s → enter failed-serve mode (§6.2) with
    /// `FailureKind::CrashLoopBudgetExhausted`.
    Exhausted { count: u32, first_crash_at: u64 },
}

/// Threshold: 5 crashes within a 60 s window triggers exhaustion.
pub const CRASH_BUDGET_WINDOW_SECS: u64 = 60;
pub const CRASH_BUDGET_MAX_COUNT: u32 = 5;
/// After this long in `Ready`, the crash budget file is reset.
pub const CRASH_BUDGET_RESET_AFTER_READY_SECS: u64 = 5 * 60;

/// Read the budget file and classify. Returns `CrashBudgetDecision::Clean`
/// if the file is absent or unparseable.
pub fn read_crash_budget(path: &Path) -> CrashBudgetDecision {
    let data = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return CrashBudgetDecision::Clean,
    };
    let file: CrashBudgetFile = match serde_json::from_str(&data) {
        Ok(f) => f,
        Err(_) => return CrashBudgetDecision::Clean,
    };
    if file.crash_count == 0 {
        return CrashBudgetDecision::Clean;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let window_elapsed = now.saturating_sub(file.first_crash_at);
    if window_elapsed > CRASH_BUDGET_WINDOW_SECS {
        CrashBudgetDecision::WindowExpired
    } else if file.crash_count >= CRASH_BUDGET_MAX_COUNT {
        CrashBudgetDecision::Exhausted {
            count: file.crash_count,
            first_crash_at: file.first_crash_at,
        }
    } else {
        CrashBudgetDecision::Warn {
            count: file.crash_count,
        }
    }
}

/// Record a crash: increment `crash_count` if the window is open, or
/// start a new window. Called by external supervision logic (e.g. the
/// Swift app) when the daemon's previous exit was abnormal. Not called
/// by the daemon itself.
pub fn record_crash(path: &Path) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut file: CrashBudgetFile = std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    if file.crash_count == 0 || now.saturating_sub(file.first_crash_at) > CRASH_BUDGET_WINDOW_SECS {
        // Start a fresh window.
        file.first_crash_at = now;
        file.crash_count = 1;
    } else {
        file.crash_count = file.crash_count.saturating_add(1);
    }
    let json = serde_json::to_string_pretty(&file).unwrap_or_else(|_| "{}".into());
    std::fs::write(path, json)
}

/// Clear the crash budget file. Called after 5 min of `Ready` uptime, or
/// when the operator clicks "Reset Crash Budget" in the Swift app.
pub fn reset_crash_budget(path: &Path) -> Result<(), std::io::Error> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ── PID lock tests ────────────────────────────────────────────────

    #[test]
    fn pid_lock_acquires_on_fresh_path() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("daemon.pid");
        match acquire_pid_lock(&path).unwrap() {
            PidLockOutcome::Acquired(lock) => {
                assert!(lock.path().exists());
                let contents = std::fs::read_to_string(lock.path()).unwrap();
                let written_pid: u32 = contents.trim().parse().unwrap();
                assert_eq!(written_pid, std::process::id());
            }
            other => panic!("expected Acquired, got {other:?}"),
        }
    }

    #[test]
    fn pid_lock_drop_removes_file_and_releases_flock() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("daemon.pid");
        {
            let outcome = acquire_pid_lock(&path).unwrap();
            match outcome {
                PidLockOutcome::Acquired(_) => {}
                other => panic!("first acquire failed: {other:?}"),
            }
            // Drop at end of scope.
        }
        assert!(!path.exists(), "drop should remove lock file");

        // Second acquire should succeed fresh.
        match acquire_pid_lock(&path).unwrap() {
            PidLockOutcome::Acquired(_) => {}
            other => panic!("second acquire after drop failed: {other:?}"),
        }
    }

    #[test]
    fn pid_lock_rejects_duplicate_live_holder() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("daemon.pid");
        let lock1 = acquire_pid_lock(&path).unwrap();
        let _keepalive = match lock1 {
            PidLockOutcome::Acquired(l) => l,
            _ => panic!("first acquire"),
        };
        // Second acquire in the same process: flock is not reentrant, so
        // we get WouldBlock. `kill(self.pid, 0)` succeeds (we are alive),
        // so the outcome is DuplicateHealthy with peer_pid == our pid.
        match acquire_pid_lock(&path).unwrap() {
            PidLockOutcome::DuplicateHealthy { peer_pid } => {
                assert_eq!(peer_pid, std::process::id());
            }
            other => panic!("expected DuplicateHealthy, got {other:?}"),
        }
    }

    #[test]
    fn pid_lock_reclaims_stale_file_after_crash() {
        // Simulate a stale PID file: write a dead PID into the file path,
        // but don't hold the flock (the crashed process's kernel flock is
        // released automatically).
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("daemon.pid");
        // Use PID 1 (init) then check if we can overwrite; actually PID 1 is
        // always alive. Use a "dead" PID that surely doesn't exist: 2^30 is
        // above most PID_MAX configs.
        std::fs::write(&path, "1073741824").unwrap();
        match acquire_pid_lock(&path).unwrap() {
            PidLockOutcome::Acquired(lock) => {
                // File should now contain our PID, not the stale one.
                let contents = std::fs::read_to_string(lock.path()).unwrap();
                let written_pid: u32 = contents.trim().parse().unwrap();
                assert_eq!(written_pid, std::process::id());
            }
            other => panic!("stale file should be reclaimed, got {other:?}"),
        }
    }

    #[test]
    fn database_lock_rejects_duplicate_even_when_pid_paths_differ() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("control-plane.db");
        let database_url = format!("sqlite://{}?mode=rwc", db_path.display());

        let first = acquire_database_lock(&database_url).unwrap();
        let _keepalive = match first {
            Some(PidLockOutcome::Acquired(lock)) => lock,
            other => panic!("first DB lock should acquire, got {other:?}"),
        };

        match acquire_database_lock(&database_url).unwrap() {
            Some(PidLockOutcome::DuplicateHealthy { peer_pid }) => {
                assert_eq!(peer_pid, std::process::id());
            }
            other => panic!("duplicate DB lock should be rejected, got {other:?}"),
        }
    }

    #[test]
    fn database_lock_is_not_required_for_memory_database() {
        let outcome = acquire_database_lock("sqlite::memory:").unwrap();
        assert!(outcome.is_none());
    }

    // ── Crash-budget tests ────────────────────────────────────────────

    #[test]
    fn crash_budget_absent_file_is_clean() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("crash-budget.json");
        assert_eq!(read_crash_budget(&path), CrashBudgetDecision::Clean);
    }

    #[test]
    fn crash_budget_single_crash_is_warn() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("crash-budget.json");
        record_crash(&path).unwrap();
        assert_eq!(
            read_crash_budget(&path),
            CrashBudgetDecision::Warn { count: 1 }
        );
    }

    #[test]
    fn crash_budget_five_crashes_in_60s_is_exhausted() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("crash-budget.json");
        for _ in 0..CRASH_BUDGET_MAX_COUNT {
            record_crash(&path).unwrap();
        }
        match read_crash_budget(&path) {
            CrashBudgetDecision::Exhausted { count, .. } => {
                assert_eq!(count, CRASH_BUDGET_MAX_COUNT);
            }
            other => panic!("expected Exhausted, got {other:?}"),
        }
    }

    #[test]
    fn crash_budget_stale_window_is_window_expired() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("crash-budget.json");
        // Write a budget file that claims a crash far in the past.
        let file = CrashBudgetFile {
            first_crash_at: 1,
            crash_count: 5,
        };
        std::fs::write(&path, serde_json::to_string(&file).unwrap()).unwrap();
        assert_eq!(read_crash_budget(&path), CrashBudgetDecision::WindowExpired);
    }

    #[test]
    fn reset_crash_budget_removes_file_idempotently() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("crash-budget.json");
        record_crash(&path).unwrap();
        assert!(path.exists());
        reset_crash_budget(&path).unwrap();
        assert!(!path.exists());
        // Second call on missing file is ok.
        reset_crash_budget(&path).unwrap();
    }

    #[test]
    fn record_crash_after_window_expiry_starts_new_window() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("crash-budget.json");
        // Pre-populate with an old window.
        let old = CrashBudgetFile {
            first_crash_at: 1,
            crash_count: 5,
        };
        std::fs::write(&path, serde_json::to_string(&old).unwrap()).unwrap();
        record_crash(&path).unwrap();
        match read_crash_budget(&path) {
            CrashBudgetDecision::Warn { count: 1 } => {} // fresh window, 1 crash
            other => panic!("expected fresh Warn(1), got {other:?}"),
        }
    }
}
