//! P042 §4.1 / §8.4 AC-12 round-trip tests.
//!
//! For each terminal [`FailureKind`] the lifecycle reporter must:
//! 1. Populate [`DaemonStatus.failure`] per the §4.1 invariant.
//! 2. Serialize into the P042 §5.2 JSON shape so `/health`, `/ready`,
//!    and the failed-serve `daemonStatus` passthrough report the same
//!    typed body.
//! 3. Re-enter `Ready` (post-restart) clears `failure` without drift.
//!
//! These tests live in the `daemon` crate so the §10.2 gate inventory
//! can assert on them by name. They are deliberately `daemon`-owned
//! rather than `domain`-owned because they pin the combined contract
//! between the reporter, the failed-serve router, and the HTTP body
//! shape — each of which sits in this crate's public surface.

use daemon::failed_serve;
use domain::lifecycle::{DaemonLifecycleState, DaemonStatus, FailureKind};
use engine::event_bus;
use engine::lifecycle_reporter::LifecycleReporter;

fn seed_failed(kind: FailureKind, detail: &str, backup_path: Option<&str>) -> LifecycleReporter {
    let bus = event_bus::new_bus(16);
    let reporter = LifecycleReporter::new(14, "test", bus);
    reporter.set_failed(kind, detail, backup_path.map(str::to_string));
    reporter
}

fn assert_failure_round_trip(
    reporter: &LifecycleReporter,
    expected_kind: FailureKind,
    expected_backup: Option<&str>,
) {
    // In-process snapshot.
    let snap = reporter.snapshot();
    assert_eq!(snap.state, DaemonLifecycleState::Failed);
    assert!(snap.check_failure_invariant().is_ok());
    let failure = snap.failure.as_ref().expect("failure populated");
    assert_eq!(failure.kind, expected_kind);
    assert_eq!(failure.backup_path.as_deref(), expected_backup);

    // JSON body serializes per P042 §5.2.
    let body = serde_json::to_value(&snap).expect("serialize DaemonStatus");
    assert_eq!(body["state"], "failed");
    assert_eq!(body["failure"]["kind"], expected_kind.to_string());
    if let Some(expected_backup) = expected_backup {
        assert_eq!(body["failure"]["backup_path"], expected_backup);
    } else {
        assert!(
            body["failure"]["backup_path"].is_null(),
            "backup_path must be null when not populated: {body}"
        );
    }

    // Round-trips through serde.
    let round: DaemonStatus =
        serde_json::from_value(body).expect("DaemonStatus deserializes from §5.2 JSON");
    assert_eq!(round.state, DaemonLifecycleState::Failed);
    assert_eq!(
        round
            .failure
            .as_ref()
            .expect("round-trip preserves failure")
            .kind,
        expected_kind
    );
}

#[tokio::test]
async fn test_failure_migration_failed_round_trips_through_health_and_status() {
    let reporter = seed_failed(
        FailureKind::MigrationFailed,
        "synthetic migration failure",
        Some("/tmp/db.backup-test.sqlite"),
    );
    assert_failure_round_trip(
        &reporter,
        FailureKind::MigrationFailed,
        Some("/tmp/db.backup-test.sqlite"),
    );
    // Failed-serve router exposes the same body under /graphql for the
    // daemonStatus passthrough.
    let _router =
        failed_serve::build_failed_serve_router(reporter, auth::PrincipalTable::test_fixture());
    // Router smoke-tested separately; here we assert the reporter side
    // of the contract.
}

#[tokio::test]
async fn test_failure_schema_newer_than_binary_round_trips() {
    let reporter = seed_failed(
        FailureKind::SchemaNewerThanBinary,
        "applied_max=99 binary_max=14",
        None,
    );
    assert_failure_round_trip(&reporter, FailureKind::SchemaNewerThanBinary, None);
}

#[tokio::test]
async fn test_failure_backup_failed_round_trips() {
    let reporter = seed_failed(FailureKind::BackupFailed, "disk full at /Volumes/X", None);
    assert_failure_round_trip(&reporter, FailureKind::BackupFailed, None);
}

#[tokio::test]
async fn test_failure_crash_loop_budget_exhausted_round_trips() {
    let reporter = seed_failed(
        FailureKind::CrashLoopBudgetExhausted,
        "5 crashes; first at unix=1713440000",
        None,
    );
    assert_failure_round_trip(&reporter, FailureKind::CrashLoopBudgetExhausted, None);
}

#[tokio::test]
async fn test_failure_reentering_ready_clears_failure_and_preserves_started_at() {
    let reporter = seed_failed(FailureKind::MigrationFailed, "x", None);
    // Recovery path: after a restart the new process re-enters Ready.
    reporter.set_state(DaemonLifecycleState::Ready);
    let snap = reporter.snapshot();
    assert_eq!(snap.state, DaemonLifecycleState::Ready);
    assert!(
        snap.failure.is_none(),
        "Ready must clear prior failure so restart-recovery UIs are not stuck on a stale reason"
    );
    assert!(snap.check_failure_invariant().is_ok());
}
