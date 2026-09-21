//! Launch-only authority for operation-owned metadata outside the repository.
//! This does not grant input, approval, workspace, or provider authority.

use acp::{ApprovedMetadataRoot, ExecutionRequest};
use anyhow::{ensure, Context, Result};
use db::repos::{run_continuations, runs};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};

/// Re-mint for each durable execution. Serialized requests never restore this
/// capability. Ordinary runs retain ACP's original workspace confinement.
pub async fn approve_metadata_root(pool: &SqlitePool, req: &mut ExecutionRequest) -> Result<()> {
    req.approved_metadata_root = None;
    let Some(operation) = run_continuations::links(pool, &req.run_id.to_string())
        .await?
        .incoming
    else {
        return Ok(());
    };
    let manifest = super::read_manifest(&operation).await?;
    let run = runs::find_by_id(pool, req.run_id)
        .await?
        .context("approved metadata root: run missing")?;
    let execution_id = req
        .agent_execution_id
        .context("approved metadata root: durable execution required")?;
    ensure!(
        operation.phase == "activated"
            && operation.successor_run_id.as_deref() == Some(run.id.to_string().as_str())
            && operation.idea_id == run.idea_id.to_string()
            && run.id == manifest.reserved_successor_run_id
            && run.idea_id == manifest.idea_id
            && run.workspace_root == manifest.successor.workspace_root
            && run.worktree_root.as_deref() == manifest.workspace.checkout_root.to_str()
            && run.chainworks_meta_root.as_deref() == manifest.workspace.metadata_root.to_str()
            && Path::new(&run.artifact_root) == manifest.workspace.metadata_root
            && req.workspace_root == run.workspace_root
            && req.worktree_root == run.worktree_root
            && req.chainworks_meta_root == run.chainworks_meta_root,
        "approved metadata root: canonical successor roots changed"
    );
    // Resolve the durable execution's actual owner, not an ID supplied only by
    // the request. Mediation and ordinary stage ownership keep their DB meaning.
    let owned: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM agent_executions a WHERE a.id=? AND a.status='running'
         AND a.agent_id=? AND a.provider=? AND a.stage_execution_id IS ?
         AND (EXISTS(SELECT 1 FROM stage_executions s WHERE s.id=a.stage_execution_id AND s.run_id=?)
           OR EXISTS(SELECT 1 FROM lead_conflict_mediations m WHERE m.id=a.lead_mediation_record_id AND m.run_id=?)))",
    )
    .bind(execution_id.to_string())
    .bind(&req.agent_id)
    .bind(&req.provider)
    .bind(&req.stage_execution_id)
    .bind(run.id.to_string())
    .bind(run.id.to_string())
    .fetch_one(pool)
    .await?;
    ensure!(
        owned,
        "approved metadata root: durable execution owner mismatch"
    );
    let current = run_continuations::find(pool, &operation.operation_id)
        .await?
        .context("approved metadata root: incoming operation missing")?;
    ensure!(
        current.phase == "activated"
            && current.version == operation.version
            && current.generation == operation.generation
            && current.successor_run_id == operation.successor_run_id
            && current.manifest_ref == operation.manifest_ref
            && current.manifest_sha256 == operation.manifest_sha256,
        "approved metadata root: incoming operation changed"
    );
    let workspace = PathBuf::from(run.workspace_root);
    let worktree_identity = *manifest
        .workspace
        .directory_identities
        .get("checkout")
        .context("approved metadata root: missing checkout identity")?;
    let metadata_identity = *manifest
        .workspace
        .directory_identities
        .get("metadata")
        .context("approved metadata root: missing metadata identity")?;
    let checkout = manifest.workspace.checkout_root;
    let metadata = manifest.workspace.metadata_root;
    req.approved_metadata_root = Some(
        tokio::task::spawn_blocking(move || {
            ApprovedMetadataRoot::from_engine_verified_roots(
                run.id,
                execution_id,
                &workspace,
                &checkout,
                &metadata,
                worktree_identity,
                metadata_identity,
            )
        })
        .await
        .context("approved metadata root: directory validation task failed")??,
    );
    Ok(())
}
