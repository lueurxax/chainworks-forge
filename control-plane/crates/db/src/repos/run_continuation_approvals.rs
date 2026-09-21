//! Exact-stage approval evidence for activated carry-forward successors.
use anyhow::{ensure, Context, Result};
use domain::{
    approval::{Approval, ApprovalDecision},
    ids::{ApprovalId, RunId, StageExecutionId},
    run_carry_forward::{canonical_digest, ApprovalEvidenceV1, ContentDigest},
};
use sqlx::{FromRow, Sqlite, SqlitePool, Transaction};

#[derive(Debug, Clone, FromRow)]
pub struct BindingRecord {
    pub approval_id: String,
    pub successor_run_id: String,
    pub stage_execution_id: String,
    pub logical_stage_id: String,
    pub evidence_json: String,
    pub evidence_sha256: String,
    pub generation: i64,
    pub status: String,
    pub consumed_transition_id: Option<String>,
    pub snapshot_artifact_id: Option<String>,
    pub first_execution_id: Option<String>,
    pub first_execution_started_at: Option<String>,
}

fn hex(digest: &ContentDigest) -> &str {
    digest
        .as_str()
        .strip_prefix("sha256:")
        .expect("validated ContentDigest")
}

pub async fn current(pool: &SqlitePool, run: RunId, stage: &str) -> Result<Option<BindingRecord>> {
    Ok(sqlx::query_as("SELECT * FROM run_continuation_approval_bindings WHERE successor_run_id=? AND logical_stage_id=? AND status='current'")
        .bind(run.to_string()).bind(stage).fetch_optional(pool).await?)
}

pub async fn create(
    pool: &SqlitePool,
    approval: &Approval,
    stage: StageExecutionId,
    evidence: &ApprovalEvidenceV1,
) -> Result<()> {
    create_checked(pool, approval, stage, evidence, false).await?;
    Ok(())
}

/// Repair only an absent atomic request. Existing inconsistent authority holds;
/// concurrent repairs observe the winner without creating another generation.
pub async fn repair_missing(
    pool: &SqlitePool,
    approval: &Approval,
    stage: StageExecutionId,
    evidence: &ApprovalEvidenceV1,
) -> Result<bool> {
    create_checked(pool, approval, stage, evidence, true).await
}

async fn create_checked(
    pool: &SqlitePool,
    approval: &Approval,
    stage: StageExecutionId,
    evidence: &ApprovalEvidenceV1,
    repair: bool,
) -> Result<bool> {
    ensure!(
        approval.run_id == evidence.successor_run_id
            && evidence.source_run_id != evidence.successor_run_id,
        "stale_approval_binding"
    );
    ensure!(
        matches!(
            approval.decision,
            ApprovalDecision::Pending | ApprovalDecision::Requested
        ),
        "approval_not_actionable"
    );
    let evidence_json = serde_json::to_string(evidence)?;
    let digest = canonical_digest(evidence)?;
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "run_continuation_approvals.create")
            .await?;
    let ownership:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM run_continuations c JOIN runs r ON r.id=c.successor_run_id JOIN stage_executions s ON s.run_id=r.id WHERE c.phase='activated' AND c.source_run_id=?1 AND c.successor_run_id=?2 AND c.manifest_sha256=?3 AND r.workflow_snapshot_hash=?4 AND r.catalog_snapshot_hash=?5 AND s.id=?6 AND s.stage_id=?7 AND s.status='waiting_approval')")
        .bind(evidence.source_run_id.to_string()).bind(evidence.successor_run_id.to_string()).bind(hex(&evidence.manifest_sha256))
        .bind(hex(&evidence.workflow_snapshot_hash)).bind(hex(&evidence.catalog_snapshot_hash)).bind(stage.to_string()).bind(&approval.stage_id).fetch_one(&mut **tx).await?;
    ensure!(ownership, "stale_approval_binding");
    validate_latest_stage_tx(&mut tx, approval.run_id, &approval.stage_id, stage).await?;
    if repair {
        let current: Option<BindingRecord> = sqlx::query_as("SELECT * FROM run_continuation_approval_bindings WHERE successor_run_id=? AND logical_stage_id=? AND status='current'")
            .bind(approval.run_id.to_string()).bind(&approval.stage_id).fetch_optional(&mut **tx).await?;
        if let Some(current) = current {
            if current.stage_execution_id == stage.to_string() {
                validate_resolution_tx(&mut tx, current.approval_id.parse()?, stage, evidence)
                    .await?;
                tx.commit().await?;
                return Ok(false);
            }
            // A failed later publication rolls supersession back. Only an
            // intact older terminal gate may be superseded during repair.
            let original: ApprovalEvidenceV1 = serde_json::from_str(&current.evidence_json)?;
            ensure!(
                hex(&canonical_digest(&original)?) == current.evidence_sha256
                    && original.source_run_id == evidence.source_run_id
                    && original.successor_run_id == evidence.successor_run_id
                    && original.manifest_sha256 == evidence.manifest_sha256
                    && original.workflow_snapshot_hash == evidence.workflow_snapshot_hash
                    && original.catalog_snapshot_hash == evidence.catalog_snapshot_hash,
                "stale_approval_binding: inconsistent prior gate"
            );
            let terminal: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM approvals a JOIN stage_executions s ON s.id=? WHERE a.id=? AND a.run_id=? AND a.stage_id=? AND a.decision IN ('granted','rejected') AND a.decided_at IS NOT NULL AND s.run_id=a.run_id AND s.stage_id=a.stage_id AND s.status='completed')")
                .bind(&current.stage_execution_id).bind(&current.approval_id).bind(approval.run_id.to_string()).bind(&approval.stage_id).fetch_one(&mut **tx).await?;
            ensure!(terminal, "stale_approval_binding: prior gate not terminal");
        }
        let inconsistent: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM run_continuation_approval_bindings WHERE stage_execution_id=?1) OR EXISTS(SELECT 1 FROM approvals WHERE run_id=?2 AND stage_id=?3 AND decision IN ('pending','requested'))")
            .bind(stage.to_string()).bind(approval.run_id.to_string()).bind(&approval.stage_id).fetch_one(&mut **tx).await?;
        ensure!(
            !inconsistent,
            "stale_approval_binding: incomplete existing request"
        );
    }
    let existing: Option<BindingRecord> =
        sqlx::query_as("SELECT * FROM run_continuation_approval_bindings WHERE approval_id=?")
            .bind(approval.id.to_string())
            .fetch_optional(&mut **tx)
            .await?;
    if let Some(existing) = existing {
        ensure!(
            existing.evidence_sha256 == hex(&digest)
                && existing.stage_execution_id == stage.to_string()
                && existing.successor_run_id == approval.run_id.to_string(),
            "idempotency_conflict"
        );
        tx.commit().await?;
        return Ok(false);
    }
    let generation:i64=sqlx::query_scalar("SELECT COALESCE(MAX(generation),0)+1 FROM run_continuation_approval_bindings WHERE successor_run_id=? AND logical_stage_id=?")
        .bind(approval.run_id.to_string()).bind(&approval.stage_id).fetch_one(&mut **tx).await?;
    sqlx::query("UPDATE run_continuation_approval_bindings SET status='superseded' WHERE successor_run_id=? AND logical_stage_id=? AND status='current'")
        .bind(approval.run_id.to_string()).bind(&approval.stage_id).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO approvals(id,run_id,stage_id,decision,requested_at,expires_at) VALUES (?,?,?,?,?,?)")
        .bind(approval.id.to_string()).bind(approval.run_id.to_string()).bind(&approval.stage_id).bind(approval.decision.to_string())
        .bind(approval.requested_at.to_rfc3339()).bind(approval.expires_at.map(|t|t.to_rfc3339())).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO run_continuation_approval_bindings(approval_id,successor_run_id,stage_execution_id,logical_stage_id,evidence_json,evidence_sha256,generation,status) VALUES (?,?,?,?,?,?,?,'current')")
        .bind(approval.id.to_string()).bind(approval.run_id.to_string()).bind(stage.to_string()).bind(&approval.stage_id)
        .bind(evidence_json).bind(hex(&digest)).bind(generation).execute(&mut **tx).await?;
    tx.commit().await?;
    Ok(true)
}

pub async fn validate_resolution_tx(
    tx: &mut Transaction<'_, Sqlite>,
    approval: ApprovalId,
    stage: StageExecutionId,
    evidence: &ApprovalEvidenceV1,
) -> Result<()> {
    validate_owner_tx(tx, evidence).await?;
    let digest = canonical_digest(evidence)?;
    let binding: Option<BindingRecord> = sqlx::query_as(
        "SELECT * FROM run_continuation_approval_bindings WHERE approval_id=? AND status='current'",
    )
    .bind(approval.to_string())
    .fetch_optional(&mut **tx)
    .await?;
    let binding = binding.context("stale_approval_binding")?;
    ensure!(
        binding.stage_execution_id == stage.to_string()
            && binding.successor_run_id == evidence.successor_run_id.to_string()
            && binding.evidence_sha256 == hex(&digest)
            && binding.consumed_transition_id.is_none(),
        "stale_approval_binding"
    );
    let row = super::approvals::find_by_id_tx(tx, approval)
        .await?
        .context("approval_not_actionable")?;
    ensure!(
        matches!(
            row.decision,
            ApprovalDecision::Pending | ApprovalDecision::Requested
        ) && row.expires_at.is_none_or(|at| at > chrono::Utc::now()),
        "approval_not_actionable"
    );
    let latest = validate_latest_stage_tx(tx, row.run_id, &row.stage_id, stage).await?;
    ensure!(
        latest.status == domain::stage::StageStatus::WaitingApproval,
        "stale_approval_binding"
    );
    Ok(())
}

pub async fn authorizes(
    pool: &SqlitePool,
    run: RunId,
    logical_stage: &str,
    stage: StageExecutionId,
    evidence: &ApprovalEvidenceV1,
    decision: ApprovalDecision,
) -> Result<bool> {
    ensure!(
        matches!(
            decision,
            ApprovalDecision::Granted | ApprovalDecision::Rejected
        ),
        "approval_not_actionable"
    );
    let digest = canonical_digest(evidence)?;
    Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM run_continuation_approval_bindings b JOIN approvals a ON a.id=b.approval_id WHERE b.successor_run_id=? AND b.logical_stage_id=? AND b.stage_execution_id=? AND b.status='current' AND b.consumed_transition_id IS NULL AND b.evidence_sha256=? AND a.decision=?)")
        .bind(run.to_string()).bind(logical_stage).bind(stage.to_string()).bind(hex(&digest)).bind(decision.to_string()).fetch_one(pool).await?)
}

pub async fn consume(
    pool: &SqlitePool,
    approval: ApprovalId,
    stage: StageExecutionId,
    evidence: &ApprovalEvidenceV1,
    transition_id: &str,
) -> Result<()> {
    ensure!(
        !transition_id.is_empty() && transition_id.len() <= 256,
        "approval_not_actionable"
    );
    let digest = canonical_digest(evidence)?;
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "run_continuation_approvals.consume")
            .await?;
    validate_owner_tx(&mut tx, evidence).await?;
    let binding: BindingRecord = sqlx::query_as(
        "SELECT * FROM run_continuation_approval_bindings WHERE approval_id=? AND status='current'",
    )
    .bind(approval.to_string())
    .fetch_optional(&mut **tx)
    .await?
    .context("stale_approval_binding")?;
    ensure!(
        binding.stage_execution_id == stage.to_string()
            && binding.successor_run_id == evidence.successor_run_id.to_string(),
        "stale_approval_binding"
    );
    validate_latest_stage_tx(
        &mut tx,
        evidence.successor_run_id,
        &binding.logical_stage_id,
        stage,
    )
    .await?;
    let decision: String = sqlx::query_scalar("SELECT decision FROM approvals WHERE id=?")
        .bind(approval.to_string())
        .fetch_one(&mut **tx)
        .await?;
    ensure!(decision == "granted", "approval_not_actionable");
    if let Some(consumed) = binding.consumed_transition_id {
        ensure!(consumed == transition_id, "approval_not_actionable");
        // Snapshot authorization alone never proves execution started.
        let mut original: ApprovalEvidenceV1 = serde_json::from_str(&binding.evidence_json)?;
        if binding.first_execution_started_at.is_some() {
            original.code_sha256 = evidence.code_sha256.clone();
        }
        ensure!(
            canonical_digest(&original)? == digest,
            "stale_approval_binding"
        );
    } else {
        ensure!(
            binding.evidence_sha256 == hex(&digest),
            "stale_approval_binding"
        );
        sqlx::query("UPDATE run_continuation_approval_bindings SET consumed_transition_id=? WHERE approval_id=? AND consumed_transition_id IS NULL")
            .bind(transition_id).bind(approval.to_string()).execute(&mut **tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Pin the engine snapshot once, before queue publication. The engine verifies
/// the no-follow bytes; this transaction binds their durable artifact identity.
pub async fn pin_snapshot(
    pool: &SqlitePool,
    approval: ApprovalId,
    entry: &str,
    snapshot: &str,
    preparation: &str,
) -> Result<()> {
    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "run_continuation_approvals.pin_snapshot",
    )
    .await?;
    let binding = consumed_binding_tx(&mut tx, approval, entry).await?;
    let evidence: ApprovalEvidenceV1 = serde_json::from_str(&binding.evidence_json)?;
    let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM artifacts WHERE id=? AND run_id=? AND stage_id=? AND name='approved_proposal' AND contract_id='approved_proposal' AND is_pinned=1 AND agent_id='engine' AND provider='engine' AND agent_execution_id IS NULL AND checksum_sha256=?)")
        .bind(snapshot).bind(&binding.successor_run_id).bind(preparation).bind(hex(&evidence.proposal_sha256)).fetch_one(&mut **tx).await?;
    ensure!(
        valid
            && binding
                .snapshot_artifact_id
                .as_deref()
                .is_none_or(|id| id == snapshot),
        "stale_approval_binding: snapshot identity"
    );
    sqlx::query("UPDATE run_continuation_approval_bindings SET snapshot_artifact_id=? WHERE approval_id=? AND snapshot_artifact_id IS NULL")
        .bind(snapshot).bind(approval.to_string()).execute(&mut **tx).await?;
    tx.commit().await?;
    Ok(())
}

/// Admission is not started evidence. A crashed/failed admission must recheck
/// the original code tuple before another execution can replace it.
pub async fn admit_execution(
    pool: &SqlitePool,
    approval: ApprovalId,
    entry: &str,
    snapshot: &str,
    stage: StageExecutionId,
    execution: domain::ids::AgentExecutionId,
    evidence: &ApprovalEvidenceV1,
) -> Result<()> {
    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "run_continuation_approvals.admit_execution",
    )
    .await?;
    let binding = consumed_binding_tx(&mut tx, approval, entry).await?;
    ensure!(
        binding.snapshot_artifact_id.as_deref() == Some(snapshot),
        "stale_approval_binding: snapshot identity"
    );
    let mut original: ApprovalEvidenceV1 = serde_json::from_str(&binding.evidence_json)?;
    if binding.first_execution_started_at.is_some() {
        original.code_sha256 = evidence.code_sha256.clone();
    }
    ensure!(
        canonical_digest(&original)? == canonical_digest(evidence)?,
        "stale_approval_binding"
    );
    let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM agent_executions a JOIN stage_executions s ON s.id=a.stage_execution_id JOIN runs r ON r.id=s.run_id JOIN artifacts p ON p.id=? AND p.run_id=s.run_id AND p.stage_id=s.stage_id WHERE a.id=? AND a.status='running' AND s.id=? AND s.run_id=? AND s.stage_id=r.current_state AND s.status='running' AND p.name='approved_proposal' AND p.contract_id='approved_proposal' AND p.is_pinned=1 AND p.agent_id='engine' AND p.provider='engine' AND p.agent_execution_id IS NULL AND p.checksum_sha256=?)")
        .bind(snapshot).bind(execution.to_string()).bind(stage.to_string()).bind(&binding.successor_run_id).bind(hex(&evidence.proposal_sha256)).fetch_one(&mut **tx).await?;
    ensure!(valid, "stale_approval_binding: execution owner");
    if binding.first_execution_started_at.is_none() {
        if let Some(prior) = binding.first_execution_id.as_deref() {
            let running: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM agent_executions WHERE id=? AND status='running')",
            )
            .bind(prior)
            .fetch_one(&mut **tx)
            .await?;
            ensure!(
                prior == execution.to_string() || !running,
                "stale_approval_binding: competing first execution"
            );
        }
        sqlx::query("UPDATE run_continuation_approval_bindings SET first_execution_id=? WHERE approval_id=? AND first_execution_started_at IS NULL")
            .bind(execution.to_string()).bind(approval.to_string()).execute(&mut **tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Called only by the production PromptSent progress sink. Queue/running rows
/// cannot substitute for this exact admitted-execution CAS.
pub async fn record_prompt_sent(
    pool: &SqlitePool,
    approval: ApprovalId,
    entry: &str,
    snapshot: &str,
    stage: StageExecutionId,
    execution: domain::ids::AgentExecutionId,
) -> Result<()> {
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "run_continuation_approvals.prompt_sent")
            .await?;
    let binding = consumed_binding_tx(&mut tx, approval, entry).await?;
    ensure!(
        binding.snapshot_artifact_id.as_deref() == Some(snapshot),
        "stale_approval_binding: snapshot identity"
    );
    ensure!(
        binding.first_execution_id.as_deref() == Some(execution.to_string().as_str()),
        "stale_approval_binding: execution identity"
    );
    let owner: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM agent_executions a JOIN stage_executions s ON s.id=a.stage_execution_id JOIN artifacts p ON p.id=? AND p.run_id=s.run_id AND p.stage_id=s.stage_id WHERE a.id=? AND s.id=? AND s.run_id=?)")
        .bind(snapshot).bind(execution.to_string()).bind(stage.to_string()).bind(&binding.successor_run_id).fetch_one(&mut **tx).await?;
    ensure!(owner, "stale_approval_binding: execution owner");
    if binding.first_execution_started_at.is_some() {
        tx.commit().await?;
        return Ok(());
    }
    let changed = sqlx::query("UPDATE run_continuation_approval_bindings SET first_execution_started_at=? WHERE approval_id=? AND status='current' AND consumed_transition_id=? AND snapshot_artifact_id=? AND first_execution_id=? AND first_execution_started_at IS NULL AND EXISTS(SELECT 1 FROM agent_executions a JOIN stage_executions s ON s.id=a.stage_execution_id JOIN runs r ON r.id=s.run_id WHERE a.id=? AND a.status='running' AND s.id=? AND s.run_id=successor_run_id AND s.stage_id=r.current_state AND s.status='running')")
        .bind(chrono::Utc::now().to_rfc3339()).bind(approval.to_string()).bind(entry).bind(snapshot).bind(execution.to_string())
        .bind(execution.to_string()).bind(stage.to_string()).execute(&mut **tx).await?.rows_affected();
    ensure!(
        changed == 1,
        "stale_approval_binding: first execution was not admitted"
    );
    tx.commit().await?;
    Ok(())
}

async fn consumed_binding_tx(
    tx: &mut Transaction<'_, Sqlite>,
    approval: ApprovalId,
    entry: &str,
) -> Result<BindingRecord> {
    let binding: BindingRecord = sqlx::query_as("SELECT * FROM run_continuation_approval_bindings WHERE approval_id=? AND status='current' AND consumed_transition_id=?")
        .bind(approval.to_string()).bind(entry).fetch_optional(&mut **tx).await?.context("stale_approval_binding: consumed entry")?;
    let evidence: ApprovalEvidenceV1 = serde_json::from_str(&binding.evidence_json)?;
    ensure!(
        hex(&canonical_digest(&evidence)?) == binding.evidence_sha256,
        "stale_approval_binding: stored evidence"
    );
    validate_owner_tx(tx, &evidence).await?;
    validate_latest_stage_tx(
        tx,
        evidence.successor_run_id,
        &binding.logical_stage_id,
        binding.stage_execution_id.parse()?,
    )
    .await?;
    let granted: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM approvals WHERE id=? AND run_id=? AND stage_id=? AND decision='granted')")
        .bind(approval.to_string()).bind(&binding.successor_run_id).bind(&binding.logical_stage_id).fetch_one(&mut **tx).await?;
    ensure!(granted, "approval_not_actionable");
    Ok(binding)
}

async fn validate_owner_tx(
    tx: &mut Transaction<'_, Sqlite>,
    evidence: &ApprovalEvidenceV1,
) -> Result<()> {
    let current:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM run_continuations c JOIN runs r ON r.id=c.successor_run_id WHERE c.phase='activated' AND c.source_run_id=? AND c.successor_run_id=? AND c.manifest_sha256=? AND r.workflow_snapshot_hash=? AND r.catalog_snapshot_hash=?)")
        .bind(evidence.source_run_id.to_string()).bind(evidence.successor_run_id.to_string())
        .bind(hex(&evidence.manifest_sha256)).bind(hex(&evidence.workflow_snapshot_hash)).bind(hex(&evidence.catalog_snapshot_hash))
        .fetch_one(&mut **tx).await?;
    ensure!(current, "stale_approval_binding");
    Ok(())
}

async fn validate_latest_stage_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run: RunId,
    logical_stage: &str,
    stage: StageExecutionId,
) -> Result<domain::stage::StageExecution> {
    let latest = super::stages::list_by_run_tx(tx, run)
        .await?
        .into_iter()
        .filter(|candidate| candidate.stage_id == logical_stage)
        .last()
        .context("stale_approval_binding")?;
    ensure!(latest.id == stage, "stale_approval_binding");
    Ok(latest)
}
