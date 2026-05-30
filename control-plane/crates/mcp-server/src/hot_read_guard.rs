use crate::tools;
use anyhow::Result;
use chrono::Utc;
pub use db::hot_read_guard::{
    CheckResult as DbCheckResult, HotReadGuard as DbHotReadGuard, ProbeGuard,
};
#[cfg(test)]
pub(crate) static P087_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub enum CheckResult {
    Allowed {
        is_probe: bool,
        probe_guard: Option<ProbeGuard>,
    },
    Denied(serde_json::Value),
}

pub struct HotReadGuard {
    inner: DbHotReadGuard,
    surface: String,
}

impl HotReadGuard {
    pub fn new(pool: sqlx::SqlitePool, surface: &str) -> Self {
        Self {
            inner: DbHotReadGuard::new(pool, surface),
            surface: surface.to_string(),
        }
    }

    pub async fn check(&self, request_id: Option<&str>) -> Result<CheckResult> {
        let now = Utc::now().timestamp_millis();
        match self.inner.check().await? {
            DbCheckResult::Allowed {
                is_probe,
                probe_guard,
            } => Ok(CheckResult::Allowed {
                is_probe,
                probe_guard,
            }),
            DbCheckResult::Denied {
                status,
                last_opened,
                retry_after,
            } => {
                let retry_after_val = retry_after.map(|t| (t - now).max(0));
                let status_str = match status {
                    db::repos::hot_read_circuit::CircuitStatus::Open => "open",
                    db::repos::hot_read_circuit::CircuitStatus::HalfOpen => "half_open",
                    _ => "closed",
                };
                let msg = if status_str == "open" {
                    "hot read circuit is open for this surface"
                } else {
                    "hot read circuit is half-open and a probe is already in progress"
                };

                Ok(CheckResult::Denied(tools::storage::typed_error_full(
                    &self.surface,
                    tools::storage::ERR_HOT_READ_CIRCUIT_OPEN,
                    msg,
                    retry_after_val,
                    Some(serde_json::json!({
                        "status": status_str,
                        "lastOpenedAtMs": last_opened,
                    })),
                    request_id,
                )))
            }
        }
    }

    pub async fn record_success(&self) -> Result<()> {
        self.inner.record_success().await
    }

    pub async fn record_violation(&self, kind: &str) -> Result<()> {
        self.inner.record_violation(kind).await
    }
}

pub fn is_hot_read_tool(name: &str) -> bool {
    matches!(
        name,
        "initialize"
            | "runs.list"
            | "tools.list"
            | "runtime.health"
            | "boundary.runtime.get"
            | "storage.health"
            | "artifacts.metadata.get"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use db::repos::hot_read_circuit::{self, CircuitStatus};

    #[tokio::test]
    async fn proposal_087_observe_mode_records_would_open_without_opening_circuit() {
        let _guard = P087_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        std::env::set_var(
            "CHAINWORKS_STORAGE_TIERING_READ_PATH_LIVENESS_MODE",
            "observe",
        );
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        let guard = HotReadGuard::new(pool.clone(), "storage.health");

        guard
            .record_violation("projection_unavailable")
            .await
            .expect("observe mode should record diagnostics");
        std::env::remove_var("CHAINWORKS_STORAGE_TIERING_READ_PATH_LIVENESS_MODE");

        let (status, successes, failures, opened_at, _, would_open) =
            hot_read_circuit::get_circuit_state(&pool, "storage.health")
                .await
                .unwrap();
        assert_eq!(status, CircuitStatus::Closed);
        assert_eq!(successes, 0);
        assert_eq!(failures, 1);
        assert_eq!(opened_at, None);
        assert!(!would_open);

        // Record 2 more violations to trigger would_open
        guard
            .record_violation("projection_unavailable")
            .await
            .unwrap();
        guard
            .record_violation("projection_unavailable")
            .await
            .unwrap();

        let (_, _, failures, _, _, would_open) =
            hot_read_circuit::get_circuit_state(&pool, "storage.health")
                .await
                .unwrap();
        assert_eq!(failures, 3);
        assert!(would_open);
    }

    #[test]
    fn proposal_087_hot_read_tool_set_includes_storage_health() {
        assert!(is_hot_read_tool("runtime.health"));
        assert!(is_hot_read_tool("storage.health"));
        assert!(is_hot_read_tool("runs.list"));
        assert!(!is_hot_read_tool("storage.maintenance.repair_slot"));
    }
}
