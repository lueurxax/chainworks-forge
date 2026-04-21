//! In-process lifecycle reporter (Proposal 042 §5.1).
//!
//! Owns the single authoritative `DaemonStatus` for this process and
//! broadcasts every transition via `engine::event_bus` as
//! [`domain::events::DomainEvent::DaemonStatusChanged`]. Consumers that
//! need push notifications (the `daemonStatusChanged` GraphQL
//! subscription) subscribe to the event bus; consumers that need a
//! snapshot (`/health`, `/ready`, `daemonStatus` query) read
//! [`LifecycleReporter::snapshot`].

use std::sync::{Arc, Mutex};

use crate::event_bus::EventSender;
use chrono::Utc;
use domain::events::DomainEvent;
use domain::lifecycle::{
    DaemonLifecycleState, DaemonStatus, DegradedKind, DegradedReason, FailureKind, FailureReason,
};
use tracing::{info, warn};

/// Thread-safe owner of the daemon's current [`DaemonStatus`].
#[derive(Clone)]
pub struct LifecycleReporter {
    inner: Arc<Mutex<DaemonStatus>>,
    events: EventSender,
}

impl LifecycleReporter {
    /// Create a new reporter initialised to `NotStarted`.
    pub fn new(
        binary_schema_version: u32,
        build_sha: impl Into<String>,
        events: EventSender,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(DaemonStatus::initial(
                binary_schema_version,
                build_sha,
            ))),
            events,
        }
    }

    /// Latest snapshot for `/health`, `/ready`, and `daemonStatus` readers.
    pub fn snapshot(&self) -> DaemonStatus {
        self.inner.lock().expect("lifecycle reporter lock").clone()
    }

    /// Transition to a new state, updating `last_state_change_at` and
    /// broadcasting the new snapshot. Validates the failure invariant
    /// (§4.1) before broadcast; emits a warning and still broadcasts if
    /// the invariant is violated so downstream tests catch the bug.
    pub fn set_state(&self, state: DaemonLifecycleState) {
        let snapshot = {
            let mut guard = self.inner.lock().expect("lifecycle reporter lock");
            guard.state = state;
            guard.last_state_change_at = Utc::now();
            if state == DaemonLifecycleState::Ready && guard.started_at.is_none() {
                guard.started_at = Some(Utc::now());
            }
            // Clear degraded/failure on re-entering Ready.
            if state == DaemonLifecycleState::Ready {
                guard.degraded.clear();
                guard.failure = None;
            }
            if let Err(msg) = guard.check_failure_invariant() {
                warn!(invariant = msg, "lifecycle reporter emitted invalid status");
            }
            guard.clone()
        };
        info!(state = %state, "daemon lifecycle state changed");
        let _ = self
            .events
            .send(DomainEvent::DaemonStatusChanged { status: snapshot });
    }

    /// Record the schema version after a successful migration preflight.
    /// Callers invoke this once, before transitioning to `Ready`.
    pub fn set_schema_version(&self, schema_version: u32) {
        let mut guard = self.inner.lock().expect("lifecycle reporter lock");
        guard.schema_version = schema_version;
    }

    /// Add a degraded reason and transition to `Degraded` if not already
    /// there. No-op if the same kind is already present.
    pub fn raise_degraded(&self, kind: DegradedKind, detail: impl Into<String>) {
        let snapshot = {
            let mut guard = self.inner.lock().expect("lifecycle reporter lock");
            if guard.degraded.iter().any(|r| r.kind == kind) {
                return;
            }
            guard.degraded.push(DegradedReason {
                kind,
                detail: detail.into(),
                since: Utc::now(),
            });
            if guard.state == DaemonLifecycleState::Ready {
                guard.state = DaemonLifecycleState::Degraded;
                guard.last_state_change_at = Utc::now();
            }
            guard.clone()
        };
        info!(
            kind = %kind,
            count = snapshot.degraded.len(),
            "daemon entered/continued Degraded"
        );
        let _ = self
            .events
            .send(DomainEvent::DaemonStatusChanged { status: snapshot });
    }

    /// Clear one degraded kind; transition back to `Ready` if the
    /// `degraded` list becomes empty and we were previously `Degraded`.
    pub fn clear_degraded(&self, kind: DegradedKind) {
        let snapshot = {
            let mut guard = self.inner.lock().expect("lifecycle reporter lock");
            let before = guard.degraded.len();
            guard.degraded.retain(|r| r.kind != kind);
            if guard.degraded.len() == before {
                return;
            }
            if guard.degraded.is_empty() && guard.state == DaemonLifecycleState::Degraded {
                guard.state = DaemonLifecycleState::Ready;
                guard.last_state_change_at = Utc::now();
            }
            guard.clone()
        };
        let _ = self
            .events
            .send(DomainEvent::DaemonStatusChanged { status: snapshot });
    }

    /// Transition to terminal `Failed` with the given reason. Populates
    /// `DaemonStatus.failure` per §4.1 invariant and broadcasts.
    pub fn set_failed(
        &self,
        kind: FailureKind,
        detail: impl Into<String>,
        backup_path: Option<String>,
    ) {
        let snapshot = {
            let mut guard = self.inner.lock().expect("lifecycle reporter lock");
            guard.state = DaemonLifecycleState::Failed;
            guard.last_state_change_at = Utc::now();
            guard.failure = Some(FailureReason {
                kind,
                detail: detail.into(),
                since: Utc::now(),
                backup_path,
            });
            guard.degraded.clear();
            guard.clone()
        };
        warn!(
            kind = %kind,
            "daemon transitioned to terminal Failed"
        );
        let _ = self
            .events
            .send(DomainEvent::DaemonStatusChanged { status: snapshot });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_bus;

    fn make() -> (
        LifecycleReporter,
        tokio::sync::broadcast::Receiver<DomainEvent>,
    ) {
        let bus = event_bus::new_bus(16);
        let rx = bus.subscribe();
        let reporter = LifecycleReporter::new(14, "test-sha", bus);
        (reporter, rx)
    }

    #[tokio::test]
    async fn set_state_broadcasts_new_snapshot() {
        let (reporter, mut rx) = make();
        reporter.set_state(DaemonLifecycleState::Starting);
        let event = rx.recv().await.unwrap();
        match event {
            DomainEvent::DaemonStatusChanged { status } => {
                assert_eq!(status.state, DaemonLifecycleState::Starting);
                assert_eq!(status.binary_schema_version, 14);
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn set_ready_populates_started_at_once() {
        let (reporter, _rx) = make();
        reporter.set_state(DaemonLifecycleState::Starting);
        reporter.set_state(DaemonLifecycleState::Ready);
        let first_started = reporter.snapshot().started_at;
        assert!(first_started.is_some());
        // Re-entering Ready (e.g. after Degraded) must not reset started_at.
        reporter.raise_degraded(DegradedKind::StaleProjection, "test");
        reporter.clear_degraded(DegradedKind::StaleProjection);
        let second_started = reporter.snapshot().started_at;
        assert_eq!(first_started, second_started);
    }

    #[tokio::test]
    async fn raise_degraded_idempotent_on_same_kind() {
        let (reporter, _rx) = make();
        reporter.set_state(DaemonLifecycleState::Ready);
        reporter.raise_degraded(DegradedKind::StaleProjection, "first");
        reporter.raise_degraded(DegradedKind::StaleProjection, "second");
        let snap = reporter.snapshot();
        assert_eq!(snap.degraded.len(), 1);
        assert_eq!(snap.state, DaemonLifecycleState::Degraded);
    }

    #[tokio::test]
    async fn clear_last_degraded_returns_to_ready() {
        let (reporter, _rx) = make();
        reporter.set_state(DaemonLifecycleState::Ready);
        reporter.raise_degraded(DegradedKind::StaleProjection, "x");
        assert_eq!(reporter.snapshot().state, DaemonLifecycleState::Degraded);
        reporter.clear_degraded(DegradedKind::StaleProjection);
        assert_eq!(reporter.snapshot().state, DaemonLifecycleState::Ready);
        assert!(reporter.snapshot().degraded.is_empty());
    }

    #[tokio::test]
    async fn set_failed_populates_failure_and_clears_degraded() {
        let (reporter, _rx) = make();
        reporter.set_state(DaemonLifecycleState::Ready);
        reporter.raise_degraded(DegradedKind::StaleProjection, "x");
        reporter.set_failed(
            FailureKind::MigrationFailed,
            "test failure",
            Some("/tmp/db.backup".into()),
        );
        let snap = reporter.snapshot();
        assert_eq!(snap.state, DaemonLifecycleState::Failed);
        assert!(snap.degraded.is_empty());
        let failure = snap.failure.as_ref().expect("failure populated");
        assert_eq!(failure.kind, FailureKind::MigrationFailed);
        assert_eq!(failure.backup_path.as_deref(), Some("/tmp/db.backup"));
        assert!(snap.check_failure_invariant().is_ok());
    }

    #[tokio::test]
    async fn ready_clears_prior_failure_field() {
        let (reporter, _rx) = make();
        reporter.set_failed(FailureKind::BackupFailed, "x", None);
        reporter.set_state(DaemonLifecycleState::Ready);
        let snap = reporter.snapshot();
        assert!(snap.failure.is_none());
    }
}
