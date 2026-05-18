use crate::repos::hot_read_circuit::{self, CircuitStatus};
use anyhow::Result;
use chrono::Utc;
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::sync::Mutex;

static IN_FLIGHT_PROBES: Mutex<Option<HashSet<String>>> = Mutex::new(None);

fn try_acquire_probe(surface: &str) -> bool {
    let mut lock = IN_FLIGHT_PROBES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let probes = lock.get_or_insert_with(HashSet::new);
    if probes.contains(surface) {
        false
    } else {
        probes.insert(surface.to_string());
        true
    }
}

fn release_probe(surface: &str) {
    let mut lock = IN_FLIGHT_PROBES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(probes) = lock.as_mut() {
        probes.remove(surface);
    }
}

pub struct ProbeGuard {
    surface: String,
}

impl Drop for ProbeGuard {
    fn drop(&mut self) {
        release_probe(&self.surface);
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum LivenessMode {
    Disabled,
    Observe,
    Enforce,
}

pub enum CheckResult {
    Allowed {
        is_probe: bool,
        probe_guard: Option<ProbeGuard>,
    },
    Denied {
        status: CircuitStatus,
        last_opened: Option<i64>,
        retry_after: Option<i64>,
    },
}

impl LivenessMode {
    pub fn current() -> Self {
        let val = std::env::var("CHAINWORKS_STORAGE_TIERING_READ_PATH_LIVENESS_MODE").ok();
        match val.as_deref() {
            Some("enforce") => Self::Enforce,
            Some("disabled") => Self::Disabled,
            Some("observe") => Self::Observe,
            None => Self::Observe,
            Some(_) => {
                tracing::warn!(
                    mode = "invalid",
                    "Unknown CHAINWORKS_STORAGE_TIERING_READ_PATH_LIVENESS_MODE value, defaulting to Observe"
                );
                Self::Observe
            }
        }
    }
}

pub struct HotReadGuard {
    pool: SqlitePool,
    surface: String,
    mode: LivenessMode,
}

impl HotReadGuard {
    pub fn new(pool: SqlitePool, surface: &str) -> Self {
        Self {
            pool,
            surface: surface.to_string(),
            mode: LivenessMode::current(),
        }
    }

    pub async fn check(&self) -> Result<CheckResult> {
        if matches!(self.mode, LivenessMode::Disabled) {
            return Ok(CheckResult::Allowed {
                is_probe: false,
                probe_guard: None,
            });
        }

        let (mut status, successes, _failures, last_opened, retry_after, _would_open) =
            hot_read_circuit::get_circuit_state(&self.pool, &self.surface).await?;

        let now = Utc::now().timestamp_millis();

        // P087: Transition to HalfOpen after retry_after_ms
        if status == CircuitStatus::Open && retry_after.map(|t| now >= t).unwrap_or(true) {
            status = CircuitStatus::HalfOpen;
        }

        match (status, &self.mode) {
            (CircuitStatus::Open, LivenessMode::Enforce) => {
                // P087-DEFECT-OPENPROBE: removed random probes while circuit is explicitly Open.
                // Recovery is driven by retry_after_ms transition to HalfOpen.
                Ok(CheckResult::Denied {
                    status,
                    last_opened,
                    retry_after,
                })
            }
            (CircuitStatus::HalfOpen, _) => {
                // P087: While half-open, only one probe runs at a time.
                // Three consecutive successes required to close (handled by record_success).
                if try_acquire_probe(&self.surface) {
                    tracing::debug!(surface = %self.surface, successes, "HotReadGuard: admitting half-open probe");
                    Ok(CheckResult::Allowed {
                        is_probe: true,
                        probe_guard: Some(ProbeGuard {
                            surface: self.surface.clone(),
                        }),
                    })
                } else {
                    // While half-open, normal traffic receives hot_read_circuit_open (HalfOpen status)
                    match self.mode {
                        LivenessMode::Enforce => Ok(CheckResult::Denied {
                            status,
                            last_opened,
                            retry_after,
                        }),
                        _ => Ok(CheckResult::Allowed {
                            is_probe: false,
                            probe_guard: None,
                        }),
                    }
                }
            }
            _ => Ok(CheckResult::Allowed {
                is_probe: false,
                probe_guard: None,
            }),
        }
    }

    pub async fn record_success(&self) -> Result<()> {
        hot_read_circuit::record_success(&self.pool, &self.surface).await
    }

    pub async fn record_violation(&self, kind: &str) -> Result<()> {
        crate::metrics::increment_counter("mcp_hot_read_violation_total");
        match self.mode {
            LivenessMode::Enforce => {
                hot_read_circuit::record_violation(&self.pool, &self.surface, kind).await
            }
            LivenessMode::Observe => {
                hot_read_circuit::record_would_open(&self.pool, &self.surface, kind).await
            }
            LivenessMode::Disabled => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::hot_read_circuit;

    #[tokio::test]
    async fn proposal_087_enforce_mode_respects_retry_after_and_single_half_open_probe() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        let guard = HotReadGuard {
            pool: pool.clone(),
            surface: "storage.health".to_string(),
            mode: LivenessMode::Enforce,
        };

        for _ in 0..3 {
            guard.record_violation("timeout").await.unwrap();
        }

        match guard.check().await.unwrap() {
            CheckResult::Denied {
                status,
                retry_after,
                ..
            } => {
                assert_eq!(status, CircuitStatus::Open);
                assert!(retry_after.is_some());
            }
            CheckResult::Allowed { .. } => panic!("open circuit must deny before retry_after_ms"),
        }

        sqlx::query(
            "UPDATE hot_read_circuit_states SET retry_after_ms = ?, circuit_status = 'open' WHERE governed_surface = ?",
        )
        .bind(Utc::now().timestamp_millis() - 1)
        .bind("storage.health")
        .execute(&pool)
        .await
        .unwrap();

        let first_probe = match guard.check().await.unwrap() {
            CheckResult::Allowed {
                is_probe,
                probe_guard,
            } => {
                assert!(is_probe);
                probe_guard.expect("half-open success path must hold the probe slot")
            }
            CheckResult::Denied { .. } => panic!("expired retry_after_ms must admit one probe"),
        };

        match guard.check().await.unwrap() {
            CheckResult::Denied { status, .. } => assert_eq!(status, CircuitStatus::HalfOpen),
            CheckResult::Allowed { .. } => panic!("concurrent half-open traffic must be denied"),
        }

        drop(first_probe);
        guard.record_success().await.unwrap();
        let (status, successes, _, _, _, _) =
            hot_read_circuit::get_circuit_state(&pool, "storage.health")
                .await
                .unwrap();
        assert_eq!(status, CircuitStatus::HalfOpen);
        assert_eq!(successes, 1);
    }
}
