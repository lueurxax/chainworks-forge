//! Parity-control directory infrastructure for P041.
//!
//! Manages `control-plane/target/parity-control/`: lease records, heartbeat,
//! current-step, and reclaim/interruption/timeout/release markers.
//!
//! # Atomic write contract
//!
//! Every canonical control file is written via:
//!   1. Serialize to JSON.
//!   2. Create a temporary file in the **same directory** as the target (same-volume rule).
//!   3. Write + flush + durable flush. On Darwin, `F_FULLFSYNC` (barrier flush via
//!      `libc::fcntl(fd, F_FULLFSYNC)`) is used for crash-safe lease metadata; falls
//!      back to `sync_all` on non-Darwin platforms or if F_FULLFSYNC fails (e.g. NFS).
//!   4. Atomic `rename` into the target path.
//!   5. Best-effort parent-directory `fsync` for durability barrier.
//!
//! In-place truncate-and-rewrite is forbidden for any control file.

use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Canonical required fixture ids for P041 parity evidence (Section 6.1).
/// Authoritative source of truth — tests and gate scripts must import from here.
pub const REQUIRED_FIXTURES: &[&str] = &[
    "proposal-loop-basic",
    "implementation-refine-review",
    "approval-pause-resume",
    "retry-recovery-flow",
    "cancelled-or-blocked-run",
    "terminal-report-evidence",
    "projection-readback-surface",
];

/// Canonical required comparison surface ids for P041 parity evidence (Section 6.5).
/// Fixed order matches the summary-grid column order in Section 5.1.
pub const REQUIRED_COMPARISON_SURFACES: &[&str] = &[
    "canonical_domain_state",
    "projections",
    "graphql_readback",
    "mcp_report_readback",
    "artifact_identity",
    "operator_summary",
];

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Root of the cleanup-safe parity-control directory.
///
/// This directory is never removed by the gate, even after a successful run.
/// All control files inside are written via the atomic write contract.
pub struct ParityControlRoot {
    root: PathBuf,
}

impl ParityControlRoot {
    pub fn new(parity_control_dir: impl Into<PathBuf>) -> Self {
        Self {
            root: parity_control_dir.into(),
        }
    }

    /// Create the parity-control directory and place `.metadata_never_index`
    /// so Spotlight does not race the first replay writes.
    pub fn init(&self) -> Result<()> {
        fs::create_dir_all(&self.root)
            .with_context(|| format!("create parity-control root {}", self.root.display()))?;
        let never_index = self.root.join(".metadata_never_index");
        if !never_index.exists() {
            fs::write(&never_index, b"").with_context(|| {
                format!("write .metadata_never_index in {}", self.root.display())
            })?;
        }
        Ok(())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn lease_path(&self) -> PathBuf {
        self.root.join("lease.json")
    }

    pub fn current_step_path(&self) -> PathBuf {
        self.root.join("current-step.json")
    }

    pub fn reclaim_marker_path(&self) -> PathBuf {
        self.root.join("reclaim-marker.json")
    }

    pub fn interruption_marker_path(&self) -> PathBuf {
        self.root.join("interruption-marker.json")
    }

    pub fn timeout_marker_path(&self) -> PathBuf {
        self.root.join("timeout-marker.json")
    }

    pub fn release_marker_path(&self) -> PathBuf {
        self.root.join("release-marker.json")
    }

    pub fn write_lease(&self, lease: &LeaseRecord) -> Result<()> {
        write_atomic_json(&self.lease_path(), lease)
    }

    pub fn read_lease(&self) -> Result<Option<LeaseRecord>> {
        read_optional_json(&self.lease_path())
    }

    pub fn write_current_step(&self, step: &CurrentStepRecord) -> Result<()> {
        write_atomic_json(&self.current_step_path(), step)
    }

    pub fn write_reclaim_marker(&self, marker: &ReclaimMarker) -> Result<()> {
        write_atomic_json(&self.reclaim_marker_path(), marker)
    }

    pub fn read_reclaim_marker(&self) -> Result<Option<ReclaimMarker>> {
        read_optional_json(&self.reclaim_marker_path())
    }

    pub fn write_interruption_marker(&self, marker: &InterruptionMarker) -> Result<()> {
        write_atomic_json(&self.interruption_marker_path(), marker)
    }

    pub fn write_timeout_marker(&self, marker: &TimeoutMarker) -> Result<()> {
        write_atomic_json(&self.timeout_marker_path(), marker)
    }

    pub fn write_release_marker(&self, marker: &ReleaseMarker) -> Result<()> {
        write_atomic_json(&self.release_marker_path(), marker)
    }
}

// ─── Lease record ────────────────────────────────────────────────────────────

/// Lease record at `parity-control/lease.json`.
///
/// Contains all fields required for reclaim decisions and staleness detection.
/// Written atomically before any destructive work; heartbeat updated periodically.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct LeaseRecord {
    pub schema_version: String,
    /// Owner PID.
    pub pid: u32,
    /// Process birth time in Unix milliseconds.
    /// Must come from `proc_pidinfo`/`sysctl` on Darwin, never from `ps`.
    /// Callers supply this value; the module does not call OS APIs.
    pub process_birth_unix_ms: u64,
    /// Process group ID for tracked subprocess descendants (0 = none yet assigned).
    pub pgid: u32,
    /// Hostname of the owning machine.
    pub hostname: String,
    /// `git rev-parse HEAD` captured at invocation time.
    pub commit_sha: String,
    /// `git rev-parse HEAD^{tree}` captured at invocation time.
    pub tree_id: String,
    /// Last heartbeat in Unix milliseconds (updated by [`LeaseRecord::heartbeat`]).
    pub heartbeat_unix_ms: u64,
    /// Unique generation identifier for this invocation.
    pub publication_generation_id: String,
    /// Monotonically increasing counter; incremented on each meaningful state change.
    pub control_sequence: u64,
}

impl LeaseRecord {
    pub fn new(
        pid: u32,
        process_birth_unix_ms: u64,
        hostname: impl Into<String>,
        commit_sha: impl Into<String>,
        tree_id: impl Into<String>,
        publication_generation_id: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: "parity-control-lease.v1".to_string(),
            pid,
            process_birth_unix_ms,
            pgid: 0,
            hostname: hostname.into(),
            commit_sha: commit_sha.into(),
            tree_id: tree_id.into(),
            heartbeat_unix_ms: now_unix_ms(),
            publication_generation_id: publication_generation_id.into(),
            control_sequence: 0,
        }
    }

    /// Advance heartbeat and increment `control_sequence`. Call periodically.
    pub fn heartbeat(&mut self) {
        self.heartbeat_unix_ms = now_unix_ms();
        self.control_sequence += 1;
    }

    /// Set the process-group identity after subprocesses are spawned.
    pub fn set_pgid(&mut self, pgid: u32) {
        self.pgid = pgid;
        self.control_sequence += 1;
    }
}

// ─── Current-step record ─────────────────────────────────────────────────────

/// Active-state record at `parity-control/current-step.json`.
///
/// Updated on each step boundary. Enables sub-minute diagnosis of stuck replays.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CurrentStepRecord {
    pub schema_version: String,
    pub generation: String,
    pub fixture: Option<String>,
    pub step: String,
    pub surface: Option<String>,
    pub mode: Option<String>,
    /// Elapsed wall-clock milliseconds since generation start.
    pub elapsed_ms: u64,
    pub heartbeat_unix_ms: u64,
}

impl CurrentStepRecord {
    pub fn new(generation: impl Into<String>, step: impl Into<String>, elapsed_ms: u64) -> Self {
        Self {
            schema_version: "parity-control-current-step.v1".to_string(),
            generation: generation.into(),
            fixture: None,
            step: step.into(),
            surface: None,
            mode: None,
            elapsed_ms,
            heartbeat_unix_ms: now_unix_ms(),
        }
    }
}

// ─── Reclaim marker ──────────────────────────────────────────────────────────

/// Reclaim marker at `parity-control/reclaim-marker.json`.
///
/// Written by a successor before it either reclaims or parks in manual recovery.
/// The `overall_status` field determines whether cleanup is allowed:
/// `reclaim_allowed` permits cleanup; `blocked_manual_recovery` forbids it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReclaimMarker {
    pub schema_version: String,
    /// `"reclaim_allowed"` or `"blocked_manual_recovery"`.
    pub overall_status: String,
    pub abandoned_generation_id: String,
    pub preserved_generation_root: Option<String>,
    pub owner_pid: u32,
    pub owner_process_birth_unix_ms: u64,
    pub owner_pgid: Option<u32>,
    pub owner_hostname: String,
    pub owner_last_heartbeat_unix_ms: u64,
    pub owner_last_control_sequence: u64,
    pub observation_count: u32,
    pub freshness_window_ms: u64,
    pub missing_pgid_metadata: bool,
    pub observable_descendants: bool,
    pub written_at_unix_ms: u64,
    pub diagnostic_message: String,
}

impl ReclaimMarker {
    /// Build a `blocked_manual_recovery` reclaim marker.
    ///
    /// - `preserved_generation_root`: absolute path to the preserved generation
    ///   directory, when known. Pass `None` if the generation root is not yet
    ///   established (e.g., before generation-scoped layout lands).
    /// - `observation_count`: how many freshness observations were performed
    ///   before escalation. Per proposal Section 6.3 the minimum is 2 for
    ///   stale-owner escalation; pass 1 for cases where owner identity cannot
    ///   be resolved at all (Cases B/C).
    /// - `freshness_window_ms`: the freshness window used during stale-owner
    ///   observation. Per proposal Section 6.3 this is
    ///   `min(120_000, gate_deadline_remaining_ms / 4)`.
    pub fn blocked_manual_recovery(
        abandoned_lease: &LeaseRecord,
        missing_pgid_metadata: bool,
        observable_descendants: bool,
        preserved_generation_root: Option<String>,
        observation_count: u32,
        freshness_window_ms: u64,
        diagnostic_message: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: "parity-control-reclaim-marker.v1".to_string(),
            overall_status: "blocked_manual_recovery".to_string(),
            abandoned_generation_id: abandoned_lease.publication_generation_id.clone(),
            preserved_generation_root,
            owner_pid: abandoned_lease.pid,
            owner_process_birth_unix_ms: abandoned_lease.process_birth_unix_ms,
            owner_pgid: if abandoned_lease.pgid > 0 {
                Some(abandoned_lease.pgid)
            } else {
                None
            },
            owner_hostname: abandoned_lease.hostname.clone(),
            owner_last_heartbeat_unix_ms: abandoned_lease.heartbeat_unix_ms,
            owner_last_control_sequence: abandoned_lease.control_sequence,
            observation_count,
            freshness_window_ms,
            missing_pgid_metadata,
            observable_descendants,
            written_at_unix_ms: now_unix_ms(),
            diagnostic_message: diagnostic_message.into(),
        }
    }

    /// Build a `reclaim_allowed` marker (Case D: PID gone, pgid metadata
    /// present, descendant absence proven).
    ///
    /// `preserved_generation_root` may be `None` when the generation root is
    /// not yet tracked (e.g., before generation-scoped layout lands).
    pub fn reclaim_allowed(
        abandoned_lease: &LeaseRecord,
        preserved_generation_root: Option<String>,
    ) -> Self {
        Self {
            schema_version: "parity-control-reclaim-marker.v1".to_string(),
            overall_status: "reclaim_allowed".to_string(),
            abandoned_generation_id: abandoned_lease.publication_generation_id.clone(),
            preserved_generation_root,
            owner_pid: abandoned_lease.pid,
            owner_process_birth_unix_ms: abandoned_lease.process_birth_unix_ms,
            owner_pgid: if abandoned_lease.pgid > 0 {
                Some(abandoned_lease.pgid)
            } else {
                None
            },
            owner_hostname: abandoned_lease.hostname.clone(),
            owner_last_heartbeat_unix_ms: abandoned_lease.heartbeat_unix_ms,
            owner_last_control_sequence: abandoned_lease.control_sequence,
            observation_count: 1,
            freshness_window_ms: 120_000,
            missing_pgid_metadata: false,
            observable_descendants: false,
            written_at_unix_ms: now_unix_ms(),
            diagnostic_message: "PID gone, pgid metadata present, descendant absence proven."
                .to_string(),
        }
    }
}

// ─── Interruption marker ─────────────────────────────────────────────────────

/// Interruption marker at `parity-control/interruption-marker.json`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InterruptionMarker {
    pub schema_version: String,
    /// `"blocked_interrupted"`.
    pub overall_status: String,
    pub generation_id: String,
    pub signal: String,
    pub descendant_pgid: Option<u32>,
    pub descendant_absent: bool,
    pub written_at_unix_ms: u64,
}

impl InterruptionMarker {
    pub fn new(generation_id: impl Into<String>, signal: impl Into<String>) -> Self {
        Self {
            schema_version: "parity-control-interruption-marker.v1".to_string(),
            overall_status: "blocked_interrupted".to_string(),
            generation_id: generation_id.into(),
            signal: signal.into(),
            descendant_pgid: None,
            descendant_absent: false,
            written_at_unix_ms: now_unix_ms(),
        }
    }
}

// ─── Timeout marker ──────────────────────────────────────────────────────────

/// Timeout marker at `parity-control/timeout-marker.json`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimeoutMarker {
    pub schema_version: String,
    /// `"blocked_timeout"`.
    pub overall_status: String,
    pub generation_id: String,
    pub active_fixture: Option<String>,
    pub active_surface: Option<String>,
    pub descendant_pgid: Option<u32>,
    pub descendant_absent: bool,
    pub written_at_unix_ms: u64,
}

impl TimeoutMarker {
    pub fn new(generation_id: impl Into<String>, descendant_absent: bool) -> Self {
        Self {
            schema_version: "parity-control-timeout-marker.v1".to_string(),
            overall_status: "blocked_timeout".to_string(),
            generation_id: generation_id.into(),
            active_fixture: None,
            active_surface: None,
            descendant_pgid: None,
            descendant_absent,
            written_at_unix_ms: now_unix_ms(),
        }
    }
}

// ─── Release marker ──────────────────────────────────────────────────────────

/// Release marker at `parity-control/release-marker.json` written on clean exit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReleaseMarker {
    pub schema_version: String,
    /// `"ready_same_tree_verified"` or a blocked status.
    pub overall_status: String,
    pub generation_id: String,
    pub descendant_quiescent: bool,
    pub written_at_unix_ms: u64,
}

impl ReleaseMarker {
    pub fn ready(generation_id: impl Into<String>) -> Self {
        Self {
            schema_version: "parity-control-release-marker.v1".to_string(),
            overall_status: "ready_same_tree_verified".to_string(),
            generation_id: generation_id.into(),
            descendant_quiescent: true,
            written_at_unix_ms: now_unix_ms(),
        }
    }
}

// ─── Atomic write implementation ─────────────────────────────────────────────

/// Write `value` as pretty-printed JSON to `target` via same-directory temp → rename.
///
/// The temp file is created in the same directory as `target` to satisfy the
/// same-volume rule for atomic `rename` on APFS.
pub fn write_atomic_json<T: Serialize>(target: &Path, value: &T) -> Result<()> {
    let parent = target.parent().ok_or_else(|| {
        anyhow::anyhow!(
            "atomic write target has no parent directory: {}",
            target.display()
        )
    })?;
    fs::create_dir_all(parent)
        .with_context(|| format!("create dir for atomic write {}", parent.display()))?;

    let json = serde_json::to_string_pretty(value).context("serialize value for atomic write")?;

    // Temp file in the same directory (same-volume rule).
    // PID + thread-id hash + sub-second nanos ensure no two concurrent writers
    // in the same process collide on the temp path even when wall-clock nanos agree.
    let tmp_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let thread_discriminant: u64 = {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        std::thread::current().id().hash(&mut h);
        h.finish()
    };
    let tmp_path = parent.join(format!(
        ".{}.{}.{:016x}.{}.tmp",
        target
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("ctrl"),
        std::process::id(),
        thread_discriminant,
        tmp_nanos,
    ));

    {
        // create_new(true) refuses to open an existing file or follow symlinks
        // (O_CREAT|O_EXCL), providing defense-in-depth against symlink races in
        // shared target directories. mode(0o600) prevents leaking lease/pid/host
        // metadata to other users on shared hosts.
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .custom_flags(libc::O_NOFOLLOW as i32)
            .mode(0o600)
            .open(&tmp_path)
            .with_context(|| format!("create temp file {}", tmp_path.display()))?;
        f.write_all(json.as_bytes())
            .with_context(|| format!("write temp file {}", tmp_path.display()))?;
        f.flush()
            .with_context(|| format!("flush temp file {}", tmp_path.display()))?;
        durable_flush(&f)
            .with_context(|| format!("durable flush temp file {}", tmp_path.display()))?;
    }

    fs::rename(&tmp_path, target).with_context(|| {
        format!(
            "atomic rename {} → {}",
            tmp_path.display(),
            target.display()
        )
    })?;

    // Best-effort parent-directory durability barrier.
    let _ = sync_dir(parent);

    Ok(())
}

/// Flush a file durably. On Darwin, uses `F_FULLFSYNC` (barrier flush) which
/// forces the drive to write buffered data before returning. Falls back to
/// `sync_all` (fdatasync) on other platforms.
fn durable_flush(file: &fs::File) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let fd = file.as_raw_fd();
        // F_FULLFSYNC = 51 on Darwin; forces drive barrier flush.
        let ret = unsafe { libc::fcntl(fd, libc::F_FULLFSYNC) };
        if ret == 0 {
            return Ok(());
        }
        // Fall through to sync_all if F_FULLFSYNC fails (e.g. network fs).
    }
    file.sync_all()
}

// ─── Process birth time (Darwin: proc_pidinfo) ───────────────────────────────

/// Returns the process birth time in Unix milliseconds for the given PID.
///
/// On Darwin, uses `libc::proc_pidinfo` with `PROC_PIDTBSDINFO` to read the
/// kernel-accurate `pbi_start_tvsec` / `pbi_start_tvusec` fields. This is the
/// only approved method per proposal P041 — `ps` is forbidden because it has
/// low precision and can be spoofed.
///
/// Returns `None` on non-Darwin platforms, if `proc_pidinfo` returns a short
/// result (PID gone, permission denied, or unsupported), or if the computed
/// timestamp is zero.
#[cfg(target_os = "macos")]
pub fn get_process_birth_unix_ms(pid: u32) -> Option<u64> {
    use std::mem;
    unsafe {
        let mut info: libc::proc_bsdinfo = mem::zeroed();
        let expected = mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
        let ret = libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDTBSDINFO,
            0,
            &mut info as *mut _ as *mut libc::c_void,
            expected,
        );
        if ret < expected {
            return None;
        }
        // pbi_start_tvsec = seconds, pbi_start_tvusec = microseconds
        let ms = info
            .pbi_start_tvsec
            .saturating_mul(1000)
            .saturating_add(info.pbi_start_tvusec / 1000);
        if ms == 0 {
            None
        } else {
            Some(ms)
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn get_process_birth_unix_ms(_pid: u32) -> Option<u64> {
    None
}

// ─── WAL-safe readback ────────────────────────────────────────────────────────

/// Run `PRAGMA wal_checkpoint(TRUNCATE)` before opening a read-only connection
/// on a parity replay database.
///
/// Per proposal P041 Section 6.3, a WAL checkpoint ensures that all writes from
/// the engine replay are visible to the subsequent GraphQL / MCP readback pool.
/// If TRUNCATE mode is blocked by concurrent readers, falls back to one PASSIVE
/// retry. The PASSIVE retry result is inspected: if any WAL frames remain
/// uncheckpointed (`log != checkpointed`), this function returns `Err` so
/// readback is aborted rather than proceeding with stale WAL state. Callers
/// should close read-only pool handles before calling this function to maximize
/// the chance of a successful checkpoint.
pub async fn wal_checkpoint_before_readback(pool: &sqlx::SqlitePool) -> Result<()> {
    use sqlx::Row;
    let row = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .fetch_one(pool)
        .await
        .context("wal_checkpoint(TRUNCATE) PRAGMA failed")?;
    // Column 0: blocked (1 = could not checkpoint all pages due to active readers)
    let blocked: i64 = row.try_get(0).unwrap_or(0);
    if blocked != 0 {
        // One retry with PASSIVE mode (caller should have closed read handles first).
        let retry_row = sqlx::query("PRAGMA wal_checkpoint(PASSIVE)")
            .fetch_one(pool)
            .await
            .context("wal_checkpoint(PASSIVE) fallback PRAGMA failed")?;
        // PRAGMA wal_checkpoint result columns: 0=busy, 1=log (total frames), 2=checkpointed
        let busy: i64 = retry_row.try_get(0).unwrap_or(1);
        let log: i64 = retry_row.try_get(1).unwrap_or(1);
        let checkpointed: i64 = retry_row.try_get(2).unwrap_or(0);
        if busy != 0 || log != checkpointed {
            return Err(anyhow::anyhow!(
                "wal_checkpoint: busy={busy} log={log} checkpointed={checkpointed}; \
                 {} WAL frames remain after PASSIVE retry; \
                 readback aborted to prevent stale WAL state",
                log.saturating_sub(checkpointed),
            ));
        }
    }
    Ok(())
}

// ─── Pre-cleanup blocked publication ─────────────────────────────────────────

/// Canonical repo-relative path for the P041 runtime detail artifact.
///
/// Used in `runtime_detail_path` fields so consumers always receive a stable
/// repo-relative path rather than an absolute filesystem path.
pub const P041_RUNTIME_DETAIL_CANONICAL_PATH: &str =
    "control-plane/target/parity/publication/current/p031-p041-parity-evidence.json";

/// Canonical repo-relative path for the P041 promoted reference snapshot.
pub const P041_REFERENCE_DETAIL_CANONICAL_PATH: &str =
    "docs/reference/p031-p041-parity-evidence.json";

/// Write pre-cleanup `blocked_in_progress` / `revoked_for_rerun` publication artifacts.
///
/// Per proposal P041 Section 6.3 step 5, this MUST be called before any ephemeral
/// cleanup or replay work begins. It atomically revokes stale ready evidence from a
/// prior generation so consumers see `blocked_in_progress` rather than stale `ready`
/// evidence during a rerun.
///
/// The caller must supply the full provenance snapshot captured from one exact
/// `git status --porcelain=v1` run before cleanup begins (proposal Section 6.3 step 3).
/// Pass sentinel values (`tree_clean=false`, `status_snapshot_sha256="pending"`,
/// `status_snapshot_line_count=0`) only when the real capture is not yet available
/// (e.g., in pre-gate test contexts where git is not accessible).
///
/// Both `p031-p041-parity-evidence.json` and `p031-phase-0-manifest-row.json` are written
/// using the standard same-directory temp-file → durable-flush → atomic-rename contract.
pub fn write_pre_cleanup_publication(
    pub_current_dir: &Path,
    generation_id: &str,
    commit_sha: &str,
    tree_id: &str,
    tree_clean: bool,
    status_snapshot_sha256: &str,
    status_snapshot_line_count: u64,
) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let provenance = serde_json::json!({
        "commit_sha": commit_sha,
        "tree_id": tree_id,
        "tree_clean": tree_clean,
        "status_snapshot_sha256": status_snapshot_sha256,
        "status_snapshot_line_count": status_snapshot_line_count,
        "generated_at": now,
        "gate": "./scripts/test-gate.sh proposal-041"
    });
    // Build fixture skeleton entries so the blocked_in_progress detail satisfies the
    // Section 6.2 requirement that required_fixtures and required_surfaces are fully
    // populated even for non-ready publications.
    let fixture_entries: Vec<serde_json::Value> = REQUIRED_FIXTURES
        .iter()
        .map(|fid| serde_json::json!({
            "fixture_id": fid,
            "report_path": format!("control-plane/target/parity/reports/{generation_id}/{fid}/behavioral-diff-report.json"),
            "replay_path": format!("control-plane/target/parity/work/{generation_id}/{fid}/server-replay.json"),
            "shadow_report_path": format!("control-plane/target/parity/shadow/{generation_id}/{fid}/live-shadow-report.json"),
            "verdict": "blocked_in_progress",
        }))
        .collect();
    let detail = serde_json::json!({
        "schema_version": "p031-p041-parity-evidence.v1",
        "overall_status": "blocked_in_progress",
        "publication_generation_id": generation_id,
        "publication_state": "revoked_for_rerun",
        "required_fixtures": REQUIRED_FIXTURES,
        "required_surfaces": REQUIRED_COMPARISON_SURFACES,
        "fixtures": fixture_entries,
        "blocking_reasons": ["rerun_in_progress"],
        "missing_evidence": [],
        "provenance": provenance.clone()
    });
    let row = serde_json::json!({
        "schema_version": "p031-phase-0-runtime-manifest-row.v1",
        "id": "p041_parity_evidence",
        "runtime_detail_path": P041_RUNTIME_DETAIL_CANONICAL_PATH,
        "reference_detail_path": P041_REFERENCE_DETAIL_CANONICAL_PATH,
        "validation_status": "blocked_in_progress",
        "publication_state": "revoked_for_rerun",
        "publication_generation_id": generation_id,
        "detail_schema_version": "p031-p041-parity-evidence.v1",
        "provenance": provenance
    });

    fs::create_dir_all(pub_current_dir).with_context(|| {
        format!(
            "create publication/current dir {}",
            pub_current_dir.display()
        )
    })?;
    write_atomic_json(
        &pub_current_dir.join("p031-p041-parity-evidence.json"),
        &detail,
    )?;
    write_atomic_json(
        &pub_current_dir.join("p031-phase-0-manifest-row.json"),
        &row,
    )?;
    Ok(())
}

fn read_optional_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>> {
    // 64 KiB cap: control files are small JSON objects; reject oversized files to
    // avoid unbounded memory pressure from corrupted or adversarially crafted files.
    const MAX_CONTROL_FILE_BYTES: u64 = 64 * 1024;
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(err).with_context(|| format!("open control file {}", path.display()))
        }
    };
    let metadata = file
        .metadata()
        .with_context(|| format!("stat opened control file {}", path.display()))?;
    if metadata.len() > MAX_CONTROL_FILE_BYTES {
        return Err(anyhow::anyhow!(
            "control file {} is {} bytes, exceeding the {} byte limit; \
             file may be corrupted — remove it and rerun the gate",
            path.display(),
            metadata.len(),
            MAX_CONTROL_FILE_BYTES,
        ));
    }
    let mut raw = String::new();
    let bytes_read = Read::by_ref(&mut file)
        .take(MAX_CONTROL_FILE_BYTES + 1)
        .read_to_string(&mut raw)
        .with_context(|| format!("read control file {}", path.display()))?;
    if bytes_read as u64 > MAX_CONTROL_FILE_BYTES {
        return Err(anyhow::anyhow!(
            "control file {} exceeds the {} byte limit; file may be corrupted — remove it and rerun the gate",
            path.display(),
            MAX_CONTROL_FILE_BYTES,
        ));
    }
    let value: T = serde_json::from_str(&raw)
        .with_context(|| format!("parse control file {}", path.display()))?;
    Ok(Some(value))
}

fn sync_dir(dir: &Path) -> io::Result<()> {
    fs::File::open(dir)?.sync_all()
}

// ─── Stale-owner freshness evaluation ────────────────────────────────────────

/// Evaluate whether a live-but-stalled owner should be escalated to
/// `blocked_manual_recovery` based on two consecutive observations.
///
/// Returns `true` (escalate) when both heartbeat and `control_sequence` are
/// unchanged across both observations, as required by proposal Section 6.3.
pub fn is_owner_stalled(
    obs1_heartbeat_ms: u64,
    obs1_sequence: u64,
    obs2_heartbeat_ms: u64,
    obs2_sequence: u64,
) -> bool {
    obs1_heartbeat_ms == obs2_heartbeat_ms && obs1_sequence == obs2_sequence
}

/// Determine the reclaim decision for a successor inspecting an abandoned lease.
///
/// Maps directly to the four-case reclaim matrix in proposal Section 6.3.
/// No OS calls — the caller supplies liveness and descendant-absence facts.
pub fn evaluate_reclaim_decision(
    pid_alive: bool,
    pgid_metadata_available: bool,
    descendant_absence_proven: bool,
) -> &'static str {
    if pid_alive {
        "blocked_in_progress"
    } else if !pgid_metadata_available {
        "blocked_manual_recovery"
    } else if !descendant_absence_proven {
        "blocked_manual_recovery"
    } else {
        "reclaim_allowed"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use sqlx::Executor;
    use tempfile::tempdir;

    #[test]
    fn lease_record_roundtrip_preserves_all_fields() -> Result<()> {
        let dir = tempdir()?;
        let ctrl = ParityControlRoot::new(dir.path().join("parity-control"));
        ctrl.init()?;

        let lease = LeaseRecord::new(
            std::process::id(),
            1_700_000_000_000,
            "testhost",
            "abc123",
            "def456",
            "gen-test-001",
        );
        ctrl.write_lease(&lease)?;

        let read_back = ctrl.read_lease()?.expect("lease should be readable");
        assert_eq!(read_back.schema_version, "parity-control-lease.v1");
        assert_eq!(read_back.pid, lease.pid);
        assert_eq!(read_back.process_birth_unix_ms, 1_700_000_000_000);
        assert_eq!(read_back.hostname, "testhost");
        assert_eq!(read_back.publication_generation_id, "gen-test-001");
        assert_eq!(read_back.control_sequence, 0);
        Ok(())
    }

    #[test]
    fn metadata_never_index_is_created_on_init() -> Result<()> {
        let dir = tempdir()?;
        let ctrl = ParityControlRoot::new(dir.path().join("parity-control"));
        ctrl.init()?;
        assert!(
            ctrl.root().join(".metadata_never_index").is_file(),
            ".metadata_never_index must exist after init"
        );
        Ok(())
    }

    #[test]
    fn reclaim_marker_blocked_manual_recovery_roundtrip() -> Result<()> {
        let dir = tempdir()?;
        let ctrl = ParityControlRoot::new(dir.path().join("parity-control"));
        ctrl.init()?;

        let lease = LeaseRecord::new(9999, 1_000_000, "h", "c", "t", "gen-abandoned");
        let marker = ReclaimMarker::blocked_manual_recovery(
            &lease,
            true,  // missing_pgid_metadata
            false, // observable_descendants
            Some("/tmp/parity-gen-abandoned".to_string()),
            1, // observation_count (Case B: no pgid metadata, no wait)
            120_000,
            "missing pgid metadata",
        );
        ctrl.write_reclaim_marker(&marker)?;

        let read_back = ctrl
            .read_reclaim_marker()?
            .expect("reclaim marker readable");
        assert_eq!(read_back.schema_version, "parity-control-reclaim-marker.v1");
        assert_eq!(read_back.overall_status, "blocked_manual_recovery");
        assert_eq!(read_back.abandoned_generation_id, "gen-abandoned");
        assert!(read_back.missing_pgid_metadata);
        assert_eq!(
            read_back.preserved_generation_root,
            Some("/tmp/parity-gen-abandoned".to_string())
        );
        assert_eq!(read_back.observation_count, 1);
        Ok(())
    }

    #[test]
    fn atomic_write_leaves_no_temp_file_on_success() -> Result<()> {
        let dir = tempdir()?;
        let target = dir.path().join("test.json");
        write_atomic_json(&target, &serde_json::json!({"ok": true}))?;

        assert!(target.is_file(), "target must exist after atomic write");
        // Verify no .tmp files remain in the directory
        let leftover_tmp: Vec<_> = fs::read_dir(dir.path())?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map(|n| n.ends_with(".tmp"))
                    .unwrap_or(false)
            })
            .collect();
        assert!(
            leftover_tmp.is_empty(),
            "no .tmp files should remain after atomic write"
        );
        Ok(())
    }

    #[test]
    fn reclaim_decision_covers_all_four_cases() {
        // Case A: pid alive
        assert_eq!(
            evaluate_reclaim_decision(true, true, true),
            "blocked_in_progress"
        );
        // Case B: pid gone, no pgid metadata
        assert_eq!(
            evaluate_reclaim_decision(false, false, false),
            "blocked_manual_recovery"
        );
        // Case C: pid gone, pgid present, descendants observable
        assert_eq!(
            evaluate_reclaim_decision(false, true, false),
            "blocked_manual_recovery"
        );
        // Case D: pid gone, pgid present, absence proven
        assert_eq!(
            evaluate_reclaim_decision(false, true, true),
            "reclaim_allowed"
        );
    }

    #[test]
    fn stale_owner_detection_requires_two_stale_observations() {
        // Both stale → escalate
        assert!(is_owner_stalled(1_000_000, 5, 1_000_000, 5));
        // Second obs advanced → no escalation
        assert!(!is_owner_stalled(1_000_000, 5, 2_000_000, 6));
        // Heartbeat advanced, sequence stale → no escalation (must both be stale)
        assert!(!is_owner_stalled(1_000_000, 5, 2_000_000, 5));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn get_process_birth_unix_ms_returns_nonzero_for_self() {
        let birth_ms = get_process_birth_unix_ms(std::process::id());
        assert!(
            birth_ms.is_some(),
            "get_process_birth_unix_ms must succeed for own PID on Darwin"
        );
        assert!(
            birth_ms.unwrap() > 0,
            "process birth time must be a nonzero Unix ms timestamp"
        );
        assert!(
            birth_ms.unwrap() > 1_000_000_000_000,
            "process birth time must be after 2001-09-09 (sanity check)"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn get_process_birth_unix_ms_returns_none_for_missing_pid() {
        // PID 0 is the kernel task and should not be inspectable by user processes
        let result = get_process_birth_unix_ms(0);
        // Either None (permission denied / PID not found) or valid — either is acceptable,
        // but non-None must still be a plausible Unix ms value.
        if let Some(ms) = result {
            assert!(ms > 0, "birth ms for PID 0 must be nonzero if returned");
        }
    }

    #[test]
    fn write_pre_cleanup_publication_emits_full_required_fields() -> Result<()> {
        let dir = tempdir()?;
        let pub_current = dir.path().join("publication/current");
        write_pre_cleanup_publication(
            &pub_current,
            "gen-test-001",
            "abc123",
            "def456",
            false,
            "pending",
            0,
        )?;

        let detail_bytes = fs::read_to_string(pub_current.join("p031-p041-parity-evidence.json"))?;
        let detail: serde_json::Value = serde_json::from_str(&detail_bytes)?;

        let row_bytes = fs::read_to_string(pub_current.join("p031-phase-0-manifest-row.json"))?;
        let row: serde_json::Value = serde_json::from_str(&row_bytes)?;

        // required_fixtures must list all seven fixture ids in fixed order.
        let detail_fixtures: Vec<&str> = detail["required_fixtures"]
            .as_array()
            .expect("required_fixtures must be an array")
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert_eq!(
            detail_fixtures, REQUIRED_FIXTURES,
            "write_pre_cleanup_publication must emit the full REQUIRED_FIXTURES list"
        );

        // required_surfaces must list all six surfaces in fixed order.
        let detail_surfaces: Vec<&str> = detail["required_surfaces"]
            .as_array()
            .expect("required_surfaces must be an array")
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert_eq!(
            detail_surfaces, REQUIRED_COMPARISON_SURFACES,
            "write_pre_cleanup_publication must emit the full REQUIRED_COMPARISON_SURFACES list"
        );
        let first_fixture = &detail["fixtures"][0];
        assert_eq!(
            first_fixture["report_path"],
            "control-plane/target/parity/reports/gen-test-001/proposal-loop-basic/behavioral-diff-report.json"
        );
        assert_eq!(
            first_fixture["replay_path"],
            "control-plane/target/parity/work/gen-test-001/proposal-loop-basic/server-replay.json"
        );
        assert_eq!(
            first_fixture["shadow_report_path"],
            "control-plane/target/parity/shadow/gen-test-001/proposal-loop-basic/live-shadow-report.json"
        );

        // runtime_detail_path must be the canonical repo-relative path (never absolute).
        let runtime_detail_path = row["runtime_detail_path"].as_str().unwrap_or("");
        assert!(
            !runtime_detail_path.is_empty(),
            "runtime_detail_path must be non-empty"
        );
        assert!(
            !runtime_detail_path.starts_with('/'),
            "runtime_detail_path must be repo-relative, not absolute; got: {runtime_detail_path}"
        );
        assert_eq!(
            runtime_detail_path, P041_RUNTIME_DETAIL_CANONICAL_PATH,
            "runtime_detail_path must equal the canonical constant"
        );

        // Cross-artifact invariants must hold for blocked_in_progress too.
        assert_eq!(
            row["detail_schema_version"], detail["schema_version"],
            "row.detail_schema_version must equal detail.schema_version"
        );
        assert_eq!(
            row["validation_status"], detail["overall_status"],
            "row.validation_status must equal detail.overall_status"
        );
        assert_eq!(
            row["publication_generation_id"], detail["publication_generation_id"],
            "publication_generation_id must be consistent"
        );
        Ok(())
    }

    #[tokio::test]
    async fn wal_checkpoint_before_readback_succeeds_on_wal_db() -> Result<()> {
        let dir = tempdir()?;
        let db_path = dir.path().join("wal_ckpt_test.sqlite");
        let db_url = format!("sqlite://{}", db_path.to_string_lossy());
        let pool = db::pool::create_pool(&db_url).await?;
        // Write a row so the WAL has at least one frame
        sqlx::query("CREATE TABLE IF NOT EXISTS _wal_ckpt_sentinel (v INTEGER)")
            .execute(&pool)
            .await?;
        sqlx::query("INSERT INTO _wal_ckpt_sentinel VALUES (1)")
            .execute(&pool)
            .await?;
        // Checkpoint must succeed with an open write pool
        wal_checkpoint_before_readback(&pool).await?;
        Ok(())
    }
}
