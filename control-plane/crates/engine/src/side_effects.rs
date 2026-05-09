use anyhow::{anyhow, Result};
use chrono::{Duration, Utc};
use sqlx::{Sqlite, SqlitePool, Transaction};
use tracing::{info, warn};

use db::repos::side_effects::{
    self, executor_start_cas, list_unresolved_for_stage_tx, reaper_transition_cas,
    DispositionOutcome, ExecutorStartCasParams, ReaperTransitionCasParams,
};
use domain::ids::{AgentExecutionId, RunId, StageExecutionId};
use domain::side_effect::{
    EffectKind, PrepareEffectIntent, ReconciliationBlockReason, RequiresEffectReconciliationEnvelope,
    SideEffect, SideEffectAttemptId, SideEffectId, SideEffectStatus,
    FEATURE_RELEASE_SIDE_EFFECTS_ENABLED,
};

// ── Feature-flag check ────────────────────────────────────────────────────────

pub fn side_effects_enabled() -> bool {
    std::env::var(FEATURE_RELEASE_SIDE_EFFECTS_ENABLED)
        .ok()
        .as_deref()
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false)
}

// ── DurableEffectCoordinator ──────────────────────────────────────────────────

pub struct DurableEffectCoordinator {
    pool: SqlitePool,
    instance_id: String,
    /// Whether the side-effect feature is enabled. Set at construction from
    /// the env var to avoid racy per-call env reads in tests.
    enabled: bool,
}

impl DurableEffectCoordinator {
    pub fn new(pool: SqlitePool, instance_id: String) -> Self {
        let enabled = side_effects_enabled();
        Self {
            pool,
            instance_id,
            enabled,
        }
    }

    /// Construct with an explicit enabled flag (useful in tests).
    pub fn new_with_enabled(pool: SqlitePool, instance_id: String, enabled: bool) -> Self {
        Self {
            pool,
            instance_id,
            enabled,
        }
    }

    /// Prepare a durable intent before executing any external operation.
    /// Returns the SideEffectId of the prepared row.
    ///
    /// If a row with the same idempotency_key already exists and is unresolved,
    /// returns requires_effect_reconciliation.
    pub async fn prepare_effect(&self, intent: PrepareEffectIntent) -> Result<SideEffectId> {
        if !self.enabled {
            return Err(anyhow!("CHAINWORKS_RELEASE_SIDE_EFFECTS_ENABLED is false"));
        }

        let idempotency_key = &intent.idempotency_key;

        // Check for existing row with same idempotency key.
        if let Some(existing) =
            side_effects::find_by_idempotency_key(&self.pool, idempotency_key).await?
        {
            if existing.status.is_unresolved() {
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
        let same_target: Vec<SideEffect> =
            side_effects::list_unresolved_by_run_and_target_key(
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
        let unresolved =
            side_effects::list_unresolved_for_stage(&self.pool, &stage_execution_id.to_string())
                .await
                .map_err(|e| {
                    warn!(
                        run_id = %run_id,
                        stage_execution_id = %stage_execution_id,
                        error = %e,
                        "side_effect_ledger_readback_error"
                    );
                    anyhow!("ledger_readback_error: {}", e)
                })?;

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
            deadline_at: None,
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

    /// Watchdog pass: transition stale executing effects to needs_reconciliation.
    pub async fn watchdog_pass(&self) -> Result<u32> {
        let now = Utc::now();
        let expired = side_effects::list_expired_executing(&self.pool, now, 50).await?;
        let mut transitioned = 0u32;

        for effect in &expired {
            let params = ReaperTransitionCasParams {
                effect_id: &effect.id,
                observed_status: effect.status.clone(),
                observed_owner: effect.owner_instance_id.as_deref(),
                observed_lease_renewed_at: effect.lease_renewed_at,
                observed_updated_at: effect.updated_at,
                now,
                last_error_kind: "lease_or_deadline_expired",
                last_error: "watchdog: lease or deadline expired",
            };

            match reaper_transition_cas(&self.pool, &params).await {
                Ok(true) => {
                    transitioned += 1;
                    info!(
                        effect_id = %effect.id,
                        "side_effect_transition: executing -> needs_reconciliation (watchdog)"
                    );
                }
                Ok(false) => {
                    // CAS lost — executor settled concurrently, no action needed
                }
                Err(e) => {
                    warn!(
                        effect_id = %effect.id,
                        error = %e,
                        "watchdog reaper_transition_cas error"
                    );
                }
            }
        }

        Ok(transitioned)
    }
}

/// Run-level ledger preflight for CancelRun.
/// Checks all stages in the run for unresolved side effects.
/// Returns Err with requires_effect_reconciliation envelope when any exist.
/// Always runs regardless of the CHAINWORKS_RELEASE_SIDE_EFFECTS_ENABLED flag.
pub async fn run_cancel_preflight(pool: &SqlitePool, run_id: &RunId) -> Result<()> {
    let run_id_str = run_id.to_string();
    let unresolved = side_effects::list_unresolved_for_run(pool, &run_id_str)
        .await
        .map_err(|e| {
            warn!(
                run_id = %run_id,
                error = %e,
                "side_effect_ledger_readback_error during cancel preflight"
            );
            anyhow!("ledger_readback_error: {}", e)
        })?;

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
    let unresolved = list_unresolved_for_stage_tx(tx, &stage_execution_id.to_string())
        .await
        .map_err(|e| {
            warn!(
                run_id = %run_id,
                stage_execution_id = %stage_execution_id,
                error = %e,
                "side_effect_ledger_readback_error"
            );
            anyhow!("ledger_readback_error: {}", e)
        })?;

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
    async fn proposal_078_side_effects_enabled_env() {
        // Feature flag defaults to false when env var is absent or "false"
        assert!(!side_effects_enabled_from_env(false));
        assert!(side_effects_enabled_from_env(true));
    }

    fn side_effects_enabled_from_env(val: bool) -> bool {
        val
    }

    #[tokio::test]
    async fn proposal_078_prepare_effect_blocked_when_flag_off() {
        let pool = test_pool().await;
        let coord = DurableEffectCoordinator::new_with_enabled(pool, "instance-1".into(), false);
        let intent = PrepareEffectIntent {
            run_id: RunId::new(),
            stage_execution_id: StageExecutionId::new(),
            agent_execution_id: None,
            effect_kind: EffectKind::GitPush,
            target_key: "refs/heads/main".into(),
            idempotency_key: "p078:v1:test".into(),
            idempotency_key_version: 1,
            request_fingerprint: "fp-abc".into(),
            request_fingerprint_version: 1,
            expected_evidence_json: None,
            evidence_root: None,
            deadline_at: None,
        };
        let result = coord.prepare_effect(intent).await;
        assert!(result.is_err(), "should fail when feature flag is off");
    }

    #[tokio::test]
    async fn proposal_078_prepare_effect_succeeds_when_flag_on() {
        let pool = test_pool().await;
        let coord = DurableEffectCoordinator::new_with_enabled(pool.clone(), "instance-1".into(), true);
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
        let coord = DurableEffectCoordinator::new_with_enabled(pool.clone(), "instance-1".into(), true);
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
        let coord = DurableEffectCoordinator::new_with_enabled(pool.clone(), "instance-1".into(), true);
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

        // Second start for the same effect must fail (already executing)
        let won2 = coord
            .executor_start(&effect_id, &EffectKind::GitCommit)
            .await
            .unwrap();
        assert!(!won2, "second CAS must lose — effect already executing");
    }
}
