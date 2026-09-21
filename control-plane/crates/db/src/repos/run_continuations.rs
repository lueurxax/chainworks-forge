//! Durable P039 authority. Filesystem witnesses and boundary authorization belong
//! to the engine service; this repository never calls providers or touches files.

use anyhow::{ensure, Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Sqlite, SqlitePool, Transaction};

#[derive(Clone, Debug)]
pub struct Reservation {
    pub source_run_id: String,
    pub caller_fingerprint: String,
    pub caller_request_id: String,
    pub intent_sha256: String,
    pub plan_ref: String,
    pub plan_sha256: String,
    pub target_ref: String,
    pub source_witness_sha256: String,
    pub reason: String,
}

#[derive(Clone, Debug, FromRow, Serialize, Deserialize)]
pub struct Continuation {
    pub operation_id: String,
    pub source_run_id: String,
    pub idea_id: String,
    pub reserved_successor_run_id: String,
    pub successor_run_id: Option<String>,
    pub caller_fingerprint: String,
    pub caller_request_id: String,
    pub intent_sha256: String,
    pub phase: String,
    pub version: i64,
    pub generation: i64,
    pub plan_ref: String,
    pub plan_sha256: String,
    pub target_ref: String,
    pub source_witness_sha256: String,
    pub manifest_ref: Option<String>,
    pub manifest_sha256: Option<String>,
    pub journal_id: String,
    pub holds_json: String,
    pub worker_claimed: bool,
    pub created_at: String,
    pub updated_at: String,
    pub deadline_at: String,
}

pub async fn find(pool: &SqlitePool, operation_id: &str) -> Result<Option<Continuation>> {
    Ok(
        sqlx::query_as("SELECT * FROM run_continuations WHERE operation_id=?")
            .bind(operation_id)
            .fetch_optional(pool)
            .await?,
    )
}

#[derive(Debug, Serialize)]
pub struct ContinuationLinks {
    pub incoming: Option<Continuation>,
    pub outgoing: Option<Continuation>,
}

/// Direction, not timestamp, is authority. An aborted outgoing operation remains
/// addressable by ID but does not hide this run's activated incoming lineage.
pub async fn links(pool: &SqlitePool, run_id: &str) -> Result<ContinuationLinks> {
    let mut tx = pool.begin().await?;
    let incoming = sqlx::query_as(
        "SELECT * FROM run_continuations WHERE successor_run_id=? AND phase='activated'",
    )
    .bind(run_id)
    .fetch_optional(&mut *tx)
    .await?;
    let outgoing = sqlx::query_as(
        "SELECT * FROM run_continuations WHERE source_run_id=? AND phase<>'aborted'",
    )
    .bind(run_id)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(ContinuationLinks { incoming, outgoing })
}

pub async fn check_source_guard(pool: &SqlitePool, run_id: &str) -> Result<()> {
    let disposition: Option<String> =
        sqlx::query_scalar("SELECT disposition FROM run_execution_fences WHERE source_run_id=?")
            .bind(run_id)
            .fetch_optional(pool)
            .await?;
    check_disposition(disposition.as_deref())
}

fn check_disposition(disposition: Option<&str>) -> Result<()> {
    if disposition.is_some() {
        crate::metrics::record_continuation_guard_denied();
    }
    match disposition {
        Some("historical") => anyhow::bail!("source_continued"),
        Some(_) => anyhow::bail!("continuation_in_progress"),
        None => Ok(()),
    }
}

/// Rechecked under the same writer lock as reservation/activation. Session rows
/// alone do not prove a live process; canonical process fate must be settled.
pub async fn ensure_quiescent_tx(tx: &mut Transaction<'_, Sqlite>, source: &str) -> Result<()> {
    // Resolve indirect owners and effective resources under the reservation's
    // writer lock. Unknown lifecycle/process ownership is not proof of absence.
    let busy: bool = sqlx::query_scalar(r#"WITH
        resources(resource_key) AS (
            SELECT resource_key FROM p039_run_resources WHERE run_id=?1 AND resource_key<>''
        ),
        owners(run_id) AS (
            SELECT run_id FROM p039_run_resource_peers WHERE source_run_id=?1
        ),
        stages(id) AS (
            SELECT id FROM stage_executions WHERE run_id IN (SELECT run_id FROM owners)
        ),
        mediations(id) AS (
            SELECT id FROM lead_conflict_mediations WHERE run_id IN (SELECT run_id FROM owners)
              OR conflict_id IN (SELECT conflict_id FROM workflow_conflicts
                  WHERE run_id IN (SELECT run_id FROM owners) OR stage_execution_id IN (SELECT id FROM stages))
        ),
        generations(id) AS (
            SELECT id FROM session_generations WHERE lineage_id IN
                (SELECT id FROM session_lineages WHERE run_id IN (SELECT run_id FROM owners))
              OR working_directory IN (SELECT resource_key FROM resources)
        ),
        agent_owners(agent_execution_id,run_id) AS (
            SELECT agent_execution_id,run_id FROM p039_agent_run_owners
            UNION SELECT id,?1 FROM agent_executions WHERE session_generation_id IN (SELECT id FROM generations)
              OR lead_mediation_record_id IN (SELECT id FROM mediations)
              OR (owner_kind='lead_conflict_mediation' AND owner_id IN (SELECT id FROM mediations))
        ),
        continuations(id) AS (
            SELECT id FROM agent_work_continuations WHERE run_id IN (SELECT run_id FROM owners)
              OR stage_execution_id IN (SELECT id FROM stage_executions WHERE run_id IN (SELECT run_id FROM owners))
              OR agent_execution_id IN (SELECT agent_execution_id FROM agent_owners WHERE run_id IN (SELECT run_id FROM owners))
              OR session_generation_id IN (SELECT id FROM generations)
              OR provider_session_id IN (SELECT id FROM providers)
        ),
        providers(id) AS (
            SELECT provider_session_id FROM provider_sessions WHERE run_id IN (SELECT run_id FROM owners)
              OR agent_execution_id IN (SELECT agent_execution_id FROM agent_owners WHERE run_id IN (SELECT run_id FROM owners))
        ),
        effects(id) AS (
            SELECT id FROM side_effects WHERE run_id IN (SELECT run_id FROM owners)
              OR stage_execution_id IN (SELECT id FROM stages)
              OR agent_execution_id IN (SELECT agent_execution_id FROM agent_owners WHERE run_id IN (SELECT run_id FROM owners))
        ),
        repairs(id) AS (
            SELECT repair_attempt_id FROM output_contract_repair_events WHERE run_id IN (SELECT run_id FROM owners)
              OR stage_execution_id IN (SELECT id FROM stages)
              OR agent_execution_id IN (SELECT agent_execution_id FROM agent_owners WHERE run_id IN (SELECT run_id FROM owners))
              OR session_generation_id IN (SELECT id FROM generations)
        ),
        helpers(id) AS (
            SELECT id FROM p080_helper_leases_v1 WHERE run_id IN (SELECT run_id FROM owners)
              OR agent_execution_id IN (SELECT agent_execution_id FROM agent_owners WHERE run_id IN (SELECT run_id FROM owners))
              OR session_generation_id IN (SELECT id FROM generations)
        ),
        barriers(id) AS (
            SELECT id FROM worktree_mutation_barriers WHERE run_id IN (SELECT run_id FROM owners)
              OR worktree_resource_key IN (SELECT resource_key FROM resources)
        ),
        queued_owners AS (
            SELECT run_id,worktree_resource_key,status,kind,json_valid(payload_json) AS valid_payload,
                CASE WHEN json_valid(payload_json) THEN json_extract(payload_json,'$.run_id') END AS payload_run,
                CASE WHEN json_valid(payload_json) THEN json_extract(payload_json,'$.stage_execution_id') END AS payload_stage,
                CASE WHEN json_valid(payload_json) THEN json_extract(payload_json,'$.agent_execution_id') END AS payload_agent,
                CASE WHEN json_valid(payload_json) THEN json_extract(payload_json,'$.mediation_record_id') END AS payload_mediation,
                CASE WHEN json_valid(payload_json) THEN json_extract(payload_json,'$.lead_mediation_record_id') END AS payload_lead_mediation,
                CASE WHEN json_valid(payload_json) THEN json_extract(payload_json,'$.continuation_id') END AS payload_continuation
            FROM work_items WHERE status NOT IN ('completed','failed','cancelled')
        )
        SELECT
        EXISTS(SELECT 1 FROM queued_owners q WHERE
            q.run_id IN (SELECT run_id FROM owners) OR q.worktree_resource_key IN (SELECT resource_key FROM resources) OR
            q.payload_run IN (SELECT run_id FROM owners) OR
            q.payload_stage IN (SELECT id FROM stage_executions WHERE run_id IN (SELECT run_id FROM owners)) OR
            q.payload_agent IN (SELECT agent_execution_id FROM agent_owners WHERE run_id IN (SELECT run_id FROM owners)) OR
            q.payload_mediation IN (SELECT id FROM mediations) OR
            q.payload_lead_mediation IN (SELECT id FROM mediations) OR
            q.payload_continuation IN (SELECT id FROM continuations) OR
            (q.run_id IS NULL AND NOT q.valid_payload) OR
            (q.run_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM runs WHERE id=q.run_id)) OR
            (q.payload_run IS NOT NULL AND NOT EXISTS(SELECT 1 FROM runs WHERE id=q.payload_run)) OR
            (q.payload_stage IS NOT NULL AND NOT EXISTS(SELECT 1 FROM stage_executions WHERE id=q.payload_stage)) OR
            (q.payload_agent IS NOT NULL AND NOT EXISTS(SELECT 1 FROM agent_owners WHERE agent_execution_id=q.payload_agent)) OR
            (q.payload_mediation IS NOT NULL AND NOT EXISTS(SELECT 1 FROM lead_conflict_mediations WHERE id=q.payload_mediation)) OR
            (q.payload_lead_mediation IS NOT NULL AND NOT EXISTS(SELECT 1 FROM lead_conflict_mediations WHERE id=q.payload_lead_mediation)) OR
            (q.payload_continuation IS NOT NULL AND NOT EXISTS(SELECT 1 FROM agent_work_continuations WHERE id=q.payload_continuation)) OR
            (q.status='running' AND q.kind IN ('invoke_agent','settle_stage','advance_run','trigger_next_stage','process_continuation')
                AND q.run_id IS NULL AND q.payload_run IS NULL AND q.payload_stage IS NULL AND q.payload_agent IS NULL
                AND q.payload_mediation IS NULL AND q.payload_lead_mediation IS NULL AND q.payload_continuation IS NULL)) OR
        EXISTS(SELECT 1 FROM stage_executions WHERE run_id IN (SELECT run_id FROM owners)
            AND status NOT IN ('completed','failed','cancelled','blocked','skipped')) OR
        EXISTS(SELECT 1 FROM agent_executions a WHERE a.status NOT IN ('completed','failed','cancelled') AND (
            a.id IN (SELECT agent_execution_id FROM agent_owners WHERE run_id IN (SELECT run_id FROM owners)) OR
            NOT EXISTS(SELECT 1 FROM agent_owners o WHERE o.agent_execution_id=a.id))) OR
        EXISTS(SELECT 1 FROM lead_conflict_mediations WHERE id IN (SELECT id FROM mediations)
            AND status NOT IN ('settled','terminal_unverifiable','canceled','superseded')) OR
        EXISTS(SELECT 1 FROM lead_mediation_confirmations WHERE (run_id IN (SELECT run_id FROM owners)
            OR mediation_record_id IN (SELECT id FROM mediations))
            AND status NOT IN ('resolved','superseded','expired','canceled')) OR
        EXISTS(SELECT 1 FROM approvals WHERE run_id IN (SELECT run_id FROM owners)
            AND decision NOT IN ('granted','rejected','expired')) OR
        EXISTS(SELECT 1 FROM escalation_ledger WHERE (run_id IN (SELECT run_id FROM owners)
            OR stage_execution_id IN (SELECT id FROM stages))
            AND status_raw NOT IN ('paused','exhausted','cancelled')) OR
        EXISTS(SELECT 1 FROM agent_work_continuations WHERE id IN (SELECT id FROM continuations) AND (
            status NOT IN ('succeeded','no_progress','failed','cancelled') OR reconciliation_status IS NOT NULL)) OR
        EXISTS(SELECT 1 FROM agent_external_side_effect_ledger WHERE continuation_id IN (SELECT id FROM continuations)
            AND (status IN ('planned','started') OR (side_effect_kind IN ('runtime_lease','worktree_lease') AND status='committed'))) OR
        EXISTS(SELECT 1 FROM supervised_workers_continuation WHERE continuation_id IN (SELECT id FROM continuations)
            AND (released_at IS NULL OR release_reason IS NULL OR release_reason NOT IN ('completed','cancelled','stale_generation_reaped'))) OR
        EXISTS(SELECT 1 FROM output_contract_repair_events WHERE repair_attempt_id IN (SELECT id FROM repairs)
            AND status='in_progress') OR
        EXISTS(SELECT 1 FROM output_contract_repair_leases WHERE lease_state<>'settled' AND (
            run_id IN (SELECT run_id FROM owners) OR stage_execution_id IN (SELECT id FROM stages)
            OR parent_agent_execution_id IN (SELECT agent_execution_id FROM agent_owners WHERE run_id IN (SELECT run_id FROM owners))
            OR repair_event_id IN (SELECT id FROM repairs))) OR
        EXISTS(SELECT 1 FROM main_sync_attempts WHERE (run_id IN (SELECT run_id FROM owners) OR barrier_id IN (SELECT id FROM barriers))
            AND status NOT IN ('succeeded','no_op','conflicted','failed')) OR
        EXISTS(SELECT 1 FROM worktree_mutation_barriers WHERE id IN (SELECT id FROM barriers)
            AND status NOT IN ('released','stolen','failed')) OR
        EXISTS(SELECT 1 FROM background_leases WHERE released_at IS NULL AND (
            run_id IN (SELECT run_id FROM owners) OR resource_key IN (SELECT resource_key FROM resources)
            OR worktree_resource_key IN (SELECT resource_key FROM resources)
            OR (run_id IS NULL AND worktree_resource_key IS NULL))) OR
        EXISTS(SELECT 1 FROM provider_sessions WHERE provider_session_id IN (SELECT id FROM providers) AND (
            process_fate<>'absent_verified' OR lifecycle_state NOT IN
            ('spawn_error_no_child','self_exit_observed','terminated_graceful','terminated_by_kill','orphan_settled'))) OR
        EXISTS(SELECT 1 FROM provider_cancellation_intents i WHERE i.intent_state<>'settled' AND (
            i.provider_session_id IN (SELECT id FROM providers) OR NOT EXISTS
            (SELECT 1 FROM provider_sessions p WHERE p.provider_session_id=i.provider_session_id))) OR
        EXISTS(SELECT 1 FROM shutdown_signal_side_effects i WHERE i.intent_state NOT IN ('observed','suppressed') AND (
            i.provider_session_id IN (SELECT id FROM providers) OR NOT EXISTS
            (SELECT 1 FROM provider_sessions p WHERE p.provider_session_id=i.provider_session_id))) OR
        EXISTS(SELECT 1 FROM p080_helper_leases_v1 WHERE id IN (SELECT id FROM helpers) AND (
            terminated_at IS NULL OR termination_reason IS NULL OR termination_reason<>'verified_terminated')) OR
        EXISTS(SELECT 1 FROM p080_helper_lease_members_v1 m JOIN p080_helper_leases_v1 l ON l.id=m.lease_id
            WHERE l.id IN (SELECT id FROM helpers) AND m.terminated_at IS NULL) OR
        EXISTS(SELECT 1 FROM runtime_invocations WHERE agent_execution_id IN
            (SELECT agent_execution_id FROM agent_owners WHERE run_id IN (SELECT run_id FROM owners))
            AND (completed_at IS NULL OR status NOT IN ('completed','failed','cancelled'))) OR
        EXISTS(SELECT 1 FROM side_effects WHERE id IN (SELECT id FROM effects) AND status NOT IN ('settled','reconciled')) OR
        EXISTS(SELECT 1 FROM side_effect_attempts a JOIN side_effects e ON e.id=a.side_effect_id
            WHERE e.id IN (SELECT id FROM effects) AND
            (a.completed_at IS NULL OR a.exit_status IS NULL OR a.exit_status IN ('unknown','timeout'))) OR
        EXISTS(SELECT 1 FROM xcode_effect_attempts x WHERE x.state IN ('prepared','dispatched','unknown') AND (
            json_extract(x.intent_json,'$.run_id') IN (SELECT run_id FROM owners) OR
            NOT EXISTS(SELECT 1 FROM runs WHERE id=json_extract(x.intent_json,'$.run_id')) OR
            EXISTS(SELECT 1 FROM resources r WHERE
                json_extract(x.project_key,'$.canonical_path')=rtrim(r.resource_key,'/') OR
                substr(json_extract(x.project_key,'$.canonical_path'),1,length(rtrim(r.resource_key,'/'))+1)=rtrim(r.resource_key,'/')||'/') OR
            EXISTS(SELECT 1 FROM xcode_effect_attempts prior WHERE json_extract(prior.intent_json,'$.run_id')=?1
                AND json_extract(prior.project_key,'$.uid')=json_extract(x.project_key,'$.uid') AND (
                    json_extract(prior.project_key,'$.canonical_path')=json_extract(x.project_key,'$.canonical_path') OR (
                    json_extract(prior.project_key,'$.device')=json_extract(x.project_key,'$.device') AND
                    json_extract(prior.project_key,'$.inode')=json_extract(x.project_key,'$.inode'))))))
        "#).bind(source).fetch_one(&mut **tx).await.context("continuation quiescence")?;
    ensure!(!busy, "source_busy");
    Ok(())
}

/// The caller supplies a freshly observed, authorized plan. Reservation is the
/// durable admission barrier, not permission to activate or to repeat an effect.
pub async fn reserve(pool: &SqlitePool, request: &Reservation) -> Result<Continuation> {
    reserve_impl(pool, request, None).await
}

/// Production admission: the engine has already checked filesystem ownership;
/// this writer transaction compares the preview's canonical database witness.
pub async fn reserve_checked(
    pool: &SqlitePool,
    request: &Reservation,
    owned_parent: &std::path::Path,
) -> Result<Continuation> {
    ensure!(
        owned_parent.is_absolute() && owned_parent.to_str().is_some(),
        "unsafe_path"
    );
    reserve_impl(pool, request, Some(owned_parent)).await
}

async fn reserve_impl(
    pool: &SqlitePool,
    request: &Reservation,
    owned_parent: Option<&std::path::Path>,
) -> Result<Continuation> {
    ensure!(
        !request.reason.trim().is_empty() && request.reason.len() <= 4096,
        "invalid reason"
    );
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "run_continuations.reserve").await?;
    let previous: Option<Continuation> = sqlx::query_as(
        "SELECT * FROM run_continuations WHERE caller_fingerprint=? AND caller_request_id=?",
    )
    .bind(&request.caller_fingerprint)
    .bind(&request.caller_request_id)
    .fetch_optional(&mut **tx)
    .await?;
    if let Some(previous) = previous {
        ensure!(
            previous.intent_sha256 == request.intent_sha256,
            "idempotency_conflict"
        );
        tx.commit().await?;
        return Ok(previous);
    }
    let unknown_owner: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM run_continuations WHERE worker_claimed=1 AND phase<>'preparing')")
        .fetch_one(&mut **tx).await?;
    ensure!(!unknown_owner, "effect_outcome_unknown");
    let disposition: Option<String> =
        sqlx::query_scalar("SELECT disposition FROM run_execution_fences WHERE source_run_id=?")
            .bind(&request.source_run_id)
            .fetch_optional(&mut **tx)
            .await?;
    check_disposition(disposition.as_deref())?;
    let source: Option<(String, String)> =
        sqlx::query_as("SELECT idea_id,status FROM runs WHERE id=?")
            .bind(&request.source_run_id)
            .fetch_optional(&mut **tx)
            .await?;
    let (idea_id, status) = source.context("source_not_blocked")?;
    ensure!(status == "blocked", "source_not_blocked");
    let competing: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM runs r WHERE idea_id=?1 AND id<>?2 AND status NOT IN ('completed','failed','cancelled') AND NOT EXISTS(SELECT 1 FROM run_execution_fences f WHERE f.source_run_id=r.id)) OR EXISTS(SELECT 1 FROM run_continuations WHERE idea_id=?1 AND phase IN ('preparing','prepared','aborting','needs_reconciliation'))")
        .bind(&idea_id).bind(&request.source_run_id).fetch_one(&mut **tx).await?;
    ensure!(!competing, "continuation_in_progress");
    ensure_quiescent_tx(&mut tx, &request.source_run_id).await?;
    if owned_parent.is_some() {
        let source = domain::ids::RunId(uuid::Uuid::parse_str(&request.source_run_id)?);
        let observed = super::run_continuation_witness::observe(&mut tx, source, None).await?;
        ensure!(
            observed.as_str().strip_prefix("sha256:")
                == Some(request.source_witness_sha256.as_str()),
            "source_changed"
        );
    }
    let queued: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM run_continuations WHERE phase='preparing'")
            .fetch_one(&mut **tx)
            .await?;
    ensure!(queued < 4, "continuation_capacity");
    let operation_id = uuid::Uuid::new_v4().to_string();
    let plan_ref = owned_parent
        .map(|p| {
            p.join(&operation_id)
                .join("plan.json")
                .to_string_lossy()
                .into_owned()
        })
        .unwrap_or_else(|| request.plan_ref.clone());
    let target_ref = owned_parent
        .map(|p| {
            p.join(&operation_id)
                .join("target.json")
                .to_string_lossy()
                .into_owned()
        })
        .unwrap_or_else(|| request.target_ref.clone());
    let successor = uuid::Uuid::new_v4().to_string();
    let journal_id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now();
    let timestamp = now.to_rfc3339();
    let payload = serde_json::json!({"operation_id":operation_id,"source_run_id":request.source_run_id,"plan_sha256":request.plan_sha256,"reason":request.reason}).to_string();
    super::command_journal::record_tx(
        &mut tx,
        &journal_id,
        "ContinueBlocked",
        &payload,
        Some(&request.source_run_id),
        now,
        Some("mcp"),
        Some(&request.caller_fingerprint),
        Some("operator"),
        Some("runs.continue_blocked"),
        Some(&request.caller_request_id),
        Some("operator"),
        None,
        None,
    )
    .await?;
    sqlx::query("INSERT INTO run_continuations(operation_id,source_run_id,idea_id,reserved_successor_run_id,caller_fingerprint,caller_request_id,intent_sha256,profile,plan_ref,plan_sha256,target_ref,source_witness_sha256,journal_id,created_at,updated_at,deadline_at) VALUES (?,?,?,?,?,?,?,'implementation_restart_v1',?,?,?,?,?,?,?,?)")
        .bind(&operation_id).bind(&request.source_run_id).bind(&idea_id).bind(successor)
        .bind(&request.caller_fingerprint).bind(&request.caller_request_id).bind(&request.intent_sha256)
        .bind(&plan_ref).bind(&request.plan_sha256).bind(&target_ref).bind(&request.source_witness_sha256)
        .bind(&journal_id).bind(&timestamp).bind(&timestamp).bind((now+chrono::Duration::minutes(15)).to_rfc3339())
        .execute(&mut **tx).await?;
    sqlx::query("INSERT INTO run_execution_fences(source_run_id,operation_id,generation,disposition,created_at) VALUES (?,?,1,'reserved',?)")
        .bind(&request.source_run_id).bind(&operation_id).bind(&timestamp).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO work_items(id,kind,payload_json,run_id,created_at,scheduled_at) VALUES (?,'prepare_run_continuation',?,NULL,?,?)")
        .bind(uuid::Uuid::new_v4().to_string()).bind(serde_json::json!({"operation_id":operation_id,"generation":1}).to_string())
        .bind(&timestamp).bind(&timestamp).execute(&mut **tx).await?;
    super::command_journal::complete_entry_tx(&mut tx, &journal_id, now).await?;
    let result: Continuation =
        sqlx::query_as("SELECT * FROM run_continuations WHERE operation_id=?")
            .bind(&operation_id)
            .fetch_one(&mut **tx)
            .await?;
    let identity = super::run_continuation_commands::CommandIdentity {
        caller_fingerprint: &request.caller_fingerprint,
        caller_request_id: &request.caller_request_id,
        command: "runs.continue_blocked",
        intent_sha256: &request.intent_sha256,
    };
    let receipt = super::run_continuation_commands::accepted(
        &result,
        &request.caller_request_id,
        &journal_id,
    )?;
    super::run_continuation_commands::record_tx(
        &mut tx,
        &identity,
        &request.source_run_id,
        &receipt,
    )
    .await?;
    tx.commit()
        .await
        .map_err(super::run_continuation_commands::unknown_commit)?;
    Ok(result)
}

async fn get_tx(tx: &mut Transaction<'_, Sqlite>, id: &str) -> Result<Continuation> {
    Ok(
        sqlx::query_as("SELECT * FROM run_continuations WHERE operation_id=?")
            .bind(id)
            .fetch_one(&mut **tx)
            .await?,
    )
}

pub async fn claim_preparation(pool: &SqlitePool) -> Result<Option<Continuation>> {
    claim_preparation_matching(pool, None).await
}

pub async fn claim_preparation_for(pool: &SqlitePool, id: &str) -> Result<Option<Continuation>> {
    claim_preparation_matching(pool, Some(id)).await
}

async fn claim_preparation_matching(
    pool: &SqlitePool,
    id: Option<&str>,
) -> Result<Option<Continuation>> {
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "run_continuations.claim").await?;
    let busy: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM run_continuations WHERE worker_claimed=1)")
            .fetch_one(&mut **tx)
            .await?;
    if busy {
        return Ok(None);
    }
    let candidate: Option<Continuation> = sqlx::query_as("SELECT * FROM run_continuations c WHERE phase='preparing' AND worker_claimed=0 AND (?1 IS NULL OR operation_id=?1) AND EXISTS(SELECT 1 FROM work_items w WHERE w.kind='prepare_run_continuation' AND w.status='pending' AND json_extract(w.payload_json,'$.operation_id')=c.operation_id) ORDER BY created_at,operation_id LIMIT 1")
        .bind(id).fetch_optional(&mut **tx).await?;
    let Some(candidate) = candidate else {
        return Ok(None);
    };
    let expired = chrono::DateTime::parse_from_rfc3339(&candidate.deadline_at)? <= Utc::now();
    let now = Utc::now().to_rfc3339();
    if expired {
        sqlx::query("UPDATE run_continuations SET phase='needs_reconciliation',version=version+1,holds_json='[\"continuation_budget_exceeded\"]',updated_at=? WHERE operation_id=?")
            .bind(&now).bind(&candidate.operation_id).execute(&mut **tx).await?;
        sqlx::query("UPDATE work_items SET status='cancelled',completed_at=? WHERE kind='prepare_run_continuation' AND json_extract(payload_json,'$.operation_id')=? AND status='pending'")
            .bind(&now).bind(&candidate.operation_id).execute(&mut **tx).await?;
        tx.commit().await?;
        return Ok(None);
    }
    sqlx::query("UPDATE run_continuations SET worker_claimed=1,version=version+1,updated_at=? WHERE operation_id=?")
        .bind(&now).bind(&candidate.operation_id).execute(&mut **tx).await?;
    sqlx::query("UPDATE work_items SET status='running',started_at=?,attempt_count=attempt_count+1 WHERE kind='prepare_run_continuation' AND json_extract(payload_json,'$.operation_id')=? AND status='pending'")
        .bind(&now).bind(&candidate.operation_id).execute(&mut **tx).await?;
    let claim = get_tx(&mut tx, &candidate.operation_id).await?;
    tx.commit().await?;
    Ok(Some(claim))
}

pub async fn begin_step(
    pool: &SqlitePool,
    id: &str,
    generation: i64,
    key: &str,
    intent_json: &str,
    destination: &str,
) -> Result<()> {
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "run_continuations.step").await?;
    let op = get_tx(&mut tx, id).await?;
    ensure!(
        op.phase == "preparing" && op.generation == generation && op.worker_claimed,
        "continuation_stale_worker"
    );
    ensure!(
        chrono::DateTime::parse_from_rfc3339(&op.deadline_at)? > Utc::now(),
        "continuation_budget_exceeded"
    );
    sqlx::query("INSERT INTO run_continuation_steps(operation_id,step_key,generation,status,intent_json,destination_identity,updated_at) VALUES (?,?,?,'dispatching',?,?,?)")
        .bind(id).bind(key).bind(generation).bind(intent_json).bind(destination).bind(Utc::now().to_rfc3339()).execute(&mut **tx).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn finish_step(
    pool: &SqlitePool,
    id: &str,
    generation: i64,
    key: &str,
    receipt_sha256: &str,
) -> Result<()> {
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "run_continuations.step").await?;
    let op = get_tx(&mut tx, id).await?;
    ensure!(
        op.phase == "preparing" && op.generation == generation && op.worker_claimed,
        "continuation_stale_worker"
    );
    let changed = sqlx::query("UPDATE run_continuation_steps SET status='verified',receipt_sha256=?,updated_at=? WHERE operation_id=? AND step_key=? AND generation=? AND status='dispatching'")
        .bind(receipt_sha256).bind(Utc::now().to_rfc3339()).bind(id).bind(key).bind(generation).execute(&mut **tx).await?.rows_affected();
    ensure!(changed == 1, "effect_outcome_unknown");
    tx.commit().await?;
    Ok(())
}

/// Startup cannot establish child settlement. Keep the worker hold and revoke
/// generation; only explicit reconciliation can admit subsequent work.
pub async fn recover_preparations(pool: &SqlitePool) -> Result<()> {
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "run_continuations.recover").await?;
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE run_continuation_steps SET status='unknown',updated_at=? WHERE status='dispatching' AND operation_id IN (SELECT operation_id FROM run_continuations WHERE phase IN ('preparing','aborting'))")
        .bind(&now).execute(&mut **tx).await?;
    sqlx::query("UPDATE run_continuations SET phase=CASE WHEN phase='aborting' THEN 'aborting' ELSE 'needs_reconciliation' END,version=version+1,generation=generation+1,holds_json='[\"effect_outcome_unknown\"]',updated_at=? WHERE phase IN ('preparing','aborting')")
        .bind(&now).execute(&mut **tx).await?;
    sqlx::query("UPDATE work_items SET status='cancelled',completed_at=? WHERE kind='prepare_run_continuation' AND status IN ('pending','running')")
        .bind(&now).execute(&mut **tx).await?;
    tx.commit().await?;
    Ok(())
}

/// An uncertain effect cannot establish child settlement. Retain the claim and
/// fence until explicit reconciliation; do not turn an error into worker release.
pub async fn hold_preparation(
    pool: &SqlitePool,
    id: &str,
    generation: i64,
    reason: &str,
) -> Result<()> {
    ensure!(
        matches!(
            reason,
            "effect_outcome_unknown"
                | "source_changed"
                | "prepared_material_changed"
                | "continuation_budget_exceeded"
        ),
        "invalid continuation hold"
    );
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "run_continuations.recover").await?;
    let op = get_tx(&mut tx, id).await?;
    ensure!(
        op.generation == generation && op.phase == "preparing" && op.worker_claimed,
        "continuation_stale_worker"
    );
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE run_continuation_steps SET status='unknown',updated_at=? WHERE operation_id=? AND status='dispatching'")
        .bind(&now).bind(id).execute(&mut **tx).await?;
    sqlx::query("UPDATE run_continuations SET phase='needs_reconciliation',version=version+1,holds_json=?,updated_at=? WHERE operation_id=?")
        .bind(serde_json::to_string(&[reason])?).bind(&now).bind(id).execute(&mut **tx).await?;
    sqlx::query("UPDATE work_items SET status='cancelled',completed_at=? WHERE kind='prepare_run_continuation' AND json_extract(payload_json,'$.operation_id')=? AND status IN ('pending','running')")
        .bind(&now).bind(id).execute(&mut **tx).await?;
    tx.commit().await?;
    Ok(())
}

/// Persist loss of an in-process queued job without claiming or replaying it.
/// A claimed, revoked or already settled operation cannot be changed by this path.
pub async fn hold_queued_preparation(
    pool: &SqlitePool,
    id: &str,
    generation: i64,
    reason: &str,
) -> Result<()> {
    ensure!(
        matches!(
            reason,
            "effect_outcome_unknown"
                | "source_changed"
                | "prepared_material_changed"
                | "continuation_budget_exceeded"
        ),
        "invalid continuation hold"
    );
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "run_continuations.recover").await?;
    let now = Utc::now().to_rfc3339();
    let changed = sqlx::query("UPDATE run_continuations SET phase='needs_reconciliation',version=version+1,holds_json=?,updated_at=? WHERE operation_id=? AND generation=? AND phase='preparing' AND worker_claimed=0")
        .bind(serde_json::to_string(&[reason])?).bind(&now).bind(id).bind(generation)
        .execute(&mut **tx).await?.rows_affected();
    ensure!(changed == 1, "continuation_stale_worker");
    sqlx::query("UPDATE work_items SET status='cancelled',completed_at=? WHERE kind='prepare_run_continuation' AND json_extract(payload_json,'$.operation_id')=? AND status='pending'")
        .bind(&now).bind(id).execute(&mut **tx).await?;
    tx.commit()
        .await
        .map_err(super::run_continuation_commands::unknown_commit)?;
    Ok(())
}

pub async fn begin_abort(pool: &SqlitePool, id: &str, version: i64) -> Result<Continuation> {
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "run_continuations.abort").await?;
    let result = begin_abort_tx(&mut tx, id, version).await?;
    tx.commit().await?;
    Ok(result)
}

async fn begin_abort_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    version: i64,
) -> Result<Continuation> {
    let op = get_tx(tx, id).await?;
    ensure!(op.version == version, "stale_operation_version");
    ensure!(
        matches!(
            op.phase.as_str(),
            "preparing" | "prepared" | "needs_reconciliation"
        ),
        "source_continued"
    );
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE run_continuations SET phase='aborting',version=version+1,generation=generation+1,updated_at=? WHERE operation_id=?")
        .bind(&now).bind(id).execute(&mut **tx).await?;
    sqlx::query("UPDATE work_items SET status='cancelled',completed_at=? WHERE kind='prepare_run_continuation' AND json_extract(payload_json,'$.operation_id')=? AND status='pending'")
        .bind(&now).bind(id).execute(&mut **tx).await?;
    get_tx(tx, id).await
}

pub async fn abort_command(
    pool: &SqlitePool,
    id: &str,
    version: i64,
    identity: &super::run_continuation_commands::CommandIdentity<'_>,
) -> Result<domain::run_carry_forward_api::RunContinuationCommandResultV1> {
    ensure!(
        identity.command == "runs.continuation_abort",
        "invalid continuation command"
    );
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "run_continuations.abort").await?;
    if let Some(previous) = super::run_continuation_commands::find_tx(&mut tx, identity).await? {
        return Ok(previous.replayed()?);
    }
    let mut op = get_tx(&mut tx, id).await?;
    ensure!(op.version == version, "stale_operation_version");
    if op.phase != "aborting" {
        op = begin_abort_tx(&mut tx, id, version).await?;
    }
    let unresolved:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM run_continuation_steps WHERE operation_id=?1 AND status IN ('dispatching','unknown')) OR EXISTS(SELECT 1 FROM work_items WHERE kind='prepare_run_continuation' AND json_extract(payload_json,'$.operation_id')=?1 AND status IN ('pending','running'))")
        .bind(id).fetch_one(&mut **tx).await?;
    if !op.worker_claimed && !unresolved {
        op = finish_abort_tx(&mut tx, id, op.version).await?;
    }
    let journal = record_operation_command_tx(&mut tx, &op, identity).await?;
    let result =
        super::run_continuation_commands::accepted(&op, identity.caller_request_id, &journal)?;
    super::run_continuation_commands::record_tx(&mut tx, identity, &op.source_run_id, &result)
        .await?;
    tx.commit()
        .await
        .map_err(super::run_continuation_commands::unknown_commit)?;
    Ok(result)
}

/// Called only by the tracked owner after its child handles are settled. It
/// cannot turn unknown effects into verified effects or release a source fence.
pub async fn settle_worker(pool: &SqlitePool, id: &str, worker_generation: i64) -> Result<()> {
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "run_continuations.settle_worker")
            .await?;
    let op = get_tx(&mut tx, id).await?;
    ensure!(
        op.worker_claimed
            && (op.generation == worker_generation
                || (op.phase == "aborting" && op.generation == worker_generation + 1)),
        "continuation_stale_worker"
    );
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE run_continuation_steps SET status='unknown',updated_at=? WHERE operation_id=? AND status='dispatching'")
        .bind(&now).bind(id).execute(&mut **tx).await?;
    sqlx::query("UPDATE run_continuations SET worker_claimed=0,phase=CASE WHEN phase='aborting' THEN 'aborting' ELSE 'needs_reconciliation' END,version=version+1,updated_at=? WHERE operation_id=?")
        .bind(&now).bind(id).execute(&mut **tx).await?;
    sqlx::query("UPDATE work_items SET status='cancelled',completed_at=? WHERE kind='prepare_run_continuation' AND json_extract(payload_json,'$.operation_id')=? AND status='running'")
        .bind(&now).bind(id).execute(&mut **tx).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn finish_abort(pool: &SqlitePool, id: &str, version: i64) -> Result<Continuation> {
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "run_continuations.abort").await?;
    let result = finish_abort_tx(&mut tx, id, version).await?;
    tx.commit().await?;
    Ok(result)
}

async fn finish_abort_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    version: i64,
) -> Result<Continuation> {
    let op = get_tx(tx, id).await?;
    ensure!(op.version == version, "stale_operation_version");
    ensure!(
        op.phase == "aborting" && !op.worker_claimed,
        "effect_outcome_unknown"
    );
    let unresolved: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM run_continuation_steps WHERE operation_id=?1 AND status IN ('dispatching','unknown')) OR EXISTS(SELECT 1 FROM work_items WHERE kind='prepare_run_continuation' AND json_extract(payload_json,'$.operation_id')=?1 AND status IN ('pending','running'))")
        .bind(id).fetch_one(&mut **tx).await?;
    ensure!(!unresolved, "effect_outcome_unknown");
    sqlx::query("UPDATE run_continuations SET phase='aborted',version=version+1,holds_json='[]',updated_at=? WHERE operation_id=?")
        .bind(Utc::now().to_rfc3339()).bind(id).execute(&mut **tx).await?;
    sqlx::query("DELETE FROM run_execution_fences WHERE operation_id=?")
        .bind(id)
        .execute(&mut **tx)
        .await?;
    get_tx(tx, id).await
}

pub async fn mark_prepared(
    pool: &SqlitePool,
    id: &str,
    generation: i64,
    manifest_ref: &str,
    manifest_sha256: &str,
) -> Result<Continuation> {
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "run_continuations.prepared").await?;
    let op = get_tx(&mut tx, id).await?;
    ensure!(
        op.phase == "preparing" && op.generation == generation && op.worker_claimed,
        "continuation_stale_worker"
    );
    ensure!(
        chrono::DateTime::parse_from_rfc3339(&op.deadline_at)? > Utc::now(),
        "continuation_budget_exceeded"
    );
    let (count,unverified): (i64,i64) = sqlx::query_as("SELECT COUNT(*),COALESCE(SUM(status<>'verified' OR generation<>?2),0) FROM run_continuation_steps WHERE operation_id=?1")
        .bind(id).bind(generation).fetch_one(&mut **tx).await?;
    ensure!(count > 0 && unverified == 0, "effect_outcome_unknown");
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE run_continuations SET phase='prepared',version=version+1,worker_claimed=0,manifest_ref=?,manifest_sha256=?,updated_at=? WHERE operation_id=?")
        .bind(manifest_ref).bind(manifest_sha256).bind(&now).bind(id).execute(&mut **tx).await?;
    sqlx::query("UPDATE work_items SET status='completed',completed_at=? WHERE kind='prepare_run_continuation' AND json_extract(payload_json,'$.operation_id')=? AND status='running'")
        .bind(&now).bind(id).execute(&mut **tx).await?;
    let result = get_tx(&mut tx, id).await?;
    tx.commit().await?;
    Ok(result)
}

/// Storage half of activation. The engine must validate the fresh filesystem,
/// compiler, delivery and approval-entry witnesses before entering this method.
/// No public adapter may construct a successor directly from caller JSON.
pub async fn activate(
    pool: &SqlitePool,
    id: &str,
    version: i64,
    manifest_sha256: &str,
    successor: &domain::run::Run,
) -> Result<Continuation> {
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "run_continuations.activate").await?;
    let result = activate_tx(&mut tx, id, version, manifest_sha256, successor).await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn activate_command(
    pool: &SqlitePool,
    id: &str,
    version: i64,
    manifest_sha256: &str,
    successor: &domain::run::Run,
    identity: &super::run_continuation_commands::CommandIdentity<'_>,
) -> Result<domain::run_carry_forward_api::RunContinuationCommandResultV1> {
    activate_command_impl(
        pool,
        id,
        version,
        manifest_sha256,
        successor,
        None,
        identity,
    )
    .await
}

pub async fn activate_checked_command(
    pool: &SqlitePool,
    id: &str,
    version: i64,
    manifest_sha256: &str,
    successor: &domain::run::Run,
    inputs: &[super::run_continuation_inputs::PreparedInput],
    identity: &super::run_continuation_commands::CommandIdentity<'_>,
) -> Result<domain::run_carry_forward_api::RunContinuationCommandResultV1> {
    activate_command_impl(
        pool,
        id,
        version,
        manifest_sha256,
        successor,
        Some(inputs),
        identity,
    )
    .await
}

async fn activate_command_impl(
    pool: &SqlitePool,
    id: &str,
    version: i64,
    manifest_sha256: &str,
    successor: &domain::run::Run,
    inputs: Option<&[super::run_continuation_inputs::PreparedInput]>,
    identity: &super::run_continuation_commands::CommandIdentity<'_>,
) -> Result<domain::run_carry_forward_api::RunContinuationCommandResultV1> {
    ensure!(
        identity.command == "runs.continuation_activate",
        "invalid continuation command"
    );
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "run_continuations.activate").await?;
    if let Some(previous) = super::run_continuation_commands::find_tx(&mut tx, identity).await? {
        return Ok(previous.replayed()?);
    }
    if let Some(inputs) = inputs {
        let op = get_tx(&mut tx, id).await?;
        ensure!(op.version == version, "stale_operation_version");
        let source = domain::ids::RunId(uuid::Uuid::parse_str(&op.source_run_id)?);
        let observed = super::run_continuation_witness::observe(&mut tx, source, Some(id)).await?;
        ensure!(
            observed.as_str().strip_prefix("sha256:") == Some(op.source_witness_sha256.as_str()),
            "source_changed"
        );
        super::run_continuation_inputs::insert_tx(&mut tx, &op, inputs).await?;
    }
    let op = activate_tx(&mut tx, id, version, manifest_sha256, successor).await?;
    let journal = record_operation_command_tx(&mut tx, &op, identity).await?;
    let result =
        super::run_continuation_commands::accepted(&op, identity.caller_request_id, &journal)?;
    super::run_continuation_commands::record_tx(&mut tx, identity, &op.source_run_id, &result)
        .await?;
    tx.commit()
        .await
        .map_err(super::run_continuation_commands::unknown_commit)?;
    Ok(result)
}

/// Records explicit verification, never retries or infers that an owner settled.
/// A None hold is valid only for an already prepared, fully verified operation.
pub async fn reconcile_command(
    pool: &SqlitePool,
    id: &str,
    version: i64,
    hold: Option<domain::run_carry_forward_api::ContinuationReasonCode>,
    identity: &super::run_continuation_commands::CommandIdentity<'_>,
) -> Result<domain::run_carry_forward_api::RunContinuationCommandResultV1> {
    use domain::run_carry_forward_api::ContinuationReasonCode as Code;
    ensure!(
        identity.command == "runs.continuation_reconcile",
        "invalid continuation command"
    );
    ensure!(
        hold.is_none()
            || matches!(
                hold,
                Some(
                    Code::EffectOutcomeUnknown
                        | Code::SourceChanged
                        | Code::PreparedMaterialChanged
                        | Code::ContinuationBudgetExceeded
                )
            ),
        "invalid continuation hold"
    );
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "run_continuations.reconcile").await?;
    if let Some(previous) = super::run_continuation_commands::find_tx(&mut tx, identity).await? {
        return Ok(previous.replayed()?);
    }
    let before = get_tx(&mut tx, id).await?;
    ensure!(before.version == version, "stale_operation_version");
    ensure!(
        matches!(
            before.phase.as_str(),
            "prepared" | "needs_reconciliation" | "aborting"
        ),
        "effect_outcome_unknown"
    );
    let unresolved:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM run_continuation_steps WHERE operation_id=? AND status<>'verified')")
        .bind(id).fetch_one(&mut **tx).await?;
    if hold.is_none() {
        ensure!(
            before.phase == "prepared" && !before.worker_claimed && !unresolved,
            "effect_outcome_unknown"
        );
    }
    if let Some(hold) = hold {
        sqlx::query("UPDATE run_continuations SET phase=CASE WHEN phase='aborting' THEN phase ELSE 'needs_reconciliation' END,version=version+1,holds_json=?,updated_at=? WHERE operation_id=?")
            .bind(serde_json::to_string(&[hold])?).bind(Utc::now().to_rfc3339()).bind(id).execute(&mut **tx).await?;
    }
    let op = get_tx(&mut tx, id).await?;
    let journal = record_operation_command_tx(&mut tx, &op, identity).await?;
    let result =
        super::run_continuation_commands::accepted(&op, identity.caller_request_id, &journal)?;
    super::run_continuation_commands::record_tx(&mut tx, identity, &op.source_run_id, &result)
        .await?;
    tx.commit()
        .await
        .map_err(super::run_continuation_commands::unknown_commit)?;
    Ok(result)
}

/// Reconciliation can finish an explicit abort only from positive canonical
/// settlement evidence. It neither adopts unknown effects nor resumes the source.
pub async fn reconcile_abort_command(
    pool: &SqlitePool,
    id: &str,
    version: i64,
    identity: &super::run_continuation_commands::CommandIdentity<'_>,
) -> Result<domain::run_carry_forward_api::RunContinuationCommandResultV1> {
    ensure!(
        identity.command == "runs.continuation_reconcile",
        "invalid continuation command"
    );
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "run_continuations.reconcile").await?;
    if let Some(previous) = super::run_continuation_commands::find_tx(&mut tx, identity).await? {
        return Ok(previous.replayed()?);
    }
    let before = get_tx(&mut tx, id).await?;
    ensure!(before.version == version, "stale_operation_version");
    ensure!(before.phase == "aborting", "effect_outcome_unknown");
    let unresolved:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM run_continuation_steps WHERE operation_id=?1 AND status<>'verified') OR EXISTS(SELECT 1 FROM work_items WHERE kind='prepare_run_continuation' AND json_extract(payload_json,'$.operation_id')=?1 AND status IN ('pending','running'))")
        .bind(id).fetch_one(&mut **tx).await?;
    let op = if !before.worker_claimed && !unresolved {
        finish_abort_tx(&mut tx, id, version).await?
    } else {
        sqlx::query("UPDATE run_continuations SET version=version+1,holds_json='[\"effect_outcome_unknown\"]',updated_at=? WHERE operation_id=?")
            .bind(Utc::now().to_rfc3339()).bind(id).execute(&mut **tx).await?;
        get_tx(&mut tx, id).await?
    };
    let journal = record_operation_command_tx(&mut tx, &op, identity).await?;
    let result =
        super::run_continuation_commands::accepted(&op, identity.caller_request_id, &journal)?;
    super::run_continuation_commands::record_tx(&mut tx, identity, &op.source_run_id, &result)
        .await?;
    tx.commit()
        .await
        .map_err(super::run_continuation_commands::unknown_commit)?;
    Ok(result)
}

/// The service has reverified the complete manifest and current target. A file
/// alone is insufficient: only a settled owner and its verified receipt qualify.
pub async fn reconcile_prepared_command(
    pool: &SqlitePool,
    id: &str,
    version: i64,
    manifest_ref: &str,
    manifest_sha256: &str,
    identity: &super::run_continuation_commands::CommandIdentity<'_>,
) -> Result<domain::run_carry_forward_api::RunContinuationCommandResultV1> {
    ensure!(
        identity.command == "runs.continuation_reconcile",
        "invalid continuation command"
    );
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "run_continuations.reconcile").await?;
    if let Some(previous) = super::run_continuation_commands::find_tx(&mut tx, identity).await? {
        return Ok(previous.replayed()?);
    }
    let op = get_tx(&mut tx, id).await?;
    ensure!(op.version == version, "stale_operation_version");
    ensure!(
        op.phase == "needs_reconciliation" && !op.worker_claimed,
        "effect_outcome_unknown"
    );
    let expected = std::path::Path::new(&op.plan_ref)
        .parent()
        .context("unsafe_path")?
        .join("manifest.json");
    ensure!(
        expected == std::path::Path::new(manifest_ref),
        "unsafe_path"
    );
    let (count,unverified):(i64,i64)=sqlx::query_as("SELECT COUNT(*),COALESCE(SUM(status<>'verified' OR generation<>?2),0) FROM run_continuation_steps WHERE operation_id=?1")
        .bind(id).bind(op.generation).fetch_one(&mut **tx).await?;
    ensure!(count > 0 && unverified == 0, "effect_outcome_unknown");
    let receipt:Option<String>=sqlx::query_scalar("SELECT receipt_sha256 FROM run_continuation_steps WHERE operation_id=? AND step_key='selected_inputs_and_target' AND status='verified'")
        .bind(id).fetch_optional(&mut **tx).await?.flatten();
    ensure!(
        receipt.as_deref() == Some(manifest_sha256),
        "prepared_material_changed"
    );
    let queued:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM work_items WHERE kind='prepare_run_continuation' AND json_extract(payload_json,'$.operation_id')=? AND status IN ('pending','running'))")
        .bind(id).fetch_one(&mut **tx).await?;
    ensure!(!queued, "effect_outcome_unknown");
    ensure_quiescent_tx(&mut tx, &op.source_run_id).await?;
    let source = domain::ids::RunId(uuid::Uuid::parse_str(&op.source_run_id)?);
    let observed = super::run_continuation_witness::observe(&mut tx, source, Some(id)).await?;
    ensure!(
        observed.as_str().strip_prefix("sha256:") == Some(op.source_witness_sha256.as_str()),
        "source_changed"
    );
    sqlx::query("UPDATE run_continuations SET phase='prepared',version=version+1,manifest_ref=?,manifest_sha256=?,holds_json='[]',updated_at=? WHERE operation_id=?")
        .bind(manifest_ref).bind(manifest_sha256).bind(Utc::now().to_rfc3339()).bind(id).execute(&mut **tx).await?;
    let op = get_tx(&mut tx, id).await?;
    let journal = record_operation_command_tx(&mut tx, &op, identity).await?;
    let result =
        super::run_continuation_commands::accepted(&op, identity.caller_request_id, &journal)?;
    super::run_continuation_commands::record_tx(&mut tx, identity, &op.source_run_id, &result)
        .await?;
    tx.commit()
        .await
        .map_err(super::run_continuation_commands::unknown_commit)?;
    Ok(result)
}

async fn record_operation_command_tx(
    tx: &mut Transaction<'_, Sqlite>,
    op: &Continuation,
    identity: &super::run_continuation_commands::CommandIdentity<'_>,
) -> Result<String> {
    let journal = uuid::Uuid::new_v4().to_string();
    let now = Utc::now();
    super::command_journal::record_tx(
        tx,
        &journal,
        identity.command,
        &serde_json::json!({"operation_id":op.operation_id,"version":op.version}).to_string(),
        Some(&op.source_run_id),
        now,
        Some("mcp"),
        Some(identity.caller_fingerprint),
        Some("operator"),
        Some(identity.command),
        Some(identity.caller_request_id),
        Some("operator"),
        None,
        None,
    )
    .await?;
    super::command_journal::complete_entry_tx(tx, &journal, now).await?;
    Ok(journal)
}

async fn activate_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    version: i64,
    manifest_sha256: &str,
    successor: &domain::run::Run,
) -> Result<Continuation> {
    let op = get_tx(tx, id).await?;
    ensure!(op.version == version, "stale_operation_version");
    ensure!(
        op.phase == "prepared" && !op.worker_claimed,
        "effect_outcome_unknown"
    );
    ensure!(
        op.manifest_sha256.as_deref() == Some(manifest_sha256),
        "prepared_material_changed"
    );
    ensure!(
        successor.id.to_string() == op.reserved_successor_run_id
            && successor.idea_id.to_string() == op.idea_id,
        "destination_conflict"
    );
    ensure!(
        successor.status == domain::run::RunStatus::Pending && successor.current_state.is_some(),
        "target_profile_invalid"
    );
    let source_status: String = sqlx::query_scalar("SELECT status FROM runs WHERE id=?")
        .bind(&op.source_run_id)
        .fetch_one(&mut **tx)
        .await?;
    ensure!(source_status == "blocked", "source_not_blocked");
    ensure_quiescent_tx(tx, &op.source_run_id).await?;
    let changed=sqlx::query("UPDATE run_execution_fences SET disposition='historical' WHERE operation_id=? AND source_run_id=? AND disposition='reserved'")
        .bind(id).bind(&op.source_run_id).execute(&mut **tx).await?.rows_affected();
    ensure!(changed == 1, "continuation_in_progress");
    super::runs::insert_tx(tx, successor).await?;
    sqlx::query("UPDATE run_continuation_inputs SET installed=1,successor_run_id=? WHERE operation_id=? AND installed=0")
        .bind(successor.id.to_string()).bind(id).execute(&mut **tx).await?;
    let now = Utc::now();
    super::work_items::enqueue_tx(
        tx,
        &crate::work_item::WorkItem {
            id: uuid::Uuid::new_v4().to_string(),
            kind: crate::work_item::WorkItemKind::AdvanceRun,
            payload_json: serde_json::json!({"run_id":successor.id.to_string()}).to_string(),
            status: crate::work_item::WorkItemStatus::Pending,
            run_id: Some(successor.id),
            stage_id: None,
            created_at: now,
            scheduled_at: now,
            attempt_count: 0,
            last_error: None,
        },
    )
    .await?;
    sqlx::query("UPDATE run_continuations SET phase='activated',version=version+1,successor_run_id=?,updated_at=? WHERE operation_id=?")
        .bind(successor.id.to_string()).bind(now.to_rfc3339()).bind(id).execute(&mut **tx).await?;
    get_tx(tx, id).await
}
