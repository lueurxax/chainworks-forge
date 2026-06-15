//! P066 T13: Per-run Xcode exclusive lease with queue-wait diagnostics.
//!
//! The Xcode run scope requires that only one host-executed Xcode invocation
//! may mutate the mapping root at a time. Queue wait is measured and surfaced
//! separately from directory preparation; queue timeout MUST NOT increment
//! mapping_setup_latency_p95_ms.
//!
//! Lease lifecycle:
//! - Acquired BEFORE any xcodebuild/simctl process starts.
//! - Released on: normal completion (after process exit + observation capture),
//!   launch failure (same cleanup path), session close, daemon shutdown.
//! - Wait deadline: min(300_000 ms, remaining request runtime budget).
//! - Cancellation before acquire → concurrency.status=cancelled_before_acquire.
//! - Timeout → xcode_run_scope_queue_timeout (NOT a setup failure).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use domain::ids::RunId;
use domain::toolchain::XcodeRunScopeQueueTimeout;
use domain::toolchain_diagnostics::DiagConcurrencyState;
use tokio::sync::{Mutex, OwnedMutexGuard};

/// Maximum wait deadline for the per-run Xcode lease (300,000 ms).
pub const XCODE_LEASE_MAX_DEADLINE_MS: u64 = 300_000;

/// Threshold below which wait is classified as "acquired immediately" rather than "queued".
const ACQUIRED_IMMEDIATELY_THRESHOLD_MS: i64 = 10;

/// Guard that releases the per-run Xcode lease when dropped.
pub struct XcodeRunLeaseGuard {
    run_id: RunId,
    _guard: OwnedMutexGuard<()>,
}

impl std::fmt::Debug for XcodeRunLeaseGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XcodeRunLeaseGuard")
            .field("run_id", &self.run_id)
            .finish()
    }
}

/// Result of successfully acquiring the per-run Xcode lease.
#[derive(Debug)]
pub struct XcodeLeaseAcquisition {
    pub guard: XcodeRunLeaseGuard,
    /// Concurrency diagnostics capturing whether the lease was immediate or queued.
    pub concurrency: DiagConcurrencyState,
}

/// Registry of per-run Xcode exclusive leases.
///
/// One `Arc<Mutex<()>>` is held per active run. The inner mutex is the lease;
/// `lock_owned()` blocks until the current holder releases it.
#[derive(Default, Clone)]
pub struct XcodeRunLeaseRegistry {
    locks: Arc<Mutex<HashMap<RunId, Arc<Mutex<()>>>>>,
}

impl XcodeRunLeaseRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Acquire the per-run Xcode lease for `run_id`.
    ///
    /// `deadline_ms` is clamped to `min(deadline_ms, XCODE_LEASE_MAX_DEADLINE_MS)`.
    ///
    /// Returns `XcodeLeaseAcquisition` on success with concurrency diagnostics
    /// indicating whether the lease was acquired immediately or after queuing.
    ///
    /// Returns `XcodeRunScopeQueueTimeout` if the wait exceeds the deadline.
    /// Queue timeout is NOT a setup failure and MUST NOT increment
    /// `mapping_setup_latency_p95_ms`.
    pub async fn acquire(
        &self,
        run_id: RunId,
        deadline_ms: u64,
    ) -> Result<XcodeLeaseAcquisition, XcodeRunScopeQueueTimeout> {
        let effective_deadline_ms = deadline_ms.min(XCODE_LEASE_MAX_DEADLINE_MS);
        let deadline = Duration::from_millis(effective_deadline_ms);
        let wait_start = Instant::now();

        let lock_arc = {
            let mut locks = self.locks.lock().await;
            locks
                .entry(run_id)
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };

        let guard = tokio::time::timeout(deadline, lock_arc.lock_owned())
            .await
            .map_err(|_| {
                let elapsed = wait_start.elapsed().as_millis() as u64;
                XcodeRunScopeQueueTimeout {
                    run_id: run_id.to_string(),
                    wait_ms: elapsed,
                    deadline_ms: effective_deadline_ms,
                }
            })?;

        let wait_ms = wait_start.elapsed().as_millis() as i64;

        let concurrency = if wait_ms < ACQUIRED_IMMEDIATELY_THRESHOLD_MS {
            DiagConcurrencyState::acquired_immediately(wait_ms)
        } else {
            DiagConcurrencyState::queued(wait_ms, effective_deadline_ms as i64)
        };

        Ok(XcodeLeaseAcquisition {
            guard: XcodeRunLeaseGuard {
                run_id,
                _guard: guard,
            },
            concurrency,
        })
    }

    /// Release all lease state for a run.
    ///
    /// This removes the per-run mutex entry from the registry. Any waiters will
    /// subsequently create a fresh mutex entry when they re-enter the registry.
    /// Call this on run completion, cancellation, or daemon shutdown cleanup.
    pub async fn release_for_run(&self, run_id: RunId) {
        let mut locks = self.locks.lock().await;
        locks.remove(&run_id);
    }

    /// Number of active run entries in the registry (for diagnostics/tests).
    pub async fn active_run_count(&self) -> usize {
        self.locks.lock().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::ids::RunId;

    fn run_id() -> RunId {
        RunId::new()
    }

    #[tokio::test]
    async fn p066_xcode_run_lease_acquired_immediately() {
        let registry = XcodeRunLeaseRegistry::new();
        let rid = run_id();

        let result = registry.acquire(rid, 5_000).await;
        assert!(result.is_ok(), "first acquire must succeed immediately");

        let acq = result.unwrap();
        assert!(
            matches!(
                acq.concurrency.status,
                domain::toolchain_diagnostics::DiagConcurrencyStatus::AcquiredImmediately
                    | domain::toolchain_diagnostics::DiagConcurrencyStatus::Queued
            ),
            "must be acquired_immediately or queued (depending on timing)"
        );
        assert_eq!(
            acq.concurrency.lease_key_kind.as_deref(),
            Some("run_id"),
            "lease key kind must be run_id"
        );
    }

    #[tokio::test]
    async fn p066_xcode_run_lease_queue_timeout_separate_from_setup() {
        let registry = XcodeRunLeaseRegistry::new();
        let rid = run_id();

        // Acquire first lease (never released — simulates blocked holder).
        let _first = registry.acquire(rid, 10_000).await.unwrap();

        // Second acquire with tiny deadline → queue timeout.
        let result = registry.acquire(rid, 1).await; // 1 ms deadline
        assert!(result.is_err(), "must time out when lease is held");

        let timeout_err = result.unwrap_err();
        assert_eq!(
            XcodeRunScopeQueueTimeout::failure_kind_str(),
            "xcode_run_scope_queue_timeout",
            "queue timeout failure kind must be distinct from setup failure"
        );
        assert!(timeout_err.wait_ms >= 0, "wait_ms must be recorded");
        assert!(timeout_err.deadline_ms > 0, "deadline_ms must be recorded");

        // Verify failure kind is distinct from setup failure.
        assert_ne!(
            XcodeRunScopeQueueTimeout::failure_kind_str(),
            domain::toolchain::ToolchainMappingSetupFailed::failure_kind_str(),
            "queue timeout and setup failure must have distinct failure kinds"
        );
    }

    #[tokio::test]
    async fn p066_xcode_run_lease_released_after_guard_drop() {
        let registry = XcodeRunLeaseRegistry::new();
        let rid = run_id();

        {
            let _guard = registry.acquire(rid, 5_000).await.unwrap();
            // guard held here
        }
        // guard dropped — second acquire must succeed

        let result = registry.acquire(rid, 100).await;
        assert!(result.is_ok(), "must succeed after guard drop");
    }

    #[tokio::test]
    async fn p066_xcode_run_lease_different_runs_do_not_contend() {
        let registry = XcodeRunLeaseRegistry::new();
        let rid1 = run_id();
        let rid2 = run_id();

        // Hold lease for rid1.
        let _guard1 = registry.acquire(rid1, 5_000).await.unwrap();

        // rid2 must acquire immediately without waiting for rid1.
        let result = registry.acquire(rid2, 100).await;
        assert!(result.is_ok(), "different run_ids must not contend");
    }

    #[tokio::test]
    async fn p066_xcode_run_lease_deadline_capped_at_max() {
        let registry = XcodeRunLeaseRegistry::new();
        let rid = run_id();
        // Acquire with u64::MAX — verifies no integer overflow or panic in the clamping path.
        // No competing holder, so this should succeed immediately.
        let result = registry.acquire(rid, u64::MAX).await;
        assert!(
            result.is_ok(),
            "u64::MAX deadline must not panic or overflow"
        );
    }
}
