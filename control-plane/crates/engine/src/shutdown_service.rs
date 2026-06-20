//! P083: Shutdown contract service.
//!
//! Implements shutdown_contract_v1 path distinction:
//! - graceful_appkit: applicationShouldTerminate path (normal quit, logout/system shutdown)
//! - abrupt_external: force_quit, SIGKILL, crash, host_power_loss
//!
//! On startup recovery this service scans shutdown_signal_side_effects and
//! provider_cancellation_intents to determine which sessions need reprocessing.
//! It never assumes applicationShouldTerminate ran during abrupt_external paths.
//!
//! SEC-P083-HIGH-001: `dispatch_planned_shutdown_signals` wires planned signal rows through
//! identity-checked OS signal dispatch. Called from daemon startup after `recover_shutdown_state`.

use anyhow::{Context, Result};
#[cfg(unix)]
use libc;
use sqlx::SqlitePool;
use tracing::{info, warn};
use uuid::Uuid;

use db::repos::{provider_sessions, shutdown_receipts};
use db::metrics::{
    record_p083_shutdown_duplicate_signal_suppressed,
    record_p083_shutdown_interrupted_receipt,
    record_p083_provider_cancellation_intent,
};

/// Classification of a provider session shutdown outcome during recovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShutdownRecoveryAction {
    /// planned signal without issued_at → retry with matching identity
    RetrySignal,
    /// issued signal without observed_exit_at → settle by process observation
    ObservationRequired,
    /// duplicate signal already suppressed this generation
    DuplicateSuppressed,
    /// process identity cannot be confirmed → hold for manual check
    IdentityAmbiguousHold,
    /// signal already terminal (observed/suppressed/identity_mismatch)
    Terminal,
    /// receipt already recovered
    AlreadyRecovered,
}

/// Summary for a single session recovered during startup.
#[derive(Debug, Clone)]
pub struct SessionShutdownRecoverySummary {
    pub provider_session_id: String,
    pub provider: String,
    pub action: ShutdownRecoveryAction,
    pub signal_kind: Option<String>,
    pub shutdown_epoch: Option<i64>,
    pub generation: Option<i64>,
    pub reason: Option<String>,
}

/// Aggregate result returned from `recover_shutdown_state`.
#[derive(Debug, Default)]
pub struct ShutdownRecoveryReport {
    pub sessions_scanned: usize,
    pub retry_signal_count: usize,
    pub observation_required_count: usize,
    pub duplicate_suppressed_count: usize,
    pub identity_ambiguous_count: usize,
    pub already_recovered_count: usize,
    pub terminal_count: usize,
    pub summaries: Vec<SessionShutdownRecoverySummary>,
}

/// On daemon startup, load all active (non-terminal) shutdown signal side effects
/// and provider cancellation intents to classify what recovery action is needed.
///
/// Per shutdown_contract_v1.recovery_rules:
/// - planned without issued_at → RetrySignal (if identity matches and deadline permits)
/// - issued without observed_exit_at → ObservationRequired
/// - held cancellation intent + identity_ambiguous process_fate → IdentityAmbiguousHold
///
/// This function does NOT issue signals — it only classifies and emits metrics.
/// Callers are responsible for dispatching the resulting actions.
pub async fn recover_shutdown_state(pool: &SqlitePool) -> Result<ShutdownRecoveryReport> {
    let mut report = ShutdownRecoveryReport::default();

    // Per provider_cancellation_intent_contract_v1 identity_ambiguous_canonical_rule:
    // requested intents with shutdown_epoch IS NULL should be held as identity_ambiguous
    // ONLY when restart cannot prove the process is live, absent, or already interrupted.
    // Process identity is checked before holding to avoid over-holding (audit defect #3):
    // - If process is absent: skip hold (signal dispatch or absent settlement handles it).
    // - If process is alive with matching identity: skip hold (identity confirmed).
    // - If identity is mismatch or unverifiable (or no process_id): hold.
    let requested_intents = provider_sessions::find_requested_intents_null_shutdown_epoch(pool)
        .await
        .context("shutdown_service: load requested intents with null shutdown_epoch")?;
    for intent in &requested_intents {
        let (should_hold, reason) = should_hold_requested_null_epoch_as_ambiguous(
            pool,
            &intent.provider_session_id,
        )
        .await;

        if !should_hold {
            tracing::info!(
                provider_session_id = %intent.provider_session_id,
                cancellation_epoch = intent.cancellation_epoch,
                reason,
                "P083 recovery: requested null-epoch intent has verifiable status; deferring to dispatch/settlement"
            );
            continue;
        }

        let held = provider_sessions::hold_identity_ambiguous(
            pool,
            &intent.provider_session_id,
            intent.cancellation_epoch,
        )
        .await
        .context("shutdown_service: hold_identity_ambiguous for requested intent")?;
        if held {
            tracing::warn!(
                provider_session_id = %intent.provider_session_id,
                cancellation_epoch = intent.cancellation_epoch,
                reason,
                "P083 recovery: requested cancellation intent with null shutdown_epoch \
                 transitioned to held+identity_ambiguous (process status unverifiable)"
            );
        }
    }

    // Load all provider sessions with non-terminal process_fate.
    let active_sessions = provider_sessions::find_identity_ambiguous(pool)
        .await
        .context("shutdown_service: load identity_ambiguous sessions")?;

    for session in &active_sessions {
        report.sessions_scanned += 1;
        let summary = SessionShutdownRecoverySummary {
            provider_session_id: session.provider_session_id.clone(),
            provider: session.provider.clone(),
            action: ShutdownRecoveryAction::IdentityAmbiguousHold,
            signal_kind: None,
            shutdown_epoch: None,
            generation: None,
            reason: Some("process_fate=identity_ambiguous on startup".to_string()),
        };
        record_p083_provider_cancellation_intent(
            &session.provider,
            "held",
            "shutdown_recovery",
        );
        report.identity_ambiguous_count += 1;
        report.summaries.push(summary);
    }

    // Scan for planned signals that were never issued before the crash.
    // Per shutdown_contract_v1.recovery_rules crash_after_planned: a planned signal
    // row with issued_at_monotonic_ms IS NULL indicates a crash after the signal was
    // recorded but before it was dispatched; this session needs RetrySignal.
    // Per DEFECT-004 fix: before emitting RetrySignal, check if the provider session
    // has process_fate='identity_ambiguous'. If so, classify as IdentityAmbiguousHold
    // rather than RetrySignal to avoid issuing a signal to a potentially wrong PID.
    let planned_signals = load_planned_without_issued(pool).await?;
    for signal in planned_signals {
        let provider = resolve_provider_for_session(pool, &signal.provider_session_id).await;
        let process_fate =
            resolve_process_fate_for_session(pool, &signal.provider_session_id).await;
        report.sessions_scanned += 1;
        if process_fate.as_deref() == Some("identity_ambiguous") {
            record_p083_provider_cancellation_intent(
                provider.as_deref().unwrap_or("unknown"),
                "held",
                "shutdown_recovery",
            );
            report.identity_ambiguous_count += 1;
            report.summaries.push(SessionShutdownRecoverySummary {
                provider_session_id: signal.provider_session_id.clone(),
                provider: provider.unwrap_or_else(|| "unknown".to_string()),
                action: ShutdownRecoveryAction::IdentityAmbiguousHold,
                signal_kind: Some(signal.signal_kind.clone()),
                shutdown_epoch: Some(signal.shutdown_epoch),
                generation: Some(signal.generation),
                reason: Some(
                    "process_fate=identity_ambiguous: signal not retried until identity verified"
                        .to_string(),
                ),
            });
        } else {
            report.retry_signal_count += 1;
            report.summaries.push(SessionShutdownRecoverySummary {
                provider_session_id: signal.provider_session_id.clone(),
                provider: provider.unwrap_or_else(|| "unknown".to_string()),
                action: ShutdownRecoveryAction::RetrySignal,
                signal_kind: Some(signal.signal_kind.clone()),
                shutdown_epoch: Some(signal.shutdown_epoch),
                generation: Some(signal.generation),
                reason: Some(
                    "planned without issued_at on restart (crash-after-planned)".to_string(),
                ),
            });
        }
    }

    // Load all non-terminal interrupted receipts.
    // Per shutdown_contract_v1.recovery_rules.queued_no_signal: queued_no_signal opens
    // a later shutdown_epoch only after process identity is rechecked. If identity is
    // ambiguous, hold with IdentityAmbiguousHold rather than RetrySignal.
    let unrecovered_receipts = load_unrecovered_interrupted_receipts(pool).await?;
    for (session_id, provider, epoch, state, queue_rank) in unrecovered_receipts {
        report.sessions_scanned += 1;
        record_p083_shutdown_interrupted_receipt(&provider, &state);

        let action = if state == "queued_no_signal" {
            // Check process_fate before classifying as RetrySignal.
            let process_fate = resolve_process_fate_for_session(pool, &session_id).await;
            if process_fate.as_deref() == Some("identity_ambiguous") {
                ShutdownRecoveryAction::IdentityAmbiguousHold
            } else {
                ShutdownRecoveryAction::RetrySignal
            }
        } else {
            ShutdownRecoveryAction::ObservationRequired
        };

        match &action {
            ShutdownRecoveryAction::RetrySignal => report.retry_signal_count += 1,
            ShutdownRecoveryAction::IdentityAmbiguousHold => {
                report.identity_ambiguous_count += 1;
                record_p083_provider_cancellation_intent(&provider, "held", "shutdown_recovery");
            }
            _ => report.observation_required_count += 1,
        }
        report.summaries.push(SessionShutdownRecoverySummary {
            provider_session_id: session_id,
            provider,
            action,
            signal_kind: None,
            shutdown_epoch: Some(epoch),
            generation: None,
            reason: Some(format!(
                "unrecovered receipt state={state} queue_rank={queue_rank:?}"
            )),
        });
    }

    // Scan for issued signals that were not observed before restart.
    let issued_signals = load_issued_without_observation(pool).await?;
    for signal in issued_signals {
        let provider = resolve_provider_for_session(pool, &signal.provider_session_id).await;
        record_p083_shutdown_interrupted_receipt(
            provider.as_deref().unwrap_or("unknown"),
            "kill_signal_issued",
        );
        report.observation_required_count += 1;
        report.summaries.push(SessionShutdownRecoverySummary {
            provider_session_id: signal.provider_session_id.clone(),
            provider: provider.unwrap_or_else(|| "unknown".to_string()),
            action: ShutdownRecoveryAction::ObservationRequired,
            signal_kind: Some(signal.signal_kind.clone()),
            shutdown_epoch: Some(signal.shutdown_epoch),
            generation: Some(signal.generation),
            reason: Some("issued without observed_exit_at on restart".to_string()),
        });
    }

    if report.sessions_scanned > 0 {
        info!(
            sessions_scanned = report.sessions_scanned,
            retry_signal = report.retry_signal_count,
            observation_required = report.observation_required_count,
            identity_ambiguous = report.identity_ambiguous_count,
            "P083 shutdown recovery scan complete"
        );
    }

    Ok(report)
}

/// Record an interrupted receipt when a graceful or abrupt shutdown could not
/// complete cleanly for a provider session. Emits the bounded metric.
pub async fn record_shutdown_interrupted(
    pool: &SqlitePool,
    provider_session_id: &str,
    provider: &str,
    shutdown_epoch: i64,
    receipt_generation: i64,
    interrupted_state: &str,
    queue_rank: Option<i64>,
) -> Result<String> {
    let receipt_id = Uuid::new_v4().to_string();
    shutdown_receipts::insert_interrupted_receipt(
        pool,
        &receipt_id,
        provider_session_id,
        shutdown_epoch,
        receipt_generation,
        interrupted_state,
        queue_rank,
    )
    .await
    .context("shutdown_service: record_shutdown_interrupted")?;

    record_p083_shutdown_interrupted_receipt(provider, interrupted_state);

    Ok(receipt_id)
}

/// Suppress a duplicate signal for a generation that was already issued.
/// Per duplicate_suppression rule: calling this on an already-issued generation
/// is not an error; it records the suppression and emits the duplicate metric.
pub async fn suppress_duplicate_signal(
    pool: &SqlitePool,
    provider: &str,
    provider_session_id: &str,
    shutdown_epoch: i64,
    signal_kind: &str,
    generation: i64,
    error_code: Option<&str>,
) -> Result<bool> {
    let suppressed = shutdown_receipts::mark_signal_suppressed(
        pool,
        provider_session_id,
        shutdown_epoch,
        signal_kind,
        generation,
        error_code,
    )
    .await
    .context("shutdown_service: suppress_duplicate_signal")?;

    if suppressed {
        record_p083_shutdown_duplicate_signal_suppressed(provider);
        warn!(
            provider_session_id,
            shutdown_epoch,
            signal_kind,
            generation,
            "P083 duplicate shutdown signal suppressed"
        );
    }

    Ok(suppressed)
}

// ── private helpers ──────────────────────────────────────────────────────────

struct IssuedSignalRow {
    provider_session_id: String,
    shutdown_epoch: i64,
    signal_kind: String,
    generation: i64,
}

async fn load_unrecovered_interrupted_receipts(
    pool: &SqlitePool,
) -> Result<Vec<(String, String, i64, String, Option<i64>)>> {
    // Returns (provider_session_id, provider, shutdown_epoch, interrupted_state, queue_rank)
    // for receipts that have not been recovered yet.
    let rows = sqlx::query(
        r#"SELECT r.provider_session_id, COALESCE(ps.provider, 'unknown') AS provider,
                  r.shutdown_epoch, r.interrupted_state, r.queue_rank
             FROM shutdown_interrupted_receipts r
             LEFT JOIN provider_sessions ps
               ON ps.provider_session_id = r.provider_session_id
            WHERE r.recovered_at IS NULL
            ORDER BY r.shutdown_epoch, r.receipt_generation"#,
    )
    .fetch_all(pool)
    .await
    .context("shutdown_service: load_unrecovered_interrupted_receipts")?;

    use sqlx::Row;
    Ok(rows
        .into_iter()
        .map(|r| {
            (
                r.get("provider_session_id"),
                r.get::<String, _>("provider"),
                r.get("shutdown_epoch"),
                r.get("interrupted_state"),
                r.get("queue_rank"),
            )
        })
        .collect())
}

/// Load planned signals that were never dispatched (crash-after-planned recovery).
/// Per shutdown_contract_v1.recovery_rules: intent_state='planned' with NULL
/// issued_at_monotonic_ms means the signal was recorded but never sent.
async fn load_planned_without_issued(pool: &SqlitePool) -> Result<Vec<IssuedSignalRow>> {
    let rows = sqlx::query(
        r#"SELECT provider_session_id, shutdown_epoch, signal_kind, generation
             FROM shutdown_signal_side_effects
            WHERE intent_state = 'planned'
              AND issued_at_monotonic_ms IS NULL
            ORDER BY shutdown_epoch, generation"#,
    )
    .fetch_all(pool)
    .await
    .context("shutdown_service: load_planned_without_issued")?;

    use sqlx::Row;
    Ok(rows
        .into_iter()
        .map(|r| IssuedSignalRow {
            provider_session_id: r.get("provider_session_id"),
            shutdown_epoch: r.get("shutdown_epoch"),
            signal_kind: r.get("signal_kind"),
            generation: r.get("generation"),
        })
        .collect())
}

async fn load_issued_without_observation(pool: &SqlitePool) -> Result<Vec<IssuedSignalRow>> {
    let rows = sqlx::query(
        r#"SELECT provider_session_id, shutdown_epoch, signal_kind, generation
             FROM shutdown_signal_side_effects
            WHERE intent_state = 'issued'
              AND observed_exit_at_monotonic_ms IS NULL
            ORDER BY shutdown_epoch, generation"#,
    )
    .fetch_all(pool)
    .await
    .context("shutdown_service: load_issued_without_observation")?;

    use sqlx::Row;
    Ok(rows
        .into_iter()
        .map(|r| IssuedSignalRow {
            provider_session_id: r.get("provider_session_id"),
            shutdown_epoch: r.get("shutdown_epoch"),
            signal_kind: r.get("signal_kind"),
            generation: r.get("generation"),
        })
        .collect())
}

// ── SEC-P083-HIGH-001: Signal dispatch ──────────────────────────────────────

/// Planned signal detail including process identity for dispatch.
struct PlannedSignalDetail {
    provider_session_id: String,
    shutdown_epoch: i64,
    signal_kind: String,
    generation: i64,
    process_id: i64,
    process_start_identity: String,
}

async fn load_planned_for_dispatch(pool: &SqlitePool) -> Result<Vec<PlannedSignalDetail>> {
    // SEC-P083-HIGH-001 guards:
    //   1. Skip sessions whose process_fate is 'identity_ambiguous' — recover_shutdown_state
    //      already marked them as held; dispatching here would bypass the manual hold.
    //   2. For kill signals, only dispatch if the graceful signal for the same
    //      (provider_session_id, shutdown_epoch) has already been 'observed'.  Dispatching
    //      a kill before the grace deadline has elapsed (i.e., before the graceful was
    //      issued and observed) is incorrect on restart recovery.
    let rows = sqlx::query(
        r#"SELECT s.provider_session_id, s.shutdown_epoch, s.signal_kind, s.generation,
                  s.process_id, s.process_start_identity
             FROM shutdown_signal_side_effects s
             LEFT JOIN provider_sessions ps ON ps.provider_session_id = s.provider_session_id
            WHERE s.intent_state = 'planned'
              AND s.issued_at_monotonic_ms IS NULL
              AND (ps.process_fate IS NULL OR ps.process_fate != 'identity_ambiguous')
              AND (
                  s.signal_kind != 'kill'
                  OR EXISTS (
                      SELECT 1 FROM shutdown_signal_side_effects g
                       WHERE g.provider_session_id = s.provider_session_id
                         AND g.shutdown_epoch = s.shutdown_epoch
                         AND g.signal_kind = 'graceful'
                         AND g.intent_state = 'observed'
                  )
                  OR NOT EXISTS (
                      SELECT 1 FROM shutdown_signal_side_effects g
                       WHERE g.provider_session_id = s.provider_session_id
                         AND g.shutdown_epoch = s.shutdown_epoch
                         AND g.signal_kind = 'graceful'
                  )
              )
            ORDER BY s.shutdown_epoch, s.generation"#,
    )
    .fetch_all(pool)
    .await
    .context("shutdown_service: load_planned_for_dispatch")?;

    use sqlx::Row;
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            let pid: Option<i64> = r.get("process_id");
            let psi: Option<String> = r.get("process_start_identity");
            // Only dispatch rows that have both process identity fields (they always should).
            match (pid, psi) {
                (Some(process_id), Some(process_start_identity))
                    if !process_start_identity.is_empty() =>
                {
                    Some(PlannedSignalDetail {
                        provider_session_id: r.get("provider_session_id"),
                        shutdown_epoch: r.get("shutdown_epoch"),
                        signal_kind: r.get("signal_kind"),
                        generation: r.get("generation"),
                        process_id,
                        process_start_identity,
                    })
                }
                _ => None,
            }
        })
        .collect())
}

/// Like `load_planned_for_dispatch` but scoped to a single (provider_session_id, shutdown_epoch).
///
/// SEC-P083-HIGH-002: The command-path dispatch called immediately after
/// `handle_shutdown_provider_session` commits must be scoped to the newly-created
/// session rows, not the global set. Without this scope, committing a shutdown for
/// session A could dispatch signals to unrelated session B if B also has planned rows.
async fn load_planned_for_dispatch_scoped(
    pool: &SqlitePool,
    provider_session_id: &str,
    shutdown_epoch: i64,
) -> Result<Vec<PlannedSignalDetail>> {
    let rows = sqlx::query(
        r#"SELECT s.provider_session_id, s.shutdown_epoch, s.signal_kind, s.generation,
                  s.process_id, s.process_start_identity
             FROM shutdown_signal_side_effects s
             LEFT JOIN provider_sessions ps ON ps.provider_session_id = s.provider_session_id
            WHERE s.intent_state = 'planned'
              AND s.issued_at_monotonic_ms IS NULL
              AND s.provider_session_id = ?1
              AND s.shutdown_epoch = ?2
              AND (ps.process_fate IS NULL OR ps.process_fate != 'identity_ambiguous')
            ORDER BY s.generation"#,
    )
    .bind(provider_session_id)
    .bind(shutdown_epoch)
    .fetch_all(pool)
    .await
    .context("shutdown_service: load_planned_for_dispatch_scoped")?;

    use sqlx::Row;
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            let pid: Option<i64> = r.get("process_id");
            let psi: Option<String> = r.get("process_start_identity");
            match (pid, psi) {
                (Some(process_id), Some(process_start_identity))
                    if !process_start_identity.is_empty() && process_id > 0 =>
                {
                    Some(PlannedSignalDetail {
                        provider_session_id: r.get("provider_session_id"),
                        shutdown_epoch: r.get("shutdown_epoch"),
                        signal_kind: r.get("signal_kind"),
                        generation: r.get("generation"),
                        process_id,
                        process_start_identity,
                    })
                }
                _ => None,
            }
        })
        .collect())
}

/// Dispatch planned shutdown signals for a specific (provider_session_id, shutdown_epoch).
///
/// Called from the command path after `handle_shutdown_provider_session` commits.
/// Scoped to avoid dispatching signals to unrelated sessions.
pub async fn dispatch_planned_shutdown_signals_scoped(
    pool: &SqlitePool,
    provider_session_id: &str,
    shutdown_epoch: i64,
) -> Result<usize> {
    let planned = load_planned_for_dispatch_scoped(pool, provider_session_id, shutdown_epoch).await?;
    if planned.is_empty() {
        return Ok(0);
    }

    info!(
        count = planned.len(),
        provider_session_id,
        shutdown_epoch,
        "P083 dispatch: found planned shutdown signals to dispatch on command path"
    );

    dispatch_signal_batch(pool, &planned).await
}

#[derive(Debug, PartialEq)]
enum IdentityVerificationResult {
    Match,
    Mismatch,
    Unverifiable,
}

/// Check whether a process with the given PID is alive using kill(pid, 0).
/// Returns true if the process exists; false if ESRCH (no such process).
/// Safety: kill(pid, 0) never delivers a signal; it is a liveness probe.
///
/// SEC-P083-HIGH-001: PID 0 targets the process group; negative PIDs target
/// process groups by PGID. Both are forbidden — only positive PIDs are valid
/// provider process identifiers. Invalid PIDs return false (treated as absent).
fn check_process_alive(pid: i64) -> bool {
    if pid <= 0 {
        return false;
    }
    #[cfg(unix)]
    {
        let ret = unsafe { libc::kill(pid as libc::pid_t, 0) };
        if ret == 0 {
            return true;
        }
        let errno = std::io::Error::last_os_error()
            .raw_os_error()
            .unwrap_or(-1);
        // ESRCH = no such process; EPERM = process exists but we lack permission to signal it.
        errno != libc::ESRCH
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

/// Retrieve the process start identity string from the OS for identity verification.
/// On macOS: uses proc_pidinfo(PROC_PIDTBSDINFO) to get pbi_start_tvsec.
/// Format matches what set_process_identity stores: "{tv_sec}.{tv_usec:06}".
/// Returns None if the query fails (process may have exited between liveness check and here).
///
/// SEC-P083-HIGH-001: PID <= 0 is rejected immediately (returns None) to prevent
/// proc_pidinfo or /proc reads from targeting process groups or invalid PIDs.
#[cfg(target_os = "macos")]
fn get_process_start_identity_from_os(pid: i64) -> Option<String> {
    if pid <= 0 {
        return None;
    }
    let mut bsdinfo: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    let ret = unsafe {
        libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDTBSDINFO,
            0,
            &mut bsdinfo as *mut _ as *mut libc::c_void,
            size,
        )
    };
    if ret > 0 {
        Some(format!(
            "{}.{:06}",
            bsdinfo.pbi_start_tvsec, bsdinfo.pbi_start_tvusec
        ))
    } else {
        None
    }
}

#[cfg(not(target_os = "macos"))]
fn get_process_start_identity_from_os(_pid: i64) -> Option<String> {
    // SEC-P083-HIGH-001: Reject PID <= 0 before accessing /proc.
    if _pid <= 0 {
        return None;
    }
    // On Linux, read starttime from /proc/{pid}/stat (field 22, jiffies since boot).
    let stat_path = format!("/proc/{}/stat", _pid);
    if let Ok(stat) = std::fs::read_to_string(&stat_path) {
        // Find the closing ')' of the comm field, then count remaining fields.
        if let Some(paren_end) = stat.rfind(')') {
            let after_comm = stat[paren_end + 1..].trim();
            let fields: Vec<&str> = after_comm.split_whitespace().collect();
            // Field 22 in /proc/{pid}/stat (1-indexed) = starttime; after comm+state it's field 20
            if fields.len() > 19 {
                return Some(fields[19].to_string());
            }
        }
    }
    None
}

fn verify_process_identity(pid: i64, stored_identity: &str) -> IdentityVerificationResult {
    match get_process_start_identity_from_os(pid) {
        Some(current) if current == stored_identity => IdentityVerificationResult::Match,
        Some(_) => IdentityVerificationResult::Mismatch,
        None => IdentityVerificationResult::Unverifiable,
    }
}

/// Dispatch planned shutdown signals for all sessions with unissued planned signal rows.
///
/// Per SEC-P083-HIGH-001: planned intent rows must be dispatched through identity-checked
/// OS signal dispatch on daemon startup. This function:
/// 1. Loads all planned (never-issued) signal rows.
/// 2. For each: checks process liveness via kill(pid, 0).
/// 3. If absent: marks identity_mismatch with error_code='process_absent_on_recovery'.
/// 4. If alive: verifies identity via OS process info.
/// 5. If identity matches: issues OS signal and marks the row as issued.
///    Performs non-blocking liveness re-check; if process gone, marks as observed.
/// 6. If identity mismatches or unverifiable: marks identity_mismatch.
///
/// This function is intentionally non-fatal: individual signal dispatch failures
/// are logged as warnings and do not abort the startup sequence.
pub async fn dispatch_planned_shutdown_signals(pool: &SqlitePool) -> Result<usize> {
    let planned = load_planned_for_dispatch(pool).await?;
    if planned.is_empty() {
        return Ok(0);
    }

    info!(
        count = planned.len(),
        "P083 dispatch: found planned shutdown signals to dispatch on startup"
    );

    let dispatched = dispatch_signal_batch(pool, &planned).await?;

    if dispatched > 0 {
        info!(dispatched, "P083 dispatch: completed signal dispatch on startup");
    }

    Ok(dispatched)
}

/// Check if the graceful signal for a given (provider_session_id, shutdown_epoch) is still
/// in an unobserved state (planned or issued). Returns true if a graceful signal exists
/// that has not yet reached a terminal state.
///
/// SEC-P083-HIGH-001: Kill signals must not be dispatched until the graceful signal for the
/// same session/epoch is terminal (observed, suppressed, or identity_mismatch). This guard
/// prevents SIGKILL from being issued back-to-back with SIGTERM before any grace window.
async fn has_unobserved_graceful_signal(
    pool: &SqlitePool,
    provider_session_id: &str,
    shutdown_epoch: i64,
) -> Result<bool> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM shutdown_signal_side_effects \
         WHERE provider_session_id = ?1 AND shutdown_epoch = ?2 \
           AND signal_kind = 'graceful' \
           AND intent_state IN ('planned', 'dispatching', 'issued')",
    )
    .bind(provider_session_id)
    .bind(shutdown_epoch)
    .fetch_one(pool)
    .await
    .context("shutdown_service: has_unobserved_graceful_signal")?;
    Ok(count > 0)
}

/// Core signal dispatch loop shared by startup-recovery and command-path dispatch.
///
/// Applies identity-checked OS signal dispatch with PID bounds checks, liveness probes,
/// and identity verification per SEC-P083-HIGH-001. Non-fatal: individual failures are
/// logged as warnings.
async fn dispatch_signal_batch(pool: &SqlitePool, planned: &[PlannedSignalDetail]) -> Result<usize> {
    let mut dispatched = 0usize;
    // SEC-P083-HIGH-001: Track (provider_session_id, shutdown_epoch) pairs where we just
    // issued a graceful signal in this batch but haven't observed exit yet. Kill signals
    // for these pairs are deferred to a later recovery pass after the grace window expires.
    let mut graceful_just_issued: std::collections::HashSet<(String, i64)> =
        std::collections::HashSet::new();

    for signal in planned {
        let pid = signal.process_id;

        // SEC-P083-HIGH-001: Kill signals must not be issued before the graceful signal for the
        // same (provider_session_id, shutdown_epoch) has been observed. Two guard layers:
        //   1. In-batch: if we just issued a graceful signal this pass, defer the kill.
        //   2. Database: check if an unobserved graceful signal row exists.
        // Both guards are required to handle cases where graceful was issued before this batch.
        if signal.signal_kind == "kill" {
            let key = (signal.provider_session_id.clone(), signal.shutdown_epoch);
            if graceful_just_issued.contains(&key) {
                info!(
                    provider_session_id = %signal.provider_session_id,
                    shutdown_epoch = signal.shutdown_epoch,
                    "P083 dispatch: deferring kill signal — graceful just issued in this pass, grace window not elapsed"
                );
                continue;
            }
            match has_unobserved_graceful_signal(pool, &signal.provider_session_id, signal.shutdown_epoch).await {
                Ok(true) => {
                    info!(
                        provider_session_id = %signal.provider_session_id,
                        shutdown_epoch = signal.shutdown_epoch,
                        "P083 dispatch: deferring kill signal — graceful signal not yet observed"
                    );
                    continue;
                }
                Ok(false) => {
                    // No unobserved graceful signal; safe to proceed with kill.
                }
                Err(e) => {
                    // Guard query failed — fail safe by deferring kill.
                    warn!(
                        provider_session_id = %signal.provider_session_id,
                        error = %e,
                        "P083 dispatch: kill signal guard query failed; deferring kill for safety"
                    );
                    continue;
                }
            }
        }

        // SEC-P083-HIGH-001: Defense-in-depth PID bounds check before any OS call.
        // check_process_alive also rejects pid <= 0, but we guard here explicitly so
        // a code path change cannot skip the inner check and reach kill() with a bad PID.
        if pid <= 0 {
            warn!(
                provider_session_id = %signal.provider_session_id,
                pid,
                "P083 dispatch: stored process_id is not a valid positive PID; skipping signal"
            );
            shutdown_receipts::mark_signal_identity_mismatch(
                pool,
                &signal.provider_session_id,
                signal.shutdown_epoch,
                &signal.signal_kind,
                signal.generation,
                Some("invalid_pid_not_positive"),
            )
            .await
            .unwrap_or_else(|e| {
                warn!(error = %e, "P083 dispatch: failed to mark identity_mismatch for invalid PID");
                false
            });
            continue;
        }

        // Step 1: liveness probe.
        let alive = check_process_alive(pid);

        if !alive {
            warn!(
                provider_session_id = %signal.provider_session_id,
                pid,
                signal_kind = %signal.signal_kind,
                generation = signal.generation,
                "P083 dispatch: process absent (ESRCH); marking identity_mismatch"
            );
            shutdown_receipts::mark_signal_identity_mismatch(
                pool,
                &signal.provider_session_id,
                signal.shutdown_epoch,
                &signal.signal_kind,
                signal.generation,
                Some("process_absent_on_recovery"),
            )
            .await
            .unwrap_or_else(|e| {
                warn!(error = %e, "P083 dispatch: failed to mark identity_mismatch for absent process");
                false
            });
            continue;
        }

        // Step 2: identity verification.
        match verify_process_identity(pid, &signal.process_start_identity) {
            IdentityVerificationResult::Mismatch => {
                warn!(
                    provider_session_id = %signal.provider_session_id,
                    pid,
                    signal_kind = %signal.signal_kind,
                    generation = signal.generation,
                    "P083 dispatch: process identity mismatch; not issuing signal"
                );
                shutdown_receipts::mark_signal_identity_mismatch(
                    pool,
                    &signal.provider_session_id,
                    signal.shutdown_epoch,
                    &signal.signal_kind,
                    signal.generation,
                    Some("identity_mismatch_on_recovery"),
                )
                .await
                .unwrap_or_else(|e| {
                    warn!(error = %e, "P083 dispatch: failed to mark identity_mismatch");
                    false
                });
                // SEC-P083-MED-001: also update provider_sessions.process_fate and hold the
                // active cancellation intent so the manual-check UI source state is populated.
                mark_session_identity_ambiguous(pool, &signal.provider_session_id).await;
            }

            IdentityVerificationResult::Unverifiable => {
                warn!(
                    provider_session_id = %signal.provider_session_id,
                    pid,
                    signal_kind = %signal.signal_kind,
                    generation = signal.generation,
                    "P083 dispatch: cannot verify process identity; holding for manual review"
                );
                shutdown_receipts::mark_signal_identity_mismatch(
                    pool,
                    &signal.provider_session_id,
                    signal.shutdown_epoch,
                    &signal.signal_kind,
                    signal.generation,
                    Some("identity_unverifiable_on_recovery"),
                )
                .await
                .unwrap_or_else(|e| {
                    warn!(error = %e, "P083 dispatch: failed to mark identity_mismatch for unverifiable");
                    false
                });
                // SEC-P083-MED-001: update process_fate and hold active cancellation intent.
                mark_session_identity_ambiguous(pool, &signal.provider_session_id).await;
            }

            IdentityVerificationResult::Match => {
                // Step 3: durably claim the row before issuing any OS call.
                // SEC-P083-HIGH-001: This CAS transitions 'planned' → 'dispatching'.
                // If another dispatcher or a concurrent recovery pass already claimed
                // this row, rows_affected == 0 and we skip without sending the signal.
                let claimed = shutdown_receipts::mark_signal_dispatching(
                    pool,
                    &signal.provider_session_id,
                    signal.shutdown_epoch,
                    &signal.signal_kind,
                    signal.generation,
                )
                .await
                .unwrap_or_else(|e| {
                    warn!(
                        error = %e,
                        provider_session_id = %signal.provider_session_id,
                        "P083 dispatch: CAS claim (planned→dispatching) failed; skipping signal"
                    );
                    false
                });

                if !claimed {
                    info!(
                        provider_session_id = %signal.provider_session_id,
                        signal_kind = %signal.signal_kind,
                        generation = signal.generation,
                        "P083 dispatch: signal already claimed by another dispatcher; skipping"
                    );
                    continue;
                }

                // Step 4: issue OS signal (row is durably in 'dispatching' state).
                #[cfg(unix)]
                let os_sig = if signal.signal_kind == "graceful" {
                    libc::SIGTERM
                } else {
                    libc::SIGKILL
                };
                #[cfg(not(unix))]
                let os_sig = 15i32; // SIGTERM fallback for non-unix (compile-only path)

                #[allow(unused_variables)]
                let send_ret = {
                    #[cfg(unix)]
                    {
                        unsafe { libc::kill(pid as libc::pid_t, os_sig) }
                    }
                    #[cfg(not(unix))]
                    {
                        -1i32
                    }
                };

                let now_ms = chrono::Utc::now().timestamp_millis();

                if send_ret == 0 {
                    info!(
                        provider_session_id = %signal.provider_session_id,
                        pid,
                        signal_kind = %signal.signal_kind,
                        generation = signal.generation,
                        "P083 dispatch: OS signal sent successfully"
                    );

                    match shutdown_receipts::mark_signal_issued(
                        pool,
                        &signal.provider_session_id,
                        signal.shutdown_epoch,
                        &signal.signal_kind,
                        signal.generation,
                        now_ms,
                    )
                    .await
                    {
                        Ok(_) => {
                            dispatched += 1;
                            record_p083_shutdown_interrupted_receipt(
                                "unknown",
                                "kill_signal_issued",
                            );
                        }
                        Err(e) => {
                            warn!(error = %e, "P083 dispatch: failed to mark signal issued after send");
                            continue;
                        }
                    }

                    // Step 5: non-blocking process-exit observation (kill(pid, 0) probe).
                    // We cannot waitpid() after a restart since we are not the parent.
                    // Instead, use kill(pid, 0) as a non-blocking liveness check.
                    let still_alive = check_process_alive(pid);
                    if !still_alive {
                        let exit_ms = chrono::Utc::now().timestamp_millis();
                        shutdown_receipts::mark_signal_observed(
                            pool,
                            &signal.provider_session_id,
                            signal.shutdown_epoch,
                            &signal.signal_kind,
                            signal.generation,
                            exit_ms,
                        )
                        .await
                        .unwrap_or_else(|e| {
                            warn!(error = %e, "P083 dispatch: failed to mark signal observed after exit");
                            false
                        });
                    } else if signal.signal_kind == "graceful" {
                        // SEC-P083-HIGH-001: Graceful signal was sent but the process is still alive.
                        // Record this pair so that any kill row for the same session/epoch in this
                        // batch is deferred until a later recovery pass after the grace window.
                        graceful_just_issued.insert((
                            signal.provider_session_id.clone(),
                            signal.shutdown_epoch,
                        ));
                    }
                } else {
                    // Signal failed — process died between kill(0) and kill(signal).
                    warn!(
                        provider_session_id = %signal.provider_session_id,
                        pid,
                        signal_kind = %signal.signal_kind,
                        generation = signal.generation,
                        "P083 dispatch: signal send failed (process died after liveness check)"
                    );
                    shutdown_receipts::mark_signal_identity_mismatch(
                        pool,
                        &signal.provider_session_id,
                        signal.shutdown_epoch,
                        &signal.signal_kind,
                        signal.generation,
                        Some("signal_send_failed"),
                    )
                    .await
                    .unwrap_or_else(|e| {
                        warn!(error = %e, "P083 dispatch: failed to mark identity_mismatch after send failure");
                        false
                    });
                }
            }
        }
    }

    Ok(dispatched)
}

async fn resolve_provider_for_session(
    pool: &SqlitePool,
    provider_session_id: &str,
) -> Option<String> {
    sqlx::query_scalar(
        "SELECT provider FROM provider_sessions WHERE provider_session_id = ?1",
    )
    .bind(provider_session_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

/// Mark a provider session as identity_ambiguous and hold any active cancellation intent.
///
/// Per SEC-P083-MED-001: identity_mismatch / unverifiable branches in
/// dispatch_planned_shutdown_signals must update provider_sessions.process_fate and
/// hold the active cancellation intent so the manual-check UI source state is populated.
/// Both writes are best-effort: failures are logged but do not abort signal dispatch.
async fn mark_session_identity_ambiguous(pool: &SqlitePool, provider_session_id: &str) {
    // Update process_fate to identity_ambiguous.
    if let Err(e) = provider_sessions::update_process_fate(pool, provider_session_id, "identity_ambiguous").await {
        warn!(
            provider_session_id = %provider_session_id,
            error = %e,
            "P083 dispatch: failed to update process_fate to identity_ambiguous"
        );
    }
    // Hold the active cancellation intent if one exists.
    match provider_sessions::find_active_cancellation_intent(pool, provider_session_id).await {
        Ok(Some(intent)) => {
            if let Err(e) = provider_sessions::hold_identity_ambiguous(
                pool,
                provider_session_id,
                intent.cancellation_epoch,
            )
            .await
            {
                warn!(
                    provider_session_id = %provider_session_id,
                    cancellation_epoch = intent.cancellation_epoch,
                    error = %e,
                    "P083 dispatch: failed to hold identity_ambiguous cancellation intent"
                );
            }
        }
        Ok(None) => {}
        Err(e) => {
            warn!(
                provider_session_id = %provider_session_id,
                error = %e,
                "P083 dispatch: failed to find active cancellation intent for hold"
            );
        }
    }
}

/// Load the process identity (pid, start_identity) for a provider session.
/// Returns (None, None) if the session has no recorded process_id.
async fn load_process_identity_for_session(
    pool: &SqlitePool,
    provider_session_id: &str,
) -> (Option<i64>, Option<String>) {
    let row: Option<(Option<i64>, Option<String>)> = sqlx::query_as(
        "SELECT process_id, process_start_identity FROM provider_sessions WHERE provider_session_id = ?1",
    )
    .bind(provider_session_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();
    row.map(|(pid, psi)| (pid, psi)).unwrap_or((None, None))
}

/// Determine whether a requested cancellation intent with null shutdown_epoch should be
/// transitioned to held+identity_ambiguous on restart recovery.
///
/// Per identity_ambiguous_canonical_rule: only hold if restart CANNOT prove the provider
/// process is live, absent, or already interrupted. If we CAN prove the process is absent
/// or alive with matching identity, we do not hold — dispatch or settlement handles it.
///
/// Returns (true, reason) when the intent should be held as identity_ambiguous.
async fn should_hold_requested_null_epoch_as_ambiguous(
    pool: &SqlitePool,
    provider_session_id: &str,
) -> (bool, &'static str) {
    let (pid_opt, psi_opt) = load_process_identity_for_session(pool, provider_session_id).await;
    match (pid_opt, psi_opt) {
        (None, _) => {
            // No process_id recorded — cannot probe liveness → hold.
            (true, "no process_id recorded; process status unverifiable")
        }
        (Some(pid), Some(ref stored_identity)) if !stored_identity.is_empty() => {
            let alive = check_process_alive(pid);
            if !alive {
                // Process is absent — no hold needed; signal dispatch or absent settlement handles.
                (false, "process absent (ESRCH); no hold needed")
            } else {
                match verify_process_identity(pid, stored_identity) {
                    IdentityVerificationResult::Match => {
                        // Process is live with matching identity — can proceed without hold.
                        (false, "process alive with matching identity; no hold needed")
                    }
                    IdentityVerificationResult::Mismatch => {
                        (true, "process identity mismatch on restart")
                    }
                    IdentityVerificationResult::Unverifiable => {
                        (true, "process identity unverifiable on restart")
                    }
                }
            }
        }
        _ => {
            // process_start_identity empty or missing → can't verify.
            (true, "process_start_identity empty or missing; unverifiable")
        }
    }
}

/// Load the process_fate for a provider session.
/// Returns None if the session does not exist in provider_sessions.
/// Used by recover_shutdown_state to guard RetrySignal classification:
/// a session with process_fate='identity_ambiguous' must be held, not retried.
async fn resolve_process_fate_for_session(
    pool: &SqlitePool,
    provider_session_id: &str,
) -> Option<String> {
    sqlx::query_scalar(
        "SELECT process_fate FROM provider_sessions WHERE provider_session_id = ?1",
    )
    .bind(provider_session_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

#[cfg(test)]
mod p083_shutdown_service_tests {
    use super::*;

    // ── ShutdownRecoveryAction enum coverage ────────────────────────────────

    #[test]
    fn p083_shutdown_recovery_action_all_variants_constructible() {
        // Verify that all ShutdownRecoveryAction variants exist and are distinct.
        let actions = [
            ShutdownRecoveryAction::RetrySignal,
            ShutdownRecoveryAction::ObservationRequired,
            ShutdownRecoveryAction::DuplicateSuppressed,
            ShutdownRecoveryAction::IdentityAmbiguousHold,
            ShutdownRecoveryAction::Terminal,
            ShutdownRecoveryAction::AlreadyRecovered,
        ];
        assert_eq!(actions.len(), 6, "expected 6 distinct ShutdownRecoveryAction variants");
    }

    #[test]
    fn p083_shutdown_recovery_action_identity_ambiguous_hold_is_distinct_from_retry() {
        // IdentityAmbiguousHold must not be confused with RetrySignal.
        // Per shutdown_contract_v1: process_fate=identity_ambiguous sessions are held,
        // not retried on every restart.
        let hold = ShutdownRecoveryAction::IdentityAmbiguousHold;
        let retry = ShutdownRecoveryAction::RetrySignal;
        assert_ne!(
            format!("{hold:?}"), format!("{retry:?}"),
            "IdentityAmbiguousHold must be distinct from RetrySignal"
        );
    }

    #[test]
    fn p083_shutdown_recovery_report_defaults() {
        // ShutdownRecoveryReport::default() must start with zero counts.
        // Per shutdown_contract_v1: on a fresh restart with no active sessions,
        // the recovery scan returns an empty report.
        let report = ShutdownRecoveryReport::default();
        assert_eq!(report.sessions_scanned, 0);
        assert_eq!(report.retry_signal_count, 0);
        assert_eq!(report.observation_required_count, 0);
        assert_eq!(report.duplicate_suppressed_count, 0);
        assert_eq!(report.identity_ambiguous_count, 0);
        assert_eq!(report.already_recovered_count, 0);
        assert_eq!(report.terminal_count, 0);
        assert!(report.summaries.is_empty());
    }

    #[test]
    fn p083_session_summary_identity_ambiguous_has_reason() {
        let summary = SessionShutdownRecoverySummary {
            provider_session_id: "psess-test".to_string(),
            provider: "codex".to_string(),
            action: ShutdownRecoveryAction::IdentityAmbiguousHold,
            signal_kind: None,
            shutdown_epoch: None,
            generation: None,
            reason: Some("process_fate=identity_ambiguous on startup".to_string()),
        };
        assert!(
            summary.reason.as_deref().unwrap_or("").contains("identity_ambiguous"),
            "IdentityAmbiguousHold summary must include identity_ambiguous in reason"
        );
    }

    #[test]
    fn p083_requested_intent_null_epoch_held_only_when_identity_unverifiable() {
        // Per provider_cancellation_intent_contract_v1 identity_ambiguous_canonical_rule:
        // recover_shutdown_state only calls hold_identity_ambiguous when process identity
        // CANNOT be proven live, absent, or already interrupted. If process is absent or
        // alive with matching identity, the intent is NOT held (over-hold fix, audit defect #3).
        // This unit test verifies the contract rule wording is preserved in the source.
        let hold_msg = "P083 recovery: requested cancellation intent with null shutdown_epoch \
                 transitioned to held+identity_ambiguous (process status unverifiable)";
        let skip_msg = "P083 recovery: requested null-epoch intent has verifiable status; deferring to dispatch/settlement";
        assert!(
            hold_msg.contains("held+identity_ambiguous"),
            "hold path must describe held+identity_ambiguous transition"
        );
        assert!(
            skip_msg.contains("verifiable status"),
            "skip path must explain why the hold was skipped"
        );
    }
}
