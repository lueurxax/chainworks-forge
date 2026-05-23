// P046: Bounded SQLite retry helper (db-owned per approved architecture contract).
//
// Retries a P046 DB read on transient SQLite busy/locked errors with the
// pinned policy: 3 total attempts, 50ms then 150ms backoff, jitter ≤ 25ms,
// total retry sleep ≤ 250ms, stops with 250ms headroom before the 2s deadline.
// Non-transient errors are returned immediately.
//
// Returns Err wrapping "transient_db_unavailable: ..." when all attempts fail.
// The caller (graphql-server) checks this prefix to emit the correct resolver error.

use tracing::{debug, warn};

pub fn is_transient_db_error(e: &anyhow::Error) -> bool {
    let msg = format!("{e:#}").to_lowercase();
    msg.contains("locked") || msg.contains("busy") || msg.contains("pool timed out")
}

pub async fn p046_retry_db<F, Fut, T>(
    field: &str,
    deadline: tokio::time::Instant,
    f: F,
) -> anyhow::Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    const BACKOFFS_MS: [u64; 2] = [50, 150];
    const MAX_SLEEP_MS: u64 = 250;
    const DEADLINE_HEADROOM_MS: u64 = 250;

    let mut last_err: anyhow::Error = anyhow::anyhow!("no attempts made");
    let mut total_sleep_ms: u64 = 0;

    for attempt in 0..3u32 {
        if attempt > 0 {
            if tokio::time::Instant::now() >= deadline {
                break;
            }
            let backoff = BACKOFFS_MS[(attempt - 1) as usize];
            let jitter: u64 = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos() as u64
                % 26)
                .min(25);
            let sleep = (backoff + jitter).min(MAX_SLEEP_MS.saturating_sub(total_sleep_ms));
            if sleep == 0 || total_sleep_ms >= MAX_SLEEP_MS {
                break;
            }
            total_sleep_ms += sleep;
            tokio::time::sleep(std::time::Duration::from_millis(sleep)).await;
            if tokio::time::Instant::now() >= deadline {
                break;
            }
        }
        const PER_ATTEMPT_TIMEOUT_MS_MAX: u64 = 300;
        let remaining = deadline
            .saturating_duration_since(tokio::time::Instant::now())
            .saturating_sub(std::time::Duration::from_millis(DEADLINE_HEADROOM_MS))
            .min(std::time::Duration::from_millis(PER_ATTEMPT_TIMEOUT_MS_MAX));
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, f()).await {
            Ok(Ok(v)) => {
                if attempt > 0 {
                    debug!(
                        "p046 db retry succeeded for {field} on attempt {}",
                        attempt + 1
                    );
                    crate::metrics::increment_counter_with_label(
                        "session_graphql_sqlite_retry_total",
                        &format!("{field}:success_after_retry"),
                    );
                }
                return Ok(v);
            }
            Ok(Err(e)) if is_transient_db_error(&e) => {
                warn!(
                    "p046 transient db error for {field} attempt {}: {e}",
                    attempt + 1
                );
                crate::metrics::increment_counter_with_label(
                    "session_graphql_sqlite_retry_total",
                    &format!("{field}:exhausted"),
                );
                last_err = e;
            }
            Ok(Err(e)) => return Err(e),
            Err(_timeout) => {
                warn!("p046 db timeout for {field} attempt {}", attempt + 1);
                crate::metrics::increment_counter_with_label(
                    "session_graphql_sqlite_retry_total",
                    &format!("{field}:deadline_headroom_stop"),
                );
                last_err = anyhow::anyhow!("query timeout");
                break;
            }
        }
    }
    crate::metrics::increment_counter_with_label(
        "session_graphql_sqlite_retry_exhausted_total",
        field,
    );
    Err(anyhow::anyhow!("transient_db_unavailable: {last_err}"))
}
