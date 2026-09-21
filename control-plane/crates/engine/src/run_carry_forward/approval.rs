//! Ordinary approval authority for activated P039 successors.
use anyhow::{ensure, Context, Result};
use db::repos::{
    approvals, run_continuation_approvals as bindings, run_continuations, runs, stages,
};
use domain::{
    approval::{Approval, ApprovalDecision},
    ids::{ApprovalId, RunId, StageExecutionId},
    run::Run,
    run_carry_forward::{canonical_digest, ApprovalEvidenceV1, ContentDigest},
    stage::StageExecution,
};
use sqlx::SqlitePool;
use std::path::Path;

use super::{manifest::db_digest, read_manifest, ManifestV1, PreviewOptions, WorkspacePlanner};

pub struct ResolutionEvidence {
    pub approval_id: ApprovalId,
    pub stage: StageExecutionId,
    pub evidence: ApprovalEvidenceV1,
}

async fn context(
    pool: &SqlitePool,
    run: RunId,
) -> Result<Option<(Run, run_continuations::Continuation, ManifestV1)>> {
    let Some(operation) = run_continuations::links(pool, &run.to_string())
        .await?
        .incoming
    else {
        return Ok(None);
    };
    let current = runs::find_by_id(pool, run)
        .await?
        .context("stale_approval_binding: run missing")?;
    // Activation freeze is deliberately not reused: successor code and proposal
    // may evolve while immutable lineage/reference evidence remains pinned.
    let manifest = read_manifest(&operation).await?;
    ensure!(
        current.id == manifest.reserved_successor_run_id
            && current.worktree_root.as_deref() == manifest.workspace.checkout_root.to_str()
            && Path::new(&current.artifact_root) == manifest.workspace.metadata_root,
        "stale_approval_binding: successor roots"
    );
    ensure!(
        current.workflow_snapshot_hash.as_deref()
            == Some(manifest.target.plan.workflow_snapshot_hash.as_str())
            && current.catalog_snapshot_hash.as_deref()
                == Some(manifest.target.plan.catalog_snapshot_hash.as_str())
            && current.workflow_snapshot_json.as_deref()
                == Some(manifest.target.plan.workflow_snapshot_json.as_str())
            && current.catalog_snapshot_json.as_deref()
                == Some(manifest.target.plan.catalog_snapshot_json.as_str()),
        "stale_approval_binding: frozen policy"
    );
    Ok(Some((current, operation, manifest)))
}

async fn evidence(
    pool: &SqlitePool,
    run: &Run,
    operation: &run_continuations::Continuation,
    manifest: &ManifestV1,
    started_code: Option<ContentDigest>,
) -> Result<ApprovalEvidenceV1> {
    let proposal = super::inputs::resolve_execution_input(
        pool,
        run,
        &manifest.target.plan,
        &manifest.target.profile.proposal_input,
    )
    .await?
    .context("stale_approval_binding: proposal input missing")?;
    let proposal_sha256 = ContentDigest::of(proposal.content.as_bytes());
    let code_sha256 = match started_code {
        Some(digest) => digest,
        None => {
            WorkspacePlanner::preview(&manifest.workspace.checkout_root, PreviewOptions::default())
                .await?
                .digest()
                .clone()
        }
    };
    Ok(ApprovalEvidenceV1 {
        source_run_id: manifest.source_run_id,
        successor_run_id: run.id,
        manifest_sha256: db_digest(
            operation
                .manifest_sha256
                .as_deref()
                .context("stale_approval_binding: manifest")?,
        )?,
        proposal_sha256,
        code_sha256,
        workflow_snapshot_hash: db_digest(&manifest.target.plan.workflow_snapshot_hash)?,
        catalog_snapshot_hash: db_digest(&manifest.target.plan.catalog_snapshot_hash)?,
        capability_delta_sha256: manifest.capability_delta_sha256.clone(),
        unresolved_findings_sha256: manifest.unresolved_findings_sha256.clone(),
    })
}

/// All ordinary gates keep their existing behavior except the declared P039 gate.
pub async fn create(pool: &SqlitePool, approval: &Approval, stage: StageExecutionId) -> Result<()> {
    if let Some((run, operation, manifest)) = context(pool, approval.run_id).await? {
        if manifest.target.profile.approval_state == approval.stage_id {
            let evidence = evidence(pool, &run, &operation, &manifest, None).await?;
            return bindings::create(pool, approval, stage, &evidence).await;
        }
    }
    approvals::insert(pool, approval).await
}

/// Ordinary retry of the stage-status/request-publication crash gap. Never
/// synthesizes authority for an old grant or replaces inconsistent evidence.
pub async fn repair_missing(pool: &SqlitePool, stage: &StageExecution) -> Result<Option<Approval>> {
    let Some((run, operation, manifest)) = context(pool, stage.run_id).await? else {
        return Ok(None);
    };
    if manifest.target.profile.approval_state != stage.stage_id {
        return Ok(None);
    }
    let fresh = evidence(pool, &run, &operation, &manifest, None).await?;
    let approval = Approval {
        id: ApprovalId::new(),
        run_id: run.id,
        stage_id: stage.stage_id.clone(),
        decision: ApprovalDecision::Requested,
        requested_at: chrono::Utc::now(),
        decided_at: None,
        comment: None,
        expires_at: None,
    };
    Ok(bindings::repair_missing(pool, &approval, stage.id, &fresh)
        .await?
        .then_some(approval))
}

/// Filesystem/Git reads stay outside the ordinary command writer transaction.
/// The caller must validate the returned tuple inside that transaction.
pub async fn resolution_evidence(
    pool: &SqlitePool,
    run_id: RunId,
    logical_stage: &str,
) -> Result<Option<ResolutionEvidence>> {
    let Some((run, operation, manifest)) = context(pool, run_id).await? else {
        return Ok(None);
    };
    if manifest.target.profile.approval_state != logical_stage {
        return Ok(None);
    }
    let stage = stages::list_by_run(pool, run_id)
        .await?
        .into_iter()
        .filter(|s| s.stage_id == logical_stage)
        .last()
        .context("stale_approval_binding: stage missing")?;
    ensure!(
        stage.status == domain::stage::StageStatus::WaitingApproval,
        "stale_approval_binding: stage not awaiting approval"
    );
    let binding = bindings::current(pool, run_id, logical_stage)
        .await?
        .context("stale_approval_binding: binding missing")?;
    ensure!(
        binding.stage_execution_id == stage.id.to_string(),
        "stale_approval_binding: prior gate execution"
    );
    Ok(Some(ResolutionEvidence {
        approval_id: binding.approval_id.parse()?,
        stage: stage.id,
        evidence: evidence(pool, &run, &operation, &manifest, None).await?,
    }))
}

/// None means a non-P039 gate. Errors must fail closed, never fall back to the
/// legacy any-grant-for-this-logical-state rule.
pub async fn authorizes(
    pool: &SqlitePool,
    run_id: RunId,
    logical_stage: &str,
    decision: ApprovalDecision,
) -> Result<Option<bool>> {
    let Some((run, operation, manifest)) = context(pool, run_id).await? else {
        return Ok(None);
    };
    if manifest.target.profile.approval_state != logical_stage {
        return Ok(None);
    }
    let Some(stage) = stages::list_by_run(pool, run_id)
        .await?
        .into_iter()
        .filter(|s| s.stage_id == logical_stage)
        .last()
    else {
        return Ok(Some(false));
    };
    let evidence = evidence(pool, &run, &operation, &manifest, None).await?;
    Ok(Some(
        bindings::authorizes(pool, run_id, logical_stage, stage.id, &evidence, decision).await?,
    ))
}

/// Recheck immediately before persisting the selected transition, including
/// transitions restored from declarative conditions by the ordinary engine.
pub async fn validate_transition(
    pool: &SqlitePool,
    run_id: RunId,
    from: &str,
    to: &str,
) -> Result<()> {
    let Some((run, operation, manifest)) = context(pool, run_id).await? else {
        return Ok(());
    };
    if from != manifest.target.profile.approval_state {
        return Ok(());
    }
    let stage = stages::list_by_run(pool, run_id)
        .await?
        .into_iter()
        .filter(|s| s.stage_id == from)
        .last()
        .context("stale_approval_binding: stage missing")?;
    let decision = if to == manifest.target.profile.preparation_state {
        ApprovalDecision::Granted
    } else {
        ApprovalDecision::Rejected
    };
    let fresh = evidence(pool, &run, &operation, &manifest, None).await?;
    ensure!(
        bindings::authorizes(pool, run_id, from, stage.id, &fresh, decision).await?,
        "stale_approval_binding"
    );
    Ok(())
}

/// Snapshot authorization is durable, but does not prove execution started.
pub async fn authorize_snapshot(pool: &SqlitePool, stage: &StageExecution) -> Result<()> {
    let Some((run, operation, manifest)) = context(pool, stage.run_id).await? else {
        return Ok(());
    };
    if stage.stage_id != manifest.target.profile.preparation_state {
        return Ok(());
    }
    let binding = bindings::current(pool, run.id, &manifest.target.profile.approval_state)
        .await?
        .context("stale_approval_binding: missing current entry")?;
    let gate: StageExecutionId = binding.stage_execution_id.parse()?;
    let latest = stages::list_by_run(pool, run.id)
        .await?
        .into_iter()
        .filter(|s| s.stage_id == manifest.target.profile.approval_state)
        .last()
        .context("stale_approval_binding: gate missing")?;
    ensure!(
        gate == latest.id,
        "stale_approval_binding: prior gate execution"
    );
    let original: ApprovalEvidenceV1 = serde_json::from_str(&binding.evidence_json)?;
    let started_code = binding
        .first_execution_started_at
        .as_ref()
        .map(|_| original.code_sha256);
    let fresh = evidence(pool, &run, &operation, &manifest, started_code).await?;
    ensure!(
        run.current_state.as_deref() == Some(stage.stage_id.as_str()),
        "stale_approval_binding: dispatch outside current entry"
    );
    let entry = format!("post-gate:{}:{}", gate, stage.stage_id);
    bindings::consume(pool, binding.approval_id.parse()?, gate, &fresh, &entry)
        .await
        .context("stale_approval_binding: snapshot authorization")
}

/// Queue publication pins the exact snapshot, without relaxing code freshness.
pub async fn before_dispatch(pool: &SqlitePool, stage: &StageExecution) -> Result<()> {
    authorize_snapshot(pool, stage).await?;
    let Some((run, _, manifest)) = context(pool, stage.run_id).await? else {
        return Ok(());
    };
    if stage.stage_id != manifest.target.profile.preparation_state {
        return Ok(());
    }
    let binding = bindings::current(pool, run.id, &manifest.target.profile.approval_state)
        .await?
        .context("stale_approval_binding: entry missing")?;
    let snapshot = snapshot_identity(pool, &run, &manifest).await?;
    bindings::pin_snapshot(
        pool,
        binding.approval_id.parse()?,
        binding
            .consumed_transition_id
            .as_deref()
            .context("stale_approval_binding: entry missing")?,
        &snapshot,
        &stage.stage_id,
    )
    .await
}

async fn snapshot_identity(pool: &SqlitePool, run: &Run, manifest: &ManifestV1) -> Result<String> {
    let super::inputs::OutputResolution::Output(snapshot) =
        super::inputs::output_for_predicate(pool, run, &manifest.target.plan, "approved_proposal")
            .await?
    else {
        anyhow::bail!("stale_approval_binding: snapshot missing");
    };
    ensure!(
        snapshot.origin == super::inputs::InputOrigin::ApprovalSnapshot,
        "stale_approval_binding: snapshot origin"
    );
    Ok(snapshot
        .artifact_id
        .context("stale_approval_binding: snapshot identity")?
        .to_string())
}

/// The frozen preparation task consumes the verified engine snapshot; it must
/// not request that same artifact as fresh provider-authored output.
pub async fn owns_preparation_snapshot(
    pool: &SqlitePool,
    run_id: RunId,
    logical_stage: &str,
    task_name: &str,
) -> Result<bool> {
    let Some((run, _, manifest)) = context(pool, run_id).await? else {
        return Ok(false);
    };
    if logical_stage != manifest.target.profile.preparation_state
        || task_name != manifest.target.profile.preparation_task
    {
        return Ok(false);
    }
    ensure!(
        run.current_state.as_deref() == Some(logical_stage),
        "stale_approval_binding: preparation outside current entry"
    );
    let binding = bindings::current(pool, run.id, &manifest.target.profile.approval_state)
        .await?
        .context("stale_approval_binding: entry missing")?;
    let snapshot = snapshot_identity(pool, &run, &manifest).await?;
    ensure!(
        binding.snapshot_artifact_id.as_deref() == Some(snapshot.as_str()),
        "stale_approval_binding: preparation snapshot identity"
    );
    Ok(true)
}

/// The actual executor boundary rechecks filesystem bytes after queue claim.
pub async fn before_execution(pool: &SqlitePool, req: &acp::ExecutionRequest) -> Result<()> {
    let Some((run, operation, manifest)) = context(pool, req.run_id).await? else {
        return Ok(());
    };
    if req.stage_id != manifest.target.profile.preparation_state {
        return Ok(());
    }
    let stage = stages::find_by_id(
        pool,
        req.stage_execution_id
            .as_deref()
            .context("stale_approval_binding: execution stage")?
            .parse()?,
    )
    .await?
    .context("stale_approval_binding: execution stage")?;
    ensure!(
        stage.run_id == run.id && stage.stage_id == req.stage_id,
        "stale_approval_binding: execution owner"
    );
    authorize_snapshot(pool, &stage).await?;
    let binding = bindings::current(pool, run.id, &manifest.target.profile.approval_state)
        .await?
        .context("stale_approval_binding: entry missing")?;
    let snapshot = snapshot_identity(pool, &run, &manifest).await?;
    let original: ApprovalEvidenceV1 = serde_json::from_str(&binding.evidence_json)?;
    let fresh = evidence(
        pool,
        &run,
        &operation,
        &manifest,
        binding
            .first_execution_started_at
            .as_ref()
            .map(|_| original.code_sha256),
    )
    .await?;
    bindings::admit_execution(
        pool,
        binding.approval_id.parse()?,
        binding
            .consumed_transition_id
            .as_deref()
            .context("stale_approval_binding: entry missing")?,
        &snapshot,
        stage.id,
        req.agent_execution_id
            .context("stale_approval_binding: execution missing")?,
        &fresh,
    )
    .await
}

/// Only the production PromptSent sink can establish started authority. Do not
/// hash code here: the authorized provider may already be producing changes.
pub async fn record_prompt_sent(
    pool: &SqlitePool,
    update: &acp::AcpPromptProgressUpdate,
) -> Result<()> {
    let Some((run, _, manifest)) = context(pool, update.run_id).await? else {
        return Ok(());
    };
    if update.stage_id != manifest.target.profile.preparation_state {
        return Ok(());
    }
    let binding = bindings::current(pool, run.id, &manifest.target.profile.approval_state)
        .await?
        .context("stale_approval_binding: entry missing")?;
    if binding.first_execution_started_at.is_some() {
        return Ok(());
    }
    bindings::record_prompt_sent(
        pool,
        binding.approval_id.parse()?,
        binding
            .consumed_transition_id
            .as_deref()
            .context("stale_approval_binding: entry missing")?,
        binding
            .snapshot_artifact_id
            .as_deref()
            .context("stale_approval_binding: snapshot missing")?,
        update
            .stage_execution_id
            .as_deref()
            .context("stale_approval_binding: execution stage")?
            .parse()?,
        update
            .agent_execution_id
            .context("stale_approval_binding: execution missing")?,
    )
    .await
}

/// Read-only evidence for an already-consumed entry, e.g. fresh approved-proposal
/// snapshot publication. Code may evolve only after durable PromptSent evidence.
/// None is only for ordinary runs; P039 missing authority is an error.
pub async fn consumed_entry_evidence(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<Option<ApprovalEvidenceV1>> {
    let Some((run, operation, manifest)) = context(pool, run_id).await? else {
        return Ok(None);
    };
    let binding = bindings::current(pool, run_id, &manifest.target.profile.approval_state)
        .await?
        .context("stale_approval_binding: missing current entry")?;
    let stage = stages::list_by_run(pool, run_id)
        .await?
        .into_iter()
        .filter(|s| s.stage_id == manifest.target.profile.approval_state)
        .last()
        .context("stale_approval_binding: gate missing")?;
    let entry = format!(
        "post-gate:{}:{}",
        stage.id, manifest.target.profile.preparation_state
    );
    ensure!(
        binding.stage_execution_id == stage.id.to_string()
            && binding.consumed_transition_id.as_deref() == Some(entry.as_str()),
        "stale_approval_binding: entry not consumed"
    );
    let approval = approvals::find_by_id(pool, binding.approval_id.parse()?)
        .await?
        .context("stale_approval_binding")?;
    ensure!(
        approval.decision == ApprovalDecision::Granted,
        "approval_not_actionable"
    );
    let original: ApprovalEvidenceV1 = serde_json::from_str(&binding.evidence_json)?;
    let fresh = evidence(
        pool,
        &run,
        &operation,
        &manifest,
        binding
            .first_execution_started_at
            .as_ref()
            .map(|_| original.code_sha256.clone()),
    )
    .await?;
    ensure!(
        fresh.code_sha256 == original.code_sha256,
        "stale_approval_binding: code changed before PromptSent"
    );
    ensure!(
        canonical_digest(&fresh)?.as_str().strip_prefix("sha256:")
            == Some(binding.evidence_sha256.as_str()),
        "stale_approval_binding: entry tuple"
    );
    Ok(Some(fresh))
}
