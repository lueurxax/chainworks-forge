//! Log retention (Proposal 042 §9.1, lines 572-582).
//!
//! `tracing_appender::rolling::daily` rotates the packaged daemon log at
//! local midnight, producing files named `daemon.log.YYYY-MM-DD`. By
//! itself it never deletes older files, so an operator who ran the app
//! daily for six months would accumulate 180 rotated files and
//! unbounded disk usage. §9.1 requires:
//!
//!   * **Total cap:** the log directory's rotated files must not exceed
//!     **50 MB** combined. The live `daemon.log` (current day) is
//!     exempt from the cap — we never delete the file being written.
//!   * **Age cap:** any rotated file older than **7 days** is removed.
//!   * **Count cap:** no more than **5** rotated files are retained,
//!     regardless of age or size.
//!
//! The caps compose by *strictness*: the retention pass deletes files
//! that violate **any** cap, not merely the most generous one. A file
//! that is under 7 days old can still be dropped if keeping it would
//! push the count above 5 or the byte total above 50 MB.
//!
//! # When retention runs
//!
//! The daemon invokes [`enforce_retention`] once at startup, right after
//! installing the rolling appender. That is enough to keep the disk
//! envelope bounded across normal restart cadences (SMAppService
//! restarts the daemon daily in the worst case). A future enhancement
//! could schedule a periodic sweep; for P042 the startup sweep matches
//! the one-shot semantics of the supervisor-owned lifecycle hooks.
//!
//! # Why a dedicated module
//!
//! `tracing-appender` does not offer retention knobs. Implementing the
//! sweep in a tiny dedicated module keeps the retention policy
//! isolated, testable, and swappable if we ever migrate to `logrotate`
//! or a different appender crate.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// P042 §9.1 total-bytes cap for rotated log files (exclusive of the
/// live file). 50 MiB in bytes.
pub const MAX_TOTAL_BYTES: u64 = 50 * 1024 * 1024;
/// P042 §9.1 age cap for rotated log files.
pub const MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);
/// P042 §9.1 file-count cap for rotated files (exclusive of the live file).
pub const MAX_ROTATED_FILES: usize = 5;

/// Outcome summary emitted via `tracing::info!` when the sweep runs.
/// Exposed from `enforce_retention` so tests can assert behavior and so
/// `main.rs` can log a structured summary instead of free-form text.
#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub struct RetentionReport {
    /// Count of rotated files inspected (excludes the live file).
    pub rotated_found: usize,
    /// Count of files deleted because they exceeded `MAX_AGE`.
    pub deleted_by_age: usize,
    /// Count of files deleted because they exceeded `MAX_ROTATED_FILES`.
    pub deleted_by_count: usize,
    /// Count of files deleted because the running total exceeded `MAX_TOTAL_BYTES`.
    pub deleted_by_size: usize,
    /// Bytes of data freed in this sweep (sum of deleted file sizes).
    pub bytes_freed: u64,
}

/// Enforce the §9.1 retention policy on a log directory. `live_basename`
/// names the current-day file (`"daemon.log"` in the packaged daemon)
/// and is skipped from deletion even if it somehow qualifies. Returns
/// the sweep outcome for observability — callers typically log the
/// report at `INFO` level and move on.
///
/// Errors from `read_dir` / `metadata` / `remove_file` are swallowed per
/// file: the retention sweep must never block daemon startup. Any
/// failure is reported via the counts not matching the expected
/// deletion (and structured logging upstream — but this function itself
/// is deliberately side-effect minimal).
pub fn enforce_retention(log_dir: &Path, live_basename: &str) -> RetentionReport {
    enforce_retention_with(
        log_dir,
        live_basename,
        MAX_TOTAL_BYTES,
        MAX_AGE,
        MAX_ROTATED_FILES,
        SystemTime::now(),
    )
}

/// Test seam — same contract as [`enforce_retention`] but with explicit
/// limits and a simulated `now`. Production callers should use
/// [`enforce_retention`]; this variant exists so unit tests can build
/// small directories and assert the algorithm without waiting 7 days.
pub fn enforce_retention_with(
    log_dir: &Path,
    live_basename: &str,
    max_total_bytes: u64,
    max_age: Duration,
    max_rotated_files: usize,
    now: SystemTime,
) -> RetentionReport {
    let mut report = RetentionReport::default();

    let entries = match fs::read_dir(log_dir) {
        Ok(entries) => entries,
        Err(_) => return report,
    };

    // Collect candidate rotated files (same basename prefix, not the
    // live file itself). Each entry is recorded with its modification
    // time (for age ordering) and its size (for byte-cap accounting).
    struct Candidate {
        path: PathBuf,
        modified: SystemTime,
        size: u64,
    }
    let mut candidates: Vec<Candidate> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        // Match `daemon.log.*` but skip the live `daemon.log` itself.
        if name == live_basename {
            continue;
        }
        if !name.starts_with(&format!("{live_basename}.")) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        candidates.push(Candidate {
            path,
            modified,
            size: meta.len(),
        });
    }

    report.rotated_found = candidates.len();

    // Sort newest → oldest so the count cap and byte cap both drop the
    // oldest survivors first.
    candidates.sort_by(|a, b| b.modified.cmp(&a.modified));

    // Age pass. Must run before count/size so the newest-first ordering
    // stays meaningful (a file older than 7 days is gone no matter
    // where it sits in the ordering).
    candidates.retain(|c| {
        let age_ok = now
            .duration_since(c.modified)
            .map(|age| age <= max_age)
            .unwrap_or(true); // Clock-skew safety: keep the file.
        if !age_ok {
            if fs::remove_file(&c.path).is_ok() {
                report.deleted_by_age += 1;
                report.bytes_freed = report.bytes_freed.saturating_add(c.size);
            }
        }
        age_ok
    });

    // Count pass. Keep the newest `max_rotated_files`; delete the rest.
    if candidates.len() > max_rotated_files {
        let overflow = candidates.split_off(max_rotated_files);
        for c in overflow {
            if fs::remove_file(&c.path).is_ok() {
                report.deleted_by_count += 1;
                report.bytes_freed = report.bytes_freed.saturating_add(c.size);
            }
        }
    }

    // Size pass. Walk newest → oldest accumulating bytes; once we cross
    // the cap every subsequent file is deleted. The newest file gets a
    // free pass even if it is itself larger than the cap (dropping it
    // would leave zero context for the next operator diagnostic).
    let mut running_total: u64 = 0;
    let mut size_kept: Vec<Candidate> = Vec::with_capacity(candidates.len());
    for c in candidates.into_iter() {
        let next_total = running_total.saturating_add(c.size);
        if !size_kept.is_empty() && next_total > max_total_bytes {
            if fs::remove_file(&c.path).is_ok() {
                report.deleted_by_size += 1;
                report.bytes_freed = report.bytes_freed.saturating_add(c.size);
            }
            continue;
        }
        running_total = next_total;
        size_kept.push(c);
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn touch(dir: &Path, name: &str, size: usize, modified: SystemTime) {
        let path = dir.join(name);
        let mut f = File::create(&path).unwrap();
        if size > 0 {
            let buf = vec![b'x'; size];
            f.write_all(&buf).unwrap();
        }
        drop(f);
        // Apply the intended mtime via filetime so the test doesn't
        // depend on the test runner's actual clock speed.
        let ft = filetime_from_system_time(modified);
        filetime::set_file_mtime(&path, ft).unwrap();
    }

    fn filetime_from_system_time(t: SystemTime) -> filetime::FileTime {
        let d = t.duration_since(SystemTime::UNIX_EPOCH).unwrap();
        filetime::FileTime::from_unix_time(d.as_secs() as i64, d.subsec_nanos())
    }

    #[test]
    fn age_cap_deletes_files_older_than_seven_days() {
        let dir = TempDir::new().unwrap();
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100 * 86_400);
        let ten_days_old = now - Duration::from_secs(10 * 86_400);
        let one_day_old = now - Duration::from_secs(1 * 86_400);
        touch(dir.path(), "daemon.log.2026-01-01", 100, ten_days_old);
        touch(dir.path(), "daemon.log.2026-01-10", 100, one_day_old);
        // Live file must always survive.
        touch(dir.path(), "daemon.log", 100, now);

        let report = enforce_retention_with(
            dir.path(),
            "daemon.log",
            10_000,
            Duration::from_secs(7 * 86_400),
            5,
            now,
        );
        assert_eq!(report.rotated_found, 2);
        assert_eq!(report.deleted_by_age, 1);
        assert_eq!(report.deleted_by_count, 0);
        assert_eq!(report.deleted_by_size, 0);
        assert!(dir.path().join("daemon.log").exists());
        assert!(dir.path().join("daemon.log.2026-01-10").exists());
        assert!(!dir.path().join("daemon.log.2026-01-01").exists());
    }

    #[test]
    fn count_cap_keeps_newest_five() {
        let dir = TempDir::new().unwrap();
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100 * 86_400);
        // 7 files, all under the age and size caps — count cap must
        // drop the two oldest.
        for i in 0..7 {
            let t = now - Duration::from_secs(i as u64 * 3600);
            touch(
                dir.path(),
                &format!("daemon.log.2026-01-{:02}", 10 - i),
                100,
                t,
            );
        }
        touch(dir.path(), "daemon.log", 100, now);

        let report = enforce_retention_with(
            dir.path(),
            "daemon.log",
            10_000,
            Duration::from_secs(30 * 86_400),
            5,
            now,
        );
        assert_eq!(report.rotated_found, 7);
        assert_eq!(report.deleted_by_count, 2);
        assert_eq!(report.deleted_by_age, 0);
        let remaining: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| {
                e.ok()
                    .and_then(|e| e.file_name().to_str().map(|s| s.to_string()))
            })
            .filter(|n| n.starts_with("daemon.log."))
            .collect();
        assert_eq!(remaining.len(), 5);
    }

    #[test]
    fn size_cap_drops_oldest_when_over_max_total_bytes() {
        let dir = TempDir::new().unwrap();
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100 * 86_400);
        // Four files each 400 bytes = 1600 total. Cap at 1000 must drop
        // the oldest until the running total stays under the cap.
        for i in 0..4 {
            let t = now - Duration::from_secs(i as u64 * 3600);
            touch(
                dir.path(),
                &format!("daemon.log.2026-01-{:02}", 10 - i),
                400,
                t,
            );
        }
        touch(dir.path(), "daemon.log", 100, now);

        let report = enforce_retention_with(
            dir.path(),
            "daemon.log",
            1000,
            Duration::from_secs(30 * 86_400),
            50,
            now,
        );
        assert_eq!(report.rotated_found, 4);
        assert_eq!(report.deleted_by_age, 0);
        assert_eq!(report.deleted_by_count, 0);
        // Newest (400) always kept. Running total 400 → next 800 → next
        // 1200 > 1000 ⇒ delete. Last is 1600 > 1000 ⇒ delete. So two
        // deletions.
        assert_eq!(report.deleted_by_size, 2);
        assert_eq!(report.bytes_freed, 800);
    }

    #[test]
    fn live_file_is_never_considered_for_deletion() {
        let dir = TempDir::new().unwrap();
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100 * 86_400);
        // Stale live file mtime (a process that was idle for weeks).
        let forty_days_ago = now - Duration::from_secs(40 * 86_400);
        touch(dir.path(), "daemon.log", 999, forty_days_ago);

        let report = enforce_retention_with(
            dir.path(),
            "daemon.log",
            10,
            Duration::from_secs(7 * 86_400),
            1,
            now,
        );
        assert_eq!(report.rotated_found, 0);
        assert!(dir.path().join("daemon.log").exists());
    }

    #[test]
    fn sweep_is_idempotent_and_noop_on_clean_dir() {
        // Use the `_with` seam so the test pins `now` instead of
        // relying on the wall-clock — otherwise the fixture's mtime
        // (1970 + 100 days) is decades older than real `SystemTime::now()`
        // and the age pass deletes the file on the first call.
        let dir = TempDir::new().unwrap();
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100 * 86_400);
        touch(dir.path(), "daemon.log", 100, now);
        touch(dir.path(), "daemon.log.2026-01-09", 100, now);

        let first = enforce_retention_with(
            dir.path(),
            "daemon.log",
            10_000,
            Duration::from_secs(7 * 86_400),
            5,
            now,
        );
        let second = enforce_retention_with(
            dir.path(),
            "daemon.log",
            10_000,
            Duration::from_secs(7 * 86_400),
            5,
            now,
        );
        assert_eq!(first.rotated_found, 1);
        assert_eq!(second.rotated_found, 1);
        assert_eq!(
            first.deleted_by_age + first.deleted_by_count + first.deleted_by_size,
            0
        );
        assert_eq!(
            second.deleted_by_age + second.deleted_by_count + second.deleted_by_size,
            0
        );
    }

    #[test]
    fn missing_directory_returns_empty_report_without_panicking() {
        let report = enforce_retention(Path::new("/this/path/does/not/exist"), "daemon.log");
        assert_eq!(report, RetentionReport::default());
    }

    #[test]
    fn unrelated_files_in_log_dir_are_ignored() {
        // Fix `now` so the sweep compares the fixture mtimes against a
        // stable reference. The production `enforce_retention` wrapper
        // reads `SystemTime::now()`, which in this test would treat a
        // 1970+100d mtime as ancient and tempt the age pass to delete.
        let dir = TempDir::new().unwrap();
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100 * 86_400);
        let old = now - Duration::from_secs(10 * 86_400);
        touch(dir.path(), "README.txt", 100, old);
        touch(dir.path(), "crash-budget.json", 100, old);
        touch(dir.path(), "daemon.log", 100, now);

        let report = enforce_retention_with(
            dir.path(),
            "daemon.log",
            10_000,
            Duration::from_secs(7 * 86_400),
            5,
            now,
        );
        assert_eq!(report.rotated_found, 0);
        // Non-matching files survive intact.
        assert!(dir.path().join("README.txt").exists());
        assert!(dir.path().join("crash-budget.json").exists());
    }
}
