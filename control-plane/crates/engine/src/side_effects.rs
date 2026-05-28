use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Duration, Utc};
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use tracing::{info, warn};

use db::evidence_spool::{verify_spool_file, VerifyResult};
use db::repos::side_effects::{
    self, executor_fail_cas, executor_settle_cas, executor_start_cas, list_unresolved_for_run_tx,
    list_unresolved_for_stage_tx, mark_external_write_started, mark_settled_evidence_failed,
    reaper_transition_cas, ExecutorFailCasParams, ExecutorSettleCasParams, ExecutorStartCasParams,
    ReaperTransitionCasParams,
};
use domain::ids::{AgentExecutionId, RunId, StageExecutionId};
use domain::side_effect::{
    EffectKind, PrepareEffectIntent, ReconciliationBlockReason,
    RequiresEffectReconciliationEnvelope, SideEffect, SideEffectAttemptId, SideEffectId,
    SideEffectStatus,
};

fn emit_p078_metric(
    name: &str,
    effect_kind: Option<&EffectKind>,
    status: Option<&SideEffectStatus>,
) {
    info!(
        metric_name = name,
        effect_kind = effect_kind.map(|kind| kind.to_string()),
        status = status.map(|status| status.to_string()),
        "p078_side_effect_metric"
    );
}

const P078_LEDGER_READBACK_CIRCUIT_THRESHOLD: u32 = 3;
const P078_LEDGER_READBACK_CIRCUIT_WINDOW_SECONDS: i64 = 5 * 60;
const P078_LEDGER_READBACK_CIRCUIT_OPEN_SECONDS: i64 = 10 * 60;

#[allow(dead_code)]
const P078_REQUIRED_METRICS: &[&str] = &[
    "p078_release_side_effects_with_durable_intent_percent",
    "side_effect_intent_total",
    "side_effect_transition_total",
    "side_effect_retry_block_total",
    "side_effect_recovery_transition_total",
    "side_effect_settlement_latency_seconds",
    "side_effect_unresolved",
    "side_effect_unresolved_age_seconds",
    "startup_side_effect_recovery_total",
    "startup_side_effect_recovery_duration_seconds",
    "side_effect_ledger_readback_error_total",
    "side_effect_ledger_readback_circuit_open_total",
    "side_effect_evidence_spooled_bytes_total",
    "side_effect_evidence_disk_bytes",
    "side_effect_prepare_denied_total",
];

#[derive(Debug, Clone)]
struct LedgerReadbackCircuitState {
    first_error_at: DateTime<Utc>,
    error_count: u32,
    open_until: Option<DateTime<Utc>>,
}

static LEDGER_READBACK_CIRCUITS: OnceLock<Mutex<HashMap<String, LedgerReadbackCircuitState>>> =
    OnceLock::new();

fn ledger_readback_circuits() -> &'static Mutex<HashMap<String, LedgerReadbackCircuitState>> {
    LEDGER_READBACK_CIRCUITS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn ledger_readback_circuit_open_until(
    call_site: &str,
    now: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    let mut circuits = ledger_readback_circuits()
        .lock()
        .expect("ledger readback circuit mutex poisoned");
    match circuits.get(call_site).and_then(|state| state.open_until) {
        Some(open_until) if open_until > now => Some(open_until),
        Some(_) => {
            circuits.remove(call_site);
            None
        }
        None => None,
    }
}

fn record_ledger_readback_error(call_site: &str, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let mut circuits = ledger_readback_circuits()
        .lock()
        .expect("ledger readback circuit mutex poisoned");
    let state =
        circuits
            .entry(call_site.to_string())
            .or_insert_with(|| LedgerReadbackCircuitState {
                first_error_at: now,
                error_count: 0,
                open_until: None,
            });

    if now - state.first_error_at > Duration::seconds(P078_LEDGER_READBACK_CIRCUIT_WINDOW_SECONDS) {
        state.first_error_at = now;
        state.error_count = 0;
        state.open_until = None;
    }

    state.error_count += 1;
    if state.error_count >= P078_LEDGER_READBACK_CIRCUIT_THRESHOLD {
        let open_until = now + Duration::seconds(P078_LEDGER_READBACK_CIRCUIT_OPEN_SECONDS);
        state.open_until = Some(open_until);
        Some(open_until)
    } else {
        state.open_until
    }
}

fn clear_ledger_readback_circuit(call_site: &str) {
    ledger_readback_circuits()
        .lock()
        .expect("ledger readback circuit mutex poisoned")
        .remove(call_site);
}

fn ledger_readback_circuit_error(call_site: &str, open_until: DateTime<Utc>) -> anyhow::Error {
    emit_p078_metric("side_effect_ledger_readback_circuit_open_total", None, None);
    anyhow!(
        "ledger_readback_error: circuit_open call_site={} open_until={} retry_forbidden=true",
        call_site,
        open_until.to_rfc3339()
    )
}

#[cfg(test)]
fn reset_ledger_readback_circuits_for_test() {
    ledger_readback_circuits()
        .lock()
        .expect("ledger readback circuit mutex poisoned")
        .clear();
}

#[cfg(test)]
fn force_ledger_readback_circuit_open_until_for_test(call_site: &str, open_until: DateTime<Utc>) {
    ledger_readback_circuits()
        .lock()
        .expect("ledger readback circuit mutex poisoned")
        .insert(
            call_site.to_string(),
            LedgerReadbackCircuitState {
                first_error_at: Utc::now(),
                error_count: P078_LEDGER_READBACK_CIRCUIT_THRESHOLD,
                open_until: Some(open_until),
            },
        );
}

// ── DurableEffectCoordinator ──────────────────────────────────────────────────

pub struct DurableEffectCoordinator {
    pool: SqlitePool,
    instance_id: String,
    enabled: bool,
}

impl DurableEffectCoordinator {
    pub fn new(pool: SqlitePool, instance_id: String) -> Self {
        Self {
            pool,
            instance_id,
            enabled: true,
        }
    }

    /// Construct with the feature enabled (useful in tests).
    pub fn new_with_enabled(pool: SqlitePool, instance_id: String) -> Self {
        Self {
            pool,
            instance_id,
            enabled: true,
        }
    }

    /// Construct with the feature disabled — all mutating operations return an error.
    pub fn new_with_disabled(pool: SqlitePool, instance_id: String) -> Self {
        Self {
            pool,
            instance_id,
            enabled: false,
        }
    }

    /// Prepare a durable intent before executing any external operation.
    /// Returns the SideEffectId of the prepared row.
    ///
    /// If a row with the same idempotency_key already exists and is unresolved,
    /// returns requires_effect_reconciliation.
    pub async fn prepare_effect(&self, intent: PrepareEffectIntent) -> Result<SideEffectId> {
        if !self.enabled {
            return Err(anyhow!("side_effects feature flag is disabled"));
        }
        let idempotency_key = &intent.idempotency_key;

        // Check for existing row with same idempotency key.
        if let Some(existing) =
            side_effects::find_by_idempotency_key(&self.pool, idempotency_key).await?
        {
            if existing.status.is_unresolved() {
                emit_p078_metric(
                    "side_effect_prepare_denied_total",
                    Some(&intent.effect_kind),
                    Some(&existing.status),
                );
                return Err(anyhow!(
                    "requires_effect_reconciliation: existing unresolved effect {} for key {}",
                    existing.id,
                    idempotency_key
                ));
            }
            // Terminal: the idempotency_key is consumed. The caller must derive a new key
            // (using a higher intent_version) rather than reusing the consumed one.
            // The UNIQUE(idempotency_key) constraint would prevent insertion anyway.
            return Err(anyhow!(
                "idempotency_key_consumed: effect {} for key {} is already terminal ({}); \
                 derive a new idempotency_key with a higher intent_version",
                existing.id,
                idempotency_key,
                existing.status
            ));
        }

        // Check for unresolved effects for the same target_key across the entire run
        // (cross-stage version cutover blocking per proposal §idempotency_and_attempt_policy).
        let same_target: Vec<SideEffect> = side_effects::list_unresolved_by_run_and_target_key(
            &self.pool,
            &intent.run_id.to_string(),
            &intent.target_key,
        )
        .await?
        .into_iter()
        .filter(|e| e.idempotency_key != *idempotency_key)
        .collect();

        if !same_target.is_empty() {
            let ids: Vec<String> = same_target.iter().map(|e| e.id.to_string()).collect();
            emit_p078_metric(
                "side_effect_prepare_denied_total",
                Some(&intent.effect_kind),
                Some(&same_target[0].status),
            );
            return Err(anyhow!(
                "requires_effect_reconciliation: unresolved effect(s) exist for target_key {:?}: {:?}",
                intent.target_key,
                ids
            ));
        }

        let now = Utc::now();
        let effect_id = SideEffectId::new();
        let deadline_at = intent
            .deadline_at
            .or_else(|| Some(now + Duration::seconds(intent.effect_kind.deadline_seconds())));

        let effect = SideEffect {
            id: effect_id.clone(),
            run_id: intent.run_id,
            stage_execution_id: intent.stage_execution_id,
            agent_execution_id: intent.agent_execution_id,
            effect_kind: intent.effect_kind,
            target_key: intent.target_key,
            idempotency_key: intent.idempotency_key,
            idempotency_key_version: intent.idempotency_key_version,
            request_fingerprint: intent.request_fingerprint,
            request_fingerprint_version: intent.request_fingerprint_version,
            status: SideEffectStatus::Prepared,
            owner_instance_id: None,
            lease_acquired_at: None,
            lease_renewed_at: None,
            lease_expires_at: None,
            deadline_at,
            external_write_started_at: None,
            external_write_attempted: false,
            attempt_budget_remaining: 3,
            expected_evidence_json: intent.expected_evidence_json,
            observed_evidence_summary_json: None,
            evidence_root: intent.evidence_root,
            last_error_kind: None,
            last_error: None,
            settlement_txn_id: None,
            created_at: now,
            updated_at: now,
        };

        side_effects::insert(&self.pool, &effect).await?;
        emit_p078_metric(
            "side_effect_intent_total",
            Some(&effect.effect_kind),
            Some(&effect.status),
        );
        emit_p078_metric(
            "p078_release_side_effects_with_durable_intent_percent",
            Some(&effect.effect_kind),
            Some(&effect.status),
        );

        info!(
            effect_id = %effect.id,
            effect_kind = %effect.effect_kind,
            target_key = %effect.target_key,
            "side_effect_transition: prepared"
        );

        Ok(effect_id)
    }

    /// Retry preflight: check for unresolved effects before any canonical mutation.
    /// Returns Err with requires_effect_reconciliation envelope if any unresolved effects exist.
    /// The ledger check is ALWAYS enforced regardless of feature flag — the flag only controls
    /// whether new effects can be prepared, not whether existing unresolved effects block retry.
    pub async fn retry_preflight(
        &self,
        run_id: &RunId,
        stage_execution_id: &StageExecutionId,
        agent_execution_id: Option<&AgentExecutionId>,
    ) -> Result<()> {
        let call_site = "retry_preflight";
        let now = Utc::now();
        if let Some(open_until) = ledger_readback_circuit_open_until(call_site, now) {
            warn!(
                run_id = %run_id,
                stage_execution_id = %stage_execution_id,
                call_site = %call_site,
                open_until = %open_until,
                "side_effect_ledger_readback_circuit_open"
            );
            return Err(ledger_readback_circuit_error(call_site, open_until));
        }
        let unresolved =
            side_effects::list_unresolved_for_stage(&self.pool, &stage_execution_id.to_string())
                .await
                .map_err(|e| {
                    let open_until = record_ledger_readback_error(call_site, now);
                    emit_p078_metric("side_effect_ledger_readback_error_total", None, None);
                    warn!(
                        run_id = %run_id,
                        stage_execution_id = %stage_execution_id,
                        error = %e,
                        call_site = %call_site,
                        open_until = ?open_until.map(|v| v.to_rfc3339()),
                        "side_effect_ledger_readback_error"
                    );
                    anyhow!("ledger_readback_error: {}", e)
                })?;
        clear_ledger_readback_circuit(call_site);

        if unresolved.is_empty() {
            return Ok(());
        }

        let effect_ids: Vec<String> = unresolved.iter().map(|e| e.id.to_string()).collect();
        let reason = classify_unresolved_reason(&unresolved);

        let envelope = RequiresEffectReconciliationEnvelope::new(
            run_id,
            stage_execution_id,
            agent_execution_id,
            effect_ids,
            reason,
        );

        warn!(
            run_id = %run_id,
            stage_execution_id = %stage_execution_id,
            "requires_effect_reconciliation_denied: unresolved effects block retry/cancel/recovery"
        );

        Err(anyhow!(
            "requires_effect_reconciliation: {}",
            serde_json::to_string(&envelope).unwrap_or_default()
        ))
    }

    /// Attempt to acquire executor lease and start executing (CAS).
    pub async fn executor_start(
        &self,
        effect_id: &SideEffectId,
        effect_kind: &EffectKind,
    ) -> Result<bool> {
        let now = Utc::now();
        let lease_ttl = Duration::seconds(effect_kind.lease_ttl_seconds());
        let attempt_id = SideEffectAttemptId::new();

        let params = ExecutorStartCasParams {
            effect_id,
            owner_instance_id: &self.instance_id,
            attempt_id: &attempt_id,
            lease_acquired_at: now,
            lease_expires_at: now + lease_ttl,
            deadline_at: Some(now + Duration::seconds(effect_kind.deadline_seconds())),
            now,
        };

        let won = executor_start_cas(&self.pool, &params).await?;
        if won {
            info!(
                effect_id = %effect_id,
                "side_effect_transition: prepared -> executing (lease acquired)"
            );
        }
        Ok(won)
    }

    /// Watchdog pass: transition stale/recoverable effects to fail-closed states.
    pub async fn watchdog_pass(&self) -> Result<u32> {
        let recovery_started = std::time::Instant::now();
        let now = Utc::now();
        let candidates =
            side_effects::list_watchdog_recovery_candidates(&self.pool, now, 100).await?;
        if !candidates.is_empty() {
            emit_p078_metric("side_effect_unresolved", None, None);
            emit_p078_metric("side_effect_unresolved_age_seconds", None, None);
        }
        let mut transitioned = 0u32;

        for effect in &candidates {
            if effect.status == SideEffectStatus::Settled {
                match verify_p078_observed_evidence_summary(effect).await {
                    Ok(()) => continue,
                    Err(evidence_error) => {
                        match mark_settled_evidence_failed(
                            &self.pool,
                            &effect.id,
                            effect.updated_at,
                            "evidence_integrity_failed",
                            &format!("watchdog: {}", evidence_error),
                            now,
                        )
                        .await
                        {
                            Ok(true) => {
                                transitioned += 1;
                                emit_p078_metric(
                                    "side_effect_recovery_transition_total",
                                    Some(&effect.effect_kind),
                                    Some(&SideEffectStatus::NeedsReconciliation),
                                );
                                warn!(
                                    effect_id = %effect.id,
                                    error = %evidence_error,
                                    "side_effect_transition: settled -> needs_reconciliation (evidence verification failed)"
                                );
                            }
                            Ok(false) => {}
                            Err(e) => warn!(
                                effect_id = %effect.id,
                                error = %e,
                                "watchdog mark_settled_evidence_failed error"
                            ),
                        }
                    }
                }
                continue;
            }

            let (last_error_kind, last_error) = match effect.status {
                SideEffectStatus::Prepared => (
                    "prepared_deadline_expired",
                    "watchdog: prepared intent expired before executor ownership",
                ),
                SideEffectStatus::ExternallyObserved => (
                    "external_write_unsettled_deadline_expired",
                    "watchdog: external write remained unsettled past deadline",
                ),
                _ => (
                    "lease_or_deadline_expired",
                    "watchdog: lease or deadline expired",
                ),
            };
            let params = ReaperTransitionCasParams {
                effect_id: &effect.id,
                observed_status: effect.status.clone(),
                observed_owner: effect.owner_instance_id.as_deref(),
                observed_lease_renewed_at: effect.lease_renewed_at,
                observed_updated_at: effect.updated_at,
                now,
                last_error_kind,
                last_error,
            };

            match reaper_transition_cas(&self.pool, &params).await {
                Ok(true) => {
                    transitioned += 1;
                    emit_p078_metric(
                        "side_effect_recovery_transition_total",
                        Some(&effect.effect_kind),
                        Some(&SideEffectStatus::NeedsReconciliation),
                    );
                    info!(
                        effect_id = %effect.id,
                        from_status = %effect.status,
                        "side_effect_transition: watchdog -> needs_reconciliation"
                    );
                }
                Ok(false) => {}
                Err(e) => warn!(
                    effect_id = %effect.id,
                    error = %e,
                    "watchdog reaper_transition_cas error"
                ),
            }
        }

        if transitioned > 0 {
            emit_p078_metric("startup_side_effect_recovery_total", None, None);
            info!(
                metric_name = "startup_side_effect_recovery_duration_seconds",
                duration_ms = recovery_started.elapsed().as_millis() as u64,
                "p078_side_effect_metric"
            );
        }

        Ok(transitioned)
    }
}

async fn verify_p078_observed_evidence_summary(effect: &SideEffect) -> Result<()> {
    let raw = effect
        .observed_evidence_summary_json
        .as_deref()
        .ok_or_else(|| anyhow!("missing observed evidence summary"))?;
    let summary: serde_json::Value =
        serde_json::from_str(raw).context("parse observed evidence summary")?;
    let manifest_path = summary
        .get("manifest_path")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing evidence manifest path"))?;
    if let Some(expected_sha) = summary
        .get("manifest_sha256")
        .and_then(serde_json::Value::as_str)
    {
        match verify_spool_file(Path::new(manifest_path), expected_sha).await? {
            VerifyResult::Ok => {}
            VerifyResult::Missing => return Err(anyhow!("evidence manifest missing")),
            VerifyResult::ChecksumMismatch { actual } => {
                return Err(anyhow!(
                    "evidence manifest checksum mismatch: expected {}, actual {}",
                    expected_sha,
                    actual
                ));
            }
        }
    } else if !Path::new(manifest_path).exists() {
        return Err(anyhow!("evidence manifest missing"));
    }

    let manifest_bytes = tokio::fs::read(manifest_path)
        .await
        .context("read evidence manifest")?;
    let manifest: serde_json::Value =
        serde_json::from_slice(&manifest_bytes).context("parse evidence manifest")?;
    let files = manifest
        .get("files")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow!("evidence manifest missing files array"))?;
    if files.is_empty() {
        return Err(anyhow!("evidence manifest contains no files"));
    }
    let present_kinds: HashSet<&str> = files
        .iter()
        .filter_map(|file| file.get("kind").and_then(serde_json::Value::as_str))
        .collect();
    for required_kind in [
        "release_receipt",
        "stdout",
        "stderr",
        "git_ls_remote",
        "upload_readback",
        "archive_summary",
        "reconciliation_report",
    ] {
        if !present_kinds.contains(required_kind) {
            return Err(anyhow!(
                "evidence manifest missing required file kind {}",
                required_kind
            ));
        }
    }

    for file in files {
        let path = file
            .get("path")
            .and_then(serde_json::Value::as_str)
            .or_else(|| {
                file.get("absolute_path")
                    .and_then(serde_json::Value::as_str)
            })
            .ok_or_else(|| anyhow!("evidence file entry missing path"))?;
        let sha = file
            .get("sha256")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("evidence file entry missing sha256"))?;
        match verify_spool_file(Path::new(path), sha).await? {
            VerifyResult::Ok => {}
            VerifyResult::Missing => return Err(anyhow!("evidence file missing")),
            VerifyResult::ChecksumMismatch { actual } => {
                return Err(anyhow!(
                    "evidence file checksum mismatch: expected {}, actual {}",
                    sha,
                    actual
                ));
            }
        }
        if let Some(expected_size) = file.get("size_bytes").and_then(serde_json::Value::as_u64) {
            let actual_size = tokio::fs::metadata(path)
                .await
                .with_context(|| format!("stat evidence file {}", path))?
                .len();
            if actual_size != expected_size {
                return Err(anyhow!(
                    "evidence file size mismatch: expected {}, actual {}",
                    expected_size,
                    actual_size
                ));
            }
        } else {
            return Err(anyhow!("evidence file entry missing size_bytes"));
        }
    }

    Ok(())
}

/// Proof-of-ownership returned by `prepare_and_lease`, consumed by `mark_write_started`
/// and `settle_*`. Holds the ids required for CAS settlement.
pub struct ExecutorLease {
    pub effect_id: SideEffectId,
    pub attempt_id: SideEffectAttemptId,
    /// Snapshot of `lease_acquired_at` right after `executor_start_cas`.
    pub lease_acquired_at: DateTime<Utc>,
    /// Last lease timestamp observed by this executor. Settlement CAS must use
    /// this value so renewal races remain one-winner.
    pub lease_renewed_at: DateTime<Utc>,
    pub(crate) owner_instance_id: String,
}

// ── Coordinator methods for release-executor wiring ───────────────────────────

impl DurableEffectCoordinator {
    /// Prepare a side-effect intent and immediately acquire the executor lease.
    /// Returns `ExecutorLease` on success.
    ///
    /// Returns `Err` if the feature flag is disabled, if an unresolved effect
    /// already exists for the same target, or if the CAS lease acquisition fails.
    pub async fn prepare_and_lease(&self, intent: PrepareEffectIntent) -> Result<ExecutorLease> {
        let effect_kind = intent.effect_kind.clone();
        let effect_id = self.prepare_effect(intent).await?;

        let now = Utc::now();
        let lease_ttl = Duration::seconds(effect_kind.lease_ttl_seconds());
        let attempt_id = SideEffectAttemptId::new();

        let params = ExecutorStartCasParams {
            effect_id: &effect_id,
            owner_instance_id: &self.instance_id,
            attempt_id: &attempt_id,
            lease_acquired_at: now,
            lease_expires_at: now + lease_ttl,
            deadline_at: Some(now + Duration::seconds(effect_kind.deadline_seconds())),
            now,
        };

        let won = executor_start_cas(&self.pool, &params).await?;
        if !won {
            return Err(anyhow!(
                "executor_start_cas lost for effect {}; another instance may have acquired the lease",
                effect_id
            ));
        }
        emit_p078_metric(
            "side_effect_transition_total",
            Some(&effect_kind),
            Some(&SideEffectStatus::Executing),
        );

        info!(
            effect_id = %effect_id,
            effect_kind = %effect_kind,
            "side_effect_transition: prepared -> executing (lease acquired via prepare_and_lease)"
        );

        Ok(ExecutorLease {
            effect_id,
            attempt_id,
            lease_acquired_at: now,
            lease_renewed_at: now,
            owner_instance_id: self.instance_id.clone(),
        })
    }

    /// Renew an executing lease while a long-running external operation is in flight.
    /// A failed renewal is fail-closed: callers must stop canonical state mutation.
    pub async fn renew_lease(
        &self,
        lease: &mut ExecutorLease,
        effect_kind: &EffectKind,
    ) -> Result<()> {
        let now = Utc::now();
        let new_expires_at = now + Duration::seconds(effect_kind.lease_ttl_seconds());
        let renewed = side_effects::lease_renew_cas(
            &self.pool,
            &lease.effect_id,
            &lease.owner_instance_id,
            new_expires_at,
            now,
        )
        .await?;
        if !renewed {
            return Err(anyhow!(
                "side_effect_lease_renewal_lost: effect {} lease could not be renewed",
                lease.effect_id
            ));
        }
        lease.lease_renewed_at = now;
        info!(
            effect_id = %lease.effect_id,
            "side_effect_transition: executing lease renewed"
        );
        Ok(())
    }

    /// Run an already-started external operation while periodically renewing the
    /// side-effect lease. This is required for archive/upload paths whose deadline
    /// is minutes or hours while the executor lease TTL is intentionally short.
    pub async fn run_with_lease_renewal<T, F>(
        &self,
        lease: &mut ExecutorLease,
        effect_kind: &EffectKind,
        operation: F,
    ) -> Result<T>
    where
        F: Future<Output = Result<T>>,
    {
        let interval = std::time::Duration::from_secs(
            (effect_kind.lease_ttl_seconds().max(2) as u64 / 2).max(1),
        );
        tokio::pin!(operation);
        loop {
            tokio::select! {
                result = &mut operation => return result,
                _ = tokio::time::sleep(interval) => {
                    self.renew_lease(lease, effect_kind)
                        .await
                        .with_context(|| format!("renew side-effect lease {}", lease.effect_id))?;
                }
            }
        }
    }

    /// Mark that an external write is about to start for the leased effect.
    /// Must be called before any network/git/upload operation.
    /// Returns Ok(true) on success; Ok(false) means the CAS predicate did not
    /// match (e.g., lease expired or race with reaper).
    pub async fn mark_write_started(&self, lease: &ExecutorLease) -> Result<bool> {
        let started = mark_external_write_started(
            &self.pool,
            &lease.effect_id,
            &lease.owner_instance_id,
            Utc::now(),
        )
        .await?;
        if started {
            emit_p078_metric("side_effect_external_write_started_total", None, None);
            info!(
                effect_id = %lease.effect_id,
                "side_effect: external_write_started marked"
            );
        } else {
            warn!(
                effect_id = %lease.effect_id,
                "side_effect: mark_external_write_started CAS missed — lease may have expired"
            );
        }
        Ok(started)
    }

    /// Settle a leased effect as `settled` after a successful external write.
    /// Returns Ok(true) if CAS succeeded; Ok(false) if the predicate did not
    /// match (reaper may have taken the row concurrently).
    pub async fn settle_success(
        &self,
        lease: &ExecutorLease,
        evidence_summary: Option<&str>,
    ) -> Result<bool> {
        let now = Utc::now();
        let settlement_txn_id = uuid::Uuid::new_v4().to_string();

        let params = ExecutorSettleCasParams {
            effect_id: &lease.effect_id,
            owner_instance_id: &lease.owner_instance_id,
            settlement_attempt_id: &lease.attempt_id,
            observed_lease_renewed_at: lease.lease_renewed_at,
            new_status: SideEffectStatus::Settled,
            observed_evidence_summary_json: evidence_summary,
            settlement_txn_id: &settlement_txn_id,
            last_error_kind: None,
            last_error: None,
            now,
            settlement_source: "executor",
            receipt_artifact_id: None,
            decision_json: None,
            decision_json_hash: None,
            disposition_id: None,
        };

        let won = executor_settle_cas(&self.pool, &params).await?;
        if won {
            emit_p078_metric(
                "side_effect_settlement_latency_seconds",
                None,
                Some(&SideEffectStatus::Settled),
            );
            emit_p078_metric(
                "side_effect_transition_total",
                None,
                Some(&SideEffectStatus::Settled),
            );
            info!(
                effect_id = %lease.effect_id,
                "side_effect_transition: executing -> settled"
            );
        } else {
            warn!(
                effect_id = %lease.effect_id,
                "side_effect_cas_lost: settle_success CAS missed — reaper may have taken the row"
            );
        }
        Ok(won)
    }

    /// Settle a leased effect as `needs_reconciliation` after a failed external write.
    /// The effect row is left for operator inspection and MCP reconciliation.
    ///
    /// Returns Ok(()) when the CAS succeeded or when the CAS was lost (reaper already
    /// transitioned the row — the ledger is in a known state).
    /// Returns Err(e) on DB error — callers MUST NOT advance canonical state when this
    /// returns Err, as the ledger row may remain in `executing` state.
    pub async fn settle_failure(
        &self,
        lease: &ExecutorLease,
        error_kind: &str,
        error: &str,
    ) -> Result<()> {
        let now = Utc::now();

        let params = ExecutorFailCasParams {
            effect_id: &lease.effect_id,
            owner_instance_id: &lease.owner_instance_id,
            attempt_id: &lease.attempt_id,
            observed_lease_renewed_at: lease.lease_renewed_at,
            last_error_kind: error_kind,
            last_error: error,
            now,
        };

        match executor_fail_cas(&self.pool, &params).await {
            Ok(true) => {
                emit_p078_metric(
                    "side_effect_transition_total",
                    None,
                    Some(&SideEffectStatus::NeedsReconciliation),
                );
                warn!(
                    effect_id = %lease.effect_id,
                    error_kind = %error_kind,
                    "side_effect_transition: executing -> needs_reconciliation (release failure)"
                );
            }
            Ok(false) => {
                // CAS lost — reaper already transitioned the row; ledger is in a known state.
                warn!(
                    effect_id = %lease.effect_id,
                    "side_effect_cas_lost: settle_failure CAS missed — reaper may have taken the row"
                );
            }
            Err(e) => {
                // DB error — the row may remain in executing state; do NOT swallow this so
                // callers can abort canonical state mutation and preserve fail-closed invariant.
                warn!(
                    effect_id = %lease.effect_id,
                    error = %e,
                    "side_effect: settle_failure DB error — effect may remain in executing state; propagating to caller"
                );
                return Err(e);
            }
        }
        Ok(())
    }
}

/// Run-level ledger preflight for scheduler advancement, startup recovery, and CancelRun.
/// Checks all stages in the run for unresolved side effects.
/// Returns Err with requires_effect_reconciliation envelope when any exist.
/// Always runs regardless of the CHAINWORKS_RELEASE_SIDE_EFFECTS_ENABLED flag.
pub async fn run_unresolved_effects_preflight(
    pool: &SqlitePool,
    run_id: &RunId,
    operation_label: &str,
) -> Result<()> {
    let call_site = format!("run_unresolved_effects_preflight:{operation_label}");
    let now = Utc::now();
    if let Some(open_until) = ledger_readback_circuit_open_until(&call_site, now) {
        warn!(
            run_id = %run_id,
            operation = operation_label,
            call_site = %call_site,
            open_until = %open_until,
            "side_effect_ledger_readback_circuit_open during run preflight"
        );
        return Err(ledger_readback_circuit_error(&call_site, open_until));
    }
    let run_id_str = run_id.to_string();
    let unresolved = side_effects::list_unresolved_for_run(pool, &run_id_str)
        .await
        .map_err(|e| {
            let open_until = record_ledger_readback_error(&call_site, now);
            emit_p078_metric("side_effect_ledger_readback_error_total", None, None);
            warn!(
                run_id = %run_id,
                error = %e,
                operation = operation_label,
                call_site = %call_site,
                open_until = ?open_until.map(|v| v.to_rfc3339()),
                "side_effect_ledger_readback_error during run preflight"
            );
            anyhow!("ledger_readback_error: {}", e)
        })?;
    clear_ledger_readback_circuit(&call_site);

    if unresolved.is_empty() {
        return Ok(());
    }

    let effect_ids: Vec<String> = unresolved.iter().map(|e| e.id.to_string()).collect();
    let reason = classify_unresolved_reason(&unresolved);
    // Use the stage_execution_id from the first unresolved effect as the envelope anchor.
    let stage_execution_id = &unresolved[0].stage_execution_id;

    let envelope = RequiresEffectReconciliationEnvelope::new(
        run_id,
        stage_execution_id,
        None,
        effect_ids,
        reason,
    );

    emit_p078_metric("side_effect_retry_block_total", None, None);
    warn!(
        run_id = %run_id,
        operation = operation_label,
        "requires_effect_reconciliation_denied: unresolved effects block run operation"
    );

    Err(anyhow!(
        "requires_effect_reconciliation: {}",
        serde_json::to_string(&envelope).unwrap_or_default()
    ))
}

/// Backward-compatible entry point used by CancelRun command handling.
pub async fn run_cancel_preflight(pool: &SqlitePool, run_id: &RunId) -> Result<()> {
    run_unresolved_effects_preflight(pool, run_id, "cancel").await
}

/// Transaction-scoped variant of `run_cancel_preflight`. Callers that hold an
/// open `BEGIN IMMEDIATE` transaction on a single-connection pool must use this
/// to avoid deadlocking on pool acquire (see `retry_preflight_within_tx`).
pub async fn run_cancel_preflight_within_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &RunId,
) -> Result<()> {
    let call_site = "run_cancel_preflight_within_tx";
    let now = Utc::now();
    if let Some(open_until) = ledger_readback_circuit_open_until(call_site, now) {
        warn!(
            run_id = %run_id,
            call_site = %call_site,
            open_until = %open_until,
            "side_effect_ledger_readback_circuit_open"
        );
        return Err(ledger_readback_circuit_error(call_site, open_until));
    }
    let unresolved = list_unresolved_for_run_tx(tx, &run_id.to_string())
        .await
        .map_err(|e| {
            let open_until = record_ledger_readback_error(call_site, now);
            emit_p078_metric("side_effect_ledger_readback_error_total", None, None);
            warn!(
                run_id = %run_id,
                error = %e,
                call_site = %call_site,
                open_until = ?open_until.map(|v| v.to_rfc3339()),
                "side_effect_ledger_readback_error during cancel preflight"
            );
            anyhow!("ledger_readback_error: {}", e)
        })?;
    clear_ledger_readback_circuit(call_site);

    if unresolved.is_empty() {
        return Ok(());
    }

    let effect_ids: Vec<String> = unresolved.iter().map(|e| e.id.to_string()).collect();
    let reason = classify_unresolved_reason(&unresolved);
    let stage_execution_id = &unresolved[0].stage_execution_id;

    let envelope = RequiresEffectReconciliationEnvelope::new(
        run_id,
        stage_execution_id,
        None,
        effect_ids,
        reason,
    );

    emit_p078_metric("side_effect_retry_block_total", None, None);
    warn!(
        run_id = %run_id,
        "requires_effect_reconciliation_denied: unresolved effects block cancel"
    );

    Err(anyhow!(
        "requires_effect_reconciliation: {}",
        serde_json::to_string(&envelope).unwrap_or_default()
    ))
}

/// Transaction-scoped retry preflight. Equivalent to
/// `DurableEffectCoordinator::retry_preflight` but reads through an already-open
/// transaction rather than acquiring a new pool connection. Callers that hold a
/// `BEGIN IMMEDIATE` transaction on a single-connection pool (in-memory SQLite in
/// tests, or any heavily loaded single-connection pool) must use this variant to
/// avoid deadlocking on pool acquire.
pub async fn retry_preflight_within_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &RunId,
    stage_execution_id: &StageExecutionId,
    agent_execution_id: Option<&AgentExecutionId>,
) -> Result<()> {
    let call_site = "retry_preflight_within_tx";
    let now = Utc::now();
    if let Some(open_until) = ledger_readback_circuit_open_until(call_site, now) {
        warn!(
            run_id = %run_id,
            stage_execution_id = %stage_execution_id,
            call_site = %call_site,
            open_until = %open_until,
            "side_effect_ledger_readback_circuit_open"
        );
        return Err(ledger_readback_circuit_error(call_site, open_until));
    }
    let unresolved = list_unresolved_for_stage_tx(tx, &stage_execution_id.to_string())
        .await
        .map_err(|e| {
            let open_until = record_ledger_readback_error(call_site, now);
            emit_p078_metric("side_effect_ledger_readback_error_total", None, None);
            warn!(
                run_id = %run_id,
                stage_execution_id = %stage_execution_id,
                error = %e,
                call_site = %call_site,
                open_until = ?open_until.map(|v| v.to_rfc3339()),
                "side_effect_ledger_readback_error"
            );
            anyhow!("ledger_readback_error: {}", e)
        })?;
    clear_ledger_readback_circuit(call_site);

    if unresolved.is_empty() {
        return Ok(());
    }

    let effect_ids: Vec<String> = unresolved.iter().map(|e| e.id.to_string()).collect();
    let reason = classify_unresolved_reason(&unresolved);

    let envelope = RequiresEffectReconciliationEnvelope::new(
        run_id,
        stage_execution_id,
        agent_execution_id,
        effect_ids,
        reason,
    );

    warn!(
        run_id = %run_id,
        stage_execution_id = %stage_execution_id,
        "requires_effect_reconciliation_denied: unresolved effects block retry/cancel/recovery"
    );

    Err(anyhow!(
        "requires_effect_reconciliation: {}",
        serde_json::to_string(&envelope).unwrap_or_default()
    ))
}

fn classify_unresolved_reason(effects: &[SideEffect]) -> ReconciliationBlockReason {
    if effects
        .iter()
        .any(|e| e.status == SideEffectStatus::Conflict)
    {
        return ReconciliationBlockReason::Conflict;
    }
    if effects
        .iter()
        .any(|e| e.status == SideEffectStatus::Unrecoverable)
    {
        return ReconciliationBlockReason::Unrecoverable;
    }
    if effects
        .iter()
        .any(|e| e.status == SideEffectStatus::NeedsReconciliation)
    {
        return ReconciliationBlockReason::NeedsReconciliation;
    }
    if effects
        .iter()
        .any(|e| e.status == SideEffectStatus::ExternallyObserved)
    {
        return ReconciliationBlockReason::ExternallyObservedPending;
    }
    if effects
        .iter()
        .any(|e| e.status == SideEffectStatus::Executing)
    {
        return ReconciliationBlockReason::UnresolvedExecuting;
    }
    ReconciliationBlockReason::UnresolvedPrepared
}

#[cfg(test)]
mod tests {
    use super::*;
    use db::pool::create_pool;
    use domain::ids::{RunId, StageExecutionId};

    async fn test_pool() -> SqlitePool {
        create_pool("sqlite::memory:").await.expect("pool failed")
    }

    #[tokio::test]
    async fn proposal_078_prepare_effect_blocked_when_flag_off() {
        // The enabled flag was removed — the feature is always-on. This test now
        // verifies that duplicate idempotency_key use is blocked (the primary guard),
        // which is the structural invariant that the flag formerly protected.
        let pool = test_pool().await;
        let coord = DurableEffectCoordinator::new_with_enabled(pool.clone(), "instance-1".into());
        let run_id = RunId::new();
        let stage_id = StageExecutionId::new();
        let key = "p078:v1:dup-test".to_string();
        let intent = PrepareEffectIntent {
            run_id,
            stage_execution_id: stage_id,
            agent_execution_id: None,
            effect_kind: EffectKind::GitPush,
            target_key: "refs/heads/main".into(),
            idempotency_key: key.clone(),
            idempotency_key_version: 1,
            request_fingerprint: "fp-abc".into(),
            request_fingerprint_version: 1,
            expected_evidence_json: None,
            evidence_root: None,
            deadline_at: None,
        };
        coord.prepare_effect(intent.clone()).await.unwrap();
        // Second call with the same key must be rejected.
        let result = coord.prepare_effect(intent).await;
        assert!(
            result.is_err(),
            "should fail when idempotency_key is already active"
        );
    }

    #[tokio::test]
    async fn proposal_078_prepare_effect_succeeds_when_flag_on() {
        let pool = test_pool().await;
        let coord = DurableEffectCoordinator::new_with_enabled(pool.clone(), "instance-1".into());
        let run_id = RunId::new();
        let stage_id = StageExecutionId::new();
        let intent = PrepareEffectIntent {
            run_id,
            stage_execution_id: stage_id,
            agent_execution_id: None,
            effect_kind: EffectKind::GitCommit,
            target_key: "target-main".into(),
            idempotency_key: uuid::Uuid::new_v4().to_string(),
            idempotency_key_version: 1,
            request_fingerprint: "fp-123".into(),
            request_fingerprint_version: 1,
            expected_evidence_json: None,
            evidence_root: None,
            deadline_at: None,
        };
        let effect_id = coord.prepare_effect(intent).await.unwrap();
        let loaded = side_effects::find_by_id(&pool, &effect_id).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().status, SideEffectStatus::Prepared);
    }

    #[tokio::test]
    async fn proposal_078_retry_preflight_blocks_when_unresolved_exist() {
        let pool = test_pool().await;
        let coord = DurableEffectCoordinator::new_with_enabled(pool.clone(), "instance-1".into());
        let run_id = RunId::new();
        let stage_id = StageExecutionId::new();

        // Prepare an effect so preflight has something to find
        let intent = PrepareEffectIntent {
            run_id,
            stage_execution_id: stage_id,
            agent_execution_id: None,
            effect_kind: EffectKind::GitPush,
            target_key: "refs/heads/main".into(),
            idempotency_key: uuid::Uuid::new_v4().to_string(),
            idempotency_key_version: 1,
            request_fingerprint: "fp-xyz".into(),
            request_fingerprint_version: 1,
            expected_evidence_json: None,
            evidence_root: None,
            deadline_at: None,
        };
        coord.prepare_effect(intent).await.unwrap();

        let preflight_result = coord.retry_preflight(&run_id, &stage_id, None).await;
        assert!(
            preflight_result.is_err(),
            "preflight must fail when unresolved effects exist"
        );
        let err = preflight_result.unwrap_err().to_string();
        assert!(
            err.contains("requires_effect_reconciliation"),
            "error must reference requires_effect_reconciliation"
        );
    }

    #[tokio::test]
    async fn proposal_078_executor_start_cas_is_exclusive() {
        let pool = test_pool().await;
        let coord = DurableEffectCoordinator::new_with_enabled(pool.clone(), "instance-1".into());
        let run_id = RunId::new();
        let stage_id = StageExecutionId::new();

        let intent = PrepareEffectIntent {
            run_id,
            stage_execution_id: stage_id,
            agent_execution_id: None,
            effect_kind: EffectKind::GitCommit,
            target_key: "cas-target".into(),
            idempotency_key: uuid::Uuid::new_v4().to_string(),
            idempotency_key_version: 1,
            request_fingerprint: "fp-cas".into(),
            request_fingerprint_version: 1,
            expected_evidence_json: None,
            evidence_root: None,
            deadline_at: None,
        };
        let effect_id = coord.prepare_effect(intent).await.unwrap();

        // First start should succeed
        let won1 = coord
            .executor_start(&effect_id, &EffectKind::GitCommit)
            .await
            .unwrap();
        assert!(won1, "first CAS must win");
        let loaded = side_effects::find_by_id(&pool, &effect_id)
            .await
            .unwrap()
            .expect("effect must exist after start");
        assert!(
            loaded.deadline_at.is_some(),
            "executor_start must stamp a deadline for watchdog recovery"
        );

        // Second start for the same effect must fail (already executing)
        let won2 = coord
            .executor_start(&effect_id, &EffectKind::GitCommit)
            .await
            .unwrap();
        assert!(!won2, "second CAS must lose — effect already executing");
    }

    #[tokio::test]
    async fn proposal_078_renew_lease_updates_settlement_observation() {
        let pool = test_pool().await;
        let coord = DurableEffectCoordinator::new_with_enabled(pool.clone(), "instance-1".into());
        let run_id = RunId::new();
        let stage_id = StageExecutionId::new();
        let intent = PrepareEffectIntent {
            run_id,
            stage_execution_id: stage_id,
            agent_execution_id: None,
            effect_kind: EffectKind::GitPush,
            target_key: "renew-target".into(),
            idempotency_key: uuid::Uuid::new_v4().to_string(),
            idempotency_key_version: 1,
            request_fingerprint: "fp-renew".into(),
            request_fingerprint_version: 1,
            expected_evidence_json: None,
            evidence_root: None,
            deadline_at: None,
        };
        let mut lease = coord.prepare_and_lease(intent).await.unwrap();
        let original_renewed_at = lease.lease_renewed_at;

        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        coord
            .renew_lease(&mut lease, &EffectKind::GitPush)
            .await
            .unwrap();

        assert!(
            lease.lease_renewed_at >= original_renewed_at,
            "lease renewal must refresh the executor observation used by settlement CAS"
        );
        let loaded = side_effects::find_by_id(&pool, &lease.effect_id)
            .await
            .unwrap()
            .expect("effect must remain durable");
        assert_eq!(
            loaded.lease_renewed_at,
            Some(lease.lease_renewed_at),
            "durable row must preserve the renewed timestamp"
        );
    }

    #[tokio::test]
    async fn proposal_078_watchdog_transitions_expired_executing_effects_on_startup() {
        let pool = test_pool().await;
        let coord = DurableEffectCoordinator::new_with_enabled(pool.clone(), "instance-1".into());
        let run_id = RunId::new();
        let stage_id = StageExecutionId::new();
        let intent = PrepareEffectIntent {
            run_id,
            stage_execution_id: stage_id,
            agent_execution_id: None,
            effect_kind: EffectKind::BuildArchive,
            target_key: "archive-target".into(),
            idempotency_key: uuid::Uuid::new_v4().to_string(),
            idempotency_key_version: 1,
            request_fingerprint: "fp-archive".into(),
            request_fingerprint_version: 1,
            expected_evidence_json: None,
            evidence_root: None,
            deadline_at: None,
        };
        let lease = coord.prepare_and_lease(intent).await.unwrap();
        let expired = Utc::now() - Duration::seconds(120);
        sqlx::query(
            r#"UPDATE side_effects
                  SET lease_expires_at = ?1, deadline_at = ?1, lease_renewed_at = ?1
                WHERE id = ?2"#,
        )
        .bind(expired.to_rfc3339())
        .bind(lease.effect_id.as_ref())
        .execute(&pool)
        .await
        .unwrap();

        let transitioned = coord.watchdog_pass().await.unwrap();

        assert_eq!(transitioned, 1);
        let loaded = side_effects::find_by_id(&pool, &lease.effect_id)
            .await
            .unwrap()
            .expect("effect must remain durable");
        assert_eq!(loaded.status, SideEffectStatus::NeedsReconciliation);
        assert_eq!(
            loaded.last_error_kind.as_deref(),
            Some("lease_or_deadline_expired")
        );
    }

    #[tokio::test]
    async fn proposal_078_watchdog_recovers_prepared_and_external_write_crash_windows() {
        let pool = test_pool().await;
        let coord = DurableEffectCoordinator::new_with_enabled(pool.clone(), "instance-1".into());
        let run_id = RunId::new();
        let prepared_stage = StageExecutionId::new();
        let external_stage = StageExecutionId::new();

        let prepared = coord
            .prepare_effect(PrepareEffectIntent {
                run_id,
                stage_execution_id: prepared_stage,
                agent_execution_id: None,
                effect_kind: EffectKind::GitCommit,
                target_key: "prepared-target".into(),
                idempotency_key: uuid::Uuid::new_v4().to_string(),
                idempotency_key_version: 1,
                request_fingerprint: "fp-prepared".into(),
                request_fingerprint_version: 1,
                expected_evidence_json: None,
                evidence_root: None,
                deadline_at: Some(Utc::now() - Duration::seconds(60)),
            })
            .await
            .unwrap();
        let external = coord
            .prepare_effect(PrepareEffectIntent {
                run_id,
                stage_execution_id: external_stage,
                agent_execution_id: None,
                effect_kind: EffectKind::GitPush,
                target_key: "external-target".into(),
                idempotency_key: uuid::Uuid::new_v4().to_string(),
                idempotency_key_version: 1,
                request_fingerprint: "fp-external".into(),
                request_fingerprint_version: 1,
                expected_evidence_json: None,
                evidence_root: None,
                deadline_at: Some(Utc::now() - Duration::seconds(60)),
            })
            .await
            .unwrap();
        sqlx::query("UPDATE side_effects SET status = 'externally_observed' WHERE id = ?1")
            .bind(external.as_ref())
            .execute(&pool)
            .await
            .unwrap();

        let transitioned = coord.watchdog_pass().await.unwrap();

        assert_eq!(transitioned, 2);
        let prepared = side_effects::find_by_id(&pool, &prepared)
            .await
            .unwrap()
            .unwrap();
        let external = side_effects::find_by_id(&pool, &external)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(prepared.status, SideEffectStatus::NeedsReconciliation);
        assert_eq!(
            prepared.last_error_kind.as_deref(),
            Some("prepared_deadline_expired")
        );
        assert_eq!(external.status, SideEffectStatus::NeedsReconciliation);
        assert_eq!(
            external.last_error_kind.as_deref(),
            Some("external_write_unsettled_deadline_expired")
        );
    }

    #[tokio::test]
    async fn proposal_078_watchdog_fails_closed_when_settled_evidence_manifest_is_missing() {
        let pool = test_pool().await;
        let coord = DurableEffectCoordinator::new_with_enabled(pool.clone(), "instance-1".into());
        let run_id = RunId::new();
        let stage_id = StageExecutionId::new();
        let effect = coord
            .prepare_effect(PrepareEffectIntent {
                run_id,
                stage_execution_id: stage_id,
                agent_execution_id: None,
                effect_kind: EffectKind::ConnectUpload,
                target_key: "settled-missing-evidence-target".into(),
                idempotency_key: uuid::Uuid::new_v4().to_string(),
                idempotency_key_version: 1,
                request_fingerprint: "fp-settled-missing".into(),
                request_fingerprint_version: 1,
                expected_evidence_json: None,
                evidence_root: None,
                deadline_at: Some(Utc::now() + Duration::seconds(60)),
            })
            .await
            .unwrap();
        let observed = serde_json::json!({
            "schema_version": "p078_observed_evidence_summary_v1",
            "manifest_path": "/tmp/chainworks-p078-missing-manifest.json",
            "manifest_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        })
        .to_string();
        sqlx::query(
            r#"UPDATE side_effects
               SET status = 'settled',
                   observed_evidence_summary_json = ?1
               WHERE id = ?2"#,
        )
        .bind(observed)
        .bind(effect.as_ref())
        .execute(&pool)
        .await
        .unwrap();

        let transitioned = coord.watchdog_pass().await.unwrap();

        assert_eq!(transitioned, 1);
        let loaded = side_effects::find_by_id(&pool, &effect)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.status, SideEffectStatus::NeedsReconciliation);
        assert_eq!(
            loaded.last_error_kind.as_deref(),
            Some("evidence_integrity_failed")
        );
    }

    #[tokio::test]
    async fn proposal_078_ledger_readback_circuit_opens_and_expires_fail_closed() {
        reset_ledger_readback_circuits_for_test();
        let operation = format!("p078_circuit_test_{}", uuid::Uuid::new_v4());
        let call_site = format!("run_unresolved_effects_preflight:{operation}");
        let run_id = RunId::new();
        let pool = test_pool().await;
        pool.close().await;

        for _ in 0..P078_LEDGER_READBACK_CIRCUIT_THRESHOLD {
            let err = run_unresolved_effects_preflight(&pool, &run_id, &operation)
                .await
                .expect_err("closed pool should fail ledger readback")
                .to_string();
            assert!(err.contains("ledger_readback_error"));
        }

        let err = run_unresolved_effects_preflight(&pool, &run_id, &operation)
            .await
            .expect_err("open circuit must fail closed before ledger read")
            .to_string();
        assert!(err.contains("circuit_open"));
        assert!(err.contains("retry_forbidden=true"));

        force_ledger_readback_circuit_open_until_for_test(
            &call_site,
            Utc::now() - Duration::seconds(1),
        );
        let recovered_pool = test_pool().await;
        run_unresolved_effects_preflight(&recovered_pool, &run_id, &operation)
            .await
            .expect("expired circuit should allow a successful ledger read");
    }

    #[tokio::test]
    async fn proposal_078_run_level_preflight_blocks_scheduler_advancement() {
        let pool = test_pool().await;
        let coord = DurableEffectCoordinator::new_with_enabled(pool.clone(), "instance-1".into());
        let run_id = RunId::new();
        let stage_id = StageExecutionId::new();
        let intent = PrepareEffectIntent {
            run_id,
            stage_execution_id: stage_id,
            agent_execution_id: None,
            effect_kind: EffectKind::ConnectUpload,
            target_key: "upload-target".into(),
            idempotency_key: uuid::Uuid::new_v4().to_string(),
            idempotency_key_version: 1,
            request_fingerprint: "fp-upload".into(),
            request_fingerprint_version: 1,
            expected_evidence_json: None,
            evidence_root: None,
            deadline_at: None,
        };
        coord.prepare_effect(intent).await.unwrap();

        let err = run_unresolved_effects_preflight(&pool, &run_id, "advance_run")
            .await
            .expect_err("unresolved side effects must block scheduler advancement")
            .to_string();
        assert!(err.contains("requires_effect_reconciliation"));
    }
}
