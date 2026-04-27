//! Daemon lifecycle types (Proposal 042 §4.1).
//!
//! Shared identity for the daemon's readback surfaces (`/health`, `/ready`,
//! `daemonStatus` query, `daemonStatusChanged` subscription). Defined in
//! `domain` so that `graphql-server`, `daemon`, Swift clients, and test
//! fixtures all resolve the same shape without duplicating a dialect of it.
//!
//! The critical type-level contract is the split between [`DegradedKind`]
//! (non-terminal, recoverable) and [`FailureKind`] (terminal). A subsystem
//! that reports `BackupFailed` cannot accidentally land in the `Degraded`
//! list, and a slow subsystem that reports `BackgroundExecutorStalled`
//! cannot accidentally trigger a terminal failure. Per P042 §4.1 every
//! `FailureKind` variant has an AC-12 round-trip test.
//!
//! `DaemonStatus::failure` is `Some` iff `state == Failed`. The invariant
//! is enforced by the lifecycle reporter (`daemon::lifecycle_reporter`)
//! rather than the type system, and is asserted by
//! `test_daemon_status_failure_field_populated_only_when_failed`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Process-wide lifecycle phases. Transitions are owned by
/// `daemon::lifecycle_reporter`; the enum is intentionally flat so wire
/// serialization via `snake_case` is unambiguous.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DaemonLifecycleState {
    NotStarted,
    Starting,
    Ready,
    Degraded,
    Restarting,
    Failed,
    Shutdown,
}

impl DaemonLifecycleState {
    /// True iff the daemon is actively serving client requests (`Ready`
    /// accepts anything; `Degraded` accepts status-only per §5.2; the
    /// rest are not serving).
    pub fn is_serving(self) -> bool {
        matches!(self, Self::Ready | Self::Degraded)
    }

    /// True iff this state is "alive" from a supervisor's liveness
    /// probe point of view — per §5.2 status-code matrix, `Degraded`
    /// is alive (must not trigger restart).
    pub fn is_live(self) -> bool {
        matches!(self, Self::Ready | Self::Degraded)
    }

    /// True iff the process has hit a terminal state for its current
    /// lifetime. Recovery requires a restart.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Failed | Self::Shutdown)
    }
}

impl std::fmt::Display for DaemonLifecycleState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::NotStarted => "not_started",
            Self::Starting => "starting",
            Self::Ready => "ready",
            Self::Degraded => "degraded",
            Self::Restarting => "restarting",
            Self::Failed => "failed",
            Self::Shutdown => "shutdown",
        };
        f.write_str(s)
    }
}

/// Reasons a daemon may be alive but not fully healthy. Every variant is
/// recoverable without a restart; reaching `Ready` again clears the
/// condition without operator action in most cases.
///
/// Explicitly **not** in this enum: anything terminal. A reviewer checking
/// a PR should treat a new `DegradedKind` variant as requiring a matching
/// entry in the Swift client's `Degraded` banner copy. Additions require
/// a proposal amendment per P042 §4.1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DegradedKind {
    BackgroundExecutorStalled,
    AcpRuntimeUnavailable,
    StaleProjection,
    AuthPrincipalTableUnreadable,
    DiskSpaceLow,
}

impl std::fmt::Display for DegradedKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::BackgroundExecutorStalled => "background_executor_stalled",
            Self::AcpRuntimeUnavailable => "acp_runtime_unavailable",
            Self::StaleProjection => "stale_projection",
            Self::AuthPrincipalTableUnreadable => "auth_principal_table_unreadable",
            Self::DiskSpaceLow => "disk_space_low",
        };
        f.write_str(s)
    }
}

/// Reasons a daemon transitioned to `Failed`. Every variant is **terminal
/// for the current process** AND **observable via the daemon's own readback
/// surfaces** (`/health`, `daemonStatus`). Startup errors that prevent the
/// daemon from binding HTTP at all — e.g. an anomalous PID lock state
/// (§6.1) — are NOT represented here; they are supervisor-owned and
/// surfaced via process exit code + app dialog.
///
/// Every variant has an AC-12 round-trip test in
/// `daemon/tests/failure_round_trip.rs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    /// DB migration preflight, backup, lock, or apply failed. Operator
    /// investigates the detail + `backup_path` and decides whether to
    /// re-run or restore.
    MigrationFailed,
    /// DB has migration versions above the binary's compile-time max.
    /// Fail-closed; no downgrade, no backup.
    SchemaNewerThanBinary,
    /// Backup copy step failed before any migration ran. Original DB is
    /// intact; operator fixes the backup directory and retries.
    BackupFailed,
    /// 5 crashes within 60 s. Daemon stays alive in degraded-serve mode
    /// (§6.2) until operator clears `crash-budget.json`.
    CrashLoopBudgetExhausted,
}

impl std::fmt::Display for FailureKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::MigrationFailed => "migration_failed",
            Self::SchemaNewerThanBinary => "schema_newer_than_binary",
            Self::BackupFailed => "backup_failed",
            Self::CrashLoopBudgetExhausted => "crash_loop_budget_exhausted",
        };
        f.write_str(s)
    }
}

/// Non-terminal reason the daemon is currently degraded. Timestamped so
/// the Swift client can display duration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DegradedReason {
    pub kind: DegradedKind,
    pub detail: String,
    pub since: DateTime<Utc>,
}

/// Terminal failure reason. `backup_path` is populated for
/// `MigrationFailed` and `BackupFailed` when a backup was successfully
/// written before the failure point — the operator needs an absolute path
/// to roll back.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FailureReason {
    pub kind: FailureKind,
    pub detail: String,
    pub since: DateTime<Utc>,
    pub backup_path: Option<String>,
}

/// Health state for the daemon-owned Xcode MCP broker. This is carried on
/// `DaemonStatus` so app clients can render broker readiness without calling
/// the broker route directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum XcodeBrokerHealthState {
    Disabled,
    Healthy,
    Degraded,
    Failed,
}

/// Point-in-time Xcode broker pool health.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XcodeBrokerHealthSnapshot {
    pub state: XcodeBrokerHealthState,
    pub reason_code: String,
    pub can_acquire_new_xcode_leases: bool,
    pub active_lease_count: usize,
    pub initialize_queue_depth: usize,
    pub last_transition_at: String,
    pub operator_message: String,
    pub pool_id: String,
    pub active_leases: usize,
    pub queued_leases: usize,
    pub max_active_leases: usize,
    pub max_queued_leases: usize,
    pub broker_disabled: bool,
    pub backend_available: bool,
    pub observation_persistence_failures: u64,
    pub stale_lease_count: usize,
    pub backend_session_count: usize,
    pub helper_cleanup_reaped_leases_total: u64,
}

/// In-process snapshot of daemon status. Emitted on every lifecycle
/// transition via `engine::event_bus` as `DomainEvent::DaemonStatusChanged`
/// and surfaced through `/health`, `/ready`, `daemonStatus`, and
/// `daemonStatusChanged`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DaemonStatus {
    pub state: DaemonLifecycleState,
    /// Applied migration version read from `_sqlx_migrations` at startup.
    /// 0 when the DB is fresh / not yet opened.
    pub schema_version: u32,
    /// Compile-time constant derived from the `migrations/` directory.
    pub binary_schema_version: u32,
    /// Short git SHA of the built binary. Empty string if the build did
    /// not embed one (cargo dev builds without a SHA env).
    pub build_sha: String,
    /// UTC timestamp of the most recent successful `Starting → Ready`
    /// transition, or `None` if the daemon has not yet reached `Ready`.
    pub started_at: Option<DateTime<Utc>>,
    pub last_state_change_at: DateTime<Utc>,
    /// Non-empty iff `state == Degraded`. Insertion-ordered: first reason
    /// is the one that caused the transition out of `Ready`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub degraded: Vec<DegradedReason>,
    /// `Some` iff `state == Failed`. Exactly one failure per terminal
    /// transition — the daemon does not accumulate failures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<FailureReason>,
    /// Optional Xcode broker health. Omitted until the daemon composition root
    /// mounts the broker pool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xcode_broker_health: Option<XcodeBrokerHealthSnapshot>,
    pub restart_count_since_boot: u32,
    pub pid: u32,
}

impl DaemonStatus {
    /// Initial status at process start, before any lifecycle transition
    /// has occurred. Callers should immediately transition this to
    /// `Starting` via the lifecycle reporter.
    pub fn initial(binary_schema_version: u32, build_sha: impl Into<String>) -> Self {
        Self {
            state: DaemonLifecycleState::NotStarted,
            schema_version: 0,
            binary_schema_version,
            build_sha: build_sha.into(),
            started_at: None,
            last_state_change_at: Utc::now(),
            degraded: Vec::new(),
            failure: None,
            xcode_broker_health: None,
            restart_count_since_boot: 0,
            pid: std::process::id(),
        }
    }

    /// Invariant check used by tests: `failure` is `Some` iff `state`
    /// is `Failed`. Returns `Ok(())` on valid, `Err(&'static str)` with
    /// the violated rule otherwise.
    pub fn check_failure_invariant(&self) -> Result<(), &'static str> {
        match (self.state, &self.failure) {
            (DaemonLifecycleState::Failed, Some(_)) => Ok(()),
            (DaemonLifecycleState::Failed, None) => Err("state == Failed but failure is None"),
            (_, Some(_)) => Err("state != Failed but failure is Some"),
            (_, None) => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_lifecycle_state_serializes_snake_case() {
        let ready = serde_json::to_string(&DaemonLifecycleState::Ready).unwrap();
        assert_eq!(ready, "\"ready\"");
        let sntbb = serde_json::to_string(&FailureKind::SchemaNewerThanBinary).unwrap();
        assert_eq!(sntbb, "\"schema_newer_than_binary\"");
    }

    #[test]
    fn daemon_lifecycle_state_predicates() {
        assert!(DaemonLifecycleState::Ready.is_live());
        assert!(DaemonLifecycleState::Degraded.is_live());
        assert!(!DaemonLifecycleState::Starting.is_live());
        assert!(!DaemonLifecycleState::Failed.is_live());

        assert!(DaemonLifecycleState::Ready.is_serving());
        assert!(DaemonLifecycleState::Degraded.is_serving());
        assert!(!DaemonLifecycleState::Failed.is_serving());

        assert!(DaemonLifecycleState::Failed.is_terminal());
        assert!(DaemonLifecycleState::Shutdown.is_terminal());
        assert!(!DaemonLifecycleState::Ready.is_terminal());
    }

    #[test]
    fn degraded_kind_and_failure_kind_are_disjoint_type_level() {
        // This test does not assert runtime behavior; it encodes the
        // type-level contract from §4.1: a value that is `DegradedKind`
        // cannot be silently used as a `FailureKind`. The compiler
        // enforces this automatically; this test serves as a reminder
        // that adding a variant to one enum never implicitly adds it
        // to the other.
        let _d: DegradedKind = DegradedKind::BackgroundExecutorStalled;
        let _f: FailureKind = FailureKind::MigrationFailed;
    }

    #[test]
    fn daemon_status_initial_has_no_failure() {
        let s = DaemonStatus::initial(14, "abc123");
        assert_eq!(s.state, DaemonLifecycleState::NotStarted);
        assert!(s.failure.is_none());
        assert!(s.degraded.is_empty());
        assert_eq!(s.binary_schema_version, 14);
        assert_eq!(s.build_sha, "abc123");
        assert!(s.check_failure_invariant().is_ok());
    }

    #[test]
    fn daemon_status_failure_invariant_catches_violations() {
        let mut s = DaemonStatus::initial(14, "sha");
        // state=NotStarted with Some(failure) is invalid.
        s.failure = Some(FailureReason {
            kind: FailureKind::MigrationFailed,
            detail: "forced".into(),
            since: Utc::now(),
            backup_path: None,
        });
        assert!(s.check_failure_invariant().is_err());
        // state=Failed with None is invalid.
        s.state = DaemonLifecycleState::Failed;
        s.failure = None;
        assert!(s.check_failure_invariant().is_err());
        // state=Failed with Some is valid.
        s.failure = Some(FailureReason {
            kind: FailureKind::MigrationFailed,
            detail: "ok".into(),
            since: Utc::now(),
            backup_path: Some("/tmp/db.backup-1-v0-to-v14.sqlite".into()),
        });
        assert!(s.check_failure_invariant().is_ok());
    }

    #[test]
    fn failure_reason_backup_path_round_trips_through_json() {
        let r = FailureReason {
            kind: FailureKind::MigrationFailed,
            detail: "CREATE TABLE foo: unique constraint violation".into(),
            since: Utc::now(),
            backup_path: Some("/Users/~/.../db.backup-1713435900-v12-to-v14.sqlite".into()),
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"kind\":\"migration_failed\""));
        assert!(json.contains("backup_path"));
        let back: FailureReason = serde_json::from_str(&json).unwrap();
        assert_eq!(back.backup_path, r.backup_path);
    }

    #[test]
    fn daemon_status_omits_empty_degraded_and_none_failure_in_json() {
        let s = DaemonStatus::initial(14, "sha");
        let json = serde_json::to_string(&s).unwrap();
        assert!(
            !json.contains("degraded"),
            "empty degraded should be omitted: {json}"
        );
        assert!(
            !json.contains("failure"),
            "None failure should be omitted: {json}"
        );
    }
}
